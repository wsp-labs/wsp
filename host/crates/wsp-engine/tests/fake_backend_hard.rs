// SPDX-License-Identifier: AGPL-3.0-only
//! The stand-in provider against the TS code's own answers (tests/fixtures/roads/fake_backend.json) and the records
//! file's restart rules. Spec: spec/fake_backend.md.
mod roads_common;

use indexmap::IndexMap;
use roads_common::*;
use serde_json::{json, Value};
use wsp_engine::fake_backend::*;
use wsp_engine::machine::{MachineBackend, MachineLife, MachineSpec, MachineState, Size};

fn spec(v: Value) -> MachineSpec {
    serde_json::from_value(v).unwrap()
}

fn backend(records: &std::path::Path) -> FakeBackend {
    FakeBackend::new(FakeBackendOptions { records: Some(records.to_path_buf()), ..Default::default() })
}

/// The random ids and the clock taken out, so a run is compared with the oracle's.
fn normalized(text: &str) -> String {
    let mut v: Value = serde_json::from_str(text).unwrap();
    let machines = v["machines"].as_object().unwrap().clone();
    let mut renamed = serde_json::Map::new();
    for (id, mut m) in machines {
        m["shape"]["createdAt"] = json!("DATE");
        let id = if id.starts_with("fk_") && id.len() == 15 && id[3..].chars().all(|c| c.is_ascii_hexdigit()) {
            "fk_ID".to_string()
        } else {
            id
        };
        renamed.insert(id, m);
    }
    v["machines"] = Value::Object(renamed);
    for row in v["snapshots"].as_array_mut().unwrap() {
        row["id"] = json!("fksnap_ID");
        row["createdAt"] = json!("DATE");
    }
    format!("{}\n", serde_json::to_string_pretty(&v).unwrap())
}

fn is_iso_millis(s: &str) -> bool {
    // 2026-10-25T00:00:00.000Z
    let b = s.as_bytes();
    s.len() == 24 && b[4] == b'-' && b[10] == b'T' && b[19] == b'.' && b[23] == b'Z' && s[20..23].chars().all(|c| c.is_ascii_digit())
}

#[tokio::test]
async fn the_records_file_is_what_the_ts_writes() {
    let d = fixture("fake_backend.json");
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("deep").join("records.json");
    let b = backend(&records);
    let made = b.create(spec(json!({ "kind": "sandbox", "cpu": 2, "labels": { "wsp-workspace": "w1" } })), None, None).await.unwrap();
    b.get("fk_c0ffee").await.unwrap().pause(None).await.unwrap();
    let snap = made.snapshot("v1", MachineLife { first_life: true }, Default::default()).await.unwrap();
    assert!(snap.starts_with("fksnap_") && snap.len() == 19, "{snap}");
    let text = std::fs::read_to_string(&records).unwrap();
    // Two-space JSON and a newline, written whole: the same bytes the TS writes, ids and clock aside.
    assert_eq!(normalized(&text), normalized(d["records"].as_str().unwrap()));
    assert_eq!(text, format!("{}\n", serde_json::to_string_pretty(&serde_json::from_str::<Value>(&text).unwrap()).unwrap()));
    let held: Value = serde_json::from_str(&text).unwrap();
    assert!(is_iso_millis(held["snapshots"][0]["createdAt"].as_str().unwrap()));
    assert!(!records.with_file_name("records.json.next").exists());
    // A restart is a new backend on the file: it finds both machines in their states and the snapshot.
    let again = backend(&records);
    let list: Vec<(String, MachineState)> = again.list(None).await.unwrap().into_iter().map(|r| (r.id, r.state)).collect();
    assert_eq!(list, vec![(made.id().to_string(), MachineState::Running), ("fk_c0ffee".to_string(), MachineState::Paused)]);
    assert_eq!(again.list_snapshots(None).await.unwrap().iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec![snap]);
}

#[test]
fn capabilities_and_rates_match_the_ts() {
    let d = fixture("fake_backend.json");
    let b = FakeBackend::new(Default::default());
    let caps = serde_json::to_value(b.capabilities()).unwrap();
    assert_eq!(serde_json::to_string(&caps).unwrap(), serde_json::to_string(&d["capabilities"]).unwrap());
    let pricing = b.pricing();
    let rates: Vec<f64> = [(2.0, 4096.0), (4.0, 8192.0), (8.0, 16384.0), (3.0, 1000.0), (1.0, 1.0)]
        .iter()
        .map(|&(cpu, mem_mb)| pricing.rate_usd_per_hour(Size { cpu, mem_mb }))
        .collect();
    let want: Vec<f64> = d["rates"].as_array().unwrap().iter().map(|r| r.as_f64().unwrap()).collect();
    assert_eq!(rates, want);
    assert_eq!(pricing.default_size(), Size { cpu: 4.0, mem_mb: 8192.0 });
    assert_eq!(pricing.builder_disk_gb(), Some(20.0));
    let storage = pricing.snapshot_storage();
    assert_eq!(serde_json::to_value(&storage.wire).unwrap(), json!({ "freeGb": 10, "usdPerGbMonth": 0.15, "billedFrom": "2026-01-01" }));
    let budgets = serde_json::to_value(b.lifecycle().unwrap().budgets()).unwrap();
    assert_eq!(budgets, json!({ "wakeAttempts": 1, "daemonAnswersMs": 1000 }));
    assert!(b.offers().list_snapshots);
}

#[tokio::test]
async fn facts_describe_and_seen_match_the_ts() {
    let d = fixture("fake_backend.json");
    let b = FakeBackend::new(Default::default());
    let made = b.create(spec(json!({ "kind": "sandbox", "cpu": 2, "labels": { "wsp-workspace": "w1" } })), None, None).await.unwrap();
    let facts = serde_json::to_value(made.facts().await.unwrap()).unwrap();
    assert_eq!(facts, d["facts"]);
    let shape = serde_json::to_value(made.describe().await.unwrap()).unwrap();
    assert_eq!((shape["cpu"].clone(), shape["memMb"].clone(), shape["diskGb"].clone()), (json!(2), json!(8192), json!(20)));
    assert!(is_iso_millis(shape["createdAt"].as_str().unwrap()));
    let seen = made.seen().unwrap();
    assert_eq!(seen.state, MachineState::Running);
    assert_eq!(seen.created_at.as_deref(), shape["createdAt"].as_str());
    assert_eq!(made.labels().unwrap(), &IndexMap::from([("wsp-workspace".to_string(), "w1".to_string())]));
}

#[tokio::test]
async fn a_machine_no_longer_recorded_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let b = backend(&records);
    let machine = b.get("m1").await.unwrap();
    // Another process dropped it from the file meanwhile.
    std::fs::write(&records, r#"{"machines":{},"snapshots":[]}"#).unwrap();
    machine.pause(None).await.unwrap();
    machine.kill().await.unwrap();
    assert_eq!(std::fs::read_to_string(&records).unwrap(), r#"{"machines":{},"snapshots":[]}"#);
    // Its state is what it was seen as when minted.
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
}

#[tokio::test]
async fn other_top_level_keys_and_seeded_rows_survive_a_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    std::fs::write(&records, r#"{"snapshots":[{"sizeBytes":8,"id":"s1","extra":"k"}],"extra":1,"machines":{}}"#).unwrap();
    backend(&records).get("a").await.unwrap();
    let text = std::fs::read_to_string(&records).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["machines", "snapshots", "extra"]);
    assert_eq!(serde_json::to_string(&v["snapshots"]).unwrap(), r#"[{"sizeBytes":8,"id":"s1","extra":"k"}]"#);
    assert_eq!(v["machines"]["a"]["state"], "running");
    assert_eq!(serde_json::to_string(&v["machines"]["a"]["labels"]).unwrap(), "{}");
    let rows = backend(&records).list_snapshots(None).await.unwrap();
    assert_eq!((rows[0].id.as_str(), rows[0].size_bytes.0), ("s1", 8.0));
}

#[tokio::test]
async fn json_that_is_not_an_object_is_an_empty_fleet_and_a_delete_writes_it_whole() {
    let dir = tempfile::tempdir().unwrap();
    for odd in ["[]", "null", "5"] {
        let records = dir.path().join("odd.json");
        std::fs::write(&records, odd).unwrap();
        assert!(backend(&records).list(None).await.unwrap().is_empty(), "{odd}");
    }
    let records = dir.path().join("compact.json");
    std::fs::write(&records, r#"{"machines":{},"snapshots":[]}"#).unwrap();
    backend(&records).delete_snapshot("nothing").await.unwrap();
    assert_eq!(std::fs::read_to_string(&records).unwrap(), "{\n  \"machines\": {},\n  \"snapshots\": []\n}\n");
}

#[tokio::test]
async fn a_known_machine_is_read_as_recorded_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let seeded =
        r#"{"machines":{"fk_1":{"state":"paused","shape":{"cpu":8,"memMb":16384,"diskGb":40},"labels":{"x":"y"}}},"snapshots":[]}"#;
    std::fs::write(&records, seeded).unwrap();
    let m = backend(&records).get("fk_1").await.unwrap();
    assert_eq!(std::fs::read_to_string(&records).unwrap(), seeded);
    assert_eq!(m.state().await.unwrap(), MachineState::Paused);
    assert_eq!(m.seen().unwrap().state, MachineState::Paused);
    assert_eq!(m.seen().unwrap().created_at, None);
    assert_eq!(serde_json::to_value(m.describe().await.unwrap()).unwrap(), json!({ "cpu": 8, "memMb": 16384, "diskGb": 40 }));
}

#[tokio::test]
async fn without_a_guest_nothing_runs_and_nothing_is_offered() {
    let m = FakeBackend::new(Default::default()).get("fk_c0ffee").await.unwrap();
    let offers = m.offers();
    assert!(!offers.preview_url && !offers.put_bytes);
    assert_eq!(m.daemon_token_path(), None);
    assert_eq!(m.exec("true", Default::default()).await.unwrap_err().to_string(), FAKE_NO_GUEST);
    assert_eq!(m.download_url("/x").await.unwrap_err().to_string(), "this provider mints no signed download URL");
    assert_eq!(m.upload_url("/x").await.unwrap_err().to_string(), "this provider mints no signed upload URL");
    assert_eq!(
        stand_in_records_unreadable("/r.json", "Unexpected end of JSON input"),
        fixture("fake_backend.json")["unreadable"].as_str().unwrap()
    );
}
