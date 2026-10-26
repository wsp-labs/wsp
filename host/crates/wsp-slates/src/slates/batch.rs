// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_batch: slates.ts:405-424, 760-1015 at 65f6b89c7. Spec: spec/slates_batch.md.
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_wire::types::slate_wire::{SlateBy, SlateCause};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::{js_trim, utf16};
use wsp_words::js_date::iso_string;
use wsp_words::words::computer::thread_word;

use super::model::{problem, refused, BatchBy, SlateRecord, MESSAGE_CHARS, PRESS_SEND_MS, REACTION_SEND_MS};
use super::{Delivered, Delivery, RecordRef, SlatesInner, Views};
use crate::pending::slate::{
    evaluate_slate_expression, is_slate_binding, parse_slate_own_path, resolve_slate_prop, run_slate_batch, slate_equal, slate_text,
    BatchEvent, BatchSend, SlateBatchContext, SlateEvalContext, SlateRunDecl, SlateWrite, SLATE_LIMITS,
};
use crate::slate_files::{keyed, pin_here, with_files, Pin, PinAsk, UnpinnedWhy};
use crate::slate_mcp::{code_key, slate_secret_mark, McpRunStart};
use crate::slate_message::{cut_at, defanged, longest};
use crate::slate_runs::{
    last_result, map_strings, running_record, CmdRunDecl, Outcome, RunAsk, RunBy, RunInput, RunInputs, RunRecord, RunStart, RunState,
    NO_FOLDER,
};

/// What a batch did (slates.ts:888-893).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchOut {
    pub asks: Vec<RunAsk>,
    /// The runs this batch started, held or not.
    pub started: Vec<String>,
    pub sends: Vec<ComposedSend>,
}

/// A send composed for the thread.
#[derive(Clone, Debug, PartialEq)]
pub struct ComposedSend {
    pub prompt: String,
    /// `send`, `steer` or `queue`.
    pub kind: String,
    pub request_id: String,
}

/// The batch's options (slates.ts:897).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatchOptions {
    pub event: Option<BatchEvent>,
    pub starts: Vec<(String, RunBy)>,
    pub request_id: Option<String>,
    pub send_at: Option<i64>,
    /// `Some(false)`: no reaction fires.
    pub react: Option<bool>,
}

/// A start's answer inside the area: the record and, when held for approval, the ask.
#[derive(Clone, Debug, PartialEq)]
pub struct Started {
    pub record: RunRecord,
    pub ask: Option<RunAsk>,
}

/// A start the batch's pure core asked for, run after its values are stored.
struct Deferred {
    run: String,
    by: RunBy,
    record: RunRecord,
}

fn by_key(thread_id: &str, run: &str) -> String {
    format!("{thread_id}\u{0}{run}")
}

pub(crate) fn slate_by(by: RunBy) -> SlateBy {
    match by {
        RunBy::Person => SlateBy::Person,
        RunBy::Agent => SlateBy::Agent,
        RunBy::Reaction => SlateBy::Reaction,
        RunBy::Timer => SlateBy::Timer,
    }
}

pub(crate) fn batch_by(by: SlateBy) -> BatchBy {
    match by {
        SlateBy::Person => BatchBy::Person,
        SlateBy::Agent => BatchBy::Agent,
        SlateBy::Reaction => BatchBy::Reaction,
        SlateBy::Timer => BatchBy::Timer,
        SlateBy::Host => BatchBy::Host,
    }
}

fn failed_record(why: &str, runs: i64) -> RunRecord {
    let mut m = Map::new();
    m.insert("state".into(), Value::String("failed".into()));
    m.insert("why".into(), Value::String(why.into()));
    m.insert("runs".into(), json!(runs));
    RunRecord(m)
}

impl SlatesInner {
    /// The declaration as its approval takes it; for a `cmd` run, the runs table's `CmdRunDecl` (files and scripts in).
    pub(crate) fn approval_decl(&self, r: &SlateRecord, declared: &SlateRunDecl) -> CmdRunDecl {
        let SlateRunDecl::Cmd(cmd) = declared else { return CmdRunDecl::default() };
        CmdRunDecl {
            cmd: cmd.cmd.clone(),
            env: cmd.env.clone(),
            args: cmd.args.clone(),
            stdin: cmd.stdin.clone(),
            on: cmd.on.clone(),
            cwd: cmd.cwd.clone(),
            timeout: cmd.timeout,
            stream: cmd.stream,
            confirm: None,
            every: cmd.every,
            always: cmd.always,
            once: cmd.once,
            then: cmd.then.clone(),
            files: with_files(r.document.as_ref(), declared).files,
            scripts: keyed(&self.pin_for(&r.thread_id, declared)),
        }
    }

    /// `(run, cmd, scripts)`, each when known.
    pub(crate) fn approval_names(&self, r: &SlateRecord, key: &str) -> (Option<String>, Option<String>, Option<IndexMap<String, String>>) {
        if let Some(asked) = self.mcp().named(&r.thread_id, key) {
            return (Some(asked.run), Some(asked.cmd), None);
        }
        let Some(doc) = r.document.as_ref() else { return (None, None, None) };
        for (name, declared) in &doc.runs {
            match declared {
                SlateRunDecl::Cmd(_) => {
                    let decl = self.approval_decl(r, declared);
                    if self.runs().key(&decl) == key {
                        return (Some(name.clone()), Some(decl.cmd), decl.scripts);
                    }
                }
                other => {
                    let with = with_files(Some(doc), other);
                    let args: Map<String, Value> = self.args_now(r, other).into_iter().collect();
                    if key == code_key(&with, &args) {
                        let target = match other {
                            SlateRunDecl::Tool(tool) => tool.tool.clone(),
                            SlateRunDecl::Resource(resource) => resource.uri.clone(),
                            SlateRunDecl::Cmd(_) => String::new(),
                        };
                        let server = match other {
                            SlateRunDecl::Tool(tool) => tool.server.clone(),
                            SlateRunDecl::Resource(resource) => resource.server.clone(),
                            SlateRunDecl::Cmd(_) => String::new(),
                        };
                        return (Some(name.clone()), Some(format!("{server}.{target}")), None);
                    }
                }
            }
        }
        (None, None, None)
    }

    fn views_now_of(&self, thread_id: &str) -> Views {
        self.views_now.borrow().get(thread_id).cloned().unwrap_or_default()
    }

    pub(crate) fn inputs_of(self: &Rc<Self>, r: RecordRef, decl: &crate::pending::slate::SlateCmdDecl) -> Rc<dyn Fn() -> RunInputs> {
        let area = Rc::downgrade(self);
        let decl = decl.clone();
        Rc::new(move || match area.upgrade() {
            Some(area) => area.inputs_resolved(&r.borrow(), &decl),
            None => RunInputs::default(),
        })
    }

    fn inputs_resolved(&self, r: &SlateRecord, decl: &crate::pending::slate::SlateCmdDecl) -> RunInputs {
        let ctx = self.context_of(r, &self.views_now_of(&r.thread_id), None);
        let secret = |name: &str| r.document.as_ref().and_then(|doc| doc.values.get(name)).is_some_and(|v| v.secret == Some(true));
        let one = |v: &Value| -> RunInput {
            if is_slate_binding(v) {
                let own = v.get("bind").and_then(Value::as_str).and_then(|bind| parse_slate_own_path(js_trim(bind)));
                if let Some(own) = own {
                    if own.segs.is_empty() && secret(&own.name) {
                        return RunInput::Secret(own.name);
                    }
                }
            }
            RunInput::Value(resolve_slate_prop(v, &ctx).unwrap_or(Value::Null))
        };
        RunInputs {
            env: decl.env.as_ref().map(|env| env.iter().map(|(name, v)| (name.clone(), one(v))).collect()),
            args: decl.args.as_ref().map(|args| args.iter().map(&one).collect()),
            stdin: decl.stdin.as_ref().map(&one),
        }
    }

    pub(crate) fn mcp_args_of(self: &Rc<Self>, r: RecordRef, decl: &SlateRunDecl) -> Rc<dyn Fn() -> Map<String, Value>> {
        let area = Rc::downgrade(self);
        let decl = decl.clone();
        Rc::new(move || match area.upgrade() {
            Some(area) => area.args_now(&r.borrow(), &decl).into_iter().collect(),
            None => Map::new(),
        })
    }

    fn args_now(&self, r: &SlateRecord, decl: &SlateRunDecl) -> IndexMap<String, Value> {
        let SlateRunDecl::Tool(tool) = decl else { return IndexMap::new() };
        let Some(args) = tool.args.as_ref() else { return IndexMap::new() };
        let ctx = self.context_of(r, &self.views_now_of(&r.thread_id), None);
        let plain = ctx.resolve.clone();
        let secrets: Vec<String> = r
            .document
            .as_ref()
            .map_or_else(Vec::new, |doc| doc.values.iter().filter(|(_, v)| v.secret == Some(true)).map(|(name, _)| name.clone()).collect());
        let marked = SlateEvalContext {
            now: ctx.now,
            item: ctx.item.clone(),
            index: ctx.index,
            resolve: Rc::new(move |path: &str| match parse_slate_own_path(path) {
                Some(own) if own.segs.is_empty() && secrets.contains(&own.name) => Some(Value::String(slate_secret_mark(&own.name))),
                _ => plain(path),
            }),
        };
        args.iter().map(|(name, v)| (name.clone(), resolve_slate_prop(v, &marked).unwrap_or(Value::Null))).collect()
    }

    pub(crate) fn on_machine(&self, thread_id: &str, decl: &SlateRunDecl) -> bool {
        matches!(decl, SlateRunDecl::Cmd(cmd) if cmd.on.as_deref() != Some("host")) && self.deps.machine_of(thread_id).is_some()
    }

    /// What an approval of a run binds where it runs: on the thread's machine as its daemon last read it, else here.
    pub(crate) fn pin_for(&self, thread_id: &str, decl: &SlateRunDecl) -> Pin {
        let folder = self.folder_for(thread_id, decl);
        if self.on_machine(thread_id, decl) {
            self.pins().pin(thread_id, folder.as_deref(), decl)
        } else {
            pin_here(folder.as_deref(), decl)
        }
    }

    /// The views a start reads, with the scripts the thread's machine holds hashed again, so a key holds what runs.
    pub(crate) async fn startable(self: Rc<Self>, r: &SlateRecord) -> Views {
        let there: Vec<PinAsk> = r.document.as_ref().map_or_else(Vec::new, |doc| {
            doc.runs
                .values()
                .filter(|decl| self.on_machine(&r.thread_id, decl))
                .map(|decl| PinAsk { folder: self.folder_for(&r.thread_id, decl), decl: decl.clone() })
                .collect()
        });
        if there.is_empty() {
            return self.clone().views_for(r, &[]).await;
        }
        let asleep = self.deps.asleep(&r.thread_id);
        let views = self.clone().views_for(r, &[]).await;
        self.pins().read(&r.thread_id, &there, asleep).await;
        views
    }

    /// A press or the agent's start wakes a napping machine anyway: first here, so its pins are read and an Always holds.
    pub(crate) async fn wake_for(self: Rc<Self>, r: &SlateRecord, starts: &[(String, RunBy)]) {
        if !self.deps.asleep(&r.thread_id) {
            return;
        }
        let unread = starts.iter().any(|(run, by)| {
            *by != RunBy::Timer
                && r.document
                    .as_ref()
                    .and_then(|doc| doc.runs.get(run))
                    .is_some_and(|decl| self.pin_for(&r.thread_id, decl).unpinned.iter().any(|u| u.why == UnpinnedWhy::Unread))
        });
        if !unread {
            return;
        }
        if self.deps.wake(&r.thread_id).await.is_ok() {
            self.startable(r).await;
        }
    }

    /// Each start held on the sheet whose key moved (its command was rewritten, its box was read), held again as now.
    pub(crate) fn held_again(self: &Rc<Self>, r: RecordRef, views: &Views) -> Vec<String> {
        let thread_id = r.borrow().thread_id.clone();
        let mut moved = Vec::new();
        for ask in self.runs().held(&thread_id) {
            let key = {
                let rec = r.borrow();
                match rec.document.as_ref().and_then(|doc| doc.runs.get(&ask.run)) {
                    Some(declared @ SlateRunDecl::Cmd(_)) => Some(self.runs().key(&self.approval_decl(&rec, declared))),
                    _ => None,
                }
            };
            let Some(key) = key else { continue };
            if key == ask.key {
                continue;
            }
            let record = self.start_now(r.clone(), &ask.run, RunBy::Person, views, None).record;
            r.borrow_mut().values.insert(ask.run.clone(), record.to_json());
            moved.push(ask.run);
        }
        moved
    }

    pub(crate) fn folder_for(&self, thread_id: &str, decl: &SlateRunDecl) -> Option<String> {
        let facts = self.deps.thread(thread_id);
        match decl {
            SlateRunDecl::Cmd(cmd) if cmd.on.as_deref() == Some("host") => facts.and_then(|f| f.host_folder.or(f.folder)),
            _ => facts.and_then(|f| f.folder),
        }
    }

    /// `prior` none is the record's own value; `Some(None)` is no prior at all.
    pub(crate) fn start_now(self: &Rc<Self>, r: RecordRef, run: &str, by: RunBy, views: &Views, prior: Option<Option<Value>>) -> Started {
        let (thread_id, declared, prior_value) = {
            let rec = r.borrow();
            let prior_value = match prior {
                None => rec.values.get(run).cloned(),
                Some(given) => given,
            };
            (rec.thread_id.clone(), rec.document.as_ref().and_then(|doc| doc.runs.get(run)).cloned(), prior_value)
        };
        let last = prior_value.as_ref().and_then(RunRecord::from_json);
        let count = last.as_ref().map_or(0, RunRecord::runs);
        let Some(declared) = declared else {
            return Started { record: failed_record(&format!("${run} is not declared"), count), ask: None };
        };
        let folder = self.folder_for(&thread_id, &declared);
        let (cmd_decl, folder) = match (&declared, folder) {
            (SlateRunDecl::Cmd(cmd), Some(folder)) => (Some(cmd.clone()), folder),
            (SlateRunDecl::Cmd(_), None) => {
                return Started { record: failed_record(NO_FOLDER, count), ask: None };
            }
            _ => (None, String::new()),
        };
        self.views_now.borrow_mut().insert(thread_id.clone(), views.clone());
        *self.capturing.borrow_mut() = Some((thread_id.clone(), run.to_string()));
        let said = match &declared {
            SlateRunDecl::Resource(_) => None,
            SlateRunDecl::Cmd(cmd) => cmd.confirm.clone(),
            SlateRunDecl::Tool(tool) => tool.confirm.clone(),
        };
        let confirm = said.map(|said| {
            let rec = r.borrow();
            let ctx = self.context_of(&rec, views, None);
            let text = slate_text(&resolve_slate_prop(&said, &ctx).unwrap_or(Value::Null));
            if text.is_empty() {
                format!("Run ${run}?")
            } else {
                text
            }
        });
        let answer = match cmd_decl {
            Some(cmd) => {
                let mut asked = self.approval_decl(&r.borrow(), &declared);
                asked.confirm = confirm;
                let asleep = asked.on.as_deref() != Some("host") && self.deps.asleep(&thread_id);
                let inputs = self.inputs_of(r.clone(), &cmd);
                self.runs().start(RunStart {
                    thread_id: thread_id.clone(),
                    run: run.to_string(),
                    decl: asked,
                    by,
                    folder,
                    inputs,
                    last: last.clone(),
                    asleep,
                })
            }
            None => {
                let mut asked = with_files(r.borrow().document.as_ref(), &declared);
                if let (SlateRunDecl::Tool(tool), Some(text)) = (&mut asked.decl, confirm) {
                    tool.confirm = Some(Value::String(text));
                }
                let args = self.mcp_args_of(r.clone(), &declared);
                self.mcp().start(McpRunStart {
                    thread_id: thread_id.clone(),
                    run: run.to_string(),
                    decl: asked,
                    by,
                    args,
                    last: last.clone(),
                })
            }
        };
        self.started_by.borrow_mut().insert(by_key(&thread_id, run), by);
        if answer.record.state() == Some(RunState::Running) {
            self.announce(&r, SlateCause::Run, slate_by(by), &[], Some(run));
        }
        *self.capturing.borrow_mut() = None;
        Started { record: answer.record, ask: if answer.outcome == Outcome::Held { answer.ask } else { None } }
    }

    pub(crate) fn provisional(self: &Rc<Self>, r: RecordRef, run: &str) -> RunRecord {
        let (thread_id, declared, prior) = {
            let rec = r.borrow();
            (
                rec.thread_id.clone(),
                rec.document.as_ref().and_then(|doc| doc.runs.get(run)).cloned(),
                rec.values.get(run).and_then(RunRecord::from_json),
            )
        };
        let count = prior.as_ref().map_or(0, RunRecord::runs);
        match declared {
            Some(declared @ (SlateRunDecl::Tool(_) | SlateRunDecl::Resource(_))) => {
                let with = with_files(r.borrow().document.as_ref(), &declared);
                let args: Map<String, Value> = self.args_now(&r.borrow(), &declared).into_iter().collect();
                self.mcp().provisional(&thread_id, &with, prior.as_ref(), &args)
            }
            _ => {
                if self.approved_always(&r.borrow(), run) {
                    let result = prior.as_ref().and_then(last_result);
                    running_record(result.as_ref(), count + 1, self.deps.clock().now())
                } else {
                    let mut m = Map::new();
                    m.insert("state".into(), Value::String("held".into()));
                    m.insert("why".into(), Value::String(crate::pending::wire::SLATE_HELD_APPROVAL.into()));
                    m.insert("runs".into(), json!(count));
                    RunRecord(m)
                }
            }
        }
    }

    pub(crate) fn approved_always(&self, r: &SlateRecord, run: &str) -> bool {
        let Some(declared @ SlateRunDecl::Cmd(cmd)) = r.document.as_ref().and_then(|doc| doc.runs.get(run)) else { return false };
        if cmd.confirm.is_some() {
            return false;
        }
        let key = self.runs().key(&self.approval_decl(r, declared));
        r.approvals.get(&key).is_some_and(|approval| approval.state == "allowed")
    }

    pub(crate) async fn run_moved(self: Rc<Self>, thread_id: String, run: String, record: RunRecord) -> Result<(), HostError> {
        let found = self.records.borrow().get(&thread_id).cloned();
        let Some(r) = found else { return Ok(()) };
        let declared = r.borrow().document.as_ref().is_some_and(|doc| doc.runs.contains_key(&run));
        if !declared {
            return Ok(());
        }
        if record.state() == Some(RunState::Running) {
            let by = self.started_by.borrow().get(&by_key(&thread_id, &run)).copied().unwrap_or(RunBy::Person);
            self.announce(&r, SlateCause::Run, slate_by(by), &[], Some(&run));
        }
        let input = vec![SlateWrite { path: format!("${run}"), value: record.to_json() }];
        self.batch(r, input, BatchBy::Run, BatchOptions::default()).await?;
        Ok(())
    }

    /// Called on the thread's chain.
    pub(crate) async fn batch(
        self: Rc<Self>,
        r: RecordRef,
        input: Vec<SlateWrite>,
        by: BatchBy,
        o: BatchOptions,
    ) -> Result<BatchOut, HostError> {
        let snap = r.borrow().clone();
        let Some(doc) = snap.document.clone() else { return Ok(BatchOut::default()) };
        let thread_id = snap.thread_id.clone();
        let views = self.clone().startable(&snap).await;
        let before = snap.values.clone();
        let deferred: Rc<RefCell<Vec<Deferred>>> = Rc::new(RefCell::new(Vec::new()));
        let timer = by == BatchBy::Timer;
        let start: Rc<dyn Fn(&str, &str) -> Value> = {
            let area = self.clone();
            let queue = deferred.clone();
            let r = r.clone();
            Rc::new(move |run: &str, start_by: &str| {
                let record = area.provisional(r.clone(), run);
                let by = match (timer, start_by) {
                    (true, _) => RunBy::Timer,
                    (false, "reaction") => RunBy::Reaction,
                    (false, _) => RunBy::Person,
                };
                let answer = record.to_json();
                queue.borrow_mut().push(Deferred { run: run.to_string(), by, record });
                answer
            })
        };
        let by_name = match by {
            BatchBy::Timer | BatchBy::Run => "run",
            BatchBy::Person => "person",
            BatchBy::Agent => "agent",
            BatchBy::Reaction => "reaction",
            BatchBy::Host => "host",
        };
        let ctx = SlateBatchContext {
            eval: self.context_of(&snap, &views, None),
            by: by_name.into(),
            event: o.event.clone(),
            reaction_sends: snap.sends_allowed.is_some(),
            react: o.react,
            start: Some(start),
        };
        self.clone().wake_for(&snap, &o.starts).await;
        let mut writes = input;
        for (run, run_by) in &o.starts {
            let record = self.start_now(r.clone(), run, *run_by, &views, None).record;
            writes.push(SlateWrite { path: format!("${run}"), value: record.to_json() });
        }
        let base = r.borrow().values.clone();
        let result = run_slate_batch(&doc, &base, writes, ctx);
        let bytes = serde_json::to_string(&result.values).map_or(0, |text| text.len());
        if bytes > SLATE_LIMITS.values_bytes {
            let note = format!("the values would be {bytes} bytes; a slate holds at most {}", SLATE_LIMITS.values_bytes);
            self.keep_problems(&mut r.borrow_mut(), &[problem("S500", "values-too-big", &note, None)]);
            self.clone().save(r.clone()).await?;
            return Ok(BatchOut::default());
        }
        {
            let mut rec = r.borrow_mut();
            rec.values = result.values.clone();
            self.keep_problems(&mut rec, &result.problems);
        }
        for run in &result.cancels {
            self.runs().cancel(&thread_id, run);
        }
        let deferred_list: Vec<Deferred> = std::mem::take(&mut *deferred.borrow_mut());
        let starts: Vec<(String, RunBy)> = deferred_list.iter().map(|d| (d.run.clone(), d.by)).collect();
        let now_snap = r.borrow().clone();
        self.clone().wake_for(&now_snap, &starts).await;
        let mut asks = Vec::new();
        for d in &deferred_list {
            let started = self.start_now(r.clone(), &d.run, d.by, &views, before.get(&d.run).cloned().map(Some));
            if let Some(ask) = started.ask.clone() {
                asks.push(ask);
            }
            if started.record.state() == d.record.state() {
                r.borrow_mut().values.insert(d.run.clone(), started.record.to_json());
            } else {
                let area = self.clone();
                let (t, run, record) = (thread_id.clone(), d.run.clone(), started.record.clone());
                let _ = self.serial(&thread_id, move || async move { area.run_moved(t, run, record).await });
            }
        }

        let sends_snap = r.borrow().clone();
        let now = o.send_at.unwrap_or_else(|| self.deps.clock().now());
        let mut sends = Vec::new();
        for send in &result.sends {
            let from_press = send.by.as_deref() == Some("person");
            let (sent_at, limit) =
                if from_press { (&self.press_sent_at, PRESS_SEND_MS) } else { (&self.reaction_sent_at, REACTION_SEND_MS) };
            let last = sent_at.borrow().get(&thread_id).copied();
            if last.is_some_and(|last| now - last < limit) {
                if from_press {
                    return Err(refused(&problem("V753", "send-rate", "too fast; try again in a moment", None), Kind::Conflict));
                }
                let note =
                    format!("reaction {}: V753 send-rate, one message a minute from reactions", send.reaction.as_deref().unwrap_or("?"));
                self.keep_problems(&mut r.borrow_mut(), &[problem("R912", "reaction-failed", &note, None)]);
                continue;
            }
            sent_at.borrow_mut().insert(thread_id.clone(), now);
            let prompt = self.compose(&sends_snap, send, &views, o.event.as_ref(), now);
            let version = r.borrow().version;
            let request_id = o.request_id.clone().unwrap_or_else(|| {
                let what = send.reaction.as_deref().or(send.piece.as_deref()).unwrap_or("slate");
                format!("{thread_id}:{version}:{what}:{now}")
            });
            sends.push(ComposedSend { prompt, kind: send.kind.clone(), request_id });
        }

        let changed: Vec<String> = {
            let rec = r.borrow();
            let mut names: IndexSet<&String> = before.keys().collect();
            names.extend(rec.values.keys());
            names.into_iter().filter(|name| !slate_equal(before.get(*name), rec.values.get(*name))).cloned().collect()
        };
        if !changed.is_empty() || !result.problems.is_empty() {
            {
                let mut rec = r.borrow_mut();
                let revision = rec.rev() + 1;
                rec.revision = Some(revision);
            }
            self.clone().save(r.clone()).await?;
            self.push(&r, &changed);
        }
        Ok(BatchOut { asks, started: deferred_list.iter().map(|d| d.run.clone()).collect(), sends })
    }

    pub(crate) fn compose(&self, r: &SlateRecord, send: &BatchSend, views: &Views, event: Option<&BatchEvent>, now: i64) -> String {
        let ctx = self.context_of(r, views, event.map(|e| (e.item.clone(), e.index)));
        let named: &[String] = send.with.as_deref().unwrap_or(&[]);
        let mut carried: Value = Value::Object(
            named
                .iter()
                .map(|path| {
                    let raw = send
                        .values
                        .as_ref()
                        .and_then(|values| values.get(path))
                        .filter(|v| !v.is_null())
                        .cloned()
                        .or_else(|| evaluate_slate_expression(path, &ctx))
                        .unwrap_or(Value::Null);
                    let scrubbed = map_strings(&raw, &|text: &str| self.scrub(&r.thread_id, text));
                    (path.clone(), defanged(&scrubbed))
                })
                .collect(),
        );
        let from_press = send.by.as_deref() == Some("person");
        let piece_id: Option<String> = send.piece.clone().or_else(|| if from_press { event.map(|e| e.piece.clone()) } else { None });
        let piece = piece_id.as_ref().and_then(|id| r.document.as_ref().and_then(|doc| doc.pieces.get(id)));
        let props = piece.and_then(|p| p.props.as_ref());
        let row_actions = props.and_then(|p| p.get("rowActions")).and_then(Value::as_array);
        let owner =
            event.and_then(|e| e.row_action).and_then(|i| row_actions.and_then(|items| usize::try_from(i).ok().and_then(|i| items.get(i))));
        let labelled: Option<&Value> = match owner {
            Some(owner) => owner.get("label"),
            None => props.and_then(|p| p.get("label")),
        };
        let label = labelled.and_then(Value::as_str).map(|text| utf16::cut(text, 80));
        let key: Option<Value> = match (event.and_then(|e| e.index), props.and_then(|p| p.get("key"))) {
            (Some(_), Some(key_prop)) => resolve_slate_prop(key_prop, &ctx),
            _ => None,
        };
        let line = |with: &Value| -> String {
            let mut m = Map::new();
            m.insert("v".into(), json!(2));
            m.insert("kind".into(), json!(if from_press { "action" } else { "reaction" }));
            m.insert("thread".into(), json!(thread_word(&r.thread_id)));
            m.insert("version".into(), json!(r.version));
            if let Some(id) = &piece_id {
                m.insert("piece".into(), json!(id));
            }
            if let Some(label) = &label {
                m.insert("label".into(), json!(label));
            }
            if let Some(event) = event.filter(|_| from_press) {
                m.insert("event".into(), json!(event.kind));
            }
            if let Some(reaction) = &send.reaction {
                m.insert("reaction".into(), json!(reaction));
            }
            if let Some(index) = event.filter(|_| from_press).and_then(|e| e.index) {
                m.insert("index".into(), json!(index));
            }
            if let Some(key) = &key {
                m.insert("key".into(), key.clone());
            }
            if !named.is_empty() {
                m.insert("with".into(), with.clone());
            }
            m.insert("by".into(), json!(if from_press { "person" } else { "reaction" }));
            m.insert("at".into(), json!(iso_string(now as f64).unwrap_or_default()));
            format!("slate: {}", serde_json::to_string(&Value::Object(m)).unwrap_or_default())
        };
        let mut prompt = format!("{}\n\n{}", send.text, line(&carried));
        for _ in 0..50 {
            if utf16::len(&prompt) <= MESSAGE_CHARS {
                break;
            }
            let Some(long) = longest(&carried, &[]) else { break };
            if long.length < 20 {
                break;
            }
            let over = utf16::len(&prompt) as i64 - MESSAGE_CHARS as i64;
            let keep = (long.length as i64 - over - 10).max(0) as usize;
            carried = cut_at(&carried, &long.at, keep);
            prompt = format!("{}\n\n{}", send.text, line(&carried));
        }
        prompt
    }

    /// The first outcome, with the send's kind.
    pub(crate) async fn deliver_all(
        self: Rc<Self>,
        thread_id: String,
        workspace_id: String,
        sends: Vec<ComposedSend>,
    ) -> Result<Option<(Delivered, String)>, HostError> {
        let mut first = None;
        for send in sends {
            let landed = self
                .deps
                .deliver(Delivery {
                    thread_id: thread_id.clone(),
                    workspace_id: workspace_id.clone(),
                    prompt: send.prompt,
                    request_id: send.request_id,
                })
                .await?;
            if first.is_none() {
                first = Some((landed, send.kind));
            }
        }
        Ok(first)
    }
}
