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
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_engine::errors::EngineError;
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineLife, MachineListRow, MachineSpec, MachineState,
    RunOptions, SnapshotOptions, Waiting,
};
use wsp_machines::machines::core_types::*;
use wsp_machines::machines::kinds::{BrokerSaid, FolderComputer, KindsArea, KindsCtx};
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::views::golden_image::MachineKind;
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_words::exit::{HostError, Kind};

use super::*;

/// A host error with these words and this kind, built whole so no fake leans on a constructor under test.
pub fn host_error(message: &str, kind: Option<Kind>) -> HostError {
    HostError { message: message.to_string(), fix: None, kind, errors: None, warnings: None, left: None, marks: Vec::new() }
}

/// What the fake broker says of an agent it adds no key for.
pub fn unbrokered(harness: &str) -> Option<HostError> {
    (harness != "claude" && harness != "codex").then(|| host_error(&format!("{harness} does not run on a cloud machine yet: wsp keeps no key there"), Some(Kind::Usage)))
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
        Arc::new(LocalShell { cwd: cwd.to_path_buf(), path_first: path_first.map(|p| p.to_path_buf()), log: Mutex::new(Vec::new()) })
    }
    /// Every command run, in order.
    pub fn ran(&self) -> Vec<String> {
        self.log.lock().unwrap().iter().map(|(c, _)| c.clone()).collect()
    }
    /// The timeout each command was run with, in order.
    pub fn timeouts(&self) -> Vec<Option<u64>> {
        self.log.lock().unwrap().iter().map(|(_, t)| *t).collect()
    }
}

fn not_here<T>() -> Result<T, EngineError> {
    Err(engine_error("the local shell does this no way"))
}

#[async_trait]
impl Machine for LocalShell {
    fn id(&self) -> &str {
        "local"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult, EngineError> {
        self.log.lock().unwrap().push((cmd.to_string(), opts.timeout_ms));
        let mut c = Command::new("sh");
        c.arg("-c").arg(cmd).current_dir(&self.cwd);
        if let Some(first) = &self.path_first {
            c.env("PATH", format!("{}:{}", first.display(), std::env::var("PATH").unwrap_or_default()));
        }
        c.stdin(if opts.stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = c.spawn().map_err(|e| engine_error(&e.to_string()))?;
        if let Some(bytes) = &opts.stdin {
            let mut stdin = child.stdin.take().expect("a piped stdin");
            stdin.write_all(bytes).map_err(|e| engine_error(&e.to_string()))?;
        }
        let out = child.wait_with_output().map_err(|e| engine_error(&e.to_string()))?;
        let code = out.status.code().unwrap_or(-1) as i64;
        Ok(ExecResult { exit_code: code.into(), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() })
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        self.exec(script, ExecOptions::default()).await
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        not_here()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        not_here()
    }
    async fn resume(&self, _signal: Option<tokio_util::sync::CancellationToken>) -> Result<(), EngineError> {
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

#[async_trait]
impl MachineBackend for PlainBackend {
    fn capabilities(&self) -> Capabilities {
        let mut caps = boat_capabilities();
        caps.images = self.images;
        caps.pause_mode = self.disk.then_some(wsp_wire::wire::capabilities::PauseMode::Disk);
        caps
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FakePricing)
    }
    fn names_workspace(&self) -> bool {
        self.names
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>, EngineError> {
        not_here()
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        not_here()
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>, EngineError> {
        Ok(Vec::new())
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<(), EngineError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The world the context reads, one per test thread.

/// What the place door says of one joined computer.
#[derive(Clone, Default)]
pub struct Door {
    pub name: String,
    pub backend: Option<Arc<dyn MachineBackend>>,
    pub folder: Option<FolderComputer>,
    pub joined: bool,
    pub logins: Option<String>,
    /// Keys the door's picks take out of the vault; none means the door answers nothing for the vault.
    pub vault_drops: Option<Vec<String>>,
    pub sign_ins: Option<IndexMap<String, String>>,
    pub github_from_vault: bool,
    pub daemon_version: Option<f64>,
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
    pub machine_exec: Option<Map<String, Value>>,
    pub hello: Option<f64>,
    /// The local wiring; `None` is a host that wired no local road.
    pub local_wiring: Option<LocalWiring>,
    pub agents_wsp_mcp: Option<Value>,
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
    pub here_daemon: Option<f64>,
    pub scratch: String,
}

impl World {
    fn fresh() -> World {
        World {
            state_path: None,
            local: None,
            default_dir: None,
            released: Arc::new(PlainBackend { images: false, disk: false, names: false }),
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
            local_wiring: Some(LocalWiring { home: "/home/me".to_string(), home_dir: "/home/me".to_string(), env: IndexMap::new(), roots_path: None, daemon_road: true, restart: false, here_daemon: None, scratch: "/tmp/wsp-local".to_string() }),
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

/// A joined computer the door holds, its folder computer on a fake machine at this home.
pub fn joined_door(name: &str, home: &str, tools: bool) -> (Door, Arc<FakeMachine>) {
    let machine = FakeMachine::new(&format!("{name}-machine"));
    let door = Door {
        name: name.to_string(),
        folder: Some(FolderComputer { machine: machine.clone(), home: home.to_string(), shape: json!({ "cpu": 2, "memMb": 4096 }), tools }),
        joined: true,
        forward_port: 41_000,
        ..Door::default()
    };
    (door, machine)
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
    let report: StageReport = Rc::new(move |stage, line, _| keep.borrow_mut().push((stage.to_string(), line.to_string())));
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
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

impl CloneRig {
    pub fn new() -> CloneRig {
        let origin_dir = tempfile::tempdir().unwrap();
        let origin = real(origin_dir.path());
        let out = Command::new("git").args(["init", "-q"]).arg(&origin).output().expect("git runs");
        assert!(out.status.success());
        std::fs::write(origin.join("README.md"), "spoo\n").unwrap();
        git_in(&origin, &["add", "."]);
        git_in(&origin, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "one"]);
        let bin = tempfile::tempdir().unwrap();
        let log = bin.path().join("calls.log");
        let real_git = String::from_utf8(Command::new("sh").args(["-c", "command -v git"]).output().unwrap().stdout).unwrap().trim().to_string();
        let record = |name: &str| format!(r#"printf '%s\t%s\n' "${{GIT_TERMINAL_PROMPT:-unset}}" "$(printf '%s\037' {name} "$@")" >> '{}'"#, log.display());
        // An url naming "missing" fails the way a clone of a repo that is not there does, and one naming "private"
        // the way a signed-out one does; a clone that fails leaves behind the folder above its own that it made.
        let pick = r#"case "$URL" in *missing*|*private*) mkdir -p "$(dirname "$4")";; esac; case "$URL" in *missing*) echo "fatal: repository '$URL' does not exist" >&2; exit 128;; *private*) echo "remote: Repository not found." >&2; echo "fatal: repository '$URL' not found" >&2; exit 128;; esac"#;
        let clone = format!(r#"exec '{}' clone -q '{}' "$4""#, real_git, origin.display());
        let git = format!("#!/bin/sh\n{}\nif [ \"$1\" = clone ]; then URL=\"$3\"; {pick}; {clone}; fi\nexec '{}' \"$@\"\n", record("git"), real_git);
        let gh = format!("#!/bin/sh\n{}\nURL=\"$3\"; {pick}; {clone}\n", record("gh"));
        for (name, text) in [("git", git), ("gh", gh)] {
            let path = bin.path().join(name);
            std::fs::write(&path, text).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        CloneRig { origin, bin, log, folders: RefCell::new(Vec::new()), _origin: origin_dir }
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
        let Ok(text) = std::fs::read_to_string(&self.log) else { return Vec::new() };
        text.trim()
            .lines()
            .map(|line| {
                let (prompt, argv) = line.split_once('\t').expect("a tab between the prompt and the argv");
                Run { prompt: prompt.to_string(), argv: argv.split('\u{1f}').filter(|w| !w.is_empty()).map(|w| w.to_string()).collect() }
            })
            .collect()
    }

    /// The last word of every clone run, which is the folder it was told to clone into.
    pub fn clone_targets(&self) -> Vec<String> {
        self.calls().into_iter().filter(|c| c.argv.get(1).map(|w| w == "clone").unwrap_or(false) || c.argv[0] == "gh").map(|c| c.argv.last().unwrap().clone()).collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The context.

fn door_of(place: &str) -> Door {
    world(|w| w.doors.get(place).cloned().unwrap_or_else(|| Door { name: place.to_string(), ..Door::default() }))
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
        world(|w| w.place_names.get(place).cloned().unwrap_or_else(|| place.to_string()))
    }
    fn wired_place(&self) -> String {
        world(|w| w.wired.clone())
    }
    fn project_held(&self, id: &str) -> ProjectView {
        world(|w| w.projects.get(id).cloned()).unwrap_or_else(|| panic!("the test held no project {id}"))
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        match record.worktree.as_ref().and_then(|t| t["path"].as_str()) {
            Some(path) => path.to_string(),
            None => self.project_held(&record.project).path,
        }
    }
    fn computer_of(&self, entry: &Live) -> String {
        let place = entry.record.borrow().place.clone();
        self.place_name(&place.unwrap_or_else(|| self.wired_place()))
    }
    fn served_by_its_computer(&self, entry: &Live) -> bool {
        world(|w| w.served.contains(&entry.id()))
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
        door_of(place).folder
    }
    fn door_joined(&self, place: &str) -> bool {
        world(|w| w.place_door) && door_of(place).joined
    }
    fn door_logins_of(&self, place: &str) -> Option<String> {
        door_of(place).logins
    }
    fn door_vault_at(&self, place: &str, vault: &IndexMap<String, String>) -> Option<IndexMap<String, String>> {
        door_of(place).vault_drops.map(|drops| vault.iter().filter(|(k, _)| !drops.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect())
    }
    fn door_sign_ins_at(&self, place: &str) -> Option<IndexMap<String, String>> {
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
            None => Ok(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() }),
        }
    }
    async fn door_forward(&self, place: &str, port: u16, pane: Value, standing: bool) -> AreaResult<u16> {
        self.call("door_forward", &[place, &port.to_string(), &serde_json::to_string(&pane).unwrap(), &standing.to_string()]);
        if let Some(Err(e)) = self.next("door_forward") {
            return Err(e);
        }
        Ok(door_of(place).forward_port)
    }
    async fn door_report_daemon_version(&self, place: &str) -> Option<f64> {
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
    async fn broker_status(&self) -> AreaResult<Value> {
        self.call("broker_status", &[]);
        match self.next("broker_status") {
            Some(Ok(Value::String(s))) if s == "never" => {
                futures::future::pending::<()>().await;
                unreachable!()
            }
            Some(answer) => answer,
            None => Err(failed("no status scripted")),
        }
    }
    async fn broker_codex_claims(&self) -> AreaResult<Value> {
        self.call("broker_codex_claims", &[]);
        self.next("broker_codex_claims").unwrap_or_else(|| Err(failed("no claims scripted")))
    }
    /// A cloud record with no place or at a provider place.
    fn is_brokered(&self, record: &WorkspaceRecord) -> bool {
        record.kind == wsp_wire::views::workspace_kind::WorkspaceKind::Cloud && record.place.as_ref().map(|p| world(|w| w.provider_places.contains(p))).unwrap_or(true)
    }
    fn codex_broker_of(&self, said: &BrokerSaid) -> Option<Value> {
        let codex = said.status.as_ref().and_then(|s| s["codex"].as_str());
        match (codex, &said.claims) {
            (Some("key"), _) => Some(json!({ "mode": "apikey" })),
            (Some(_), Some(claims)) => Some(json!({ "mode": "chatgpt", "claims": claims })),
            _ => None,
        }
    }
    fn unbrokered_refusal(&self, harness: &str) -> Option<HostError> {
        unbrokered(harness)
    }
    fn codex_login_unread_refusal(&self) -> HostError {
        codex_unread()
    }
    fn placeholders_of(&self, said: &BrokerSaid, token: &str) -> (Value, IndexMap<String, String>) {
        let status = said.status.as_ref();
        let kind = match status.and_then(|s| s["claude"].as_str()) {
            Some(k @ ("key" | "gateway")) => k,
            _ => "token",
        };
        let vault = if status.and_then(|s| s["codex"].as_str()) == Some("key") { IndexMap::from([("OPENAI_API_KEY".to_string(), format!("sk-wsp-{token}"))]) } else { IndexMap::new() };
        (json!({ "token": token, "kind": kind }), vault)
    }
    fn brokered_env(&self, token: &str, record: &WorkspaceRecord) -> IndexMap<String, String> {
        let mut env = IndexMap::from([("GH_TOKEN".to_string(), token.to_string())]);
        if let Some(b) = &record.branch_of_record {
            env.insert("WSP_BRANCH_OF_RECORD".to_string(), format!("{}/{}@{}", b.owner, b.name, b.branch));
        }
        env
    }
    async fn land_on_record(&self, entry: &Live, project: &ProjectView, lines: Option<Vec<String>>) -> AreaResult<()> {
        self.call("land_on_record", &[&entry.id(), &project.id, &serde_json::to_string(&lines).unwrap()]);
        if let Some(Err(e)) = self.next("land_on_record") {
            return Err(e);
        }
        Ok(())
    }
    /// The clone as project-landing.ts writes it, less the host command a test never reads: the object it was given
    /// is kept whole, so a test reads what the area passed.
    fn clone_lines(&self, o: Value) -> Vec<String> {
        self.call("clone_lines", &[&serde_json::to_string(&o).unwrap()]);
        let remote = o["remote"].as_str().expect("a remote").to_string();
        let checkout = o["checkout"].as_str().expect("a checkout").to_string();
        let branch = o.get("branch").and_then(|b| b.as_str()).map(|b| b.to_string());
        let parent = checkout.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        let mut lines = vec!["set -e".to_string(), format!("mkdir -p {}", wsp_wire::types::shell_quote::shell_quote(&parent))];
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
    async fn deploy_daemon(&self, machine: &Arc<dyn Machine>, token: Option<String>) -> AreaResult<()> {
        self.call("deploy_daemon", &[machine.id(), token.as_deref().unwrap_or("none")]);
        Ok(())
    }
    async fn local_daemon_road(&self) -> Option<AreaResult<DaemonReachView>> {
        world(|w| w.local_road.clone()).map(Ok)
    }
    fn machine_exec_opts(&self) -> Option<Map<String, Value>> {
        world(|w| w.machine_exec.clone())
    }
    async fn copy_import(&self, entry: &Live, o: Value, _report: Rc<dyn Fn(&str, &str)>) -> AreaResult<Value> {
        self.call("copy_import", &[&entry.id(), &serde_json::to_string(&o).unwrap()]);
        Ok(json!({ "copied": entry.id() }))
    }
    async fn hello_version(&self, entry: &Live) -> Option<f64> {
        self.call("hello_version", &[&entry.id()]);
        world(|w| w.hello)
    }
    async fn bundler_plan(&self, o: &Value) -> AreaResult<Value> {
        self.call("bundler_plan", &[&serde_json::to_string(o).unwrap()]);
        match self.next("bundler_plan") {
            Some(answer) => answer,
            None => Ok(json!({ "files": 12, "bytes": 3456 })),
        }
    }
    fn has_local(&self) -> bool {
        world(|w| w.local_wiring.is_some())
    }
    fn local_backend(&self) -> Option<Arc<dyn MachineBackend>> {
        world(|w| w.local_wiring.as_ref().map(|_| self.backend.clone() as Arc<dyn MachineBackend>))
    }
    fn local_home(&self, agent: &str) -> String {
        world(|w| w.local_wiring.as_ref().map(|l| format!("{}/.{agent}", l.home)).unwrap_or_default())
    }
    fn local_home_dir(&self) -> String {
        world(|w| w.local_wiring.as_ref().map(|l| l.home_dir.clone()).unwrap_or_default())
    }
    fn local_env(&self) -> IndexMap<String, String> {
        world(|w| w.local_wiring.as_ref().map(|l| l.env.clone()).unwrap_or_default())
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
    async fn here_daemon_version(&self) -> Option<AreaResult<Option<f64>>> {
        world(|w| w.local_wiring.as_ref().and_then(|l| l.here_daemon).map(|v| Ok(Some(v))))
    }
    fn local_scratch(&self) -> String {
        world(|w| w.local_wiring.as_ref().map(|l| l.scratch.clone()).unwrap_or_default())
    }
    fn agents_wsp_mcp(&self) -> Option<Value> {
        world(|w| w.agents_wsp_mcp.clone())
    }
    fn agents_here_url(&self) -> Option<String> {
        world(|w| w.agents_here_url.clone())
    }
    async fn door_login_user(&self, place: &str) -> AreaResult<String> {
        self.call("door_login_user", &[place]);
        Ok(world(|w| w.login_user.clone()))
    }
    async fn end_thread(&self, place: &str, thread_id: &str, o: Value) -> AreaResult<Value> {
        self.call("end_thread", &[place, thread_id, &o.to_string()]);
        Ok(Value::Null)
    }
    async fn end_owed(&self, place: &str, thread_id: &str) -> AreaResult<Value> {
        self.call("end_owed", &[place, thread_id]);
        Ok(Value::Null)
    }
    fn sleep(&self, ms: f64) -> futures::future::LocalBoxFuture<'static, ()> {
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        let _cancel = self.clock.schedule(ms, Box::new(move || {
            let _ = tx.send(());
        }));
        Box::pin(async move {
            let _ = rx.await;
        })
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}
