// SPDX-License-Identifier: AGPL-3.0-only
//! Unit slates_reads: slates.ts:549-685, 1233-1279, 1482-1488 at 65f6b89c7. Spec: spec/slates_reads.md.
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use wsp_wire::text::json_stringify;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;
use wsp_words::js::{js_trim, utf16};

use super::model::{problem, sources_named, SlateRecord, SLATE_SEND_KEY};
use super::{CatalogParams, ReadParams, SlatesInner, Views};
use crate::pending::slate::expr::{evaluate_slate_expression, slate_dependencies};
use crate::pending::slate::paths::{get_slate_value, parse_slate_own_path, slate_step};
use crate::pending::slate::{
    is_slate_binding, print_slate, sketch_slate, slate_catalog, SketchOptions, SlateDoc, SlateEvalContext, SlateProblem, SlateValues,
};
use crate::pending::wire::{SlateApprovalView, SlateAsk, SlateCmdAsk, SlateEmpty, SlateReadAnswer, SlateView};
use crate::slate_files::{cannot_pin, keyed, Pin, UnpinnedWhy};
use crate::slate_read::{is_run_record, misread, run_shown, READ_OUTPUT_CHARS};
use crate::slate_runs::{map_strings, RunRecord, HELD_APPROVAL};
use crate::slate_sources::{resolve_in, view_sources};

fn visit_prop(value: &Value, found: &mut IndexSet<String>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| visit_prop(item, found)),
        Value::Object(fields) => {
            if is_slate_binding(value) {
                if let Some(bind) = fields.get("bind").and_then(Value::as_str) {
                    found.extend(slate_dependencies(bind));
                }
            } else if let (Some(format), 1) = (fields.get("format").and_then(Value::as_str), fields.len()) {
                for hole in format_holes(format) {
                    found.extend(slate_dependencies(hole));
                }
            } else {
                fields.values().for_each(|item| visit_prop(item, found));
            }
        }
        _ => {}
    }
}

fn format_holes(text: &str) -> Vec<&str> {
    let mut holes = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else { break };
        holes.push(&after[..end]);
        rest = &after[end + 1..];
    }
    holes
}

pub fn bound_paths(doc: &SlateDoc) -> Vec<String> {
    let mut found: IndexSet<String> = IndexSet::new();
    for piece in doc.pieces.values() {
        if let Some(props) = &piece.props {
            visit_prop(&Value::Object(props.iter().map(|(k, v)| (k.clone(), v.clone())).collect()), &mut found);
        }
        if let Some(when) = &piece.when {
            found.extend(slate_dependencies(when));
        }
    }
    for expr in doc.derived.values() {
        found.extend(slate_dependencies(expr));
    }
    found.into_iter().filter(|path| !path.starts_with("item") && !path.starts_with("index")).collect()
}

pub fn waiting_of(r: &SlateRecord) -> Vec<String> {
    let Some(doc) = r.document.as_ref() else { return Vec::new() };
    doc.runs.keys().filter(|name| held_for_person(r.values.get(name.as_str()))).map(|name| format!("${name}")).collect()
}

fn held_for_person(rec: Option<&Value>) -> bool {
    let Some(Value::Object(fields)) = rec else { return false };
    if !is_run_record(rec) {
        return false;
    }
    let why_ok = match fields.get("why") {
        None => true,
        Some(Value::String(why)) => why == HELD_APPROVAL,
        Some(_) => false,
    };
    fields.get("state").and_then(Value::as_str) == Some("held") && why_ok
}

struct Scope {
    now: i64,
    item: Option<Value>,
    index: Option<i64>,
    views: Views,
    values: SlateValues,
    derived: IndexMap<String, String>,
    cache: RefCell<IndexMap<String, Option<Value>>>,
    busy: RefCell<IndexSet<String>>,
}

fn eval_context(scope: &Rc<Scope>) -> SlateEvalContext {
    let reader = scope.clone();
    SlateEvalContext {
        now: scope.now,
        item: scope.item.clone(),
        index: scope.index,
        resolve: Rc::new(move |path: &str| resolve_path(&reader, path)),
    }
}

fn resolve_path(scope: &Rc<Scope>, path: &str) -> Option<Value> {
    if !path.starts_with('$') {
        return resolve_in(&scope.views, path);
    }
    let own = parse_slate_own_path(path)?;
    let Some(expr) = scope.derived.get(&own.name) else {
        return get_slate_value(&scope.values, path);
    };
    let cached = scope.cache.borrow().contains_key(&own.name);
    if !cached {
        let busy = scope.busy.borrow().contains(&own.name);
        if busy {
            return None;
        }
        scope.busy.borrow_mut().insert(own.name.clone());
        let value = evaluate_slate_expression(expr, &eval_context(scope));
        scope.busy.borrow_mut().shift_remove(&own.name);
        scope.cache.borrow_mut().insert(own.name.clone(), value);
    }
    let base = scope.cache.borrow().get(&own.name).cloned().flatten();
    own.segs.iter().fold(base, slate_step)
}

fn starts_with_version(text: &str) -> bool {
    text.strip_prefix("slate v").is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

fn with_version(text: String, version: i64) -> String {
    let Some(rest) = text.strip_prefix("slate") else { return text };
    if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        return text;
    }
    format!("slate v{version}{rest}")
}

impl SlatesInner {
    pub(crate) async fn views_for(self: Rc<Self>, r: &SlateRecord, extra: &[String]) -> Views {
        let document = r.document.as_ref().and_then(|doc| serde_json::to_string(doc).ok()).unwrap_or_default();
        let text = format!("{document} {}", extra.join(" "));
        let mut names = vec!["time".to_string()];
        names.extend(sources_named(&text));
        let ctx = self.deps.sources(&r.thread_id, &r.workspace_id);
        view_sources(&names, ctx.as_ref()).await
    }

    /// `row` is the event's item and index.
    pub(crate) fn context_of(&self, r: &SlateRecord, views: &Views, row: Option<(Option<Value>, Option<i64>)>) -> SlateEvalContext {
        let scope = Rc::new(Scope {
            now: self.deps.clock().now(),
            item: row.as_ref().and_then(|(item, _)| item.clone()),
            index: row.as_ref().and_then(|(_, index)| *index),
            views: views.clone(),
            values: r.values.clone(),
            derived: r.document.as_ref().map(|doc| doc.derived.clone()).unwrap_or_default(),
            cache: RefCell::new(IndexMap::new()),
            busy: RefCell::new(IndexSet::new()),
        });
        eval_context(&scope)
    }

    pub(crate) fn scrub(&self, thread_id: &str, text: &str) -> String {
        self.runs().secrets().scrub(thread_id, text)
    }

    pub(crate) fn sketched(
        &self,
        r: &SlateRecord,
        views: &Views,
        check: bool,
        asked: &[SlateProblem],
        warnings: &[SlateProblem],
    ) -> String {
        let eval = self.context_of(r, views, None);
        let (problems, waiting) =
            if check { (Vec::new(), Vec::new()) } else { ([asked.to_vec(), self.problems_of(r, views)].concat(), waiting_of(r)) };
        let options = SketchOptions { eval, version: r.version, warnings: warnings.to_vec(), check, problems, waiting };
        let text = sketch_slate(r.document.as_ref(), &r.values, options);
        let text = if starts_with_version(&text) { text } else { with_version(text, r.version) };
        self.scrub(&r.thread_id, &text)
    }

    pub(crate) async fn sketch_of(self: Rc<Self>, r: &SlateRecord) -> String {
        let views = self.clone().views_for(r, &[]).await;
        self.sketched(r, &views, false, &[], &[])
    }

    pub(crate) fn problems_of(&self, r: &SlateRecord, views: &Views) -> Vec<SlateProblem> {
        let mut found: Vec<SlateProblem> = r.problems.clone();
        if r.document.is_none() && r.empty == Some(SlateEmpty::RewoundBefore) {
            found.push(problem(
                "Z804",
                "rewound-before",
                "the thread was rewound to before this slate existed; write one with slate_write",
                None,
            ));
        }
        if let Some(doc) = r.document.as_ref() {
            for path in bound_paths(doc) {
                if !path.starts_with('$') && resolve_in(views, &path).is_none() {
                    found.push(problem("R900", "data-missing", &format!("{path} has no value yet"), None));
                }
            }
        }
        found
    }

    pub(crate) fn changed_since(&self, r: &SlateRecord, run: &str, key: &str, pin: &Pin) -> Option<String> {
        let now: IndexMap<String, String> = keyed(pin).unwrap_or_default();
        let unread: Vec<String> = pin.unpinned.iter().filter(|u| u.why == UnpinnedWhy::Unread).map(|u| u.path.clone()).collect();
        for (k, a) in &r.approvals {
            if k == key || a.state != "allowed" || a.run.as_deref() != Some(run) {
                continue;
            }
            if !unread.is_empty() {
                let computer =
                    self.deps.thread(&r.thread_id).and_then(|facts| facts.computer).unwrap_or_else(|| "this computer".to_string());
                return Some(format!("{} could not be read on {computer}, so it asks again", unread.join(", ")));
            }
            let Some(scripts) = a.scripts.as_ref() else { continue };
            let paths: IndexSet<&String> = scripts.keys().chain(now.keys()).collect();
            let changed: Vec<&String> = paths.into_iter().filter(|path| scripts.get(*path) != now.get(*path)).collect();
            if !changed.is_empty() {
                let names: Vec<&str> = changed.iter().map(|path| path.as_str()).collect();
                return Some(format!("{} changed since you allowed it, so it asks again", names.join(", ")));
            }
        }
        None
    }

    pub(crate) fn asks_of(&self, r: &SlateRecord) -> Vec<SlateAsk> {
        let computer = self.deps.thread(&r.thread_id).and_then(|facts| facts.computer).unwrap_or_else(|| "this computer".to_string());
        let mut asks: Vec<SlateAsk> = self
            .runs()
            .held(&r.thread_id)
            .into_iter()
            .map(|a| {
                let pin = r.document.as_ref().and_then(|doc| doc.runs.get(&a.run)).map(|decl| self.pin_for(&r.thread_id, decl));
                let by_pin = pin.as_ref().and_then(|pin| cannot_pin(pin).or_else(|| self.changed_since(r, &a.run, &a.key, pin)));
                let why = by_pin
                    .or_else(|| r.values.get(&a.run).and_then(RunRecord::from_json).and_then(|rec| rec.why().map(String::from)))
                    .unwrap_or_else(|| HELD_APPROVAL.to_string());
                SlateAsk::Cmd(Box::new(SlateCmdAsk {
                    key: a.key,
                    run: a.run,
                    cmd: a.cmd,
                    env: a.env,
                    args: a.args,
                    stdin: a.stdin,
                    computer: computer.clone(),
                    folder: a.folder,
                    timeout_s: serde_json::to_value(a.timeout).unwrap_or(Value::Null),
                    confirm: a.confirm,
                    then: a.then,
                    files: a.files,
                    no_always: pin.filter(|pin| !pin.unpinned.is_empty()).map(|_| true),
                    why,
                }))
            })
            .collect();
        asks.extend(self.mcp().held(&r.thread_id));
        asks
    }

    pub(crate) fn view_of(&self, r: &SlateRecord) -> SlateView {
        let mut approvals: IndexMap<String, SlateApprovalView> = r
            .approvals
            .iter()
            .map(|(key, a)| {
                let view = SlateApprovalView {
                    state: a.state.clone(),
                    at: a.at,
                    run: a.run.clone(),
                    cmd: a.cmd.clone(),
                    scripts: a.scripts.clone(),
                };
                (key.clone(), view)
            })
            .collect();
        if let Some(at) = r.sends_allowed {
            let send = SlateApprovalView { state: "allowed".to_string(), at, run: None, cmd: None, scripts: None };
            approvals.insert(SLATE_SEND_KEY.to_string(), send);
        }
        SlateView {
            thread_id: r.thread_id.clone(),
            workspace_id: r.workspace_id.clone(),
            version: r.version,
            revision: r.rev(),
            document: r.document.as_ref().and_then(|doc| serde_json::to_value(doc).ok()),
            values: r.values.clone(),
            empty: if r.document.is_none() { Some(r.empty.unwrap_or(SlateEmpty::None)) } else { None },
            comments: r.comments.clone(),
            approvals,
            asks: self.asks_of(r),
            problems: r.problems.clone(),
            shown_once: r.shown_once,
            can_undo: r.previous.is_some(),
            updated_at: r.updated_at,
        }
    }

    pub(crate) async fn read(self: Rc<Self>, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        let thread_id = self.target_of(p.target.thread_id.as_deref(), p.target.turn_token.as_deref(), caller, false)?;
        let found = self.clone().record_of(&thread_id).await?;
        let r = match found {
            Some(rec) => {
                let copy = rec.borrow().clone();
                copy
            }
            None => self.fresh_record(&thread_id)?,
        };
        let asked = p.values.clone().unwrap_or_default();
        let paths: Vec<String> = match r.document.as_ref() {
            Some(doc) if asked.iter().any(|v| v == "*") => bound_paths(doc),
            _ => asked.iter().filter(|v| v.as_str() != "*").cloned().collect(),
        };
        let views = self.clone().views_for(&r, &paths).await;
        let ctx = self.context_of(&r, &views, None);
        let doc = r.document.as_ref();
        let clean = |v: &Value| map_strings(v, &|text: &str| self.scrub(&r.thread_id, text));
        let misreads: Vec<SlateProblem> = paths.iter().flat_map(|path| misread(doc, path)).collect();
        let sketch =
            if p.sketch != Some(false) { self.sketched(&r, &views, false, &misreads, &[]) } else { format!("slate v{}", r.version) };

        let mut asks: Vec<String> = Vec::new();
        let mut values: IndexMap<String, Value> = IndexMap::new();
        for path in &paths {
            let shown = clean(&evaluate_slate_expression(path, &ctx).unwrap_or(Value::Null));
            let said = json_stringify(&shown);
            let len = utf16::len(&said);
            let line = if len > READ_OUTPUT_CHARS {
                format!("  {path} = {}... ({len} characters)", utf16::cut(&said, READ_OUTPUT_CHARS))
            } else {
                format!("  {path} = {said}")
            };
            asks.push(line);
            values.insert(path.clone(), shown);
        }

        let mut lines = vec![sketch];
        if !asks.is_empty() {
            lines.push("read:".to_string());
            lines.extend(asks);
        }
        if let (Some(doc), true) = (doc, p.text != Some(false)) {
            lines.push(String::new());
            lines.push(print_slate(doc));
        }

        let document = (p.document == Some(true)).then(|| r.document.as_ref().and_then(|doc| serde_json::to_value(doc).ok()));

        let state: IndexMap<String, Value> = r
            .values
            .iter()
            .filter(|(name, _)| doc.is_none_or(|d| !d.runs.contains_key(name.as_str())))
            .map(|(name, v)| (format!("${name}"), clean(v)))
            .collect();

        let mut derived: IndexMap<String, Value> = IndexMap::new();
        if let Some(doc) = doc {
            for name in doc.derived.keys() {
                let value = (ctx.resolve)(&format!("${name}")).unwrap_or(Value::Null);
                derived.insert(format!("${name}"), clean(&value));
            }
        }

        let mut runs: IndexMap<String, Value> = IndexMap::new();
        if let Some(doc) = doc {
            for name in doc.runs.keys() {
                let shown = run_shown(r.values.get(name), name, !paths.is_empty());
                runs.insert(format!("${name}"), clean(&shown));
            }
        }

        let mut problems = misreads;
        problems.extend(self.problems_of(&r, &views));

        let mut approvals: IndexMap<String, String> = IndexMap::new();
        for (key, a) in &r.approvals {
            let (run, _, _) = self.approval_names(&r, key);
            approvals.insert(run.map(|run| format!("${run}")).unwrap_or_else(|| key.clone()), a.state.clone());
        }

        Ok(SlateReadAnswer {
            version: r.version,
            text: lines.join("\n"),
            document,
            values,
            state,
            derived,
            runs,
            problems,
            waiting: waiting_of(&r),
            comments: r.comments.clone(),
            approvals,
        })
    }

    pub(crate) async fn catalog(self: Rc<Self>, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError> {
        self.refuse_sub_thread(caller)?;
        let text = slate_catalog(p.name.as_deref());
        let name = match p.name.as_deref().map(js_trim) {
            Some(name) if !name.is_empty() && text.starts_with(&format!("{name} is not in the catalog")) => name.to_string(),
            _ => return Ok(text),
        };
        let Ok(thread_id) = self.target_of(p.target.thread_id.as_deref(), p.target.turn_token.as_deref(), caller, false) else {
            return Ok(text);
        };
        let mcp = self.mcp();
        match mcp.catalog(&thread_id, &name).await {
            Ok(found) => Ok(found),
            Err(message) => Ok(format!("{text}\n{name} as an MCP server of this thread's agent: {message}")),
        }
    }

    pub(crate) async fn resolve(self: Rc<Self>, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        let Some(found) = self.clone().record_of(thread_id).await? else {
            return Ok(paths.iter().map(|path| (path.clone(), Value::Null)).collect());
        };
        let r = {
            let copy = found.borrow().clone();
            copy
        };
        let views = self.clone().views_for(&r, paths).await;
        let ctx = self.context_of(&r, &views, None);
        let mut values: IndexMap<String, Value> = IndexMap::new();
        for path in paths {
            let raw = evaluate_slate_expression(path, &ctx).unwrap_or(Value::Null);
            values.insert(path.clone(), map_strings(&raw, &|text: &str| self.scrub(&r.thread_id, text)));
        }
        Ok(values)
    }
}
