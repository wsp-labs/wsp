// SPDX-License-Identifier: AGPL-3.0-only
//! The `drivenAdapter` of session-row.test.ts:51 and `driven` of session-row-oracle.test.ts:36: a harness whose turns the
//! test drives line by line (`calls`, `answers`, `raise`, `reply`, `emit`, `end`), that names a session by `titles`, and
//! that answers a permission prompt. One adapter, with the three extras the oracle's copy adds (`rename`, a made title,
//! a start that throws on a `boom` prompt) behind `Driven::oracle()`, and held-turn.test.ts:34's copy (steers, says
//! nothing at its start, answers nothing) behind `Driven::held_turn()`.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AnswerOutcome, ExecFailure, HarnessAdapter, HarnessExec, HarnessSession, HarnessStartOptions, OnEvent, PermissionAnswer,
    SessionRenameWrite, SharedFuture, TitleTurn, TurnResult,
};
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};

use super::adapter::{event, result};

/// One turn the test drives.
pub struct DTurn {
    pub session_id: String,
    pub prompt: String,
    on_event: OnEvent,
    finish: RefCell<Option<oneshot::Sender<TurnResult>>>,
}

impl DTurn {
    /// `emit(event)`: the event as the harness prints it, under this turn's session.
    pub fn emit(&self, mut doc: Value) {
        if let Some(o) = doc.as_object_mut() {
            o.entry("sessionId").or_insert_with(|| json!(self.session_id));
        }
        (self.on_event)(event(doc));
    }

    /// serve-replay.test.ts:36 `delta(text)`: one text delta.
    pub fn delta(&self, text: &str) {
        self.emit(json!({ "type": "turn.delta", "kind": "text", "text": text }));
    }

    /// `end()` of serve-replay.test.ts:37: the result is exactly `doc`, with none of `end`'s defaults.
    pub fn finish(&self, doc: Value) {
        self.settle(result(doc));
    }

    /// `calls(toolUseId, toolName, input)`: a tool use delta.
    pub fn calls(&self, tool_use_id: &str, tool_name: &str, input: &str) {
        self.emit(json!({ "type": "turn.delta", "kind": "tool_use", "text": input, "toolName": tool_name, "toolUseId": tool_use_id }));
    }

    /// `answers(toolUseId)`: the tool's result delta.
    pub fn answers(&self, tool_use_id: &str) {
        self.emit(json!({ "type": "turn.delta", "kind": "tool_result", "text": "ok", "toolUseId": tool_use_id }));
    }

    /// `raise()`: the permission prompt `ask`.
    pub fn raise(&self, ask: Value) {
        self.emit(json!({ "type": "permission.ask", "ask": ask }));
    }

    /// `end(r)` / `reply()`: the turn's result, then its end. `more` is merged over `{ status: "completed", text: "done" }`.
    pub fn end(&self, more: Value) {
        let mut doc = json!({ "status": "completed", "text": "done" });
        if let (Some(doc), Some(more)) = (doc.as_object_mut(), more.as_object()) {
            doc.extend(more.clone());
        }
        self.settle(result(doc));
    }

    pub fn reply(&self) {
        self.end(json!({}));
    }

    /// A settle that resolves `finished` and prints nothing (held-turn.test.ts:44).
    fn finish_quiet(&self, r: TurnResult) {
        if let Some(tx) = self.finish.borrow_mut().take() {
            let _ = tx.send(r);
        }
    }

    fn settle(&self, r: TurnResult) {
        let Some(tx) = self.finish.borrow_mut().take() else { return };
        self.emit(json!({ "type": "turn.done", "result": serde_json::to_value(&r).expect("a result") }));
        self.emit(json!({ "type": "session.end", "exitCode": 0, "sawResult": true }));
        let _ = tx.send(r);
    }
}

/// The adapter's own state, shared with the test.
pub struct Driven {
    pub turns: RefCell<Vec<Rc<DTurn>>>,
    /// While true each turn replies as soon as it starts.
    pub quick: Cell<bool>,
    /// What the harness calls a session, by session id.
    pub titles: RefCell<HashMap<String, String>>,
    mode: Mode,
    opened: Cell<u32>,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Row,
    Oracle,
    HeldTurn,
    Replay,
}

impl Driven {
    /// session-row.test.ts:51 `drivenAdapter(turns, quick, titles)`.
    pub fn new() -> Rc<Driven> {
        Driven::of(Mode::Row)
    }

    fn of(mode: Mode) -> Rc<Driven> {
        Rc::new(Driven { turns: RefCell::default(), quick: Cell::new(false), titles: RefCell::default(), mode, opened: Cell::new(0) })
    }

    /// serve-replay.test.ts:23 `drivenHarness()`: one fixed session, each `delta` one `session.delta` on the wire.
    pub fn replay() -> Rc<Driven> {
        Driven::of(Mode::Replay)
    }

    /// held-turn.test.ts:34 `drivenAdapter(said)`: a turn that says nothing by itself, and steers.
    pub fn held_turn() -> Rc<Driven> {
        Driven::of(Mode::HeldTurn)
    }

    /// session-row-oracle.test.ts:36 `driven(turns)`: renames, makes a title, throws at a `boom` prompt.
    pub fn oracle() -> Rc<Driven> {
        Driven::of(Mode::Oracle)
    }

    pub fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let driven = self.clone();
        Arc::new(move |_ctx: HarnessAdapterContext| Ok(Arc::new(DrivenAdapter { driven: driven.clone() }) as Arc<dyn HarnessAdapter>))
    }

    /// `turns.at(-1)`.
    pub fn last(&self) -> Rc<DTurn> {
        self.turns.borrow().last().cloned().expect("a turn was started")
    }

    pub fn len(&self) -> usize {
        self.turns.borrow().len()
    }

    pub fn turn(&self, n: usize) -> Rc<DTurn> {
        self.turns.borrow()[n].clone()
    }
}

struct DrivenAdapter {
    driven: Rc<Driven>,
}

impl HarnessAdapter for DrivenAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let d = &self.driven;
        if d.mode == Mode::Oracle && o.prompt.starts_with("boom") {
            return Err(AdapterError("the agent would not start".into()));
        }
        let session_id = match &o.resume {
            _ if d.mode == Mode::Replay => "77777777-7777-4777-8777-777777777777".to_string(),
            Some(id) => id.clone(),
            None => {
                d.opened.set(d.opened.get() + 1);
                format!("11111111-1111-4111-8111-{:012}", d.opened.get())
            }
        };
        let (tx, rx) = oneshot::channel();
        let on_event = o.on_event.clone().expect("an event sink");
        let turn = Rc::new(DTurn { session_id: session_id.clone(), prompt: o.prompt.clone(), on_event, finish: RefCell::new(Some(tx)) });
        let start = match d.mode {
            Mode::Oracle => json!({ "type": "session.start", "sessionId": session_id, "cwd": "/root", "model": "claude-sonnet-4-5" }),
            Mode::Row => json!({ "type": "session.start", "sessionId": session_id, "cwd": "/root" }),
            Mode::HeldTurn => json!({ "type": "session.start", "sessionId": session_id }),
            Mode::Replay => json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5" }),
        };
        turn.emit(start);
        d.turns.borrow_mut().push(turn.clone());
        if d.quick.get() {
            let quick = turn.clone();
            tokio::task::spawn_local(async move { quick.reply() });
        }
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end was never sent") }.boxed_local();
        Ok(Arc::new(DrivenSession { mode: d.mode, turn, finished: finished.shared() }))
    }

    fn steers(&self) -> bool {
        self.driven.mode == Mode::HeldTurn
    }

    fn renames(&self) -> bool {
        self.driven.mode == Mode::Oracle
    }

    fn session_title(
        &self,
        id: String,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        if self.driven.mode == Mode::HeldTurn {
            return None;
        }
        let titles = self.driven.clone();
        Some(async move { Ok(titles.titles.borrow().get(&id).cloned()) }.boxed_local())
    }

    fn rename_session(
        &self,
        _id: String,
        _title: String,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        (self.driven.mode == Mode::Oracle).then(|| async { Ok(SessionRenameWrite::Written) }.boxed_local())
    }

    fn title_for(
        &self,
        _turn: TitleTurn,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        (self.driven.mode == Mode::Oracle).then(|| async { Ok(Some("a made title".to_string())) }.boxed_local())
    }
}

struct DrivenSession {
    mode: Mode,
    turn: Rc<DTurn>,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for DrivenSession {
    fn local_id(&self) -> String {
        self.turn.session_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let turn = self.turn.clone();
        let quiet = self.mode == Mode::HeldTurn;
        async move {
            if quiet {
                turn.finish_quiet(result(json!({ "status": "interrupted" })));
            } else {
                turn.settle(result(json!({ "status": "interrupted" })));
            }
        }
        .boxed_local()
    }
    fn answers(&self) -> bool {
        self.mode != Mode::HeldTurn
    }
    fn answer(&self, ask_id: String, a: PermissionAnswer) -> Option<LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        if self.mode == Mode::HeldTurn {
            return None;
        }
        let turn = self.turn.clone();
        Some(
            async move {
                turn.emit(json!({
                    "type": "permission.close",
                    "askId": ask_id,
                    "outcome": serde_json::to_value(a.outcome).expect("an outcome"),
                    "optionId": a.option_id,
                }));
                Ok(AnswerOutcome::Answered)
            }
            .boxed_local(),
        )
    }
}
