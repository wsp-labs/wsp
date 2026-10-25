// SPDX-License-Identifier: AGPL-3.0-only
//! ssh-backend.ts:617-969 at a4033dc34. Spec: spec/ssh_machine.md.
//!
//! Root over a login that is not root, the one read that adopts a machine, and the machine and backend a host holds
//! for a computer it reaches over ssh.
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cancel::Cancel;
use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use wsp_catalog::catalog::catalog_agents;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::limits::is_plain_path;
use wsp_words::js::{is_js_whitespace, js_trim, to_number};

use crate::errors::Result;
use crate::machine::{
    BytesLanded, ExecOptions, ExecResult, Machine, MachineFacts, MachineKind, MachineLife, MachineOffers, MachineShape, MachineState,
    RunOptions, SnapshotOptions,
};
use crate::machine_facts::{
    arch_of, mem_mb_of, os_name_of, read_values, system_of, uptime_ms_of, ARCH_READ, HOME_READ, MEM_READ, OS_READ, SHELL_READ, SYSTEM_READ,
    UPTIME_READ,
};
use crate::ssh_backend::dial::{ssh_client, utf16_tail, SshReach, SshRide, SshTransport, SshTransportOptions, SSH_LINE_CAP};
use crate::ssh_backend::keys::{
    client_words, known_host_key, known_hosts_written, local_run, offered_host_key, plain, ssh_host_name, ssh_refusal_line,
    KnownHostsEntry, OfferedKey, SshHostKeyReader, SshLocalRun, SSH_DIAL_MS,
};

/// ssh-backend.ts:618.
pub const SUDO_ASKS_LINE: &str = "a password is required";

/// ssh-backend.ts:622. How a login reaches root there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshSudo {
    Root,
    Free,
    Taken,
    Asks,
    Wrong,
    Tty,
    None,
}

/// ssh-backend.ts:625. The read of a login answers one of root, free, asks, tty, none (never taken or wrong).
pub type SshSudoRoad = SshSudo;

/// ssh-backend.ts:639.
pub const SSH_SUDO_READ: &str = concat!(
    "if [ \"$(id -u)\" -eq 0 ]; then echo WSP_SUDO root\n",
    "elif ! command -v sudo >/dev/null 2>&1; then echo WSP_SUDO none\n",
    "elif said=$(LC_ALL=C sudo -n -v 2>&1 && LC_ALL=C sudo -n -l bash -c true 2>&1); then echo WSP_SUDO free\n",
    "else case \"$said\" in *\"must have a tty\"*) echo WSP_SUDO tty ;; *) echo WSP_SUDO asks ;; esac; fi",
);

/// ssh-backend.ts:648.
pub const SSH_SUDO_TRY: &str = "LC_ALL=C sudo -S -k -p '' bash -c 'echo WSP_SUDO taken'";

/// The words a failed read says about the machine, as the TS cuts them: ssh's own words where it said any, else what
/// the machine printed, trimmed, and the last 300 units of either.
fn said_tail(res: &ExecResult) -> String {
    let words = client_words(&res.stderr);
    utf16_tail(&if words.is_empty() { js_trim(&res.stdout).to_string() } else { words }, SSH_LINE_CAP)
}

/// ssh-backend.ts:652's line for a login that answered with no mark.
fn did_not_answer(reach: &SshReach, res: &ExecResult) -> String {
    format!("{}@{} did not answer over ssh: {}", reach.user, reach.host, said_tail(res))
}

/// The mark a line of the read's output is, read the way a multiline regex reads one: a line is the text between
/// line terminators, and it must be the mark and nothing more.
fn sudo_mark(line: &str) -> Option<SshSudo> {
    match line {
        "WSP_SUDO root" => Some(SshSudo::Root),
        "WSP_SUDO free" => Some(SshSudo::Free),
        "WSP_SUDO asks" => Some(SshSudo::Asks),
        "WSP_SUDO tty" => Some(SshSudo::Tty),
        "WSP_SUDO none" => Some(SshSudo::None),
        _ => None,
    }
}

/// ssh-backend.ts:652. How the login reaches root, read as the login with nothing of the person's sent. Throws ssh's own
/// line where the login does not stand.
pub async fn read_ssh_sudo(reach: &SshReach, transport: &SshTransport) -> Result<SshSudoRoad> {
    let opts = SshTransportOptions { timeout_ms: Some(SSH_DIAL_MS), ride: SshRide::Login, ..Default::default() };
    let read = transport(reach.clone(), SSH_SUDO_READ.to_string(), opts).await;
    let said = read.stdout.split(['\n', '\r', '\u{2028}', '\u{2029}']).find_map(sudo_mark);
    match said {
        Some(road) => Ok(road),
        None if read.exit_code.get() == 255 => Err(plain(ssh_refusal_line(&read, reach))),
        None => Err(plain(did_not_answer(reach, &read))),
    }
}

/// ssh-backend.ts:660. One try of the person's password; answers taken, wrong, tty or none.
pub async fn try_ssh_sudo(reach: &SshReach, password: &str, transport: &SshTransport) -> SshSudo {
    let opts = SshTransportOptions {
        timeout_ms: Some(SSH_DIAL_MS),
        ride: SshRide::Login,
        stdin: Some(format!("{password}\n").into_bytes()),
        ..Default::default()
    };
    let tried = transport(reach.clone(), SSH_SUDO_TRY.to_string(), opts).await;
    if tried.stdout.contains("WSP_SUDO taken") {
        SshSudo::Taken
    } else if tried.stderr.contains("must have a tty") {
        SshSudo::Tty
    } else if tried.stderr.contains("incorrect password attempt") {
        SshSudo::Wrong
    } else {
        SshSudo::None
    }
}

/// ssh-backend.ts:670. The store variable each harness reads, by name, in catalog order.
pub fn ssh_store_vars() -> Vec<String> {
    catalog_agents().into_iter().filter_map(|agent| agent.state_home_env.clone()).filter(|name| is_store_name(name)).collect()
}

/// `^[A-Z_][A-Z0-9_]*$`: the shape a name must have to be read into a shell line.
fn is_store_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_uppercase() || c == '_')
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// ssh-backend.ts:677. What the read asks the machine's own shell for.
pub fn login_read() -> String {
    let mut parts = vec![r#"printf "path %s\n" "$PATH""#.to_string()];
    parts.extend(ssh_store_vars().iter().map(|name| format!(r#"printf "store:{name} %s\n" "${name}""#)));
    parts.join("; ")
}

/// ssh-backend.ts:687. What one dial reads off a machine before its record exists.
pub fn ssh_read_script() -> String {
    [
        HOME_READ.to_string(),
        SYSTEM_READ.to_string(),
        ARCH_READ.to_string(),
        r#"printf "user %s\n" "$(id -un)""#.to_string(),
        SHELL_READ.to_string(),
        r#"printf "shell %s\n" "$shell""#.to_string(),
        format!("env -u BASH_ENV bash --noprofile --norc -c {} 2>/dev/null", shell_quote(&login_read())),
        r#"printf "cpu %s\n" "$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 0)""#.to_string(),
        MEM_READ.to_string(),
    ]
    .join("\n")
}

/// ssh-backend.ts:705. What the machine answers about itself every time a status is built.
pub fn ssh_facts_script() -> String {
    OS_READ.iter().chain(UPTIME_READ.iter()).copied().chain(std::iter::once(HOME_READ)).collect::<Vec<_>>().join("\n")
}

/// ssh-backend.ts:35. The machine's login environment: the kept store folders in `ssh_store_vars` order, then `HOME`,
/// `USER`, `PATH`.
pub type SshLogin = IndexMap<String, String>;

/// ssh-backend.ts:709's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct SshMachineRead {
    pub login: SshLogin,
    pub shape: MachineShape,
    pub system: Option<String>,
    pub arch: Option<String>,
    pub shell: Option<String>,
}

/// ssh-backend.ts:709. One dial that both proves the machine answers and records what wsp needs of it.
pub async fn read_ssh_machine(reach: &SshReach, transport: &SshTransport) -> Result<SshMachineRead> {
    let opts = SshTransportOptions { timeout_ms: Some(30_000), ..Default::default() };
    let res = transport(reach.clone(), ssh_read_script(), opts).await;
    if res.exit_code.get() != 0 {
        return Err(plain(did_not_answer(reach, &res)));
    }
    let values = read_values(&res.stdout);
    let home = values.get("home").map(String::as_str);
    let home = match home {
        Some(home) if !home.is_empty() && is_plain_path(home) => home,
        _ => return Err(plain(home_refusal(reach, home))),
    };
    let mut login = SshLogin::new();
    for name in ssh_store_vars() {
        if let Some(folder) = values.get(&format!("store:{name}")).filter(|folder| !folder.is_empty() && is_plain_path(folder)) {
            login.insert(name, folder.clone());
        }
    }
    login.insert("HOME".to_string(), home.to_string());
    login.insert("USER".to_string(), values.get("user").cloned().unwrap_or_else(|| reach.user.clone()));
    login.insert("PATH".to_string(), plain_path(values.get("path").map(String::as_str)));
    let cpu = to_number(values.get("cpu").map_or("0", String::as_str));
    let mem_mb = mem_mb_of(&values).unwrap_or(0) as f64;
    Ok(SshMachineRead {
        login,
        shape: MachineShape { cpu: Some(JsNumber(cpu)), mem_mb: Some(JsNumber(mem_mb)), disk_gb: None, created_at: None, used_bytes: None },
        system: system_of(&values),
        arch: arch_of(&values),
        shell: values.get("shell").filter(|shell| !shell.is_empty()).cloned(),
    })
}

/// ssh-backend.ts:709's refusal for a home the machine did not name as a plain path.
fn home_refusal(reach: &SshReach, home: Option<&str>) -> String {
    let said = match home {
        None | Some("") => "no home folder".to_string(),
        Some(home) => format!("{}, which is not a plain path", serde_json::to_string(home).unwrap_or_default()),
    };
    format!("{}@{} answered over ssh with {said} for {}", reach.user, reach.host, reach.user)
}

/// Loopback names and addresses, and the suffix a Mac gives its own name on the local network: what a dial that
/// names the computer wsp runs on looks like.
const LOOPBACK_NAMES: [&str; 5] = ["localhost", "127.0.0.1", "::1", "[::1]", "0.0.0.0"];

/// One trailing dot taken off a name, as `.replace(/\.$/, "")` takes it.
fn strip_dot(name: &str) -> String {
    name.strip_suffix('.').unwrap_or(name).to_string()
}

/// ssh-backend.ts:748. Whether this dial reaches the computer wsp is running on: the same machine as the local workspace,
/// under another name. `names` is what this computer answers to.
pub fn ssh_dials_this_computer(reach: &SshReach, names: &[String]) -> bool {
    let host = strip_dot(&reach.host.to_lowercase());
    if LOOPBACK_NAMES.contains(&host.as_str()) || host.starts_with("127.") {
        return true;
    }
    names
        .iter()
        .map(|name| strip_dot(&name.to_lowercase()))
        .filter(|name| !name.is_empty())
        .any(|name| host == name || host == format!("{name}.local") || format!("{host}.local") == name)
}

/// ssh-backend.ts:757.
pub const DEFAULT_REMOTE_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// ssh-backend.ts:764. The machine's login PATH with every folder held to the plain-path rule.
pub fn plain_path(path: Option<&str>) -> String {
    let kept: Vec<&str> = path
        .unwrap_or("")
        .split(':')
        .filter(|folder| !folder.is_empty() && is_plain_path(folder) && !folder.chars().any(is_js_whitespace))
        .collect();
    if kept.is_empty() {
        DEFAULT_REMOTE_PATH.to_string()
    } else {
        kept.join(":")
    }
}

/// ssh-backend.ts:770.
pub const SSH_BYTES_OK: &str = "WSP_BYTES_OK";

/// Node's `path.posix.dirname`: the path with its last segment and the slashes before it taken off.
fn posix_dirname(path: &str) -> String {
    let bytes = path.as_bytes();
    if bytes.is_empty() {
        return ".".to_string();
    }
    let has_root = bytes[0] == b'/';
    let mut end = None;
    let mut matched_slash = true;
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'/' {
            if !matched_slash {
                end = Some(i);
                break;
            }
        } else {
            matched_slash = false;
        }
    }
    match end {
        None if has_root => "/".to_string(),
        None => ".".to_string(),
        Some(1) if has_root => "//".to_string(),
        Some(end) => path[..end].to_string(),
    }
}

/// ssh-backend.ts:777. The script that takes a file's bytes off the connection's stdin.
pub fn put_bytes_script(path: &str, size: usize, tmp: &str) -> String {
    let tmp_q = shell_quote(tmp);
    [
        "set -e".to_string(),
        "umask 077".to_string(),
        format!("mkdir -p {}", shell_quote(&posix_dirname(path))),
        format!("cat > {tmp_q}"),
        format!("[ \"$(wc -c < {tmp_q} | tr -d ' ')\" = {size} ] || {{ rm -f {tmp_q}; echo WSP_BYTES_SHORT; exit 1; }}"),
        format!("mv -f {tmp_q} {}", shell_quote(path)),
        format!("echo {SSH_BYTES_OK}"),
    ]
    .join("\n")
}

/// Where a machine says why its facts cannot be read (the TS's `console.warn`).
pub type WarnSink = Arc<dyn Fn(&str)>;

/// ssh-backend.ts:795. A machine reached over ssh.
pub struct SshMachine {
    pub reach: SshReach,
    id: String,
    transport: SshTransport,
    /// ssh-backend.ts:801. The last words warned about this machine failing to say what it is.
    quiet: Mutex<Option<String>>,
    warn: WarnSink,
}

/// The current time in milliseconds since the epoch, as `Date.now()` answers.
fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl SshMachine {
    /// ssh-backend.ts:803. Warnings go to stderr.
    pub fn new(reach: SshReach, transport: SshTransport) -> Result<Self> {
        Self::with_warn(reach, transport, Arc::new(|line: &str| eprintln!("{line}")))
    }

    /// The same with a warning sink of the caller's.
    pub fn with_warn(reach: SshReach, transport: SshTransport, warn: WarnSink) -> Result<Self> {
        let id = crate::ssh_backend::dial::ssh_machine_id(&reach)?;
        Ok(Self { reach, id, transport, quiet: Mutex::new(None), warn })
    }
}

#[async_trait(?Send)]
impl Machine for SshMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { put_bytes: true, describe: true, facts: true, ..Default::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let opts = SshTransportOptions { timeout_ms: opts.timeout_ms, ..Default::default() };
        Ok((self.transport)(self.reach.clone(), cmd.to_string(), opts).await)
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        let opts = SshTransportOptions { timeout_ms: Some(opts.deadline_ms), on_line: opts.on_line, ..Default::default() };
        Ok((self.transport)(self.reach.clone(), script.to_string(), opts).await)
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Err(plain("a machine reached over ssh cannot be snapshotted".to_string()))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Err(plain("a machine reached over ssh cannot be paused".to_string()))
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Err(plain("a machine reached over ssh cannot be resumed".to_string()))
    }
    /// The machine is the person's own and wsp never made it, so nothing here ends it.
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(plain("a machine reached over ssh serves no signed download URL; its files are read over the connection".to_string()))
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Err(plain("a machine reached over ssh serves no signed upload URL; its files are written over the connection".to_string()))
    }
    async fn describe(&self) -> Result<MachineShape> {
        Ok(read_ssh_machine(&self.reach, &self.transport).await?.shape)
    }
    /// What this machine says it is, read over the connection on every status: nothing here is remembered. A machine that
    /// answers with none of it leaves the rows waiting, and its reason goes to the warning sink once per change.
    async fn facts(&self) -> Result<MachineFacts> {
        let opts = SshTransportOptions { timeout_ms: Some(10_000), ..Default::default() };
        let res = (self.transport)(self.reach.clone(), ssh_facts_script(), opts).await;
        let values = if res.exit_code.get() == 0 { read_values(&res.stdout) } else { IndexMap::new() };
        let os = os_name_of(&values);
        let uptime_ms = uptime_ms_of(&values, now_ms());
        let folder = values.get("home").cloned();
        match (os, uptime_ms, folder) {
            (Some(os), Some(uptime_ms), Some(folder)) if !folder.is_empty() => {
                *self.quiet.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
                Ok(MachineFacts { os, uptime_ms: JsNumber(uptime_ms as f64), folder })
            }
            _ => {
                let why = format!(
                    "{}@{} did not say what it is over ssh (exit {}): {}",
                    self.reach.user,
                    self.reach.host,
                    res.exit_code.get(),
                    said_tail(&res)
                );
                let mut quiet = self.quiet.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if quiet.as_deref() != Some(why.as_str()) {
                    (self.warn)(&why);
                }
                *quiet = Some(why.clone());
                Err(plain(why))
            }
        }
    }
    /// The machine's own road for bytes, which is the connection itself: the file rides stdin under a script that
    /// writes it, since nothing here mints a URL anything could PUT to.
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        let tmp = format!("{path}.wsp-in-{}", hex::encode(rand::random::<[u8; 6]>()));
        let opts = SshTransportOptions { stdin: Some(bytes.to_vec()), timeout_ms, ..Default::default() };
        let res = (self.transport)(self.reach.clone(), put_bytes_script(path, bytes.len(), &tmp), opts).await;
        if res.exit_code.get() != 0 || !res.stdout.contains(SSH_BYTES_OK) {
            return Err(plain(format!(
                "{} bytes did not land at {path} over ssh (exit {}): {}",
                bytes.len(),
                res.exit_code.get(),
                said_tail(&res)
            )));
        }
        Ok(None)
    }
}

type SshReader<T> = Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, T>>;

/// ssh-backend.ts:881. Every seam the backend reads through; `None` takes the real one.
#[derive(Clone, Default)]
pub struct SshBackendOptions {
    pub transport: Option<SshTransport>,
    pub host_key: Option<SshHostKeyReader>,
    pub known_hosts: Option<SshReader<KnownHostsEntry>>,
    pub offered_key: Option<SshReader<OfferedKey>>,
    pub host_name: Option<SshReader<Option<String>>>,
}

/// ssh-backend.ts:896. What every script one backend carries rides with: the login's sudo password, or the login itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshRiding {
    SudoPassword(String),
    AsLogin,
}

/// ssh-backend.ts:937's answer.
pub struct SshAdopted {
    pub machine: Arc<SshMachine>,
    pub login: SshLogin,
    pub shape: MachineShape,
    pub system: Option<String>,
    pub arch: Option<String>,
    pub shell: Option<String>,
    pub host_key: Option<String>,
}

/// ssh-backend.ts:900. The reads a host makes of a computer over ssh before it joins it.
#[derive(Clone)]
pub struct SshBackend {
    transport: SshTransport,
    host_key: SshHostKeyReader,
    known_hosts: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, KnownHostsEntry>>,
    offered: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, OfferedKey>>,
    host_name: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, Option<String>>>,
}

impl SshBackend {
    /// ssh-backend.ts:907.
    pub fn new(opts: SshBackendOptions) -> Self {
        let run: SshLocalRun = local_run();
        let host_key: SshHostKeyReader = {
            let run = run.clone();
            Arc::new(move |reach: SshReach| {
                let run = run.clone();
                async move { known_host_key(&reach, &run).await }.boxed_local()
            })
        };
        let known_hosts: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, KnownHostsEntry>> = {
            let run = run.clone();
            Arc::new(move |reach: SshReach| {
                let run = run.clone();
                async move { known_hosts_written(&reach, &run).await }.boxed_local()
            })
        };
        let offered: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, OfferedKey>> = {
            let run = run.clone();
            Arc::new(move |reach: SshReach| {
                let run = run.clone();
                async move { offered_host_key(&reach, &run).await }.boxed_local()
            })
        };
        let host_name: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, Option<String>>> = {
            let run = run.clone();
            Arc::new(move |reach: SshReach| {
                let run = run.clone();
                async move { ssh_host_name(&reach, &run).await }.boxed_local()
            })
        };
        Self {
            transport: opts.transport.unwrap_or_else(ssh_client),
            host_key: opts.host_key.unwrap_or(host_key),
            known_hosts: opts.known_hosts.unwrap_or(known_hosts),
            offered: opts.offered_key.unwrap_or(offered),
            host_name: opts.host_name.unwrap_or(host_name),
        }
    }

    /// ssh-backend.ts:916. How the login reaches root there, read as the login with nothing of the person's sent.
    pub async fn sudo_for(&self, reach: &SshReach) -> Result<SshSudoRoad> {
        read_ssh_sudo(reach, &self.transport).await
    }

    /// ssh-backend.ts:921. One try of the person's password on a login whose sudo asks for one.
    pub async fn sudo_try(&self, reach: &SshReach, password: &str) -> SshSudo {
        try_ssh_sudo(reach, password, &self.transport).await
    }

    /// ssh-backend.ts:927. The same backend with every script it carries, and every machine it adopts, riding `riding`.
    /// A script already marked as the login passes untouched, and an inner wrapper runs last, so the first password wins.
    pub fn riding(&self, riding: SshRiding) -> SshBackend {
        let base = self.transport.clone();
        let transport: SshTransport = Arc::new(move |reach: SshReach, script: String, opts: SshTransportOptions| {
            let ride = match (&opts.ride, &riding) {
                (SshRide::Login, _) => None,
                (_, SshRiding::AsLogin) => Some(SshRide::Login),
                (_, SshRiding::SudoPassword(password)) => Some(SshRide::RootByPassword(password.clone())),
            };
            let opts = match ride {
                Some(ride) => SshTransportOptions { ride, ..opts },
                None => opts,
            };
            base(reach, script, opts)
        });
        SshBackend {
            transport,
            host_key: self.host_key.clone(),
            known_hosts: self.known_hosts.clone(),
            offered: self.offered.clone(),
            host_name: self.host_name.clone(),
        }
    }

    /// ssh-backend.ts:937. Reads the machine, then the key this computer's client holds for it. The read's failure ends
    /// the call before any key is read.
    pub async fn adopt(&self, reach: &SshReach) -> Result<SshAdopted> {
        let read = read_ssh_machine(reach, &self.transport).await?;
        let host_key = self.key_for(reach).await;
        Ok(SshAdopted {
            machine: Arc::new(SshMachine::new(reach.clone(), self.transport.clone())?),
            login: read.login,
            shape: read.shape,
            system: read.system,
            arch: read.arch,
            shell: read.shell,
            host_key,
        })
    }

    /// ssh-backend.ts:946. The key this computer's ssh client holds for a machine, or nothing where it holds none.
    pub async fn key_for(&self, reach: &SshReach) -> Option<String> {
        (self.host_key)(reach.clone()).await
    }

    /// ssh-backend.ts:954. The entry that key was written into on this computer, as the client itself answers.
    pub async fn known_hosts_entry(&self, reach: &SshReach) -> KnownHostsEntry {
        (self.known_hosts)(reach.clone()).await
    }

    /// ssh-backend.ts:960. What the machine itself answers with, asked of the machine and not of this computer's files.
    pub async fn offered_key_for(&self, reach: &SshReach) -> OfferedKey {
        (self.offered)(reach.clone()).await
    }

    /// ssh-backend.ts:966. The address a dial of this login reaches; nothing where the dial goes through another computer.
    pub async fn host_name_for(&self, reach: &SshReach) -> Option<String> {
        (self.host_name)(reach.clone()).await
    }
}
