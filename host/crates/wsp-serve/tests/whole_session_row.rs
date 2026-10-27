// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-row.test.ts at 65f6b89c7, case for case. A row that moves goes out as that one row: a
//! thread's start, end, prompt or mark pushes the row a listing would answer for it, and nothing reads the workspace's
//! whole list again.
#[macro_use]
mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::*;
use serde_json::{json, Value};
use wsp_core::clock::{Clock, RealClock, Timer};
use wsp_core::types::api::{AnswerAsk, StartOptions};
use wsp_words::words::turn::kept_opening;

const BRIEF_UNIT: &str = "Build the ticket. ";

fn brief() -> String {
    BRIEF_UNIT.repeat(600)
}

fn ask() -> Value {
    json!({
        "askId": "ask_1",
        "toolName": "Read",
        "toolUseId": "toolu_read",
        "input": "{\"file_path\":\"/root/hello.txt\"}",
        "detail": "hello.txt",
        "options": [
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
        ],
    })
}

/// `{ now: () => Date.now() + skew.ms, schedule: realClock.schedule }`.
struct Skewed(Rc<Cell<i64>>);

impl Clock for Skewed {
    fn now(&self) -> i64 {
        RealClock.now() + self.0.get()
    }
    fn schedule(&self, f: Timer, ms: i64) -> wsp_core::clock::Cancel {
        RealClock.schedule(f, ms)
    }
}

struct Row {
    fx: Fx,
    driven: Rc<Driven>,
    events: Events,
    skew: Rc<Cell<i64>>,
}

fn start() -> Row {
    let backend = StubBackend::new(None);
    let driven = Driven::new();
    let skew = Rc::new(Cell::new(0));
    let clock_skew = skew.clone();
    let fx = runtime(&backend).adapter("claude", driven.factory()).with(move |o| o.clock = Some(Rc::new(Skewed(clock_skew)))).build();
    let events = fx.events();
    Row { fx, driven, events, skew }
}

/// `rowsOf(from, threadId)`: the `session.row` events of one thread after the first `from` events heard.
fn rows_of(events: &Events, from: usize, thread_id: &str) -> Vec<Value> {
    events.all().into_iter().skip(from).filter(|e| e["type"] == "session.row" && e["threadId"] == thread_id).collect()
}

fn opts(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn on_thread(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), thread: Some(thread.into()), ..Default::default() }
}

fn thread_of(view: &Value) -> String {
    view["threadId"].as_str().expect("a thread id").to_string()
}

fn status_of(row: &Value) -> Option<&str> {
    row["row"]["status"].as_str()
}

// "a row that moves" > "in a workspace of 400 threads, a turn's start and end each push that thread's one row, the end under 5 KB"
// (The TS counts `rt.sessions.list` calls through a spy on the public object; the Rust runtime's `sessions()` is a
// trait object whose internal callers never pass through a wrapper a test can put in, so `lists` is not counted.)
#[test]
fn in_a_workspace_of_400_threads_a_turns_start_and_end_each_push_that_threads_one_row_the_end_under_5_kb() {
    drive(|| async {
        let r = start();
        let rt = r.fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("boat"), None).await.expect("a workspace").view.id;
        r.driven.quick.set(true);
        let mut thread_ids = vec![];
        for k in 0..400 {
            let handle = rt.sessions().start(&ws, opts(&format!("{k} {}", brief())), None).await.expect("a start");
            handle.finished().await;
            thread_ids.push(thread_of(&j(&handle.view())));
        }
        r.driven.quick.set(false);
        let listed = rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        assert!(serde_json::to_string(&j(&listed)).expect("json").len() > 400_000);

        let thread_id = thread_ids.last().expect("a thread").clone();
        let from = r.events.len();
        let handle = rt.sessions().start(&ws, on_thread("go on", &thread_id), None).await.expect("a start");
        let events = r.events.clone();
        let t = thread_id.clone();
        wait!([events, t] rows_of(&events, from, &t).iter().any(|e| status_of(e) == Some("running") && !e["row"]["claudeSessionId"].is_null()));
        let ending = r.events.len();
        r.driven.last().reply();
        handle.finished().await;
        let events = r.events.clone();
        let t = thread_id.clone();
        wait!([events, t] rows_of(&events, ending, &t).last().and_then(|e| status_of(e).map(str::to_string)).as_deref() == Some("completed"));

        let ended = rows_of(&r.events, ending, &thread_id).pop().expect("a row");
        let view = j(&handle.view());
        assert_eq!(ended["id"], view["id"]);
        assert_matches(
            &ended["row"],
            json!({ "id": view["id"], "threadId": thread_id, "workspaceId": ws, "status": "completed", "harness": "claude" }),
        );
        // Every row of the thread repeats its brief's head, which the window was told with the row's first push.
        assert!(ended["row"].get("prompt").is_none());
        let sent: usize = r.events.all().iter().skip(ending).map(|e| serde_json::to_string(e).expect("json").len()).sum();
        assert!(sent < 5_000, "{sent}");
    });
}

// "a row that moves" > "carries a row's opening prompt on the first push under each id, and drops the id a launch left"
#[test]
fn carries_a_rows_opening_prompt_on_the_first_push_under_each_id_and_drops_the_id_a_launch_left() {
    drive(|| async {
        let r = start();
        let rt = r.fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle = rt.sessions().start(&ws, opts(&brief()), None).await.expect("a start");
        let thread_id = thread_of(&j(&handle.view()));
        let events = r.events.clone();
        let t = thread_id.clone();
        wait!([events, t] !rows_of(&events, 0, &t).is_empty());
        r.driven.turn(0).reply();
        handle.finished().await;
        let events = r.events.clone();
        let t = thread_id.clone();
        wait!([events, t] rows_of(&events, 0, &t).last().and_then(|e| status_of(e).map(str::to_string)).as_deref() == Some("completed"));
        let pushed = rows_of(&r.events, 0, &thread_id);
        let mut ids: Vec<String> = vec![];
        for e in &pushed {
            let id = e["id"].as_str().expect("an id").to_string();
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        for id in &ids {
            let with_rows: Vec<&Value> = pushed.iter().filter(|e| &e["id"] == id && !e["row"].is_null()).collect();
            let (first, rest) = with_rows.split_first().expect("a row under the id");
            assert_eq!(first["row"]["prompt"], json!(kept_opening(&brief())));
            assert!(rest.iter().all(|e| e["row"].get("prompt").is_none()));
        }
        // The row lives under the harness's session id once the turn is up, and every other id it went by is dropped.
        let own = j(&handle.view())["id"].clone();
        let left: Vec<Value> = ids
            .iter()
            .filter(|id| json!(id) != own)
            .map(|id| pushed.iter().rfind(|e| e["id"] == json!(id)).map_or(Value::Null, |e| e["row"].clone()))
            .collect();
        let want: Vec<Value> = ids.iter().skip(1).map(|_| Value::Null).collect();
        assert_eq!(left, want);
    });
}

// "a row that moves" > "pushes a prompt's own row and the row of a thread stopped behind it, and both again when it is answered"
#[test]
fn pushes_a_prompts_own_row_and_the_row_of_a_thread_stopped_behind_it_and_both_again_when_it_is_answered() {
    drive(|| async {
        let r = start();
        let rt = r.fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let caller = rt.sessions().start(&ws, opts("ask the other one"), None).await.expect("a start");
        let driven = r.driven.clone();
        wait!([driven] driven.len() == 1);
        let target = rt.sessions().start(&ws, opts("read the file"), None).await.expect("a start");
        let driven = r.driven.clone();
        wait!([driven] driven.len() == 2);
        let caller_thread = thread_of(&j(&caller.view()));
        let target_thread = thread_of(&j(&target.view()));
        r.driven.turn(0).calls("toolu_wait", "mcp__wsp__threads_wait", &json!({ "threads": [target_thread], "timeout": 60 }).to_string());

        let asked = r.events.len();
        r.driven.turn(1).raise(ask());
        let events = r.events.clone();
        let (c, t) = (caller_thread.clone(), target_thread.clone());
        wait!([events, c, t] rows_of(&events, asked, &c).last().is_some_and(|e| e["row"]["waitingOn"]["threadId"] == json!(t)));
        assert!(!rows_of(&r.events, asked, &target_thread).last().expect("a row")["row"]["asking"].is_null());

        let answered = r.events.len();
        rt.sessions()
            .answer(
                j(&target.view())["id"].as_str().expect("an id"),
                AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None },
                None,
            )
            .await
            .expect("an answer");
        let events = r.events.clone();
        let c = caller_thread.clone();
        wait!([events, c] rows_of(&events, answered, &c).last().is_some_and(|e| !e["row"].is_null()));
        assert!(rows_of(&r.events, answered, &caller_thread).last().expect("a row")["row"].get("waitingOn").is_none());
        let events = r.events.clone();
        let t = target_thread.clone();
        wait!([events, t] rows_of(&events, answered, &t).last().is_some_and(|e| !e["row"].is_null()));
        assert!(rows_of(&r.events, answered, &target_thread).last().expect("a row")["row"].get("asking").is_none());

        r.driven.turn(1).reply();
        r.driven.turn(0).answers("toolu_wait");
        r.driven.turn(0).reply();
        caller.finished().await;
        target.finished().await;
    });
}

// "a row that moves" > "pushes the row a read mark moved"
#[test]
fn pushes_the_row_a_read_mark_moved() {
    drive(|| async {
        let r = start();
        let rt = r.fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        r.driven.quick.set(true);
        let handle = rt.sessions().start(&ws, opts("one"), None).await.expect("a start");
        handle.finished().await;
        let view = j(&handle.view());
        let thread_id = thread_of(&view);
        let from = r.events.len();
        rt.sessions().read(&thread_id, None).await.expect("a read");
        let ended_at = view["endedAt"].as_f64().expect("an end");
        let events = r.events.clone();
        let t = thread_id.clone();
        wait!([events, t] rows_of(&events, from, &t).last().is_some_and(|e| e["row"]["readAt"].as_f64().is_some_and(|at| at >= ended_at)));
    });
}

// "a row that moves" > "pushes every row a failed batch held with the next one"
// Not ported: the TS fails the push by mocking `node:os`'s `hostname` for one read (`vi.mock("node:os")`); the Rust host
// reads `libc::gethostname` (wsp-core/src/host_id.rs:56) with no seam a test can make fail, so the case has no Rust form.

// "a row that moves" > "pushes a thread's row once a rename made in the harness is read, which the next row to move asks for"
#[test]
fn pushes_a_threads_row_once_a_rename_made_in_the_harness_is_read_which_the_next_row_to_move_asks_for() {
    drive(|| async {
        let r = start();
        let rt = r.fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        r.driven.quick.set(true);
        let one = rt.sessions().start(&ws, opts("one"), None).await.expect("a start");
        one.finished().await;
        let two = rt.sessions().start(&ws, opts("two"), None).await.expect("a start");
        two.finished().await;
        let one_view = j(&one.view());
        let thread_id = thread_of(&one_view);
        r.driven
            .titles
            .borrow_mut()
            .insert(one_view["claudeSessionId"].as_str().expect("a session").to_string(), "named in the harness".into());
        r.skew.set(60_000);
        let from = r.events.len();
        rt.sessions().read(&thread_of(&j(&two.view())), None).await.expect("a read");
        let events = r.events.clone();
        let t = thread_id.clone();
        wait!([events, t] rows_of(&events, from, &t).last().is_some_and(|e| e["row"]["harnessTitle"] == "named in the harness"));
    });
}

// "a row that moves" > "pushes the row of a settled thread a restart replaces, naming the restart"
#[test]
fn pushes_the_row_of_a_settled_thread_a_restart_replaces_naming_the_restart() {
    drive(|| async {
        let r = start();
        let rt = r.fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        r.driven.quick.set(true);
        let old = rt.sessions().start(&ws, opts("the first try"), None).await.expect("a start");
        old.finished().await;
        let replaced = thread_of(&j(&old.view()));
        rt.sessions().settle(vec![replaced.clone()], None, Default::default()).await.expect("a settle");
        let from = r.events.len();
        let restart = rt
            .sessions()
            .start(&ws, StartOptions { prompt: "the second try".into(), replaces: Some(replaced.clone()), ..Default::default() }, None)
            .await
            .expect("a start");
        restart.finished().await;
        let restart_thread = thread_of(&j(&restart.view()));
        let events = r.events.clone();
        let t = replaced.clone();
        wait!([events, t, restart_thread] rows_of(&events, from, &t).last().is_some_and(|e| e["row"]["replacedBy"] == json!(restart_thread)));
    });
}
