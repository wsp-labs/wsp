// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_writes: slates.ts:464-468, 1017-1093, 1118-1231 at 65f6b89c7. Spec: spec/slates_writes.md.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;

use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use serde_json::{json, Map, Value};
use tokio::task::spawn_local;
use wsp_core::console;
use wsp_wire::types::slate_wire::{SlateBy, SlateCause};
use wsp_wire::views::workspace::{road_of, Caller, WorkspaceOrigin};
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::js::js_trim_start;
use wsp_words::words::computer::thread_word;

use super::batch::{batch_by, BatchOptions};
use super::model::{own_path, problem, refused, snap_of, usage, BatchBy, PreviousSnap, SlateSnap};
use super::reads::waiting_of;
use super::{RecordRef, SlatesInner, StateParams, Views, WriteParams};
use crate::pending::slate::{
    apply_slate_patch, parse_slate, parse_slate_own_path, parse_slate_patch, run_slate_batch, slate_nearest, validate_slate, BatchEvent,
    SlateBatchContext, SlateDoc, SlateProblem, SlateRunDecl, SlateValues, SlateWrite, SLATE_LIMITS,
};
use crate::pending::wire::{SlateEmpty, SlateStateAnswer, SlateWriteAnswer};
use crate::slate_mcp::refused_away;
use crate::slate_read::is_run_record;
use crate::slate_refused::invalid;
use crate::slate_runs::{RunBy, RunRecord, RunState};

fn pr_checks_named(doc: &SlateDoc) -> bool {
    static PR_CHECKS: OnceLock<Regex> = OnceLock::new();
    let text = serde_json::to_string(doc).unwrap_or_default();
    PR_CHECKS.get_or_init(|| Regex::new(r"(?-u:\b)pr\.checks(?-u:\b)").expect("the pr.checks pattern")).is_match(&text)
}

fn coded_usage(happened: &str, fix: &str, code: &str) -> HostError {
    let mut e = usage_refusal(happened, fix);
    e.code = Some(code.to_string());
    e
}

impl SlatesInner {
    /// V751 when the agent's budget is spent.
    pub(crate) fn spend_write(&self, thread_id: &str) -> Result<(), HostError> {
        match self.writes.spend(thread_id) {
            Some(wait) => Err(refused(
                &problem("V751", "write-rate", &format!("wait {wait} s; a slate that rewrites itself constantly is a bug"), None),
                Kind::Conflict,
            )),
            None => Ok(()),
        }
    }

    async fn refused_on(self: &Rc<Self>, thread_id: &str, before: Option<&SlateDoc>, doc: Option<&SlateDoc>) -> Vec<SlateProblem> {
        let away = refused_away(before, doc, self.deps.away(thread_id).as_ref(), self.deps.missing_on(thread_id)).await;
        let mcp = self.mcp();
        mcp.refused_runs(thread_id, before, doc, away).await
    }

    /// Rehearses a document's values and press on a copy of the record; nothing is stored, started or sent.
    pub(crate) async fn rehearse(
        self: Rc<Self>,
        r: RecordRef,
        doc: SlateDoc,
        values: SlateValues,
        p: &WriteParams,
        warnings: Vec<SlateProblem>,
    ) -> Result<SlateWriteAnswer, HostError> {
        let asked: Vec<(String, Value)> =
            p.values.clone().unwrap_or_default().into_iter().map(|(path, value)| (own_path(&path), value)).collect();
        let now = self.deps.clock().now();
        let mut seeded = values;
        for (path, value) in &asked {
            let Some(own) = parse_slate_own_path(path) else { continue };
            if !doc.runs.contains_key(&own.name) {
                continue;
            }
            let was: Map<String, Value> =
                seeded.get(&own.name).filter(|v| is_run_record(Some(*v))).and_then(|v| v.as_object().cloned()).unwrap_or_default();
            let mut base = Map::new();
            base.insert("state".into(), json!("done"));
            base.insert("runs".into(), json!(1));
            for (key, v) in &was {
                base.insert(key.clone(), v.clone());
            }
            if !was.contains_key("endedAt") {
                base.insert("state".into(), json!("done"));
                base.insert("endedAt".into(), json!(now));
            }
            let mut next = seeded.clone();
            next.insert(own.name.clone(), Value::Object(base));
            seeded = crate::pending::slate::set_slate_value(&next, path, value.clone()).unwrap_or(seeded);
        }
        let mut preview = r.borrow().clone();
        let version = preview.version;
        preview.document = Some(doc.clone());
        preview.values = seeded.clone();
        let views = self.clone().views_for(&preview, &[]).await;
        let writes: Vec<SlateWrite> = asked
            .iter()
            .filter(|(path, _)| match parse_slate_own_path(path) {
                Some(own) => !doc.runs.contains_key(&own.name),
                None => true,
            })
            .map(|(path, value)| SlateWrite { path: path.clone(), value: value.clone() })
            .collect();
        if let Some(press) = &p.press {
            if !doc.pieces.contains_key(&press.piece) {
                let pieces: Vec<String> = doc.pieces.keys().cloned().collect();
                let near = slate_nearest(&press.piece, &pieces);
                let note = format!("there is no piece {} to press", press.piece);
                return Err(invalid(&[problem("D203", "piece-missing", &note, near.as_deref())], &warnings));
            }
        }
        let event = p.press.as_ref().map(|press| BatchEvent {
            piece: press.piece.clone(),
            kind: "press".into(),
            item: None,
            index: press.index,
            row_action: press.action,
        });
        let ctx = SlateBatchContext {
            eval: self.context_of(&preview, &views, None),
            by: "person".into(),
            event,
            reaction_sends: false,
            react: None,
            start: None,
        };
        let result = run_slate_batch(&doc, &seeded, writes, ctx);
        let mut after_record = preview.clone();
        after_record.values = result.values.clone();
        let after = self.sketched(&after_record, &views, false, &[], &warnings);
        let mut would: Vec<String> = Vec::new();
        would.extend(result.starts.iter().map(|s| format!("would start ${} ({})", s.run, s.why)));
        would.extend(result.cancels.iter().map(|run| format!("would cancel ${run}")));
        would.extend(result.sends.iter().map(|s| format!("would {} {}", s.kind, serde_json::to_string(&s.text).unwrap_or_default())));
        would.extend(result.other.iter().map(|step| format!("would {step} in the window")));
        let text = if would.is_empty() { after } else { format!("{after}\n{}", would.join("\n")) };
        Ok(SlateWriteAnswer { version, text, warnings, problems: result.problems, waiting: None })
    }

    /// Stores a whole document or a clear, runs its timers and announces it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn write_document(
        self: Rc<Self>,
        r: RecordRef,
        next: Option<SlateDoc>,
        values: SlateValues,
        by: SlateBy,
        cause: SlateCause,
        pieces: Vec<String>,
        warnings: Vec<SlateProblem>,
    ) -> Result<SlateWriteAnswer, HostError> {
        let bytes = serde_json::to_string(&values).map_or(0, |text| text.len());
        if bytes > SLATE_LIMITS.values_bytes {
            let note = format!("the values are {bytes} bytes; a slate holds at most {}", SLATE_LIMITS.values_bytes);
            return Err(invalid(&[problem("S500", "values-too-big", &note, None)], &warnings));
        }
        let old = r.borrow().clone();
        self.drop_declared(&old, next.as_ref());
        {
            let mut rec = r.borrow_mut();
            if rec.version > 0 {
                let snap = snap_of(&rec);
                let version = rec.version;
                rec.previous = Some(PreviousSnap { document: snap.document, values: snap.values, empty: snap.empty, version });
            }
            rec.document = next.clone();
            rec.values = values.clone();
            rec.empty = if next.is_none() { Some(SlateEmpty::Cleared) } else { None };
            rec.version += 1;
            let revision = rec.rev() + 1;
            rec.revision = Some(revision);
        }
        let thread_id = r.borrow().thread_id.clone();
        self.records.borrow_mut().insert(thread_id.clone(), r.clone());
        let snap = r.borrow().clone();
        self.arm_timers(&snap, &HashSet::new());
        if let Some(doc) = &next {
            let asking = self.clone().startable(&snap).await;
            self.held_again(r.clone(), &asking);
            for (run, decl) in &doc.runs {
                let idle = r.borrow().values.get(run).and_then(RunRecord::from_json).is_some_and(|rec| rec.state() == Some(RunState::Idle));
                if decl_has_every(decl) && idle && self.provisional(r.clone(), run).state() == Some(RunState::Held) {
                    let record = self.start_now(r.clone(), run, RunBy::Timer, &asking, None).record;
                    r.borrow_mut().values.insert(run.clone(), record.to_json());
                }
            }
        }
        self.clone().save(r.clone()).await?;
        self.announce(&r, cause, by, &pieces, None);
        if next.as_ref().is_some_and(pr_checks_named) {
            let workspace_id = r.borrow().workspace_id.clone();
            self.deps.watch_pr(&workspace_id);
        }
        let names: Vec<String> = values.keys().cloned().collect();
        self.push(&r, &names);
        let snap = r.borrow().clone();
        let views = self.clone().views_for(&snap, &[]).await;
        Ok(SlateWriteAnswer {
            version: snap.version,
            text: self.sketched(&snap, &views, false, &[], &warnings),
            problems: self.problems_of(&snap, &views),
            warnings,
            waiting: Some(waiting_of(&snap)),
        })
    }

    /// Puts the slate before the last one back; the undo keeps one step.
    pub(crate) async fn undo(self: Rc<Self>, r: RecordRef, by: SlateBy) -> Result<SlateWriteAnswer, HostError> {
        let back = r.borrow().previous.clone().ok_or_else(|| {
            refused(
                &problem("V752", "nothing-to-undo", "there is no earlier slate to go back to; the undo keeps one step", None),
                Kind::Conflict,
            )
        })?;
        let mut rec = r.borrow().clone();
        let current = snap_of(&rec);
        let version = rec.version;
        rec.previous = Some(PreviousSnap { document: current.document, values: current.values, empty: current.empty, version });
        self.become_snap(&mut rec, SlateSnap { document: back.document, values: back.values, empty: back.empty });
        rec.version += 1;
        let revision = rec.rev() + 1;
        rec.revision = Some(revision);
        *r.borrow_mut() = rec;
        self.clone().save(r.clone()).await?;
        self.announce(&r, SlateCause::Undo, by, &[], None);
        let names: Vec<String> = r.borrow().values.keys().cloned().collect();
        self.push(&r, &names);
        let snap = r.borrow().clone();
        let text = self.clone().sketch_of(&snap).await;
        Ok(SlateWriteAnswer { version: snap.version, text, warnings: Vec::new(), problems: Vec::new(), waiting: None })
    }

    /// Writes a whole slate or a patch, checked or stored; refusals before the chain never queue.
    pub(crate) async fn write(self: Rc<Self>, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        let thread_id = self.target_of(p.target.thread_id.as_deref(), p.target.turn_token.as_deref(), caller, true)?;
        let given = usize::from(p.text.is_some()) + usize::from(p.document.is_some());
        let rehearsal = p.values.is_some() || p.press.is_some();
        if rehearsal && p.check != Some(true) {
            return Err(usage_refusal(
                "values and press rehearse a slate, which only a check does.",
                "Add check, or set the live values with slate_state.",
            ));
        }
        if given > 1 || (given == 0 && !rehearsal) {
            let happened = format!(
                "A601 step-arg: a slate write takes exactly one of text or document, and this one has {}.",
                if given == 0 { "none" } else { "both" }
            );
            return Err(coded_usage(&happened, "Send text alone.", "A601"));
        }
        let by = self.by_of(caller);
        let whole = p.document.is_some() || p.text.as_deref().is_some_and(|text| js_trim_start(text).starts_with("<slate"));
        let area = self.clone();
        let chain_thread = thread_id.clone();
        self.serial(&thread_id, move || area.write_on_chain(chain_thread, p, by, whole, given, rehearsal)).await
    }

    async fn write_on_chain(
        self: Rc<Self>,
        thread_id: String,
        p: WriteParams,
        by: SlateBy,
        whole: bool,
        given: usize,
        rehearsal: bool,
    ) -> Result<SlateWriteAnswer, HostError> {
        let r = match self.clone().record_of(&thread_id).await? {
            Some(found) => found,
            None => Rc::new(RefCell::new(self.fresh_record(&thread_id)?)),
        };
        if given == 0 {
            let doc = r.borrow().document.clone();
            let Some(doc) = doc else {
                let note = "this thread has no slate to rehearse; send the <slate> with check";
                return Err(usage(&problem("Z802", "no-slate", note, None)));
            };
            let values = r.borrow().values.clone();
            return self.rehearse(r, doc, values, &p, Vec::new()).await;
        }
        let (before_doc, before_values, version) = {
            let rec = r.borrow();
            (rec.document.clone(), rec.values.clone(), rec.version)
        };
        if whole {
            let checked = match (&p.text, &p.document) {
                (Some(text), _) => parse_slate(text),
                (None, Some(document)) => validate_slate(document),
                (None, None) => validate_slate(&Value::Null),
            };
            let mut errors = checked.errors.clone();
            errors.extend(self.refused_on(&thread_id, before_doc.as_ref(), checked.document.as_ref()).await);
            let Some(doc) = checked.document.clone().filter(|_| errors.is_empty()) else {
                return Err(invalid(&errors, &checked.warnings));
            };
            if rehearsal {
                let values = self.values_for(&thread_id, before_doc.as_ref(), &before_values, &doc);
                return self.rehearse(r, doc, values, &p, checked.warnings).await;
            }
            if p.check == Some(true) {
                let mut preview = r.borrow().clone();
                preview.values = self.values_for(&thread_id, None, &IndexMap::new(), &doc);
                preview.document = Some(doc);
                let text = self.sketched(&preview, &Views::new(), true, &[], &checked.warnings);
                return Ok(SlateWriteAnswer { version, text, warnings: checked.warnings, problems: Vec::new(), waiting: None });
            }
            self.check_version(&r.borrow(), p.if_version)?;
            if by == SlateBy::Agent {
                self.spend_write(&thread_id)?;
            }
            let values = self.values_for(&thread_id, before_doc.as_ref(), &before_values, &doc);
            let pieces: Vec<String> = doc.pieces.keys().cloned().collect();
            return self.write_document(r, Some(doc), values, by, SlateCause::Write, pieces, checked.warnings).await;
        }
        if version == 0 {
            let note = "this thread has no slate to patch; write a whole <slate> first";
            return Err(usage(&problem("Z802", "no-slate", note, None)));
        }
        let text = p.text.clone().unwrap_or_default();
        let parsed = parse_slate_patch(&text, before_doc.as_ref());
        let Some(ops) = parsed.ops.clone().filter(|_| parsed.errors.is_empty()) else {
            return Err(invalid(&parsed.errors, &[]));
        };
        if ops.len() == 1 && ops[0].get("op").and_then(Value::as_str) == Some("undo") {
            if p.check == Some(true) {
                let snap = r.borrow().clone();
                let text = self.clone().sketch_of(&snap).await;
                return Ok(SlateWriteAnswer { version: snap.version, text, warnings: Vec::new(), problems: Vec::new(), waiting: None });
            }
            self.check_version(&r.borrow(), p.if_version)?;
            return self.undo(r, by).await;
        }
        let applied = apply_slate_patch(before_doc.as_ref(), &before_values, &ops, &parsed.lines);
        let next_ref = applied.document.clone().flatten();
        let mut errors = applied.errors.clone();
        errors.extend(self.refused_on(&thread_id, before_doc.as_ref(), next_ref.as_ref()).await);
        let Some(next) = applied.document.clone().filter(|_| errors.is_empty()) else {
            return Err(invalid(&errors, &applied.warnings));
        };
        let values = match &next {
            None => before_values.clone(),
            Some(doc) => {
                let base = applied.values.clone().unwrap_or_else(|| before_values.clone());
                self.values_for(&thread_id, before_doc.as_ref(), &base, doc)
            }
        };
        if rehearsal {
            if let Some(doc) = &next {
                return self.rehearse(r, doc.clone(), values, &p, applied.warnings).await;
            }
        }
        if p.check == Some(true) {
            let mut preview = r.borrow().clone();
            preview.document = next.clone();
            preview.values = values;
            let text = self.sketched(&preview, &Views::new(), true, &[], &applied.warnings);
            return Ok(SlateWriteAnswer { version, text, warnings: applied.warnings, problems: Vec::new(), waiting: None });
        }
        self.check_version(&r.borrow(), p.if_version)?;
        if by == SlateBy::Agent {
            self.spend_write(&thread_id)?;
        }
        let mut touched: IndexSet<String> = IndexSet::new();
        for op in &ops {
            if let Some(id) = op.get("id").and_then(Value::as_str) {
                touched.insert(id.to_string());
            }
        }
        let cause = if next.is_none() { SlateCause::Clear } else { SlateCause::Write };
        self.write_document(r, next, values, by, cause, touched.into_iter().collect(), applied.warnings).await
    }

    /// The person's values and the agent's values and starts, on the chain.
    pub(crate) async fn state(self: Rc<Self>, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        let thread_id = self.target_of(p.target.thread_id.as_deref(), p.target.turn_token.as_deref(), caller, true)?;
        let by = self.by_of(caller);
        let paired = road_of(caller) == Some(WorkspaceOrigin::Paired);
        if paired && p.start.as_ref().is_some_and(|starts| !starts.is_empty()) {
            return Err(usage_refusal(
                "a paired computer writes a slate's values and starts no run.",
                "Press it on the computer the slate runs on.",
            ));
        }
        let area = self.clone();
        let chain_thread = thread_id.clone();
        self.serial(&thread_id, move || area.state_on_chain(chain_thread, p, by, paired)).await
    }

    async fn state_on_chain(
        self: Rc<Self>,
        thread_id: String,
        p: StateParams,
        by: SlateBy,
        paired: bool,
    ) -> Result<SlateStateAnswer, HostError> {
        let r = self.clone().need_record(&thread_id).await?;
        let snap = r.borrow().clone();
        let Some(doc) = snap.document.clone() else {
            return Err(usage(&problem("Z802", "no-slate", "this slate is empty; write one with slate_write", None)));
        };
        self.check_version(&snap, p.if_version)?;
        let values_asked = p.values.clone().unwrap_or_default();
        let starts_asked = p.start.clone().unwrap_or_default();
        if values_asked.is_empty() && starts_asked.is_empty() {
            return Err(usage_refusal(
                "slate state sets values or starts runs, and this one names neither.",
                "Give values, start, or both.",
            ));
        }
        let mut starts: Vec<(String, RunBy)> = Vec::new();
        let mut not_started: Vec<String> = Vec::new();
        for named in &starts_asked {
            let run = named.strip_prefix('$').unwrap_or(named).to_string();
            let Some(decl) = doc.runs.get(&run).cloned() else {
                let names: Vec<String> = doc.runs.keys().cloned().collect();
                let fix = slate_nearest(&run, &names).map(|near| format!("${near}"));
                let note = format!("${run} is not a run");
                return Err(invalid(&[problem("K702", "run-name", &note, fix.as_deref())], &[]));
            };
            let held = snap.values.get(&run).and_then(RunRecord::from_json).is_some_and(|rec| rec.state() == Some(RunState::Held));
            let has_confirm = match &decl {
                SlateRunDecl::Cmd(cmd) => cmd.confirm.is_some(),
                SlateRunDecl::Tool(tool) => tool.confirm.is_some(),
                SlateRunDecl::Resource(_) => false,
            };
            let not_resource = !matches!(decl, SlateRunDecl::Resource(_));
            if self.approved_always(&snap, &run) {
                starts.push((run.clone(), if by == SlateBy::Agent { RunBy::Agent } else { RunBy::Person }));
            } else if held {
                not_started.push(format!("${run}: it already waits for the person to allow it on the slate, and starts once they do"));
            } else if not_resource && has_confirm {
                not_started
                    .push(format!("${run}: it has confirm, so it asks the person every start and only a press or a <when> starts it"));
            } else {
                not_started.push(format!(
                    "${run}: you start only a run the person allowed \"Always in this thread\"; a press, a <when> or every= starts it and the slate asks them"
                ));
            }
        }
        let mut input: Vec<SlateWrite> = Vec::new();
        for (written, value) in &values_asked {
            let path = own_path(written);
            let Some(own) = parse_slate_own_path(&path) else {
                let note = format!("{written} is not a value path; a value path reads $name, with .field and [index] below it");
                return Err(invalid(&[problem("S501", "name-undeclared", &note, None)], &[]));
            };
            let derived = doc.derived.contains_key(&own.name);
            if derived || doc.runs.contains_key(&own.name) {
                let what = if derived { "derived value" } else { "run" };
                let note = format!("${} is a {what} and cannot be written", own.name);
                return Err(invalid(&[problem("A607", "set-not-value", &note, Some("write the value it reads"))], &[]));
            }
            let Some(decl) = doc.values.get(&own.name).cloned() else {
                let names: Vec<String> = doc.values.keys().cloned().collect();
                let fix = slate_nearest(&own.name, &names).map(|near| format!("${near}"));
                let note = format!("${} is not declared", own.name);
                return Err(invalid(&[problem("S501", "name-undeclared", &note, fix.as_deref())], &[]));
            };
            if decl.secret == Some(true) {
                if by == SlateBy::Agent || !own.segs.is_empty() {
                    let note = format!("${} is a secret: only the person types it, into its input", own.name);
                    return Err(invalid(&[problem("S520", "secret-exposed", &note, None)], &[]));
                }
                let Some(text) = value.as_str() else {
                    let note = format!("${} takes the typed text", own.name);
                    return Err(invalid(&[problem("S520", "secret-exposed", &note, None)], &[]));
                };
                let secrets = self.runs().secrets();
                let handle = if text.is_empty() {
                    secrets.clear(&thread_id, &own.name)
                } else {
                    secrets.set(&thread_id, &own.name, text, decl.keep == Some(true))
                };
                let handle_value = serde_json::to_value(handle).unwrap_or(Value::Null);
                input.push(SlateWrite { path: format!("${}", own.name), value: handle_value });
                continue;
            }
            input.push(SlateWrite { path, value: value.clone() });
        }
        if by == SlateBy::Agent {
            self.spend_write(&thread_id)?;
            if !input.is_empty() && starts.is_empty() {
                self.runs().release(&thread_id);
            }
        }
        let had_input = !input.is_empty();
        let opts = if paired { BatchOptions { react: Some(false), ..Default::default() } } else { BatchOptions::default() };
        let sends = if had_input { self.clone().batch(r.clone(), input, batch_by(by), opts).await?.sends } else { Vec::new() };
        if by == SlateBy::Agent && had_input {
            self.announce(&r, SlateCause::State, by, &[], None);
        }
        if !starts.is_empty() {
            self.clone().batch(r.clone(), Vec::new(), BatchBy::Run, BatchOptions { starts, ..Default::default() }).await?;
        }
        if !sends.is_empty() {
            let (workspace_id, area) = (r.borrow().workspace_id.clone(), self.clone());
            let word = thread_word(&thread_id);
            let deliver_thread = thread_id.clone();
            spawn_local(async move {
                if let Err(e) = area.deliver_all(deliver_thread, workspace_id, sends).await {
                    console::warn(&format!("a slate send in thread {word} was not delivered: {}", e.message));
                }
            });
        }
        let after = r.borrow().clone();
        let views = self.clone().views_for(&after, &[]).await;
        let text = self.sketched(&after, &views, false, &[], &[]);
        let text = if not_started.is_empty() {
            text
        } else {
            let lines: Vec<String> = not_started.iter().map(|line| format!("  {line}")).collect();
            format!("{text}\nnot started:\n{}", lines.join("\n"))
        };
        Ok(SlateStateAnswer {
            version: after.version,
            text,
            problems: self.problems_of(&after, &views),
            waiting: Some(waiting_of(&after)),
            not_started: if not_started.is_empty() { None } else { Some(not_started) },
        })
    }
}

fn decl_has_every(decl: &SlateRunDecl) -> bool {
    match decl {
        SlateRunDecl::Cmd(cmd) => cmd.every.is_some(),
        SlateRunDecl::Tool(tool) => tool.every.is_some(),
        SlateRunDecl::Resource(resource) => resource.every.is_some(),
    }
}
