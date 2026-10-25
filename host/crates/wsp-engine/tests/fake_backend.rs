// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/fake-backend.test.ts. Spec: spec/fake_backend.md.
//!
//! The provider a harness serves a fixture through: it answers for machines it never minted, says plainly that there
//! is no guest behind them until one is wired, and keeps what it holds where a second process reads it.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use wsp_engine::errors::Result;
use wsp_engine::fake_backend::{stand_in_records_unreadable, FakeBackend, FakeBackendOptions, FakeGuest, FAKE_NO_GUEST};
use wsp_engine::machine::{ExecOptions, MachineBackend, MachineLife, MachineState, PreviewReach, RunOptions, SnapshotOptions, SnapshotRow};
use wsp_wire::types::js_number::JsNumber;

fn err_of<T>(r: Result<T>) -> String {
    match r {
        Ok(_) => panic!("expected a failure, got a success"),
        Err(e) => e.to_string(),
    }
}

fn backend_on(records: &Path) -> FakeBackend {
    FakeBackend::new(FakeBackendOptions { records: Some(records.to_path_buf()), ..Default::default() })
}

fn ids(rows: &[SnapshotRow]) -> Vec<String> {
    rows.iter().map(|r| r.id.clone()).collect()
}

#[tokio::test]
async fn names_itself_when_a_road_reaches_for_the_guest() {
    let machine = FakeBackend::new(FakeBackendOptions::default()).get("fk_c0ffee").await.unwrap();
    assert!(err_of(machine.exec("echo WSP_LAUNCHED", ExecOptions::default()).await).contains(FAKE_NO_GUEST));
    let run = RunOptions { deadline_ms: 1_000, ..Default::default() };
    assert!(err_of(machine.run("echo hi", run).await).contains(FAKE_NO_GUEST));
    assert!(FAKE_NO_GUEST.contains("stand-in provider for testing"));
    assert!(FAKE_NO_GUEST.contains("nothing runs on them"));
    for jargon in ["out of memory", "no guest", "capability", "landsBytes"] {
        assert!(!FAKE_NO_GUEST.contains(jargon), "{jargon}");
    }
}

#[tokio::test]
async fn still_answers_for_a_machine_a_fixture_names() {
    let backend = FakeBackend::new(FakeBackendOptions::default());
    assert_eq!(backend.get("fk_c0ffee").await.unwrap().state().await.unwrap(), MachineState::Running);
    assert_eq!(backend.get("fk_c0ffee.paused").await.unwrap().state().await.unwrap(), MachineState::Running);
}

#[tokio::test]
async fn holds_its_machines_in_the_file_it_was_given() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    backend_on(&records).get("fk_c0ffee").await.unwrap().pause(None).await.unwrap();
    let second = backend_on(&records);
    assert_eq!(second.get("fk_c0ffee").await.unwrap().state().await.unwrap(), MachineState::Paused);
    let listed: Vec<String> = second.list(None).await.unwrap().into_iter().map(|m| m.id).collect();
    assert_eq!(listed, ["fk_c0ffee"]);
}

#[tokio::test]
async fn lists_the_snapshots_a_harness_seeded_and_the_ones_its_machines_take() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let sealed = SnapshotRow {
        id: "fksnap_v2".to_string(),
        name: Some("wsp-standin-default-v2".to_string()),
        size_bytes: JsNumber(8.0),
        restored_bytes: None,
        created_at: Some("2026-09-12T09:00:00Z".to_string()),
        parent: None,
    };
    let seeded = json!({
        "machines": {},
        "snapshots": [{ "id": "fksnap_v2", "name": "wsp-standin-default-v2", "sizeBytes": 8, "createdAt": "2026-09-12T09:00:00Z" }],
    });
    std::fs::write(&records, seeded.to_string()).unwrap();
    let backend = backend_on(&records);
    assert_eq!(backend.list_snapshots(None).await.unwrap(), vec![sealed.clone()]);
    let machine = backend.get("fk_c0ffee").await.unwrap();
    let taken = machine.snapshot("wsp-standin-project-notes", MachineLife { first_life: true }, SnapshotOptions::default()).await.unwrap();
    assert_eq!(ids(&backend_on(&records).list_snapshots(None).await.unwrap()), [sealed.id.clone(), taken.clone()]);
    backend.delete_snapshot(&sealed.id).await.unwrap();
    assert_eq!(ids(&backend_on(&records).list_snapshots(None).await.unwrap()), [taken]);
    let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(&records).unwrap()).unwrap();
    assert_eq!(on_disk["machines"]["fk_c0ffee"]["state"], "running");
}

#[tokio::test]
async fn never_takes_a_file_it_cannot_read_for_an_empty_fleet() {
    let dir = tempfile::tempdir().unwrap();
    let records = dir.path().join("records.json");
    let whole = json!({
        "machines": { "fk_slr_2": { "state": "paused", "shape": { "cpu": 4, "memMb": 8192, "diskGb": 20 }, "labels": {} } },
        "snapshots": [{ "id": "fksnap_v1", "sizeBytes": 8 }, { "id": "fksnap_v2", "sizeBytes": 8 }],
    })
    .to_string();
    let torn = whole[..whole.len() / 2].to_string();
    std::fs::write(&records, &torn).unwrap();
    let backend = backend_on(&records);
    let sentence = stand_in_records_unreadable(records.to_str().unwrap(), "");
    let up_to_colon = &sentence[..sentence.rfind(':').unwrap()];
    assert!(err_of(backend.get("fk_slr_1").await).starts_with(up_to_colon));
    assert!(err_of(backend.list_snapshots(None).await).contains("is not this stand-in provider's records"));
    assert_eq!(std::fs::read_to_string(&records).unwrap(), torn);

    std::fs::write(&records, &whole).unwrap();
    backend.get("fk_slr_1").await.unwrap().pause(None).await.unwrap();
    assert!(!PathBuf::from(format!("{}.next", records.display())).exists());
    let after = backend_on(&records);
    assert_eq!(ids(&after.list_snapshots(None).await.unwrap()), ["fksnap_v1", "fksnap_v2"]);
    assert_eq!(after.get("fk_slr_2").await.unwrap().state().await.unwrap(), MachineState::Paused);

    let fresh_dir = tempfile::tempdir().unwrap();
    let fresh = backend_on(&fresh_dir.path().join("records.json"));
    assert_eq!(fresh.list_snapshots(None).await.unwrap(), vec![]);
    assert_eq!(fresh.list(None).await.unwrap(), vec![]);
}

struct Guest {
    root: PathBuf,
}

#[async_trait]
impl FakeGuest for Guest {
    fn folder(&self, machine_id: &str) -> PathBuf {
        self.root.join(machine_id)
    }
    fn token_path(&self, machine_id: &str) -> String {
        self.root.join(machine_id).join("token").to_str().unwrap().to_string()
    }
    async fn reach(&self, machine_id: &str, _port: u16) -> Result<PreviewReach> {
        Ok(PreviewReach { url: format!("http://127.0.0.1:1/{machine_id}"), token: String::new(), expires_at: JsNumber(1.0) })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_a_machines_commands_in_that_machines_own_folder_once_a_guest_is_wired() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let guest: Arc<dyn FakeGuest> = Arc::new(Guest { root: root.clone() });
    assert!(!FakeBackend::new(FakeBackendOptions::default()).capabilities().preview_urls);
    assert!(!FakeBackend::new(FakeBackendOptions::default()).get("fk_c0ffee").await.unwrap().offers().preview_url);
    let backend = FakeBackend::new(FakeBackendOptions { guest: Some(guest), ..Default::default() });
    assert!(backend.capabilities().preview_urls);
    let machine = backend.get("fk_c0ffee").await.unwrap();
    let pwd = machine.exec("pwd", ExecOptions::default()).await.unwrap();
    assert_eq!(pwd.stdout.trim(), std::fs::canonicalize(root.join("fk_c0ffee")).unwrap().to_str().unwrap());
    let run = machine.run("echo hello", RunOptions { deadline_ms: 5_000, ..Default::default() }).await.unwrap();
    assert_eq!(run.stdout.trim(), "hello");
    assert_eq!(
        machine.preview_url(7070).await.unwrap(),
        PreviewReach { url: "http://127.0.0.1:1/fk_c0ffee".to_string(), token: String::new(), expires_at: JsNumber(1.0) }
    );
    assert_eq!(machine.daemon_token_path(), Some(root.join("fk_c0ffee").join("token").to_str().unwrap()));
}
