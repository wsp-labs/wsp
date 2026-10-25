// SPDX-License-Identifier: AGPL-3.0-only
//! What the machines slice's tests share sits in the submodules (a fake Boat API, a clock, a warn recorder). What
//! the provision slice's tests share: a machine whose every call is a closure the test hands in, and the box
//! guest (packages/engine/test/box-guest.ts at a4033dc34), a computer somebody owns as a temp directory whose execs
//! and runs are this computer's own bash.
#![allow(dead_code)]
pub mod fake_box;
pub mod fake_clock;
pub mod fake_machine;
pub mod scene;
pub mod warn;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;

use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::{
    BytesLanded, ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions, SnapshotOptions,
};

pub type ExecFn = Arc<dyn Fn(String, ExecOptions) -> LocalBoxFuture<'static, Result<ExecResult>>>;
pub type RunFn = Arc<dyn Fn(String, RunOptions) -> LocalBoxFuture<'static, Result<ExecResult>>>;
pub type UrlFn = Arc<dyn Fn(String) -> LocalBoxFuture<'static, Result<String>>>;
pub type PutFn = Arc<dyn Fn(String, Vec<u8>) -> LocalBoxFuture<'static, Result<Option<BytesLanded>>>>;

pub fn res(exit_code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: exit_code.into(), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

pub fn ok() -> ExecResult {
    res(0, "", "")
}

pub fn other(message: &str) -> EngineError {
    EngineError::Other { message: message.to_string(), code: None }
}

/// A machine whose calls are the closures it holds. `run` falls back to `exec` when not set, as most TS stubs do;
/// `put_bytes` set makes `offers().put_bytes` true, which is the TS's `machine.putBytes !== undefined`.
#[derive(Clone)]
pub struct FakeMachine {
    pub id: String,
    pub exec: ExecFn,
    pub run: Option<RunFn>,
    pub download_url: Option<UrlFn>,
    pub upload_url: Option<UrlFn>,
    pub put_bytes: Option<PutFn>,
}

impl FakeMachine {
    pub fn new(exec: ExecFn) -> Self {
        FakeMachine { id: "spoo".to_string(), exec, run: None, download_url: None, upload_url: None, put_bytes: None }
    }
    /// A machine that answers every exec and run with exit 0 and records each command, in order, in `seen`.
    pub fn recording(seen: Arc<Mutex<Vec<String>>>) -> Self {
        FakeMachine::new(Arc::new(move |cmd, _| {
            seen.lock().unwrap().push(cmd);
            Box::pin(async { Ok(ok()) })
        }))
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
    fn takes_stdin(&self) -> bool {
        true
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { put_bytes: self.put_bytes.is_some(), ..MachineOffers::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        (self.exec)(cmd.to_string(), opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        match &self.run {
            Some(run) => run(script.to_string(), opts).await,
            None => (self.exec)(script.to_string(), ExecOptions::default()).await,
        }
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Ok("snap".to_string())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&wsp_engine::cancel::Cancel>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Err(other("no state on the fake"))
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        match &self.download_url {
            Some(f) => f(path.to_string()).await,
            None => Ok(format!("https://signed.example/dl?path={}", url_encode(path))),
        }
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        match &self.upload_url {
            Some(f) => f(path.to_string()).await,
            None => Ok(format!("https://signed.example/ul?path={}", url_encode(path))),
        }
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], _timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        match &self.put_bytes {
            Some(f) => f(path.to_string(), bytes.to_vec()).await,
            None => Err(other("no byte road on this fake")),
        }
    }
}

/// encodeURIComponent.
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// decodeURIComponent of a `path=` query value.
pub fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

/// The command through a real bash with `stdin` (when given) written and closed, as the daemon does with the field
/// its exec frame carries.
pub async fn bash(cmd: &str, stdin: Option<Vec<u8>>, path_first: Option<&Path>) -> ExecResult {
    use tokio::io::AsyncWriteExt;
    let mut command = tokio::process::Command::new("bash");
    command.arg("-c").arg(cmd);
    if let Some(dir) = path_first {
        command.env("PATH", format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default()));
    }
    command.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = command.spawn().expect("bash runs");
    let mut input = child.stdin.take().unwrap();
    if let Some(bytes) = stdin {
        input.write_all(&bytes).await.unwrap();
    }
    drop(input);
    let out = child.wait_with_output().await.unwrap();
    ExecResult {
        exit_code: out.status.code().map(i64::from).unwrap_or(1).into(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// box-guest.ts: what the guest was asked, in order.
#[derive(Debug, Default)]
pub struct GuestLog {
    pub cmds: Vec<String>,
    pub runs: Vec<String>,
    pub run_unlogged: Vec<bool>,
    pub landed: Vec<String>,
}

/// box-guest.ts:29. A computer somebody owns as a temp directory: every exec and run is bash; the byte road writes the
/// file; `command -v` checks answer `ok <c>` for the names in `present` and `no <c>` otherwise; the df free read
/// answers 3000 MiB in KiB; a uv download answers exit 0 with nothing.
pub struct BoxGuest {
    pub root: PathBuf,
    pub log: Arc<Mutex<GuestLog>>,
    pub machine: FakeMachine,
    _dir: tempfile::TempDir,
}

impl BoxGuest {
    pub fn root_str(&self) -> String {
        self.root.to_str().unwrap().to_string()
    }
    pub fn cmds(&self) -> Vec<String> {
        self.log.lock().unwrap().cmds.clone()
    }
    pub fn runs(&self) -> Vec<String> {
        self.log.lock().unwrap().runs.clone()
    }
    pub fn landed(&self) -> Vec<String> {
        self.log.lock().unwrap().landed.clone()
    }
}

/// box-guest.ts:29 `boxGuest(present)`. The df line is `golden_seam::free_kb_cmd()`'s text, which the test passes in
/// as `free_kb_cmd` so the guest does not reach a golden body itself; `None` answers no df line specially.
pub fn box_guest(present: &[&str], free_kb_cmd: Option<String>) -> BoxGuest {
    let dir = tempfile::Builder::new().prefix("wsp-box-guest-").tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let log = Arc::new(Mutex::new(GuestLog::default()));
    let present: Vec<String> = present.iter().map(|s| s.to_string()).collect();
    let exec_log = log.clone();
    let exec: ExecFn = Arc::new(move |cmd: String, opts: ExecOptions| {
        exec_log.lock().unwrap().cmds.push(cmd.clone());
        let present = present.clone();
        let free = free_kb_cmd.clone();
        Box::pin(async move {
            if cmd.contains("astral-sh/uv/releases") {
                return Ok(ok());
            }
            if free.as_deref() == Some(cmd.as_str()) {
                return Ok(res(0, &format!("{}\n", 3000 * 1024), ""));
            }
            if cmd.contains("command -v") {
                let asked: Vec<String> =
                    cmd.split("command -v '").skip(1).filter_map(|s| s.split('\'').next().map(str::to_string)).collect();
                let out =
                    asked.iter().map(|c| format!("{} {c}", if present.contains(c) { "ok" } else { "no" })).collect::<Vec<_>>().join("\n");
                return Ok(res(0, &out, ""));
            }
            Ok(bash(&cmd, opts.stdin, None).await)
        })
    });
    let run_log = log.clone();
    let exec_for_run = exec.clone();
    let run: RunFn = Arc::new(move |script: String, opts: RunOptions| {
        {
            let mut l = run_log.lock().unwrap();
            l.runs.push(script.clone());
            l.run_unlogged.push(opts.unlogged);
        }
        exec_for_run(script, ExecOptions::default())
    });
    let put_log = log.clone();
    let put: PutFn = Arc::new(move |path: String, bytes: Vec<u8>| {
        put_log.lock().unwrap().landed.push(path.clone());
        Box::pin(async move {
            let p = PathBuf::from(&path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, bytes).unwrap();
            Ok(None)
        })
    });
    let machine = FakeMachine { id: "spoo".to_string(), exec, run: Some(run), download_url: None, upload_url: None, put_bytes: Some(put) };
    BoxGuest { root, log, machine, _dir: dir }
}

/// sha256 hex of a text, as the tests' `digest`.
pub fn digest(text: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// Writes `text` at `root/rel`, parents made.
pub fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// Reads `root/rel` as UTF-8.
pub fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

/// An oracle fixture under tests/fixtures/oracle, as written by oracle/provision.oracle.ts.
pub fn oracle(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle").join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}
