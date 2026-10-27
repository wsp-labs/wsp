// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-run.test.ts at 65f6b89c7, case for case. A reply's shell block run where it stands, as the
//! thread keeps it: the run's start and end recorded on the thread so every window and a reload draw the same block, its
//! output kept to the tail, and a run that ended staying ended whichever window reports it again.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;
use wsp_core::types::events::ANY_EVENT;
use wsp_wire::types::run_block::RUN_OUTPUT_MAX_CHARS;
use wsp_wire::views::session_events_more::RunStep;
use wsp_wire::views::workspace::Caller;

fn replying(text: &str) -> Rc<Scripted> {
    let text = text.to_string();
    Scripted::new().cwd("/root/app").reply(move |_| text.clone()).pieces(1)
}

fn step(doc: Value) -> RunStep {
    serde_json::from_value(doc).expect("a run step")
}

async fn runs_of(rt: &wsp_serve::AssembledRuntime, ws: &str) -> Vec<Value> {
    rt.sessions().history(ws, None).await.expect("a history").iter().map(j).filter(|e| e["type"] == "session.run").collect()
}

struct Thread {
    fx: Fx,
    ws: String,
    thread_id: String,
    turn_id: String,
}

async fn finished_thread(reply: &str, prompt: &str) -> Thread {
    let fx = runtime(&StubBackend::new(None)).adapter("claude", replying(reply).factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let handle = fx.rt.sessions().start(&ws, StartOptions { prompt: prompt.into(), ..Default::default() }, None).await.expect("a start");
    handle.finished().await;
    let view = j(&handle.view());
    Thread { fx, ws, thread_id: view["threadId"].as_str().expect("a thread").into(), turn_id: view["id"].as_str().expect("a turn").into() }
}

// "a reply's block run where it stands" > "is recorded on the thread from its start to its end, and every window hears each step"
#[test]
fn is_recorded_on_the_thread_from_its_start_to_its_end_and_every_window_hears_each_step() {
    drive(|| async {
        let t = finished_thread("Stop them:\n\n```sh\nkill 60082 60083\n```", "stop the servers").await;
        let rt = t.fx.rt.clone();
        let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
        let kept = heard.clone();
        let _ = rt.events().on(
            ANY_EVENT,
            Rc::new(move |e| {
                let e = serde_json::to_value(e).expect("an event");
                if e["type"] == "session.run" {
                    kept.borrow_mut().push(e);
                }
                Ok(())
            }),
        );
        let block = format!("{}:m0@12", t.turn_id);
        let started = rt
            .sessions()
            .run(
                step(json!({ "threadId": t.thread_id, "turnId": t.turn_id, "runId": "run-1", "block": block, "command": "kill 60082 60083", "state": "running", "ptyId": "pty_7" })),
                None,
            )
            .await
            .expect("a run");
        // The typed answer is the event's fields without the `type` tag the wire adds: the tag is the type's own constant.
        assert_eq!(wsp_wire::views::session_events_more::SessionRunEvent::TYPE, "session.run");
        assert_matches(
            &j(&started),
            json!({ "workspaceId": t.ws, "threadId": t.thread_id, "turnId": t.turn_id, "runId": "run-1", "block": block, "command": "kill 60082 60083", "state": "running", "ptyId": "pty_7" }),
        );
        let ended = rt
            .sessions()
            .run(
                step(json!({ "threadId": t.thread_id, "turnId": t.turn_id, "runId": "run-1", "block": block, "command": "kill 60082 60083", "state": "exited", "exitCode": 1, "output": "kill: 60082: No such process" })),
                None,
            )
            .await
            .expect("a run");
        assert_matches(&j(&ended), json!({ "state": "exited", "exitCode": 1, "output": "kill: 60082: No such process" }));
        assert_eq!(
            heard.borrow().iter().map(|e| e["state"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>(),
            ["running", "exited"]
        );
        // The thread's own history carries both, so a reload folds the same block.
        assert_eq!(
            runs_of(&rt, &t.ws).await.iter().map(|e| json!([e["runId"], e["state"]])).collect::<Vec<_>>(),
            [json!(["run-1", "running"]), json!(["run-1", "exited"])]
        );
        rt.close().await.expect("a close");
    });
}

// "a reply's block run where it stands" > "keeps a long output's tail, and a run that ended stays as it ended when another window reports it again"
#[test]
fn keeps_a_long_outputs_tail_and_a_run_that_ended_stays_as_it_ended_when_another_window_reports_it_again() {
    drive(|| async {
        let t = finished_thread("```sh\nyes\n```", "go").await;
        let rt = t.fx.rt.clone();
        let base = |state: Value| {
            let mut doc = json!({ "threadId": t.thread_id, "turnId": t.turn_id, "runId": "run-2", "block": format!("{}:m0@0", t.turn_id), "command": "yes" });
            doc.as_object_mut().expect("an object").extend(state.as_object().expect("an object").clone());
            step(doc)
        };
        let long = (0..40_000).map(|i| format!("y {i}")).collect::<Vec<_>>().join("\n");
        let ended = rt.sessions().run(base(json!({ "state": "exited", "exitCode": 130, "output": long })), None).await.expect("a run");
        let output = j(&ended)["output"].as_str().expect("an output").to_string();
        assert!(output.chars().count() as u64 <= RUN_OUTPUT_MAX_CHARS);
        assert!(output.ends_with("y 39999"));
        let again = rt.sessions().run(base(json!({ "state": "lost" })), None).await.expect("a run");
        assert_matches(&j(&again), json!({ "state": "exited", "exitCode": 130 }));
        assert_eq!(runs_of(&rt, &t.ws).await.len(), 1);
        rt.close().await.expect("a close");
    });
}

// "a reply's block run where it stands" > "is refused for a thread this host does not hold, and to a thread's own token"
#[test]
fn is_refused_for_a_thread_this_host_does_not_hold_and_to_a_threads_own_token() {
    drive(|| async {
        let t = finished_thread("ok", "go").await;
        let rt = t.fx.rt.clone();
        let run = |thread: &str| {
            step(json!({ "threadId": thread, "turnId": t.turn_id, "runId": "r", "block": "b", "command": "ls", "state": "running" }))
        };
        let refused = rt.sessions().run(run("nope"), None).await.expect_err("a refusal");
        assert!(refused.message.contains("no thread"), "{}", refused.message);
        let by: wsp_wire::views::workspace_more::ThreadScope = serde_json::from_value(json!({
            "kind": "thread", "threadId": t.thread_id, "workspaceId": t.ws, "rootThreadId": t.thread_id,
        }))
        .expect("a scope");
        let thread = Caller::Scoped { origin: wsp_wire::views::workspace_more::WorkspaceOrigin::Relayed, by };
        assert!(rt.sessions().run(run(&t.thread_id), Some(thread)).await.is_err());
        rt.close().await.expect("a close");
    });
}
