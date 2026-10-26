// SPDX-License-Identifier: AGPL-3.0-only
//! Helpers the golden seal and template tests share: wrappers over the recording backend for the hooks its options do
//! not carry, a frame recorder that keeps each frame's `left`, the scripts the tests run for real under bash, and a
//! matcher that reads a wire object the way `toMatchObject` does.
#![allow(dead_code)]
use std::cell::RefCell;
use std::path::Path;
use std::process::{Command, Output};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use wsp_engine::errors::{classify, EngineError, Result, RetryClock};
use wsp_engine::fetch::Fetch;
use wsp_engine::golden::{DeployDaemon, StageFn};
use wsp_engine::machine::{
    BackendOffers, BackendPricing, BaseTemplates, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife,
    MachineOffers, MachineShape, MachineSpec, MachineState, RunOptions, Seen, SnapshotOptions, TemplateRow, Waiting,
};
use wsp_engine::machine::MachineListRow;
use wsp_engine::vault::TarEntry;
use wsp_wire::views::golden_image::{GoldenStage, GoldenStep};
use wsp_words::words::image::BuilderReading;

use crate::golden_roads_common::*;

/// golden-tools.ts `MIB`.
pub const MIB: u64 = 1024 * 1024;

/// golden-tools.ts and tool-sizes.ts `BUILDER_DISK_GB`: the root disk a builder asks for. The Rust crate keeps no such
/// constant, so the value the TS test states is held here.
pub const BUILDER_DISK_GB: f64 = 20.0;

/// catalog signin.ts `NEVER_IN_IMAGE` at 65f6b89c7: the login file, the key file, and each shared login's target
/// (`GUEST_HOME`, `CLAUDE_CONFIG_REL`, `CLAUDE_KEY_FILE` and codex's `shared.target`).
pub const NEVER_IN_IMAGE: [&str; 3] = ["/root/.claude-cfg/.credentials.json", "/root/.claude-cfg/anthropic-api-key", "/root/.codex/auth.json"];

pub fn not_on_road() -> EngineError {
    EngineError::Other { message: "not on this road".to_string(), code: None }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// `tarOf` of golden.test.ts:160: the two files the vault case asks for, from the root.
pub fn vault_tgz() -> Vec<u8> {
    wsp_engine::vault::tar_of(&[
        TarEntry::File { path: "root/.config/gh/hosts.yml".to_string(), mode: 0o600, content: b"github.com:\n".to_vec() },
        TarEntry::File { path: "etc/profile.d/wsp-secrets.sh".to_string(), mode: 0o600, content: b"export A=1\n".to_vec() },
    ])
    .expect("the vault archive")
}

pub fn fetch_of(body: Vec<u8>) -> Arc<dyn Fetch> {
    FetchStub::with_body(body)
}

/// golden.test.ts:1 `FAST_WAIT` in the templates describe (`readyMs: 50, pollMs: 1`).
pub fn fast_wait() -> wsp_engine::golden::TemplateWait {
    wsp_engine::golden::TemplateWait { ready_ms: Some(50), poll_ms: Some(1), on_status: None }
}

pub fn results(pairs: Vec<(&str, ExecResult)>) -> IndexMap<String, ExecResult> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

pub fn template(id: &str, name: &str, status: &str) -> TemplateRow {
    serde_json::from_value(serde_json::json!({ "id": id, "name": name, "status": status })).expect("a template row")
}

pub fn shape(json: Value) -> MachineShape {
    serde_json::from_value(json).expect("a machine shape")
}

pub fn deploy_with(f: impl Fn(Arc<dyn Machine>) -> LocalBoxFuture<'static, Result<Option<String>, EngineError>> + 'static) -> DeployDaemon {
    Arc::new(f)
}

/// A daemon hook that takes 15 ms on the clock it is handed, as golden.test.ts:337's `setTimeout(r, 15)` does.
pub fn sleeping_deploy(clock: Rc<Manual>) -> DeployDaemon {
    deploy_with(move |_m| {
        let clock = clock.clone();
        Box::pin(async move {
            clock.sleep(15).await;
            Ok::<Option<String>, EngineError>(None)
        })
    })
}

/// A machine handle that names its stream the way the TS fake does: a desktop answers `wss://fake/stream/<id>`.
struct MachWrap {
    inner: Arc<dyn Machine>,
    url: Option<String>,
}

#[async_trait(?Send)]
impl Machine for MachWrap {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn kind(&self) -> MachineKind {
        self.inner.kind()
    }
    fn stream_url(&self) -> Option<&str> {
        self.url.as_deref()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.inner.labels()
    }
    fn seen(&self) -> Option<&Seen> {
        self.inner.seen()
    }
    fn offers(&self) -> MachineOffers {
        self.inner.offers()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.inner.exec(cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        self.inner.run(script, opts).await
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        self.inner.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        self.inner.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        self.inner.resume(signal).await
    }
    async fn kill(&self) -> Result<()> {
        self.inner.kill().await
    }
    async fn state(&self) -> Result<MachineState> {
        self.inner.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        self.inner.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        self.inner.upload_url(path).await
    }
    async fn describe(&self) -> Result<MachineShape> {
        self.inner.describe().await
    }
}

fn with_stream(m: Arc<dyn Machine>) -> Arc<dyn Machine> {
    let url = (m.kind() == MachineKind::Desktop).then(|| format!("wss://fake/stream/{}", m.id()));
    Arc::new(MachWrap { inner: m, url })
}

/// The recording backend with the hooks a test sets by hand: a callback when a machine is created, the template offer
/// withdrawn (a backend missing one of the four template calls), and a listing that refuses.
pub struct Wrap {
    pub inner: Arc<dyn MachineBackend>,
    pub on_create: Option<Rc<dyn Fn()>>,
    pub no_templates: bool,
    pub list_fails: bool,
}

impl Wrap {
    pub fn over(rec: &Rec) -> Self {
        Wrap { inner: rec.dyn_backend(), on_create: None, no_templates: false, list_fails: false }
    }
    pub fn arc(self) -> Arc<dyn MachineBackend> {
        Arc::new(self)
    }
}

#[async_trait(?Send)]
impl MachineBackend for Wrap {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        self.inner.pricing()
    }
    fn base_templates(&self) -> Option<BaseTemplates> {
        self.inner.base_templates()
    }
    fn offers(&self) -> BackendOffers {
        let mut offers = self.inner.offers();
        if self.no_templates {
            offers.templates = false;
        }
        offers
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        if let Some(on_create) = &self.on_create {
            on_create();
        }
        Ok(with_stream(self.inner.create(spec, waiting, sent).await?))
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        Ok(with_stream(self.inner.get(id).await?))
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        self.inner.list(labels).await
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.inner.delete_snapshot(id).await
    }
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String> {
        self.inner.promote_snapshot(snapshot_id, name).await
    }
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        self.inner.get_template(id).await
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        if self.list_fails {
            return Err(EngineError::Provider(classify(502, None, Some("upstream unavailable"), None)));
        }
        self.inner.list_templates().await
    }
    async fn delete_template(&self, id: &str) -> Result<()> {
        self.inner.delete_template(id).await
    }
}

/// A machine that answers `exec` by running the command under bash with the given HOME, as golden.test.ts:534's
/// `machine` does: a leading `export PATH=` gets this computer's node folder first.
pub struct Shell {
    pub home: String,
    pub node_dir: String,
}

#[async_trait(?Send)]
impl Machine for Shell {
    fn id(&self) -> &str {
        "shell"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        let script = match cmd.strip_prefix("export PATH=") {
            Some(rest) => format!("export PATH={}:{rest}", self.node_dir),
            None => cmd.to_string(),
        };
        let out = Command::new("bash").args(["-c", &script]).env_clear().env("HOME", &self.home).output().expect("bash");
        Ok(exec_of(&out))
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        Err(not_on_road())
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Err(not_on_road())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Err(not_on_road())
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        Err(not_on_road())
    }
    async fn kill(&self) -> Result<()> {
        Err(not_on_road())
    }
    async fn state(&self) -> Result<MachineState> {
        Err(not_on_road())
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(not_on_road())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Err(not_on_road())
    }
}

/// One frame of the stage listener, with the `left` it carried (none when it carried none).
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub stage: String,
    pub detail: Option<String>,
    pub left: Option<Vec<String>>,
}

pub fn frame_recorder() -> (StageFn, Rc<RefCell<Vec<Frame>>>) {
    let frames: Rc<RefCell<Vec<Frame>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = frames.clone();
    let on_stage: StageFn = Arc::new(move |stage: GoldenStage, detail: Option<&str>, _step: Option<GoldenStep>, left: Option<&[String]>| {
        sink.borrow_mut().push(Frame {
            stage: stage_word(stage).to_string(),
            detail: detail.map(str::to_string),
            left: left.map(<[String]>::to_vec),
        });
    });
    (on_stage, frames)
}

/// The word a reading of the builder carries in the TS: its state, or `unread`.
pub fn reading_word(reading: &BuilderReading) -> String {
    match reading {
        BuilderReading::State(state) => state.clone(),
        BuilderReading::Unread => "unread".to_string(),
    }
}

/// `writeStub`: a script at `path`, executable. The TS links it to a shared runner; a plain file answers the same.
pub fn write_stub(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, script).expect("write a stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod a stub");
}

/// `(statSync(path).mode & 0o777).toString(8)`.
pub fn mode_of(path: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).expect("a path to stat").permissions().mode();
    format!("{:o}", mode & 0o777)
}

/// `Array.prototype.findIndex`: the first line that satisfies `pred`, or -1.
pub fn find_index(lines: &[String], pred: impl Fn(&str) -> bool) -> i64 {
    lines.iter().position(|line| pred(line)).map_or(-1, |i| i as i64)
}

pub fn bash_inherit(script: &str) -> Output {
    Command::new("bash").args(["-c", script]).output().expect("bash")
}

/// `spawnSync("bash", ["-c", script], { env })`: the child sees only the given variables.
pub fn bash_env(script: &str, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("bash");
    cmd.args(["-c", script]).env_clear();
    for (name, value) in env {
        cmd.env(name, value);
    }
    cmd.output().expect("bash")
}

/// A child's answer as exec-detached reads it: its exit code (1 when it has none) and both streams.
pub fn exec_of(out: &Output) -> ExecResult {
    exec(out.status.code().unwrap_or(1) as i64, &String::from_utf8_lossy(&out.stdout), &String::from_utf8_lossy(&out.stderr))
}

/// `execFileSync("sh", ...)`'s stdout of a command line that prints a path.
pub fn dir_of_tool(name: &str) -> String {
    let path = std::env::var_os("PATH").expect("a PATH");
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.parent().map(|dir| dir.display().to_string()))
        .unwrap_or_else(|| panic!("{name} is on this computer's PATH"))
}

fn same_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// `expect(actual).toMatchObject(want)`: every key `want` names is present in the wire object with the same value.
pub fn match_object<T: Serialize>(actual: &T, want: Value) {
    let got = serde_json::to_value(actual).expect("a wire value");
    let Value::Object(want) = want else { panic!("match_object takes an object") };
    for (key, expect) in &want {
        let have = got.get(key).cloned().unwrap_or(Value::Null);
        assert!(same_json(&have, expect), "{key}: want {expect}, got {have}, in {got}");
    }
}

/// `expect(actual).toEqual(want)` for a wire value.
pub fn equal_json<T: Serialize>(actual: &T, want: Value) {
    let got = serde_json::to_value(actual).expect("a wire value");
    assert!(same_json(&got, &want), "want {want}, got {got}");
}

/// `readFileSync(path).trim().split("\n")`.
pub fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).expect("a file to read").trim().split('\n').map(str::to_string).collect()
}

/// `expect(err.message).toThrow(needle)`: the message contains the text.
pub fn assert_message_has(message: &str, needle: &str) {
    assert!(message.contains(needle), "want the message to contain {needle:?}, got {message:?}");
}
