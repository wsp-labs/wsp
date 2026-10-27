// SPDX-License-Identifier: AGPL-3.0-only
//! The one-off adapters the TS whole-runtime tests write inline (`const dying: HarnessAdapterFactory = () => ({ steers,
//! start })`): a factory over a start function, and a session that is a local id and the result it finishes with.
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent, SharedFuture, TurnResult};
use wsp_core::types::harness::HarnessAdapterFactory;

use super::adapter::{event, result};

/// `start: o => ...` of a TS adapter literal.
pub type StartFn = Rc<dyn Fn(HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError>>;

/// `() => ({ steers: false, start })`.
pub fn adapter_of(start: StartFn) -> HarnessAdapterFactory {
    Arc::new(move |_ctx| Ok(Arc::new(Scripted { start: start.clone() }) as Arc<dyn HarnessAdapter>))
}

struct Scripted {
    start: StartFn,
}

impl HarnessAdapter for Scripted {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        (self.start)(o)
    }
}

/// `{ localId, finished, interrupt: async () => {} }`.
pub struct Plain {
    pub local_id: String,
    pub finished: SharedFuture<TurnResult>,
}

impl Plain {
    pub fn of(local_id: &str, finished: LocalBoxFuture<'static, TurnResult>) -> Arc<dyn HarnessSession> {
        Arc::new(Plain { local_id: local_id.into(), finished: finished.shared() })
    }
}

impl HarnessSession for Plain {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

/// `Promise.resolve().then(() => { for (e of events) o.onEvent(e); return result })`: the events go out after the start
/// has returned and the caller's own code after it has run, then the future answers `done`. A promise's microtask
/// runs after the caller's synchronous code; one `yield_now` here runs before the Rust start's own bookkeeping (the
/// thread record it writes once the turn is under way), so the events wait three turns.
pub fn after_a_turn(on_event: OnEvent, events: Vec<Value>, done: Value) -> LocalBoxFuture<'static, TurnResult> {
    async move {
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        for doc in events {
            on_event(event(doc));
        }
        result(done)
    }
    .boxed_local()
}

/// A `turn.done`, then a `session.end`, of the session.
pub fn done_and_end(session: &str, done: &Value, exit_code: i32, saw_result: bool) -> Vec<Value> {
    vec![
        json!({ "type": "turn.done", "sessionId": session, "result": done }),
        json!({ "type": "session.end", "sessionId": session, "exitCode": exit_code, "sawResult": saw_result }),
    ]
}
