// SPDX-License-Identifier: AGPL-3.0-only
//! BoxBackend and BoxMachine replaying the scenes `oracle/box_scenes.oracle.ts` ran on the TS: every request, sleep,
//! warn line and answer as the TS made them (spec/box_machine.md, rules B-M).
mod common;

use std::collections::BTreeSet;

use common::scene::*;
use serde_json::{json, Value};
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend, MachineLife, MachineSpec, SnapshotOptions};

fn life() -> MachineLife {
    MachineLife { first_life: false }
}

fn spec(extra: Value) -> MachineSpec {
    let mut v = json!({ "kind": "sandbox", "idempotencyKey": "key-1" });
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    serde_json::from_value(v).unwrap()
}

fn timer_json(t: Option<wsp_engine::box_backend::Timer>) -> Value {
    t.map(|t| json!({ "until": t.until, "spanMs": t.span_ms })).unwrap_or(Value::Null)
}

fn threw<T>(r: wsp_engine::errors::Result<T>) -> Value {
    r.err().map(|e| json!(e.to_string())).unwrap_or(Value::Null)
}

async fn preview(name: &str, port: u16) {
    let s = scene(name);
    let got = s.machine.preview_url(port).await.unwrap();
    s.same_road();
    s.same_out(serde_json::to_value(got).unwrap());
}

// B-M1..B-M6: the firewall loop.

#[tokio::test]
async fn firewall_wiped_past_the_add_window_warns_without_words() {
    preview("firewall_wiped_past_the_add_window_warns_without_words", 3000).await;
}

#[tokio::test]
async fn firewall_keeps_the_last_refusal_after_a_later_add_held() {
    preview("firewall_keeps_the_last_refusal_after_a_later_add_held", 3000).await;
}

#[tokio::test]
async fn firewall_blank_stderr_hides_stdout() {
    preview("firewall_blank_stderr_hides_stdout", 3000).await;
}

#[tokio::test]
async fn firewall_keeps_the_last_200_units_of_a_long_refusal() {
    preview("firewall_keeps_the_last_200_units_of_a_long_refusal", 3000).await;
}

#[tokio::test]
async fn firewall_standing_read_that_throws_reads_as_not_standing() {
    preview("firewall_standing_read_that_throws_reads_as_not_standing", 8080).await;
}

#[tokio::test]
async fn route_token_is_the_decoded_query_value() {
    preview("route_token_is_the_decoded_query_value", 7070).await;
}

// B-M7..B-M11: pause.

async fn pause(name: &str, wait: Option<u64>) {
    let s = scene(name);
    let got = s.machine.pause(wait).await;
    s.same_road();
    match got {
        Ok(()) => s.same_out(Value::Null),
        Err(e) => s.same_err(e),
    }
}

#[tokio::test]
async fn pause_of_an_archived_box_sends_nothing_and_sleeps_not() {
    pause("pause_of_an_archived_box_sends_nothing_and_sleeps_not", None).await;
}

#[tokio::test]
async fn pause_archived_at_the_budget_read_is_done() {
    pause("pause_archived_at_the_budget_read_is_done", None).await;
}

#[tokio::test]
async fn pause_archiving_at_the_budget_read_times_out() {
    let s = scene("pause_archiving_at_the_budget_read_times_out");
    let e = s.machine.pause(None).await.unwrap_err();
    assert!(matches!(e, wsp_engine::errors::EngineError::MoveUnanswered { .. }), "{e:?}");
    s.same_road();
    s.same_err(e);
}

#[tokio::test]
async fn pause_of_a_lost_box_fails_missing_and_sends_no_stop() {
    let s = scene("pause_of_a_lost_box_fails_missing_and_sends_no_stop");
    let e = s.machine.pause(None).await.unwrap_err();
    assert!(wsp_engine::errors::is_missing(&e), "{e:?}");
    s.same_road();
    s.same_err(e);
}

#[tokio::test]
async fn pause_wait_stands_in_for_the_budget() {
    pause("pause_wait_stands_in_for_the_budget", Some(35)).await;
}

// B-M12..B-M15: snapshot.

async fn snapshot(name: &str) {
    let s = scene(name);
    let got = s.machine.snapshot("wsp-v1", life(), SnapshotOptions::default()).await;
    s.same_road();
    match got {
        Ok(v) => s.same_out(json!(v)),
        Err(e) => s.same_err(e),
    }
}

#[tokio::test]
async fn snapshot_failed_with_empty_error_has_no_colon() {
    snapshot("snapshot_failed_with_empty_error_has_no_colon").await;
}

#[tokio::test]
async fn snapshot_ready_at_the_budget_read_wins() {
    snapshot("snapshot_ready_at_the_budget_read_wins").await;
}

#[tokio::test]
async fn snapshot_budget_words_round_half_up() {
    snapshot("snapshot_budget_words_round_half_up").await;
}

#[tokio::test]
async fn snapshot_budget_under_half_a_second_reads_0_s() {
    snapshot("snapshot_budget_under_half_a_second_reads_0_s").await;
}

// B-M16..B-M18: kill and the timer it forgets.

async fn kill(name: &str) {
    let s = scene(name);
    s.on.backend.box_backstop(ID, 1_600_000, false).await.unwrap();
    let r = s.machine.kill().await;
    let got = json!({ "threw": threw(r), "timer": timer_json(s.on.backend.last_timer(ID)), "deletion": s.machine.deletion() });
    s.same_road();
    s.same_out(got);
}

#[tokio::test]
async fn kill_answered_without_an_operation_keeps_no_deletion_and_forgets_the_timer() {
    kill("kill_answered_without_an_operation_keeps_no_deletion_and_forgets_the_timer").await;
}

#[tokio::test]
async fn kill_refused_keeps_the_timer() {
    kill("kill_refused_keeps_the_timer").await;
}

#[tokio::test]
async fn kill_of_a_gone_box_forgets_the_timer() {
    kill("kill_of_a_gone_box_forgets_the_timer").await;
}

// B-M19..B-M25: exec's readings.

async fn exec(name: &str, timeout_ms: Option<u64>) {
    let s = scene(name);
    let got = s.machine.exec("echo hi", ExecOptions { timeout_ms, ..Default::default() }).await;
    s.same_road();
    match got {
        Ok(r) => s.same_out(serde_json::to_value(r).unwrap()),
        Err(e) => s.same_err(e),
    }
}

#[tokio::test]
async fn exec_exit_code_wins_over_timed_out() {
    exec("exec_exit_code_wins_over_timed_out", None).await;
}

#[tokio::test]
async fn exec_null_exit_timed_out_reads_124() {
    exec("exec_null_exit_timed_out_reads_124", Some(1500)).await;
}

#[tokio::test]
async fn exec_null_exit_reads_minus_1_and_absent_streams_read_empty() {
    exec("exec_null_exit_reads_minus_1_and_absent_streams_read_empty", Some(0)).await;
}

#[tokio::test]
async fn exec_at_the_inline_cap_stays_inline() {
    exec("exec_at_the_inline_cap_stays_inline", Some(30_000)).await;
}

#[tokio::test]
async fn exec_loader_emfile_on_a_later_line_is_unusable() {
    let s = scene("exec_loader_emfile_on_a_later_line_is_unusable");
    let e = s.machine.exec("echo hi", ExecOptions::default()).await.unwrap_err();
    assert!(matches!(e, wsp_engine::errors::EngineError::GuestUnusable { status: 200, .. }), "{e:?}");
    s.same_road();
    s.same_err(e);
}

#[tokio::test]
async fn exec_loader_emfile_not_at_a_line_start_is_a_plain_answer() {
    exec("exec_loader_emfile_not_at_a_line_start_is_a_plain_answer", None).await;
}

#[tokio::test]
async fn exec_no_work_folder_wins_over_machine_not_running() {
    let s = scene("exec_no_work_folder_wins_over_machine_not_running");
    let e = s.machine.exec("echo hi", ExecOptions::default()).await.unwrap_err();
    assert!(matches!(e, wsp_engine::errors::EngineError::GuestUnusable { status: 409, .. }), "{e:?}");
    s.same_road();
    s.same_err(e);
}

// B-M26..B-M34: bytes in and out.

async fn put(name: &str, path: &str, bytes: &[u8]) {
    let s = scene(name);
    let got = s.machine.put_bytes(path, bytes, None).await;
    s.same_road();
    match got {
        Ok(_) => s.same_out(Value::Null),
        Err(e) => s.same_err(e),
    }
}

#[tokio::test]
async fn put_of_no_bytes_into_tmp_is_one_empty_put() {
    put("put_of_no_bytes_into_tmp_is_one_empty_put", "/tmp/empty", &[]).await;
}

#[tokio::test]
async fn put_to_the_bare_tmp_path_joins_into_root() {
    put("put_to_the_bare_tmp_path_joins_into_root", "/tmp", &[1]).await;
}

#[tokio::test]
async fn put_to_a_relative_path_cuts_the_folder_as_the_ts_does() {
    put("put_to_a_relative_path_cuts_the_folder_as_the_ts_does", "file.txt", &[1, 2]).await;
}

#[tokio::test]
async fn put_join_failure_names_stdout_when_stderr_is_blank() {
    put("put_join_failure_names_stdout_when_stderr_is_blank", "/root/a b/x", &[1, 2]).await;
}

async fn read(name: &str, path: &str, size: Option<u64>) {
    let s = scene(name);
    let got = s.machine.read_bytes(path, size, None).await;
    s.same_road();
    match got {
        Ok(b) => s.same_out(json!({ "bytes": b })),
        Err(e) => s.same_err(e),
    }
}

#[tokio::test]
async fn read_with_a_blank_size_reads_zero_and_takes_the_straight_road() {
    read("read_with_a_blank_size_reads_zero_and_takes_the_straight_road", "/tmp/x", None).await;
}

#[tokio::test]
async fn read_size_in_exponent_form_is_a_number() {
    read("read_size_in_exponent_form_is_a_number", "/root/x", None).await;
}

#[tokio::test]
async fn read_size_with_a_fraction_is_refused() {
    read("read_size_with_a_fraction_is_refused", "/tmp/x", None).await;
}

#[tokio::test]
async fn read_cut_refused_still_removes_every_piece() {
    read("read_cut_refused_still_removes_every_piece", "/root/x", Some(4 * 1024 * 1024 + 1)).await;
}

#[tokio::test]
async fn read_short_still_removes_and_names_both_sizes() {
    read("read_short_still_removes_and_names_both_sizes", "/root/x", Some(4 * 1024 * 1024 + 1)).await;
}

#[tokio::test]
async fn describe_reads_half_a_gigabyte_and_leaves_out_what_is_missing() {
    let s = scene("describe_reads_half_a_gigabyte_and_leaves_out_what_is_missing");
    let got = s.machine.describe().await.unwrap();
    s.same_road();
    s.same_out(serde_json::to_value(got).unwrap());
}
