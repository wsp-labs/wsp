// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/waiting-behind.test.ts at 65f6b89c7, case for case. Two threads where one is waiting on the
//! other: the caller's own call is a wsp send that cannot answer until the thread it names is done, and that thread is
//! stopped on a question. Each test names the TS test above it.
mod common;

use std::rc::Rc;

use common::driven::{driven, Turns};
use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::{AnswerAsk, StartOptions};
use wsp_core::types::wiring::SessionHandle;
use wsp_serve::AssembledRuntime;
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::session_events::SessionEvent;
use wsp_words::thread_state::{thread_word_of, waiting_line};
use wsp_words::views::session::fold_threads;
use wsp_words::words::turn::{turn_settled_parts, TurnClocks};

struct Pair {
    fx: Fx,
    turns: Rc<Turns>,
    workspace_id: String,
    caller: Rc<dyn SessionHandle>,
    target: Rc<dyn SessionHandle>,
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn wired() -> (Fx, Rc<Turns>, FakeClock) {
    let backend = StubBackend::new(None);
    let turns = Turns::new();
    let wait = FakeClock::at_now();
    let fx = runtime(&backend).clock(&wait).adapter("claude", driven(&turns)).build();
    (fx, turns, wait)
}

/// The two threads of the story: the one that sends, and the one it sends to.
async fn pair() -> Pair {
    let (fx, turns, _) = wired();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    let caller = fx.rt.sessions().start(&ws.view.id, start("ask the other one"), None).await.expect("a start");
    until(|| async { turns.len() == 1 }).await;
    let target = fx.rt.sessions().start(&ws.view.id, start("read the file"), None).await.expect("a start");
    until(|| async { turns.len() == 2 }).await;
    Pair { fx, turns, workspace_id: ws.view.id, caller, target }
}

async fn thread_of(rt: &AssembledRuntime, workspace_id: &str, handle: &Rc<dyn SessionHandle>) -> ThreadView {
    let rows = rt.sessions().list(Some(workspace_id.to_string()), None).await.expect("a list");
    let id = handle.view().thread_id;
    fold_threads(&rows).into_iter().find(|t| t.thread_id == id).expect("the thread")
}

async fn word_of(p: &Pair, handle: &Rc<dyn SessionHandle>) -> String {
    thread_word_of(&thread_of(&p.fx.rt, &p.workspace_id, handle).await)
}

// "a thread waiting on another thread's turn" > "reads as waiting on the person, carries the other thread's question, and one answer ends both waits"
#[test]
fn reads_as_waiting_on_the_person_carries_the_other_threads_question_and_one_answer_ends_both_waits() {
    drive(|| async {
        let p = pair().await;
        let rt = &*p.fx.rt;
        let target_thread = p.target.view().thread_id.expect("a thread");
        // The caller's agent calls send and the call is still running; nothing has asked anybody anything yet.
        p.turns.get(0).calls("toolu_send", "mcp__wsp__send", &json!({ "thread": target_thread, "message": "read hello.txt" }).to_string());
        assert_eq!(word_of(&p, &p.caller).await, "Working");
        // The thread it sent to stops on a question. Nothing on the caller's own row changed, and the caller is stuck.
        p.turns.get(1).raise(json!({}));
        until(|| async { word_of(&p, &p.caller).await == "Needs you" }).await;
        let caller = thread_of(rt, &p.workspace_id, &p.caller).await;
        let behind = caller.waiting_on.clone().expect("waitingOn");
        assert_matches(
            &j(&behind),
            json!({ "threadId": target_thread, "workspaceId": p.workspace_id, "title": "read the file", "prompt": { "askId": "ask_1", "toolName": "Read" } }),
        );
        let options: Vec<Value> = j(&behind)["prompt"]["options"].as_array().expect("options").iter().map(|o| o["id"].clone()).collect();
        assert_eq!(options, [json!("allow"), json!("deny")]);
        // The caller was asked nothing of its own; the line it shows is the other thread's question.
        assert!(caller.asking.is_none());
        assert_eq!(waiting_line(&caller).as_deref(), Some("Permission for Read: hello.txt needs an answer"));
        // The prompt is answered off the caller's row, which is where the person is reading: the session it names is
        // the other thread's turn, so one click ends the wait on both.
        let answered = rt
            .sessions()
            .answer(&behind.session_id, AnswerAsk { ask_id: behind.prompt.ask_id.clone(), option_id: "allow".into(), reason: None }, None)
            .await
            .expect("an answer");
        assert_eq!(j(&answered), json!({ "outcome": "answered" }));
        until(|| async { word_of(&p, &p.target).await == "Working" }).await;
        assert_eq!(word_of(&p, &p.caller).await, "Working");
        assert!(j(&thread_of(rt, &p.workspace_id, &p.caller).await).get("waitingOn").is_none());
        p.turns.get(1).reply(json!({}));
        p.turns.get(0).answers("toolu_send");
        p.turns.get(0).reply(json!({}));
        p.caller.finished().await;
        p.target.finished().await;
        rt.close().await.expect("a close");
    });
}

// "follows only a call that waits for the other thread: a detached send answers at once and is behind nobody"
#[test]
fn follows_only_a_call_that_waits_for_the_other_thread_a_detached_send_answers_at_once_and_is_behind_nobody() {
    drive(|| async {
        let p = pair().await;
        let target_thread = p.target.view().thread_id.expect("a thread");
        p.turns.get(0).calls(
            "toolu_send",
            "mcp__wsp__send",
            &json!({ "thread": target_thread, "message": "go", "detach": true }).to_string(),
        );
        p.turns.get(1).raise(json!({}));
        until(|| async { word_of(&p, &p.target).await == "Needs you" }).await;
        assert_eq!(word_of(&p, &p.caller).await, "Working");
        // A call of the agent's own, into no thread at all, is no wait either.
        p.turns.get(0).calls("toolu_read", "Read", &json!({ "file_path": "/root/notes.md" }).to_string());
        assert_eq!(word_of(&p, &p.caller).await, "Working");
        p.turns.get(1).reply(json!({}));
        p.turns.get(0).reply(json!({}));
        p.caller.finished().await;
        p.target.finished().await;
        p.fx.rt.close().await.expect("a close");
    });
}

// "drops the wait when the call the thread was inside answers, whatever the other thread is still doing"
#[test]
fn drops_the_wait_when_the_call_the_thread_was_inside_answers_whatever_the_other_thread_is_still_doing() {
    drive(|| async {
        let p = pair().await;
        let target_thread = p.target.view().thread_id.expect("a thread");
        p.turns.get(0).calls("toolu_wait", "mcp__wsp__threads_wait", &json!({ "threads": [target_thread], "timeout": 60 }).to_string());
        p.turns.get(1).raise(json!({}));
        until(|| async { word_of(&p, &p.caller).await == "Needs you" }).await;
        // The wait gave up on its deadline while the question still stands: the caller is working again, and the thread
        // that was asked still is not.
        p.turns.get(0).answers("toolu_wait");
        until(|| async { word_of(&p, &p.caller).await == "Working" }).await;
        assert_eq!(word_of(&p, &p.target).await, "Needs you");
        p.turns.get(1).reply(json!({}));
        p.turns.get(0).reply(json!({}));
        p.caller.finished().await;
        p.target.finished().await;
        p.fx.rt.close().await.expect("a close");
    });
}

// "what a settled turn reports as worked" > "takes off the minutes it stood on a question, and says where they went when they are most of it"
#[test]
fn takes_off_the_minutes_it_stood_on_a_question_and_says_where_they_went_when_they_are_most_of_it() {
    drive(|| async {
        let (fx, turns, wait) = wired();
        let rt = &*fx.rt;
        let ws = create_on(rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let handle = rt.sessions().start(&ws.view.id, start("read the file"), None).await.expect("a start");
        until(|| async { turns.len() == 1 }).await;
        turns.get(0).raise(json!({}));
        until(|| async { rt.sessions().list(Some(ws.view.id.clone()), None).await.expect("a list")[0].asking.is_some() }).await;
        wait.advance(60_000);
        rt.sessions()
            .answer(&handle.view().id, AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None }, None)
            .await
            .expect("an answer");
        until(|| async { rt.sessions().list(Some(ws.view.id.clone()), None).await.expect("a list")[0].asking.is_none() }).await;
        // The harness counts wall time from launch to result: a minute of it was the person.
        turns.get(0).reply(json!({ "durationMs": 64000, "costUsd": 0.05 }));
        handle.finished().await;
        let history = rt.sessions().history(&ws.view.id, None).await.expect("a history");
        let done: SessionEvent = history.into_iter().find(|e| j(e)["type"] == "session.done").expect("session.done");
        let result = j(&done)["result"].clone();
        assert_matches(&result, json!({ "durationMs": 64000, "waitedMs": 60000 }));
        let result = serde_json::from_value(result).expect("a result");
        assert_eq!(turn_settled_parts(&TurnClocks::from(&result), None, None), ["Worked for 4.0s", "waited on you 1m", "$0.05"]);
        rt.close().await.expect("a close");
    });
}
