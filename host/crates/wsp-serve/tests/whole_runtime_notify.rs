// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-notify.test.ts at 65f6b89c7, case for case. A thread whose start named who to tell: the line
//! of its end goes to the parent by a steer or by a turn of the parent's own, to the person as a row only, and survives a nap
//! and a host restart.
#[macro_use]
mod common;

use std::rc::Rc;

use common::comeback::{has_event, history, rows};
use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::StartOptions;
use wsp_wire::types::env::TURN_TOKEN_ENV;
use wsp_wire::views::session::SessionOrigin;
use wsp_words::words::thread::NO_SUCH_TURN;
use wsp_words::words::turn::{notify_line, NotifyLength};

fn opts(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn tell(prompt: &str, targets: &[&str]) -> StartOptions {
    StartOptions { prompt: prompt.into(), notify: Some(targets.iter().map(|t| t.to_string()).collect()), ..Default::default() }
}

fn by_agent(mut o: StartOptions) -> StartOptions {
    o.started_by = Some(SessionOrigin::Agent);
    o
}

fn thread(h: &Rc<dyn wsp_core::types::wiring::SessionHandle>) -> String {
    j(&h.view())["threadId"].as_str().expect("a thread").to_string()
}

fn sid(h: &Rc<dyn wsp_core::types::wiring::SessionHandle>) -> String {
    j(&h.view())["claudeSessionId"].as_str().expect("a session").to_string()
}

fn short(id: &str) -> String {
    id.chars().take(8).collect()
}

fn types_of(events: &[Value]) -> Vec<String> {
    events.iter().map(|e| e["type"].as_str().unwrap_or_default().to_string()).collect()
}

fn of_thread(events: Vec<Value>, thread: &str) -> Vec<Value> {
    events.into_iter().filter(|e| e["threadId"] == thread).collect()
}

fn result_of(doc: Value) -> wsp_adapters::port::TurnResult {
    serde_json::from_value(doc).expect("a turn result")
}

struct Fix {
    h: Rc<Held>,
    fx: Fx,
    ws: String,
}

async fn fixture(steers: bool) -> Fix {
    let h = Held::new(steers);
    let fx = runtime(&StubBackend::new(None)).adapter("claude", h.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    Fix { h, fx, ws }
}

async fn started(f: &Fix, o: StartOptions) -> Rc<dyn wsp_core::types::wiring::SessionHandle> {
    f.fx.rt.sessions().start(&f.ws, o, None).await.expect("a start")
}

fn steered_len(h: &Rc<Held>) -> usize {
    h.steered.borrow().len()
}

fn starts_len(h: &Rc<Held>) -> usize {
    h.starts.borrow().len()
}

// "a thread whose start named who to tell" > "a running parent that steers is told by one steer: the line, with the outcome, duration, cost and the reply whole, and the child's transcript holds a session.notify naming the parent"
#[test]
fn a_running_parent_that_steers_is_told_by_one_steer_the_line_with_the_outcome_duration_cost_and_the_reply_whole_and_the_childs_transcript_holds_a_session_notify_naming_the_parent(
) {
    drive(|| async {
        let f = fixture(true).await;
        let parent = started(&f, opts("orchestrate")).await;
        let parent_thread = thread(&parent);
        let kid = started(&f, by_agent(tell("build it", &[&parent_thread]))).await;
        let kid_thread = thread(&kid);
        assert_ne!(kid_thread, parent_thread);
        f.h.end(1, "Ran the gate.\n\nAll 12 tests green.\n", json!({ "durationMs": 492_000, "costUsd": 1.94 }));
        let line = format!("thread {} finished (completed, 8m 12s, $1.94): Ran the gate.\n\nAll 12 tests green.", short(&kid_thread));
        let h = f.h.clone();
        let want = vec![line.clone()];
        wait!([h, want] * h.steered.borrow() == want);
        assert_eq!(f.h.starts.borrow().iter().map(|s| s.prompt.clone()).collect::<Vec<_>>(), ["orchestrate", "build it"]);
        let all = history(&f.fx.rt, &f.ws).await;
        assert_eq!(types_of(&of_thread(all.clone(), &kid_thread)), ["session.start", "session.notify", "session.done", "session.end"]);
        let notify = all.iter().find(|e| e["type"] == "session.notify").expect("a notify");
        assert_matches(
            notify,
            json!({ "threadId": kid_thread, "turnId": kid.turn_id(), "sessionId": sid(&kid), "notify": parent_thread, "text": line }),
        );
        assert_eq!(types_of(&of_thread(all.clone(), &parent_thread)), ["session.start", "session.steer"]);
        let steer = all.iter().find(|e| e["type"] == "session.steer").expect("a steer");
        assert_matches(steer, json!({ "threadId": parent_thread, "turnId": parent.turn_id(), "prompt": line }));
        f.h.end(0, "handled", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "an idle parent is told by a turn of its own: a start that resumes the parent's session with the line as its prompt"
#[test]
fn an_idle_parent_is_told_by_a_turn_of_its_own_a_start_that_resumes_the_parents_session_with_the_line_as_its_prompt() {
    drive(|| async {
        let f = fixture(false).await;
        let parent =
            started(&f, StartOptions { prompt: "orchestrate".into(), started_by: Some(SessionOrigin::Cli), ..Default::default() }).await;
        let parent_thread = thread(&parent);
        let parent_sid = sid(&parent);
        f.h.end(0, "waiting for the builder", json!({}));
        parent.finished().await;
        let kid = started(&f, tell("build it", &[&parent_thread])).await;
        let kid_thread = thread(&kid);
        f.h.end(1, "done", json!({ "durationMs": 1_500, "costUsd": 0.0042 }));
        let h = f.h.clone();
        wait!([h] starts_len(&h) == 3);
        let line = format!("thread {} finished (completed, 1.5s, $0.00): done", short(&kid_thread));
        assert_matches(
            &json!({ "prompt": f.h.starts.borrow()[2].prompt, "resume": f.h.starts.borrow()[2].resume }),
            json!({ "prompt": line, "resume": parent_sid }),
        );
        assert!(f.h.steered.borrow().is_empty());
        let listed = rows(&f.fx.rt, &f.ws).await;
        let mine: Vec<Value> =
            listed.iter().filter(|r| r["threadId"] == parent_thread).map(|r| json!([r["status"], r["prompt"], r["startedBy"]])).collect();
        assert_eq!(mine, [json!(["running", "orchestrate", "cli"])]);
        let all = history(&f.fx.rt, &f.ws).await;
        assert_eq!(types_of(&of_thread(all.clone(), &parent_thread)), ["session.start", "session.done", "session.end", "session.start"]);
        assert_matches(all.last().expect("an event"), json!({ "type": "session.start", "threadId": parent_thread, "prompt": line }));
        f.h.end(2, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a child's finished line taken by its lead marks the child read, by steer or by a turn of the lead's own; the child's report is read there"
#[test]
fn a_childs_finished_line_taken_by_its_lead_marks_the_child_read_by_steer_or_by_a_turn_of_the_leads_own_the_childs_report_is_read_there() {
    for steers in [true, false] {
        drive(move || async move {
            let f = fixture(steers).await;
            let parent = started(&f, opts("orchestrate")).await;
            let parent_thread = thread(&parent);
            if !steers {
                f.h.end(0, "waiting for the builder", json!({}));
                parent.finished().await;
            }
            let kid = started(&f, by_agent(tell("build it", &[&parent_thread]))).await;
            let kid_thread = thread(&kid);
            let kid_row = |rt: Rc<wsp_serve::AssembledRuntime>, ws: String, t: String| async move {
                rows(&rt, &ws).await.into_iter().find(|r| r["threadId"] == t).expect("the child's row")
            };
            let before = kid_row(f.fx.rt.clone(), f.ws.clone(), kid_thread.clone()).await["readAt"].as_f64().unwrap_or(0.0);
            f.h.end(1, "all green", json!({}));
            kid.finished().await;
            let h = f.h.clone();
            wait!([h] if steers { steered_len(&h) } else { starts_len(&h) - 2 } == 1);
            let (rt, ws, t) = (f.fx.rt.clone(), f.ws.clone(), kid_thread.clone());
            wait!([rt, ws, t] {
                let row = rows(&rt, &ws).await.into_iter().find(|r| r["threadId"] == t).expect("the child's row");
                row["readAt"].as_f64().zip(row["endedAt"].as_f64()).is_some_and(|(read, ended)| read >= ended)
            });
            let after = kid_row(f.fx.rt.clone(), f.ws.clone(), kid_thread.clone()).await["readAt"].as_f64().unwrap_or(0.0);
            assert!(after > before);
            f.h.end(if steers { 0 } else { 2 }, "handled", json!({}));
            f.fx.rt.close().await.expect("a close");
        });
    }
}

// "a thread whose start named who to tell" > "a parent that replied while its process still runs is told once that process exits: the line waits for its session.end, then goes as a turn of its own"
#[test]
fn a_parent_that_replied_while_its_process_still_runs_is_told_once_that_process_exits_the_line_waits_for_its_session_end_then_goes_as_a_turn_of_its_own(
) {
    drive(|| async {
        let f = fixture(true).await;
        let events = f.fx.events();
        let parent = started(&f, opts("orchestrate")).await;
        let parent_thread = thread(&parent);
        let parent_sid = sid(&parent);
        f.h.reply(0, "waiting for the builder", json!({}));
        assert_eq!(j(&parent.view())["status"], "running");
        let kid = started(&f, tell("build it", &[&parent_thread])).await;
        f.h.end(1, "done", json!({ "durationMs": 1_500, "costUsd": 0.0042 }));
        let line = format!("thread {} finished (completed, 1.5s, $0.00): done", short(&thread(&kid)));
        settle().await;
        // The parent's turn has replied, so it takes no steer; the line queues behind that process, as any send does.
        assert!(f.h.steered.borrow().is_empty());
        assert_eq!(starts_len(&f.h), 2);
        let queued = events.of("session.queued");
        assert_eq!(queued.len(), 1);
        assert_matches(&queued[0], json!({ "threadId": parent_thread, "prompt": line }));
        let all = history(&f.fx.rt, &f.ws).await;
        assert_matches(
            all.iter().find(|e| e["type"] == "session.notify").expect("a notify"),
            json!({ "notify": parent_thread, "text": line }),
        );

        f.h.exit(0);
        parent.finished().await;
        let h = f.h.clone();
        wait!([h] starts_len(&h) == 3);
        assert_matches(
            &json!({ "prompt": f.h.starts.borrow()[2].prompt, "resume": f.h.starts.borrow()[2].resume }),
            json!({ "prompt": line, "resume": parent_sid }),
        );
        assert!(f.h.steered.borrow().is_empty());
        let all = history(&f.fx.rt, &f.ws).await;
        assert_eq!(types_of(&of_thread(all, &parent_thread)), ["session.start", "session.done", "session.end", "session.start"]);
        f.h.end(2, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a turn that replied and is then ended by a nap keeps its reply's status and tells its parent nothing more"
#[test]
fn a_turn_that_replied_and_is_then_ended_by_a_nap_keeps_its_replys_status_and_tells_its_parent_nothing_more() {
    drive(|| async {
        let f = fixture(false).await;
        let events = f.fx.events();
        let turn = started(&f, tell("orchestrate", &["me"])).await;
        f.h.reply(0, "the reply", json!({ "durationMs": 1_500, "costUsd": 0.0042 }));
        let line = format!("thread {} finished (completed, 1.5s, $0.00): the reply", short(&thread(&turn)));
        f.fx.rt.workspaces().nap(&f.ws, None).await.expect("a nap");
        assert_eq!(events.of("session.notify").iter().map(|e| e["text"].clone()).collect::<Vec<_>>(), [json!(line)]);
        assert_eq!(rows(&f.fx.rt, &f.ws).await[0]["status"], "completed");
        let all = history(&f.fx.rt, &f.ws).await;
        assert_eq!(types_of(&all), ["session.start", "session.notify", "session.done", "session.end"]);
        assert_matches(
            all.last().expect("an event"),
            json!({ "type": "session.end", "exitCode": null, "sawResult": true, "reason": "machine paused while the agent was working" }),
        );
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a turn that replied and whose host restarts before its process exited settles to its reply's status at load and tells its parent nothing more"
#[test]
fn a_turn_that_replied_and_whose_host_restarts_before_its_process_exited_settles_to_its_replys_status_at_load_and_tells_its_parent_nothing_more(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h1 = Held::new(false);
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h1.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let turn = rt1.rt.sessions().start(&ws, tell("orchestrate", &["me"]), None).await.expect("a start");
        h1.reply(0, "the reply", json!({ "durationMs": 1_500, "costUsd": 0.0042 }));
        let line = format!("thread {} finished (completed, 1.5s, $0.00): the reply", short(&thread(&turn)));
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", Held::new(false).factory()).build();
        let all = history(&rt2.rt, &ws).await;
        assert_eq!(types_of(&all), ["session.start", "session.notify", "session.done", "session.end"]);
        assert_eq!(all.iter().filter(|e| e["type"] == "session.notify").map(|e| e["text"].clone()).collect::<Vec<_>>(), [json!(line)]);
        assert_matches(
            all.last().expect("an event"),
            json!({ "type": "session.end", "exitCode": null, "sawResult": true, "reason": "host restarted while the agent was working" }),
        );
        assert_eq!(rows(&rt2.rt, &ws).await[0]["status"], "completed");
        rt2.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a line waiting behind its parent's turn when the host stops goes once, as the parent's next turn, after the host starts again"
#[test]
fn a_line_waiting_behind_its_parents_turn_when_the_host_stops_goes_once_as_the_parents_next_turn_after_the_host_starts_again() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h1 = Held::new(true);
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h1.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let parent = rt1.rt.sessions().start(&ws, opts("orchestrate"), None).await.expect("a start");
        let parent_thread = thread(&parent);
        h1.reply(0, "waiting for the builder", json!({}));
        let kid = rt1.rt.sessions().start(&ws, tell("build it", &[&parent_thread]), None).await.expect("a start");
        h1.end(1, "done", json!({}));
        let line = format!("thread {} finished (completed): done", short(&thread(&kid)));
        settle().await;
        assert!(h1.steered.borrow().is_empty());
        assert_eq!(starts_len(&h1), 2);
        rt1.rt.close().await.expect("a close");

        let h2 = Held::new(true);
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h2.factory()).build();
        rows(&rt2.rt, &ws).await;
        let h = h2.clone();
        wait!([h] starts_len(&h) == 1);
        assert_matches(
            &json!({ "prompt": h2.starts.borrow()[0].prompt, "resume": h2.starts.borrow()[0].resume }),
            json!({ "prompt": line, "resume": sid(&parent) }),
        );
        h2.end(0, "read it", json!({}));
        settle().await;
        assert_eq!(starts_len(&h2), 1);
        assert!(h2.steered.borrow().is_empty());
        rt2.rt.close().await.expect("a close");

        let rt3 = runtime(&backend).store(&state.store).adapter("claude", Held::new(true).factory()).build();
        rows(&rt3.rt, &ws).await;
        settle().await;
        let prompts: Vec<Value> = history(&rt3.rt, &ws)
            .await
            .iter()
            .filter(|e| e["type"] == "session.start" && e["threadId"] == parent_thread)
            .map(|e| e["prompt"].clone())
            .collect();
        assert_eq!(prompts, [json!("orchestrate"), json!(line)]);
        rt3.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a running parent that cannot steer is told once its turn ends: the line waits behind it as a queued send does"
#[test]
fn a_running_parent_that_cannot_steer_is_told_once_its_turn_ends_the_line_waits_behind_it_as_a_queued_send_does() {
    drive(|| async {
        let f = fixture(false).await;
        let events = f.fx.events();
        let parent = started(&f, opts("orchestrate")).await;
        let parent_thread = thread(&parent);
        let kid = started(&f, tell("build it", &[&parent_thread])).await;
        f.h.end(1, "done", json!({ "durationMs": 60_000, "costUsd": 0.5 }));
        let line = format!("thread {} finished (completed, 1m, $0.50): done", short(&thread(&kid)));
        let ev = events.clone();
        wait!([ev] ev.of("session.queued").len() == 1);
        assert_matches(&events.of("session.queued")[0], json!({ "threadId": parent_thread, "prompt": line }));
        settle().await;
        assert_eq!(starts_len(&f.h), 2);
        f.h.end(0, "first done", json!({}));
        let h = f.h.clone();
        wait!([h] starts_len(&h) == 3);
        assert_matches(
            &json!({ "prompt": f.h.starts.borrow()[2].prompt, "resume": f.h.starts.borrow()[2].resume }),
            json!({ "prompt": line, "resume": sid(&parent) }),
        );
        f.h.end(2, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "failed and interrupted ends carry their words; me records the line for the person and starts nothing"
#[test]
fn failed_and_interrupted_ends_carry_their_words_me_records_the_line_for_the_person_and_starts_nothing() {
    drive(|| async {
        let f = fixture(true).await;
        let events = f.fx.events();
        let failing = started(&f, tell("die", &["me"])).await;
        f.h.end(0, "", json!({ "status": "failed", "error": "the harness died", "durationMs": 3_000 }));
        let stopped = started(&f, tell("run", &["me"])).await;
        f.h.end(1, "Stopped mid-way.", json!({ "status": "interrupted", "durationMs": 12_000, "costUsd": 0.03 }));
        failing.finished().await;
        stopped.finished().await;
        let told = events.of("session.notify");
        assert_eq!(told.len(), 2);
        assert_matches(
            &told[0],
            json!({ "threadId": thread(&failing), "notify": "me", "text": format!("thread {} finished (failed, 3.0s): the harness died", short(&thread(&failing))) }),
        );
        assert_matches(
            &told[1],
            json!({ "threadId": thread(&stopped), "notify": "me", "text": format!("thread {} finished (interrupted, 12s, $0.03): Stopped mid-way.", short(&thread(&stopped))) }),
        );
        assert_eq!(starts_len(&f.h), 2);
        assert!(f.h.steered.borrow().is_empty());
        // Pushed before the turn's session.done, where a follower keyed on the turn stops reading.
        let turn = failing.turn_id();
        let kinds: Vec<String> = events
            .all()
            .iter()
            .filter(|e| e["turnId"] == json!(turn))
            .map(|e| e["type"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(kinds, ["session.start", "session.notify", "session.done", "session.end"]);
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a thread id no thread carries is refused before anything starts"
#[test]
fn a_thread_id_no_thread_carries_is_refused_before_anything_starts() {
    drive(|| async {
        let f = fixture(true).await;
        let refused = f.fx.rt.sessions().start(&f.ws, tell("build it", &["thread_nobody"]), None).await.err().expect("a refusal");
        assert!(refused.message.contains("no thread thread_nobody to notify"), "{}", refused.message);
        assert!(f.h.starts.borrow().is_empty());
        assert!(rows(&f.fx.rt, &f.ws).await.is_empty());
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "the thread keeps who to tell: a later send into it, and a turn after the host restarted, both tell the parent"
#[test]
fn the_thread_keeps_who_to_tell_a_later_send_into_it_and_a_turn_after_the_host_restarted_both_tell_the_parent() {
    drive(|| async {
        let h = Held::new(true);
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let parent = rt.rt.sessions().start(&ws, opts("orchestrate"), None).await.expect("a start");
        let parent_thread = thread(&parent);
        let kid = rt.rt.sessions().start(&ws, tell("build it", &[&parent_thread]), None).await.expect("a start");
        let kid_thread = thread(&kid);
        h.end(1, "first", json!({}));
        let hh = h.clone();
        wait!([hh] steered_len(&hh) == 1);
        let again = rt
            .rt
            .sessions()
            .start(&ws, StartOptions { prompt: "and the docs".into(), thread: Some(kid_thread.clone()), ..Default::default() }, None)
            .await
            .expect("a start");
        assert_eq!(thread(&again), kid_thread);
        h.end(2, "second", json!({}));
        let hh = h.clone();
        wait!([hh] steered_len(&hh) == 2);
        assert_eq!(h.steered.borrow()[1], format!("thread {} finished (completed): second", short(&kid_thread)));
        h.end(0, "parent done", json!({}));
        rt.rt.close().await.expect("a close");

        let h2 = Held::new(true);
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h2.factory()).build();
        let parent_again = rt2
            .rt
            .sessions()
            .start(&ws, StartOptions { prompt: "still here".into(), thread: Some(parent_thread.clone()), ..Default::default() }, None)
            .await
            .expect("a start");
        assert_eq!(thread(&parent_again), parent_thread);
        let third = rt2
            .rt
            .sessions()
            .start(&ws, StartOptions { prompt: "and the tests".into(), thread: Some(kid_thread.clone()), ..Default::default() }, None)
            .await
            .expect("a start");
        assert_eq!(thread(&third), kid_thread);
        h2.end(1, "third", json!({}));
        let hh = h2.clone();
        let want = vec![format!("thread {} finished (completed): third", short(&kid_thread))];
        wait!([hh, want] * hh.steered.borrow() == want);
        h2.end(0, "ok", json!({}));
        rt2.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a thread cannot notify itself: a send into it that names its own thread is refused, and nothing starts"
#[test]
fn a_thread_cannot_notify_itself_a_send_into_it_that_names_its_own_thread_is_refused_and_nothing_starts() {
    drive(|| async {
        let f = fixture(true).await;
        let own = started(&f, opts("orchestrate")).await;
        f.h.end(0, "ready", json!({}));
        own.finished().await;
        let own_thread = thread(&own);
        let refused =
            f.fx.rt
                .sessions()
                .start(
                    &f.ws,
                    StartOptions {
                        prompt: "again".into(),
                        thread: Some(own_thread.clone()),
                        notify: Some(vec![own_thread.clone()]),
                        ..Default::default()
                    },
                    None,
                )
                .await
                .err()
                .expect("a refusal");
        assert!(refused.message.contains("a thread cannot notify itself"), "{}", refused.message);
        assert_eq!(starts_len(&f.h), 1);
        assert_eq!(types_of(&history(&f.fx.rt, &f.ws).await), ["session.start", "session.done", "session.end"]);
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a cycle is refused: a start whose notify already leads back to this thread, at any length of the chain"
#[test]
fn a_cycle_is_refused_a_start_whose_notify_already_leads_back_to_this_thread_at_any_length_of_the_chain() {
    drive(|| async {
        let f = fixture(true).await;
        let a = started(&f, opts("a")).await;
        let b = started(&f, tell("b", &[&thread(&a)])).await;
        let c = started(&f, tell("c", &[&thread(&b)])).await;
        // Ended child first, so each end steers its line into a parent still running instead of opening a turn on it.
        f.h.end(2, "idle", json!({}));
        let h = f.h.clone();
        wait!([h] steered_len(&h) == 1);
        f.h.end(1, "idle", json!({}));
        let h = f.h.clone();
        wait!([h] steered_len(&h) == 2);
        f.h.end(0, "idle", json!({}));
        a.finished().await;
        for (target, who) in [(thread(&b), &b), (thread(&c), &c)] {
            let refused = f
                .fx
                .rt
                .sessions()
                .start(
                    &f.ws,
                    StartOptions { prompt: "a again".into(), thread: Some(thread(&a)), notify: Some(vec![target]), ..Default::default() },
                    None,
                )
                .await
                .err()
                .expect("a refusal");
            let words = format!("thread {} already notifies this thread; a cycle would run forever", short(&thread(who)));
            assert!(refused.message.contains(&words), "{}", refused.message);
        }
        assert_eq!(starts_len(&f.h), 3);
        // A chain that does not come back is fine: a fresh thread may name c, and c's own chain ends at a.
        let d = started(&f, tell("d", &[&thread(&c)])).await;
        assert_eq!(format!("{:?}", d.outcome()).to_lowercase(), "started");
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a parent whose workspace naps while the child ends gets the line when the workspace wakes, as the first turn of its own"
#[test]
fn a_parent_whose_workspace_naps_while_the_child_ends_gets_the_line_when_the_workspace_wakes_as_the_first_turn_of_its_own() {
    drive(|| async {
        let f = fixture(true).await;
        let events = f.fx.events();
        let parent = started(&f, opts("orchestrate")).await;
        f.h.end(0, "waiting", json!({}));
        parent.finished().await;
        let kid = started(&f, tell("build it", &[&thread(&parent)])).await;
        f.fx.rt.workspaces().nap(&f.ws, None).await.expect("a nap");
        let line = format!("thread {} finished (failed): machine paused while the agent was working", short(&thread(&kid)));
        assert_matches(&events.of("session.notify")[0], json!({ "notify": thread(&parent), "text": line }));
        assert_eq!(starts_len(&f.h), 2);
        f.fx.rt.workspaces().wake(&f.ws, None).await.expect("a wake");
        let h = f.h.clone();
        wait!([h] starts_len(&h) == 3);
        assert_matches(
            &json!({ "prompt": f.h.starts.borrow()[2].prompt, "resume": f.h.starts.borrow()[2].resume }),
            json!({ "prompt": line, "resume": sid(&parent) }),
        );
        assert_matches(events.of("session.start").last().expect("a start"), json!({ "threadId": thread(&parent), "prompt": line }));
        f.h.end(2, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a line held for a napping parent outlives a host restart and goes when the workspace wakes"
#[test]
fn a_line_held_for_a_napping_parent_outlives_a_host_restart_and_goes_when_the_workspace_wakes() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h1 = Held::new(true);
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h1.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let parent = rt1.rt.sessions().start(&ws, opts("orchestrate"), None).await.expect("a start");
        h1.end(0, "waiting", json!({}));
        parent.finished().await;
        let kid = rt1.rt.sessions().start(&ws, tell("build it", &[&thread(&parent)]), None).await.expect("a start");
        rt1.rt.workspaces().nap(&ws, None).await.expect("a nap");
        let line = format!("thread {} finished (failed): machine paused while the agent was working", short(&thread(&kid)));
        rt1.rt.close().await.expect("a close");

        let h2 = Held::new(true);
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h2.factory()).build();
        rows(&rt2.rt, &ws).await;
        settle().await;
        assert!(h2.starts.borrow().is_empty());
        rt2.rt.workspaces().wake(&ws, None).await.expect("a wake");
        let h = h2.clone();
        wait!([h] starts_len(&h) == 1);
        assert_matches(
            &json!({ "prompt": h2.starts.borrow()[0].prompt, "resume": h2.starts.borrow()[0].resume }),
            json!({ "prompt": line, "resume": sid(&parent) }),
        );
        h2.end(0, "read it", json!({}));
        rt2.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a line queued behind its parent's turn when the person pauses the parent's workspace goes to the parent at the wake, not to the person"
#[test]
fn a_line_queued_behind_its_parents_turn_when_the_person_pauses_the_parents_workspace_goes_to_the_parent_at_the_wake_not_to_the_person() {
    drive(|| async {
        let f = fixture(false).await;
        let events = f.fx.events();
        let parent = started(&f, opts("orchestrate")).await;
        let parent_thread = thread(&parent);
        let kid = started(&f, tell("build it", &[&parent_thread])).await;
        f.h.end(1, "done", json!({}));
        let line = format!("thread {} finished (completed): done", short(&thread(&kid)));
        let ev = events.clone();
        wait!([ev] ev.of("session.queued").len() == 1);
        assert_matches(&events.of("session.queued")[0], json!({ "threadId": parent_thread, "prompt": line }));
        f.fx.rt.workspaces().nap(&f.ws, None).await.expect("a nap");
        // The nap's interrupt reaches the parent's process, which exits; the held harness waits to be told.
        f.h.end(0, "", json!({ "status": "interrupted" }));
        settle().await;
        assert_eq!(starts_len(&f.h), 2);
        f.fx.rt.workspaces().wake(&f.ws, None).await.expect("a wake");
        let h = f.h.clone();
        wait!([h] starts_len(&h) == 3);
        assert_matches(
            &json!({ "prompt": f.h.starts.borrow()[2].prompt, "resume": f.h.starts.borrow()[2].resume }),
            json!({ "prompt": line, "resume": sid(&parent) }),
        );
        assert_eq!(events.of("session.notify").iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(), [json!(parent_thread)]);
        f.h.end(2, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a turn the runtime ends is told too: a nap while the child works reaches the parent as failed with the reason"
#[test]
fn a_turn_the_runtime_ends_is_told_too_a_nap_while_the_child_works_reaches_the_parent_as_failed_with_the_reason() {
    drive(|| async {
        let f = fixture(true).await;
        let events = f.fx.events();
        let kid = started(&f, tell("build it", &["me"])).await;
        f.fx.rt.workspaces().nap(&f.ws, None).await.expect("a nap");
        let turn = kid.turn_id();
        let kinds: Vec<String> = events
            .all()
            .iter()
            .filter(|e| e["turnId"] == json!(turn))
            .map(|e| e["type"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(kinds, ["session.start", "session.notify", "session.end"]);
        assert_matches(
            &events.of("session.notify")[0],
            json!({ "notify": "me", "text": format!("thread {} finished (failed): machine paused while the agent was working", short(&thread(&kid))) }),
        );
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a turn a host restart cut is told the same way: the parent, in another workspace whose rows load later, gets the line as a turn of its own since its own turn was cut too"
#[test]
fn a_turn_a_host_restart_cut_is_told_the_same_way_the_parent_in_another_workspace_whose_rows_load_later_gets_the_line_as_a_turn_of_its_own_since_its_own_turn_was_cut_too(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h1 = Held::new(true);
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h1.factory()).build();
        // The child's workspace has the older sessions document, so the sweep meets the child before its parent.
        let kid_ws = create_on(&rt1.rt, CreateOn::golden("builder"), None).await.expect("a workspace").view.id;
        let warmup = rt1.rt.sessions().start(&kid_ws, opts("warm up"), None).await.expect("a start");
        h1.end(0, "ready", json!({}));
        warmup.finished().await;
        let parent_ws = create_on(&rt1.rt, CreateOn::golden("lead"), None).await.expect("a workspace").view.id;
        let parent = rt1.rt.sessions().start(&parent_ws, opts("orchestrate"), None).await.expect("a start");
        let parent_thread = thread(&parent);
        let kid = rt1.rt.sessions().start(&kid_ws, by_agent(tell("build it", &[&parent_thread])), None).await.expect("a start");
        let kid_thread = thread(&kid);
        rt1.rt.close().await.expect("a close");

        let fc = FakeClock::new(now_ms() + 723_000);
        let h2 = Held::new(true);
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h2.factory()).clock(&fc).build();
        let line = format!("thread {} finished (failed): cut by a host restart after 12m 03s", short(&kid_thread));
        let kid_history = of_thread(history(&rt2.rt, &kid_ws).await, &kid_thread);
        assert_eq!(types_of(&kid_history), ["session.start", "session.notify", "session.end"]);
        assert_matches(
            &kid_history[1],
            json!({ "type": "session.notify", "turnId": kid.turn_id(), "sessionId": sid(&kid), "notify": parent_thread, "text": line }),
        );
        assert_matches(
            &kid_history[2],
            json!({ "type": "session.end", "turnId": kid.turn_id(), "reason": "host restarted while the agent was working" }),
        );
        let h = h2.clone();
        wait!([h] starts_len(&h) == 1);
        assert_matches(
            &json!({ "prompt": h2.starts.borrow()[0].prompt, "resume": h2.starts.borrow()[0].resume }),
            json!({ "prompt": line, "resume": sid(&parent) }),
        );
        assert!(h2.steered.borrow().is_empty());
        let parent_history = of_thread(history(&rt2.rt, &parent_ws).await, &parent_thread);
        assert_eq!(types_of(&parent_history), ["session.start", "session.end", "session.start"]);
        assert_matches(&parent_history[2], json!({ "type": "session.start", "prompt": line }));
        h2.end(0, "read it", json!({}));
        rt2.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a turn a host restart cut that was to tell me records the line in its thread, with the row's span"
#[test]
fn a_turn_a_host_restart_cut_that_was_to_tell_me_records_the_line_in_its_thread_with_the_rows_span() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h1 = Held::new(true);
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h1.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let kid = rt1.rt.sessions().start(&ws, tell("build it", &["me"]), None).await.expect("a start");
        rt1.rt.close().await.expect("a close");

        let fc = FakeClock::new(now_ms() + 45_000);
        let rt2 = runtime(&backend).store(&state.store).clock(&fc).build();
        let events = rt2.events();
        let all = history(&rt2.rt, &ws).await;
        assert_eq!(types_of(&all), ["session.start", "session.notify", "session.end"]);
        assert_matches(
            &all[1],
            json!({ "type": "session.notify", "turnId": kid.turn_id(), "notify": "me", "text": format!("thread {} finished (failed): cut by a host restart after 0m 45s", short(&thread(&kid))) }),
        );
        assert_eq!(events.of("session.notify").len(), 1);
        rt2.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "me is the thread the request came out of: two turns on one machine launch under a token each, and each token resolves me to its own thread"
#[test]
fn me_is_the_thread_the_request_came_out_of_two_turns_on_one_machine_launch_under_a_token_each_and_each_token_resolves_me_to_its_own_thread(
) {
    drive(|| async {
        let f = fixture(true).await;
        let one = started(&f, opts("coordinate one")).await;
        let two = started(&f, opts("coordinate two")).await;
        let tokens: Vec<String> = f.h.envs.borrow().iter().map(|e| e[TURN_TOKEN_ENV].clone()).collect();
        let hex32 = |t: &str| t.len() == 32 && t.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
        assert_eq!(tokens.iter().filter(|t| hex32(t)).count(), 2);
        assert_eq!(tokens.iter().collect::<std::collections::HashSet<_>>().len(), 2);
        // Neither turn's token is anything the machine's own login carries: it is this launch's alone.
        assert!(wsp_core::types::harness::guest_login_env().get(TURN_TOKEN_ENV).is_none());
        let mut a = by_agent(tell("build for one", &["me"]));
        a.turn_token = Some(tokens[0].clone());
        let kid_one = started(&f, a).await;
        let mut b = by_agent(tell("build for two", &["me"]));
        b.turn_token = Some(tokens[1].clone());
        let kid_two = started(&f, b).await;
        f.h.end(2, "one done", json!({}));
        f.h.end(3, "two done", json!({}));
        let (rt, ws) = (f.fx.rt.clone(), f.ws.clone());
        wait!([rt, ws] history(&rt, &ws).await.iter().filter(|e| e["type"] == "session.notify").count() == 2);
        let told: Vec<Value> = history(&f.fx.rt, &f.ws)
            .await
            .iter()
            .filter(|e| e["type"] == "session.notify")
            .map(|e| json!([e["threadId"], e["notify"]]))
            .collect();
        assert_eq!(told, [json!([thread(&kid_one), thread(&one)]), json!([thread(&kid_two), thread(&two)])]);
        f.h.end(0, "read it", json!({}));
        f.h.end(1, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a turn the runtime ended rather than its own process leaves its token naming nobody"
#[test]
fn a_turn_the_runtime_ended_rather_than_its_own_process_leaves_its_token_naming_nobody() {
    drive(|| async {
        let f = fixture(false).await;
        let turn = started(&f, opts("coordinate")).await;
        let token = f.h.envs.borrow()[0][TURN_TOKEN_ENV].clone();
        // A nap ends the turn from this side: its harness process never exits, so nothing on that road clears the row.
        f.fx.rt.workspaces().nap(&f.ws, None).await.expect("a nap");
        assert_eq!(j(&turn.view())["status"], "failed");
        f.fx.rt.workspaces().wake(&f.ws, None).await.expect("a wake");
        let mut o = tell("build it", &["me"]);
        o.turn_token = Some(token);
        let refused = f.fx.rt.sessions().start(&f.ws, o, None).await.err().expect("a refusal");
        assert!(refused.message.contains(NO_SUCH_TURN), "{}", refused.message);
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a token no turn on this host carries is refused, and nothing starts"
#[test]
fn a_token_no_turn_on_this_host_carries_is_refused_and_nothing_starts() {
    drive(|| async {
        let f = fixture(false).await;
        let mut o = tell("build it", &["me"]);
        o.turn_token = Some("f".repeat(32));
        let refused = f.fx.rt.sessions().start(&f.ws, o, None).await.err().expect("a refusal");
        assert!(refused.message.contains(NO_SUCH_TURN), "{}", refused.message);
        assert!(f.h.starts.borrow().is_empty());
        assert!(rows(&f.fx.rt, &f.ws).await.is_empty());
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "the token dies with the turn that carried it, and a start with no token still reaches the person"
#[test]
fn the_token_dies_with_the_turn_that_carried_it_and_a_start_with_no_token_still_reaches_the_person() {
    drive(|| async {
        let f = fixture(false).await;
        let turn = started(&f, opts("coordinate")).await;
        let token = f.h.envs.borrow()[0][TURN_TOKEN_ENV].clone();
        f.h.end(0, "handed off", json!({}));
        turn.finished().await;
        settle().await;
        let mut o = tell("build it", &["me"]);
        o.turn_token = Some(token);
        let refused = f.fx.rt.sessions().start(&f.ws, o, None).await.err().expect("a refusal");
        assert!(refused.message.contains(NO_SUCH_TURN), "{}", refused.message);
        // No token at all is every road that is not a turn: the app, a person's shell, an agent nobody launched here.
        let kid = started(&f, tell("build it", &["me"])).await;
        f.h.end(1, "done", json!({}));
        let (rt, ws) = (f.fx.rt.clone(), f.ws.clone());
        wait!([rt, ws] has_event(&rt, &ws, "session.notify").await);
        let all = history(&f.fx.rt, &f.ws).await;
        assert_matches(
            all.iter().find(|e| e["type"] == "session.notify").expect("a notify"),
            json!({ "threadId": thread(&kid), "notify": "me" }),
        );
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "notify takes several targets: a builder's end reaches its orchestrator and a reviewer, each once, with the same line"
#[test]
fn notify_takes_several_targets_a_builders_end_reaches_its_orchestrator_and_a_reviewer_each_once_with_the_same_line() {
    drive(|| async {
        let h = Held::new(true);
        let fx = runtime(&StubBackend::new(None)).adapter("claude", h.factory()).build();
        let lead = create_on(&fx.rt, CreateOn::golden("lead"), None).await.expect("a workspace").view.id;
        let box_ws = create_on(&fx.rt, CreateOn::golden("box"), None).await.expect("a workspace").view.id;
        let orchestrator = fx.rt.sessions().start(&lead, opts("orchestrate"), None).await.expect("a start");
        let reviewer = fx.rt.sessions().start(&lead, opts("review what lands"), None).await.expect("a start");
        let builder = fx
            .rt
            .sessions()
            .start(&box_ws, by_agent(tell("build it", &[&thread(&orchestrator), &thread(&reviewer)])), None)
            .await
            .expect("a start");
        h.end(2, "Ran the gate.\nAll 12 tests green.", json!({}));
        let line = notify_line(
            &thread(&builder),
            &result_of(json!({ "status": "completed", "text": "Ran the gate.\nAll 12 tests green." })),
            Some(NotifyLength::Whole),
        );
        let (hh, want) = (h.clone(), vec![line.clone(), line.clone()]);
        wait!([hh, want] * hh.steered.borrow() == want);
        let told: Vec<Value> = history(&fx.rt, &box_ws).await.into_iter().filter(|e| e["type"] == "session.notify").collect();
        assert_eq!(told.iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(), [json!(thread(&orchestrator)), json!(thread(&reviewer))]);
        assert_eq!(told.iter().map(|e| e["text"].clone()).collect::<Vec<_>>(), [json!(line), json!(line)]);
        h.end(0, "read it", json!({}));
        h.end(1, "read it", json!({}));
        fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a target whose thread is gone when the child ends falls back to the person, and the person is told once however many fell away"
#[test]
fn a_target_whose_thread_is_gone_when_the_child_ends_falls_back_to_the_person_and_the_person_is_told_once_however_many_fell_away() {
    drive(|| async {
        let h = Held::new(true);
        let fx = runtime(&StubBackend::new(None)).adapter("claude", h.factory()).build();
        let lead = create_on(&fx.rt, CreateOn::golden("lead"), None).await.expect("a workspace").view.id;
        let box_ws = create_on(&fx.rt, CreateOn::golden("box"), None).await.expect("a workspace").view.id;
        let orchestrator = fx.rt.sessions().start(&lead, opts("orchestrate"), None).await.expect("a start");
        let builder =
            fx.rt.sessions().start(&box_ws, by_agent(tell("build it", &[&thread(&orchestrator), "me"])), None).await.expect("a start");
        // The orchestrator's workspace is deleted while the builder works, so the thread the report was addressed to is
        // not there to take it.
        fx.rt.workspaces().delete(&lead, None).await.expect("a delete");
        h.end(1, "done", json!({}));
        let (rt, ws) = (fx.rt.clone(), box_ws.clone());
        wait!([rt, ws] has_event(&rt, &ws, "session.notify").await);
        let told: Vec<Value> = history(&fx.rt, &box_ws).await.into_iter().filter(|e| e["type"] == "session.notify").collect();
        assert_eq!(told.iter().map(|e| e["notify"].clone()).collect::<Vec<_>>(), [json!("me")]);
        assert_eq!(
            told[0]["text"],
            json!(notify_line(&thread(&builder), &result_of(json!({ "status": "completed", "text": "done" })), None))
        );
        assert!(h.steered.borrow().is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "a target list never holds the sender: its own thread by id or through me is refused, and a target named twice is one target"
#[test]
fn a_target_list_never_holds_the_sender_its_own_thread_by_id_or_through_me_is_refused_and_a_target_named_twice_is_one_target() {
    drive(|| async {
        let f = fixture(true).await;
        let one = started(&f, opts("one")).await;
        let one_thread = thread(&one);
        let token = f.h.envs.borrow()[0][TURN_TOKEN_ENV].clone();
        let refused =
            f.fx.rt
                .sessions()
                .start(
                    &f.ws,
                    StartOptions {
                        prompt: "again".into(),
                        thread: Some(one_thread.clone()),
                        notify: Some(vec![one_thread.clone()]),
                        ..Default::default()
                    },
                    None,
                )
                .await
                .err()
                .expect("a refusal");
        assert!(refused.message.contains("a thread cannot notify itself"), "{}", refused.message);
        let refused =
            f.fx.rt
                .sessions()
                .start(
                    &f.ws,
                    StartOptions {
                        prompt: "again".into(),
                        thread: Some(one_thread.clone()),
                        notify: Some(vec!["me".into()]),
                        turn_token: Some(token),
                        ..Default::default()
                    },
                    None,
                )
                .await
                .err()
                .expect("a refusal");
        assert!(refused.message.contains("a thread cannot notify itself"), "{}", refused.message);
        assert_eq!(starts_len(&f.h), 1);
        // Named twice is told once: a list is a set of targets, not a count of them.
        let two = started(&f, by_agent(tell("two", &[&one_thread, &one_thread]))).await;
        f.h.end(1, "done", json!({}));
        let line = notify_line(&thread(&two), &result_of(json!({ "status": "completed", "text": "done" })), Some(NotifyLength::Whole));
        let (h, want) = (f.h.clone(), vec![line]);
        wait!([h, want] * h.steered.borrow() == want);
        assert_eq!(history(&f.fx.rt, &f.ws).await.iter().filter(|e| e["type"] == "session.notify").count(), 1);
        f.h.end(0, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}

// "a thread whose start named who to tell" > "the line into a thread carries the final message whole; the person's stays its last line"
#[test]
fn the_line_into_a_thread_carries_the_final_message_whole_the_persons_stays_its_last_line() {
    drive(|| async {
        let f = fixture(true).await;
        let parent = started(&f, opts("orchestrate")).await;
        let kid = started(&f, by_agent(tell("build it", &[&thread(&parent)]))).await;
        let mine = started(&f, tell("build mine", &["me"])).await;
        let text = "Ran the gate.\n\nAll 12 tests green.\n";
        let report = result_of(json!({ "status": "completed", "text": text, "durationMs": 492_000, "costUsd": 1.94 }));
        f.h.end(1, text, json!({ "durationMs": 492_000, "costUsd": 1.94 }));
        f.h.end(2, text, json!({ "durationMs": 492_000, "costUsd": 1.94 }));
        let whole = notify_line(&thread(&kid), &report, Some(NotifyLength::Whole));
        assert!(whole.contains("Ran the gate.\n\nAll 12 tests green."));
        let (h, want) = (f.h.clone(), vec![whole.clone()]);
        wait!([h, want] * h.steered.borrow() == want);
        let told: Vec<Value> = history(&f.fx.rt, &f.ws).await.into_iter().filter(|e| e["type"] == "session.notify").collect();
        assert_eq!(told.iter().find(|e| e["notify"] == json!(thread(&parent))).expect("the parent's line")["text"], json!(whole));
        assert_eq!(
            told.iter().find(|e| e["notify"] == "me").expect("the person's line")["text"],
            json!(notify_line(&thread(&mine), &report, None))
        );
        f.h.end(0, "read it", json!({}));
        f.fx.rt.close().await.expect("a close");
    });
}
