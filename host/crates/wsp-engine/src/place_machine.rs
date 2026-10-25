// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/place-machine.ts at a4033dc34. Spec: spec/place_machine.md.
//!
//! The computer itself as something the engine can run steps on: a command, a long command and bytes over the exec
//! frame its daemon already serves, and every call that belongs to a workspace refused in one sentence.
use std::sync::Arc;

use crate::cancel::Cancel;
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::sync::OnceCell;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::daemon::{DaemonExecReply, EXEC_TIMEOUT_MAX_MS};
use wsp_wire::wire::helpers::base64_length;
use wsp_words::agents_report::login_path_refusal;
use wsp_words::js::utf16;
use wsp_words::project_path::place_provision_paths;
use wsp_words::thread_cgroup::cgroup_join_line;
use wsp_words::thread_shell::{thread_shell_files, thread_shell_vars};

use crate::errors::{EngineError, Result};
use crate::exec_detached::{exec_detached_in, machine_answer, INLINE_EXEC_MS};
use crate::link_backend::{daemon_reply, obj, other, read, split_op, LinkAsk, MachineLink, LINK_MARGIN_MS};
use crate::machine::{
    BackendOffers, BackendPricing, BytesLanded, Capabilities, ExecOptions, ExecResult, Framing, Lifecycle, Machine, MachineBackend,
    MachineKind, MachineLife, MachineListRow, MachineOffers, MachineState, RunOptions, Size, SnapshotOptions, SnapshotStoragePricing,
    Waiting,
};
use crate::target_line::{as_login, login_shell_path, target_login, GivenLogin, TargetLogin};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::wire::machine_link::MachineSpec;
use wsp_wire::wire::machine_link::SnapshotStoragePricing as WireSnapshotStoragePricing;

/// place-machine.ts:17.
pub const NOT_A_WORKSPACE: &str = "this is the computer itself, not a workspace on it";

/// place-machine.ts:23. How much of a file one frame carries.
pub const PLACE_PART_BYTES: usize = 1024 * 1024;

/// place-machine.ts:28. The slowest this road has been measured at, in bytes a millisecond.
pub const LINK_BYTES_PER_MS: u64 = 213;

/// place-machine.ts:33. What one part's frame asks for; `floor_ms` `None` is `INLINE_EXEC_MS`.
pub fn place_part_bound_ms(bytes: usize, floor_ms: Option<u64>) -> u64 {
    let base = base64_length(bytes as u64).div_ceil(LINK_BYTES_PER_MS);
    (2 * base).max(floor_ms.unwrap_or(INLINE_EXEC_MS)).min(EXEC_TIMEOUT_MAX_MS as u64)
}

/// place-machine.ts:40. Writes a folder's shell startup files and names them in the launch's environment.
pub fn shell_start_line(dir: &str) -> String {
    let files: Vec<String> = thread_shell_files(dir, &IndexMap::new())
        .into_iter()
        .map(|(name, text)| {
            let at = shell_quote(&format!("{dir}/{name}"));
            format!("printf %s {} > {at}.$$ && mv -f {at}.$$ {at}", shell_quote(&text))
        })
        .collect();
    let vars = thread_shell_vars(dir);
    format!(
        "{{ mkdir -p {} && {}; }} 2>/dev/null && export ZDOTDIR={} BASH_ENV={}; ",
        shell_quote(dir),
        files.join(" && "),
        shell_quote(&vars.zdotdir),
        shell_quote(&vars.bash_env)
    )
}

/// node's `path.posix.dirname` for the folder a part is written into.
fn posix_dirname(p: &str) -> String {
    let trimmed = p.trim_end_matches('/');
    let has_root = p.starts_with('/');
    match trimmed.rfind('/').filter(|&at| at >= 1) {
        Some(1) if has_root => "//".into(),
        Some(at) => trimmed[..at].to_string(),
        None if has_root => "/".into(),
        None => ".".into(),
    }
}

/// What every command on a computer carries in front of it, and what an upload is refused with where the login's
/// PATH inside that leaves no room for it. `prefix` is the text inside the login's line before the command (the PATH
/// a folder names); `cgroup` is the line before the login's line that moves the shell into the thread's cgroup.
pub struct LoginFraming {
    prefix: Option<String>,
    login: TargetLogin,
    cgroup: Option<String>,
}

impl Framing for LoginFraming {
    fn wrap(&self, cmd: &str) -> String {
        let wrapped = as_login(&self.login, &format!("{}{cmd}", self.prefix.as_deref().unwrap_or("")));
        match &self.cgroup {
            Some(join) => format!("{join}\n{wrapped}"),
            None => wrapped,
        }
    }
    fn no_room(&self) -> Option<String> {
        self.login.path.as_ref().map(|path| login_path_refusal(utf16::len(path) as f64))
    }
}

/// Refused in one sentence: what a call that belongs to a workspace answers on the computer itself.
fn refused<T>() -> Result<T> {
    Err(other(NOT_A_WORKSPACE))
}

/// The name of every part a file's upload writes, as a shell brace list.
fn part_names(path: &str, parts: usize) -> String {
    format!("{}.part{{0..{}}}", shell_quote(path), parts - 1)
}

/// The file in parts over `m`'s own exec, joined on the computer and taken away; a failure removes every part name.
/// The exec is `m`'s, so a folder's parts, join and cleanup ride as its login.
async fn put_bytes_over(m: &dyn Machine, id: &str, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
    let parts = bytes.len().div_ceil(PLACE_PART_BYTES).max(1);
    let put = hex::encode(rand::random::<[u8; 6]>());
    match land_parts(m, id, path, bytes, timeout_ms, &put, parts).await {
        Ok(()) => Ok(Some(BytesLanded { pieces: parts as u32 })),
        Err(e) => {
            let _ = m
                .exec(&format!("rm -f {}", part_names(path, parts)), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() })
                .await;
            Err(e)
        }
    }
}

async fn land_parts(m: &dyn Machine, id: &str, path: &str, bytes: &[u8], timeout_ms: Option<u64>, put: &str, parts: usize) -> Result<()> {
    let at = shell_quote(path);
    let names = part_names(path, parts);
    let folder = shell_quote(&posix_dirname(path));
    for i in 0..parts {
        let start = i * PLACE_PART_BYTES;
        let part = &bytes[start..((i + 1) * PLACE_PART_BYTES).min(bytes.len())];
        let line = ["umask 077".to_string(), format!("mkdir -p {folder}"), format!("cat > {at}.part{i}")].join("\n");
        let res = m
            .exec(
                &line,
                ExecOptions {
                    timeout_ms: Some(place_part_bound_ms(part.len(), timeout_ms)),
                    idempotency_key: Some(format!("{put}/{i}")),
                    stdin: Some(part.to_vec()),
                },
            )
            .await?;
        if res.exit_code.get() != 0 {
            return Err(other(format!("part {} of {parts} did not land at {path} on {id}: {}", i + 1, machine_answer(&res))));
        }
    }
    let join = format!("umask 077; if [ -e {at}.part0 ]; then cat {names} > {at} && rm -f {names}; fi");
    let joined = m.exec(&join, ExecOptions { timeout_ms, idempotency_key: Some(format!("{put}/join")), stdin: None }).await?;
    if joined.exit_code.get() != 0 {
        return Err(other(format!("{path} did not land on {id}: {}", machine_answer(&joined))));
    }
    Ok(())
}

/// One computer you own, driven over the link its daemon holds to this host: one exec frame per command, a
/// detached run under wsp's own folder there for anything longer, and bytes in parts on the frames' own stdin.
/// Nothing here pauses, snapshots or kills it: it is the computer the person is keeping, not a machine wsp made.
pub struct PlaceMachine {
    link: Arc<dyn MachineLink>,
    id: String,
    home: String,
}

impl PlaceMachine {
    /// place-machine.ts:64.
    pub fn new(link: Arc<dyn MachineLink>, id: &str, home: &str) -> Self {
        Self { link, id: id.to_string(), home: home.to_string() }
    }

    /// place-machine.ts:75. The daemon's own exec frame, its timeout held under the cap the daemon holds its timer to.
    async fn frame_exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let timeout = opts.timeout_ms.unwrap_or(INLINE_EXEC_MS).min(EXEC_TIMEOUT_MAX_MS as u64);
        let mut params = obj(json!({ "cmd": cmd, "timeoutMs": timeout }));
        if let Some(stdin) = &opts.stdin {
            params.insert("stdin".to_string(), Value::String(STANDARD.encode(stdin)));
        }
        let ask = LinkAsk { timeout_ms: Some(timeout + LINK_MARGIN_MS), idempotency_key: opts.idempotency_key };
        let reply = read::<DaemonExecReply>(self.link.request("exec", params, ask).await?)?;
        Ok(ExecResult { exit_code: reply.exit_code, stdout: reply.stdout, stderr: reply.stderr })
    }
}

#[async_trait(?Send)]
impl Machine for PlaceMachine {
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
        MachineOffers { put_bytes: true, ..Default::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.frame_exec(cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        exec_detached_in(self, script, opts, &place_provision_paths(&self.home).run_dir).await
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        refused()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        refused()
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        refused()
    }
    async fn kill(&self) -> Result<()> {
        refused()
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        refused()
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        refused()
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        put_bytes_over(self, &self.id, path, bytes, timeout_ms).await
    }
}

/// The computer a thread in a folder on it runs on: every command runs as the owner of the home the computer was
/// joined with, in that home, as targetLogin reads it once at the first command.
pub struct PlaceFolderMachine {
    base: PlaceMachine,
    home: String,
    path: Option<String>,
    login: OnceCell<TargetLogin>,
}

impl PlaceFolderMachine {
    /// place-machine.ts:164.
    pub fn new(link: Arc<dyn MachineLink>, id: &str, home: &str, path: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            base: PlaceMachine::new(link, id, home),
            home: home.to_string(),
            path: path.map(str::to_string),
            login: OnceCell::new(),
        })
    }

    /// place-machine.ts:174. Who every command runs as, read once; a read that failed is read again next time.
    pub async fn login_of(&self) -> Result<TargetLogin> {
        self.login
            .get_or_try_init(|| async {
                let given = GivenLogin { home: Some(self.home.clone()), path: self.path.clone() };
                let at = target_login(&self.base, &given).await?;
                Ok::<_, EngineError>(match login_shell_path(&self.base, &at).await {
                    None => at,
                    Some(path) => TargetLogin { path: Some(path), ..at },
                })
            })
            .await
            .cloned()
    }

    /// The framing every command here runs under, read off the login.
    async fn login_framing(&self, prefix: Option<String>, cgroup: Option<String>) -> Result<LoginFraming> {
        Ok(LoginFraming { prefix, login: self.login_of().await?, cgroup })
    }

    /// place-machine.ts:208. This computer as a turn's launch reaches it: each line in the thread's cgroup, with
    /// `path_first` in front of the login's PATH for those lines alone.
    pub fn in_cgroup(self: &Arc<Self>, cgroup: &str, path_first: Option<&str>) -> Arc<dyn Machine> {
        let prefix = path_first.map(|path| format!("{}export PATH={}:\"$PATH\"; ", shell_start_line(path), shell_quote(path)));
        Arc::new(CgroupFolder { folder: self.clone(), join: cgroup_join_line(cgroup), prefix })
    }
}

#[async_trait(?Send)]
impl Machine for PlaceFolderMachine {
    fn id(&self) -> &str {
        self.base.id()
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn takes_stdin(&self) -> bool {
        true
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { framed: true, put_bytes: true, daemon_frame: true, ..Default::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let wrapped = self.login_framing(None, None).await?.wrap(cmd);
        self.base.frame_exec(&wrapped, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        exec_detached_in(self, script, opts, &place_provision_paths(&self.home).run_dir).await
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        self.base.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        self.base.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        self.base.resume(signal).await
    }
    /// A folder's record going takes nothing off the computer: the computer and the folder both stay.
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        self.base.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        self.base.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        self.base.upload_url(path).await
    }
    async fn framed(&self) -> Result<Arc<dyn Framing>> {
        Ok(Arc::new(self.login_framing(None, None).await?))
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        put_bytes_over(self, self.base.id(), path, bytes, timeout_ms).await
    }
    /// One daemon frame answered by that computer's own daemon, naming no machine.
    async fn daemon_frame(&self, frame: Map<String, Value>) -> Result<Map<String, Value>> {
        let (op, params) = split_op(frame);
        Ok(daemon_reply(&*self.base.link, &op, params).await)
    }
}

/// place-machine.ts:208's `Object.create(this)` with `framed` and `exec` replaced: a folder's machine whose every
/// command is a line of a thread's cgroup, and whose parts, run and daemon frames go through that same framing.
struct CgroupFolder {
    folder: Arc<PlaceFolderMachine>,
    join: String,
    prefix: Option<String>,
}

impl CgroupFolder {
    async fn framing(&self) -> Result<LoginFraming> {
        self.folder.login_framing(self.prefix.clone(), Some(self.join.clone())).await
    }
}

#[async_trait(?Send)]
impl Machine for CgroupFolder {
    fn id(&self) -> &str {
        self.folder.id()
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn takes_stdin(&self) -> bool {
        true
    }
    fn offers(&self) -> MachineOffers {
        self.folder.offers()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let wrapped = self.framing().await?.wrap(cmd);
        self.folder.base.frame_exec(&wrapped, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        exec_detached_in(self, script, opts, &place_provision_paths(&self.folder.home).run_dir).await
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        self.folder.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        self.folder.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        self.folder.resume(signal).await
    }
    async fn kill(&self) -> Result<()> {
        self.folder.kill().await
    }
    async fn state(&self) -> Result<MachineState> {
        self.folder.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        self.folder.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        self.folder.upload_url(path).await
    }
    async fn framed(&self) -> Result<Arc<dyn Framing>> {
        Ok(Arc::new(self.framing().await?))
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        put_bytes_over(self, self.folder.base.id(), path, bytes, timeout_ms).await
    }
    async fn daemon_frame(&self, frame: Map<String, Value>) -> Result<Map<String, Value>> {
        self.folder.daemon_frame(frame).await
    }
}

/// The pricing a folder on a computer somebody joined carries: nothing costs anything, and the shape is the default.
struct FolderPricing {
    shape: Size,
}

impl BackendPricing for FolderPricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        self.shape
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing { free_gb: JsNumber(0.0), usd_per_gb_month: JsNumber(0.0), billed_from: String::new() },
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

/// The backend a folder on a joined computer stands on: that computer as one machine, which wsp never makes.
struct PlaceFolderBackend {
    machine: Arc<dyn Fn() -> Arc<dyn Machine>>,
    shape: Size,
}

#[async_trait(?Send)]
impl MachineBackend for PlaceFolderBackend {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: false,
            pause_mode: None,
            replaces_machine: false,
            preview_urls: false,
            signed_urls: false,
            callback_relay: false,
            disk_snapshots: false,
            images: false,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            kept: true,
            copies: false,
            own_network: false,
            sizes: Vec::new(),
            sizes_last: false,
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FolderPricing { shape: self.shape })
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        None
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers::default()
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        refused()
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        Ok((self.machine)())
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        Ok(Vec::new())
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        refused()
    }
}

/// place-machine.ts:236. The backend a folder on a joined computer stands on: that computer as one machine that is always
/// there, which wsp never makes, pauses, sizes or images.
pub fn place_folder_backend(machine: Arc<dyn Fn() -> Arc<dyn Machine>>, shape: Size) -> Arc<dyn MachineBackend> {
    Arc::new(PlaceFolderBackend { machine, shape })
}
