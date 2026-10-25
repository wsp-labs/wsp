// SPDX-License-Identifier: AGPL-3.0-only
//! BoxBackend and BoxMachine replaying the scenes `oracle/box_scenes.oracle.ts` ran on the TS: every request, sleep,
//! warn line and answer as the TS made them (spec/box_machine.md and spec/box_fleet.md, rules B-M and B-F).
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

// B-F1..B-F9: create's POST, its lost answers, and what a failure leaves.

async fn create(name: &str, spec: MachineSpec, sent: Option<String>) {
    let s = scene(name);
    let got = s.on.backend.box_create(spec, None, sent).await;
    s.same_road();
    match got {
        Ok(m) => s.same_out(json!({ "id": m.id, "replayed": m.replayed })),
        Err(e) => s.same_err(e),
    }
}

#[tokio::test]
async fn create_in_progress_waits_2000_and_the_request_asks_a_gateway_again_itself() {
    create("create_in_progress_waits_2000_and_the_request_asks_a_gateway_again_itself", spec(json!({})), None).await;
}

#[tokio::test]
async fn create_lost_then_answered_warns_the_first_lost_words_and_is_not_replayed() {
    create("create_lost_then_answered_warns_the_first_lost_words_and_is_not_replayed", spec(json!({})), None).await;
}

#[tokio::test]
async fn create_lost_until_the_window_answers_the_last_failure() {
    create("create_lost_until_the_window_answers_the_last_failure", spec(json!({})), None).await;
}

#[tokio::test]
async fn create_refused_after_a_lost_answer_is_the_refusal() {
    create("create_refused_after_a_lost_answer_is_the_refusal", spec(json!({})), None).await;
}

#[tokio::test]
async fn create_with_an_empty_key_sends_it_empty() {
    create("create_with_an_empty_key_sends_it_empty", spec(json!({ "idempotencyKey": "" })), None).await;
}

#[tokio::test]
async fn create_with_sent_bytes_names_the_box_after_the_bytes() {
    let sent = r#"{"type":"small","ttlSeconds":60,"noEnv":true,"name":"wsp;w=from-bytes"}"#.to_string();
    create("create_with_sent_bytes_names_the_box_after_the_bytes", spec(json!({})), Some(sent)).await;
}

async fn create_then_timer(name: &str, spec: MachineSpec) {
    let s = scene(name);
    let r = s.on.backend.box_create(spec, None, None).await;
    let got = json!({ "threw": threw(r), "timer": timer_json(s.on.backend.last_timer("bx_new")) });
    s.same_road();
    s.same_out(got);
}

#[tokio::test]
async fn create_replayed_that_fails_keeps_the_box_and_its_timer() {
    create_then_timer("create_replayed_that_fails_keeps_the_box_and_its_timer", spec(json!({ "idleTimeoutMs": 900_000 }))).await;
}

#[tokio::test]
async fn create_fresh_that_fails_with_a_refused_kill_keeps_its_timer() {
    create_then_timer("create_fresh_that_fails_with_a_refused_kill_keeps_its_timer", spec(json!({ "idleTimeoutMs": 900_000 }))).await;
}

#[tokio::test]
async fn create_rename_refused_kills_and_answers_the_rename() {
    create_then_timer("create_rename_refused_kills_and_answers_the_rename", spec(json!({}))).await;
}

// B-F10..B-F13: replay.

fn iso(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms).unwrap().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

async fn replay(name: &str, made_at: &str) {
    let s = scene(name);
    let bytes = r#"{"type":"small","ttlSeconds":60,"noEnv":true,"name":"wsp;w=w1"}"#;
    let before = serde_json::to_value(s.on.backend.limits().await.unwrap()).unwrap();
    let got = s.on.backend.box_replay_create("key-1", bytes, made_at).await;
    match got {
        Ok(m) => {
            let after = serde_json::to_value(s.on.backend.limits().await.unwrap()).unwrap();
            let machine = m.map(|m| {
                let seen = m.seen.clone().unwrap();
                let mut seen_json = json!({ "state": seen.state });
                if let Some(c) = seen.created_at {
                    seen_json["createdAt"] = json!(c);
                }
                json!({ "id": m.id, "kind": m.kind, "labels": m.labels, "seen": seen_json, "replayed": m.replayed })
            });
            s.same_road();
            s.same_out(json!({ "machine": machine, "countedUnchanged": before == after }));
        }
        Err(e) => {
            s.same_road();
            s.same_err(e);
        }
    }
}

#[tokio::test]
async fn replay_of_an_unparsable_time_is_asked() {
    replay("replay_of_an_unparsable_time_is_asked", "not a time").await;
}

#[tokio::test]
async fn replay_at_the_window_edge_is_not_asked() {
    replay("replay_at_the_window_edge_is_not_asked", &iso(1_000_000 - 86_275_000)).await;
}

#[tokio::test]
async fn replay_a_millisecond_inside_the_window_is_asked_and_not_counted() {
    replay("replay_a_millisecond_inside_the_window_is_asked_and_not_counted", &iso(1_000_000 - 86_274_999)).await;
}

#[tokio::test]
async fn replay_of_a_cancelled_box_is_not_renamed() {
    replay("replay_of_a_cancelled_box_is_not_renamed", &iso(1_000_000)).await;
}

// B-F14..B-F19: paging, both listings, snapshots.

#[tokio::test]
async fn list_pages_follow_an_empty_cursor_and_encode_it() {
    let s = scene("list_pages_follow_an_empty_cursor_and_encode_it");
    let got = s.on.backend.box_list(None).await.unwrap();
    s.same_road();
    s.same_out(serde_json::to_value(got).unwrap());
}

#[tokio::test]
async fn named_after_skips_nameless_boxes_and_keeps_the_first_row_per_id() {
    let s = scene("named_after_skips_nameless_boxes_and_keeps_the_first_row_per_id");
    let names: BTreeSet<String> = ["snap-a", "snap-b"].iter().map(|n| n.to_string()).collect();
    let got = s.on.backend.box_named_after(&names).await.unwrap();
    let rows: Vec<Value> = got
        .into_iter()
        .map(|m| {
            let mut v = json!({ "id": m.id, "name": m.name, "state": m.state });
            if let Some(c) = m.created_at {
                v["createdAt"] = json!(c);
            }
            v
        })
        .collect();
    s.same_road();
    s.same_out(json!(rows));
}

async fn snapshots(name: &str, ids: Option<Vec<String>>) {
    let s = scene(name);
    let got = s.on.backend.box_list_snapshots(ids.as_deref()).await.unwrap();
    s.same_road();
    s.same_out(serde_json::to_value(got).unwrap());
}

const U: &str = "dc8cd70e-d16b-472a-b5d7-070bc84db7a7";

#[tokio::test]
async fn snapshot_rows_read_blank_completion_null_chain_and_string_sizes_as_the_ts() {
    snapshots("snapshot_rows_read_blank_completion_null_chain_and_string_sizes_as_the_ts", None).await;
}

#[tokio::test]
async fn snapshot_history_asked_in_capitals_is_read_to_its_end() {
    snapshots("snapshot_history_asked_in_capitals_is_read_to_its_end", Some(vec![U.to_uppercase()])).await;
}

#[tokio::test]
async fn snapshot_history_stops_at_the_page_holding_every_asked_id() {
    snapshots("snapshot_history_stops_at_the_page_holding_every_asked_id", Some(vec![U.to_string(), "wsp-v1".to_string()])).await;
}

#[tokio::test]
async fn delete_snapshot_by_a_capital_uuid_takes_the_history_road() {
    let s = scene("delete_snapshot_by_a_capital_uuid_takes_the_history_road");
    s.on.backend.box_delete_snapshot(&U.to_uppercase()).await.unwrap();
    s.same_road();
}

// B-F20..B-F23: backstops.

async fn backstop(s: &Scene, until: i64, exact: bool) -> Value {
    match s.on.backend.box_backstop(ID, until, exact).await {
        Ok(()) => json!("ok"),
        Err(e) => json!(format!("threw: {e}")),
    }
}

#[tokio::test]
async fn backstop_whose_limits_read_fails_backs_off() {
    let s = scene("backstop_whose_limits_read_fails_backs_off");
    let got = json!([backstop(&s, 2_000_000, false).await, backstop(&s, 2_000_000, false).await, timer_json(s.on.backend.last_timer(ID))]);
    s.same_road();
    s.same_out(got);
}

#[tokio::test]
async fn backstop_in_the_past_asks_one_second() {
    let s = scene("backstop_in_the_past_asks_one_second");
    let got = json!([backstop(&s, 0, false).await, timer_json(s.on.backend.last_timer(ID))]);
    s.same_road();
    s.same_out(got);
}

#[tokio::test]
async fn backstop_exact_ignores_the_backoff() {
    let s = scene("backstop_exact_ignores_the_backoff");
    let got = json!([
        backstop(&s, 2_000_000, false).await,
        backstop(&s, 2_000_000, false).await,
        backstop(&s, 2_000_000, true).await,
        timer_json(s.on.backend.last_timer(ID))
    ]);
    s.same_road();
    s.same_out(got);
}

#[tokio::test]
async fn backstop_backoff_doubles_from_the_failure_read_at_entry() {
    let s = scene("backstop_backoff_doubles_from_the_failure_read_at_entry");
    let mut got = Vec::new();
    for wait in [0, 30_000, 60_000, 120_000, 240_000, 300_000, 299_999] {
        s.on.clock.advance(wait);
        got.push(backstop(&s, 9_000_000, false).await);
    }
    s.same_road();
    s.same_out(json!(got));
}
