// SPDX-License-Identifier: AGPL-3.0-only
//! Ticket #2446, reader `others`: the failing tests of findings 1 to 12, each self-contained. The TS answer each one
//! asserts is written inline with the TS file and line it comes from (node 22.23.4, the review's probes). Every test
//! fails on the current code for its finding's reason.
// The one lock serializes the test that sets a process variable; it is held across the awaits on purpose.
#![allow(clippy::await_holding_lock)]
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::{Arc, Mutex};

use serde_json::json;

use common::{reply, reply_text, FakeFetch, FrozenClock, Play, Warned};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::fetch::Fetch;
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
use wsp_engine::machine::{ExecOptions, MachineBackend, MachineSpec, MachineState};
use wsp_engine::solari_backend::{preview_token_expiry, SolariBackend, SolariBackendOptions};

/// Every test that sets or removes a process variable holds this for its whole length.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn solari(f: &Arc<FakeFetch>) -> Arc<SolariBackend> {
    let fetch: Arc<dyn Fetch> = f.clone();
    SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(fetch),
        clock: Some(Arc::new(FrozenClock)),
        warn: Some(Warned::default().sink()),
        ..Default::default()
    })
}

fn sandbox() -> MachineSpec {
    serde_json::from_value(json!({"kind": "sandbox"})).unwrap()
}

fn created() -> Play {
    reply(201, json!({"sandboxId": "x", "kind": "sandbox"}))
}

fn fake_at(records: &std::path::Path) -> FakeBackend {
    FakeBackend::new(FakeBackendOptions { records: Some(records.to_path_buf()), ..Default::default() })
}

/// Finding 1 (MAJOR), a file that is not UTF-8.
/// TS fake-backend.ts:132 (`readFileSync(path, "utf8")` never fails on bytes) and :141 (text that does not parse
/// throws, nothing is written). Node output for a file `SQLite format 3\0` + ff fe 80 81, `get("fk_new")`:
///   <dir>/r1.json is not this stand-in provider's records: Unexpected token 'S', "SQLite format 3 ����" is not valid JSON
///   file after: 53514c69746520666f726d6174203300fffe8081
#[tokio::test]
async fn a_records_file_that_is_not_utf8_is_refused_and_left_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let garbage: Vec<u8> = b"SQLite format 3\0\xff\xfe\x80\x81".to_vec();
    std::fs::write(&records, &garbage).unwrap();
    let got = fake_at(&records).get("fk_new").await;
    let said = got.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        said.contains("is not this stand-in provider's records"),
        "answered {:?}, wanted the unreadable-records error",
        got.as_ref().map(|m| m.id().to_string())
    );
    assert_eq!(std::fs::read(&records).unwrap(), garbage, "the file was written over");
}

/// Finding 1, a label that is not UTF-8.
/// TS fake-backend.ts:132 (node decodes with U+FFFD). Node output for a label holding bytes ff fe, `list()`:
///   [{"id":"fk_a","state":"paused","labels":{"x":"��"}}]
#[tokio::test]
async fn a_records_file_with_a_label_that_is_not_utf8_is_read_with_replacement_characters() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let mut bytes = br#"{"machines":{"fk_a":{"state":"paused","shape":{},"labels":{"x":""#.to_vec();
    bytes.extend([0xff, 0xfe]);
    bytes.extend(br#""}}},"snapshots":[]}"#);
    std::fs::write(&records, &bytes).unwrap();
    let rows = fake_at(&records).list(None).await.unwrap();
    assert_eq!(rows.len(), 1, "the fleet read as empty");
    assert_eq!(rows[0].id, "fk_a");
    assert_eq!(rows[0].state, MachineState::Paused);
    assert_eq!(rows[0].labels.get("x").map(String::as_str), Some("\u{fffd}\u{fffd}"));
}

/// Finding 1, `machines: null`.
/// TS fake-backend.ts:299 (`held.machines[machineId]` on null throws, nothing is written). Node output, `get("fk_new")`:
///   Cannot read properties of null (reading 'fk_new')
///   file after: {"machines":null,"snapshots":[]}
#[tokio::test]
async fn a_records_file_whose_machines_is_null_is_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let torn = r#"{"machines":null,"snapshots":[]}"#;
    std::fs::write(&records, torn).unwrap();
    let got = fake_at(&records).get("fk_new").await;
    assert!(got.is_err(), "answered a machine for a file that is not records");
    assert_eq!(std::fs::read_to_string(&records).unwrap(), torn, "the file was written over");
}

/// Finding 1, `snapshots: null`.
/// TS fake-backend.ts:332 (`held.snapshots.filter` on null throws, nothing is written). Node output, `deleteSnapshot("s1")`:
///   Cannot read properties of null (reading 'filter')
///   file after: {"machines":{},"snapshots":null}
#[tokio::test]
async fn a_records_file_whose_snapshots_is_null_is_not_rewritten_by_a_delete() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let torn = r#"{"machines":{},"snapshots":null}"#;
    std::fs::write(&records, torn).unwrap();
    let got = fake_at(&records).delete_snapshot("s1").await;
    assert!(got.is_err(), "deleted from a file that is not records");
    assert_eq!(std::fs::read_to_string(&records).unwrap(), torn, "the file was written over");
}

/// Finding 2 (MINOR).
/// TS fake-backend.ts:241 (`describe`) and :245 (`facts`) on every fake machine. Node output:
///   fake machine has: {"describe":"function","facts":"function"}
#[tokio::test]
async fn a_fake_machine_offers_describe_and_facts_as_the_ts_machine_has_both() {
    let b = FakeBackend::new(FakeBackendOptions::default());
    let m = b.create(sandbox(), None, None).await.unwrap();
    let o = m.offers();
    assert!(o.describe, "describe is on the TS fake machine");
    assert!(o.facts, "facts is on the TS fake machine");
}

/// Finding 3 (MINOR).
/// TS fake-backend.ts:322-323 (`list`) and :229-231 (`state`) read the record's fields without requiring them. Node output
/// for the record `{"state":"paused"}`:
///   list: [{"id":"fk_a","state":"paused"}]
///   state: paused
#[tokio::test]
async fn a_machine_record_that_holds_only_its_state_is_listed_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    std::fs::write(&records, r#"{"machines":{"fk_a":{"state":"paused"}},"snapshots":[]}"#).unwrap();
    let b = fake_at(&records);
    let rows = b.list(None).await;
    assert_eq!(rows.as_ref().map(|r| r.len()).ok(), Some(1), "list answered {:?}", rows.as_ref().err().map(|e| e.to_string()));
    let m = b.get("fk_a").await;
    assert!(m.is_ok(), "get answered {:?}", m.err().map(|e| e.to_string()));
}

/// Finding 4 (MINOR), a listed sandbox of an unknown kind.
/// TS solari-backend.ts:328-335 (the listing never reads `kind`). Node output for `{sandboxes:[{a, kind "browser"},{b, kind "sandbox", paused}]}`:
///   list: [{"id":"a","state":"running","labels":{}},{"id":"b","state":"paused","labels":{}}]
#[tokio::test]
async fn one_listed_sandbox_of_a_kind_the_table_never_learned_does_not_fail_the_listing() {
    let f = FakeFetch::fixed(vec![(
        "GET /sandboxes",
        reply(
            200,
            json!({"sandboxes": [{"sandboxId": "a", "kind": "browser", "state": "running"}, {"sandboxId": "b", "kind": "sandbox", "state": "paused"}]}),
        ),
    )]);
    let rows = solari(&f).list(None).await;
    assert_eq!(rows.as_ref().map(|r| r.len()).ok(), Some(2), "list answered {:?}", rows.err().map(|e| e.to_string()));
}

/// Finding 4, a template of an unknown status.
/// TS solari-backend.ts:305-307 (`checkKey` is `listTemplates`) and :53 (`templateRowOf` copies `status`). Node output
/// for `{templates:[{t1, base, status "pending"}]}`:
///   checkKey: resolved
///   listTemplates: [{"id":"t1","name":"base","status":"pending"}]
#[tokio::test]
async fn the_key_check_passes_when_a_template_carries_a_status_word_the_table_never_learned() {
    let f = FakeFetch::fixed(vec![(
        "GET /templates",
        reply(200, json!({"templates": [{"templateId": "t1", "name": "base", "status": "pending"}]})),
    )]);
    let got = solari(&f).check_key().await;
    assert!(got.is_ok(), "check_key answered {:?}", got.err().map(|e| e.to_string()));
}

/// Finding 5 (NIT), a refusal whose `error` is an object or an array.
/// TS solari-backend.ts:246 and :157-158 (`new Error(e.message || ...)` reads any truthy value through String()).
/// Node output for a DELETE answering 500:
///   error={"message":"nested"}: message "[object Object]" (typeof object)
///   error=["a","b"]: message "a,b" (typeof object)
///   error=[[1]]: message "1" (typeof object)
#[tokio::test]
async fn a_refusal_whose_error_is_an_object_or_an_array_reads_as_javascript_writes_it() {
    for (error, want) in [(json!({"message": "nested"}), "[object Object]"), (json!(["a", "b"]), "a,b"), (json!([[1]]), "1")] {
        let f = FakeFetch::fixed(vec![("POST /sandboxes", created()), ("DELETE /sandboxes/x", reply(500, json!({ "error": error })))]);
        let b = solari(&f);
        let m = b.create(sandbox(), None, None).await.unwrap();
        let e = m.kill().await.unwrap_err();
        assert_eq!(e.to_string(), want, "error {error}");
    }
}

/// Finding 6 (NIT), a create reply with `createdAt: null`.
/// TS solari-backend.ts:299 (`res.createdAt !== undefined`, and null is not undefined). Node output for
/// `{sandboxId:"x", kind:"sandbox", createdAt:null, state:"paused"}`:
///   seen: {"state":"paused","createdAt":null}
#[tokio::test]
async fn a_create_reply_with_a_null_created_at_still_carries_the_state_it_named() {
    let f = FakeFetch::fixed(vec![(
        "POST /sandboxes",
        reply(201, json!({"sandboxId": "x", "kind": "sandbox", "createdAt": null, "state": "paused"})),
    )]);
    let m = solari(&f).create(sandbox(), None, None).await.unwrap();
    assert_eq!(m.seen().map(|s| s.state), Some(MachineState::Paused));
}

/// Finding 7 (NIT), a preview expiry past i64.
/// TS solari-backend.ts:151 (`Number(exp)` is a double of any size). Node output for a token whose claims hold
/// `"exp":100000000000000000000`:
///   exp 1e20: 100000000000000000000
#[tokio::test]
async fn a_preview_expiry_past_the_range_of_an_i64_reads_as_the_js_number() {
    use base64::Engine as _;
    let token = format!("{}.sig", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"exp":100000000000000000000}"#));
    assert_eq!(preview_token_expiry(&token, 1000) as f64, 1e20);
}

/// Finding 8 (MINOR), a variable set after the local backend was built.
/// TS local-backend.ts:195 (`opts.env ?? process.env`, the live object, read at each spawn). Node output for
/// `printf %s "$REVIEW_OTHERS_LATE_VAR"` after setting it:
///   exec after env set: set-after-construction
#[tokio::test]
async fn the_local_machine_runs_a_command_under_the_process_environment_as_it_is_now() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let b = LocalBackend::new(LocalBackendOptions { root: dir.path().join("work"), env: None });
    let m = b.get("local").await.unwrap();
    std::env::set_var("REVIEW_OTHERS_LATE_VAR", "set-after-construction");
    let r = m.exec("printf %s \"$REVIEW_OTHERS_LATE_VAR\"", ExecOptions::default()).await.unwrap();
    std::env::remove_var("REVIEW_OTHERS_LATE_VAR");
    assert_eq!(r.stdout, "set-after-construction");
}

/// Finding 9 (NIT), a reply nested 200 deep in a field nobody reads.
/// TS solari-backend.ts:494-497 (`state()` after `JSON.parse`, which parses 5,000,000 levels). Node output for
/// `{"state":"paused","x":[[...200 deep...]]}`:
///   state: paused
#[tokio::test]
async fn a_reply_with_an_unread_field_nested_two_hundred_deep_still_reads() {
    let body = format!(r#"{{"state":"paused","x":{}{}}}"#, "[".repeat(200), "]".repeat(200));
    let f = FakeFetch::fixed(vec![("POST /sandboxes", created()), ("GET /sandboxes/x", reply_text(200, &body))]);
    let b = solari(&f);
    let m = b.create(sandbox(), None, None).await.unwrap();
    assert_eq!(m.state().await.unwrap(), MachineState::Paused);
}

/// Finding 10 (NIT, wsp-wire), a number from 1e21 up.
/// TS (JavaScript's own number printing, as `JSON.stringify` writes every number the TS answers). Node output:
///   String(1e21) + " and JSON " + JSON.stringify(1e21): 1e+21 and JSON 1e+21
#[test]
fn a_number_from_1e21_up_is_written_as_js_writes_it() {
    use wsp_wire::types::JsNumber;
    assert_eq!(serde_json::to_string(&JsNumber(1e21)).unwrap(), "1e+21");
    assert_eq!(serde_json::to_value(JsNumber(1e21)).ok().map(|v| v.to_string()), Some("1e+21".to_string()));
}

/// Finding 11 (NIT), the stand-in copier's worktree path.
/// TS copier.ts:155 (`join(ask.home, "worktrees", ask.project, branch)`, node's path.join keeps a trailing slash).
/// Node output for `join("/h", "worktrees", "x/", "")`:
///   join: /h/worktrees/x/
#[tokio::test]
async fn the_stand_in_worktree_path_keeps_the_trailing_slash_path_join_keeps() {
    use wsp_engine::copier::{fake_copier, Copier, WorktreeAsk};
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("h").to_str().unwrap().to_string();
    let ask: WorktreeAsk =
        serde_json::from_value(json!({"from": "/f", "home": home, "project": "x/", "branch": "", "modules": []})).unwrap();
    let got = fake_copier(None, None).worktree(&ask).await.unwrap();
    assert_eq!(got.path, format!("{home}/worktrees/x/"));
}

/// Finding 12 (NIT), the order of the fake's machines.
/// TS fake-backend.ts:323 (`Object.entries(...machines)` walks integer-like keys first, ascending). Node output
/// for the machines `b`, `2`, `1`, `a` in the file, in that order:
///   ORDER_TS ["1","2","b","a"]
#[tokio::test]
async fn the_fake_lists_integer_like_machine_ids_first_as_a_javascript_object_walks_them() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let m = |s: &str| format!(r#"{{"state":"{s}","shape":{{}},"labels":{{}}}}"#);
    std::fs::write(
        &records,
        format!(r#"{{"machines":{{"b":{},"2":{},"1":{},"a":{}}},"snapshots":[]}}"#, m("running"), m("paused"), m("gone"), m("running")),
    )
    .unwrap();
    let ids: Vec<String> = fake_at(&records).list(None).await.unwrap().into_iter().map(|r| r.id).collect();
    assert_eq!(ids, ["1", "2", "b", "a"]);
}
