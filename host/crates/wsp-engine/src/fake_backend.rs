// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/fake-backend.ts at a4033dc34. Spec: spec/fake_backend.md.
//!
//! A provider that answers out of memory, or out of one records file two host processes share, so a harness can
//! serve a fixture state whose workspaces are forks.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::cancel::Cancel;
use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::child_exec::{run_child, ChildOptions};
use crate::errors::{EngineError, RealRetryClock, Result, RetryClock};
use crate::machine::{
    not_on_this_road, BackendOffers, BackendPricing, BytesLanded, Capabilities, ExecOptions, ExecResult, Lifecycle, LifecycleBudgets,
    Machine, MachineBackend, MachineFacts, MachineKind, MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec,
    MachineState, OnLine, PreviewReach, RunOptions, Seen, Size, SnapshotOptions, SnapshotRow, SnapshotStoragePricing, Waiting,
    WireSnapshotStoragePricing,
};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::JsNumber;
use wsp_wire::views::activity::iso_time;
use wsp_wire::wire::capabilities::{MachineSizeOffer, PauseMode};
use wsp_words::js::to_fixed;

/// fake-backend.ts:75. What a road that reaches for the guest is told.
pub const FAKE_NO_GUEST: &str = "this is a stand-in provider for testing: its machines are records in memory, so nothing runs on them and nothing can be opened inside them";

/// fake-backend.ts:94 `shell`'s answer: how a Linux guest's command runs on the computer holding the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestShell {
    pub cmd: String,
    pub cwd: PathBuf,
    pub env: IndexMap<String, String>,
}

/// fake-backend.ts:83. A folder on the caller's own computer standing in for a machine's disk.
#[async_trait(?Send)]
pub trait FakeGuest {
    /// Where this machine's commands run and its daemon is rooted.
    fn folder(&self, machine_id: &str) -> PathBuf;
    /// The file this machine's daemon reads its token from.
    fn token_path(&self, machine_id: &str) -> String;
    /// The route to one port on this machine.
    async fn reach(&self, machine_id: &str, port: u16) -> Result<PreviewReach>;
    /// Optional: the guest's own answer for a command written for a Linux machine.
    fn shell(&self, _machine_id: &str, _cmd: &str) -> Option<GuestShell> {
        None
    }
    /// Whether the guest lands bytes (the TS's `putBytes` present).
    fn has_put_bytes(&self) -> bool {
        false
    }
    async fn put_bytes(&self, _machine_id: &str, _path: &str, _bytes: &[u8]) -> Result<()> {
        crate::machine::not_on_this_road()
    }
}

/// fake-backend.ts:126. What a reader is told when the file is there and is not records.
pub fn stand_in_records_unreadable(path: &str, why: &str) -> String {
    format!("{path} is not this stand-in provider's records: {why}")
}

/// fake-backend.ts:250.
#[derive(Clone, Default)]
pub struct FakeBackendOptions {
    /// The shape every machine takes when a create names none; the pricing's default size otherwise.
    pub size: Option<(f64, f64)>,
    /// A file this stand-in keeps its machines and snapshots in; this process alone when none is named.
    pub records: Option<PathBuf>,
    pub guest: Option<Arc<dyn FakeGuest>>,
}

/// fake-backend.ts:113. Where this stand-in's records live: a file read at every call and written at every change, or
/// this process alone. Held as the TS object, so a key the file carries beyond `machines` and `snapshots` keeps its place.
struct Records {
    file: Option<PathBuf>,
    memory: Mutex<Map<String, Value>>,
}

/// fake-backend.ts:108.
fn empty_held() -> Map<String, Value> {
    let mut held = Map::new();
    held.insert("machines".into(), json!({}));
    held.insert("snapshots".into(), json!([]));
    held
}

fn machines_of(held: &Map<String, Value>) -> Map<String, Value> {
    held.get("machines").and_then(Value::as_object).cloned().unwrap_or_default()
}

fn snapshots_of(held: &Map<String, Value>) -> Vec<Value> {
    held.get("snapshots").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn other(message: impl Into<String>) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

fn io(e: std::io::Error) -> EngineError {
    other(e.to_string())
}

impl Records {
    /// fake-backend.ts:128. A file that cannot be read is an empty fleet; one that is there and does not parse fails the
    /// read, and is never written over. JSON that is not an object reads as an empty fleet.
    fn read(&self) -> Result<Map<String, Value>> {
        let Some(path) = &self.file else { return Ok(self.memory.lock().unwrap().clone()) };
        let Ok(text) = std::fs::read_to_string(path) else { return Ok(empty_held()) };
        let mut held = empty_held();
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(parsed)) => held.extend(parsed),
            Ok(_) => {}
            Err(e) => return Err(other(stand_in_records_unreadable(&path.to_string_lossy(), &e.to_string()))),
        }
        Ok(held)
    }

    /// fake-backend.ts:146. Written beside the file and renamed over it, so a reader sees the records before a change or
    /// after it, never half of them.
    fn write(&self, held: Map<String, Value>) -> Result<()> {
        let Some(path) = &self.file else {
            *self.memory.lock().unwrap() = held;
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let next = PathBuf::from(format!("{}.next", path.display()));
        let text = serde_json::to_string_pretty(&Value::Object(held)).unwrap_or_default();
        std::fs::write(&next, format!("{text}\n")).map_err(io)?;
        std::fs::rename(&next, path).map_err(io)
    }
}

/// One machine's record as the TS writes it: `state, shape, labels`.
#[derive(Serialize, Deserialize)]
struct MachineRecord {
    state: MachineState,
    shape: MachineShape,
    #[serde(default)]
    labels: IndexMap<String, String>,
}

/// fake-backend.ts:156. One machine this provider holds.
pub struct FakeMachine {
    id: String,
    labels: IndexMap<String, String>,
    shape: MachineShape,
    seen: Seen,
    records: Arc<Records>,
    guest: Option<Arc<dyn FakeGuest>>,
    token_path: Option<String>,
}

impl FakeMachine {
    /// One command where a guest is wired, on the caller's own computer in this machine's folder; a refusal that names
    /// this stand-in where none is.
    async fn shell(&self, cmd: &str, timeout_ms: Option<u64>, on_line: Option<OnLine>) -> Result<ExecResult> {
        let Some(guest) = &self.guest else { return Err(other(FAKE_NO_GUEST)) };
        let at = guest.folder(&self.id);
        let run = guest.shell(&self.id, cmd).unwrap_or_else(|| GuestShell {
            cmd: cmd.to_string(),
            cwd: at.clone(),
            env: IndexMap::from([("HOME".to_string(), at.to_string_lossy().into_owned())]),
        });
        std::fs::create_dir_all(&run.cwd).map_err(io)?;
        let mut env: IndexMap<String, String> =
            std::env::vars_os().map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned())).collect();
        env.extend(run.env);
        let args = ["-c".to_string(), run.cmd];
        Ok(run_child("bash", &args, ChildOptions { cwd: Some(run.cwd), env: Some(env), timeout_ms, on_line, stdin: None }).await)
    }

    /// fake-backend.ts:203. Appends the snapshot to the records and writes them.
    fn record_snapshot(&self, name: &str) -> Result<String> {
        let row_id = format!("fksnap_{}", hex::encode(rand::random::<[u8; 6]>()));
        let mut held = self.records.read()?;
        let mut snapshots = snapshots_of(&held);
        let gib = 8u64 * 1024 * 1024 * 1024;
        snapshots.push(
            json!({ "id": row_id, "name": name, "sizeBytes": gib, "restoredBytes": gib, "createdAt": iso_time(RealRetryClock.now()) }),
        );
        held.insert("snapshots".into(), Value::Array(snapshots));
        self.records.write(held)?;
        Ok(row_id)
    }

    /// fake-backend.ts:210. A machine no longer recorded is left alone and nothing is written.
    fn move_to(&self, to: MachineState) -> Result<()> {
        let mut held = self.records.read()?;
        let mut machines = machines_of(&held);
        let Some(record) = machines.get_mut(&self.id) else { return Ok(()) };
        if let Some(record) = record.as_object_mut() {
            record.insert("state".into(), serde_json::to_value(to).unwrap_or_default());
        }
        held.insert("machines".into(), Value::Object(machines));
        self.records.write(held)
    }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        Some(&self.labels)
    }
    fn seen(&self) -> Option<&Seen> {
        Some(&self.seen)
    }
    fn daemon_token_path(&self) -> Option<&str> {
        self.token_path.as_deref()
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers {
            preview_url: self.guest.is_some(),
            put_bytes: self.guest.as_ref().is_some_and(|g| g.has_put_bytes()),
            ..MachineOffers::default()
        }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.shell(cmd, opts.timeout_ms, None).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        self.shell(script, Some(opts.deadline_ms), opts.on_line.clone()).await
    }
    async fn snapshot(&self, name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        self.record_snapshot(name)
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        self.move_to(MachineState::Paused)
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        self.move_to(MachineState::Running)
    }
    async fn kill(&self) -> Result<()> {
        self.move_to(MachineState::Gone)
    }
    async fn state(&self) -> Result<MachineState> {
        let held = self.records.read()?;
        let recorded = machines_of(&held).get(&self.id).and_then(|m| serde_json::from_value::<MachineState>(m["state"].clone()).ok());
        Ok(recorded.unwrap_or(self.seen.state))
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(other("this provider mints no signed download URL"))
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Err(other("this provider mints no signed upload URL"))
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        match &self.guest {
            Some(guest) => guest.reach(&self.id, port).await,
            None => not_on_this_road(),
        }
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], _timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        match &self.guest {
            Some(guest) if guest.has_put_bytes() => {
                guest.put_bytes(&self.id, path, bytes).await?;
                Ok(None)
            }
            _ => not_on_this_road(),
        }
    }
    async fn describe(&self) -> Result<MachineShape> {
        Ok(self.shape.clone())
    }
    async fn facts(&self) -> Result<MachineFacts> {
        Ok(MachineFacts { os: "Debian GNU/Linux 12".into(), uptime_ms: JsNumber(10_800_000.0), folder: "/root".into() })
    }
}

/// fake-backend.ts:259.
pub struct FakeBackend {
    records: Arc<Records>,
    guest: Option<Arc<dyn FakeGuest>>,
    size: (f64, f64),
}

/// fake-backend.ts:52. The sizes this provider offers.
const SIZES: [(f64, f64); 3] = [(2.0, 4096.0), (4.0, 8192.0), (8.0, 16384.0)];

/// fake-backend.ts:57. A made-up price, rounded to four places as `toFixed` rounds it.
fn rate_usd_per_hour(size: Size) -> f64 {
    let raw = size.cpu * 0.02 + (size.mem_mb / 1024.0) * 0.01;
    to_fixed(raw, 4).parse::<f64>().unwrap_or(f64::NAN)
}

/// fake-backend.ts:61.
struct FakePricing;

impl BackendPricing for FakePricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        rate_usd_per_hour(size)
    }
    fn default_size(&self) -> Size {
        Size { cpu: 4.0, mem_mb: 8192.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing {
                free_gb: JsNumber(10.0),
                usd_per_gb_month: JsNumber(0.15),
                billed_from: "2026-01-01".into(),
            },
            free_named: None,
            usd_per_named_month: None,
        }
    }
    fn builder_disk_gb(&self) -> Option<f64> {
        Some(20.0)
    }
}

/// fake-backend.ts:69. Instant everywhere: nothing waits.
struct FakeLifecycle;

#[async_trait(?Send)]
impl Lifecycle for FakeLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        LifecycleBudgets { wake_attempts: JsInt::from(1), daemon_answers_ms: JsNumber(1_000.0), resume_asks: None }
    }
}

/// `id(prefix)`: a prefix, an underscore and 12 hex digits.
fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", hex::encode(rand::random::<[u8; 6]>()))
}

impl FakeBackend {
    /// fake-backend.ts:269.
    pub fn new(opts: FakeBackendOptions) -> Self {
        Self {
            records: Arc::new(Records { file: opts.records, memory: Mutex::new(empty_held()) }),
            guest: opts.guest,
            size: opts.size.unwrap_or((4.0, 8192.0)),
        }
    }

    /// The fake's machine itself (the TS hands back its own class), for a caller that reads its seen state.
    pub async fn get_fake(&self, machine_id: &str) -> Result<Arc<FakeMachine>> {
        Ok(Arc::new(self.mint(machine_id, None)?))
    }

    /// fake-backend.ts:297. A known machine is used as recorded and nothing is written; an unknown one is recorded running
    /// with the spec's figures (0 kept, the size's where none is given) and written.
    fn mint(&self, machine_id: &str, spec: Option<&MachineSpec>) -> Result<FakeMachine> {
        let mut held = self.records.read()?;
        let mut machines = machines_of(&held);
        if !machines.contains_key(machine_id) {
            let figure = |given: Option<JsNumber>, default: f64| given.map_or(default, |x| x.0);
            let record = MachineRecord {
                state: MachineState::Running,
                shape: MachineShape {
                    cpu: Some(JsNumber(figure(spec.and_then(|s| s.cpu), self.size.0))),
                    mem_mb: Some(JsNumber(figure(spec.and_then(|s| s.mem_mb), self.size.1))),
                    disk_gb: Some(JsNumber(figure(spec.and_then(|s| s.disk_gb), 20.0))),
                    created_at: Some(iso_time(RealRetryClock.now())),
                    used_bytes: None,
                },
                labels: spec.and_then(|s| s.labels.clone()).unwrap_or_default(),
            };
            machines.insert(machine_id.to_string(), serde_json::to_value(&record).map_err(|e| other(e.to_string()))?);
            held.insert("machines".into(), Value::Object(machines.clone()));
            self.records.write(held)?;
        }
        let record: MachineRecord =
            serde_json::from_value(machines.get(machine_id).cloned().unwrap_or(Value::Null)).map_err(|e| other(e.to_string()))?;
        Ok(FakeMachine {
            id: machine_id.to_string(),
            labels: record.labels,
            seen: Seen { state: record.state, created_at: record.shape.created_at.clone(), paused_at: None },
            shape: record.shape,
            records: self.records.clone(),
            guest: self.guest.clone(),
            token_path: self.guest.as_ref().map(|g| g.token_path(machine_id)),
        })
    }
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: true,
            pause_mode: Some(PauseMode::Memory),
            replaces_machine: true,
            // A machine is reachable at a port only where a guest was wired.
            preview_urls: self.guest.is_some(),
            signed_urls: false,
            callback_relay: false,
            disk_snapshots: true,
            images: true,
            snapshots_any_life: true,
            snapshot_listing: true,
            templates: false,
            kept: false,
            copies: true,
            own_network: true,
            sizes: SIZES
                .iter()
                .map(|&(cpu, mem_mb)| MachineSizeOffer {
                    cpu: JsNumber(cpu),
                    mem_mb: JsNumber(mem_mb),
                    rate_usd_per_hour: JsNumber(rate_usd_per_hour(Size { cpu, mem_mb })),
                    name: None,
                })
                .collect(),
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FakePricing)
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        Some(Arc::new(FakeLifecycle))
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers { list_snapshots: true, ..BackendOffers::default() }
    }
    async fn create(&self, spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Ok(Arc::new(self.mint(&new_id("fk"), Some(&spec))?))
    }
    /// A machine this backend never minted is minted here in the state its id says: a fixture state names its
    /// machines before any process has held them.
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        Ok(Arc::new(self.mint(id, None)?))
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        let held = self.records.read()?;
        machines_of(&held)
            .into_iter()
            .map(|(id, record)| {
                let record: MachineRecord = serde_json::from_value(record).map_err(|e| other(e.to_string()))?;
                Ok(MachineListRow { id, state: record.state, labels: record.labels, size: None })
            })
            .collect()
    }
    async fn list_snapshots(&self, _ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        let held = self.records.read()?;
        snapshots_of(&held).into_iter().map(|row| serde_json::from_value(row).map_err(|e| other(e.to_string()))).collect()
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        let mut held = self.records.read()?;
        let kept: Vec<Value> = snapshots_of(&held).into_iter().filter(|row| row["id"].as_str() != Some(id)).collect();
        held.insert("snapshots".into(), Value::Array(kept));
        self.records.write(held)
    }
}
