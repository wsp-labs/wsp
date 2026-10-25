// SPDX-License-Identifier: AGPL-3.0-only
//! The two machines the machine road is tested against. `ScriptGuest` is runtime/test/script-guest.ts with the
//! stub backend's machine folded in: the launch, poll, write and reap contract held in memory, a log that grows a step
//! per poll, an exit file and a leader that can die. `LocalGuest` is this computer's bash as the guest
//! (machine-exec.test.ts `localGuest`). Both record every exec's text.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine as _;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use regex::Regex;
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::{ExecOptions, ExecResult, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::Machine;

/// One poll's move of the guest.
#[derive(Clone, Debug, Default)]
pub struct Step {
    pub append: Option<Vec<u8>>,
    pub exit: Option<i32>,
    pub dead: bool,
    /// Work ticks the run's group did since the poll before.
    pub work: Option<i64>,
}

impl Step {
    pub fn none() -> Self {
        Self::default()
    }
    pub fn append(text: impl AsRef<[u8]>) -> Self {
        Self { append: Some(text.as_ref().to_vec()), ..Self::default() }
    }
    pub fn exit(mut self, code: i32) -> Self {
        self.exit = Some(code);
        self
    }
    pub fn only_exit(code: i32) -> Self {
        Self { exit: Some(code), ..Self::default() }
    }
    pub fn dead() -> Self {
        Self { dead: true, ..Self::default() }
    }
    pub fn work(mut self, ticks: i64) -> Self {
        self.work = Some(ticks);
        self
    }
}

/// The scripted guest's own answer to one exec.
pub type Inner = Arc<dyn Fn(&str) -> Result<ExecResult>>;
/// What a test puts in front of the guest (the TS `backend.execImpl = async (m, cmd) => ...`): the command, its
/// input, and the guest's own answer to call or not.
pub type Hook = Arc<dyn Fn(String, Option<Vec<u8>>, Inner) -> LocalBoxFuture<'static, Result<ExecResult>>>;

#[derive(Default)]
struct State {
    log: Vec<u8>,
    exit_file: String,
    alive: bool,
    work: i64,
    child: bool,
    launch: String,
    step: usize,
    steps: Vec<Step>,
    claimed: bool,
    probe_fails: Option<EngineError>,
    probe_garbles: bool,
    probes: u32,
    kills: Vec<String>,
    writes: Vec<String>,
    calls: Vec<String>,
    pieces: HashMap<String, String>,
    disk: IndexMap<String, String>,
    landed: IndexMap<String, String>,
}

pub struct ScriptGuest {
    state: Arc<Mutex<State>>,
    hook: Arc<Mutex<Option<Hook>>>,
}

/// The guest as the `Machine` the factory is handed.
pub struct GuestMachine {
    id: String,
    state: Arc<Mutex<State>>,
    hook: Arc<Mutex<Option<Hook>>>,
}

fn ok(stdout: impl Into<String>) -> Result<ExecResult> {
    Ok(ExecResult { exit_code: super::code(0), stdout: stdout.into(), stderr: String::new() })
}

impl ScriptGuest {
    /// `steps` moves the guest one poll at a time; with none it stays as it is until the test moves it.
    pub fn new(steps: Vec<Step>) -> (Arc<Self>, Arc<dyn Machine>) {
        let state = Arc::new(Mutex::new(State { alive: true, steps, ..State::default() }));
        let hook = Arc::new(Mutex::new(None));
        let guest = Arc::new(Self { state: state.clone(), hook: hook.clone() });
        let machine: Arc<dyn Machine> = Arc::new(GuestMachine { id: "m1".into(), state, hook });
        (guest, machine)
    }

    /// The guest's own answer, as a hook calls it.
    pub fn inner(&self) -> Inner {
        let state = self.state.clone();
        Arc::new(move |cmd: &str| answer(&state, cmd))
    }

    pub fn set_hook(&self, hook: Hook) {
        *self.hook.lock().unwrap() = Some(hook);
    }

    pub fn push_step(&self, step: Step) {
        self.state.lock().unwrap().steps.push(step);
    }

    pub fn kills(&self) -> Vec<String> {
        self.state.lock().unwrap().kills.clone()
    }
    pub fn writes(&self) -> Vec<String> {
        self.state.lock().unwrap().writes.clone()
    }
    pub fn calls(&self) -> Vec<String> {
        self.state.lock().unwrap().calls.clone()
    }
    /// Every poll exec so far.
    pub fn polls(&self) -> Vec<String> {
        self.calls().into_iter().filter(|c| c.contains("__WSP_EOF_")).collect()
    }
    pub fn get_script(&self) -> String {
        at(&self.state.lock().unwrap(), ".sh")
    }
    pub fn get_launch(&self) -> String {
        self.state.lock().unwrap().launch.clone()
    }
    pub fn get_input(&self) -> String {
        at(&self.state.lock().unwrap(), ".in")
    }
    pub fn exit(&self, code: i32) {
        self.state.lock().unwrap().exit_file = code.to_string();
    }
    pub fn append(&self, text: &str) {
        self.state.lock().unwrap().log.extend_from_slice(text.as_bytes());
    }
    pub fn child_alive(&self) -> bool {
        self.state.lock().unwrap().child
    }
    pub fn files(&self) -> Vec<String> {
        self.state.lock().unwrap().disk.keys().cloned().collect()
    }
    /// The guest swept the run's files with no reader of its own around.
    pub fn sweep(&self) {
        self.state.lock().unwrap().claimed = false;
    }
    /// The machine stops answering the attach's one question.
    pub fn refuse_probes(&self, e: EngineError) {
        self.state.lock().unwrap().probe_fails = Some(e);
    }
    /// The machine answers, but neither that it holds the run nor that it does not.
    pub fn garble_probes(&self) {
        self.state.lock().unwrap().probe_garbles = true;
    }
    pub fn probes(&self) -> u32 {
        self.state.lock().unwrap().probes
    }
}

fn at(s: &State, suffix: &str) -> String {
    s.landed.iter().find(|(p, _)| p.ends_with(suffix)).map(|(_, t)| t.clone()).unwrap_or_default()
}

fn land(s: &mut State, cmd: &str) {
    let re = Regex::new(r"(?:printf %s '([A-Za-z0-9+/=]*)'|cat '([^']*)'\.\{0\.\.(\d+)\}) \| base64 -d (>>?) '([^']*)'").unwrap();
    let found: Vec<(String, bool, String)> = re
        .captures_iter(cmd)
        .map(|m| {
            let b64 = match m.get(1) {
                Some(b) => b.as_str().to_string(),
                None => {
                    let base = &m[2];
                    let n: usize = m[3].parse().unwrap();
                    (0..=n).map(|i| s.pieces.get(&format!("{base}.{i}")).cloned().unwrap_or_default()).collect()
                }
            };
            let text = String::from_utf8_lossy(&base64::engine::general_purpose::STANDARD.decode(b64).unwrap_or_default()).into_owned();
            (m[5].to_string(), &m[4] == ">>", text)
        })
        .collect();
    for (path, appends, text) in found {
        let whole = if appends { at(s, &path) + &text } else { text };
        s.disk.insert(path.clone(), whole.clone());
        s.landed.insert(path, whole);
    }
}

fn first_capture(re: &str, cmd: &str) -> Option<String> {
    Regex::new(re).unwrap().captures(cmd).map(|c| c[1].to_string())
}

fn answer(state: &Mutex<State>, cmd: &str) -> Result<ExecResult> {
    let mut s = state.lock().unwrap();
    s.calls.push(cmd.to_string());
    if cmd.ends_with("echo WSP_PIECE") {
        let m = Regex::new(r"printf %s '([A-Za-z0-9+/=]*)' > '([^']*)'\.(\d+) \|\| exit 1\n").unwrap().captures(cmd).map(|c| (c[1].to_string(), c[2].to_string(), c[3].to_string()));
        let (b64, base, n) = m.unwrap_or_else(|| panic!("piece exec without a numbered file: {cmd}"));
        s.pieces.insert(format!("{base}.{n}"), b64);
        return ok("WSP_PIECE\n");
    }
    if cmd.contains("WSP_LAUNCHED") {
        s.launch = cmd.to_string();
        land(&mut s, cmd);
        s.child = true;
        s.claimed = true;
        return ok("WSP_LAUNCHED\n");
    }
    if cmd.contains("WSP_RUN") {
        s.probes += 1;
        if let Some(e) = s.probe_fails.clone() {
            return Err(e);
        }
        if s.probe_garbles {
            return Ok(ExecResult { exit_code: super::code(1), stdout: String::new(), stderr: "bash: line 1: unexpected".into() });
        }
        let channel = if cmd.contains(".in") { at(&s, ".in") } else { String::new() };
        return ok(if s.claimed { format!("WSP_RUN\n{channel}") } else { "WSP_GONE\n".into() });
    }
    let signals = !cmd.contains("echo WSP_CTX") && (cmd.contains("kill -KILL") || cmd.contains("kill -TERM"));
    let rm = first_capture(r"rm -rf '([^']+)'\.\*", cmd);
    if signals || rm.is_some() {
        if signals {
            s.kills.push(cmd.to_string());
        }
        if signals && cmd.contains("-- -$P") {
            s.child = false;
        }
        if signals && !cmd.contains(".tail") {
            s.alive = false;
        }
        if let Some(base) = rm {
            s.claimed = false;
            let prefix = format!("{base}.");
            s.disk.retain(|p, _| !p.starts_with(&prefix));
        }
        if let Some(mark) = first_capture(r"(__WSP_REAPED_[a-z0-9]+__)", cmd) {
            let from: usize = first_capture(r"tail -c \+(\d+)", cmd).map(|n| n.parse::<usize>().unwrap() - 1).unwrap_or(0);
            let end = (from + 262_144).min(s.log.len());
            let chunk = if from < s.log.len() { base64::engine::general_purpose::STANDARD.encode(&s.log[from..end]) } else { String::new() };
            return ok(format!("{chunk}\n{mark} {}\n", s.log.len()));
        }
        return ok("");
    }
    if cmd.contains("echo WSP_OK") {
        s.writes.push(cmd.to_string());
        if !s.exit_file.is_empty() {
            return ok("WSP_GONE\n");
        }
        land(&mut s, cmd);
        return ok("WSP_OK\n");
    }
    if let Some(sentinel) = first_capture(r"(__WSP_EOF_[a-z0-9]+__)", cmd) {
        let i = s.step;
        if let Some(step) = s.steps.get(i).cloned() {
            s.step += 1;
            if let Some(a) = step.append {
                s.log.extend_from_slice(&a);
            }
            if let Some(code) = step.exit {
                s.exit_file = code.to_string();
            }
            if step.dead {
                s.alive = false;
            }
            s.work += step.work.unwrap_or(0);
        }
        let from: usize = first_capture(r"tail -c \+(\d+)", cmd).map(|n| n.parse::<usize>().unwrap() - 1).unwrap_or(0);
        let end = (from + 262_144).min(s.log.len());
        let chunk = if from < s.log.len() { base64::engine::general_purpose::STANDARD.encode(&s.log[from..end]) } else { String::new() };
        let asked = cmd.contains("awk -v p=");
        let work = if asked { s.work.to_string() } else { String::new() };
        return ok(format!("{chunk}\n{sentinel} {} {} {work}\n", s.exit_file, if s.alive { "up" } else { "down" }));
    }
    panic!("guest got unexpected command: {cmd}");
}

#[async_trait(?Send)]
impl Machine for GuestMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let hook = self.hook.lock().unwrap().clone();
        let state = self.state.clone();
        match hook {
            Some(h) => h(cmd.to_string(), opts.stdin, Arc::new(move |c: &str| answer(&state, c))).await,
            None => answer(&state, cmd),
        }
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        panic!("the machine road never runs")
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        panic!("not on this road")
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        panic!("not on this road")
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        panic!("not on this road")
    }
    async fn kill(&self) -> Result<()> {
        panic!("not on this road")
    }
    async fn state(&self) -> Result<MachineState> {
        panic!("not on this road")
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
}

/// This computer's bash as the guest. `stdin` makes it a machine whose exec hands the command its input (a computer
/// you own); without it the input is dropped (a provider's exec). `shell` is bash text run before every command.
pub struct LocalGuest {
    pub id: String,
    pub stdin: bool,
    pub shell: String,
    pub calls: Arc<Mutex<Vec<String>>>,
}

impl LocalGuest {
    /// The guest, its run folder under a fresh temporary folder, and the folder (kept until the guard drops).
    pub fn new(stdin: bool, shell: &str) -> (Arc<Self>, String, tempfile::TempDir) {
        let dir = tempfile::Builder::new().prefix("wsp-machine-exec-").tempdir().expect("a temp folder");
        let run_dir = dir.path().join("run").to_string_lossy().into_owned();
        (Arc::new(Self { id: "local".into(), stdin, shell: shell.to_string(), calls: Arc::new(Mutex::new(Vec::new())) }), run_dir, dir)
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait(?Send)]
impl Machine for LocalGuest {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn takes_stdin(&self) -> bool {
        self.stdin
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.calls.lock().unwrap().push(cmd.to_string());
        let script = format!("{}{}{}", super::linux_shell_prelude(), self.shell, cmd);
        let mut child = tokio::process::Command::new("bash")
            .args(["-c", &script])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("bash");
        {
            use tokio::io::AsyncWriteExt;
            let mut input = child.stdin.take().unwrap();
            if self.stdin {
                if let Some(bytes) = opts.stdin {
                    let _ = input.write_all(&bytes).await;
                }
            }
            drop(input);
        }
        let out = child.wait_with_output().await.expect("bash's output");
        Ok(ExecResult { exit_code: super::code(out.status.code().unwrap_or(1) as i64), stdout: String::from_utf8_lossy(&out.stdout).into_owned(), stderr: String::from_utf8_lossy(&out.stderr).into_owned() })
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        panic!("the machine road never runs")
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        panic!("not on this road")
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        panic!("not on this road")
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        panic!("not on this road")
    }
    async fn kill(&self) -> Result<()> {
        panic!("not on this road")
    }
    async fn state(&self) -> Result<MachineState> {
        panic!("not on this road")
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
}

/// A machine in front of another whose every exec goes through `f` (the TS `{ ...machine, exec: ... }`).
pub struct Wrapped {
    pub inner: Arc<dyn Machine>,
    pub f: Arc<dyn Fn(Arc<dyn Machine>, String, ExecOptions) -> LocalBoxFuture<'static, Result<ExecResult>>>,
}

#[async_trait(?Send)]
impl Machine for Wrapped {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn kind(&self) -> MachineKind {
        self.inner.kind()
    }
    fn takes_stdin(&self) -> bool {
        self.inner.takes_stdin()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        (self.f)(self.inner.clone(), cmd.to_string(), opts).await
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        panic!("the machine road never runs")
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        panic!("not on this road")
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        panic!("not on this road")
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        panic!("not on this road")
    }
    async fn kill(&self) -> Result<()> {
        panic!("not on this road")
    }
    async fn state(&self) -> Result<MachineState> {
        panic!("not on this road")
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        panic!("not on this road")
    }
}

/// The errors the TS tests throw, as the engine's variants.
pub mod errors {
    use wsp_engine::errors::{EngineError, ErrorKind, RoadCause, RoadError, WspError};

    /// `new TypeError("fetch failed", { cause: <code> })`: nothing answered, the name never resolved.
    pub fn fetch_failed(code: &str) -> EngineError {
        EngineError::Road(RoadError {
            name: "TypeError".into(),
            message: "fetch failed".into(),
            code: None,
            cause: Some(RoadCause { message: format!("getaddrinfo {code} api.example"), code: Some(code.into()), errors: None }),
        })
    }
    /// `Object.assign(new Error("read ECONNRESET"), { code: "ECONNRESET" })`: sent, and the answer lost.
    pub fn reset() -> EngineError {
        EngineError::Other { message: "read ECONNRESET".into(), code: Some("ECONNRESET".into()) }
    }
    /// A plain `new Error(message)`.
    pub fn plain(message: &str) -> EngineError {
        EngineError::Other { message: message.into(), code: None }
    }
    /// `Object.assign(new Error(message), { kind, status })`: a refusal the far end answered.
    pub fn refusal(kind: ErrorKind, status: u16, message: &str) -> EngineError {
        EngineError::Provider(WspError { kind, status, code: None, message: message.into(), request_id: None, window: None, said: None })
    }
    /// `new PlaceAbsentError(sentence)`.
    pub fn absent(sentence: &str) -> EngineError {
        EngineError::PlaceAbsent { message: sentence.into() }
    }
    /// `new MachineUnreachableError(id, said, status, line)`.
    pub fn unreachable(machine_id: &str, said: &str, status: u16, line: &str) -> EngineError {
        EngineError::MachineUnreachable { machine_id: machine_id.into(), said: said.into(), status, line: line.into(), exec_failed: false }
    }
    /// `new ExecFailedError(id, said, status)`, its line given.
    pub fn exec_failed(machine_id: &str, said: &str, status: u16, line: &str) -> EngineError {
        EngineError::MachineUnreachable { machine_id: machine_id.into(), said: said.into(), status, line: line.into(), exec_failed: true }
    }
}
