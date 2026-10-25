// SPDX-License-Identifier: AGPL-3.0-only
//! Review of packet e1: orphans.rs (date_parse), key_fingerprint.rs and vault_cipher.rs against the TS. Every expected
//! value was printed by the TS under node 22.
#[path = "rest_common/mod.rs"]
mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_engine::cancel::Cancel;
use wsp_wire::wire::capabilities::Capabilities;

use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::{
    BackendPricing, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineListRow, MachineSpec, MachineState,
    RunOptions, Size, SnapshotOptions, SnapshotStoragePricing, Waiting, WireSnapshotStoragePricing,
};
use wsp_engine::orphans::{reap, ReapOptions, ReapResult};
use wsp_wire::types::JsNumber;

const NOW: i64 = 1_800_000_000_000;
const ME: &str = "h_me";

fn lab(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

struct StubPricing;
impl BackendPricing for StubPricing {
    fn rate_usd_per_hour(&self, s: Size) -> f64 {
        s.cpu * 0.035 + (s.mem_mb / 1024.0) * 0.01
    }
    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing {
                free_gb: JsNumber(10.0),
                usd_per_gb_month: JsNumber(0.05),
                billed_from: "2026-10-01".into(),
            },
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

struct StubMachine {
    id: String,
    log: Arc<Mutex<Vec<String>>>,
    kill_fails: Option<EngineError>,
}

#[async_trait(?Send)]
impl Machine for StubMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        Ok(common::ok(""))
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        Ok(common::ok(""))
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Ok("s".into())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        self.log.lock().unwrap().push(format!("kill {}", self.id));
        match &self.kill_fails {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Ok("u".into())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Ok("u".into())
    }
}

struct StubBackend {
    rows: Vec<MachineListRow>,
    log: Arc<Mutex<Vec<String>>>,
    get_fails: HashMap<String, EngineError>,
    kill_fails: HashMap<String, EngineError>,
}

impl StubBackend {
    fn new() -> Arc<Self> {
        Arc::new(StubBackend { rows: vec![], log: Arc::new(Mutex::new(Vec::new())), get_fails: HashMap::new(), kill_fails: HashMap::new() })
    }
}

#[async_trait(?Send)]
impl MachineBackend for StubBackend {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: true,
            pause_mode: None,
            replaces_machine: true,
            preview_urls: true,
            signed_urls: true,
            callback_relay: true,
            disk_snapshots: true,
            images: true,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            sizes: vec![],
            kept: false,
            copies: true,
            own_network: true,
            sizes_last: false,
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(StubPricing)
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Err(common::other("unused"))
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        self.log.lock().unwrap().push(format!("get {id}"));
        if let Some(e) = self.get_fails.get(id) {
            return Err(e.clone());
        }
        Ok(Arc::new(StubMachine { id: id.to_string(), log: self.log.clone(), kill_fails: self.kill_fails.get(id).cloned() }))
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        self.log.lock().unwrap().push("list".into());
        Ok(self.rows.clone())
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}

fn running(id: &str, labels: IndexMap<String, String>) -> MachineListRow {
    MachineListRow { id: id.to_string(), state: MachineState::Running, labels, size: None }
}

fn stop() -> wsp_engine::orphans::StopMachine {
    Arc::new(|m: Arc<dyn Machine>| Box::pin(async move { m.kill().await }))
}

async fn sweep(backend: &Arc<StubBackend>, rows: Vec<MachineListRow>, known: Vec<String>) -> ReapResult {
    sweep_at(backend, rows, known, NOW, None).await
}

async fn sweep_at(backend: &Arc<StubBackend>, rows: Vec<MachineListRow>, known: Vec<String>, now: i64, older: Option<i64>) -> ReapResult {
    reap(ReapOptions {
        backend: backend.clone(),
        listing: Some(rows),
        known_ids: Box::new(move || known.clone()),
        owner: ME.to_string(),
        stop: stop(),
        older_than_ms: older,
        now: Some(Box::new(move || now)),
    })
    .await
    .unwrap()
}

/// What Rust's `date_parse` read a createdAt label as, in epoch ms: an own machine's age is `now - parsed`, in the
/// result whether it is killed or spared; no age means NaN.
async fn rust_date(label: &str) -> Option<i64> {
    let backend = StubBackend::new();
    let res = sweep(&backend, vec![running("m", lab(&[("wsp", "1"), ("wsp-owner", ME), ("createdAt", label)]))], vec![]).await;
    let age = res.reaped.first().map(|r| r.age_ms).or_else(|| res.spared.first().map(|s| s.age_ms)).expect("the row is reaped or spared");
    age.map(|a| NOW - a)
}

// ---- date_parse (orphans.rs:20), reached through `reap`; every value below is what node's Date.parse printed

async fn misreads(cases: &[(&str, Option<i64>)]) -> Vec<String> {
    let mut bad = Vec::new();
    for (input, want) in cases {
        let got = rust_date(input).await;
        if got != *want {
            bad.push(format!("{input:?}: node {want:?}, rust {got:?}"));
        }
    }
    bad
}

#[tokio::test]
async fn hour_24_is_the_next_midnight_and_a_day_past_the_months_end_rolls_over() {
    let bad = misreads(&[
        ("2026-01-01T24:00:00Z", Some(1767312000000)),
        ("2026-01-01T24:00:00.000Z", Some(1767312000000)),
        ("2026-02-30T00:00:00Z", Some(1772409600000)),
        ("2026-02-29T00:00:00Z", Some(1772323200000)),
        ("2026-02-30", Some(1772409600000)),
        ("2026-04-31", Some(1777593600000)),
    ])
    .await;
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[tokio::test]
async fn a_year_past_9999_is_a_time() {
    let bad = misreads(&[
        ("+010000-01-01T00:00:00Z", Some(253402300800000)),
        ("9999-12-31T23:59:59.999Z", Some(253402300799999)),
        ("+275760-09-13T00:00:00.000Z", Some(8640000000000000)),
        ("-271821-04-20T00:00:00.000Z", Some(-8640000000000000)),
    ])
    .await;
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[tokio::test]
async fn forms_node_reads_no_time_from_are_no_age_here_too() {
    let bad = misreads(&[
        ("2026-01-01T23:59:60Z", None),
        ("2026-01-01T00:00:00+05", None),
        ("2026-01-01T00Z", None),
        ("2026-01-01T00:00:00Z[UTC]", None),
        ("2026-01-01T00:00:00+05:30[Asia/Kolkata]", None),
        ("2026-01-01T00:00:00+24:00", None),
        ("2026-01-01T00:00:00,123Z", None),
        ("20260101T000000Z", None),
        ("20260101", None),
    ])
    .await;
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[tokio::test]
async fn a_fraction_before_1970_is_floored_to_the_millisecond_as_node_floors_it() {
    let bad = misreads(&[("1969-12-31T12:34:56.123456Z", Some(-41103877)), ("0001-01-01T12:34:56.123456Z", Some(-62135551503877))]).await;
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// ---- vault_cipher: sealed files the TS probe built, opened by node with the oracle key in tests/fixtures/oracle/vault.json;
// (name, sealed file base64, node's plain base64, node's error)
const VAULT_CASES: &[(&str, &str, Option<&str>, &str)] = &[
    ("nonce16", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiL0JoUlVTOEgrb09Sc2lxZUI0aTl1QT09Iiwibm9uY2UiOiJScUZRWXdoYnNzejdSbERleW9tYVVRPT0iLCJlcGhlbWVyYWwiOiJNQ293QlFZREsyVnVBeUVBbDAwWTlYQmZuckpTNE1QNUtjYWVjSmdjU2ZpVFRuSy9rYVJ1RW9IR3cwQT0iLCJpbWFnZSI6eyJuYW1lIjoiZGVmYXVsdCIsInZlcnNpb24iOjEsImhhc2giOiJhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhIiwicmVjaXBlSGFzaCI6InJoIiwibG9naW5zIjpbeyJuYW1lIjoiY29kZXgiLCJzdGF0ZSI6InNpZ25lZC1pbiJ9XSwic2VhbGVkQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoiLCJzZWFsZWRGcm9tIjoidGhpcy1tYWMiLCJ2YXVsdCI6eyJzaGEyNTYiOiJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwiYnl0ZXMiOjEyOCwicGF0aHMiOjIsInRha2VuQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoifX19CmBaXkEcN0ER/NDFcosEuhN5KADMQR7xhyuTIw==", Some("aGVsbG8gdmF1bHQ="), ""),
    ("nonce1", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiSXRzNWpXWTdESlZrakdNaUdxbG8wQT09Iiwibm9uY2UiOiJsdz09IiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQWpjeDF1WFJkNzlxNXh2SGpPK3BnWTIreXpCdGxUWlZYNFlZTWZGZG0zbkE9IiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQo5PGVVo4fUj2FpXuWRMo2i90eSDwPnQskdK4M=", Some("aGVsbG8gdmF1bHQ="), ""),
    ("nonce13", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiUTRFSjVhVHdIcmtDM2VURWpudTN1QT09Iiwibm9uY2UiOiI3R1hZa0w3YzZIUk93NVZmN1E9PSIsImVwaGVtZXJhbCI6Ik1Db3dCUVlESzJWdUF5RUF3aTdxYUxQcnBmaSttM3dIRDdtcDZ0UHZNQmdxRE1sVklIUHlyQ2RRKzBVPSIsImltYWdlIjp7Im5hbWUiOiJkZWZhdWx0IiwidmVyc2lvbiI6MSwiaGFzaCI6ImFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWEiLCJyZWNpcGVIYXNoIjoicmgiLCJsb2dpbnMiOlt7Im5hbWUiOiJjb2RleCIsInN0YXRlIjoic2lnbmVkLWluIn1dLCJzZWFsZWRBdCI6IjIwMjYtMDktMTJUMDA6MDA6MDAuMDAwWiIsInNlYWxlZEZyb20iOiJ0aGlzLW1hYyIsInZhdWx0Ijp7InNoYTI1NiI6ImJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmIiLCJieXRlcyI6MTI4LCJwYXRocyI6MiwidGFrZW5BdCI6IjIwMjYtMDktMTJUMDA6MDA6MDAuMDAwWiJ9fX0Kwk+Zv4iMN+gV1bcbONSv0fQJ9UidnteHIEZ/", Some("aGVsbG8gdmF1bHQ="), ""),
    ("nonce0_text", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiZmJqZnZzZE56WEtyT1B3Sno4eGNyUT09Iiwibm9uY2UiOiIiLCJlcGhlbWVyYWwiOiJNQ293QlFZREsyVnVBeUVBeU9NOGx0bGt3anppejNsd09IYlNzZHgvMDQxS1hDR05pblVtcFlwei9Wcz0iLCJpbWFnZSI6eyJuYW1lIjoiZGVmYXVsdCIsInZlcnNpb24iOjEsImhhc2giOiJhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhIiwicmVjaXBlSGFzaCI6InJoIiwibG9naW5zIjpbeyJuYW1lIjoiY29kZXgiLCJzdGF0ZSI6InNpZ25lZC1pbiJ9XSwic2VhbGVkQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoiLCJzZWFsZWRGcm9tIjoidGhpcy1tYWMiLCJ2YXVsdCI6eyJzaGEyNTYiOiJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwiYnl0ZXMiOjEyOCwicGF0aHMiOjIsInRha2VuQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoifX19CkVvuqMkespbJWifE0gihO6Nt+wEBt9OgMDSCQ==", None, "Invalid initialization vector"),
    ("eph_nopad", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoicUFXRzlLQ0Z0a3BETGpQK2VZeW1Ldz09Iiwibm9uY2UiOiIxRFAybUpIYkl1OXl4bW0yIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQVFKNzRUYjdoTW9kcTk5VFo2dUdPUlpJRkIzTDg4Z0tCcHVyL2RvODRZMzAiLCJpbWFnZSI6eyJuYW1lIjoiZGVmYXVsdCIsInZlcnNpb24iOjEsImhhc2giOiJhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhIiwicmVjaXBlSGFzaCI6InJoIiwibG9naW5zIjpbeyJuYW1lIjoiY29kZXgiLCJzdGF0ZSI6InNpZ25lZC1pbiJ9XSwic2VhbGVkQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoiLCJzZWFsZWRGcm9tIjoidGhpcy1tYWMiLCJ2YXVsdCI6eyJzaGEyNTYiOiJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwiYnl0ZXMiOjEyOCwicGF0aHMiOjIsInRha2VuQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoifX19CkYxcQ6ai4MpC1pGAfgkXHA05YriY+UIcqXVDA==", Some("aGVsbG8gdmF1bHQ="), ""),
    ("salt_nopad", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiRTROdjZFT3ZCZ2tHMnlHMTFNQzQ4QSIsIm5vbmNlIjoicWt1YUNsQWxiRlA0TTRnRCIsImVwaGVtZXJhbCI6Ik1Db3dCUVlESzJWdUF5RUEzdWwrVWhQZHhDY0hWL0JTcWZHUEVxakVkTTZORlM3SVc5ZlN0RlhnalhFPSIsImltYWdlIjp7Im5hbWUiOiJkZWZhdWx0IiwidmVyc2lvbiI6MSwiaGFzaCI6ImFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWEiLCJyZWNpcGVIYXNoIjoicmgiLCJsb2dpbnMiOlt7Im5hbWUiOiJjb2RleCIsInN0YXRlIjoic2lnbmVkLWluIn1dLCJzZWFsZWRBdCI6IjIwMjYtMDktMTJUMDA6MDA6MDAuMDAwWiIsInNlYWxlZEZyb20iOiJ0aGlzLW1hYyIsInZhdWx0Ijp7InNoYTI1NiI6ImJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmIiLCJieXRlcyI6MTI4LCJwYXRocyI6MiwidGFrZW5BdCI6IjIwMjYtMDktMTJUMDA6MDA6MDAuMDAwWiJ9fX0KfzhHefBCepoCZsU8rkG3SU1fuNRfg3C1DosX", Some("aGVsbG8gdmF1bHQ="), ""),
    ("nonce_nopad_12", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiWk5pTVEyai9uLzZxT3RFVFpyd3RsQT09Iiwibm9uY2UiOiJSTjBGUmdabWs4RWhFRmI0IiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQWcvL28wcnhVeFY1WDY3Y2htMHdlVHlrdVpzTm9jQXkzSk1lby9GTHQ1eXc9IiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQpBAdxg4iwwd+0InpvtMYZw8eyf6+9u2mmwSo8=", Some("aGVsbG8gdmF1bHQ="), ""),
    ("eph_ws", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiWTdEWnEwSGZLU3BCYmZyTER4S3l1UT09Iiwibm9uY2UiOiJMa0tETWllM0tVSmd2MnVqIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlxuVnVBeUVBRlV0NUxzVC9SNDNySSszTWw0WTFHU3kzVjFKbWM2VExwZ2NRZFNwRGhncz0iLCJpbWFnZSI6eyJuYW1lIjoiZGVmYXVsdCIsInZlcnNpb24iOjEsImhhc2giOiJhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhIiwicmVjaXBlSGFzaCI6InJoIiwibG9naW5zIjpbeyJuYW1lIjoiY29kZXgiLCJzdGF0ZSI6InNpZ25lZC1pbiJ9XSwic2VhbGVkQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoiLCJzZWFsZWRGcm9tIjoidGhpcy1tYWMiLCJ2YXVsdCI6eyJzaGEyNTYiOiJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwiYnl0ZXMiOjEyOCwicGF0aHMiOjIsInRha2VuQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoifX19CnfZOchTm/oThAqWki3QJJgkySosTAJMmCCAJA==", Some("aGVsbG8gdmF1bHQ="), ""),
    ("eph_urlsafe", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiSDZzZTdoME8yRHBaWTEzTFpnOVl6UT09Iiwibm9uY2UiOiI1VWJsVmtSdjY2UnM2SFZpIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQUd0U0djQWNKZVBXU2NsZ25fQ0dQU29vekRSWUNRMXl2NEVnTUpjdHFheVE9IiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQomNCUEnT5s4Lr5gJvBs1W2l/C7KJEPzDv5tgk=", Some("aGVsbG8gdmF1bHQ="), ""),
    ("eph_zero_point", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiVlJ6WFlPMGxHQXhibUtCazl0TzhqUT09Iiwibm9uY2UiOiIyc1Jza09zNEpuVDgzem9vIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE9IiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQrzYivk0FmIh5yRiS3SnrleZn6wj95SmMwttHk=", None, "error:1C8000A4:Provider routines::failed during derivation"),
    ("eph_one_point", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiR1VRUXNGUkRjWTljUDFXZGxvWU5LUT09Iiwibm9uY2UiOiJ4NmFzN0VDeGFTWjl2bmFzIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQUFRQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE9IiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQpNqBZd1YQGwEnkChAQC2kD+ejBUoJKlLL36Hs=", None, "error:1C8000A4:Provider routines::failed during derivation"),
    ("eph_ed25519", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiV2I0RCtLSlFjQnhvZE1mM3h5VDdzZz09Iiwibm9uY2UiOiJseGtBUjBrM2xLcW9ZVGdkIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ3QXlFQVR0N2JUMmtzbk15MHJOZXNLZVFvWXlMS3VmY3NqczdVUjVxODZNUXZFMlE9IiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQrhsiWbfO/p/QbDhjZElsJWFwXnPEYhmZi+8GY=", None, "Incompatible key types for Diffie-Hellman: x25519 and ed25519"),
    ("eph_trailing_byte", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiQ2hkLytoaDd1SmFLS2xtVTc5NGtpZz09Iiwibm9uY2UiOiJPeXB3NklwRzZJMlFxd2kvIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQWdPYldkK0lWemNZU2dSVlpoM0k1TjdNVldCK21WNXBlYkxUQXMwSHdMVjBBIiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQpQbbRfoJpSnJBHR907hgf7YbQoEEOV1+BEY2M=", Some("aGVsbG8gdmF1bHQ="), ""),
    ("eph_short", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiSExyNXA2dnZhL3d3VFFoWGhXZGMrUT09Iiwibm9uY2UiOiJMWE01WjlKNFk2Z1NRc3RWIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQUJ3Y0hCd2NIQndjSEJ3Y0hCd2NIQndjSEJ3Y0hCd2NIQndjSEJ3Y0hCdz09IiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQrjIoGql2g/B1cJc51dk1YfT7pD+RPEWCB04qw=", None, "Failed to read asymmetric key"),
    ("eph_empty", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoicnQxQmx2UWpoK2N2TExzZVdJNG1tQT09Iiwibm9uY2UiOiJkS0NYbDdrQ2dRV05MdlZkIiwiZXBoZW1lcmFsIjoiIiwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQoH5fXXe+aIczy2SJb902bo6cCd17r4HD282bs=", None, "Failed to read asymmetric key"),
    ("eph_garbage_text", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiSUdRdFpzT2pPY2dSVHd6a1d0YTlpUT09Iiwibm9uY2UiOiJOU0o4ejcrTUtYVWNWWmZ4IiwiZXBoZW1lcmFsIjoiISEhISIsImltYWdlIjp7Im5hbWUiOiJkZWZhdWx0IiwidmVyc2lvbiI6MSwiaGFzaCI6ImFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWEiLCJyZWNpcGVIYXNoIjoicmgiLCJsb2dpbnMiOlt7Im5hbWUiOiJjb2RleCIsInN0YXRlIjoic2lnbmVkLWluIn1dLCJzZWFsZWRBdCI6IjIwMjYtMDktMTJUMDA6MDA6MDAuMDAwWiIsInNlYWxlZEZyb20iOiJ0aGlzLW1hYyIsInZhdWx0Ijp7InNoYTI1NiI6ImJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmIiLCJieXRlcyI6MTI4LCJwYXRocyI6MiwidGFrZW5BdCI6IjIwMjYtMDktMTJUMDA6MDA6MDAuMDAwWiJ9fX0KFhTvYqSd5nDK8lBnhWFeU5zBsuFs8yv/gh6E", None, "Failed to read asymmetric key"),
    ("invalid_utf8_in_image_name", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiUGRaLzVQL0JvS1JrR2M0VHExRW1oQT09Iiwibm9uY2UiOiJEak9ZZXRlUDlmR1NMNGN2IiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQXM1R1FRZitaOStUejlLa1psaGpjNGFmaHZ4bGVHRWR5WDBXY0dsVjF0bEE9IiwiaW1hZ2UiOnsibmFtZSI6Im7/bSIsInZlcnNpb24iOjEsImhhc2giOiJhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhIiwicmVjaXBlSGFzaCI6InJoIiwibG9naW5zIjpbeyJuYW1lIjoiY29kZXgiLCJzdGF0ZSI6InNpZ25lZC1pbiJ9XSwic2VhbGVkQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoiLCJzZWFsZWRGcm9tIjoidGhpcy1tYWMiLCJ2YXVsdCI6eyJzaGEyNTYiOiJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwiYnl0ZXMiOjEyOCwicGF0aHMiOjIsInRha2VuQXQiOiIyMDI2LTA5LTEyVDAwOjAwOjAwLjAwMFoifX19Cpqskgszu0MvPkv7y1AaSYnwds+J5TpO+MwwGA==", Some("aGVsbG8gdmF1bHQ="), ""),
    ("deep_unknown_key", "eyJmb3JtYXQiOiJ3c3AtdmF1bHQtMSIsImNpcGhlciI6ImFlcy0yNTYtZ2NtIiwidG8iOiJrZXkiLCJzYWx0IjoiaEFIYzVPUGRjVUdNcDNoM0pibk8wQT09Iiwibm9uY2UiOiJjYUNKdDBCYzNZVGxxM0FRIiwiZXBoZW1lcmFsIjoiTUNvd0JRWURLMlZ1QXlFQXhrT2MxWGlwdUlOV0pHbHNTcDE1VU5jbmg2MU1xUkpzZ3djU1RRektCV009IiwieCI6W1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tbW1tdXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXSwiaW1hZ2UiOnsibmFtZSI6ImRlZmF1bHQiLCJ2ZXJzaW9uIjoxLCJoYXNoIjoiYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYSIsInJlY2lwZUhhc2giOiJyaCIsImxvZ2lucyI6W3sibmFtZSI6ImNvZGV4Iiwic3RhdGUiOiJzaWduZWQtaW4ifV0sInNlYWxlZEF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIiwic2VhbGVkRnJvbSI6InRoaXMtbWFjIiwidmF1bHQiOnsic2hhMjU2IjoiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYiIsImJ5dGVzIjoxMjgsInBhdGhzIjoyLCJ0YWtlbkF0IjoiMjAyNi0wOS0xMlQwMDowMDowMC4wMDBaIn19fQr+CSrvM/e1XqloLajM/vz3fR+bzqfM80Ib08s=", Some("aGVsbG8gdmF1bHQ="), ""),
];

fn oracle_private_key() -> [u8; 32] {
    use base64::Engine;
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle/vault.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    base64::engine::general_purpose::STANDARD.decode(v["privateKey"].as_str().unwrap()).unwrap().try_into().unwrap()
}

/// Rust's answer on one probe file: the plain base64, or the error's words.
fn rust_open(name: &str) -> std::result::Result<String, String> {
    use base64::Engine;
    let (_, sealed, _, _) = VAULT_CASES.iter().find(|c| c.0 == name).unwrap();
    let bytes = base64::engine::general_purpose::STANDARD.decode(sealed).unwrap();
    wsp_engine::vault_cipher::open(&bytes, &wsp_engine::vault_cipher::VaultKey::Private(oracle_private_key()))
        .map(|o| base64::engine::general_purpose::STANDARD.encode(o.plain))
        .map_err(|e| e.to_string())
}

fn node_opens(name: &str) -> String {
    VAULT_CASES.iter().find(|c| c.0 == name).unwrap().2.expect("node opens this file").to_string()
}

#[test]
fn an_ephemeral_key_that_is_a_low_order_point_fails_the_derivation_as_node_does() {
    for name in ["eph_zero_point", "eph_one_point"] {
        let got = rust_open(name);
        assert!(got.is_err(), "{name}: node refuses with its derivation error, rust answered {got:?}");
    }
}

#[test]
fn a_nonce_of_another_length_than_twelve_opens_as_node_opens_it() {
    for name in ["nonce16", "nonce1", "nonce13"] {
        assert_eq!(rust_open(name), Ok(node_opens(name)), "{name}");
    }
}

#[test]
fn header_values_in_unpadded_url_safe_or_spaced_base64_open_as_node_reads_them() {
    for name in ["eph_nopad", "salt_nopad", "nonce_nopad_12", "eph_ws", "eph_urlsafe"] {
        assert_eq!(rust_open(name), Ok(node_opens(name)), "{name}");
    }
}

#[test]
fn an_ephemeral_der_with_a_trailing_byte_opens_as_node_reads_it() {
    assert_eq!(rust_open("eph_trailing_byte"), Ok(node_opens("eph_trailing_byte")));
}

#[test]
fn a_header_with_bytes_that_are_not_utf8_opens_as_node_reads_it_with_replacement_characters() {
    assert_eq!(rust_open("invalid_utf8_in_image_name"), Ok(node_opens("invalid_utf8_in_image_name")));
}

#[test]
fn a_header_with_a_deeply_nested_unknown_key_opens_as_node_reads_it() {
    {
        let name = "deep_unknown_key";
        assert_eq!(rust_open(name), Ok(node_opens(name)), "{name}");
    }
}

#[test]
fn an_ephemeral_key_node_cannot_read_is_refused_in_node_words() {
    assert_eq!(rust_open("eph_empty"), Err("Failed to read asymmetric key".to_string()));
    assert_eq!(rust_open("eph_short"), Err("Failed to read asymmetric key".to_string()));
    assert_eq!(rust_open("eph_garbage_text"), Err("Failed to read asymmetric key".to_string()));
    assert_eq!(rust_open("eph_ed25519"), Err("Incompatible key types for Diffie-Hellman: x25519 and ed25519".to_string()));
    assert_eq!(rust_open("nonce0_text"), Err("Invalid initialization vector".to_string()));
}

// ---- key_fingerprint

#[test]
fn a_key_string_with_utf16_units_past_ff_reads_their_low_byte_as_node_does() {
    use wsp_engine::key_fingerprint::key_fingerprint;
    // node's Buffer.from(s, "base64") on a two-byte string decodes each UTF-16 unit by its low byte.
    for (s, want) in [
        ("\u{141}A\u{141}", "SHA256:lqKW0iTyhcZ77pPDD4owkVfw2qNdxbh+QQt4YwoJz8c"),
        ("J\u{141}", "SHA256:CfyWCC00wt/BKV2SBzteodyO+NqV8U397QEf+5bT5Us"),
        ("\u{1F600}a0*=-a-9D!_\u{141}\rU", "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU"),
        ("+\u{141}=", "SHA256:5Zao5cSd0gp5vQwafdGQpnRQfREpeFdG/MHF5kzYQWw"),
    ] {
        assert_eq!(key_fingerprint(s), want, "{s:?}");
    }
}
