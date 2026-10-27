// SPDX-License-Identifier: AGPL-3.0-only
//! waiting-behind.test.ts:25 `drivenAdapter`: an adapter whose every turn is driven by the test, on a session id of its
//! own so two threads never share a row. A turn writes a tool call, answers it, raises a prompt and replies when told.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AnswerOutcome, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent, PermissionAnswer, SharedFuture,
    TurnResult,
};
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};

use super::adapter::{event, result};

/// `ASK`: the permission prompt a test raises.
pub fn ask() -> Value {
    json!({
        "askId": "ask_1",
        "toolName": "Read",
        "toolUseId": "toolu_read",
        "input": "{\"file_path\":\"/root/hello.txt\"}",
        "detail": "hello.txt",
        "options": [
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "deny", "label": "Deny", "effect": "deny" }
        ]
    })
}

struct Inner {
    session_id: String,
    on_event: OnEvent,
    open: RefCell<Vec<(String, Value)>>,
    settle: RefCell<Option<oneshot::Sender<TurnResult>>>,
}

/// What a running fake turn lets a test do to it.
#[derive(Clone)]
pub struct Turn(Rc<Inner>);

impl Turn {
    pub fn calls(&self, tool_use_id: &str, tool_name: &str, input: &str) {
        (self.0.on_event)(event(json!({
            "type": "turn.delta", "sessionId": self.0.session_id, "kind": "tool_use", "text": input,
            "toolName": tool_name, "toolUseId": tool_use_id
        })));
    }

    pub fn answers(&self, tool_use_id: &str) {
        (self.0.on_event)(event(json!({
            "type": "turn.delta", "sessionId": self.0.session_id, "kind": "tool_result", "text": "ok", "toolUseId": tool_use_id
        })));
    }

    /// `raise(overrides)`: the prompt of `ask()` with the keys of `overrides` laid over it.
    pub fn raise(&self, overrides: Value) {
        let mut ask = ask();
        if let (Some(ask), Some(more)) = (ask.as_object_mut(), overrides.as_object()) {
            ask.extend(more.clone());
        }
        let ask_id = ask["askId"].as_str().unwrap_or_default().to_string();
        self.0.open.borrow_mut().push((ask_id, ask.clone()));
        (self.0.on_event)(event(json!({ "type": "permission.ask", "sessionId": self.0.session_id, "ask": ask })));
    }

    /// `reply(result)`: closes every open prompt as cancelled, then `turn.done` and `session.end`.
    pub fn reply(&self, more: Value) {
        let Some(tx) = self.0.settle.borrow_mut().take() else { return };
        let mut doc = json!({ "status": "completed", "text": "done" });
        if let (Some(doc), Some(more)) = (doc.as_object_mut(), more.as_object()) {
            doc.extend(more.clone());
        }
        let r = result(doc.clone());
        let open: Vec<(String, Value)> = self.0.open.borrow_mut().drain(..).collect();
        for (ask_id, _) in open {
            (self.0.on_event)(event(json!({
                "type": "permission.close", "sessionId": self.0.session_id, "askId": ask_id, "outcome": "cancelled"
            })));
        }
        (self.0.on_event)(event(json!({ "type": "turn.done", "sessionId": self.0.session_id, "result": doc })));
        (self.0.on_event)(event(json!({ "type": "session.end", "sessionId": self.0.session_id, "exitCode": 0, "sawResult": true })));
        let _ = tx.send(r);
    }
}

/// The turns a driven adapter has started, in order.
#[derive(Default)]
pub struct Turns(RefCell<Vec<Turn>>);

impl Turns {
    pub fn new() -> Rc<Turns> {
        Rc::default()
    }
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }
    pub fn get(&self, n: usize) -> Turn {
        self.0.borrow()[n].clone()
    }
}

/// `drivenAdapter(turns)`.
pub fn driven(turns: &Rc<Turns>) -> HarnessAdapterFactory {
    let turns = turns.clone();
    let opened = Rc::new(Cell::new(0u32));
    Arc::new(move |_ctx: HarnessAdapterContext| {
        Ok(Arc::new(Driven { turns: turns.clone(), opened: opened.clone() }) as Arc<dyn HarnessAdapter>)
    })
}

struct Driven {
    turns: Rc<Turns>,
    opened: Rc<Cell<u32>>,
}

impl HarnessAdapter for Driven {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.opened.set(self.opened.get() + 1);
        let session_id = format!("11111111-1111-4111-8111-{:012}", self.opened.get());
        let on_event = o.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let inner = Rc::new(Inner {
            session_id: session_id.clone(),
            on_event: on_event.clone(),
            open: RefCell::default(),
            settle: RefCell::new(Some(tx)),
        });
        on_event(event(json!({ "type": "session.start", "sessionId": session_id, "cwd": "/root" })));
        self.turns.0.borrow_mut().push(Turn(inner.clone()));
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end was never sent") }.boxed_local();
        Ok(Arc::new(DrivenSession { inner, finished: finished.shared() }))
    }
}

struct DrivenSession {
    inner: Rc<Inner>,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for DrivenSession {
    fn local_id(&self) -> String {
        self.inner.session_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let turn = Turn(self.inner.clone());
        async move { turn.reply(json!({ "status": "interrupted" })) }.boxed_local()
    }
    fn answers(&self) -> bool {
        true
    }
    fn answer(&self, ask_id: String, a: PermissionAnswer) -> Option<LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        let inner = self.inner.clone();
        Some(
            async move {
                let held = {
                    let mut open = inner.open.borrow_mut();
                    open.iter().position(|(id, _)| *id == ask_id).map(|at| open.remove(at))
                };
                if held.is_none() {
                    return Ok(AnswerOutcome::Gone);
                }
                let outcome = serde_json::to_value(a.outcome).expect("an outcome");
                (inner.on_event)(event(json!({
                    "type": "permission.close", "sessionId": inner.session_id, "askId": ask_id,
                    "outcome": outcome, "optionId": a.option_id
                })));
                Ok(AnswerOutcome::Answered)
            }
            .boxed_local(),
        )
    }
}
