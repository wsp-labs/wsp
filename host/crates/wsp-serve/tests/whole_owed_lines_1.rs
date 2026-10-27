// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/owed-lines.test.ts at 65f6b89c7, case for case. A child's finished line that cannot start into its lead stays
//! owed until a turn of the lead takes it, and reaches the lead once: through a steer whose answer was lost or came late, a launch that
//! failed after its start answered, and a host restart, on a clock the case moves. A line that cannot reach the lead within an hour of
//! wall time, one the lead's door or its computer refuses, and one whose try the person stopped go to the person instead, on either
//! side of a restart. The runtime and the real Claude and Codex adapters, fed by hand. Each test names the TS test above it. The TS
//! `memoryStore()` is the sqlite store here.
//! Part 1 of 2: the TS cases 1 to 19 in its order; the helpers of the file are in whole_owed_lines/helpers.rs.
#![allow(dead_code, unused_imports)]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::fed::{fed_runs, init, lifecycle, result, said, FedRuns, Loses, Run};
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::port::{
    AttachRunOptions, Attached, ExecFailure, ExecFailureKind, ExecOptions, ExecStream, ExecStreamFactory, HarnessAdapter,
};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::NOTIFY_OWED;
use wsp_core::types::wiring::SessionHandle;
use wsp_serve::AssembledRuntime;
use wsp_wire::views::session_events::NOTIFY_ME;
use wsp_words::exit::HostError;

include!("whole_owed_lines/helpers.rs");

// "a child's finished line its lead cannot take yet" > "a steer whose write landed but whose answer was lost reaches the lead once, and the person is told nothing"
#[test]
fn a_steer_whose_write_landed_but_whose_answer_was_lost_reaches_the_lead_once_and_the_person_is_told_nothing() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_run.loses.set(Some(Loses::Landed));
        s.kid_ends(None, None).await;
        assert_eq!(s.steers().len(), 1);
        let uuid = serde_json::from_str::<Value>(&s.steers()[0]).expect("a json line")["uuid"].as_str().expect("a uuid").to_string();
        s.lead_run.push(&[
            lifecycle(&uuid, "queued"),
            lifecycle(&uuid, "started"),
            said("msg_1", "read it"),
            result("read it", 1, None),
            lifecycle(&uuid, "completed"),
        ]);
        let rt = s.rt();
        until(|| async { has_type(&rt.rt, &s.lead_ws, "session.done").await }).await;
        s.lead_run.end(Some(0));
        tick().await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.steers().len(), 1);
        assert!(s.launches().is_empty());
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write never landed reaches the lead once, as its next turn, once the turn it was steered into ends"
#[test]
fn a_steer_whose_write_never_landed_reaches_the_lead_once_as_its_next_turn_once_the_turn_it_was_steered_into_ends() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_run.loses.set(Some(Loses::Lost));
        s.kid_ends(None, None).await;
        assert!(s.steers().is_empty());
        // The lead's turn ends without its agent ever taking the message.
        s.lead_exits().await;
        until(|| async { s.launches().len() == 1 }).await;
        s.taken(0).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.steers().is_empty());
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch that fails after its start answered keeps the line owed, and the next try reaches the lead once"
#[test]
fn a_launch_that_fails_after_its_start_answered_keeps_the_line_owed_and_the_next_try_reaches_the_lead_once() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        // The box the lead runs on never answers the launch's posts: the stream fails with no word from the agent.
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        assert_eq!(s.owed().await, 1);
        s.next_try(&at, 2).await;
        s.taken(1).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 2);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch that fails after the lead's agent announced itself is not sent again: the agent had the line"
#[test]
fn a_launch_that_fails_after_the_leads_agent_announced_itself_is_not_sent_again_the_agent_had_the_line() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].push(&[init()]);
        let rt = s.rt();
        until(|| async { history(&rt.rt, &s.lead_ws).await.iter().filter(|e| e["type"] == "session.start").count() == 2 }).await;
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write never landed is still sent back once when the host restarts before the turn it was steered into ends"
#[test]
fn a_steer_whose_write_never_landed_is_still_sent_back_once_when_the_host_restarts_before_the_turn_it_was_steered_into_ends() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_run.loses.set(Some(Loses::Lost));
        s.kid_ends(None, None).await;
        s.restart(&FakeClock::new(a.now())).await;
        // The second host reads the lead's run again; its turn ends without its agent ever taking the message.
        s.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        tick().await;
        s.lead_run.end(Some(0));
        until(|| async { s.launches().len() == 1 }).await;
        s.taken(0).await;
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.steers().is_empty());
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch whose agent had not announced itself when the host restarted is read on the next host, and the line is not sent again"
#[test]
fn a_launch_whose_agent_had_not_announced_itself_when_the_host_restarted_is_read_on_the_next_host_and_the_line_is_not_sent_again() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        s.restart(&FakeClock::new(a.now())).await;
        tick().await;
        assert_eq!(s.launches().len(), 1);
        s.launches()[0].push(&[init(), said("msg_t0", "read it"), result("read it", 1, None)]);
        let rt = s.rt();
        until(|| async { done_count(&rt.rt, &s.lead_ws).await >= 2 }).await;
        s.launches()[0].end(Some(0));
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        until(|| async { s.owed().await == 0 }).await;
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line still owed when the host restarts reaches the lead from the next host, once"
#[test]
fn a_line_still_owed_when_the_host_restarts_reaches_the_lead_from_the_next_host_once() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        // The host stops before the line's next try.
        s.restart(&FakeClock::new(a.now() + 5 * MIN)).await;
        until(|| async { s.launches().len() == 2 }).await;
        s.taken(1).await;
        tick().await;
        assert_eq!(s.launches().len(), 2);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line its lead's computer keeps failing tells the person once, an hour of wall time after it became owed and not an hour of the next host, and is never sent after"
#[test]
fn a_line_its_leads_computer_keeps_failing_tells_the_person_once_an_hour_of_wall_time_after_it_became_owed_and_not_an_hour_of_the_next_host_and_is_never_sent_after(
) {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        // Each launch takes long minutes to fail, so the hour holds four tries, two on each host.
        a.advance(20 * MIN);
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        s.next_try(&a, 2).await;
        a.advance(10 * MIN);
        s.launches()[1].fail_with(launch_lost());
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        s.next_try(&b, 3).await;
        b.advance(25 * MIN);
        s.launches()[2].fail_with(launch_lost());
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        s.next_try(&b, 4).await;
        // The fourth try is still out at the hour the line has been owed, half an hour into the second host's own: it is stopped there and
        // the person is told.
        b.advance(10 * MIN);
        s.launches()[3].fail_with(launch_lost());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        let told = s.to_person(None).await;
        let head = format!("thread {} finished (completed", &s.kid_thread()[..8]);
        assert!(told[0].starts_with(&head) && told[0].ends_with(&format!("): {REPORT}")), "{}", told[0]);
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 4);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line whose start the lead's agent refuses goes to the person at once and is never tried again"
#[test]
fn a_line_whose_start_the_leads_agent_refuses_goes_to_the_person_at_once_and_is_never_tried_again() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        // The agent refuses to start on the lead once the child works, so the line's own start is refused outright.
        s.kid_ends(None, Some(&|| s.refuse_starts())).await;
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        assert_eq!(s.refused_starts(), 1);
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.refused_starts(), 1);
        assert!(s.launches().is_empty());
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write outlasts the lead's reply and the window its held reply waits reaches the lead once, as its next turn"
#[test]
fn a_steer_whose_write_outlasts_the_leads_reply_and_the_window_its_held_reply_waits_reaches_the_lead_once_as_its_next_turn() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts { result_exit_ms: Some(200), ..Default::default() }).await;
        let land = Gate::new();
        *s.lead_run.gate.borrow_mut() = Some(land.clone());
        s.kid_ends(None, None).await;
        // The lead's agent gives its last word while the write is on its way, and its held reply stands before the write answers: the turn
        // ends with the message unread.
        s.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        sleep_ms(600).await;
        *s.lead_run.gate.borrow_mut() = None;
        land.open();
        s.lead.finished().await;
        until(|| async { !s.launches().is_empty() }).await;
        s.taken(0).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a steer whose write outlasts the lead's process reaches the lead once, as its next turn"
#[test]
fn a_steer_whose_write_outlasts_the_leads_process_reaches_the_lead_once_as_its_next_turn() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        let land = Gate::new();
        *s.lead_run.gate.borrow_mut() = Some(land.clone());
        s.kid_ends(None, None).await;
        // The lead replies and its process exits while the write is on its way.
        s.lead_exits().await;
        tick().await;
        *s.lead_run.gate.borrow_mut() = None;
        land.open();
        until(|| async { !s.launches().is_empty() }).await;
        s.taken(0).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a try whose launch the lead's computer never answers is stopped at the line's hour, and the person is told then"
#[test]
fn a_try_whose_launch_the_leads_computer_never_answers_is_stopped_at_the_lines_hour_and_the_person_is_told_then() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        // The launch's posts went out and nothing answers either way: the stream neither runs nor fails.
        at.advance(59 * MIN);
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        at.advance(2 * MIN);
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a launch the lead's computer refuses goes to the person at once and is never tried again"
#[test]
fn a_launch_the_leads_computer_refuses_goes_to_the_person_at_once_and_is_never_tried_again() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_refused());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "the person's stop of the lead's turn carrying the line holds: the line is not launched again, and the person is told it"
#[test]
fn the_persons_stop_of_the_leads_turn_carrying_the_line_holds_the_line_is_not_launched_again_and_the_person_is_told_it() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        let rows: Vec<Value> = s.rt().rt.sessions().list(Some(s.lead_ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row = rows.iter().find(|v| v["threadId"] == s.lead_thread.as_str() && v["status"] == "running").expect("the running row");
        s.rt().rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("a stop");
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a lead started with --notify me hears nothing of the line's failed tries, and the person gets the one row of the child's line at the hour"
#[test]
fn a_lead_started_with_notify_me_hears_nothing_of_the_lines_failed_tries_and_the_person_gets_the_one_row_of_the_childs_line_at_the_hour() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts { lead_notify: Some(vec![NOTIFY_ME.into()]), ..Default::default() }).await;
        s.lead_waits().await;
        let lead_thread = s.lead_thread.clone();
        let before = s.to_person(Some(&lead_thread)).await;
        assert_eq!(before.len(), 1);
        s.kid_ends(None, None).await;
        s.keep_failing(&at, 70, launch_lost).await;
        assert!(s.launches().len() > 5);
        assert_eq!(s.to_person(Some(&lead_thread)).await, before);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a restart keeps the spacing of the line's tries: the next host does not start it over at the first wait"
#[test]
fn a_restart_keeps_the_spacing_of_the_lines_tries_the_next_host_does_not_start_it_over_at_the_first_wait() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_lost());
        s.next_try(&a, 2).await;
        s.launches()[1].fail_with(launch_lost());
        s.next_try(&a, 3).await;
        s.launches()[2].fail_with(launch_lost());
        tick().await;
        // Three tries failed: the next waits two minutes, and a restart in the middle of that wait keeps it.
        a.advance(30_000);
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        assert_eq!(s.launches().len(), 3);
        b.advance(60_000);
        tick().await;
        assert_eq!(s.launches().len(), 3);
        s.next_try(&b, 4).await;
        s.launches()[3].fail_with(launch_lost());
        tick().await;
        // The fourth failure waits four minutes, on the second host as on the first.
        b.advance(200_000);
        tick().await;
        assert_eq!(s.launches().len(), 4);
        s.next_try(&b, 5).await;
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a child's lines into two leads that fall on two hosts tell the person once"
#[test]
fn a_childs_lines_into_two_leads_that_fall_on_two_hosts_tell_the_person_once() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        let rt = s.rt();
        let other_ws = create_on(&rt.rt, lead_of("lead-2"), None).await.expect("a workspace").view.id;
        let other = rt
            .rt
            .sessions()
            .start(&other_ws, StartOptions { prompt: "coordinate".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { s.fed.run_of("coordinate").is_some() }).await;
        let other_run = s.fed.run_of("coordinate").expect("the run");
        other_run.push(&[init(), said("msg_o", "waiting"), result("waiting", 1, None)]);
        until(|| async { has_type(&rt.rt, &other_ws, "session.done").await }).await;
        other_run.end(Some(0));
        other.finished().await;
        s.kid_ends(Some(vec![s.lead_thread.clone(), other.view().thread_id.expect("a thread")]), None).await;
        until(|| async { s.launches().len() == 2 }).await;
        let into = |ws: &str| s.launches().into_iter().find(|r| r.env.get("LAB_WORKSPACE").is_some_and(|w| w == ws)).expect("the launch");
        // The first lead's computer refuses its launch: the person is told now. The second's never answers.
        into(&s.lead_ws).fail_with(launch_refused());
        into(&other_ws).fail_with(launch_lost());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        tick().await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.keep_failing(&b, 70, launch_lost).await;
        assert!(s.launches().len() > 3);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line whose steer never answered when the host restarted reaches the person when the lead's turn ends, never nobody"
#[test]
fn a_codex_leads_line_whose_steer_never_answered_when_the_host_restarted_reaches_the_person_when_the_leads_turn_ends_never_nobody() {
    drive(|| async {
        let (factory, fed) = fed_runs();
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let a = FakeClock::at_now();
        let mut rt = codex_host(&backend, &state, &factory, &a);
        let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
        let lead = rt
            .rt
            .sessions()
            .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { fed.runs.borrow().len() == 1 }).await;
        let lead_run = fed.runs.borrow()[0].clone();
        lead_run.push(&opened());
        until(|| async { writes_of(&lead_run, "turn/start") }).await;
        lead_run.push(&[turn_started()]);
        until(|| async { has_type(&rt.rt, &lead_ws, "session.start").await }).await;
        let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
        // The child's line steers into the lead's turn, and that write never lands and never answers.
        lead_run.loses.set(Some(Loses::Lost));
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(vec![lead.view().thread_id.expect("a thread")]),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
        let kid_thread = kid.view().thread_id.expect("a thread");
        until(|| async { fed.run_of("parse the files").is_some() }).await;
        let kid_run = fed.run_of("parse the files").expect("the run");
        kid_run.push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        until(|| async {
            history(&rt.rt, &kid_ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == kid_thread.as_str())
        })
        .await;
        kid_run.end(Some(0));
        kid.finished().await;
        tick().await;
        assert_eq!(rt.store.list(NOTIFY_OWED).await.expect("a list").len(), 1);
        let runs_before = fed.runs.borrow().len();
        rt.rt.close().await.expect("a close");
        rt = codex_host(&backend, &state, &factory, &FakeClock::new(a.now()));
        rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
        tick().await;
        // The lead's turn goes on and ends on the second host; its agent never had the line.
        lead_run.push(&[agent_message(), completed()]);
        tick().await;
        lead_run.end(Some(0));
        until(|| async { person_rows(&rt.rt, &kid_ws, &lead_ws, &kid_thread).await == 1 }).await;
        let later: Vec<Rc<Run>> = fed.runs.borrow()[runs_before..].to_vec();
        for r in &later {
            r.push(&opened());
        }
        tick().await;
        assert!(fed.runs.borrow()[runs_before..].is_empty());
        assert_eq!(fed.runs.borrow().iter().map(|r| r.writes().iter().filter(|w| w.contains(REPORT)).count()).sum::<usize>(), 0);
        assert_eq!(person_rows(&rt.rt, &kid_ws, &lead_ws, &kid_thread).await, 1);
        assert!(rt.store.list(NOTIFY_OWED).await.expect("a list").is_empty());
        rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart, the person's stop of the re-opened turn carrying the line holds: it is not launched again, and the person is told it"
#[test]
fn after_a_restart_the_persons_stop_of_the_re_opened_turn_carrying_the_line_holds_it_is_not_launched_again_and_the_person_is_told_it() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        assert_eq!(s.launches().len(), 1);
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        let rows: Vec<Value> = s.rt().rt.sessions().list(Some(s.lead_ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row = rows.iter().find(|v| v["threadId"] == s.lead_thread.as_str() && v["status"] == "running").expect("the running row");
        s.rt().rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("a stop");
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}
