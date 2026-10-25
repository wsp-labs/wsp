// SPDX-License-Identifier: AGPL-3.0-only
//! `SubSpawnCtx` over the fake host (sub-fixture.ts): a lead on this computer with Boat as its cloud, the lead's daemon
//! answering `git.status`, the seal of the image, the picks, and a fork that fails as told. Every call is kept by name;
//! the state beyond `FakeHost`'s fields lives in the thread's `World`, which `lead_on_this_computer` resets.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_engine::box_backend::BoxLimits;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{Machine, MachineBackend, MachineSpec};
use wsp_core::clock::Cancel;
use wsp_core::types::internal::{KindModule, SessionEntry, SubStartRecord, GONE_REASON, SUB_STARTS};
use wsp_core::types::wiring::{LiveWorkspace, StageReport, SubRecord, WorkspaceRecord};
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_life::SessionFile;
use wsp_machines::machines::sub_spawn::{ImageRecord, Packed, ProviderRow, SubSpawnArea, SubSpawnCtx};
use wsp_wire::views::golden_image::GoldenManifest;
use wsp_wire::views::session::ThreadCapWait;
use wsp_wire::views::workspace::{Caller, ProjectView, SeedChoice, SeedPlan};
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_words::exit::HostError;

use super::*;

pub const LEAD_WS: &str = "ws_lead";
pub const LEAD_THREAD: &str = "t_lead";
pub const SUB_WS: &str = "ws_sub";
pub const SUB_THREAD: &str = "t_sub";
pub const NAME: &str = "Fix the flaky upload test";
pub const FOLDER: &str = "/root/web";
pub const LEAD_FOLDER: &str = "/work/web";
/// The id every spawn is given, from the first bytes `random_hex` hands out.
pub const SPAWNED_ID: &str = "ws_0a1b2c3d";
pub const SPAWNED_AT: &str = "2025-10-09T08:53:20.000Z";
pub const REPO: &str = "https://github.com/acme/web.git";
pub const SEED_CHOICES_KEY: &str = "seed-choices";

/// One failure the fork gives a create: a box that runs nothing, one whose environment did not land, a cancelled
/// create. `replayed` is a box the provider answered a replayed key with, which its own cleanup leaves standing.
#[derive(Clone, Debug)]
pub struct Fault {
    pub kind: &'static str,
    pub replayed: bool,
}

pub fn fault(kind: &'static str) -> Fault {
    Fault { kind, replayed: false }
}

pub fn replayed(kind: &'static str) -> Fault {
    Fault { kind, replayed: true }
}

/// What the lead's folder reads as through its daemon (sub-fixture.ts `LeadGit`).
#[derive(Clone, Debug)]
pub struct LeadGit {
    pub head: String,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub uncommitted: usize,
    pub ignored: usize,
    pub counts_unknown: bool,
    pub default_branch: Option<String>,
}

/// The setup image on Boat (sub-fixture.ts `sealImage`): its head, its sign-ins and the agents its recipe carries.
#[derive(Clone, Debug)]
pub struct ImageWorld {
    pub head: u32,
    pub logins: Vec<(String, String)>,
    pub agents: Vec<String>,
}

/// One line a start reported: its stage, its words and what it said beside them.
pub type Report = (String, String, Option<Value>);

pub struct World {
    pub git: LeadGit,
    pub status_error: Option<String>,
    pub read_ms: f64,
    pub project: Value,
    pub lead_folder: String,
    pub rows: Vec<ProviderRow>,
    pub image: Option<ImageWorld>,
    pub broker_ready: bool,
    pub gh_signed_in: Arc<AtomicBool>,
    pub real_git: Arc<AtomicBool>,
    pub remote_heads: Arc<Mutex<Vec<String>>>,
    pub lead_remote_heads: Arc<Mutex<Vec<String>>>,
    /// The files git ignores in the lead's folder, as the picks menu lists them: path, bytes, kind.
    pub menu: Vec<(String, u64, String)>,
    pub left: Vec<String>,
    pub packs: Vec<Vec<String>>,
    pub spend_capped: bool,
    pub taken_names: Vec<String>,
    pub kept_size: Option<(f64, f64)>,
    pub held: Rc<Cell<i64>>,
    pub faults: VecDeque<Fault>,
    pub states: Vec<String>,
    pub daemon_road: bool,
    pub ping_fault: Option<String>,
    pub fork_ms: f64,
    pub reports: Rc<RefCell<Vec<Report>>>,
    pub local: Arc<FakeMachine>,
    pub lead_machine: Arc<FakeMachine>,
    pub broker_off: IndexSet<String>,
    pub off_at_fork: Vec<bool>,
    /// The record as stored at each `rows_sent`, and how many of them ran to their end.
    pub at_rows_sent: Vec<Option<Value>>,
    pub rows_sent_done: u32,
    /// The record as `enqueue` was handed it, as JS would write it.
    pub enqueued: Vec<String>,
    pub next_id: u32,
    /// What every box the fork makes answers: a command holding the words, and the reply.
    pub machine_answers: Vec<(String, Reply)>,
}

fn local_rules(m: &FakeMachine, gh: Arc<AtomicBool>, heads: Arc<Mutex<Vec<String>>>, real: Arc<AtomicBool>) {
    m.on(move |cmd| {
        if cmd.starts_with("gh auth token") {
            return Some(if gh.load(Ordering::SeqCst) { ok("") } else { exit(1, "no oauth token found for github.com") });
        }
        if cmd.contains("git -C") && real.load(Ordering::SeqCst) {
            return Some(sh(cmd));
        }
        if cmd.contains(" ls-remote --heads origin") {
            let lines: String = heads.lock().unwrap().iter().map(|h| format!("c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00\trefs/heads/{h}\n")).collect();
            return Some(ok(&lines));
        }
        None
    });
}

/// A command run by the real shell, as the lead's computer runs git.
fn sh(cmd: &str) -> Reply {
    let o = std::process::Command::new("sh").arg("-c").arg(cmd).output().expect("sh runs");
    Reply::Out { code: o.status.code().unwrap_or(-1) as i64, stdout: String::from_utf8_lossy(&o.stdout).into_owned(), stderr: String::from_utf8_lossy(&o.stderr).into_owned() }
}

impl World {
    fn new() -> World {
        let gh = Arc::new(AtomicBool::new(false));
        let real = Arc::new(AtomicBool::new(false));
        let heads = Arc::new(Mutex::new(Vec::new()));
        let lead_heads = Arc::new(Mutex::new(Vec::new()));
        let local = FakeMachine::new("local");
        local_rules(&local, gh.clone(), heads.clone(), real.clone());
        let lead_machine = FakeMachine::new("bx_lead");
        local_rules(&lead_machine, gh.clone(), lead_heads.clone(), real.clone());
        World {
            git: LeadGit { head: "feat/upload".into(), upstream: Some("origin/feat/upload".into()), ahead: 0, uncommitted: 0, ignored: 0, counts_unknown: false, default_branch: None },
            status_error: None,
            read_ms: 0.0,
            project: project_json("web", "here", LEAD_FOLDER, "git@github.com:acme/web.git"),
            lead_folder: LEAD_FOLDER.into(),
            rows: vec![
                ProviderRow { id: "box".into(), name: "Boat".into(), kind: "provider".into() },
                ProviderRow { id: "solari".into(), name: "Solari".into(), kind: "provider".into() },
            ],
            image: Some(ImageWorld { head: 1, logins: vec![("GitHub CLI login".into(), "copied".into())], agents: vec!["claude".into(), "codex".into()] }),
            broker_ready: false,
            gh_signed_in: gh,
            real_git: real,
            remote_heads: heads,
            lead_remote_heads: lead_heads,
            menu: vec![("CLAUDE.local.md".into(), 1100, "config".into()), ("local.properties".into(), 143, "config".into())],
            left: Vec::new(),
            packs: Vec::new(),
            spend_capped: false,
            taken_names: Vec::new(),
            kept_size: None,
            held: Rc::new(Cell::new(0)),
            faults: VecDeque::new(),
            states: Vec::new(),
            daemon_road: false,
            ping_fault: None,
            fork_ms: 0.0,
            reports: Rc::new(RefCell::new(Vec::new())),
            local,
            lead_machine,
            broker_off: IndexSet::new(),
            off_at_fork: Vec::new(),
            at_rows_sent: Vec::new(),
            rows_sent_done: 0,
            enqueued: Vec::new(),
            next_id: 0x0a1b2c3d,
            machine_answers: Vec::new(),
        }
    }
}

thread_local! {
    static WORLD: Rc<RefCell<World>> = Rc::new(RefCell::new(World::new()));
}

/// This test's world.
pub fn w() -> Rc<RefCell<World>> {
    WORLD.with(|w| w.clone())
}

fn reset() {
    *w().borrow_mut() = World::new();
}

pub fn project_json(name: &str, computer: &str, path: &str, remote: &str) -> Value {
    json!({
        "id": "pr_1", "name": name, "computer": computer, "source": { "kind": "folder", "path": path }, "path": path, "remote": remote,
        "defaultBranch": "main", "memoryKey": name, "memoryDir": "/tmp/memory", "createdAt": "2026-10-01T00:00:00.000Z"
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Fixtures.

/// A host with the lead's thread live on this computer, Boat as its cloud answering the plan of sub-fixture.ts `PAID`,
/// the image sealed with its GitHub login copied, and the area over it.
pub struct Fixture {
    pub host: Rc<FakeHost>,
    pub area: Rc<SubSpawnArea>,
    pub lead: Entry,
}

pub fn lead_on_this_computer() -> Fixture {
    reset();
    let host = FakeHost::new();
    *host.backend.limits.lock().unwrap() = Some(paid());
    let lead = host.put_lead(lead_record("local", "web"), w().borrow().local.clone());
    let area = SubSpawnArea::new(host.clone());
    Fixture { host, area, lead }
}

/// The lead on a computer the person added (`leadAndBox`): a box named lab-box, its folder there, reached over its own
/// machine, and a project whose computer is not this one.
pub fn lead_on_a_box() -> Fixture {
    reset();
    let host = FakeHost::new();
    *host.backend.limits.lock().unwrap() = Some(paid());
    {
        let world = w();
        let mut world = world.borrow_mut();
        world.project = project_json("lab-box", "hetzner", "/root/lab-box", "git@github.com:acme/lab.git");
        world.lead_folder = "/root/lab-box".into();
    }
    let machine = w().borrow().lead_machine.clone();
    let lead = host.put_lead(lead_record("place", "lab-box"), machine);
    let area = SubSpawnArea::new(host.clone());
    Fixture { host, area, lead }
}

pub fn lead_record(kind: &str, name: &str) -> Value {
    json!({
        "id": LEAD_WS, "name": name, "kind": kind, "machineId": "local", "phase": "running", "golden": "", "createdAt": "2026-10-12T05:00:00.000Z",
        "project": "pr_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }
    })
}

pub fn lead_caller() -> Caller {
    caller(LEAD_THREAD, LEAD_WS, LEAD_THREAD)
}

impl FakeHost {
    /// An entry for a lead, on a machine of its own that the cloud's backend knows nothing of.
    pub fn put_lead(&self, record: Value, machine: Arc<FakeMachine>) -> Entry {
        let record: WorkspaceRecord = serde_json::from_value(record).expect("the record reads as the TS writes it");
        let id = record.id.clone();
        let entry = live_entry(Rc::new(RefCell::new(record)), machine);
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    /// A queued record and the stand-in the queue holds it by: no machine, an empty id.
    pub fn put_absent(&self, record: Value) -> Rc<RefCell<WorkspaceRecord>> {
        let record: WorkspaceRecord = serde_json::from_value(record).expect("the record reads as the TS writes it");
        let id = record.id.clone();
        let cell = Rc::new(RefCell::new(record));
        let entry = live_entry(cell.clone(), FakeMachine::new(""));
        self.live.borrow_mut().insert(id, entry);
        cell
    }
    /// A released record and the dead stand-in its entry is.
    pub fn put_dead(&self, record: Value) -> Entry {
        let record: WorkspaceRecord = serde_json::from_value(record).expect("the record reads as the TS writes it");
        let id = record.id.clone();
        let entry = live_entry(Rc::new(RefCell::new(record)), FakeMachine::new(""));
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    /// The names of every call, in order.
    pub fn names(&self) -> Vec<String> {
        self.calls.borrow().iter().map(|(n, _)| n.clone()).collect()
    }
    /// The first position of a call by name.
    pub fn at(&self, name: &str) -> usize {
        self.names().iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} was never called"))
    }
    /// Stores the picks the person made for the lead's folder, as the seed verb writes them.
    pub fn pick(&self, files: &[&str]) {
        let key = w().borrow().project["path"].as_str().unwrap().to_string();
        self.store.borrow_mut().entry(SEED_CHOICES_KEY.to_string()).or_default().insert(key, json!({ "files": files, "memory": false, "commits": false, "remember": true }));
    }
    /// The stages every start reported so far.
    pub fn reported(&self) -> Vec<Report> {
        w().borrow().reports.borrow().clone()
    }
}

/// The lead's folder is this real checkout: its daemon reads it, its project is recorded there.
pub fn lead_folder_at(path: &str) {
    let world = w();
    let mut world = world.borrow_mut();
    world.lead_folder = path.to_string();
    world.project["path"] = json!(path);
    world.project["source"]["path"] = json!(path);
}

/// The lead's machine and this computer, both answering git as the shell on this computer does.
pub fn real_git() {
    w().borrow().real_git.store(true, Ordering::SeqCst);
}

/// Boat's start of the day: the sizes its plan sells as a refusal names them.
pub fn sizes_sold(names: &[&str]) -> Vec<wsp_words::sub_thread::SubSize> {
    names
        .iter()
        .map(|n| {
            let (cpu, mem, rate) = match *n {
                "small" => (2.0, 4096.0, 0.018),
                "default" => (4.0, 8192.0, 0.036),
                "large" => (8.0, 16384.0, 0.072),
                other => panic!("{other} is no class"),
            };
            wsp_words::sub_thread::SubSize { name: n.to_string(), cpu, mem_mb: mem, rate_usd_per_hour: rate }
        })
        .collect()
}

/// sub-fixture.ts `TRIAL`: the trial plan, which sells the two smaller classes and fewer starts.
pub fn trial() -> BoxLimits {
    use wsp_engine::box_backend::{BoxClassType, StartCount, StartWindows};
    BoxLimits {
        trial: true,
        plan_key: Some("trial".to_string()),
        plan_usd: None,
        machines_at_once: Some(2.0),
        active_boxes: None,
        starts: Some(StartWindows { minute: StartCount { used: 0.0, limit: 5.0 }, hour: StartCount { used: 0.0, limit: 25.0 }, day: StartCount { used: 0.0, limit: 75.0 } }),
        sizes: vec![BoxClassType::Small, BoxClassType::Default],
        can_start: true,
        start_blocked_reason: None,
        credit_balance_hours: None,
        billing_status: None,
    }
}

/// The archive the fake pack makes of these picks: each path, then as many bytes as the file holds.
pub fn tar_of(picks: &[(&str, u64)]) -> Vec<u8> {
    let mut tar = Vec::new();
    for (path, bytes) in picks {
        tar.extend_from_slice(path.as_bytes());
        tar.push(b'\n');
        tar.extend(std::iter::repeat(b'x').take(*bytes as usize));
    }
    tar
}

pub fn holds(bytes: &[u8], needle: &str) -> bool {
    let n = needle.as_bytes();
    bytes.windows(n.len()).any(|w| w == n)
}

/// A value as JavaScript's `JSON.stringify` writes it: keys in the order they stand, a whole number without its point.
pub fn js_text(v: &Value) -> String {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        Value::Array(a) => format!("[{}]", a.iter().map(js_text).collect::<Vec<_>>().join(",")),
        Value::Object(o) => format!("{{{}}}", o.iter().map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), js_text(v))).collect::<Vec<_>>().join(",")),
        other => serde_json::to_string(other).unwrap(),
    }
}

pub fn js_of<T: serde::Serialize>(v: &T) -> String {
    js_text(&serde_json::to_value(v).unwrap())
}

/// The words of a failure and its exit class, as a terminal reads it.
pub fn said_of(e: &AreaError) -> String {
    match e {
        AreaError::Engine(e) => e.to_string(),
        AreaError::Host(h) => h.message.clone(),
    }
}

pub fn host_of(e: AreaError) -> HostError {
    match e {
        AreaError::Host(h) => h,
        AreaError::Engine(e) => panic!("an engine error where a refusal was expected: {e}"),
    }
}

pub fn class_of(h: &HostError) -> &'static str {
    wsp_words::exit::exit_class_of(h).as_str()
}

/// The session file of Codex on a machine (sub-fixture.ts `CODEX_SESSION_FILE`).
pub fn codex_session_file(id: &str) -> SessionFile {
    SessionFile { folder: "/root/.codex/sessions".to_string(), name: format!("-{id}.jsonl"), depth: 3 }
}

fn seal(head: u32, logins: &[(String, String)]) -> Value {
    let logins: Vec<Value> = logins.iter().map(|(name, state)| json!({ "name": name, "state": state })).collect();
    let version = |n: u32| {
        json!({
            "version": n, "snapshotId": format!("wsp-host-default-v{n}"), "baseTemplate": "base", "setupSha": "abc", "createdAt": "2026-10-07T00:00:00.000Z",
            "smoke": { "cmd": "true", "exitCode": 0 }, "logins": logins, "noSecrets": true
        })
    };
    json!({ "head": head, "versions": (1..=head).map(version).collect::<Vec<_>>() })
}

// ---------------------------------------------------------------------------------------------------------------
// The context.

#[async_trait(?Send)]
impl SubSpawnCtx for FakeHost {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn remove_live(&self, id: &str) {
        self.call("remove_live", &[id]);
        self.live.borrow_mut().shift_remove(id);
    }
    fn is_absent_machine(&self, entry: &Entry) -> bool {
        entry.machine.id().is_empty()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_records.borrow().clone()
    }
    fn push_head(&self, thread_id: &str) {
        self.call("push_head", &[thread_id]);
    }
    async fn local_machine(&self) -> AreaResult<Arc<dyn Machine>> {
        self.call("local_machine", &[]);
        let m: Arc<dyn Machine> = w().borrow().local.clone();
        Ok(m)
    }
    fn provider_backend(&self, id: &str) -> Option<Arc<dyn MachineBackend>> {
        (id == "box").then(|| self.backend.clone() as Arc<dyn MachineBackend>)
    }
    async fn computer_rows(&self) -> AreaResult<Vec<ProviderRow>> {
        self.call("computer_rows", &[]);
        Ok(w().borrow().rows.clone())
    }
    async fn landing_place(&self, row_id: &str) -> AreaResult<Option<String>> {
        self.call("landing_place", &[row_id]);
        Ok((row_id == "box").then(|| "box".to_string()))
    }
    fn place_settings(&self, _place: &str) -> Option<Map<String, Value>> {
        let (cpu, mem) = w().borrow().kept_size?;
        let mut m = Map::new();
        m.insert("size".to_string(), json!({ "cpu": cpu, "memMb": mem }));
        Some(m)
    }
    fn delete_after_ms(&self, place: &str) -> Option<f64> {
        self.settings_delete_after.borrow().get(place).copied()
    }
    fn place_name(&self, computer: &str) -> String {
        computer.to_string()
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        record.place.clone()
    }
    fn backend_for(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        Ok(self.backend.clone())
    }
    fn name_given(&self, name: &str) -> String {
        name.to_string()
    }
    fn spawn_guard(&self, origin: Option<&Caller>) -> Result<Box<dyn FnOnce()>, HostError> {
        self.call("spawn_guard", &[&caller_words(origin)]);
        if let Some(Err(e)) = self.next("spawn_guard") {
            return Err(host_of(e));
        }
        let held = w().borrow().held.clone();
        held.set(held.get() + 1);
        Ok(Box::new(move || held.set(held.get() - 1)))
    }
    fn project_held(&self, id: &str) -> ProjectView {
        self.call("project_held", &[id]);
        serde_json::from_value(w().borrow().project.clone()).expect("the project reads as the TS writes it")
    }
    async fn copy_blocked(&self, lead: &Entry) -> AreaResult<()> {
        self.call("copy_blocked", &[&lead.id()]);
        Ok(())
    }
    async fn daemon_frame(&self, entry: &Entry, frame: Value) -> AreaResult<Value> {
        self.call("daemon_frame", &[&entry.id(), &frame.to_string()]);
        let (git, error, ms) = {
            let world = w();
            let world = world.borrow();
            (world.git.clone(), world.status_error.clone(), world.read_ms)
        };
        if ms > 0.0 {
            self.clock.advance(ms).await;
        }
        if let Some(said) = error {
            return Err(AreaError::Engine(engine_error(&said)));
        }
        if frame["op"] != "git.status" {
            return Ok(json!({ "id": 1, "ok": false, "error": format!("{} was not answered", frame["op"].as_str().unwrap_or("")) }));
        }
        let mut entries: Vec<Value> = (0..git.uncommitted).map(|i| json!({ "path": format!("f{i}"), "xy": " M" })).collect();
        entries.extend((0..git.ignored).map(|i| json!({ "path": format!("ignored{i}"), "xy": "!!" })));
        let mut branch = json!({ "oid": BASE_SHA, "head": git.head, "ahead": git.ahead, "behind": 0 });
        if let Some(up) = git.upstream {
            branch["upstream"] = json!(up);
        }
        let mut reply = json!({ "id": 1, "ok": true, "branch": branch, "entries": entries, "root": frame["cwd"] });
        if git.counts_unknown {
            reply["countsUnknown"] = json!(true);
        }
        if let Some(d) = git.default_branch {
            reply["defaultBranch"] = json!(d);
        }
        Ok(reply)
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        match &record.sub {
            Some(sub) => sub.folder.clone(),
            None => w().borrow().lead_folder.clone(),
        }
    }
    fn brokered(&self, kind: &str, place: Option<&str>) -> bool {
        self.call("brokered", &[kind, place.unwrap_or("")]);
        w().borrow().broker_ready
    }
    fn brokered_start(&self) -> bool {
        w().borrow().broker_ready
    }
    async fn repo_of_record(&self, remote: &str, project: &str, on: &str) -> AreaResult<Map<String, Value>> {
        self.call("repo_of_record", &[remote, project, on]);
        let mut m = Map::new();
        m.insert("repoId".to_string(), json!(4242));
        m.insert("owner".to_string(), json!("acme"));
        m.insert("name".to_string(), json!("web"));
        Ok(m)
    }
    fn free_branch_name(&self, wanted: &str, heads: Option<&str>) -> String {
        let mut taken: IndexSet<String> = IndexSet::new();
        for e in self.live.borrow().values() {
            let r = e.record.borrow();
            if let Some(b) = &r.branch_of_record {
                taken.insert(b.branch.clone());
            }
            if let Some(s) = &r.sub {
                taken.insert(s.branch.clone());
            }
        }
        for line in heads.unwrap_or("").split('\n') {
            if let Some(r) = line.split('\t').nth(1) {
                if let Some(h) = r.strip_prefix("refs/heads/") {
                    taken.insert(h.to_string());
                }
            }
        }
        let mut branch = wanted.to_string();
        let mut n = 2;
        while taken.contains(&branch) {
            branch = format!("{wanted}-{n}");
            n += 1;
        }
        branch
    }
    async fn copy_of(&self, place: &str, image: &str) -> AreaResult<Option<GoldenManifest>> {
        self.call("copy_of", &[place, image]);
        let image = w().borrow().image.clone();
        Ok(image.map(|i| serde_json::from_value(seal(i.head, &i.logins)).expect("the seal reads as the TS writes it")))
    }
    async fn record_of(&self, image: &str) -> AreaResult<Option<ImageRecord>> {
        self.call("record_of", &[image]);
        let image = w().borrow().image.clone();
        Ok(image.map(|i| ImageRecord { logins: Some(i.logins.clone()), agents: i.agents.iter().map(|a| (a.clone(), true)).collect() }))
    }
    async fn place_guard(&self, place: &str) -> AreaResult<()> {
        self.call("place_guard", &[place]);
        if let Some(Err(e)) = self.next("place_guard") {
            return Err(e);
        }
        if w().borrow().spend_capped {
            return Err(AreaError::Host(HostError::new(wsp_words::words::computer::spend_cap_refusal(place, 0.0, 0.0))));
        }
        Ok(())
    }
    async fn default_agent_of(&self, lead: &Entry) -> AreaResult<String> {
        self.call("default_agent_of", &[&lead.id()]);
        Ok("claude".to_string())
    }
    fn unbrokered_refusal(&self, harness: &str) -> Option<HostError> {
        self.call("unbrokered_refusal", &[harness]);
        (harness == "opencode").then(|| wsp_words::place_name::brokered_agent_refusal("OpenCode", &["Claude Code", "Codex"]))
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        w().borrow().taken_names.iter().any(|n| n == name).then(|| format!("a thread named {name} already exists"))
    }
    fn tree_of(&self, scope: &ThreadScope) -> IndexMap<String, Value> {
        let mut m = IndexMap::new();
        m.insert("parentThreadId".to_string(), json!(scope.thread_id));
        m.insert("rootThreadId".to_string(), json!(scope.root_thread_id));
        m
    }
    fn forking_add(&self, name: &str) {
        self.call("forking_add", &[name]);
    }
    fn forking_remove(&self, name: &str) {
        self.call("forking_remove", &[name]);
    }
    fn supersede_failed(&self, name: &str) {
        self.call("supersede_failed", &[name]);
    }
    async fn enqueue(&self, record: &Rc<RefCell<WorkspaceRecord>>, wait: Option<ThreadCapWait>) -> AreaResult<()> {
        let id = record.borrow().id.clone();
        self.call("enqueue", &[&id, if wait.is_some() { "wait" } else { "none" }]);
        w().borrow_mut().enqueued.push(js_of(&*record.borrow()));
        self.persist_now(&record.borrow());
        let entry = live_entry(record.clone(), FakeMachine::new(""));
        self.live.borrow_mut().insert(id, entry);
        Ok(())
    }
    fn queue_wait(&self, place: &str, limits: Option<&BoxLimits>) -> Option<ThreadCapWait> {
        self.call("queue_wait", &[place, if limits.is_some() { "limits" } else { "no limits" }]);
        None
    }
    fn view(&self, record: &WorkspaceRecord) -> Value {
        json!({ "id": record.id, "name": record.name, "phase": record.phase, "machineId": record.machine_id })
    }
    fn stage_reporter(&self, id: &str, name: &str, started_at: f64, asked_by: Option<Value>) -> StageReport {
        self.call("stage_reporter", &[id, name, &started_at.to_string(), &asked_by.map(|v| v.to_string()).unwrap_or_default()]);
        let reports = w().borrow().reports.clone();
        Rc::new(move |stage: &str, message: &str, said: Option<&Value>| reports.borrow_mut().push((stage.to_string(), message.to_string(), said.cloned())))
    }
    /// The create road's own part: the cloud is asked for a box from the record's image at its size, the box reports
    /// the states it moves through, and a fault still leaves the box it made.
    async fn fork(&self, record: &Rc<RefCell<WorkspaceRecord>>, on_machine: Box<dyn Fn(Arc<dyn Machine>)>, report: StageReport) -> AreaResult<()> {
        let (id, name, golden, size) = {
            let r = record.borrow();
            (r.id.clone(), r.name.clone(), r.golden.clone(), r.size)
        };
        self.call("fork", &[&id, &name]);
        let off = w().borrow().broker_off.contains(&id);
        w().borrow_mut().off_at_fork.push(off);
        let gate = self.stand_up_gate.borrow().clone();
        if let Some(g) = gate {
            let _ = g.acquire().await;
        }
        if let Some(Err(e)) = self.next("fork") {
            return Err(e);
        }
        let mut labels = Map::new();
        labels.insert(wsp_engine::labels::NAME_LABEL.to_string(), json!(name));
        let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox", "fromSnapshot": golden, "cpu": size.cpu, "memMb": size.mem_mb, "labels": labels }))
            .expect("the spec reads as the TS writes it");
        self.backend.create(spec, None, None).await.map_err(AreaError::Engine)?;
        let m = self.backend.machines.lock().unwrap().last().cloned().expect("the backend kept the box it made");
        let (states, daemon_road, fork_ms, fault, answers) = {
            let world = w();
            let mut world = world.borrow_mut();
            (world.states.clone(), world.daemon_road, world.fork_ms, world.faults.pop_front(), world.machine_answers.clone())
        };
        for (needle, reply) in answers {
            m.answer(&needle, reply);
        }
        if daemon_road {
            m.offers.lock().unwrap().daemon_answers = true;
        }
        for state in &states {
            report("fork-requested", &wsp_words::words::units::boat_box_state_line(&m.id, state), Some(&json!({ "box": { "id": m.id, "state": state } })));
        }
        if fork_ms > 0.0 {
            self.clock.advance(fork_ms).await;
        }
        if let Some(f) = fault {
            if !f.replayed {
                *m.killed.lock().unwrap() = true;
            }
            return Err(AreaError::Engine(match f.kind {
                "unusable" => EngineError::guest_unusable(&m.id, "Boat", &format!("{} is missing", wsp_engine::box_backend::BOX_WORK_DIR), 200),
                "env" => EngineError::EnvNotLanded { machine_id: m.id.clone(), exit_code: 124, detail: String::new() },
                _ => EngineError::CreateCancelled { machine_id: m.id.clone(), why: "no capacity".to_string(), line: wsp_words::words::units::boat_cancelled_line(Some(&name), Some("no capacity")) },
            }));
        }
        on_machine(m as Arc<dyn Machine>);
        Ok(())
    }
    fn attach(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Entry {
        let id = record.borrow().id.clone();
        self.call("attach", &[&id, machine.id()]);
        let entry = live_entry(record.clone(), machine);
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    fn attach_dead(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Entry {
        let id = record.borrow().id.clone();
        self.call("attach_dead", &[&id]);
        let entry = live_entry(record.clone(), FakeMachine::new(""));
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        self.call("ping_daemon", &[&entry.id()]);
        w().borrow().ping_fault.clone()
    }
    fn sync_daemon(&self, entry: &Entry) {
        self.call("sync_daemon", &[&entry.id()]);
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        self.call("persist", &[&record.id]);
        if let Some(Err(e)) = self.next("persist") {
            return Err(e);
        }
        self.persist_now(record);
        Ok(())
    }
    async fn emit_status_reachable(&self, entry: &Entry) -> AreaResult<()> {
        self.call("emit_status", &[&entry.id(), "reachable"]);
        Ok(())
    }
    async fn unfork(&self, entry: &Entry) -> AreaResult<()> {
        self.call("unfork", &[&entry.id(), entry.machine.id()]);
        if let Some(Err(e)) = self.next("unfork") {
            return Err(e);
        }
        entry.machine.kill().await.map_err(AreaError::Engine)
    }
    fn broker_lift(&self, workspace_id: &str) {
        self.call("broker_lift", &[workspace_id]);
        w().borrow_mut().broker_off.shift_remove(workspace_id);
    }
    /// A slow place for the row: it reads the computers before it goes out, and the refusal must wait for it.
    async fn rows_sent(&self) {
        self.call("rows_sent", &[]);
        let at = self.store.borrow().get("workspaces").and_then(|c| c.get(SUB_WS)).cloned();
        w().borrow_mut().at_rows_sent.push(at);
        for _ in 0..30 {
            tokio::task::yield_now().await;
        }
        w().borrow_mut().rows_sent_done += 1;
    }
    fn session_file(&self, _entry: &Entry, agent: &str, session_id: &str) -> Option<SessionFile> {
        match agent {
            "claude" => Some(super::life_ctx::claude_session_file(session_id)),
            "codex" => Some(codex_session_file(session_id)),
            _ => None,
        }
    }
    async fn get_seed_choice(&self, folder: &str) -> AreaResult<Option<SeedChoice>> {
        self.call("get_seed_choice", &[folder]);
        let held = self.store.borrow().get(SEED_CHOICES_KEY).and_then(|c| c.get(folder)).cloned();
        Ok(held.map(|v| serde_json::from_value(v).expect("the picks read as the seed verb writes them")))
    }
    async fn get_blob(&self, collection: &str, key: &str) -> AreaResult<Option<Vec<u8>>> {
        self.call("get_blob", &[collection, key]);
        Ok(self.blob(collection, key))
    }
    async fn delete_create(&self, key: &str) -> AreaResult<()> {
        self.call("delete_create", &[key]);
        Ok(())
    }
    fn seeds(&self) -> bool {
        true
    }
    async fn homes_here(&self) -> AreaResult<Value> {
        self.call("homes_here", &[]);
        Ok(json!({}))
    }
    async fn seed_plan(&self, folder: &str, _homes: &Value) -> AreaResult<SeedPlan> {
        self.call("seed_plan", &[folder]);
        let files: Vec<Value> = w().borrow().menu.iter().map(|(path, bytes, kind)| json!({ "path": path, "dir": false, "bytes": bytes, "kind": kind, "ticked": true })).collect();
        Ok(serde_json::from_value(json!({
            "source": folder, "remote": null, "branch": "feat/upload", "defaultBranch": "main", "unpushed": null, "uncommitted": 0,
            "memory": null, "remembered": false, "files": files
        }))
        .expect("the plan reads as the TS writes it"))
    }
    /// The archive of the files asked, as many bytes as they hold; a plan not marked for a provider is a failure.
    async fn seed_pack(&self, plan: &SeedPlan, files: &[String], _homes: &Value) -> AreaResult<Packed> {
        self.call("seed_pack", &[&files.join(",")]);
        assert_eq!(plan.provider, Some(true), "a sub-thread's plan is marked for a provider before it is packed");
        let world = w();
        let mut world = world.borrow_mut();
        world.packs.push(files.to_vec());
        let sized: Vec<(String, u64)> = files.iter().map(|f| (f.clone(), world.menu.iter().find(|(p, _, _)| p == f).map(|(_, b, _)| *b).expect("a pick the menu lists"))).collect();
        let picks: Vec<(&str, u64)> = sized.iter().map(|(p, b)| (p.as_str(), *b)).collect();
        Ok(Packed { tar: tar_of(&picks), files: files.len() as u64, bytes: sized.iter().map(|(_, b)| *b).sum(), left: world.left.clone() })
    }
    fn folder_named(&self, path: &str) -> String {
        path.to_string()
    }
    fn emit(&self, event: Value) {
        self.emit_now(event);
    }
    fn now(&self) -> f64 {
        self.clock.now()
    }
    fn random_hex(&self, bytes: usize) -> String {
        let n = {
            let world = w();
            let mut world = world.borrow_mut();
            world.next_id += 1;
            world.next_id - 1
        };
        format!("{n:08x}").chars().cycle().take(bytes * 2).collect()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        self.spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Records a start leaves.

/// The record a spawn writes and hands to the queue, as `spawn_sub` leaves it (sub-spawn.ts:223), with the spawn the
/// queue's stand-up reads: a lead on this computer, Boat at the small size, picks as `spawn` says.
pub fn spawned_record(id: &str, spawn: Value) -> Value {
    let mut r = sub_record(id, "running");
    r["machineId"] = json!("");
    r["createdAt"] = json!(SPAWNED_AT);
    r["sub"]["spawn"] = spawn;
    r
}

pub fn fresh_spawn(started_at: f64) -> Value {
    json!({ "startedAt": started_at, "branch": { "branch": "feat/upload", "sha": BASE_SHA, "ms": 0 }, "log": [] })
}

/// A sub-thread whose machine was deleted after its done, with its session kept (sub-resume.test.ts `released`).
pub fn released_record(id: &str) -> Value {
    let mut r = sub_record(id, "released");
    r["machineId"] = json!("");
    r["diskGone"] = json!("the disk was deleted at Boat");
    let sub = &mut r["sub"];
    sub["session"] = json!({ "agent": "claude", "id": "s2", "kept": 1_760_000_000_000u64, "bytes": 24, "path": "/root/.claude-cfg/projects/-root-web/s2.jsonl" });
    sub["result"] = json!({ "branch": BRANCH, "pushedSha": PUSHED_SHA, "commits": 3, "unpushed": 0, "changed": 0, "readAt": "2026-10-12T06:30:00.000Z" });
    sub["releasedAt"] = json!(1_759_999_000_000u64);
    sub["lost"] = json!("the machine was lost");
    sub["spawn"] = json!({ "startedAt": 1_759_990_000_000u64, "branch": { "branch": "feat/upload", "sha": BASE_SHA, "ms": 3 }, "machine": { "since": 1_759_990_000_100u64, "id": "bx_old", "states": [], "ms": 5 }, "readyAt": 1_759_990_001_000u64, "log": ["ready"] });
    r
}

pub const SESSION_BYTES: &[u8] = b"gzipped claude session";

/// A released sub-thread on a host holding its kept session: the entry, the area, and the thread it answers.
pub fn released_sub() -> (Fixture, Entry) {
    let f = lead_on_this_computer();
    f.host.pick(&["CLAUDE.local.md"]);
    f.host.thread_records.borrow_mut().push((SUB_THREAD.to_string(), SUB_WS.to_string()));
    f.host.blobs.borrow_mut().insert(("sub-sessions".to_string(), SUB_THREAD.to_string()), SESSION_BYTES.to_vec());
    let entry = f.host.put_dead(released_record(SUB_WS));
    (f, entry)
}

/// A queued spawn of the first start, the stand-in held by the queue: the record cell and its thread.
pub fn queued_sub(f: &Fixture, spawn: Value) -> Rc<RefCell<WorkspaceRecord>> {
    f.host.thread_records.borrow_mut().push((SUB_THREAD.to_string(), SUB_WS.to_string()));
    f.host.put_absent(spawned_record(SUB_WS, spawn))
}

pub fn lead_scope() -> ThreadScope {
    match lead_caller() {
        Caller::Scoped { by, .. } => by,
        Caller::Road(_) => unreachable!(),
    }
}
