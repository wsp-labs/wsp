// SPDX-License-Identifier: AGPL-3.0-only
//! BoxBackend and BoxMachine replaying the scenes `oracle/box_scenes.oracle.ts` ran on the TS: every request, sleep,
//! warn line and answer as the TS made them (spec/box_fleet.md, rules B-F).
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

#[tokio::test]
async fn backstop_after_an_hour_reads_the_kept_limits_of_any_age() {
    let s = scene("backstop_after_an_hour_reads_the_kept_limits_of_any_age");
    s.on.backend.limits().await.unwrap();
    s.on.clock.advance(3_600_000);
    let got =
        json!([backstop(&s, 99_000_000, false).await, timer_json(s.on.backend.last_timer(ID)), s.on.backend.capabilities().sizes.len()]);
    s.same_road();
    s.same_out(got);
}
