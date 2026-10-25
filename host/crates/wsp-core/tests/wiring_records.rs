// SPDX-License-Identifier: AGPL-3.0-only
//! wiring.ts against the TS oracle (tests/fixtures/core_types/wiring.json): the constants and word tables, and the
//! records read off JSON and written back as they came.

mod common;

use common::{fixture, json_of, wire};
use serde_json::{json, Value};
use wsp_core::types::wiring::*;
use wsp_frames::WorkspaceKind;
use wsp_wire::views::project_bundle::ProjectAgentOutcome;

fn same_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_json(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same_json(v, w))),
        _ => a == b,
    }
}

fn text(v: &Value) -> &str {
    v.as_str().expect("a string")
}

// --- constants -----------------------------------------------------------------------------------------------------

macro_rules! same_str {
    ($c:expr, $($name:ident),+ $(,)?) => {$(
        assert_eq!($name, text(&$c[stringify!($name)]), stringify!($name));
    )+};
}

macro_rules! same_num {
    ($c:expr, $($name:ident),+ $(,)?) => {$(
        assert_eq!($name as u64, $c[stringify!($name)].as_u64().expect("a whole number"), stringify!($name));
    )+};
}

// wiring.ts:799-821,866-868,934-1102
#[test]
fn constants_times_and_caps_match_the_ts() {
    let f = fixture("wiring");
    let c = &f["constants"];
    same_num!(
        c,
        PROVIDER_READ_MS,
        GONE_CONFIRM_MS,
        WAKE_LATE_READ_MS,
        DAEMON_HELLO_TIMEOUT_MS,
        PORT_PROBE_TIMEOUT_MS,
        CLONE_MS,
        MERGE_DEADLINE_MS,
        LISTING_DEADLINE_MS,
        PORT_PROBE_BODY_CAP,
        VAULT_CAP_BYTES,
        GRACE_MS,
        DAEMON_REVIVE_AGAIN_MS,
        DAEMON_LACKS_AGAIN_MS,
        LOG_READ_EVERY_MS,
        LOG_READ_MS,
        RESET_EXEC_MS,
        PLAN_READ_EVERY_MS,
    );
}

// wiring.ts:825,845,913
#[test]
fn constants_sentences_and_the_vaults_collection_match_the_ts() {
    let f = fixture("wiring");
    let c = &f["constants"];
    same_str!(c, ADD_IS_A_COMPUTER_LINE, NO_COPIER_HERE, VAULTS);
}

// wiring.ts:949
#[test]
fn constants_vault_skip_is_local_cache_npm_in_that_order() {
    let f = fixture("wiring");
    let want: Vec<&str> = f["constants"]["VAULT_SKIP"].as_array().unwrap().iter().map(text).collect();
    assert_eq!(VAULT_SKIP.to_vec(), want);
}

fn outcome(word: &str) -> ProjectAgentOutcome {
    wire(&Value::String(word.to_owned()))
}

// wiring.ts:851-856
#[test]
fn outcome_word_names_moved_transcript_only_carried_and_nothing_as_the_import_line_reads_them() {
    let f = fixture("wiring");
    let words = f["constants"]["OUTCOME_WORDS"].as_object().unwrap();
    assert_eq!(words.keys().map(String::as_str).collect::<Vec<_>>(), vec!["moved", "transcript-only", "carried", "nothing"]);
    for (key, want) in words {
        assert_eq!(outcome_word(outcome(key)), Some(text(want)), "outcome_word({key})");
    }
}

// wiring.ts:851-856
#[test]
fn outcome_word_of_failed_is_none() {
    assert_eq!(outcome_word(ProjectAgentOutcome::Failed), None);
}

// wiring.ts:895-900
#[test]
fn home_word_names_moved_transcript_only_carried_and_nothing_as_the_export_line_reads_them() {
    let f = fixture("wiring");
    let words = f["constants"]["HOME_WORDS"].as_object().unwrap();
    assert_eq!(words.keys().map(String::as_str).collect::<Vec<_>>(), vec!["moved", "transcript-only", "carried", "nothing"]);
    for (key, want) in words {
        assert_eq!(home_word(outcome(key)), Some(text(want)), "home_word({key})");
    }
}

// wiring.ts:895-900
#[test]
fn home_word_of_failed_is_none() {
    assert_eq!(home_word(ProjectAgentOutcome::Failed), None);
}

// --- WorkspaceRecord -----------------------------------------------------------------------------------------------

fn record_doc(extra: Value) -> Value {
    let mut doc = json!({
        "id": "w1", "name": "n", "machineId": "m1", "phase": "running", "golden": "g1", "createdAt": "2026-10-01T00:00:00.000Z",
        "project": "p1", "spec": {}, "size": {"cpu": 2, "memMb": 4096}, "firstLife": true,
    });
    for (k, v) in extra.as_object().expect("an object") {
        doc[k] = v.clone();
    }
    doc
}

// wiring.ts:318-402
#[test]
fn workspace_record_with_no_kind_reads_as_cloud() {
    let doc = record_doc(json!({}));
    assert!(doc.get("kind").is_none());
    let record: WorkspaceRecord = wire(&doc);
    assert_eq!(record.kind, WorkspaceKind::Cloud);
}

// wiring.ts:318-402
#[test]
fn workspace_record_with_a_kind_keeps_it() {
    let record: WorkspaceRecord = wire(&record_doc(json!({"kind": "local"})));
    assert_eq!(record.kind, WorkspaceKind::Local);
}

// wiring.ts:318-402
#[test]
fn workspace_record_with_no_idle_window_reads_none_and_writes_no_key() {
    let doc = record_doc(json!({"kind": "local"}));
    let record: WorkspaceRecord = wire(&doc);
    assert_eq!(record.idle_window_ms, None);
    let out = serde_json::to_value(&record).unwrap();
    assert!(out.get("idleWindowMs").is_none(), "wrote {out}");
    assert!(same_json(&out, &doc), "written back differently:\n  read    {doc}\n  written {out}");
}

// wiring.ts:318-402
#[test]
fn workspace_record_with_a_null_idle_window_reads_auto_nap_off_and_writes_null() {
    let doc = record_doc(json!({"kind": "local", "idleWindowMs": null}));
    let record: WorkspaceRecord = wire(&doc);
    assert_eq!(record.idle_window_ms, Some(None));
    let out = serde_json::to_value(&record).unwrap();
    assert_eq!(out.get("idleWindowMs"), Some(&Value::Null));
    assert!(same_json(&out, &doc), "written back differently:\n  read    {doc}\n  written {out}");
}

// wiring.ts:318-402
#[test]
fn workspace_record_with_an_idle_window_reads_its_milliseconds_and_writes_them_back() {
    let doc = record_doc(json!({"kind": "local", "idleWindowMs": 600000}));
    let record: WorkspaceRecord = wire(&doc);
    assert_eq!(record.idle_window_ms, Some(Some(wsp_wire::types::js_number::JsNumber(600000.0))));
    let out = serde_json::to_value(&record).unwrap();
    assert_eq!(out["idleWindowMs"].as_f64(), Some(600000.0));
    assert!(same_json(&out, &doc), "written back differently:\n  read    {doc}\n  written {out}");
}

// --- GoldenPromotion -----------------------------------------------------------------------------------------------

fn promotion(s: &str) -> GoldenPromotion {
    serde_json::from_str(s).unwrap_or_else(|e| panic!("{s} does not read: {e}"))
}

// wiring.ts:987
#[test]
fn golden_promotion_with_a_template_and_sharing_writes_golden_version_template_id_sharing_in_that_order() {
    let s = r#"{"golden":"g","version":2,"templateId":"t","sharing":3}"#;
    let p = promotion(s);
    assert_eq!(p.done, PromotionDone::Promoted { template_id: "t".into(), sharing: Some(3) });
    assert_eq!(json_of(&p), s);
}

// wiring.ts:987
#[test]
fn golden_promotion_with_a_template_and_no_sharing_writes_no_sharing_key() {
    let s = r#"{"golden":"g","version":2,"templateId":"t"}"#;
    let p = promotion(s);
    assert_eq!(p.done, PromotionDone::Promoted { template_id: "t".into(), sharing: None });
    assert_eq!(json_of(&p), s);
}

// wiring.ts:987
#[test]
fn golden_promotion_that_failed_reads_its_error_and_writes_golden_version_error_in_that_order() {
    let s = r#"{"golden":"g","version":2,"error":"lost"}"#;
    let p = promotion(s);
    assert_eq!(p.done, PromotionDone::Failed { error: "lost".into() });
    assert_eq!(json_of(&p), s);
}

// --- CreatedWorkspace ----------------------------------------------------------------------------------------------

fn view_doc() -> Value {
    json!({
        "id": "w1", "name": "n", "machineId": "m1", "phase": "running", "kind": "cloud", "golden": "g1",
        "createdAt": "2026-10-01T00:00:00.000Z", "project": {"id": "p1", "name": "proj", "path": "/p", "computer": "c1"},
        "folder": "/work",
    })
}

// wiring.ts:91-95
#[test]
fn created_workspace_flattens_the_view_and_writes_the_notice_beside_it() {
    let mut doc = view_doc();
    doc["notice"] = json!("heads up");
    let created: CreatedWorkspace = wire(&doc);
    assert_eq!(created.view.id, "w1");
    assert_eq!(created.view.folder.as_deref(), Some("/work"));
    assert_eq!(created.notice.as_deref(), Some("heads up"));
    let out = serde_json::to_value(&created).unwrap();
    assert_eq!(out, doc);
}

// wiring.ts:91-95
#[test]
fn created_workspace_with_no_notice_writes_exactly_the_view() {
    let doc = view_doc();
    let created: CreatedWorkspace = wire(&doc);
    assert_eq!(created.notice, None);
    assert!(created.sub.is_none());
    let out = serde_json::to_value(&created).unwrap();
    assert_eq!(out, doc);
    assert!(out.get("notice").is_none());
    assert!(out.get("sub").is_none());
}
