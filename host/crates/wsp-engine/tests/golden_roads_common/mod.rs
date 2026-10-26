// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts:34-185: `recordingBackend`, `stageRecorder`, the fetch stubs and the constants the golden tests share.
//! A backend whose machines answer every call from a script and record each one in order, so a test asserts the exact
//! sequence a seal, a prepare or an update drove them through.
#![allow(dead_code, clippy::type_complexity)]
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_engine::errors::{classify, EngineError, ErrorKind, Result, RetryClock, WspError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::golden::StageFn;
use wsp_engine::machine::{
    BackendOffers, BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife,
    MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, RunOptions, Seen, Size, SnapshotOptions, SnapshotProgress,
    SnapshotStoragePricing, TemplateRow, Waiting, WireSnapshotStoragePricing,
};
use wsp_wire::views::golden_image::{GoldenStage, GoldenStep};

/// golden.test.ts:176 `REACH_OK`: what a machine that still serves answers the reach check with.
pub fn reach_ok() -> ExecResult {
    exec(0, "ok\n", "")
}

pub fn exec(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: code.into(), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

pub fn ok() -> ExecResult {
    exec(0, "", "")
}

/// golden.test.ts:174 `FAST_KILL`.
pub fn fast_kill() -> wsp_engine::gone::KillConfirm {
    wsp_engine::gone::KillConfirm { grace_ms: Some(20), poll_ms: Some(1) }
}

/// The disk read `df -Pk /root | awk 'NR==2{print $4}'` of the tools loop.
pub const FREE_KB_CMD: &str = "df -Pk /root | awk 'NR==2{print $4}'";

pub fn run<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(f)
}

/// A clock whose sleeps cost nothing and move its own time, from a fixed epoch millisecond.
pub struct Manual {
    pub at: Cell<i64>,
    pub slept: RefCell<Vec<u64>>,
}

impl Manual {
    pub fn new() -> Rc<Self> {
        Rc::new(Manual { at: Cell::new(1_790_000_000_000), slept: RefCell::new(Vec::new()) })
    }
}

#[async_trait(?Send)]
impl RetryClock for Manual {
    fn now(&self) -> i64 {
        self.at.get()
    }
    async fn sleep(&self, ms: u64) {
        self.slept.borrow_mut().push(ms);
        self.at.set(self.at.get() + ms as i64);
    }
    fn random(&self) -> f64 {
        0.0
    }
}

/// golden.test.ts:150 `refusedSnapshot`: the 502 the backend maps, with the reply's request id.
pub fn refused_snapshot(request_id: &str) -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::SnapshotUnavailable,
        status: 502,
        code: None,
        message: "Failed to snapshot sandbox".to_string(),
        request_id: Some(request_id.to_string()),
        window: None,
        said: None,
    })
}

pub fn not_found(what: &str) -> EngineError {
    EngineError::Provider(classify(404, None, Some(what), None))
}

pub fn conflict(what: &str) -> EngineError {
    EngineError::Provider(classify(409, None, Some(what), None))
}

/// What the recording backend does, as `recordingBackend`'s options say.
#[derive(Default)]
pub struct Opts {
    /// `execResults`: exact command text to its answer.
    pub exec_results: IndexMap<String, ExecResult>,
    /// `ignoreKill(id, nth)`: a kill the provider accepts and changes nothing for.
    pub ignore_kill: Option<Box<dyn Fn(&str, u32) -> bool>>,
    /// `refuseKill(id, nth)`: the provider's refusal of a kill; the machine stays as it was.
    pub refuse_kill: Option<Box<dyn Fn(&str, u32) -> Option<EngineError>>>,
    /// `built(spec)`: what the provider says it built; the machines then offer `describe`.
    pub built: Option<Box<dyn Fn(&MachineSpec) -> MachineShape>>,
    /// `exec(cmd)`: answers before `exec_results`.
    pub exec: Option<Box<dyn Fn(&str) -> Option<ExecResult>>>,
    /// `stream`: each stdout line of a run reaches its `on_line`.
    pub stream: bool,
    pub snapshots_any_life: bool,
    /// `snapshot(id, nth)`: runs when a snapshot is asked; an `Err` is the provider's refusal.
    pub snapshot: Option<Box<dyn Fn(&str, u32) -> Result<()>>>,
    pub snapshot_progress: Vec<SnapshotProgress>,
    /// `state(id)`: runs on each state read; an `Err` fails the read.
    pub state: Option<Box<dyn Fn(&str) -> Result<()>>>,
    pub templates: bool,
    /// `templateStatus(id, nth)`: what each read of a template answers.
    pub template_status: Option<Box<dyn Fn(&str, u32) -> &'static str>>,
    /// The provider's builder disk, 20 GB; false asks for none, as a container does.
    pub no_disk: bool,
    pub signed_urls: bool,
    /// The base templates the backend names for its kinds.
    pub base_templates: Option<(String, String)>,
}

/// What the backend recorded.
#[derive(Default)]
pub struct State {
    pub created: RefCell<Vec<MachineSpec>>,
    pub snapshots: RefCell<Vec<String>>,
    pub killed: RefCell<Vec<String>>,
    pub deleted_snapshots: RefCell<Vec<String>>,
    /// Every create/kill/snapshot in order, so sequencing under the machine cap is provable.
    pub timeline: RefCell<Vec<String>>,
    /// The scripts that went through `run`, per machine.
    pub ran: RefCell<Vec<(String, String)>>,
    /// Every inline exec with its bound.
    pub inline: RefCell<Vec<(String, String, Option<u64>)>>,
    pub gone: RefCell<HashSet<String>>,
    pub templates: RefCell<IndexMap<String, TemplateRow>>,
    pub promoted: RefCell<Vec<(String, String)>>,
    pub deleted_templates: RefCell<Vec<String>>,
    template_reads: RefCell<HashMap<String, u32>>,
    kill_count: RefCell<HashMap<String, u32>>,
    machines: RefCell<IndexMap<String, Rc<RecMachine>>>,
    next_id: Cell<u32>,
}

impl State {
    pub fn killed(&self) -> Vec<String> {
        self.killed.borrow().clone()
    }
    pub fn timeline(&self) -> Vec<String> {
        self.timeline.borrow().clone()
    }
    pub fn created(&self) -> Vec<MachineSpec> {
        self.created.borrow().clone()
    }
    pub fn deleted_snapshots(&self) -> Vec<String> {
        self.deleted_snapshots.borrow().clone()
    }
    /// The scripts run on one machine, in order.
    pub fn ran_on(&self, id: &str) -> Vec<String> {
        self.ran.borrow().iter().filter(|(m, _)| m == id).map(|(_, s)| s.clone()).collect()
    }
    /// The inline commands asked of one machine, in order.
    pub fn inline_on(&self, id: &str) -> Vec<String> {
        self.inline.borrow().iter().filter(|(m, _, _)| m == id).map(|(_, c, _)| c.clone()).collect()
    }
}

pub struct Rec {
    pub backend: Arc<RecBackend>,
    pub state: Rc<State>,
}

impl Rec {
    pub fn dyn_backend(&self) -> Arc<dyn MachineBackend> {
        self.backend.clone()
    }
}

pub struct RecBackend {
    opts: Rc<Opts>,
    state: Rc<State>,
}

struct RecMachine {
    id: String,
    kind: MachineKind,
    spec: MachineSpec,
    opts: Rc<Opts>,
    state: Rc<State>,
}

pub fn recording_backend(opts: Opts) -> Rec {
    let opts = Rc::new(Opts { signed_urls: true, ..opts });
    let state = Rc::new(State::default());
    Rec { backend: Arc::new(RecBackend { opts, state: state.clone() }), state }
}

/// `recordingBackend()` with the TS's defaults: signed URLs, no templates.
pub fn recording() -> Rec {
    recording_backend(Opts { signed_urls: true, ..Opts::default() })
}

struct RecPricing {
    disk: bool,
}

impl BackendPricing for RecPricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        size.cpu * 0.035 + (size.mem_mb / 1024.0) * 0.01
    }
    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: serde_json::from_value::<WireSnapshotStoragePricing>(
                serde_json::json!({ "freeGb": 10, "usdPerGbMonth": 0.05, "billedFrom": "2026-10-01" }),
            )
            .expect("snapshot storage pricing"),
            free_named: None,
            usd_per_named_month: None,
        }
    }
    fn builder_disk_gb(&self) -> Option<f64> {
        self.disk.then_some(20.0)
    }
}

impl RecMachine {
    fn answer(&self, cmd: &str) -> ExecResult {
        if let Some(hook) = &self.opts.exec {
            if let Some(res) = hook(cmd) {
                return res;
            }
        }
        if let Some(res) = self.opts.exec_results.get(cmd) {
            return res.clone();
        }
        if cmd == "echo ok" {
            return reach_ok();
        }
        ok()
    }
}

#[async_trait(?Send)]
impl Machine for RecMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        self.kind
    }
    fn stream_url(&self) -> Option<&str> {
        None
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { describe: self.opts.built.is_some(), ..Default::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.state.inline.borrow_mut().push((self.id.clone(), cmd.to_string(), opts.timeout_ms));
        Ok(self.answer(cmd))
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        self.state.ran.borrow_mut().push((self.id.clone(), script.to_string()));
        let res = self.answer(script);
        if self.opts.stream {
            if let Some(on_line) = &opts.on_line {
                for line in res.stdout.split('\n').filter(|l| !l.is_empty()) {
                    on_line(line);
                }
            }
        }
        Ok(res)
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        if !life.first_life && !self.opts.snapshots_any_life {
            return Err(EngineError::NotFirstLife { machine_id: self.id.clone(), action: format!("snapshot {name}") });
        }
        self.state.timeline.borrow_mut().push(format!("snapshot {}", self.id));
        let nth = self.state.timeline.borrow().iter().filter(|t| **t == format!("snapshot {}", self.id)).count() as u32;
        if let Some(hook) = &self.opts.snapshot {
            hook(&self.id, nth)?;
        }
        for p in &self.opts.snapshot_progress {
            if let Some(on) = &opts.on_progress {
                on(*p);
            }
        }
        self.state.snapshots.borrow_mut().push(name.to_string());
        Ok(format!("snap_{name}"))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        self.state.killed.borrow_mut().push(self.id.clone());
        self.state.timeline.borrow_mut().push(format!("kill {}", self.id));
        let nth = {
            let mut counts = self.state.kill_count.borrow_mut();
            let n = counts.entry(self.id.clone()).or_insert(0);
            *n += 1;
            *n
        };
        if let Some(refuse) = &self.opts.refuse_kill {
            if let Some(e) = refuse(&self.id, nth) {
                return Err(e);
            }
        }
        let ignored = self.opts.ignore_kill.as_ref().is_some_and(|f| f(&self.id, nth));
        if !ignored {
            self.state.gone.borrow_mut().insert(self.id.clone());
        }
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        if let Some(hook) = &self.opts.state {
            hook(&self.id)?;
        }
        Ok(if self.state.gone.borrow().contains(&self.id) { MachineState::Gone } else { MachineState::Running })
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Ok("https://x".to_string())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Ok("https://x".to_string())
    }
    async fn describe(&self) -> Result<MachineShape> {
        match &self.opts.built {
            Some(f) => Ok(f(&self.spec)),
            None => Err(EngineError::Other { message: "not on this road".to_string(), code: None }),
        }
    }
}

#[async_trait(?Send)]
impl MachineBackend for RecBackend {
    fn capabilities(&self) -> Capabilities {
        let mut caps = wsp_engine::no_provider_backend::NoProviderBackend::new(None).capabilities();
        caps.live_clone_forks = true;
        caps.replaces_machine = true;
        caps.preview_urls = true;
        caps.signed_urls = self.opts.signed_urls;
        caps.callback_relay = true;
        caps.disk_snapshots = true;
        caps.images = true;
        caps.snapshots_any_life = self.opts.snapshots_any_life;
        caps.snapshot_listing = false;
        caps.templates = self.opts.templates;
        caps.kept = false;
        caps.copies = true;
        caps.own_network = true;
        caps.sizes = Vec::new();
        caps
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(RecPricing { disk: !self.opts.no_disk })
    }
    fn base_templates(&self) -> Option<wsp_engine::machine::BaseTemplates> {
        self.opts.base_templates.as_ref().map(|(sandbox, desktop)| {
            serde_json::from_value(serde_json::json!({ "sandbox": sandbox, "desktop": desktop })).expect("base templates")
        })
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers { templates: self.opts.templates, ..Default::default() }
    }
    async fn create(&self, spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        if let Some(t) = &spec.template {
            if t.starts_with("tpl_") && !self.state.templates.borrow().contains_key(t) {
                return Err(not_found(&format!("no template {t}")));
            }
        }
        self.state.created.borrow_mut().push(spec.clone());
        let n = self.state.next_id.get() + 1;
        self.state.next_id.set(n);
        let id = format!("m{n}");
        self.state.timeline.borrow_mut().push(format!("create {id}"));
        let machine = Rc::new(RecMachine { id: id.clone(), kind: spec.kind, spec, opts: self.opts.clone(), state: self.state.clone() });
        self.state.machines.borrow_mut().insert(id, machine.clone());
        Ok(Arc::new(Shared(machine)))
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        let found = self.state.machines.borrow().get(id).cloned();
        match found {
            Some(m) if !self.state.gone.borrow().contains(id) => Ok(Arc::new(Shared(m))),
            _ => Err(not_found(&format!("no machine {id}"))),
        }
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        Ok(Vec::new())
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        if self.state.templates.borrow().keys().any(|t| *t == format!("tpl_{id}")) {
            return Err(conflict("SnapshotBacksTemplate"));
        }
        self.state.deleted_snapshots.borrow_mut().push(id.to_string());
        Ok(())
    }
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String> {
        self.state.timeline.borrow_mut().push(format!("promote {snapshot_id}"));
        self.state.promoted.borrow_mut().push((snapshot_id.to_string(), name.to_string()));
        let id = format!("tpl_{snapshot_id}");
        self.state.templates.borrow_mut().insert(id.clone(), template_row(&id, name, "ready", None));
        Ok(id)
    }
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        let row = self.state.templates.borrow().get(id).cloned();
        let Some(row) = row else {
            return Err(not_found(&format!("no template {id}")));
        };
        let nth = {
            let mut reads = self.state.template_reads.borrow_mut();
            let n = reads.entry(id.to_string()).or_insert(0);
            *n += 1;
            *n
        };
        let status = self.opts.template_status.as_ref().map(|f| f(id, nth)).unwrap_or_else(|| status_word(&row));
        let error = (status == "failed").then(|| "restore copy failed".to_string());
        Ok(template_row(&row.id, &row.name, status, error))
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        let mut rows = vec![template_row("base", "base", "ready", None)];
        rows.extend(self.state.templates.borrow().values().cloned());
        Ok(rows)
    }
    async fn delete_template(&self, id: &str) -> Result<()> {
        self.state.deleted_templates.borrow_mut().push(id.to_string());
        self.state.templates.borrow_mut().shift_remove(id);
        Ok(())
    }
}

/// One machine handle shared by every `get` of it, so a test's hooks see one machine.
struct Shared(Rc<RecMachine>);

#[async_trait(?Send)]
impl Machine for Shared {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn kind(&self) -> MachineKind {
        self.0.kind()
    }
    fn seen(&self) -> Option<&Seen> {
        None
    }
    fn offers(&self) -> MachineOffers {
        self.0.offers()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.0.exec(cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        self.0.run(script, opts).await
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        self.0.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        self.0.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        self.0.resume(signal).await
    }
    async fn kill(&self) -> Result<()> {
        self.0.kill().await
    }
    async fn state(&self) -> Result<MachineState> {
        self.0.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        self.0.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        self.0.upload_url(path).await
    }
    async fn describe(&self) -> Result<MachineShape> {
        self.0.describe().await
    }
}

fn template_row(id: &str, name: &str, status: &str, error: Option<String>) -> TemplateRow {
    let mut v = serde_json::json!({ "id": id, "name": name, "status": status });
    if let Some(e) = error {
        v["error"] = serde_json::Value::String(e);
    }
    serde_json::from_value(v).expect("a template row")
}

fn status_word(row: &TemplateRow) -> &'static str {
    match serde_json::to_value(row).ok().and_then(|v| v.get("status").and_then(|s| s.as_str().map(str::to_string))).as_deref() {
        Some("building") => "building",
        Some("failed") => "failed",
        _ => "ready",
    }
}

/// golden.test.ts:182 `stageRecorder`: `stage` or `stage:detail`, in order.
pub struct Stages {
    pub lines: Rc<RefCell<Vec<String>>>,
    pub on_stage: StageFn,
    /// The `left` list of each frame that carried one, with its stage.
    pub left: Rc<RefCell<Vec<(String, Vec<String>)>>>,
}

pub fn stage_recorder() -> Stages {
    let lines: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let left: Rc<RefCell<Vec<(String, Vec<String>)>>> = Rc::new(RefCell::new(Vec::new()));
    let (l, k) = (lines.clone(), left.clone());
    let on_stage: StageFn =
        Arc::new(move |stage: GoldenStage, detail: Option<&str>, _step: Option<GoldenStep>, left: Option<&[String]>| {
            let word = stage_word(stage);
            l.borrow_mut().push(match detail {
                None => word.to_string(),
                Some(d) => format!("{word}:{d}"),
            });
            if let Some(left) = left {
                k.borrow_mut().push((word.to_string(), left.to_vec()));
            }
        });
    Stages { lines, on_stage, left }
}

impl Stages {
    pub fn all(&self) -> Vec<String> {
        self.lines.borrow().clone()
    }
}

pub fn stage_word(stage: GoldenStage) -> &'static str {
    match serde_json::to_value(stage).ok().as_ref().and_then(|v| v.as_str()) {
        Some("creating") => "creating",
        Some("deploying-daemon") => "deploying-daemon",
        Some("applying-setup") => "applying-setup",
        Some("uploading-files") => "uploading-files",
        Some("installing-harness") => "installing-harness",
        Some("installing-tools") => "installing-tools",
        Some("installing-mcp") => "installing-mcp",
        Some("ready") => "ready",
        Some("snapshotting") => "snapshotting",
        Some("promoting") => "promoting",
        Some("smoke-forking") => "smoke-forking",
        Some("sealed") => "sealed",
        Some("failed") => "failed",
        other => panic!("a stage this harness does not know: {other:?}"),
    }
}

/// golden.test.ts:190 `BASE_STEP` and `sansBase`: the base floor's own lines under its stage.
pub fn sans_base(stages: &[String]) -> Vec<String> {
    stages
        .iter()
        .filter(|s| {
            let Some(rest) = s.strip_prefix("deploying-daemon:") else { return true };
            !(rest.starts_with("free disk unknown") || is_step_line(rest) || is_installed_tally(rest))
        })
        .cloned()
        .collect()
}

fn is_step_line(rest: &str) -> bool {
    // `.* (\d+/\d+)$`
    let Some(open) = rest.rfind(" (") else { return false };
    let tail = &rest[open + 2..];
    let Some(inner) = tail.strip_suffix(')') else { return false };
    let Some((a, b)) = inner.split_once('/') else { return false };
    !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit())
}

fn is_installed_tally(rest: &str) -> bool {
    let digits = rest.bytes().take_while(|c| c.is_ascii_digit()).count();
    digits > 0 && rest[digits..].starts_with(" installed")
}

/// A fetch that answers every request with one status and records each request's method, URL and body.
pub struct FetchStub {
    pub status: u16,
    pub body: Vec<u8>,
    pub seen: RefCell<Vec<(String, String, Vec<u8>)>>,
}

impl FetchStub {
    pub fn ok() -> Arc<Self> {
        Arc::new(FetchStub { status: 200, body: Vec::new(), seen: RefCell::new(Vec::new()) })
    }
    pub fn with_body(body: Vec<u8>) -> Arc<Self> {
        Arc::new(FetchStub { status: 200, body, seen: RefCell::new(Vec::new()) })
    }
    /// The bodies of every PUT, in order.
    pub fn puts(&self) -> Vec<Vec<u8>> {
        self.seen.borrow().iter().filter(|(m, _, _)| m == "PUT").map(|(_, _, b)| b.clone()).collect()
    }
}

#[async_trait(?Send)]
impl Fetch for FetchStub {
    async fn fetch(&self, req: FetchRequest) -> std::result::Result<FetchResponse, wsp_engine::errors::RoadError> {
        let body = req.body.clone().unwrap_or_default();
        self.seen.borrow_mut().push((req.method.clone(), req.url.clone(), body));
        Ok(FetchResponse { status: self.status, headers: Vec::new(), body: self.body.clone() })
    }
}

/// golden.test.ts:178 `namesIn`: what a gzipped archive holds, read with this computer's tar.
pub fn names_in(tgz: &[u8]) -> Vec<String> {
    let out = tar_read(&["-tzf", "-"], tgz);
    String::from_utf8(out).unwrap().trim().split('\n').map(str::to_string).collect()
}

/// golden.test.ts:179 `fileIn`: one file out of a gzipped archive.
pub fn file_in(tgz: &[u8], path: &str) -> String {
    String::from_utf8(tar_read(&["-xzOf", "-", path], tgz)).unwrap()
}

fn tar_read(args: &[&str], input: &[u8]) -> Vec<u8> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child =
        Command::new("tar").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("tar");
    let mut stdin = child.stdin.take().unwrap();
    let bytes = input.to_vec();
    let feeder = std::thread::spawn(move || {
        let _ = stdin.write_all(&bytes);
    });
    let out = child.wait_with_output().expect("tar output");
    feeder.join().unwrap();
    assert!(out.status.success(), "tar {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

// --- option builders: the TS passes object literals, Rust fills the fields it leaves out ---------------------------

use wsp_engine::golden::{BuildGoldenOptions, MachineSize, PrepareBuilderOptions, SealGoldenOptions, UpgradeBuilderOptions};

pub fn build_opts(rec: &Rec, setup: &str, smoke: &str) -> BuildGoldenOptions {
    BuildGoldenOptions {
        backend: rec.dyn_backend(),
        size: MachineSize::default(),
        setup: setup.to_string(),
        smoke: smoke.to_string(),
        kind: None,
        base_template: None,
        manifest: None,
        smoke_timeout_ms: None,
        on_stage: None,
        name: None,
        host_id: "h1".to_string(),
    }
}

pub fn prepare_opts(rec: &Rec, setup: &str) -> PrepareBuilderOptions {
    PrepareBuilderOptions {
        backend: rec.dyn_backend(),
        size: MachineSize::default(),
        kind: None,
        base_template: None,
        deploy_daemon: None,
        setup: setup.to_string(),
        import: None,
        fetch: None,
        on_stage: None,
    }
}

pub fn seal_opts(rec: &Rec, smoke: &str) -> SealGoldenOptions {
    SealGoldenOptions {
        backend: rec.dyn_backend(),
        size: MachineSize::default(),
        smoke: smoke.to_string(),
        smoke_timeout_ms: None,
        manifest: None,
        on_stage: None,
        kill_confirm: None,
        logins: None,
        keep_builder: false,
        snapshot_retry_ms: None,
        name: None,
        host_id: "h1".to_string(),
        template_ready_ms: None,
        template_poll_ms: None,
        refuse: None,
        vault_paths: None,
        fetch: None,
    }
}

pub fn upgrade_opts(
    rec: &Rec,
    head: wsp_wire::views::golden_image::GoldenVersion,
    delta: wsp_engine::golden::GoldenDelta,
    setup: &str,
) -> UpgradeBuilderOptions {
    UpgradeBuilderOptions {
        backend: rec.dyn_backend(),
        size: MachineSize::default(),
        head,
        delta,
        setup: setup.to_string(),
        fetch: None,
        on_stage: None,
    }
}

// --- golden.test.ts:1061-1112, the import stages' fixtures --------------------------------------------------------

use wsp_engine::golden_seam::{AgentInstall, GoldenFiles, GoldenImport, PackedFiles, SkippedPath, ToolInstall};

/// `mb(n)`: n MB as df's kB column prints it.
pub fn mb(n: u64) -> String {
    (n * 1024).to_string()
}

/// golden.test.ts:1066 `importOf()` with no overrides: the recipe `h1`, three files over two rungs, three tools and
/// two agents. A test overrides a field with struct update: `GoldenImport { recipe_hash: "h2".into(), ..import_of() }`.
pub fn import_of() -> GoldenImport {
    let mut rungs = IndexMap::new();
    rungs.insert("identity".to_string(), 1u64);
    rungs.insert("shell".to_string(), 2u64);
    let tool = |id: &str, label: &str, manager: &str, cmd: &str, after: Option<&str>| ToolInstall {
        id: id.to_string(),
        label: label.to_string(),
        manager: manager.to_string(),
        cmd: cmd.to_string(),
        after: after.map(str::to_string),
        ..Default::default()
    };
    GoldenImport {
        recipe_hash: "h1".to_string(),
        files: Some(GoldenFiles {
            count: 3,
            rungs,
            lands: Vec::new(),
            bytes: 4096,
            skipped: vec![SkippedPath { id: "shell/bashrc".into(), path: "~/.bashrc".into(), note: "no longer on this computer".into() }],
            pack: Arc::new(|_| {
                Box::pin(async { Ok(PackedFiles { tar: b"tgz-bytes".to_vec(), bytes: 1200, unpacked: 4096, ..Default::default() }) })
            }),
            volatile: None,
        }),
        tools: vec![
            tool("tools/homebrew", "Homebrew", "brew", "brew-bootstrap", None),
            tool("tools/brew/gh", "gh", "brew", "brew install gh", Some("tools/homebrew")),
            tool("tools/npm/bun", "bun@1.4.0", "npm", "npm install -g bun@1.4.0", None),
        ],
        agents: vec![
            AgentInstall {
                id: "agents/claude".into(),
                name: "Claude Code".into(),
                install: "claude-install".into(),
                smoke: "claude --version".into(),
                road: "script".into(),
                ..Default::default()
            },
            AgentInstall {
                id: "agents/codex".into(),
                name: "Codex".into(),
                install: "codex-install".into(),
                smoke: "codex --version".into(),
                road: "npm".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

/// One answer of `backendFor`: a fixed result, or one computed at each ask.
pub enum Answer {
    Fixed(ExecResult),
    Each(Rc<dyn Fn() -> ExecResult>),
}

/// What df reports as free: a fixed string, or one computed at each ask.
pub enum Free {
    Kb(String),
    Each(Rc<dyn Fn() -> String>),
}

/// golden.test.ts:1093 `backendFor`'s result: the recording backend, every command asked of any machine (`exec` and
/// `run` alike, in order), and the fetch stub that records the uploads.
pub struct BackendFor {
    pub rec: Rec,
    pub cmds: Rc<RefCell<Vec<String>>>,
    pub fetch: Arc<FetchStub>,
}

impl BackendFor {
    pub fn cmds(&self) -> Vec<String> {
        self.cmds.borrow().clone()
    }
    pub fn dyn_fetch(&self) -> Arc<dyn Fetch> {
        self.fetch.clone()
    }
}

/// golden.test.ts:1093 `backendFor(answers, free)`: the fake answers exec by substring match, first hit wins; `free` is
/// what df reports.
pub fn backend_for(answers: Vec<(&str, Answer)>, free: Free) -> BackendFor {
    let cmds: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let seen = cmds.clone();
    let answers: Vec<(String, Answer)> = answers.into_iter().map(|(n, a)| (n.to_string(), a)).collect();
    let hook = move |cmd: &str| -> Option<ExecResult> {
        seen.borrow_mut().push(cmd.to_string());
        if let Some((_, a)) = answers.iter().find(|(needle, _)| cmd.contains(needle.as_str())) {
            return Some(match a {
                Answer::Fixed(r) => r.clone(),
                Answer::Each(f) => f(),
            });
        }
        if cmd == FREE_KB_CMD {
            let kb = match &free {
                Free::Kb(s) => s.clone(),
                Free::Each(f) => f(),
            };
            return Some(exec(0, &format!("{kb}\n"), ""));
        }
        if cmd == "echo ok" {
            return Some(reach_ok());
        }
        if cmd.contains("echo WSP_CTX") {
            return Some(exec(0, "WSP_CTX\nWSP_CTX_END\n", ""));
        }
        Some(ok())
    };
    BackendFor { rec: recording_backend(Opts { exec: Some(Box::new(hook)), ..Opts::default() }), cmds, fetch: FetchStub::ok() }
}

/// `backendFor()` with no answers and 3,000 MB free.
pub fn backend_default() -> BackendFor {
    backend_for(Vec::new(), Free::Kb(mb(3000)))
}

/// golden.test.ts:177 `CONTEXT_WRITTEN`: the stage detail the machine context leaves when no agent is on the guest,
/// whatever its archive weighs (`^installing-mcp:machine context: \d+(\.\d+)? KB written; no agent on the machine$`).
pub fn is_context_written(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("installing-mcp:machine context: ") else { return false };
    let Some(rest) = rest.strip_suffix(" KB written; no agent on the machine") else { return false };
    let (whole, frac) = match rest.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (rest, None),
    };
    !whole.is_empty()
        && whole.bytes().all(|c| c.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|c| c.is_ascii_digit()))
}

/// `Date.parse(iso)` in milliseconds, none for a string that is no time.
pub fn parse_ms(iso: &str) -> Option<i64> {
    iso.parse::<jiff::Timestamp>().ok().map(|t| t.as_millisecond())
}
