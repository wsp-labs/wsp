// SPDX-License-Identifier: AGPL-3.0-only
//! One test per scene of the daemon oracle: the same world, the same act, and the log, the answers and the records the
//! TS wrote. The fixture carries each scene's `world`; the act of each is encoded here from oracle/daemon.oracle.ts.
//! Spec: spec/daemon.md.

mod common;
#[path = "support/daemon.rs"]
mod support;

use serde_json::{json, Value};
use support::{answer_json, flag, moments_json, reach, session_event, spawn, status, World};
use wsp_core::types::internal::MachineMoment;
use wsp_machines::machines::machines::Unreached;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;

use common::T0;

fn start(name: &str) -> (Value, World) {
    let sc = common::scene(&common::fixture("daemon"), name).clone();
    let w = World::new(&sc["world"]);
    (sc, w)
}

fn revive_again_ms() -> i64 {
    common::fixture("daemon")["DAEMON_REVIVE_AGAIN_MS"].as_i64().expect("DAEMON_REVIVE_AGAIN_MS")
}

async fn finish(sc: &Value, w: &World, skip: &[&str]) {
    common::flush().await;
    w.check(sc, skip);
}

async fn finish_with(sc: &Value, w: &World, skip: &[&str], fix: &dyn Fn(&mut Vec<Value>)) {
    common::flush().await;
    w.check_with(sc, skip, fix);
}

#[test]
fn sync_current_daemon_writes_roots_and_asks_the_version() {
    common::run_local(async {
        let (sc, w) = start("sync: current daemon writes roots and asks the version");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_an_old_daemon_is_replaced_noted_updating_roots_written_after() {
    common::run_local(async {
        let (sc, w) = start("sync: an old daemon is replaced, noted updating, roots written after");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_no_daemon_yet_is_noted_installing() {
    common::run_local(async {
        let (sc, w) = start("sync: no daemon yet is noted installing");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_version_not_read_is_marked_for_the_next_probe() {
    common::run_local(async {
        let (sc, w) = start("sync: a version not read is marked for the next probe");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        w.push(json!(["unreadAt", moments_json(&w.area.unread_at().borrow())]));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_version_read_that_throws_rejects_the_sync() {
    common::run_local(async {
        let (sc, w) = start("sync: a version read that throws rejects the sync");
        if let Err(e) = w.area.sync_daemon(&w.entry("a")).await {
            w.push(json!(["rejected", e.message]));
        }
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_served_by_its_computer_a_folder_writes_roots_only() {
    common::run_local(async {
        let (sc, w) = start("sync: served by its computer, a folder, writes roots only");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_served_by_its_computer_not_a_folder_does_nothing() {
    common::run_local(async {
        let (sc, w) = start("sync: served by its computer, not a folder, does nothing");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_machine_that_said_it_lacks_something_inside_the_hour_is_left_alone() {
    common::run_local(async {
        let (sc, w) = start("sync: a machine that said it lacks something inside the hour is left alone");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_the_hour_out_it_is_offered_again_and_a_deploy_that_lands_clears_the_mark() {
    common::run_local(async {
        let (sc, w) = start("sync: the hour out, it is offered again and a deploy that lands clears the mark");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_another_machine_s_refusal_is_taken_off_and_the_sync_goes_on() {
    common::run_local(async {
        let (sc, w) = start("sync: another machine's refusal is taken off and the sync goes on");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_deploy_refused_for_what_the_machine_lacks_keeps_its_sentence_and_flashes_() {
    common::run_local(async {
        let (sc, w) = start("sync: a deploy refused for what the machine lacks keeps its sentence and flashes it");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_deploy_the_machine_never_answered_keeps_an_old_refusal() {
    common::run_local(async {
        let (sc, w) = start("sync: a deploy the machine never answered keeps an old refusal");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_any_other_deploy_failure_drops_the_refusal_and_flashes_update_failed() {
    common::run_local(async {
        let (sc, w) = start("sync: any other deploy failure drops the refusal and flashes update failed");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_an_install_that_fails_flashes_install_failed() {
    common::run_local(async {
        let (sc, w) = start("sync: an install that fails flashes install failed");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_no_road_to_deploy_asks_no_version() {
    common::run_local(async {
        let (sc, w) = start("sync: no road to deploy asks no version");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_no_byte_road_and_no_signed_urls_asks_no_version() {
    common::run_local(async {
        let (sc, w) = start("sync: no byte road and no signed urls asks no version");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_byte_road_with_no_signed_urls_deploys() {
    common::run_local(async {
        let (sc, w) = start("sync: a byte road with no signed urls deploys");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_waits_out_a_running_turn_then_deploys() {
    common::run_local(async {
        let (sc, w) = start("sync: waits out a running turn, then deploys");
        let p = spawn(w.area.sync_daemon(&w.entry("a")));
        common::flush().await;
        w.push(json!(["mark", "turn ends"]));
        w.set_session_status("s1", "completed");
        w.emit(session_event("session.delta", "a"));
        w.emit(session_event("session.done", "a"));
        p.await.expect("the sync task").expect("the sync");
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_queued_row_is_no_turn() {
    common::run_local(async {
        let (sc, w) = start("sync: a queued row is no turn");
        let _ = w.area.sync_daemon(&w.entry("a")).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_a_machine_that_napped_under_the_wait_is_left_alone() {
    common::run_local(async {
        let (sc, w) = start("sync: a machine that napped under the wait is left alone");
        let p = spawn(w.area.sync_daemon(&w.entry("a")));
        common::flush().await;
        w.set_phase("a", "napping");
        w.set_session_status("s1", "completed");
        w.emit(session_event("session.end", "a"));
        p.await.expect("the sync task").expect("the sync");
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn sync_two_at_once_on_one_machine_share_the_work() {
    common::run_local(async {
        let (sc, w) = start("sync: two at once on one machine share the work");
        let entry = w.entry("a");
        let p1 = spawn(w.area.sync_daemon(&entry));
        let p2 = spawn(w.area.sync_daemon(&entry));
        // The oracle's ["same", true, 1] is logged the moment the second call returns; whether the first call's own
        // lines come before it is the call's start, not its contract, so it is asserted here and not in the log.
        let held = w.area.daemon_syncs();
        assert_eq!(held, vec!["m-a".to_string()], "two syncs on one machine hold one key");
        common::flush().await;
        let settled = w.area.daemon_settled(&entry);
        w.open_deploy();
        p1.await.expect("the first sync task").expect("the first sync");
        p2.await.expect("the second sync task").expect("the second sync");
        settled.await;
        common::flush().await;
        w.push(json!(["after", w.area.daemon_syncs().len()]));
        finish(&sc, &w, &["same"]).await;
    })
}

#[test]
fn roots_every_record_on_the_machine_deduped_landed_folders_after() {
    common::run_local(async {
        let (sc, w) = start("roots: every record on the machine, deduped, landed folders after");
        let _ = w.area.write_daemon_roots(&w.entry("a"), false).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn roots_a_closed_host_writes_nothing() {
    common::run_local(async {
        let (sc, w) = start("roots: a closed host writes nothing");
        let _ = w.area.write_daemon_roots(&w.entry("a"), false).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn roots_a_failure_warns_the_last_200_characters() {
    common::run_local(async {
        let (sc, w) = start("roots: a failure warns the last 200 characters");
        let _ = w.area.write_daemon_roots(&w.entry("a"), false).await;
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn roots_strict_hands_the_failure_back() {
    common::run_local(async {
        let (sc, w) = start("roots: strict hands the failure back");
        if let Err(e) = w.area.write_daemon_roots(&w.entry("a"), true).await {
            w.push(json!(["rejected", e.message.encode_utf16().count()]));
        }
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn revive_no_daemon_on_a_running_machine_deploys_notes_restarting_clears() {
    common::run_local(async {
        let (sc, w) = start("revive: no-daemon on a running machine deploys, notes restarting, clears");
        let entry = w.entry("a");
        w.area.revive_daemon(&entry, reach("no-daemon"));
        common::flush().await;
        w.push(json!(["revivedAt", moments_json(&w.area.revived_at())]));
        w.area.revive_daemon(&entry, reach("no-daemon"));
        w.clock.advance(revive_again_ms() - 1);
        w.area.revive_daemon(&entry, reach("no-daemon"));
        w.clock.advance(1);
        w.area.revive_daemon(&entry, reach("no-daemon"));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn revive_only_for_no_daemon_on_a_running_machine_with_a_road_and_no_unreached_mark() {
    common::run_local(async {
        let (sc, w) = start("revive: only for no-daemon on a running machine with a road and no unreached mark");
        w.area.revive_daemon(&w.entry("a"), reach("reachable"));
        w.area.revive_daemon(&w.entry("b"), reach("no-daemon"));
        w.area.revive_daemon(&w.entry("c"), reach("no-daemon"));
        w.area.unreached().borrow_mut().insert("d".to_string(), Unreached { machine_id: "m-d".to_string(), line: "cannot reach".to_string() });
        w.area.revive_daemon(&w.entry("d"), reach("no-daemon"));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn revive_a_failed_restart_flashes_restart_failed() {
    common::run_local(async {
        let (sc, w) = start("revive: a failed restart flashes restart failed");
        w.area.revive_daemon(&w.entry("a"), reach("no-daemon"));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn revive_one_at_a_time_per_machine() {
    common::run_local(async {
        let (sc, w) = start("revive: one at a time per machine");
        let entry = w.entry("a");
        w.area.revive_daemon(&entry, reach("no-daemon"));
        w.clock.advance(revive_again_ms());
        w.area.revive_daemon(&entry, reach("no-daemon"));
        common::flush().await;
        w.open_deploy();
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn unreachedof_a_replaced_machine_s_mark_is_dropped() {
    common::run_local(async {
        let (sc, w) = start("unreachedOf: a replaced machine's mark is dropped");
        let entry = w.entry("a");
        let unreached = w.area.unreached();
        let read = |w: &World| {
            let of = w.area.unreached_of(&entry);
            let held: Vec<Value> = unreached.borrow().iter().map(|(k, u)| json!([k, {"machineId": u.machine_id, "line": u.line}])).collect();
            w.push(json!(["of", of, held]));
        };
        unreached.borrow_mut().insert("a".to_string(), Unreached { machine_id: "m-old".to_string(), line: "cannot reach".to_string() });
        read(&w);
        unreached.borrow_mut().insert("a".to_string(), Unreached { machine_id: "m-a".to_string(), line: "cannot reach".to_string() });
        read(&w);
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn offer_again_only_with_a_refusal_and_facts() {
    common::run_local(async {
        let (sc, w) = start("offer again: only with a refusal and facts");
        w.area.offer_daemon_again(&w.entry("a"), &status("running", "reachable", false));
        w.area.offer_daemon_again(&w.entry("b"), &status("running", "reachable", true));
        w.push(json!(["mark", "with facts"]));
        w.area.offer_daemon_again(&w.entry("a"), &status("running", "reachable", true));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn read_again_after_the_window_reachable_and_running_same_machine() {
    common::run_local(async {
        let (sc, w) = start("read again: after the window, reachable and running, same machine");
        let entry = w.entry("a");
        let unread = w.area.unread_at();
        let moment = |machine: &str| MachineMoment { machine_id: machine.to_string(), at: T0 as f64 };
        w.area.read_version_again(&entry, &status("running", "reachable", false));
        unread.borrow_mut().insert("a".to_string(), moment("m-old"));
        w.area.read_version_again(&entry, &status("running", "reachable", false));
        w.push(json!(["unread", moments_json(&unread.borrow())]));
        unread.borrow_mut().insert("a".to_string(), moment("m-a"));
        w.clock.advance(revive_again_ms() - 1);
        w.area.read_version_again(&entry, &status("running", "reachable", false));
        w.clock.advance(1);
        w.area.read_version_again(&entry, &status("napping", "reachable", false));
        w.area.read_version_again(&entry, &status("running", "no-daemon", false));
        w.push(json!(["mark", "now"]));
        w.area.read_version_again(&entry, &status("running", "reachable", false));
        common::flush().await;
        w.push(json!(["unread", moments_json(&unread.borrow())]));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn turnruns_counts_a_queued_row_daemonsettled_does_not_wait_beside_a_turn() {
    common::run_local(async {
        let (sc, w) = start("turnRuns counts a queued row, daemonSettled does not wait beside a turn");
        w.push(json!(["turnRuns", w.area.turn_runs("a"), w.area.turn_runs("b")]));
        let p = spawn(w.area.sync_daemon(&w.entry("a")));
        common::flush().await;
        let settled = flag();
        let (area, entry, done) = (w.area.clone(), w.entry("a"), settled.clone());
        spawn(async move {
            area.daemon_settled(&entry).await;
            done.set(true);
        });
        common::flush().await;
        w.push(json!(["settled before the deploy ends", settled.get()]));
        w.open_deploy();
        p.await.expect("the sync task").expect("the sync");
        common::flush().await;
        w.push(json!(["settled after", settled.get()]));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn send_detached_started() {
    common::run_local(async {
        let (sc, w) = start("send detached: started");
        let answer = w.area.send_detached("a", "hi", None, None).await.expect("the send");
        w.push(json!(["answer", answer_json(&answer)]));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn send_detached_steered() {
    common::run_local(async {
        let (sc, w) = start("send detached: steered");
        let answer = w.area.send_detached("a", "hi", Some("t1"), Some(&Caller::Road(WorkspaceOrigin::Here))).await.expect("the send");
        w.push(json!(["answer", answer_json(&answer)]));
        // The oracle's origin is the loose object {kind: "person"}; the Rust origin is a road, and the fake writes its word.
        finish_with(&sc, &w, &[], &|log| log[0][3] = json!({"kind": "here"})).await;
    })
}

#[test]
fn send_detached_queued_answers_at_the_event_with_this_request_s_id() {
    common::run_local(async {
        let (sc, w) = start("send detached: queued answers at the event with this request's id");
        let answer = w.area.send_detached("a", "hi", Some("t1"), None).await.expect("the send");
        w.push(json!(["answer", answer_json(&answer)]));
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        // The oracle did not pin randomUUID, so the fixture holds one run's id; the expected id is this run's own.
        let request = w.last_request_id();
        finish_with(&sc, &w, &[], &|log| log[2][1]["requestId"] = json!(request)).await;
    })
}

#[test]
fn send_detached_held_answers_the_wait() {
    common::run_local(async {
        let (sc, w) = start("send detached: held answers the wait");
        let answer = w.area.send_detached("a", "hi", None, None).await.expect("the send");
        w.push(json!(["answer", answer_json(&answer)]));
        // The oracle's wait is {place, running, limit}; the Rust wait is {placeId, place, running, atOnce}.
        finish_with(&sc, &w, &[], &|log| log[1][1]["capped"] = json!({"placeId": "box", "place": "box", "running": 2, "atOnce": 2})).await;
    })
}

#[test]
fn send_detached_a_refused_start_rejects() {
    common::run_local(async {
        let (sc, w) = start("send detached: a refused start rejects");
        if let Err(e) = w.area.send_detached("a", "hi", None, None).await {
            w.push(json!(["rejected", e.message]));
        }
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn to_first_thread_the_earliest_row_with_a_thread_ties_to_the_first_listed() {
    common::run_local(async {
        let (sc, w) = start("to first thread: the earliest row with a thread, ties to the first listed");
        let answer = w.area.to_first_thread("a", "go", None).await.expect("the send");
        w.push(json!(["answer", answer_json(&answer)]));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn to_first_thread_none_opens_one() {
    common::run_local(async {
        let (sc, w) = start("to first thread: none opens one");
        let answer = w.area.to_first_thread("a", "go", None).await.expect("the send");
        w.push(json!(["answer", answer_json(&answer)]));
        finish(&sc, &w, &[]).await;
    })
}

#[test]
fn to_first_thread_a_row_with_no_startedat_counts_as_0() {
    common::run_local(async {
        let (sc, w) = start("to first thread: a row with no startedAt counts as 0");
        let answer = w.area.to_first_thread("a", "go", None).await.expect("the send");
        w.push(json!(["answer", answer_json(&answer)]));
        finish(&sc, &w, &[]).await;
    })
}
