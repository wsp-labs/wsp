// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/owed-lines.test.ts at 65f6b89c7, case for case. A child's finished line that cannot start into its lead stays
//! owed until a turn of the lead takes it, and reaches the lead once: through a steer whose answer was lost or came late, a launch that
//! failed after its start answered, and a host restart, on a clock the case moves. A line that cannot reach the lead within an hour of
//! wall time, one the lead's door or its computer refuses, and one whose try the person stopped go to the person instead, on either
//! side of a restart. The runtime and the real Claude and Codex adapters, fed by hand. Each test names the TS test above it. The TS
//! `memoryStore()` is the sqlite store here.
//! Part 2 of 2: the TS cases 20 to 38 in its order; the helpers of the file are in whole_owed_lines/helpers.rs.
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

// "a child's finished line its lead cannot take yet" > "after a restart, a re-opened turn carrying the line that the lead's computer refuses goes to the person at once and is never launched again"
#[test]
fn after_a_restart_a_re_opened_turn_carrying_the_line_that_the_leads_computer_refuses_goes_to_the_person_at_once_and_is_never_launched_again(
) {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.launches()[0].fail_with(launch_refused());
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart, a re-opened turn carrying the line that the lead's computer never answers is stopped at the line's hour, and the person is told then"
#[test]
fn after_a_restart_a_re_opened_turn_carrying_the_line_that_the_leads_computer_never_answers_is_stopped_at_the_lines_hour_and_the_person_is_told_then(
) {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        b.advance(59 * MIN);
        tick().await;
        assert!(s.to_person(None).await.is_empty());
        b.advance(2 * MIN);
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        b.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart, a re-opened turn carrying the line that the lead's computer did not answer goes again on the clock"
#[test]
fn after_a_restart_a_re_opened_turn_carrying_the_line_that_the_leads_computer_did_not_answer_goes_again_on_the_clock() {
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.launches()[0].fail_with(launch_lost());
        s.next_try(&b, 2).await;
        s.taken(1).await;
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a Claude Code lead whose CLI tells no unread messages: a steer whose write never answered tells the person, as main did, and is never lost"
#[test]
fn a_claude_code_lead_whose_cli_tells_no_unread_messages_a_steer_whose_write_never_answered_tells_the_person_as_main_did_and_is_never_lost()
{
    drive(|| async {
        let a = FakeClock::at_now();
        let s = setup(&a, Opts { lead_init: Some(old_init()), ..Default::default() }).await;
        s.lead_run.loses.set(Some(Loses::Lost));
        s.kid_ends(None, None).await;
        until(|| async { s.to_person(None).await.len() == 1 }).await;
        let b = FakeClock::new(a.now());
        s.restart(&b).await;
        tick().await;
        s.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        tick().await;
        s.lead_run.end(Some(0));
        for _ in 0..30 {
            b.advance(5 * MIN);
            tick().await;
        }
        assert!(s.launches().is_empty());
        assert!(s.steers().is_empty());
        assert_eq!(s.to_person(None).await.len(), 1);
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a line whose try failed and whose lead then naps past its hour gets a try at the wake, as a line that never failed does"
#[test]
fn a_line_whose_try_failed_and_whose_lead_then_naps_past_its_hour_gets_a_try_at_the_wake_as_a_line_that_never_failed_does() {
    drive(|| async {
        let at = FakeClock::at_now();
        let s = setup(&at, Opts::default()).await;
        s.lead_waits().await;
        s.kid_ends(None, None).await;
        s.launches()[0].fail_with(launch_lost());
        tick().await;
        s.rt().rt.workspaces().nap(&s.lead_ws, None).await.expect("a nap");
        at.advance(2 * HOUR);
        tick().await;
        assert_eq!(s.launches().len(), 1);
        s.rt().rt.workspaces().wake(&s.lead_ws, None).await.expect("a wake");
        until(|| async { s.launches().len() == 2 }).await;
        s.taken(1).await;
        assert!(s.to_person(None).await.is_empty());
        assert_eq!(s.owed().await, 0);
        s.close().await;
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's server that opens the thread for the line's try and exits before the turn starts leaves the line to the person, never nobody"
#[test]
fn a_codex_leads_server_that_opens_the_thread_for_the_lines_try_and_exits_before_the_turn_starts_leaves_the_line_to_the_person_never_nobody(
) {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.push(&opened());
        until(|| async { writes_of(&c.try_run, "turn/start") }).await;
        tick().await;
        c.try_run.end(Some(1));
        until(|| async { c.to_person().await == 1 }).await;
        c.steps(30, 5 * MIN).await;
        assert_eq!(c.runs(), 3);
        assert_eq!(c.to_person().await, 1);
        assert!(c.owed().await.is_empty());
        c.rt.rt.close().await.expect("a close");
        let _ = &c.state;
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's server that opens the thread for the line's try and refuses its turn leaves the line to the person, never nobody"
#[test]
fn a_codex_leads_server_that_opens_the_thread_for_the_lines_try_and_refuses_its_turn_leaves_the_line_to_the_person_never_nobody() {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.push(&opened());
        until(|| async { writes_of(&c.try_run, "turn/start") }).await;
        c.try_run.push(&[r#"{"id":"wsp-turn","error":{"code":-32603,"message":"the model is not available on this plan"}}"#.into()]);
        tick().await;
        c.try_run.end(Some(0));
        until(|| async { c.to_person().await == 1 }).await;
        c.steps(30, 5 * MIN).await;
        assert_eq!(c.runs(), 3);
        assert_eq!(c.to_person().await, 1);
        assert!(c.owed().await.is_empty());
        c.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's launch of the line that its computer never answered is tried again, and the line is taken once the next try's turn starts"
#[test]
fn a_codex_leads_launch_of_the_line_that_its_computer_never_answered_is_tried_again_and_the_line_is_taken_once_the_next_trys_turn_starts() {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.fail_with(launch_lost());
        tick().await;
        c.steps(12, 10_000).await;
        assert_eq!(c.runs(), 4);
        let next = c.fed.runs.borrow()[3].clone();
        next.push(&opened());
        until(|| async { next.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        assert_eq!(c.owed().await.len(), 1);
        next.push(&[turn_started()]);
        until(|| async { c.owed().await.is_empty() }).await;
        assert_eq!(c.to_person().await, 0);
        c.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart whose re-open of the line's try the lead's computer never answers, the person is told at the line's hour and the line is settled"
#[test]
fn after_a_restart_whose_re_open_of_the_lines_try_the_leads_computer_never_answers_the_person_is_told_at_the_lines_hour_and_the_line_is_settled(
) {
    drive(|| async {
        let d = dark_restart().await;
        let mut told_at: Option<i64> = None;
        let mut minute = 5;
        while minute <= 120 {
            d.b.advance(5 * MIN);
            tick().await;
            if told_at.is_none() && d.to_person().await > 0 {
                told_at = Some(minute);
            }
            minute += 5;
        }
        let told = told_at.expect("the person was told");
        assert!(told <= 65, "told at minute {told}");
        assert_eq!(d.to_person().await, 1);
        assert_eq!(d.h.fed.runs_of(REPORT).len(), 1);
        assert_eq!(d.h.owed().await, 0);
        // The try is out of reach: the host writes no end for it, and the lead's one end is its first turn's.
        assert_eq!(d.ends_of_lead().await, 1);
        d.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "after a restart whose re-open of the line's try the lead's computer never answers, the person's stop of that row settles the line, tells them, and writes no end for the turn"
#[test]
fn after_a_restart_whose_re_open_of_the_lines_try_the_leads_computer_never_answers_the_persons_stop_of_that_row_settles_the_line_tells_them_and_writes_no_end_for_the_turn(
) {
    drive(|| async {
        let d = dark_restart().await;
        let rows: Vec<Value> = d.rt.rt.sessions().list(Some(d.lead_ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row = rows.iter().find(|v| v["status"] == "running").expect("the running row");
        let stopped = d.rt.rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("a stop");
        assert_eq!(j(&stopped), json!({ "outcome": "not-running" }));
        until(|| async { d.to_person().await == 1 }).await;
        for _ in 0..20 {
            d.b.advance(MIN);
            tick().await;
        }
        assert_eq!(d.to_person().await, 1);
        assert_eq!(d.h.fed.runs_of(REPORT).len(), 1);
        assert_eq!(d.h.owed().await, 0);
        assert_eq!(d.ends_of_lead().await, 1);
        d.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a stop the person sends while the next host's re-open of a turn is still out writes no end of its own: the turn ends once, as its agent ends it"
#[test]
fn a_stop_the_person_sends_while_the_next_hosts_re_open_of_a_turn_is_still_out_writes_no_end_of_its_own_the_turn_ends_once_as_its_agent_ends_it(
) {
    drive(|| async {
        let h = two_hosts();
        let a = FakeClock::at_now();
        let mut rt = h.host(&a);
        let ws = create_on(&rt.rt, CreateOn { idle_window_ms: wsp_core::types::wiring::IdleWindow::Off, ..CreateOn::golden("app") }, None)
            .await
            .expect("a workspace")
            .view
            .id;
        let turn = rt
            .rt
            .sessions()
            .start(&ws, StartOptions { prompt: "build the parser".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { h.fed.run_of("build the parser").is_some() }).await;
        let run = h.fed.run_of("build the parser").expect("the run");
        run.push(&[init(), said("msg_1", "Reading the grammar first.")]);
        until(|| async { history(&rt.rt, &ws).await.iter().any(|e| e["type"] == "session.delta") }).await;
        let rows: Vec<Value> = rt.rt.sessions().list(Some(ws.clone()), None).await.expect("a list").iter().map(j).collect();
        let row_id = rows.iter().find(|v| v["status"] == "running").expect("the running row")["id"].as_str().expect("an id").to_string();
        rt.rt.close().await.expect("a close");
        h.hold();
        rt = h.host(&FakeClock::new(a.now()));
        let stopping = {
            let (rt, id) = (rt.rt.clone(), row_id.clone());
            tokio::task::spawn_local(async move { rt.sessions().interrupt(&id, None, None).await })
        };
        tick().await;
        h.release();
        stopping.await.expect("the stop runs").expect("a stop");
        tick().await;
        run.push(&[said("msg_2", "Parser done: three files changed."), result("Parser done: three files changed.", 1, None)]);
        tick().await;
        run.end(Some(0));
        until(|| async { has_type(&rt.rt, &ws, "session.end").await }).await;
        for _ in 0..5 {
            tick().await;
        }
        let rows = rows_of(&history(&rt.rt, &ws).await, &turn.turn_id());
        assert_eq!(rows.iter().filter(|r| *r == "session.end").count(), 1);
        assert_eq!(rows.last().map(String::as_str), Some("session.end"));
        rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a next host that comes up past the line's hour reads the try only once its re-open answered: the lead took the line, one turn, nobody told it failed"
#[test]
fn a_next_host_that_comes_up_past_the_lines_hour_reads_the_try_only_once_its_re_open_answered_the_lead_took_the_line_one_turn_nobody_told_it_failed(
) {
    drive(|| async { took_the_line(up_later(61).await) });
}

// "a child's finished line its lead cannot take yet" > "a next host that comes up within the line's hour reads the try only once its re-open answered: the lead took the line, one turn, nobody told it failed"
#[test]
fn a_next_host_that_comes_up_within_the_lines_hour_reads_the_try_only_once_its_re_open_answered_the_lead_took_the_line_one_turn_nobody_told_it_failed(
) {
    drive(|| async { took_the_line(up_later(30).await) });
}

// "a child's finished line its lead cannot take yet" > "a Codex turn whose server prints app-server-turn's warning after the thread answered writes its start row first, and the note after it"
#[test]
fn a_codex_turn_whose_server_prints_app_server_turns_warning_after_the_thread_answered_writes_its_start_row_first_and_the_note_after_it() {
    drive(|| async { warning_case(warnings().remove(0)).await });
}

// "a child's finished line its lead cannot take yet" > "a Codex turn whose server prints app-server-subagent's warning after the thread answered writes its start row first, and the note after it"
#[test]
fn a_codex_turn_whose_server_prints_app_server_subagents_warning_after_the_thread_answered_writes_its_start_row_first_and_the_note_after_it(
) {
    drive(|| async { warning_case(warnings().remove(1)).await });
}

// "a child's finished line its lead cannot take yet" > "a Codex turn whose server prints the startup configWarning before the thread answered writes its start row first, and the note after it"
#[test]
fn a_codex_turn_whose_server_prints_the_startup_config_warning_before_the_thread_answered_writes_its_start_row_first_and_the_note_after_it()
{
    drive(|| async { warning_case(warnings().remove(2)).await });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line try whose server warned after the thread answered, and a host restart before turn/started: the line is taken once, by the turn that starts"
#[test]
fn a_codex_leads_line_try_whose_server_warned_after_the_thread_answered_and_a_host_restart_before_turn_started_the_line_is_taken_once_by_the_turn_that_starts(
) {
    drive(|| async {
        let h = two_hosts();
        let a = FakeClock::at_now();
        let mut rt = h.host(&a);
        let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
        let lead = rt
            .rt
            .sessions()
            .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { h.fed.runs.borrow().len() == 1 }).await;
        let lead_run = h.fed.runs.borrow()[0].clone();
        lead_run.push(&opened());
        until(|| async { writes_of(&lead_run, "turn/start") }).await;
        lead_run.push(&[turn_started(), agent_message(), completed()]);
        until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
        lead_run.end(Some(0));
        lead.finished().await;
        let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(vec![lead.view().thread_id.expect("a thread")]),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
        let kid_thread = kid.view().thread_id.expect("a thread");
        until(|| async { h.fed.run_of("parse the files").is_some() }).await;
        h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        until(|| async {
            history(&rt.rt, &kid_ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == kid_thread.as_str())
        })
        .await;
        h.fed.run_of("parse the files").expect("the run").end(Some(0));
        kid.finished().await;
        tick().await;
        until(|| async { h.fed.runs.borrow().len() == 3 }).await;
        let try_run = h.fed.runs.borrow()[2].clone();
        let mut first = opened();
        first.push(warnings().remove(0).line);
        try_run.push(&first);
        until(|| async { try_run.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        tick().await;
        assert_eq!(h.owed().await, 1);
        rt.rt.close().await.expect("a close");
        let b = FakeClock::new(a.now());
        rt = h.host(&b);
        rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
        tick().await;
        try_run.push(&[turn_started(), agent_message(), completed()]);
        until(|| async { h.owed().await == 0 }).await;
        try_run.end(Some(0));
        for _ in 0..20 {
            b.advance(5 * MIN);
            tick().await;
        }
        let lead_history = history(&rt.rt, &lead_ws).await;
        assert_eq!(lead_history.iter().filter(|e| e["type"] == "session.start").count(), 2);
        let mut all = history(&rt.rt, &kid_ws).await;
        all.extend(lead_history);
        assert!(all.iter().all(|e| !(e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == kid_thread.as_str())));
        assert_eq!(h.fed.runs.borrow().len(), 3);
        rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line try whose server opened the thread and whose launch then lost its computer is tried again under the same request, as a fresh turn"
#[test]
fn a_codex_leads_line_try_whose_server_opened_the_thread_and_whose_launch_then_lost_its_computer_is_tried_again_under_the_same_request_as_a_fresh_turn(
) {
    drive(|| async {
        let c = codex_lead().await;
        c.try_run.push(&opened());
        until(|| async { writes_of(&c.try_run, "turn/start") }).await;
        c.try_run.fail_with(launch_lost());
        tick().await;
        c.steps(12, 10_000).await;
        assert_eq!(c.runs(), 4);
        let next = c.fed.runs.borrow()[3].clone();
        next.push(&opened());
        until(|| async { next.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        next.push(&[turn_started()]);
        until(|| async { c.owed().await.is_empty() }).await;
        assert_eq!(c.to_person().await, 0);
        c.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished line its lead cannot take yet" > "a Codex lead's line try that ended before turn/started still leaves the line untaken on the next host, whose retry launches it fresh"
#[test]
fn a_codex_leads_line_try_that_ended_before_turn_started_still_leaves_the_line_untaken_on_the_next_host_whose_retry_launches_it_fresh() {
    drive(|| async {
        let h = two_hosts();
        let a = FakeClock::at_now();
        let mut rt = h.host(&a);
        let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
        let lead = rt
            .rt
            .sessions()
            .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        until(|| async { h.fed.runs.borrow().len() == 1 }).await;
        let first = h.fed.runs.borrow()[0].clone();
        first.push(&opened());
        until(|| async { writes_of(&first, "turn/start") }).await;
        first.push(&[turn_started(), agent_message(), completed()]);
        until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
        first.end(Some(0));
        lead.finished().await;
        let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(vec![lead.view().thread_id.expect("a thread")]),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
        until(|| async { h.fed.run_of("parse the files").is_some() }).await;
        h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        until(|| async { has_type(&rt.rt, &kid_ws, "session.done").await }).await;
        h.fed.run_of("parse the files").expect("the run").end(Some(0));
        kid.finished().await;
        until(|| async { h.fed.runs.borrow().len() == 3 }).await;
        let tried = h.fed.runs.borrow()[2].clone();
        tried.push(&opened());
        until(|| async { writes_of(&tried, "turn/start") }).await;
        tried.fail_with(launch_lost());
        tick().await;
        rt.rt.close().await.expect("a close");
        let b = FakeClock::new(a.now());
        rt = h.host(&b);
        rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
        for _ in 0..12 {
            if h.fed.runs.borrow().len() >= 4 {
                break;
            }
            b.advance(10_000);
            tick().await;
        }
        assert_eq!(h.fed.runs.borrow().len(), 4);
        let next = h.fed.runs.borrow()[3].clone();
        next.push(&opened());
        until(|| async { next.writes().iter().any(|w| w.contains("turn/start") && w.contains("Parsed 40 files")) }).await;
        next.push(&[turn_started()]);
        until(|| async { h.owed().await == 0 }).await;
        rt.rt.close().await.expect("a close");
    });
}
