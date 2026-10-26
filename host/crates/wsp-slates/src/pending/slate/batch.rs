// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/batch.ts at 65f6b89c7.

use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use wsp_words::words::units::fmt_bytes;

use super::*;

pub type SlateBatchSend = BatchSend;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlateWriter {
    Person,
    Agent,
    Reaction,
    Run,
    Timer,
    Host,
}

impl SlateWriter {
    pub fn word(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Agent => "agent",
            Self::Reaction => "reaction",
            Self::Run => "run",
            Self::Timer => "timer",
            Self::Host => "host",
        }
    }

    pub fn from_word(word: &str) -> Option<Self> {
        [Self::Person, Self::Agent, Self::Reaction, Self::Run, Self::Timer, Self::Host].into_iter().find(|w| w.word() == word)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlateBatchWindowStep {
    pub step: Value,
    pub piece: String,
    pub value: Option<Value>,
    pub values: Option<IndexMap<String, Value>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Row {
    item: Option<Value>,
    index: Option<i64>,
}

#[derive(Clone, Debug)]
struct Origin {
    by: &'static str,
    piece: Option<String>,
    reaction: Option<String>,
    row: Row,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OwnKind {
    Value,
    Secret,
    Derived,
    Run,
}

struct Refusal {
    code: &'static str,
    why: String,
}

#[derive(Clone)]
pub(crate) struct OwnReader {
    doc: Rc<SlateDoc>,
    values: Rc<SlateValues>,
    source: Rc<dyn Fn(&str) -> Option<Value>>,
    now: i64,
    derived: Rc<std::cell::RefCell<IndexMap<String, Value>>>,
}

impl OwnReader {
    pub(crate) fn new(doc: Rc<SlateDoc>, values: Rc<SlateValues>, source: Rc<dyn Fn(&str) -> Option<Value>>, now: i64) -> Self {
        Self { doc, values, source, now, derived: Rc::default() }
    }

    pub(crate) fn now(&self) -> i64 {
        self.now
    }

    pub(crate) fn context(&self, item: Option<Value>, index: Option<i64>) -> SlateEvalContext {
        let reader = self.clone();
        SlateEvalContext { now: self.now, item, index, resolve: Rc::new(move |path: &str| reader.resolve(path)) }
    }

    pub(crate) fn resolve(&self, path: &str) -> Option<Value> {
        if !path.starts_with('$') {
            return (self.source)(path);
        }
        let own = parse_slate_own_path(path)?;
        if !own.segs.is_empty() {
            return None;
        }
        if let Some(expr) = self.doc.derived.get(&own.name) {
            if let Some(v) = self.derived.borrow().get(&own.name) {
                return Some(v.clone());
            }
            self.derived.borrow_mut().insert(own.name.clone(), Value::Null);
            let v = evaluate_slate_expression(expr, &self.context(None, None)).unwrap_or(Value::Null);
            self.derived.borrow_mut().insert(own.name, v.clone());
            return Some(v);
        }
        self.values.get(&own.name).cloned()
    }
}

pub(crate) fn is_secret_handle(v: &Value) -> bool {
    let Some(m) = v.as_object() else { return false };
    m.get("secret") == Some(&Value::Bool(true))
        && m.get("set").is_some_and(Value::is_boolean)
        && m.get("len").is_some_and(Value::is_number)
        && m.keys().all(|k| matches!(k.as_str(), "secret" | "set" | "len" | "at"))
}

struct Batch {
    doc: Rc<SlateDoc>,
    values: Rc<SlateValues>,
    ctx: SlateBatchContext,
    out: SlateBatchResult,
}

impl Batch {
    fn new(doc: &SlateDoc, values: Rc<SlateValues>, ctx: SlateBatchContext) -> Self {
        let out = SlateBatchResult {
            values: (*values).clone(),
            changed: Vec::new(),
            problems: Vec::new(),
            starts: Vec::new(),
            cancels: Vec::new(),
            sends: Vec::new(),
            other: Vec::new(),
            other_steps: Vec::new(),
        };
        Self { doc: Rc::new(doc.clone()), values, ctx, out }
    }

    fn reader_over(&self, values: &Rc<SlateValues>, row: &Row) -> SlateEvalContext {
        OwnReader::new(self.doc.clone(), values.clone(), self.ctx.eval.resolve.clone(), self.ctx.eval.now)
            .context(row.item.clone(), row.index)
    }

    fn kind(&self, name: &str) -> Option<OwnKind> {
        if let Some(decl) = self.doc.values.get(name) {
            return Some(if decl.secret == Some(true) { OwnKind::Secret } else { OwnKind::Value });
        }
        if self.doc.derived.contains_key(name) {
            return Some(OwnKind::Derived);
        }
        self.doc.runs.contains_key(name).then_some(OwnKind::Run)
    }

    fn read_in(&self, path: &str, values: &Rc<SlateValues>) -> Option<Value> {
        evaluate_slate_expression(path, &self.reader_over(values, &Row::default()))
    }

    fn problem(&mut self, code: &str, message: &str, piece: Option<&str>) {
        let mut problem = slate_problem(code, message, None);
        problem.piece = piece.map(str::to_string);
        self.out.problems.push(problem);
    }

    fn write(&mut self, path: &str, value: Value, by: SlateWriter) {
        let Some(own) = parse_slate_own_path(path) else {
            self.problem("S501", &format!("{path} is not one of the slate's own paths; they read $name"), None);
            return;
        };
        let Some(kind) = self.kind(&own.name) else {
            self.problem("S501", &format!("${} is not declared", own.name), None);
            return;
        };
        let name = &own.name;
        match kind {
            OwnKind::Derived => {
                self.problem("A607", &format!("${name} is derived and cannot be written; write the value it reads"), None);
                return;
            }
            OwnKind::Run if !(matches!(by, SlateWriter::Run | SlateWriter::Host) && own.segs.is_empty()) => {
                self.problem("A607", &format!("${name} is a run; its result is the host's to write"), None);
                return;
            }
            OwnKind::Secret if !(own.segs.is_empty() && is_secret_handle(&value) && by != SlateWriter::Agent) => {
                self.problem("S520", &format!("${name} is a secret; only the person's typing fills it, and its value is its handle"), None);
                return;
            }
            _ => {}
        }
        if let Some(refused) = self.set(path, value) {
            self.problem(refused.code, &refused.why, None);
        }
    }

    fn set(&mut self, path: &str, value: Value) -> Option<Refusal> {
        let Some(next) = set_slate_value(&self.values, path, value.clone()) else {
            return Some(Refusal {
                code: "A607",
                why: format!("{path} steps into a value that is not a list or a record, or past {} items", SLATE_LIMITS.list_items),
            });
        };
        let bytes = slate_bytes(&wsp_wire::text::json_stringify(&serde_json::to_value(&next).unwrap_or(Value::Null)));
        if bytes > SLATE_LIMITS.values_bytes {
            return Some(Refusal { code: "S500", why: format!("the values would pass {}", fmt_bytes(SLATE_LIMITS.values_bytes as f64)) });
        }
        if !slate_equal(get_slate_value(&self.values, path).as_ref(), Some(&value)) {
            self.values = Rc::new(next);
        }
        None
    }

    fn set_step(&mut self, path: &str, value: Value) -> Option<String> {
        self.set(path, value).map(|r| format!("{} ({})", r.why, r.code))
    }

    fn steps(&mut self, steps: &[Value], from: &Origin) {
        let who = match &from.reaction {
            Some(reaction) => format!("reaction {reaction}"),
            None => format!("piece {}", from.piece.as_deref().unwrap_or_default()),
        };
        for (i, s) in steps.iter().enumerate() {
            if let Some(why) = self.step(s, from) {
                let kind = s.get("do").and_then(Value::as_str).unwrap_or_default();
                let message = format!("{who}, step {} ({kind}): {why}; the steps after it did not run", i + 1);
                let piece = from.reaction.clone().or_else(|| from.piece.clone());
                self.problem("R912", &message, piece.as_deref());
                return;
            }
        }
    }

    fn step(&mut self, s: &Value, from: &Origin) -> Option<String> {
        let kind = s.get("do").and_then(Value::as_str).unwrap_or_default();
        let text = |key: &str| s.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
        match kind {
            "set" | "toggle" => {
                let path = text("path");
                let own = parse_slate_own_path(&path);
                let Some(own) = own.filter(|p| self.kind(&p.name).is_some()) else {
                    return Some(format!("{path} is not declared"));
                };
                match self.kind(&own.name)? {
                    OwnKind::Derived => return Some(format!("${} is derived and cannot be written (A607)", own.name)),
                    OwnKind::Run => return Some(format!("${} is a run and cannot be written (A607)", own.name)),
                    OwnKind::Secret => return Some(format!("${} is a secret; only the person's typing fills it (S520)", own.name)),
                    OwnKind::Value => {}
                }
                let value = if kind == "set" {
                    let raw = s.get("value").cloned().unwrap_or(Value::Null);
                    resolve_slate_prop(&raw, &self.reader_over(&self.values, &from.row)).unwrap_or(Value::Null)
                } else {
                    Value::Bool(get_slate_value(&self.values, &path) != Some(Value::Bool(true)))
                };
                self.set_step(&path, value)
            }
            "start" => {
                let run = text("run");
                let Some(decl) = self.doc.runs.get(&run) else {
                    return Some(format!("${run} is not a run (K702)"));
                };
                let once = match decl {
                    SlateRunDecl::Cmd(c) => c.once == Some(true),
                    SlateRunDecl::Tool(t) => t.once == Some(true),
                    SlateRunDecl::Resource(_) => false,
                };
                let running = self.values.get(&run).and_then(|r| r.get("state")).and_then(Value::as_str) == Some("running");
                if once && running {
                    return None;
                }
                let why = match &from.reaction {
                    Some(reaction) => format!("reaction {reaction}"),
                    None => format!(
                        "a {} on {}",
                        self.ctx.event.as_ref().map_or("press", |e| e.kind.as_str()),
                        from.piece.as_deref().unwrap_or_default()
                    ),
                };
                self.out.starts.push(BatchStart { run: run.clone(), by: from.by.to_string(), why });
                let record = self.ctx.start.as_ref().map(|start| start(&run, from.by))?;
                self.set_step(&format!("${run}"), record)
            }
            "cancel" => {
                let run = text("run");
                if !self.doc.runs.contains_key(&run) {
                    return Some(format!("${run} is not a run (K702)"));
                }
                self.out.cancels.push(run);
                None
            }
            "send" | "steer" | "queue" => {
                if from.by == "reaction" && !self.ctx.reaction_sends {
                    return Some("the person has not let this slate message the agent from a reaction".to_string());
                }
                let with: Option<Vec<String>> = s
                    .get("with")
                    .and_then(Value::as_array)
                    .map(|paths| paths.iter().map(|p| p.as_str().unwrap_or_default().to_string()).collect());
                let reader = self.reader_over(&self.values, &from.row);
                let mut values = IndexMap::new();
                for path in with.iter().flatten() {
                    values.insert(path.clone(), evaluate_slate_expression(path, &reader).unwrap_or(Value::Null));
                }
                self.out.sends.push(BatchSend {
                    kind: kind.to_string(),
                    text: text("text"),
                    with,
                    values: Some(values),
                    piece: from.piece.clone(),
                    reaction: from.reaction.clone(),
                    by: Some(from.by.to_string()),
                });
                None
            }
            _ => {
                let window = slate_step_module(kind).is_some_and(|m| m.runs == "window");
                let Some(piece) = from.piece.clone().filter(|_| window && from.by == "person") else {
                    return Some(format!("{kind} runs in the window on a press (A608)"));
                };
                let reader = self.reader_over(&self.values, &from.row);
                let mut out = SlateBatchWindowStep { step: s.clone(), piece, value: None, values: None };
                match kind {
                    "open" => {
                        out.value =
                            Some(resolve_slate_prop(&s.get("target").cloned().unwrap_or(Value::Null), &reader).unwrap_or(Value::Null))
                    }
                    "copy" => {
                        out.value = Some(resolve_slate_prop(&s.get("text").cloned().unwrap_or(Value::Null), &reader).unwrap_or(Value::Null))
                    }
                    "fill" => {
                        let mut values = IndexMap::new();
                        for path in s.get("with").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                            values.insert(path.to_string(), evaluate_slate_expression(path, &reader).unwrap_or(Value::Null));
                        }
                        out.values = Some(values);
                    }
                    _ => {}
                }
                self.out.other.push(kind.to_string());
                self.out.other_steps.push(out);
                None
            }
        }
    }

    fn event(&mut self) {
        let Some(e) = self.ctx.event.clone() else { return };
        let doc = Rc::clone(&self.doc);
        let Some(piece) = doc.pieces.get(&e.piece) else {
            self.problem("D203", &format!("{} is not on the slate; press again once it has redrawn", e.piece), None);
            return;
        };
        let steps: Option<&Vec<Value>> = match e.row_action {
            Some(action) => piece
                .props
                .as_ref()
                .and_then(|p| p.get("rowActions"))
                .and_then(Value::as_array)
                .and_then(|actions| usize::try_from(action).ok().and_then(|at| actions.get(at)))
                .and_then(|a| a.get("on"))
                .and_then(|on| on.get("press"))
                .and_then(Value::as_array),
            None => piece.on.as_ref().and_then(|on| on.get(&e.kind)).and_then(Value::as_array),
        };
        let Some(steps) = steps.filter(|s| !s.is_empty()) else { return };
        let mut item = e.item.clone();
        let items = piece.props.as_ref().and_then(|p| p.get("items"));
        if let (true, Some(index), Some(items)) = (item.is_none(), e.index, items) {
            let listed = resolve_slate_prop(items, &self.reader_over(&self.values, &Row::default()));
            item =
                listed.as_ref().and_then(Value::as_array).and_then(|list| usize::try_from(index).ok().and_then(|at| list.get(at)).cloned());
        }
        let by = if self.ctx.by == "agent" { "reaction" } else { "person" };
        let from = Origin { by, piece: Some(e.piece.clone()), reaction: None, row: Row { item, index: e.index } };
        self.steps(steps, &from);
    }

    fn fired(&self, before: &Rc<SlateValues>, after: &Rc<SlateValues>) -> Vec<(usize, String)> {
        if Rc::ptr_eq(before, after) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (i, reaction) in self.doc.reactions.iter().enumerate() {
            let key = reaction.get("id").and_then(Value::as_str).map_or_else(|| format!("reaction-{}", i + 1), str::to_string);
            let on = &reaction["on"];
            if let Some(paths) = on.get("change").and_then(Value::as_array) {
                let moved = paths.iter().filter_map(Value::as_str).any(|p| {
                    let was = self.read_in(p, before).unwrap_or(Value::Null);
                    let now = self.read_in(p, after).unwrap_or(Value::Null);
                    !slate_equal(Some(&was), Some(&now))
                });
                if moved {
                    out.push((i, key));
                }
                continue;
            }
            let done = on.get("done").and_then(Value::as_str).unwrap_or_default();
            let was = before.get(done);
            let now = after.get(done);
            let terminal =
                now.and_then(|n| n.get("state")).and_then(Value::as_str).is_some_and(|s| matches!(s, "done" | "failed" | "cancelled"));
            if terminal && !slate_equal(was, now) {
                out.push((i, key));
            }
        }
        out
    }
}

pub fn run_slate_batch(doc: &SlateDoc, values: &SlateValues, input: Vec<SlateWrite>, ctx: SlateBatchContext) -> SlateBatchResult {
    let start = Rc::new(values.clone());
    let by = SlateWriter::from_word(&ctx.by).unwrap_or(SlateWriter::Agent);
    let react = ctx.react != Some(false);
    let mut b = Batch::new(doc, Rc::clone(&start), ctx);
    for w in input {
        b.write(&w.path, w.value, by);
    }
    b.event();
    let mut before = Rc::clone(&start);
    let mut round = 1;
    while react {
        let fired = b.fired(&before, &b.values);
        if fired.is_empty() {
            break;
        }
        if round > SLATE_LIMITS.rounds {
            let keys: Vec<&str> = fired.iter().map(|(_, key)| key.as_str()).collect();
            b.problem(
                "R910",
                &format!(
                    "reactions ran {} rounds in one batch and stopped: {} would have fired again",
                    SLATE_LIMITS.rounds,
                    keys.join(", ")
                ),
                None,
            );
            break;
        }
        before = Rc::clone(&b.values);
        let doc = Rc::clone(&b.doc);
        for (i, key) in fired {
            let steps = doc.reactions[i].get("do").and_then(Value::as_array).cloned().unwrap_or_default();
            let from = Origin { by: "reaction", piece: None, reaction: Some(key), row: Row::default() };
            b.steps(&steps, &from);
        }
        round += 1;
    }
    let names: IndexSet<&String> = start.keys().chain(b.values.keys()).collect();
    b.out.changed = names.into_iter().filter(|n| !slate_equal(start.get(*n), b.values.get(*n))).map(|n| format!("${n}")).collect();
    b.out.values = (*b.values).clone();
    b.out
}
