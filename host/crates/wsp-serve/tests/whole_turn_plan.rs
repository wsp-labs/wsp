// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/turn-plan.test.ts at 65f6b89c7, case for case. The agent's plan as its adapter reads it: each
//! `turn.plan` is recorded as the turn's `session.plan`, under the turn and the thread, as the adapter sent it.
mod common;

use std::sync::Arc;

use common::adapter::{event, result};
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, TurnResult};
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;

fn steps() -> Value {
    json!([{ "text": "read the ticket", "state": "done" }, { "text": "write the test", "state": "working" }])
}

struct Planning;

impl HarnessAdapter for Planning {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let session_id = "sess-plan".to_string();
        let on_event = o.on_event.clone().expect("an event sink");
        let done = result(json!({ "status": "completed", "text": "done" }));
        let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
        let (id, finish) = (session_id.clone(), done.clone());
        tokio::task::spawn_local(async move {
            on_event(event(json!({ "type": "session.start", "sessionId": id })));
            on_event(event(json!({ "type": "turn.plan", "sessionId": id, "steps": steps() })));
            on_event(event(json!({ "type": "turn.plan", "sessionId": id, "text": "# Plan\n\n1. Test first" })));
            on_event(event(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&finish).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(finish);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(PlanSession { local_id: session_id, finished: finished.shared() }))
    }
}

struct PlanSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for PlanSession {
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

fn planning() -> HarnessAdapterFactory {
    Arc::new(|_ctx| Ok(Arc::new(Planning) as Arc<dyn HarnessAdapter>))
}

// "the agent's plan" > "is recorded under the turn and the thread, steps and text as the adapter read them"
#[test]
fn is_recorded_under_the_turn_and_the_thread_steps_and_text_as_the_adapter_read_them() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let fx = runtime(&backend).adapter("claude", planning()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("plans"), None).await.expect("a workspace").view.id;
        let events = fx.events();
        let handle =
            fx.rt.sessions().start(&ws, StartOptions { prompt: "plan it".into(), ..Default::default() }, None).await.expect("a start");
        handle.finished().await;
        let start = events.of("session.start").into_iter().find(|e| e["prompt"] == "plan it").expect("the start");
        let plans = events.of("session.plan");
        let thread = j(&handle.view().thread_id);
        assert_eq!(plans.len(), 2);
        assert_matches(&plans[0], json!({ "type": "session.plan", "turnId": start["turnId"], "threadId": thread, "steps": steps() }));
        assert_matches(
            &plans[1],
            json!({ "type": "session.plan", "turnId": start["turnId"], "threadId": thread, "text": "# Plan\n\n1. Test first" }),
        );
        fx.rt.close().await.expect("a close");
    });
}
