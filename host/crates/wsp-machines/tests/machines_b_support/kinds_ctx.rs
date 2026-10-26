// SPDX-License-Identifier: AGPL-3.0-only
//! `KindsCtx` over the fake host. The state the trait reads beyond FakeHost's own fields lives in one `World` per
//! test thread; `kinds()` and `kinds_with()` reset it, so every test starts from the defaults. The local machine
//! answers by really running each command with `sh -c`, as the TS tests' real git and fake `gh` on PATH do.
use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_adapters::codex::command::{AccountModels, BrokerMode, CodexBroker};
use wsp_adapters::pending::CodexClaims;
use wsp_core::types::internal::{EndThread, ImportLanded, ImportReport};
use wsp_core::types::internal::{KindModule, PortRoute};
use wsp_core::types::pending::MachineExecOptions;
use wsp_core::types::wiring::{
    LiveWorkspace, Local, PackedProject, PackedState, ProjectBundler, ProjectImportOptions,
    StageReport, StateRequest, WorkspaceRecord,
};
use wsp_engine::errors::EngineError;
use wsp_engine::link_backend::{LinkAsk, MachineLink};
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineLife,
    MachineListRow, MachineOffers, MachineSpec, MachineState, RunOptions, SnapshotOptions, Waiting,
};
use wsp_engine::place_machine::PlaceFolderMachine;
use wsp_machines::machines::area_error::AreaResult;
use wsp_machines::machines::brokered::{ClaudePlaceholderKind, Placeholders};
use wsp_machines::machines::kinds::{BrokerSaid, KindsArea, KindsCtx};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::thread_ends::OwedEnd;
use wsp_machines::places::types::FolderComputer;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::views::golden_image::MachineKind;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::project_bundle::{ProjectImportResult, ProjectImportStage, ProjectPlan};
use wsp_wire::views::workspace::{DaemonReachView, ProjectView, WorkspaceProject};
use wsp_wire::views::workspace_more::WorktreeFolder;
use wsp_wire::wire::machine_link::PreviewReach;
use wsp_wire::wire::runtime::{BrokerStatus, BrokerStatusClaude, BrokerStatusCodex};
use wsp_words::exit::{HostError, Kind};

use super::*;

/// A host error with these words and this kind.
pub fn host_error(message: &str, kind: Option<Kind>) -> HostError {
    match kind {
        Some(kind) => HostError::with_kind(message, kind),
        None => HostError::new(message),
    }
}

/// What the fake broker says of an agent it adds no key for.
pub fn unbrokered(harness: &str) -> Option<HostError> {
    (harness != "claude" && harness != "codex").then(|| {
        host_error(
            &format!("{harness} does not run on a cloud machine yet: wsp keeps no key there"),
            Some(Kind::Usage),
        )
    })
}

/// What the fake broker refuses a codex launch with when it names no login.
pub fn codex_unread() -> HostError {
    host_error("Codex on a cloud machine runs on the Codex login on your computer, and wsp read none just now", Some(Kind::Auth))
}

/// Whether two backend handles are one backend.
pub fn same_backend(a: &Arc<dyn MachineBackend>, b: &Arc<dyn MachineBackend>) -> bool {
    Arc::as_ptr(a) as *const () == Arc::as_ptr(b) as *const ()
}

// ---------------------------------------------------------------------------------------------------------------
// This computer's machine: every command really run by sh, in a folder, with a folder of fake commands first on PATH.

pub struct LocalShell {
    pub cwd: PathBuf,
    pub path_first: Option<PathBuf>,
    pub log: Mutex<Vec<(String, Option<u64>)>>,
}

impl LocalShell {
    pub fn new(cwd: &Path, path_first: Option<&Path>) -> Arc<LocalShell> {
        Arc::new(LocalShell {
            cwd: cwd.to_path_buf(),
            path_first: path_first.map(|p| p.to_path_buf()),
            log: Mutex::new(Vec::new()),
        })
    }
    /// Every command run, in order.
    pub fn ran(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .map(|(c, _)| c.clone())
            .collect()
    }
    /// The timeout each command was run with, in order.
    pub fn timeouts(&self) -> Vec<Option<u64>> {
        self.log.lock().unwrap().iter().map(|(_, t)| *t).collect()
    }
}

fn not_here<T>() -> Result<T, EngineError> {
    Err(engine_error("the local shell does this no way"))
}

#[async_trait(?Send)]
impl Machine for LocalShell {
    fn id(&self) -> &str {
        "local"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult, EngineError> {
        self.log
            .lock()
            .unwrap()
            .push((cmd.to_string(), opts.timeout_ms));
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd).current_dir(&self.cwd);
        if let Some(first) = &self.path_first {
            c.env(
                "PATH",
                format!(
                    "{}:{}",
                    first.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            );
        }
        c.stdin(if opts.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        let mut child = c.spawn().map_err(|e| engine_error(&e.to_string()))?;
        if let Some(bytes) = &opts.stdin {
            let mut stdin = child.stdin.take().expect("a piped stdin");
            stdin
                .write_all(bytes)
                .map_err(|e| engine_error(&e.to_string()))?;
        }
        let out = child
            .wait_with_output()
            .map_err(|e| engine_error(&e.to_string()))?;
        let code = out.status.code().unwrap_or(-1) as i64;
        Ok(ExecResult {
            exit_code: code.into(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        self.exec(script, ExecOptions::default()).await
    }
    async fn snapshot(
        &self,
        _name: &str,
        _life: MachineLife,
        _opts: SnapshotOptions,
    ) -> Result<String, EngineError> {
        not_here()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        not_here()
    }
    async fn resume(
        &self,
        _signal: Option<&wsp_engine::cancel::Cancel>,
    ) -> Result<(), EngineError> {
        not_here()
    }
    async fn kill(&self) -> Result<(), EngineError> {
        not_here()
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String, EngineError> {
        not_here()
    }
    async fn upload_url(&self, _path: &str) -> Result<String, EngineError> {
        not_here()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A backend with no lifecycle, as the released one is, whose three facts a test sets.

pub struct PlainBackend {
    pub images: bool,
    pub disk: bool,
    pub names: bool,
}

#[async_trait(?Send)]
impl MachineBackend for PlainBackend {
    fn capabilities(&self) -> Capabilities {
        let mut caps = boat_capabilities();
        caps.images = self.images;
        caps.pause_mode = self
            .disk
            .then_some(wsp_wire::wire::capabilities::PauseMode::Disk);
        caps
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FakePricing)
    }
    fn names_workspace(&self) -> bool {
        self.names
    }
    async fn create(
        &self,
        _spec: MachineSpec,
        _waiting: Option<Waiting>,
        _sent: Option<String>,
    ) -> Result<Arc<dyn Machine>, EngineError> {
        not_here()
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        not_here()
    }
    async fn list(
        &self,
        _labels: Option<&IndexMap<String, String>>,
    ) -> Result<Vec<MachineListRow>, EngineError> {
        Ok(Vec::new())
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<(), EngineError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The world the context reads, one per test thread.

/// A road to a computer that answers nothing: a `PlaceFolderMachine` rides one, and no test here reads through it.
pub struct SilentLink;

#[async_trait(?Send)]
impl MachineLink for SilentLink {
    async fn request(
        &self,
        _op: &str,
        _params: Map<String, Value>,
        _ask: LinkAsk,
    ) -> Result<Map<String, Value>, EngineError> {
        Err(engine_error("the silent link answers nothing"))
    }
    async fn forward(&self, _place_port: u16) -> Result<u16, EngineError> {
        Err(engine_error("the silent link carries no port"))
    }
}

/// The folder computer a door holds, as the parts of a `FolderComputer` (which is not `Clone`). Its machine is made
/// when a call asks for the computer, so a test that never reads it never builds one.
#[derive(Clone)]
pub struct FolderSpec {
    pub id: String,
    pub home: String,
    pub cpu: u32,
    pub mem_mb: u64,
    pub tools: bool,
}

/// What the place door says of one joined computer.
#[derive(Clone, Default)]
pub struct Door {
    pub name: String,
    pub backend: Option<Arc<dyn MachineBackend>>,
    pub folder: Option<FolderSpec>,
    pub joined: bool,
    pub logins: Option<String>,
    /// Keys the door's picks take out of the vault; none means the door answers nothing for the vault.
    pub vault_drops: Option<Vec<String>>,
    pub sign_ins: Option<IndexMap<String, AgentSignInState>>,
    pub github_from_vault: bool,
    pub daemon_version: Option<u32>,
    pub forward_port: u16,
}

pub struct World {
    pub state_path: Option<String>,
    pub local: Option<Arc<dyn Machine>>,
    pub default_dir: Option<tempfile::TempDir>,
    pub released: Arc<dyn MachineBackend>,
    pub place_names: IndexMap<String, String>,
    pub wired: String,
    pub projects: IndexMap<String, ProjectView>,
    pub served: IndexSet<String>,
    pub place_door: bool,
    pub doors: IndexMap<String, Door>,
    pub vault: IndexMap<String, String>,
    pub broker_ready: bool,
    pub broker_has_status: bool,
    pub provider_places: IndexSet<String>,
    pub deploy: bool,
    pub local_road: Option<DaemonReachView>,
    pub machine_exec: Option<MachineExecOptions>,
    pub hello: Option<u32>,
    /// The local wiring; `None` is a host that wired no local road.
    pub local_wiring: Option<LocalWiring>,
    pub agents_wsp_mcp: Option<McpServerSpec>,
    pub agents_here_url: Option<String>,
    pub login_user: String,
}

/// What the local wiring answers (kinds.ts `local`).
pub struct LocalWiring {
    pub home: String,
    pub home_dir: String,
    pub env: IndexMap<String, String>,
    pub roots_path: Option<String>,
    pub daemon_road: bool,
    pub restart: bool,
    pub here_daemon: Option<u32>,
    pub scratch: String,
}

impl World {
    fn fresh() -> World {
        World {
            state_path: None,
            local: None,
            default_dir: None,
            released: Arc::new(PlainBackend {
                images: false,
                disk: false,
                names: false,
            }),
            place_names: IndexMap::from([("box".to_string(), "Boat".to_string())]),
            wired: "default".to_string(),
            projects: IndexMap::new(),
            served: IndexSet::new(),
            place_door: false,
            doors: IndexMap::new(),
            vault: IndexMap::new(),
            broker_ready: true,
            broker_has_status: true,
            provider_places: IndexSet::from(["box".to_string()]),
            deploy: false,
            local_road: None,
            machine_exec: None,
            hello: None,
            local_wiring: Some(LocalWiring {
                home: "/home/me".to_string(),
                home_dir: "/home/me".to_string(),
                env: IndexMap::new(),
                roots_path: None,
                daemon_road: true,
                restart: false,
                here_daemon: None,
                scratch: "/tmp/wsp-local".to_string(),
            }),
            agents_wsp_mcp: None,
            agents_here_url: None,
            login_user: "root".to_string(),
        }
    }
}

thread_local! {
    static WORLD: RefCell<World> = RefCell::new(World::fresh());
}

/// Reads or sets the world this test thread's host answers from.
pub fn world<R>(f: impl FnOnce(&mut World) -> R) -> R {
    WORLD.with(|w| f(&mut w.borrow_mut()))
}

/// A host and the area over it, on the default world.
pub fn kinds() -> (Rc<FakeHost>, Rc<KindsArea>) {
    kinds_with(|_| {})
}

/// The same, with the world set before the area is made, which is when it reads which kinds are wired.
pub fn kinds_with(set: impl FnOnce(&mut World)) -> (Rc<FakeHost>, Rc<KindsArea>) {
    WORLD.with(|w| *w.borrow_mut() = World::fresh());
    world(set);
    let host = FakeHost::new();
    let area = KindsArea::new(host.clone());
    (host, area)
}

/// A joined computer the door holds, its folder computer a `PlaceFolderMachine` over a silent link at this home.
pub fn joined_door(name: &str, home: &str, tools: bool) -> Door {
    Door {
        name: name.to_string(),
        folder: Some(FolderSpec {
            id: format!("{name}-machine"),
            home: home.to_string(),
            cpu: 2,
            mem_mb: 4096,
            tools,
        }),
        joined: true,
        forward_port: 41_000,
        ..Door::default()
    }
}

/// A project as the host holds it, from the TS shape with `extra` keys over it.
pub fn project(id: &str, path: &str, extra: Value) -> ProjectView {
    let remote = "https://github.com/spoo-me/frontend.git";
    let name = path.rsplit('/').next().unwrap_or(path);
    let mut v = json!({
        "id": id, "name": name, "computer": "default", "source": { "kind": "git", "url": remote }, "path": path, "remote": remote,
        "defaultBranch": "main", "memoryKey": format!("-recorded-{id}"), "memoryDir": format!("/root/.claude-cfg/projects/-recorded-{id}"),
        "createdAt": "2026-10-12T06:00:00.000Z"
    });
    for (k, val) in extra.as_object().expect("extra keys are an object") {
        v[k] = val.clone();
    }
    serde_json::from_value(v).expect("the project reads as the TS writes it")
}

/// Holds a project in the world.
pub fn hold(p: &ProjectView) {
    world(|w| {
        w.projects.insert(p.id.clone(), p.clone());
    });
}

/// A record of any kind off the sub-thread record, with `extra` keys over it and the keys named in `without` taken out.
pub fn record_of(id: &str, extra: Value, without: &[&str]) -> Value {
    let mut v = sub_record(id, "running");
    for (k, val) in extra.as_object().expect("extra keys are an object") {
        v[k] = val.clone();
    }
    for k in without {
        v.as_object_mut().unwrap().remove(*k);
    }
    v
}

/// The stage reports a call made, as `(stage, line)`.
pub fn reports() -> (Rc<RefCell<Vec<(String, String)>>>, StageReport) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let keep = seen.clone();
    let report: StageReport =
        Rc::new(move |stage, line, _| keep.borrow_mut().push((word_of(&stage), line.to_string())));
    (seen, report)
}

// ---------------------------------------------------------------------------------------------------------------
// A repo to clone from and a git and a gh ahead of the real ones that write down how they were run (projects.test.ts
// cloneRig): no network, and the argv and the prompt setting are read off what the area really spawned.

pub struct CloneRig {
    pub origin: PathBuf,
    pub bin: tempfile::TempDir,
    pub log: PathBuf,
    folders: RefCell<Vec<tempfile::TempDir>>,
    _origin: tempfile::TempDir,
}

/// One recorded run of the fake git or gh: the prompt setting it saw and its argv.
#[derive(Debug, PartialEq)]
pub struct Run {
    pub prompt: String,
    pub argv: Vec<String>,
}

fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("the folder stands")
}

fn git_in(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

impl CloneRig {
    pub fn new() -> CloneRig {
        let origin_dir = tempfile::tempdir().unwrap();
        let origin = real(origin_dir.path());
        let out = Command::new("git")
            .args(["init", "-q"])
            .arg(&origin)
            .output()
            .expect("git runs");
        assert!(out.status.success());
        std::fs::write(origin.join("README.md"), "spoo\n").unwrap();
        git_in(&origin, &["add", "."]);
        git_in(
            &origin,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "one",
            ],
        );
        let bin = tempfile::tempdir().unwrap();
        let log = bin.path().join("calls.log");
        let real_git = String::from_utf8(
            Command::new("sh")
                .args(["-c", "command -v git"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let record = |name: &str| {
            format!(
                r#"printf '%s\t%s\n' "${{GIT_TERMINAL_PROMPT:-unset}}" "$(printf '%s\037' {name} "$@")" >> '{}'"#,
                log.display()
            )
        };
        // An url naming "missing" fails the way a clone of a repo that is not there does, and one naming "private"
        // the way a signed-out one does; a clone that fails leaves behind the folder above its own that it made.
        let pick = r#"case "$URL" in *missing*|*private*) mkdir -p "$(dirname "$4")";; esac; case "$URL" in *missing*) echo "fatal: repository '$URL' does not exist" >&2; exit 128;; *private*) echo "remote: Repository not found." >&2; echo "fatal: repository '$URL' not found" >&2; exit 128;; esac"#;
        let clone = format!(
            r#"exec '{}' clone -q '{}' "$4""#,
            real_git,
            origin.display()
        );
        let git = format!("#!/bin/sh\n{}\nif [ \"$1\" = clone ]; then URL=\"$3\"; {pick}; {clone}; fi\nexec '{}' \"$@\"\n", record("git"), real_git);
        let gh = format!("#!/bin/sh\n{}\nURL=\"$3\"; {pick}; {clone}\n", record("gh"));
        for (name, text) in [("git", git), ("gh", gh)] {
            let path = bin.path().join(name);
            std::fs::write(&path, text).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        CloneRig {
            origin,
            bin,
            log,
            folders: RefCell::new(Vec::new()),
            _origin: origin_dir,
        }
    }

    /// A path under the rig's one folder, as the TS `into(...parts)` joins under `here()`: the folder stands, the
    /// parts do not.
    pub fn folder_at(&self, parts: &[&str]) -> PathBuf {
        if self.folders.borrow().is_empty() {
            self.folders.borrow_mut().push(tempfile::tempdir().unwrap());
        }
        let root = real(self.folders.borrow()[0].path());
        parts.iter().fold(root, |p, part| p.join(part))
    }

    /// The machine every command of the area runs on: this rig's fake git and gh first on PATH.
    pub fn shell(&self) -> Arc<LocalShell> {
        let cwd = self.folder_at(&[]);
        LocalShell::new(&cwd, Some(self.bin.path()))
    }

    /// Every run of the fake git and gh, in order.
    pub fn calls(&self) -> Vec<Run> {
        let Ok(text) = std::fs::read_to_string(&self.log) else {
            return Vec::new();
        };
        text.trim()
            .lines()
            .map(|line| {
                let (prompt, argv) = line
                    .split_once('\t')
                    .expect("a tab between the prompt and the argv");
                Run {
                    prompt: prompt.to_string(),
                    argv: argv
                        .split('\u{1f}')
                        .filter(|w| !w.is_empty())
                        .map(|w| w.to_string())
                        .collect(),
                }
            })
            .collect()
    }

    /// The last word of every clone run, which is the folder it was told to clone into.
    pub fn clone_targets(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|c| c.argv.get(1).map(|w| w == "clone").unwrap_or(false) || c.argv[0] == "gh")
            .map(|c| c.argv.last().unwrap().clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Reading a kind module's answers off an entry. The module's futures own what they read, so each helper makes the
// call in one statement, which ends the borrow of the entry before anything is awaited.

/// The record an entry holds, as it stands now.
pub fn held_record(e: &Entry) -> WorkspaceRecord {
    e.borrow().record.borrow().clone()
}

/// Puts the worktree the TS test wrote as JSON on an entry's record.
pub fn set_worktree(e: &Entry, tree: Value) {
    let tree: WorktreeFolder =
        serde_json::from_value(tree).expect("the worktree reads as the TS writes it");
    e.borrow().record.borrow_mut().worktree = Some(tree);
}

/// The wire word of a stage or any other enum that serializes as one string.
pub fn word_of<T: serde::Serialize>(word: &T) -> String {
    serde_json::to_value(word)
        .unwrap()
        .as_str()
        .expect("a word")
        .to_string()
}

pub async fn dropped_of(m: &dyn KindModule, e: &Entry) -> Result<(), HostError> {
    let asked = m.dropped(&e.borrow());
    asked.await
}

pub async fn roots_of(m: &dyn KindModule, e: &Entry, dests: &[String]) -> Result<(), HostError> {
    let asked = m.roots(&e.borrow(), dests);
    asked.await
}

pub async fn daemon_road_of(m: &dyn KindModule, e: &Entry) -> Result<DaemonReachView, HostError> {
    let asked = m.daemon_road(&e.borrow());
    asked.await
}

pub async fn daemon_version_of(m: &dyn KindModule, e: &Entry) -> Result<Option<u32>, HostError> {
    let asked = m.daemon_version(&e.borrow());
    asked.await
}

pub async fn import_of(
    m: &dyn KindModule,
    e: &Entry,
    o: ProjectImportOptions,
    report: ImportReport,
) -> Result<ImportLanded, HostError> {
    let asked = m.import(&e.borrow(), o, report);
    asked.await
}

pub async fn land_project_of(
    m: &dyn KindModule,
    e: &Entry,
    project: ProjectView,
    report: StageReport,
) -> Result<(), HostError> {
    let asked = m.land_project(&e.borrow(), project, report);
    asked.await
}

/// The port a place forwards; a module that answers no route fails here.
pub async fn port_reach_of(
    m: &dyn KindModule,
    e: &Entry,
    port: u16,
    standing: bool,
) -> Result<PortRoute, HostError> {
    let asked = m
        .port_reach(&e.borrow(), port, standing)
        .expect("a place forwards the port");
    asked.await
}

/// The deploy a recipe wired, or `None` where the kind has none.
pub fn deploy_of(m: &dyn KindModule, e: &Entry) -> Option<Local<'static, Option<String>>> {
    m.deploy_daemon(&e.borrow())
}

// ---------------------------------------------------------------------------------------------------------------
// The import options a test hands a kind: the bundler is the one the TS test's `plan` was read from.

/// A folder on this computer as the bundler measures it: its plan is read, and nothing else is asked of it.
pub struct FakeBundler {
    pub plans: Mutex<u32>,
}

impl FakeBundler {
    pub fn new() -> Rc<FakeBundler> {
        Rc::new(FakeBundler {
            plans: Mutex::new(0),
        })
    }
    pub fn plans(&self) -> u32 {
        *self.plans.lock().unwrap()
    }
}

#[async_trait(?Send)]
impl ProjectBundler for FakeBundler {
    async fn plan(&self) -> Result<ProjectPlan, HostError> {
        *self.plans.lock().unwrap() += 1;
        Ok(ProjectPlan {
            source: "/home/z/src".to_string(),
            repo: true,
            files: 12.0.into(),
            bytes: 3456.0.into(),
            secrets: Vec::new(),
            values: None,
            excluded: Vec::new(),
            skipped: Vec::new(),
            agents: Vec::new(),
        })
    }
    async fn pack(
        &self,
        _carry: &[String],
        _rewrite: &[String],
        _provider: bool,
    ) -> Result<PackedProject, HostError> {
        Err(host_error("the fake bundler packs nothing", None))
    }
    async fn pack_state(&self, _req: StateRequest) -> Result<PackedState, HostError> {
        Err(host_error("the fake bundler packs no state", None))
    }
}

/// Import options for a workspace's dest, the bundler kept to read back how often its plan was asked.
pub fn import_options(workspace_id: &str, dest: &str) -> (ProjectImportOptions, Rc<FakeBundler>) {
    let bundler = FakeBundler::new();
    let o = ProjectImportOptions {
        workspace_id: workspace_id.to_string(),
        source: "/home/z/src".to_string(),
        dest: dest.to_string(),
        replace: None,
        carry: None,
        rewrite: None,
        agents: None,
        bundler: bundler.clone(),
    };
    (o, bundler)
}

/// The options as words, for keeping them on a call: the bundler is a handle and has none.
pub fn import_words(o: &ProjectImportOptions) -> String {
    json!({ "workspaceId": o.workspace_id, "source": o.source, "dest": o.dest, "replace": o.replace, "carry": o.carry, "rewrite": o.rewrite, "agents": o.agents }).to_string()
}

/// The import stages a call reported, as `(stage, line)`.
pub fn import_report() -> (Rc<RefCell<Vec<(String, String)>>>, ImportReport) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let keep = seen.clone();
    let report: ImportReport = Rc::new(
        move |stage: ProjectImportStage, line: &str, _: Option<(u64, u64)>| {
            keep.borrow_mut().push((word_of(&stage), line.to_string()));
            Ok(())
        },
    );
    (seen, report)
}

/// An import report whose bus listener throws at every stage: the TS's `report` throws and the import stops there.
pub fn import_report_refused(said: &str) -> ImportReport {
    let said = said.to_string();
    Rc::new(
        move |_: ProjectImportStage, _: &str, _: Option<(u64, u64)>| {
            Err(HostError::new(said.clone()))
        },
    )
}

/// A number as a result carries it.
pub fn js(n: f64) -> JsNumber {
    JsNumber(n)
}

pub fn js_int(n: f64) -> JsInt {
    JsInt(JsNumber(n))
}

// ---------------------------------------------------------------------------------------------------------------
// A machine whose preview route answers, for the road `Workspace::daemon_reach` reads: the url the removed FakeRoad
// handed back. Every other call goes to the fake machine inside, so its log and its offers are the test's own.

pub struct PreviewMachine(pub Arc<FakeMachine>);

#[async_trait(?Send)]
impl Machine for PreviewMachine {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn kind(&self) -> MachineKind {
        self.0.kind()
    }
    fn offers(&self) -> MachineOffers {
        self.0.offers()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult, EngineError> {
        self.0.exec(cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult, EngineError> {
        self.0.run(script, opts).await
    }
    async fn snapshot(
        &self,
        name: &str,
        life: MachineLife,
        opts: SnapshotOptions,
    ) -> Result<String, EngineError> {
        self.0.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<(), EngineError> {
        self.0.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&wsp_engine::cancel::Cancel>) -> Result<(), EngineError> {
        self.0.resume(signal).await
    }
    async fn kill(&self) -> Result<(), EngineError> {
        self.0.kill().await
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        self.0.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String, EngineError> {
        self.0.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String, EngineError> {
        self.0.upload_url(path).await
    }
    async fn preview_url(&self, _port: u16) -> Result<PreviewReach, EngineError> {
        Ok(PreviewReach {
            url: "http://127.0.0.1:1/".to_string(),
            token: "preview".to_string(),
            expires_at: 9_999_999_999_999.0.into(),
        })
    }
    async fn daemon_answers(&self, timeout_ms: Option<u64>) -> Result<bool, EngineError> {
        self.0.daemon_answers(timeout_ms).await
    }
    async fn start_daemon(&self) -> Result<ExecResult, EngineError> {
        self.0.start_daemon().await
    }
    async fn daemon_exits(&self) -> Result<Option<String>, EngineError> {
        self.0.daemon_exits().await
    }
    async fn put_bytes(
        &self,
        path: &str,
        bytes: &[u8],
        timeout_ms: Option<u64>,
    ) -> Result<Option<wsp_engine::machine::BytesLanded>, EngineError> {
        self.0.put_bytes(path, bytes, timeout_ms).await
    }
    async fn read_bytes(
        &self,
        path: &str,
        size: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> Result<Vec<u8>, EngineError> {
        self.0.read_bytes(path, size, timeout_ms).await
    }
}

/// `put_live` on a machine whose preview route answers: the fake machine is still the one the backend holds.
pub fn put_live_preview(host: &FakeHost, record: Value, machine: Arc<FakeMachine>) -> Entry {
    let record: WorkspaceRecord =
        serde_json::from_value(record).expect("the record reads as the TS writes it");
    let id = record.id.clone();
    host.backend.add(machine.clone());
    let entry = live_entry(
        Rc::new(RefCell::new(record)),
        Arc::new(PreviewMachine(machine)),
    );
    host.live.borrow_mut().insert(id, entry.clone());
    entry
}

// ---------------------------------------------------------------------------------------------------------------
// The context.

fn door_of(place: &str) -> Door {
    world(|w| {
        w.doors.get(place).cloned().unwrap_or_else(|| Door {
            name: place.to_string(),
            ..Door::default()
        })
    })
}

#[async_trait(?Send)]
impl KindsCtx for FakeHost {
    fn now(&self) -> f64 {
        self.clock.now()
    }
    fn state_path(&self) -> Option<String> {
        world(|w| w.state_path.clone())
    }
    async fn local_machine(&self) -> AreaResult<Arc<dyn Machine>> {
        self.call("local_machine", &[]);
        if let Some(Err(e)) = self.next("local_machine") {
            return Err(e);
        }
        Ok(world(|w| {
            if w.local.is_none() {
                let dir = tempfile::tempdir().unwrap();
                w.local = Some(LocalShell::new(dir.path(), None));
                w.default_dir = Some(dir);
            }
            w.local.clone().unwrap()
        }))
    }
    fn own_backend(&self) -> Arc<dyn MachineBackend> {
        self.backend.clone()
    }
    fn released_backend(&self) -> Arc<dyn MachineBackend> {
        world(|w| w.released.clone())
    }
    fn place_name(&self, place: &str) -> String {
        world(|w| {
            w.place_names
                .get(place)
                .cloned()
                .unwrap_or_else(|| place.to_string())
        })
    }
    fn wired_place(&self) -> String {
        world(|w| w.wired.clone())
    }
    fn project_held(&self, id: &str) -> AreaResult<ProjectView> {
        if let Some(e) = self.project_held_failure.borrow().clone() {
            return Err(AreaError::Host(e));
        }
        Ok(world(|w| w.projects.get(id).cloned())
            .unwrap_or_else(|| panic!("the test held no project {id}")))
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> AreaResult<String> {
        if let Some(e) = self.checkout_failure.borrow().clone() {
            return Err(AreaError::Host(e));
        }
        match record.worktree.as_ref().map(|t| t.path.clone()) {
            Some(path) => Ok(path),
            None => Ok(world(|w| w.projects.get(&record.project).cloned())
                .unwrap_or_else(|| panic!("the test held no project {}", record.project))
                .path),
        }
    }
    fn computer_of(&self, entry: &LiveWorkspace) -> String {
        let place = entry.record.borrow().place.clone();
        self.place_name(&place.unwrap_or_else(|| self.wired_place()))
    }
    fn served_by_its_computer(&self, entry: &LiveWorkspace) -> bool {
        world(|w| w.served.contains(&entry.record.borrow().id))
    }
    fn has_place_door(&self) -> bool {
        world(|w| w.place_door)
    }
    fn door_backend_of(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        door_of(place).backend
    }
    fn door_name_of(&self, place: &str) -> String {
        door_of(place).name
    }
    fn door_folder_computer(&self, place: &str) -> Option<FolderComputer> {
        door_of(place).folder.map(|f| FolderComputer {
            machine: PlaceFolderMachine::new(Arc::new(SilentLink), &f.id, &f.home, None),
            home: f.home,
            cpu: f.cpu,
            mem_mb: f.mem_mb,
            tools: f.tools,
        })
    }
    fn door_joined(&self, place: &str) -> bool {
        world(|w| w.place_door) && door_of(place).joined
    }
    fn door_logins_of(&self, place: &str) -> Option<String> {
        door_of(place).logins
    }
    fn door_vault_at(
        &self,
        place: &str,
        vault: &IndexMap<String, String>,
    ) -> Option<IndexMap<String, String>> {
        door_of(place).vault_drops.map(|drops| {
            vault
                .iter()
                .filter(|(k, _)| !drops.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
    }
    fn door_sign_ins_at(&self, place: &str) -> Option<IndexMap<String, AgentSignInState>> {
        door_of(place).sign_ins
    }
    fn door_github_from_vault(&self, place: &str) -> bool {
        door_of(place).github_from_vault
    }
    async fn door_exec(&self, place: &str, cmd: &str, timeout_ms: u64) -> AreaResult<ExecResult> {
        self.call("door_exec", &[place, cmd, &timeout_ms.to_string()]);
        match self.next("door_exec") {
            Some(Err(e)) => Err(e),
            Some(Ok(v)) => Ok(ExecResult {
                exit_code: v["exitCode"].as_i64().unwrap_or(0).into(),
                stdout: v["stdout"].as_str().unwrap_or("").to_string(),
                stderr: v["stderr"].as_str().unwrap_or("").to_string(),
            }),
            None => Ok(ExecResult {
                exit_code: 0.into(),
                stdout: String::new(),
                stderr: String::new(),
            }),
        }
    }
    async fn door_forward(
        &self,
        place: &str,
        port: u16,
        pane: Option<(String, String)>,
        standing: bool,
    ) -> AreaResult<u16> {
        let pane =
            pane.map(|(workspace_id, name)| json!({ "workspaceId": workspace_id, "name": name }));
        self.call(
            "door_forward",
            &[
                place,
                &port.to_string(),
                &serde_json::to_string(&pane).unwrap(),
                &standing.to_string(),
            ],
        );
        if let Some(Err(e)) = self.next("door_forward") {
            return Err(e);
        }
        Ok(door_of(place).forward_port)
    }
    async fn door_report_daemon_version(&self, place: &str) -> Option<u32> {
        self.call("door_report_daemon_version", &[place]);
        door_of(place).daemon_version
    }
    fn vault(&self) -> IndexMap<String, String> {
        world(|w| w.vault.clone())
    }
    fn broker_ready(&self) -> bool {
        world(|w| w.broker_ready)
    }
    fn has_broker_status(&self) -> bool {
        world(|w| w.broker_has_status)
    }
    /// A scripted `"never"` is a read that never answers.
    async fn broker_status(&self) -> AreaResult<BrokerStatus> {
        self.call("broker_status", &[]);
        match self.next("broker_status") {
            Some(Ok(Value::String(s))) if s == "never" => {
                futures::future::pending::<()>().await;
                unreachable!()
            }
            Some(answer) => answer.map(|v| {
                serde_json::from_value(v).expect("the scripted status reads as the TS writes it")
            }),
            None => Err(failed("no status scripted")),
        }
    }
    async fn broker_codex_claims(&self) -> AreaResult<Option<CodexClaims>> {
        self.call("broker_codex_claims", &[]);
        match self.next("broker_codex_claims") {
            Some(Ok(v)) => Ok(Some(CodexClaims {
                account_id: v["accountId"]
                    .as_str()
                    .expect("scripted claims name an account")
                    .to_string(),
                plan_type: v["planType"].as_str().map(String::from),
                user_id: v["userId"].as_str().map(String::from),
                fedramp: v["fedramp"].as_bool(),
            })),
            Some(Err(e)) => Err(e),
            None => Err(failed("no claims scripted")),
        }
    }
    fn broker_codex_models(&self) -> Option<AccountModels> {
        None
    }
    /// A cloud record with no place or at a provider place.
    fn is_brokered(&self, record: &WorkspaceRecord) -> bool {
        record.kind == wsp_wire::views::workspace_kind::WorkspaceKind::Cloud
            && record
                .place
                .as_ref()
                .map(|p| world(|w| w.provider_places.contains(p)))
                .unwrap_or(true)
    }
    fn codex_broker_of(&self, said: &BrokerSaid) -> Option<CodexBroker> {
        let codex = said.status.as_ref().and_then(|s| s.codex);
        match (codex, &said.claims) {
            (Some(BrokerStatusCodex::Key), _) => Some(CodexBroker {
                mode: BrokerMode::Apikey,
                providers: None,
            }),
            (Some(_), Some(claims)) => Some(CodexBroker {
                mode: BrokerMode::Chatgpt {
                    claims: claims.clone(),
                    models: None,
                },
                providers: None,
            }),
            _ => None,
        }
    }
    fn unbrokered_refusal(&self, harness: &str) -> Option<HostError> {
        unbrokered(harness)
    }
    fn codex_login_unread_refusal(&self) -> HostError {
        codex_unread()
    }
    fn placeholders_of(&self, said: &BrokerSaid, token: &str) -> Placeholders {
        let status = said.status.as_ref();
        let claude_kind = match status.and_then(|s| s.claude) {
            Some(BrokerStatusClaude::Key) => ClaudePlaceholderKind::Key,
            Some(BrokerStatusClaude::Gateway) => ClaudePlaceholderKind::Gateway,
            _ => ClaudePlaceholderKind::Token,
        };
        let vault = if status.and_then(|s| s.codex) == Some(BrokerStatusCodex::Key) {
            IndexMap::from([("OPENAI_API_KEY".to_string(), format!("sk-wsp-{token}"))])
        } else {
            IndexMap::new()
        };
        Placeholders {
            claude_token: token.to_string(),
            claude_kind,
            vault,
        }
    }
    fn brokered_env(&self, token: &str, record: &WorkspaceRecord) -> IndexMap<String, String> {
        let mut env = IndexMap::from([("GH_TOKEN".to_string(), token.to_string())]);
        if let Some(b) = &record.branch_of_record {
            env.insert(
                "WSP_BRANCH_OF_RECORD".to_string(),
                format!("{}/{}@{}", b.owner, b.name, b.branch),
            );
        }
        env
    }
    fn land_on_record(
        &self,
        entry: &LiveWorkspace,
        project: ProjectView,
        lines: Option<Vec<String>>,
    ) -> LocalBoxFuture<'static, AreaResult<()>> {
        self.call(
            "land_on_record",
            &[
                &entry.record.borrow().id,
                &project.id,
                &serde_json::to_string(&lines).unwrap(),
            ],
        );
        let answer = self.next("land_on_record");
        Box::pin(async move {
            match answer {
                Some(Err(e)) => Err(e),
                _ => Ok(()),
            }
        })
    }
    /// The clone as project-landing.ts writes it, less the host command a test never reads: the object it was given
    /// is kept whole, so a test reads what the area passed.
    fn clone_lines(&self, o: Value) -> Vec<String> {
        self.call("clone_lines", &[&serde_json::to_string(&o).unwrap()]);
        let remote = o["remote"].as_str().expect("a remote").to_string();
        let checkout = o["checkout"].as_str().expect("a checkout").to_string();
        let branch = o
            .get("branch")
            .and_then(|b| b.as_str())
            .map(|b| b.to_string());
        let parent = checkout
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default();
        let mut lines = vec![
            "set -e".to_string(),
            format!(
                "mkdir -p {}",
                wsp_wire::types::shell_quote::shell_quote(&parent)
            ),
        ];
        let mut words = vec!["git", "clone"];
        if o.get("brokered").is_some() {
            words.extend(["--depth", "1"]);
        }
        if let Some(b) = &branch {
            words.extend(["--branch", b.as_str()]);
        }
        words.extend(["--", remote.as_str(), checkout.as_str()]);
        lines.push(shell_line(&words));
        lines
    }
    async fn daemon_token_of(&self, machine: &Arc<dyn Machine>) -> AreaResult<Option<String>> {
        self.call("daemon_token_of", &[machine.id()]);
        match self.next("daemon_token_of") {
            Some(Ok(Value::String(s))) => Ok(Some(s)),
            Some(Err(e)) => Err(e),
            _ => Ok(None),
        }
    }
    fn forget_daemon_token(&self, machine_id: &str) {
        self.call("forget_daemon_token", &[machine_id]);
    }
    fn has_deploy(&self) -> bool {
        world(|w| w.deploy)
    }
    async fn deploy_daemon(
        &self,
        machine: &Arc<dyn Machine>,
        token: Option<String>,
    ) -> AreaResult<()> {
        self.call(
            "deploy_daemon",
            &[machine.id(), token.as_deref().unwrap_or("none")],
        );
        Ok(())
    }
    async fn local_daemon_road(&self) -> Option<AreaResult<DaemonReachView>> {
        world(|w| w.local_road.clone()).map(Ok)
    }
    fn machine_exec_opts(&self) -> Option<MachineExecOptions> {
        world(|w| w.machine_exec.clone())
    }
    fn copy_import(
        &self,
        entry: &LiveWorkspace,
        o: ProjectImportOptions,
        _report: ImportReport,
    ) -> LocalBoxFuture<'static, AreaResult<ImportLanded>> {
        let id = entry.record.borrow().id.clone();
        self.call("copy_import", &[&id, &import_words(&o)]);
        let project = WorkspaceProject {
            name: "copied".to_string(),
            dest: o.dest.clone(),
            imported_at: "2025-10-09T08:53:20.000Z".to_string(),
            size: None,
        };
        let result = ProjectImportResult {
            dest: o.dest,
            files: 1.0.into(),
            bytes: 1.0.into(),
            parts: 1.0.into(),
            cut: Vec::new(),
            rewritten: Vec::new(),
            agents: Vec::new(),
            project,
        };
        Box::pin(async move {
            Ok(ImportLanded {
                result,
                done: format!("copied {id}"),
            })
        })
    }
    fn hello_version(&self, entry: &LiveWorkspace) -> LocalBoxFuture<'static, Option<u32>> {
        self.call("hello_version", &[&entry.record.borrow().id]);
        let said = world(|w| w.hello);
        Box::pin(async move { said })
    }
    fn has_local(&self) -> bool {
        world(|w| w.local_wiring.is_some())
    }
    fn local_backend(&self) -> Option<Arc<dyn MachineBackend>> {
        world(|w| {
            w.local_wiring
                .as_ref()
                .map(|_| self.backend.clone() as Arc<dyn MachineBackend>)
        })
    }
    fn local_home(&self, agent: &str) -> String {
        world(|w| {
            w.local_wiring
                .as_ref()
                .map(|l| format!("{}/.{agent}", l.home))
                .unwrap_or_default()
        })
    }
    fn local_home_dir(&self) -> String {
        world(|w| {
            w.local_wiring
                .as_ref()
                .map(|l| l.home_dir.clone())
                .unwrap_or_default()
        })
    }
    fn local_env(&self) -> IndexMap<String, String> {
        world(|w| {
            w.local_wiring
                .as_ref()
                .map(|l| l.env.clone())
                .unwrap_or_default()
        })
    }
    fn local_roots_path(&self) -> Option<String> {
        world(|w| w.local_wiring.as_ref().and_then(|l| l.roots_path.clone()))
    }
    fn has_local_daemon_road(&self) -> bool {
        world(|w| w.local_wiring.as_ref().is_some_and(|l| l.daemon_road))
    }
    fn has_local_restart(&self) -> bool {
        world(|w| w.local_wiring.as_ref().is_some_and(|l| l.restart))
    }
    async fn here_daemon_version(&self) -> Option<AreaResult<Option<u32>>> {
        world(|w| {
            w.local_wiring
                .as_ref()
                .and_then(|l| l.here_daemon)
                .map(|v| Ok(Some(v)))
        })
    }
    fn local_scratch(&self) -> String {
        world(|w| {
            w.local_wiring
                .as_ref()
                .map(|l| l.scratch.clone())
                .unwrap_or_default()
        })
    }
    fn agents_wsp_mcp(&self) -> Option<McpServerSpec> {
        world(|w| w.agents_wsp_mcp.clone())
    }
    fn agents_here_url(&self) -> Option<String> {
        world(|w| w.agents_here_url.clone())
    }
    async fn door_login_user(&self, place: &str) -> AreaResult<String> {
        self.call("door_login_user", &[place]);
        Ok(world(|w| w.login_user.clone()))
    }
    async fn end_thread(
        &self,
        place: &str,
        thread_id: &str,
        o: EndThread,
    ) -> AreaResult<Option<String>> {
        self.call(
            "end_thread",
            &[
                place,
                thread_id,
                &format!("remove={} away={}", o.remove, o.away),
            ],
        );
        Ok(None)
    }
    async fn end_owed(&self, place: &str, thread_id: &str) -> AreaResult<Option<OwedEnd>> {
        self.call("end_owed", &[place, thread_id]);
        Ok(None)
    }
    fn sleep(&self, ms: f64) -> futures::future::LocalBoxFuture<'static, ()> {
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        let _cancel = self.clock.schedule(
            ms,
            Box::new(move || {
                let _ = tx.send(());
            }),
        );
        Box::pin(async move {
            let _ = rx.await;
        })
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}
