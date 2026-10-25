// SPDX-License-Identifier: AGPL-3.0-only
//! One test per scene of "machines: phases" in oracle/machines.oracle.ts (the scene "drop" is replayed in
//! machines_attach.rs): the same world, the same act, and the log, the answer and the record the TS left
//! (tests/fixtures/oracle/machines_phases.json). The fixture carries the inputs and the expected result, not the act;
//! each act is encoded here from the oracle. Spec: spec/machines_phases.md.
//!
//! Two things the oracle logs have no Rust record, so they are read another way (support `assert_notes`, `Rules`): the
//! engine workspace's `notePaused`/`noteRunning` (the phase they leave is read off the workspace) and the promise
//! identity flags (`joined`, the third member of `during`): a join is read off the single run its log shows.

mod common;
#[path = "support/machines.rs"]
mod support;

use std::sync::Arc;

use serde_json::json;
use support::{EntryOpts, Opts, Prove, World};
use tokio::sync::Notify;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::MachineState;
use wsp_machines::machines::machines::Refusal;
use wsp_wire::views::workspace::WorkspacePhase;

#[test]
fn nap_pausing_is_written_and_pushed_before_the_provider_is_asked_then_napping() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "nap: pausing is written and pushed before the provider is asked, then napping");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts::default());
        w.sessions_of();
        w.rt.nap_refusals.borrow_mut().insert("ws_1".into(), Refusal { machine_id: "m-1".into(), said: "old".into() });
        e.entry.borrow_mut().unchecked = true;
        w.clock.advance(5);
        let area = w.area.clone();
        let a = support::started(async move { area.nap_with("ws_1", Some("idle 20 min")).await }).await;
        let area = w.area.clone();
        let b = support::started(async move { area.nap_with("ws_1", None).await }).await;
        let view = a.await.expect("the nap ran").expect("the nap landed");
        b.await.expect("the second nap ran").expect("the second nap joined");
        let answer = w.after_with(&e.entry, json!({ "answer": support::view_json(&view) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn nap_a_record_not_running_answers_its_view() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "nap: a record not running answers its view");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { phase: "napping", state: Some(Ok(MachineState::Paused)), ..Default::default() });
        let view = w.area.nap_with("ws_1", None).await.expect("a view");
        let answer = w.after_with(&e.entry, json!({ "answer": support::view_json(&view) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn nap_a_napping_record_over_a_running_machine_is_adopted_first_then_napped() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "nap: a napping record over a running machine is adopted first, then napped");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { phase: "napping", state: Some(Ok(MachineState::Running)), ..Default::default() });
        let view = w.area.nap_with("ws_1", None).await.expect("a view");
        let answer = w.after_with(&e.entry, json!({ "answer": support::view_json(&view) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn nap_a_refused_pause_puts_the_engine_s_phase_back_and_stands_the_refusal() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "nap: a refused pause puts the engine's phase back and stands the refusal");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { nap: Some(EngineError::nap_refused("m-1", "provider will not pause")), ..Default::default() });
        for _ in 0..2 {
            if let Err(err) = w.area.nap_with("ws_1", None).await {
                w.rejected(&err, false);
            }
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn nap_a_refused_stop_is_kept_too() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "nap: a refused stop is kept too");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { nap: Some(EngineError::stop_refused("m-1", "stop refused")), ..Default::default() });
        if let Err(err) = w.area.nap_with("ws_1", None).await {
            w.rejected(&err, false);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn nap_a_404_confirmed_gone_settles_gone_and_is_thrown() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "nap: a 404 confirmed gone settles gone and is thrown");
        let w = World::new(Opts { reads: Some(vec![Ok(MachineState::Gone)]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { nap: Some(support::missing("404 not found")), ..Default::default() });
        w.sessions_of();
        if let Err(err) = w.area.nap_with("ws_1", None).await {
            w.rejected(&err, false);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn nap_a_404_the_reads_do_not_confirm_takes_the_refused_road() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "nap: a 404 the reads do not confirm takes the refused road");
        let w = World::new(Opts { reads: Some(vec![Ok(MachineState::Running)]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { nap: Some(support::missing("404 not found")), ..Default::default() });
        if let Err(err) = w.area.nap_with("ws_1", None).await {
            w.rejected(&err, false);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn adopt_pause_the_record_follows_a_pause_outside_wsp() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "adopt pause: the record follows a pause outside wsp");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts::default());
        w.sessions_of();
        e.entry.borrow_mut().unchecked = true;
        e.entry.borrow_mut().record.disk_gone = Some("x".into());
        w.area.adopt_pause(&e.entry).await.expect("adopted");
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn adopt_pause_not_while_napping_waking_or_not_running() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "adopt pause: not while napping, waking or not running");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts::default());
        let view = w.view_of(&e.entry);
        e.entry.borrow_mut().napping = Some(support::ready_joined(view.clone()));
        w.area.adopt_pause(&e.entry).await.expect("returns");
        e.entry.borrow_mut().napping = None;
        e.entry.borrow_mut().waking = Some(support::ready_joined(view));
        w.area.adopt_pause(&e.entry).await.expect("returns");
        e.entry.borrow_mut().waking = None;
        e.entry.borrow_mut().record.phase = WorkspacePhase::Napping;
        w.area.adopt_pause(&e.entry).await.expect("returns");
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn adopt_running_the_record_follows_a_machine_the_provider_runs_joined() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "adopt running: the record follows a machine the provider runs, joined");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { phase: "napping", record: json!({ "gone": "old words", "firstLife": true }), ..Default::default() });
        let area = w.area.clone();
        let entry = e.entry.clone();
        let a = support::started(async move { area.adopt_running(&entry).await }).await;
        let area = w.area.clone();
        let entry = e.entry.clone();
        let b = support::started(async move { area.adopt_running(&entry).await }).await;
        a.await.expect("ran").expect("adopted");
        b.await.expect("ran").expect("joined");
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn runs_under_napping_only_a_napping_record_a_failed_read_reads_paused() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "runs under napping: only a napping record, a failed read reads paused");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { phase: "napping", state: Some(Err(support::plain("timeout"))), ..Default::default() });
        let r1 = w.area.runs_under_napping(&e.entry).await;
        e.entry.borrow_mut().record.phase = WorkspacePhase::Running;
        let r2 = w.area.runs_under_napping(&e.entry).await;
        let answer = json!({ "r1": r1, "r2": r2 });
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn settle_gone_a_confirmed_gone_marks_the_record_and_ends_its_sessions() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "settle gone: a confirmed gone marks the record and ends its sessions");
        let w = World::new(Opts { reads: Some(vec![Ok(MachineState::Gone)]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { record: json!({ "firstLife": true }), ..Default::default() });
        w.sessions_of();
        e.entry.borrow_mut().unchecked = true;
        let outcome = w.area.settle_gone(&e.entry, "machine m-1 is gone").await.expect("a verdict");
        let answer = w.after_with(&e.entry, json!({ "outcome": support::outcome_str(outcome) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn settle_gone_a_read_that_says_running_is_not_gone() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "settle gone: a read that says running is not gone");
        let w = World::new(Opts { reads: Some(vec![Ok(MachineState::Running)]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { record: json!({}), ..Default::default() });
        
        let outcome = w.area.settle_gone(&e.entry, "the poll found it").await.expect("a verdict");
        let answer = w.after_with(&e.entry, json!({ "outcome": support::outcome_str(outcome) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn settle_gone_reads_that_fail_leave_it_unchecked() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "settle gone: reads that fail leave it unchecked");
        let w = World::new(Opts { reads: Some(vec![Err(support::plain("503 down"))]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { record: json!({}), ..Default::default() });
        
        let outcome = w.area.settle_gone(&e.entry, "the poll found it").await.expect("a verdict");
        let answer = w.after_with(&e.entry, json!({ "outcome": support::outcome_str(outcome) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn mark_gone_moot_when_already_gone_on_another_machine_dropped_or_deleting() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "mark gone: moot when already gone, on another machine, dropped or deleting");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts::default());
        let mut outs = vec![w.area.mark_gone(&e.entry, "m-other", "r").await.expect("an outcome")];
        e.entry.borrow_mut().deleting = Some(support::ready_joined(()));
        outs.push(w.area.mark_gone(&e.entry, "m-1", "r").await.expect("an outcome"));
        e.entry.borrow_mut().deleting = None;
        w.rt.live.borrow_mut().shift_remove("ws_1");
        outs.push(w.area.mark_gone(&e.entry, "m-1", "r").await.expect("an outcome"));
        w.rt.live.borrow_mut().insert("ws_1".into(), e.entry.clone());
        e.entry.borrow_mut().record.phase = WorkspacePhase::Gone;
        outs.push(w.area.mark_gone(&e.entry, "m-1", "r").await.expect("an outcome"));
        let answer = w.after_with(&e.entry, json!({ "outs": outs.into_iter().map(support::outcome_str).collect::<Vec<_>>() }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn mark_gone_a_sub_thread_s_record_is_released_by_its_life() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "mark gone: a sub-thread's record is released by its life");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { sub: true, ..Default::default() });
        let outcome = w.area.mark_gone(&e.entry, "m-1", "r").await.expect("an outcome");
        let answer = w.after_with(&e.entry, json!({ "outcome": support::outcome_str(outcome) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn adopt_gone_deferred_to_a_nap_or_wake_dropped_while_reread_unchecked_said() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "adopt gone: deferred to a nap or wake, dropped while reread, unchecked said");
        let w = World::new(Opts { reads: Some(vec![Err(support::plain("503 down"))]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts::default());
        let view = w.view_of(&e.entry);
        e.entry.borrow_mut().napping = Some(support::ready_joined(view));
        w.area.adopt_gone(&e.entry, "seen").await.expect("deferred");
        e.entry.borrow_mut().napping = None;
        let area = w.area.clone();
        let entry = e.entry.clone();
        let first = support::started(async move { area.adopt_gone(&entry, "seen").await }).await;
        w.area.adopt_gone(&e.entry, "seen again").await.expect("dropped while the first reads");
        first.await.expect("ran").expect("read");
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn recover_gone_a_running_machine_brings_the_record_back_running() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "recover gone: a running machine brings the record back running");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { phase: "gone", state: Some(Ok(MachineState::Running)), record: json!({ "gone": "words", "diskGone": "d" }), ..Default::default() });
        e.entry.borrow_mut().unchecked = true;
        let phase = w.area.recover_gone(&e.entry).await.expect("a read");
        let answer = w.after_with(&e.entry, json!({ "phase": phase.map(support::phase_name) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn recover_gone_a_paused_machine_brings_it_back_napping() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "recover gone: a paused machine brings it back napping");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { phase: "gone", state: Some(Ok(MachineState::Paused)), record: json!({ "gone": "words", "diskGone": "d" }), ..Default::default() });
        let phase = w.area.recover_gone(&e.entry).await.expect("a read");
        let answer = w.after_with(&e.entry, json!({ "phase": phase.map(support::phase_name) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn recover_gone_gone_unknown_or_failed_reads_move_nothing() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "recover gone: gone, unknown or failed reads move nothing");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { phase: "gone", state: Some(Ok(MachineState::Gone)), ..Default::default() });
        let a = w.area.recover_gone(&e.entry).await.expect("a read");
        e.machine.set_state(Err(support::plain("x")));
        let b = w.area.recover_gone(&e.entry).await.expect("a read");
        let answer = w.after_with(&e.entry, json!({ "a": a.map(support::phase_name), "b": b.map(support::phase_name) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn status_a_running_row_the_poll_reads_paused_is_adopted_as_a_pause() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "status: a running row the poll reads paused is adopted as a pause");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts::default());
        w.emit(w.status_event("ws_1", "running", "paused"));
        // The TS listener runs `adoptPause` up to its first wait before `emit` returns; here the adoption is a task
        // the area starts, which gets one turn to reach the same point.
        tokio::task::yield_now().await;
        w.emit(w.status_event("ws_none", "running", "paused"));
        common::flush().await;
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn proven_a_machine_with_no_proof_road_is_ready_at_once() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "proven: a machine with no proof road is ready at once");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts::default());
        e.entry.borrow_mut().unchecked = true;
        let view = w.area.proven(&e.entry).await.expect("ready");
        let answer = w.after_with(&e.entry, json!({ "view": support::view_json(&view) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn proven_waking_while_the_proof_runs_running_after_joined() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "proven: waking while the proof runs, running after, joined");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let gate = Arc::new(Notify::new());
        let e = w.entry_in(EntryOpts { prove: Some(Prove::Gate(gate.clone())), ..Default::default() });
        e.entry.borrow_mut().unchecked = true;
        let area = w.area.clone();
        let entry = e.entry.clone();
        let a = support::started(async move { area.proven(&entry).await }).await;
        let area = w.area.clone();
        let entry = e.entry.clone();
        let b = support::started(async move { area.proven(&entry).await }).await;
        common::flush().await;
        w.log.push(json!(["during", support::phase_name(e.phase())]));
        gate.notify_one();
        let view = a.await.expect("ran").expect("proven");
        b.await.expect("ran").expect("joined");
        let answer = w.after_with(&e.entry, json!({ "view": support::view_json(&view) }));
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn proven_a_stop_ends_it_with_the_stopped_line() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "proven: a stop ends it with the stopped line");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { prove: Some(Prove::Never), ..Default::default() });
        e.entry.borrow_mut().unchecked = true;
        let area = w.area.clone();
        let entry = e.entry.clone();
        let a = support::started(async move { area.proven(&entry).await }).await;
        common::flush().await;
        let stop = e.entry.borrow().wake_stop.clone().expect("a stop the row can fire");
        stop.cancel();
        if let Err(err) = a.await.expect("ran") {
            w.rejected(&err, false);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn proven_a_restore_not_finished_says_no_commands_yet() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "proven: a restore not finished says no commands yet");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { prove: Some(Prove::Fail(EngineError::restore_unfinished("m-1", &support::plain("disk streaming")))), ..Default::default() });
        if let Err(err) = w.area.proven(&e.entry).await {
            w.rejected(&err, false);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn proven_any_other_failure_is_said_and_thrown_as_itself() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "proven: any other failure is said and thrown as itself");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { prove: Some(Prove::Fail(support::plain("exec refused"))), ..Default::default() });
        if let Err(err) = w.area.proven(&e.entry).await {
            w.rejected(&err, false);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn proven_a_machine_the_provider_lost_takes_the_gone_road() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "proven: a machine the provider lost takes the gone road");
        let w = World::new(Opts { reads: Some(vec![Ok(MachineState::Gone)]), gone_confirm_ms: 0, ..Default::default() });
        let e = w.entry_in(EntryOpts { prove: Some(Prove::Fail(support::missing("404 no such box"))), ..Default::default() });
        if let Err(err) = w.area.proven(&e.entry).await {
            w.rejected(&err, false);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

#[test]
fn proven_a_delete_that_took_the_record_has_the_last_word() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(&fx, "proven: a delete that took the record has the last word");
        let w = World::new(Opts { reads: Some(vec![]), gone_confirm_ms: 0, ..Default::default() });
        let gate = Arc::new(Notify::new());
        let e = w.entry_in(EntryOpts { prove: Some(Prove::Gate(gate.clone())), ..Default::default() });
        let area = w.area.clone();
        let entry = e.entry.clone();
        let a = support::started(async move { area.proven(&entry).await }).await;
        common::flush().await;
        w.rt.live.borrow_mut().shift_remove("ws_1");
        gate.notify_one();
        if let Err(err) = a.await.expect("ran") {
            w.rejected(&err, true);
        }
        let answer = w.after(&e.entry);
        common::flush().await;
        support::check_phases(&w, scene, answer, &e);
    })
}

