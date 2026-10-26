// SPDX-License-Identifier: AGPL-3.0-only
//! ssh-backend.ts:1-375 at a4033dc34. Spec: spec/ssh_dial.md.
//!
//! How wsp dials a machine over ssh: the dial's written forms, the ssh client's argv, the master connection's folder,
//! the person's config carried onto a forward child, and the held forward itself.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use futures::future::LocalBoxFuture;
use futures::task::{waker, ArcWake};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use wsp_words::js::{is_js_whitespace, js_trim, to_number};

use crate::child_exec::{errno_name, errno_sentence, process_env, run_child, ChildOptions, LineFeed};
use crate::errors::{EngineError, Result};
use crate::machine::{ExecResult, OnLine};
use crate::run_env::DROP_SUDO_MARKS;
use crate::ssh_backend::keys::{client_words, passwd_home, plain, posix_join, ssh_refusal_line, SshLocalRun, SSH_LOCAL_READ_MS};
use wsp_wire::types::app_ports::LOOPBACK;
use wsp_wire::types::shell_quote::shell_quote;

/// ssh-backend.ts:24. How the ssh client is dialled; `key_path` absent leaves ssh its own config and agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshReach {
    pub user: String,
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
}

/// ssh-backend.ts:38.
pub const SSH_DEFAULT_PORT: u16 = 22;

/// ssh-backend.ts:43. The one written form of an ssh machine: its id on the record, which is also the dial.
pub fn ssh_machine_id(reach: &SshReach) -> Result<String> {
    let text = format!("ssh://{}@{}:{}", encode_uri_component(&reach.user), reach.host, reach.port);
    let mut url = url::Url::parse(&text).map_err(|_| plain("Invalid URL".to_string()))?;
    if let Some(key) = &reach.key_path {
        // `searchParams.set` reads the query the host already put there as pairs and writes the whole of it back.
        let mut pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
        match pairs.iter().position(|(k, _)| k == "key") {
            Some(first) => {
                pairs[first].1 = key.clone();
                let mut seen = 0;
                pairs.retain(|(k, _)| {
                    if k == "key" {
                        seen += 1;
                    }
                    k != "key" || seen == 1
                });
            }
            None => pairs.push(("key".to_string(), key.clone())),
        }
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    Ok(url.to_string())
}

/// JavaScript's `encodeURIComponent`: the unreserved marks and ASCII letters and digits stay, every other UTF-8 byte is
/// written as `%XX`.
fn encode_uri_component(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// ssh-backend.ts:51. The dial a person's `user@host` word names; `port` and `key_path` are what they typed beside it.
pub fn parse_ssh_address(address: &str, port: Option<u16>, key_path: Option<&str>) -> Result<SshReach> {
    let (user, rest) = match address.rfind('@') {
        Some(at) => (&address[..at], &address[at + 1..]),
        None => ("", address),
    };
    let (host, named) = match rest.rfind(':') {
        Some(colon) => (&rest[..colon], Some(to_number(&rest[colon + 1..]))),
        None => (rest, None),
    };
    if user.is_empty() || host.is_empty() {
        return Err(plain(format!("{address} is not an ssh address; name the machine as user@host")));
    }
    let wanted = match (port, named) {
        (Some(port), _) => f64::from(port),
        (None, Some(named)) => named,
        (None, None) => f64::from(SSH_DEFAULT_PORT),
    };
    if wanted.fract() != 0.0 || !(1.0..=65535.0).contains(&wanted) {
        return Err(plain(format!("{address} names no port ssh can dial")));
    }
    Ok(SshReach { user: user.to_string(), host: host.to_string(), port: wanted as u16, key_path: key_path.map(str::to_string) })
}

/// ssh-backend.ts:67. The login, and the port only where it is not ssh's own.
pub fn ssh_login_word(reach: &SshReach) -> String {
    if reach.port == SSH_DEFAULT_PORT {
        format!("{}@{}", reach.user, reach.host)
    } else {
        format!("{}@{}:{}", reach.user, reach.host, reach.port)
    }
}

/// ssh-backend.ts:73. The host's first label, or the whole address where cutting would leave a number.
pub fn ssh_machine_name(reach: &SshReach) -> String {
    let numeric = !reach.host.is_empty() && reach.host.chars().all(|c| c.is_ascii_digit() || c == '.');
    match reach.host.find('.') {
        Some(dot) if !numeric => reach.host[..dot].to_string(),
        _ => reach.host.clone(),
    }
}

/// ssh-backend.ts:85. Who a script runs as: root by whichever road the login has, or the login itself. A read as the
/// login never carries a password, so the enum has no room for both.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SshRide {
    /// Root as the login where it is root, else through `sudo -n`.
    #[default]
    Root,
    /// Root through sudo reading this password off stdin.
    RootByPassword(String),
    /// The login itself.
    Login,
}

/// ssh-backend.ts:85. What one script rides with.
#[derive(Clone, Default)]
pub struct SshTransportOptions {
    pub timeout_ms: Option<u64>,
    pub on_line: Option<OnLine>,
    pub stdin: Option<Vec<u8>>,
    pub ride: SshRide,
}

/// ssh-backend.ts:82. How a script reaches the machine; a test hands its own and reads what it was asked to run.
pub type SshTransport = Arc<dyn Fn(SshReach, String, SshTransportOptions) -> LocalBoxFuture<'static, Result<ExecResult>>>;

/// ssh-backend.ts:89.
pub const SSH_CONNECT_TIMEOUT_S: u64 = 10;

/// ssh-backend.ts:92. In UTF-16 units.
pub const SSH_LINE_CAP: usize = 300;

/// ssh-backend.ts:96. Words a box wrote: control characters gone, `cap` UTF-16 units kept (48 where the TS defaults).
pub fn box_word(said: &str, cap: usize) -> String {
    let kept: String = said.chars().filter(|c| !matches!(*c as u32, 0x00..=0x1f | 0x7f..=0x9f)).collect();
    utf16_cut(&kept, cap)
}

/// The first `n` UTF-16 units of `text`; a unit that splits a surrogate pair is written as U+FFFD.
pub(crate) fn utf16_cut(text: &str, n: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&units[..n.min(units.len())])
}

/// The last `n` UTF-16 units of `text`, as `slice(-n)`; a unit that splits a surrogate pair is written as U+FFFD.
pub(crate) fn utf16_tail(text: &str, n: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&units[units.len().saturating_sub(n)..])
}

/// ssh-backend.ts:101.
pub const SSH_CONTROL_PERSIST_S: u64 = 60;

/// The home directory node's `homedir()` answers: `$HOME` when it is set, empty or not, else the passwd entry's.
fn node_homedir() -> String {
    match std::env::var_os("HOME") {
        Some(home) => home.to_string_lossy().into_owned(),
        None => passwd_home().unwrap_or_default(),
    }
}

/// ssh-backend.ts:106. `join(home, ".wsp", "ssh")`, normalized; `None` is Node's `homedir()` ($HOME when set, else passwd).
pub fn ssh_control_dir(home: Option<&str>) -> PathBuf {
    let home = home.map_or_else(node_homedir, str::to_string);
    PathBuf::from(posix_join(&home, ".wsp/ssh"))
}

/// ssh-backend.ts:111.
pub const SSH_CONTROL_DIR_MODE: u32 = 0o700;

/// ssh-backend.ts:115. Makes the folder (parents included) and tightens it to 0700 whatever it was. A failure is node's
/// own error, `ENOTDIR: not a directory, mkdir '<folder>'`.
pub fn make_ssh_control_dir(home: Option<&str>) -> Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let dir = ssh_control_dir(home);
    let node_error = |e: std::io::Error, syscall: &str| match errno_name(&e) {
        Some(code) => plain(format!("{code}: {}, {syscall} '{}'", errno_sentence(code), dir.display())),
        None => plain(e.to_string()),
    };
    std::fs::DirBuilder::new().recursive(true).mode(SSH_CONTROL_DIR_MODE).create(&dir).map_err(|e| node_error(e, "mkdir"))?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(SSH_CONTROL_DIR_MODE)).map_err(|e| node_error(e, "chmod"))?;
    Ok(dir)
}

/// ssh-backend.ts:125. The master socket for one user, host and port; `dir` `None` is `ssh_control_dir(None)`.
pub fn ssh_control_path(reach: &SshReach, dir: Option<&str>) -> String {
    let dir = dir.map_or_else(|| ssh_control_dir(None).to_string_lossy().into_owned(), str::to_string);
    let digest = Sha256::digest(format!("{}@{}:{}", reach.user, reach.host, reach.port));
    let name = format!("wsp-ssh-{}.sock", &hex::encode(digest)[..16]);
    posix_join(&dir, &name)
}

/// ssh-backend.ts:136. Who a script runs as, as `ssh_args` spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SshAs {
    #[default]
    Root,
    RootByPassword,
    Login,
}

/// ssh-backend.ts:141 `opts`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SshArgsOptions {
    pub control_dir: Option<String>,
    /// The one road that carries a file's bytes on stdin, which drops `-n`.
    pub stdin: bool,
    pub as_: SshAs,
}

/// ssh-backend.ts:141's root road: a root login runs the script as it is, else sudo with the given flags.
fn root_road(sudo: &str) -> String {
    format!("if [ \"$(id -u)\" -eq 0 ]; then exec bash -c \"$1\"; fi\nexec sudo {sudo} bash -c \"{DROP_SUDO_MARKS}\n$1\"")
}

/// ssh-backend.ts:141. The ssh client's argv for one script.
pub fn ssh_args(reach: &SshReach, script: &str, opts: &SshArgsOptions) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if !opts.stdin {
        args.push("-n".to_string());
    }
    args.push("-T".to_string());
    args.push("-o".to_string());
    args.push("ControlMaster=auto".to_string());
    args.push("-o".to_string());
    args.push(format!("ControlPath={}", ssh_control_path(reach, opts.control_dir.as_deref())));
    args.push("-o".to_string());
    args.push(format!("ControlPersist={SSH_CONTROL_PERSIST_S}"));
    args.extend(ssh_dial_args(reach));
    args.push(format!("{}@{}", reach.user, reach.host));
    args.push("bash".to_string());
    args.push("-c".to_string());
    match opts.as_ {
        SshAs::Login => args.push(shell_quote(script)),
        SshAs::Root => {
            args.push(shell_quote(&root_road("-n")));
            args.push("wsp".to_string());
            args.push(shell_quote(script));
        }
        SshAs::RootByPassword => {
            args.push(shell_quote(&root_road("-S -k -p ''")));
            args.push("wsp".to_string());
            args.push(shell_quote(script));
        }
    }
    args
}

/// ssh-backend.ts:166. How every ssh child this host starts is dialled, whatever it then does on the connection.
pub fn ssh_dial_args(reach: &SshReach) -> Vec<String> {
    let timeout = format!("ConnectTimeout={SSH_CONNECT_TIMEOUT_S}");
    let mut args: Vec<String> = ["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new", "-o", timeout.as_str(), "-p"]
        .iter()
        .map(|word| word.to_string())
        .collect();
    args.push(reach.port.to_string());
    if let Some(key) = &reach.key_path {
        args.extend(["-i".to_string(), key.clone(), "-o".to_string(), "IdentitiesOnly=yes".to_string()]);
    }
    args
}

/// ssh-backend.ts:182 `"one" | "rest"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarriedShape {
    One,
    Rest,
}

/// ssh-backend.ts:182. The keys of the person's config a forward child carries: `ssh -G` name, option, shape.
pub const CARRIED_SSH_OPTIONS: [(&str, &str, CarriedShape); 13] = [
    ("identityfile", "IdentityFile", CarriedShape::One),
    ("identitiesonly", "IdentitiesOnly", CarriedShape::One),
    ("certificatefile", "CertificateFile", CarriedShape::One),
    ("proxycommand", "ProxyCommand", CarriedShape::Rest),
    ("userknownhostsfile", "UserKnownHostsFile", CarriedShape::Rest),
    ("globalknownhostsfile", "GlobalKnownHostsFile", CarriedShape::Rest),
    ("hostkeyalias", "HostKeyAlias", CarriedShape::One),
    ("checkhostip", "CheckHostIP", CarriedShape::One),
    ("hostkeyalgorithms", "HostKeyAlgorithms", CarriedShape::One),
    ("pubkeyacceptedalgorithms", "PubkeyAcceptedAlgorithms", CarriedShape::One),
    ("identityagent", "IdentityAgent", CarriedShape::One),
    ("addkeystoagent", "AddKeysToAgent", CarriedShape::One),
    ("usekeychain", "UseKeychain", CarriedShape::One),
];

/// ssh-backend.ts:207. The refusal for a known hosts file the config names that is not on this computer.
pub fn missing_known_hosts_line(target: &str, file: &str) -> String {
    format!(
        "{}: your ssh config names {} as a known hosts file and this computer has no file there, so wsp will not dial back over ssh with it; create it, or rename a path that has a space in it",
        utf16_cut(target, 48),
        utf16_cut(file, 72)
    )
}

/// ssh-backend.ts:215. A dial with the person's config already read into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshCarried {
    pub reach: SshReach,
    pub options: Vec<String>,
}

/// A value ssh re-reads as one word: a space, a quote or a backslash in it would split it or be eaten, so it goes in
/// double quotes with ssh's own escapes.
fn carried_value(value: &str, shape: CarriedShape) -> String {
    let needs_quotes = value.chars().any(|c| is_js_whitespace(c) || matches!(c, '"' | '\'' | '\\'));
    if shape == CarriedShape::Rest || !needs_quotes {
        return value.to_string();
    }
    let escaped: String = value.chars().flat_map(|c| if matches!(c, '"' | '\\') { vec!['\\', c] } else { vec![c] }).collect();
    format!("\"{escaped}\"")
}

/// ssh-backend.ts:232's jump command: the hops before the last one ride `-J`, the last one is the `ssh://` target.
fn jump_command(spec: &str) -> String {
    let mut hops: Vec<&str> = spec.split(',').collect();
    let last = hops.pop().unwrap_or("");
    let mut words = vec!["ssh".to_string(), "-o".to_string(), "BatchMode=yes".to_string()];
    if !hops.is_empty() {
        words.push("-J".to_string());
        words.push(shell_quote(&hops.join(",")));
    }
    words.push("-W".to_string());
    words.push("'[%h]:%p'".to_string());
    words.push(shell_quote(&format!("ssh://{last}")));
    words.join(" ")
}

/// ssh-backend.ts:232. The person's config for one dial, read through `ssh -G` with it and without it. Throws ssh's own
/// line on a config it cannot read, and the missing-file sentence on a known hosts file the config names that is not
/// on this computer.
pub async fn carried_ssh_values(reach: &SshReach, run: &SshLocalRun) -> Result<SshCarried> {
    let target = format!("{}@{}", reach.user, reach.host);
    let read = |bare: bool| {
        let mut args = vec!["-G".to_string()];
        if bare {
            args.extend(["-F".to_string(), "/dev/null".to_string()]);
        }
        args.extend(ssh_dial_args(reach));
        args.push(target.clone());
        async move {
            let said = run("ssh".to_string(), args, SSH_LOCAL_READ_MS).await;
            if said.exit_code.get() != 0 {
                return Err(plain(ssh_refusal_line(&said, reach)));
            }
            Ok(crate::machine_facts::read_lists(&said.stdout))
        }
    };
    let config = read(false).await?;
    let defaults = read(true).await?;
    let first = |key: &str| config.get(key).and_then(|values| values.first()).cloned().unwrap_or_default();
    let mut options: Vec<String> = Vec::new();
    for (key, option, shape) in CARRIED_SSH_OPTIONS {
        let values = config.get(key).cloned().unwrap_or_default();
        if values.join("\n") == defaults.get(key).cloned().unwrap_or_default().join("\n") {
            continue;
        }
        if key.ends_with("knownhostsfile") {
            let missing = values
                .iter()
                .flat_map(|value| value.split(' '))
                .find(|file| !file.is_empty() && *file != "none" && !(file.starts_with('/') && std::path::Path::new(file).exists()));
            if let Some(file) = missing {
                return Err(EngineError::MissingKnownHosts { message: missing_known_hosts_line(&target, file) });
            }
        }
        let said: Vec<String> = if key == "identityagent" { values.iter().map(|value| value.replace('%', "%%")).collect() } else { values };
        options.extend(said.iter().filter(|value| !value.is_empty()).map(|value| format!("{option}={}", carried_value(value, shape))));
    }
    let jump = first("proxyjump");
    if first("proxycommand").is_empty() && !jump.is_empty() && jump != "none" {
        options.push(format!("ProxyCommand={}", jump_command(&jump)));
    }
    let port = to_number(&first("port"));
    let port = if port.fract() == 0.0 && (1.0..=65535.0).contains(&port) { port as u16 } else { reach.port };
    let user = first("user");
    let host = first("hostname");
    Ok(SshCarried {
        reach: SshReach {
            user: if user.is_empty() { reach.user.clone() } else { user },
            host: if host.is_empty() { reach.host.clone() } else { host },
            port,
            key_path: reach.key_path.clone(),
        },
        options,
    })
}

/// ssh-backend.ts:262.
pub const SSH_BACK_UP: &str = "WSP_BACK_UP";

/// ssh-backend.ts:263. What the forward child runs on the box: the forward is up once its line prints, then cat holds the
/// session open on the host's stdin pipe.
const BACK_COMMAND: &str = "echo WSP_BACK_UP; exec cat";

/// ssh-backend.ts:268. The forward child's argv: the carried dial under no config, then one remote forward from the box's
/// loopback to this computer's door.
pub fn ssh_back_args(carried: &SshCarried, box_port: u16, door_port: u16) -> Vec<String> {
    let mut args: Vec<String> = vec!["-F".to_string(), "/dev/null".to_string()];
    args.extend(ssh_dial_args(&carried.reach));
    for option in &carried.options {
        args.extend(["-o".to_string(), option.clone()]);
    }
    for word in
        ["-T", "-o", "ExitOnForwardFailure=yes", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=3", "-o", "ControlPath=none"]
    {
        args.push(word.to_string());
    }
    args.push("-R".to_string());
    args.push(format!("{LOOPBACK}:{box_port}:{LOOPBACK}:{door_port}"));
    args.push(format!("{}@{}", carried.reach.user, carried.reach.host));
    args.push(BACK_COMMAND.to_string());
    args
}

/// How a held child ended (`exit` with its code or its signal, or `error`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildEnd {
    /// Exited with this code.
    Exited(i64),
    /// Ended on this signal, named as Node names it (`SIGTERM`), with no code.
    Signalled(String),
    /// Never started, or failed as a process: the error's message.
    Failed(String),
}

/// ssh-backend.ts:291. The part of a started child the holder reads; a test hands its own. `exit` resolves once, when
/// the child ends; a real child's pipes close as it does, and a fake closes them before resolving it.
pub struct HeldChild {
    pub stdin: Box<dyn AsyncWrite + Unpin>,
    pub stdout: Box<dyn AsyncRead + Unpin>,
    pub stderr: Box<dyn AsyncRead + Unpin>,
    /// Sends the named signal; true where it was sent.
    pub kill: Box<dyn FnMut(&str) -> bool>,
    pub exit: LocalBoxFuture<'static, ChildEnd>,
}

/// ssh-backend.ts:300.
pub type SshSpawn = Arc<dyn Fn(&str, &[String]) -> HeldChild>;

/// ssh-backend.ts:302. The real ssh child, with this process's env and three pipes.
pub fn spawn_ssh() -> SshSpawn {
    use std::os::unix::process::ExitStatusExt;
    Arc::new(|file: &str, args: &[String]| {
        let spawned =
            tokio::process::Command::new(file).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                let why = errno_name(&e).map_or_else(|| e.to_string(), str::to_string);
                return HeldChild {
                    stdin: Box::new(tokio::io::sink()),
                    stdout: Box::new(tokio::io::empty()),
                    stderr: Box::new(tokio::io::empty()),
                    kill: Box::new(|_: &str| false),
                    exit: futures::future::ready(ChildEnd::Failed(format!("spawn {file} {why}"))).boxed_local(),
                };
            }
        };
        let pid = child.id();
        let stdin: Box<dyn AsyncWrite + Unpin> = match child.stdin.take() {
            Some(stdin) => Box::new(stdin),
            None => Box::new(tokio::io::sink()),
        };
        let stdout: Box<dyn AsyncRead + Unpin> = match child.stdout.take() {
            Some(stdout) => Box::new(stdout),
            None => Box::new(tokio::io::empty()),
        };
        let stderr: Box<dyn AsyncRead + Unpin> = match child.stderr.take() {
            Some(stderr) => Box::new(stderr),
            None => Box::new(tokio::io::empty()),
        };
        HeldChild {
            stdin,
            stdout,
            stderr,
            kill: Box::new(move |signal: &str| send_signal(pid, signal)),
            exit: async move {
                match child.wait().await {
                    Ok(status) => match (status.code(), status.signal()) {
                        (Some(code), _) => ChildEnd::Exited(i64::from(code)),
                        (None, Some(number)) => ChildEnd::Signalled(signal_name(number)),
                        (None, None) => ChildEnd::Exited(0),
                    },
                    Err(e) => ChildEnd::Failed(e.to_string()),
                }
            }
            .boxed_local(),
        }
    })
}

/// The signal a name stands for, sent to the child; false where the child is gone or the name is unknown.
fn send_signal(pid: Option<u32>, signal: &str) -> bool {
    let number = match signal {
        "SIGHUP" => libc::SIGHUP,
        "SIGINT" => libc::SIGINT,
        "SIGKILL" => libc::SIGKILL,
        "SIGTERM" => libc::SIGTERM,
        _ => return false,
    };
    match pid {
        // SAFETY: kill(2) takes a pid and a signal number and reads no memory of ours.
        Some(pid) => unsafe { libc::kill(pid as i32, number) == 0 },
        None => false,
    }
}

/// The name Node gives a signal number (`signo_string`), as this platform numbers them.
fn signal_name(number: i32) -> String {
    let name = match number {
        libc::SIGHUP => "SIGHUP",
        libc::SIGINT => "SIGINT",
        libc::SIGQUIT => "SIGQUIT",
        libc::SIGILL => "SIGILL",
        libc::SIGTRAP => "SIGTRAP",
        libc::SIGABRT => "SIGABRT",
        libc::SIGBUS => "SIGBUS",
        libc::SIGFPE => "SIGFPE",
        libc::SIGKILL => "SIGKILL",
        libc::SIGUSR1 => "SIGUSR1",
        libc::SIGSEGV => "SIGSEGV",
        libc::SIGUSR2 => "SIGUSR2",
        libc::SIGPIPE => "SIGPIPE",
        libc::SIGALRM => "SIGALRM",
        libc::SIGTERM => "SIGTERM",
        libc::SIGCHLD => "SIGCHLD",
        libc::SIGCONT => "SIGCONT",
        libc::SIGSTOP => "SIGSTOP",
        libc::SIGTSTP => "SIGTSTP",
        libc::SIGTTIN => "SIGTTIN",
        libc::SIGTTOU => "SIGTTOU",
        libc::SIGURG => "SIGURG",
        libc::SIGXCPU => "SIGXCPU",
        libc::SIGXFSZ => "SIGXFSZ",
        libc::SIGVTALRM => "SIGVTALRM",
        libc::SIGPROF => "SIGPROF",
        libc::SIGWINCH => "SIGWINCH",
        libc::SIGIO => "SIGIO",
        libc::SIGSYS => "SIGSYS",
        #[cfg(target_os = "linux")]
        libc::SIGSTKFLT => "SIGSTKFLT",
        #[cfg(target_os = "linux")]
        libc::SIGPWR => "SIGPWR",
        other => return format!("SIG{other}"),
    };
    name.to_string()
}

/// ssh-backend.ts:313.
pub const HELD_TAIL: usize = 4096;

/// ssh-backend.ts:306. One held forward.
pub struct BackForward {
    inner: Arc<BackForwardState>,
}

/// The holder's own state; its shape is the implementer's. The child's pipes and exit are read from the moment the
/// holder starts, by a local task that polls through the same waiters a caller of `up` or `ended` leaves, so a child
/// that writes more than a pipe holds before anyone asks is never stalled (ssh-backend.ts:329-336).
struct BackForwardState {
    waiters: Arc<Waiters>,
    pump: RefCell<Pump>,
}

/// The callers waiting on the holder. A read that is pending wakes every one of them, so no caller is left asleep
/// because another one polled the same pipe last.
#[derive(Default)]
struct Waiters(Mutex<Vec<Waker>>);

impl Waiters {
    fn add(&self, waker: &Waker) {
        let mut list = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !list.iter().any(|held| held.will_wake(waker)) {
            list.push(waker.clone());
        }
    }

    fn wake_all(&self) {
        let list = std::mem::take(&mut *self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
        for waker in list {
            waker.wake();
        }
    }
}

impl ArcWake for Waiters {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.wake_all();
    }
}

/// What the holder has read of the child so far.
struct Pump {
    stdout: Option<Box<dyn AsyncRead + Unpin>>,
    stderr: Option<Box<dyn AsyncRead + Unpin>>,
    stdin: Option<Box<dyn AsyncWrite + Unpin>>,
    kill: Box<dyn FnMut(&str) -> bool>,
    exit: Option<LocalBoxFuture<'static, ChildEnd>>,
    exit_end: Option<ChildEnd>,
    out_lines: LineFeed,
    err_lines: LineFeed,
    said: VecDeque<String>,
    up: Option<std::result::Result<(), String>>,
    ended: Option<String>,
    released: bool,
}

impl Pump {
    /// Reads what is ready on both pipes, takes the exit once it is there, and settles on the exit: a helper the child left
    /// holding a pipe does not keep the forward from ending (ssh-backend.ts:346 settles on `exit`, not on the pipes' close).
    fn poll(&mut self, cx: &mut Context<'_>) {
        self.read_pipes(cx);
        if let Some(exit) = self.exit.as_mut() {
            if let Poll::Ready(end) = exit.poll_unpin(cx) {
                self.exit = None;
                self.exit_end = Some(end);
                self.read_pipes(cx);
            }
        }
        if self.ended.is_none() && self.exit_end.is_some() {
            self.settle();
            self.stdout = None;
            self.stderr = None;
            self.stdin = None;
        }
    }

    fn read_pipes(&mut self, cx: &mut Context<'_>) {
        for line in take_lines(&mut self.stdout, &mut self.out_lines, cx) {
            if js_trim(&line) == SSH_BACK_UP && self.up.is_none() {
                self.up = Some(Ok(()));
            }
        }
        for line in take_lines(&mut self.stderr, &mut self.err_lines, cx) {
            self.said.push_back(line);
            if self.said.len() > 50 {
                self.said.pop_front();
            }
        }
    }

    /// The words a dial's failure is reported in: the last line the client's own words keep, else the child's exit.
    fn settle(&mut self) {
        let kept: Vec<String> = self.said.iter().cloned().collect();
        let line = client_words(&kept.join("\n")).split('\n').next_back().unwrap_or("").to_string();
        let fallback = match self.exit_end.as_ref() {
            Some(ChildEnd::Exited(code)) => format!("ssh exited with {code}"),
            Some(ChildEnd::Signalled(signal)) => format!("ssh ended on {signal}"),
            Some(ChildEnd::Failed(message)) => message.clone(),
            None => String::new(),
        };
        let words = box_word(if line.is_empty() { &fallback } else { &line }, SSH_LINE_CAP);
        if self.up.is_none() {
            self.up = Some(Err(words.clone()));
        }
        self.ended = Some(words);
    }

    fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.stdin = None;
        (self.kill)("SIGTERM");
    }
}

/// Every complete line a pipe has ready, each with one trailing `\r` gone and cut to `SSH_LINE_CAP` units. A pipe that
/// reads to its end is dropped; a partial line at the end is never read.
fn take_lines(pipe: &mut Option<Box<dyn AsyncRead + Unpin>>, feed: &mut LineFeed, cx: &mut Context<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(reader) = pipe.as_mut() {
        let mut buf = [0u8; 8192];
        let mut read = ReadBuf::new(&mut buf);
        match Pin::new(reader).poll_read(cx, &mut read) {
            Poll::Ready(Ok(())) if !read.filled().is_empty() => {
                let text = String::from_utf8_lossy(read.filled()).into_owned();
                for line in feed.feed(&text) {
                    let line = line.strip_suffix('\r').unwrap_or(&line);
                    lines.push(utf16_cut(line, SSH_LINE_CAP));
                }
            }
            Poll::Ready(_) => *pipe = None,
            Poll::Pending => break,
        }
    }
    lines
}

impl Drop for BackForwardState {
    fn drop(&mut self) {
        self.waiters.wake_all();
    }
}

impl BackForwardState {
    /// Registers the caller and reads what it can; a settled state wakes everyone waiting on it.
    fn drive(&self, cx: &Context<'_>) {
        self.waiters.add(cx.waker());
        let before = {
            let pump = self.pump.borrow();
            (pump.up.is_some(), pump.ended.is_some())
        };
        let combined = waker(self.waiters.clone());
        let mut inner = Context::from_waker(&combined);
        let after = {
            let mut pump = self.pump.borrow_mut();
            pump.poll(&mut inner);
            (pump.up.is_some(), pump.ended.is_some())
        };
        if before != after {
            self.waiters.wake_all();
        }
    }

    fn poll_up(&self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), String>> {
        self.drive(cx);
        match self.pump.borrow().up.clone() {
            Some(answer) => Poll::Ready(answer),
            None => Poll::Pending,
        }
    }

    fn poll_ended(&self, cx: &mut Context<'_>) -> Poll<String> {
        self.drive(cx);
        match self.pump.borrow().ended.clone() {
            Some(words) => Poll::Ready(words),
            None => Poll::Pending,
        }
    }
}

impl BackForward {
    /// Answers once the forward stands; fails with ssh's line when the child ends first. Pending until one happens.
    pub async fn up(&self) -> std::result::Result<(), String> {
        futures::future::poll_fn(|cx| self.inner.poll_up(cx)).await
    }

    /// Answers that line whenever the child ends.
    pub async fn ended(&self) -> String {
        futures::future::poll_fn(|cx| self.inner.poll_ended(cx)).await
    }

    /// Ends it: the first call closes stdin and sends SIGTERM, later calls nothing.
    pub fn release(&self) {
        self.inner.pump.borrow_mut().release();
    }
}

/// ssh-backend.ts:323. Starts the forward child and holds it, draining its pipes on a local task that keeps the holder's
/// state until the child ends, so a holder that is dropped leaves the child running until `release` or its own end, as the
/// TS's does. Outside a tokio `LocalSet` there is no task to drain on: `up` and `ended` then read the pipes as they are
/// awaited.
pub fn hold_back_forward(carried: &SshCarried, box_port: u16, door_port: u16, spawn: SshSpawn) -> BackForward {
    let child = spawn("ssh", &ssh_back_args(carried, box_port, door_port));
    let pump = Pump {
        stdout: Some(child.stdout),
        stderr: Some(child.stderr),
        stdin: Some(child.stdin),
        kill: child.kill,
        exit: Some(child.exit),
        exit_end: None,
        out_lines: LineFeed::new(Some(HELD_TAIL)),
        err_lines: LineFeed::new(Some(HELD_TAIL)),
        said: VecDeque::new(),
        up: None,
        ended: None,
        released: false,
    };
    let state = Arc::new(BackForwardState { waiters: Arc::new(Waiters::default()), pump: RefCell::new(pump) });
    let drained = state.clone();
    try_spawn_local(futures::future::poll_fn(move |cx| {
        drained.drive(cx);
        if drained.pump.borrow().ended.is_some() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }));
    BackForward { inner: state }
}

/// `spawn_local` where there is a `LocalSet` to run it, and nothing where there is none; true where it was spawned.
fn try_spawn_local(task: impl std::future::Future<Output = ()> + 'static) -> bool {
    static QUIET: std::sync::Once = std::sync::Once::new();
    // The one panic `spawn_local` makes outside a LocalSet is caught here, so it is kept out of the log.
    QUIET.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let said = match info.payload().downcast_ref::<&str>() {
                Some(said) => said.to_string(),
                None => info.payload().downcast_ref::<String>().cloned().unwrap_or_default(),
            };
            if !said.contains("called from outside of a `task::LocalSet`") {
                previous(info);
            }
        }));
    });
    let mut task = Some(task);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(task) = task.take() {
            tokio::task::spawn_local(task);
        }
    }))
    .is_ok()
}

/// ssh-backend.ts:366. The ssh client on this computer, carrying one script to the machine.
pub fn ssh_client() -> SshTransport {
    Arc::new(|reach: SshReach, script: String, opts: SshTransportOptions| {
        async move {
            let control_dir = make_ssh_control_dir(None)?;
            let (as_, stdin) = match &opts.ride {
                SshRide::Login => (SshAs::Login, opts.stdin.clone()),
                SshRide::RootByPassword(password) => {
                    let mut line = format!("{password}\n").into_bytes();
                    line.extend(opts.stdin.clone().unwrap_or_default());
                    (SshAs::RootByPassword, Some(line))
                }
                SshRide::Root => (SshAs::Root, opts.stdin.clone()),
            };
            let args = ssh_args(
                &reach,
                &script,
                &SshArgsOptions { control_dir: Some(control_dir.to_string_lossy().into_owned()), stdin: stdin.is_some(), as_ },
            );
            let child = ChildOptions {
                env: Some(process_env()),
                timeout_ms: opts.timeout_ms,
                on_line: opts.on_line.clone(),
                stdin,
                ..Default::default()
            };
            Ok(run_child("ssh", &args, child).await)
        }
        .boxed_local()
    })
}
