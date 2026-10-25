// SPDX-License-Identifier: AGPL-3.0-only
//! ssh-backend.ts:377-615 at a4033dc34. Spec: spec/ssh_keys.md.
//!
//! The key a machine over ssh is known by, read off this computer's own client with nothing dialled; the alias and
//! address a typed word names; and the words a refused dial is reported in.
use std::ffi::CStr;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use wsp_words::js::{is_js_whitespace, js_trim, number_string, to_number};

use crate::child_exec::{run_child, ChildOptions};
use crate::errors::{EngineError, Result};
use crate::key_fingerprint::key_fingerprint;
use crate::machine::ExecResult;
use crate::machine_facts::read_values;
use crate::ssh_backend::dial::{
    parse_ssh_address, ssh_dial_args, utf16_tail, SshReach, SshRide, SshTransport, SshTransportOptions, SSH_DEFAULT_PORT, SSH_LINE_CAP,
};

/// ssh-backend.ts:380. How the key a machine holds is read.
pub type SshHostKeyReader = Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, Option<String>>>;

/// ssh-backend.ts:384. One command on this computer: file, args, timeout in ms. A test hands its own.
pub type SshLocalRun = Arc<dyn Fn(String, Vec<String>, u64) -> LocalBoxFuture<'static, ExecResult>>;

/// ssh-backend.ts:386. `run_child(file, args, { env: this process's, timeout_ms })`.
pub fn local_run() -> SshLocalRun {
    Arc::new(|file: String, args: Vec<String>, timeout_ms: u64| {
        async move {
            let env = std::env::vars().collect();
            run_child(&file, &args, ChildOptions { env: Some(env), timeout_ms: Some(timeout_ms), ..Default::default() }).await
        }
        .boxed_local()
    })
}

/// ssh-backend.ts:390.
pub const SSH_LOCAL_READ_MS: u64 = 5_000;
/// ssh-backend.ts:395.
pub const SSH_KEYSCAN_S: u64 = 5;
/// ssh-backend.ts:396.
pub const SSH_KEYSCAN_MS: u64 = 10_000;

/// ssh-backend.ts:401. The name the client writes this machine's key under, off its own `ssh -G` answers.
pub fn known_host_target(values: &IndexMap<String, String>) -> Option<String> {
    if let Some(alias) = values.get("hostkeyalias").filter(|alias| !alias.is_empty()) {
        return Some(alias.clone());
    }
    let host = values.get("hostname").filter(|host| !host.is_empty())?;
    let port = values.get("port").map_or(f64::NAN, |port| to_number(port));
    if port.is_finite() && port.fract() == 0.0 && port != f64::from(SSH_DEFAULT_PORT) {
        Some(format!("[{host}]:{}", number_string(port)))
    } else {
        Some(host.clone())
    }
}

/// ssh-backend.ts:415. The files the client checks a host key against, in its order; `home` `None` is the passwd
/// entry's home, never `$HOME`.
pub fn known_host_files(values: &IndexMap<String, String>, home: Option<&str>) -> Vec<String> {
    let mut files = Vec::new();
    for key in ["userknownhostsfile", "globalknownhostsfile"] {
        let named = values.get(key).map_or("", String::as_str);
        for file in named.split(is_js_whitespace).filter(|file| !file.is_empty() && *file != "none") {
            match file.strip_prefix("~/") {
                Some(rest) => {
                    let base = home.map(str::to_string).or_else(passwd_home).unwrap_or_default();
                    files.push(posix_join(&base, rest));
                }
                None => files.push(file.to_string()),
            }
        }
    }
    files
}

/// ssh-backend.ts:433. The key a dial would be checked against, out of `ssh-keygen -F` lines, as `"{type} SHA256:.."`.
pub fn host_key_found(found: &str, algorithms: &str) -> Option<String> {
    let preferred: Vec<&str> = algorithms.split(',').map(|name| if is_rsa_sha2(name) { "ssh-rsa" } else { name }).collect();
    let mut best: Option<(usize, String)> = None;
    for line in found.split('\n') {
        let fields: Vec<&str> = js_trim(line).split(is_js_whitespace).filter(|field| !field.is_empty()).collect();
        if fields.len() < 3 {
            continue;
        }
        let Some(rank) = preferred.iter().position(|name| *name == fields[1]) else { continue };
        if best.as_ref().is_none_or(|(held, _)| rank < *held) {
            best = Some((rank, format!("{} {}", fields[1], key_fingerprint(fields[2]))));
        }
    }
    best.map(|(_, key)| key)
}

/// ssh-backend.ts:452. Whether the client knows this machine only through an authority.
pub fn trusted_by_authority(found: &str) -> bool {
    let lines: Vec<&str> = found.split('\n').map(|line| js_trim(line)).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
    !lines.is_empty() && lines.iter().all(|line| line.starts_with("@cert-authority "))
}

/// What the machine itself answered a scan with, and what in the person's config stopped the scan where one did
/// (ssh-backend.ts:482's answer). Both `None` is the TS's `{}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OfferedKey {
    pub key: Option<String>,
    /// `"ProxyJump"` or `"ProxyCommand"`.
    pub stopped_by: Option<String>,
}

/// ssh-backend.ts:482.
pub async fn offered_host_key(reach: &SshReach, run: &SshLocalRun) -> OfferedKey {
    let config = run("ssh".to_string(), dial_config_args(reach), SSH_LOCAL_READ_MS).await;
    if config.exit_code.get() != 0 {
        return OfferedKey::default();
    }
    let values = read_values(&config.stdout);
    if let Some(stopped_by) = proxied_by(&values) {
        return OfferedKey { key: None, stopped_by: Some(stopped_by.to_string()) };
    }
    let algorithms = values.get("hostkeyalgorithms").map_or("", String::as_str);
    let key = host_key_found(&offered_host_keys(&values, run).await, algorithms);
    OfferedKey { key, stopped_by: None }
}

/// ssh-backend.ts:498.
pub fn ssh_word_refusal(word: &str) -> String {
    format!("{word} is neither a login like root@host nor an alias your ssh config gives a HostName")
}

/// ssh-backend.ts:505. The dial one typed word names: a login, or an alias the person's config renames.
pub async fn ssh_word_reach(word: &str, port: Option<u16>, key_path: Option<&str>, run: &SshLocalRun) -> Result<SshReach> {
    if word.contains('@') {
        return parse_ssh_address(word, port, key_path);
    }
    if !is_word_name(word) || is_numeric_word(word) {
        return Err(plain(ssh_word_refusal(word)));
    }
    let config = run("ssh".to_string(), vec!["-G".to_string(), word.to_string()], SSH_LOCAL_READ_MS).await;
    let values = if config.exit_code.get() == 0 { read_values(&config.stdout) } else { IndexMap::new() };
    let host = values.get("hostname").map_or("", String::as_str);
    let user = values.get("user").map_or("", String::as_str);
    if host.is_empty() || user.is_empty() || host.to_lowercase() == word.to_lowercase() {
        return Err(plain(ssh_word_refusal(word)));
    }
    let address = format!("{user}@{word}");
    let port = match port {
        Some(typed) => Some(typed),
        None => {
            let read = values.get("port").map_or(f64::NAN, |port| to_number(port));
            let read = if read.is_nan() || read == 0.0 { f64::from(SSH_DEFAULT_PORT) } else { read };
            if read.fract() != 0.0 || !(1.0..=65535.0).contains(&read) {
                return Err(plain(format!("{address} names no port ssh can dial")));
            }
            Some(read as u16)
        }
    };
    parse_ssh_address(&address, port, key_path)
}

/// ssh-backend.ts:522. The address ssh dials for a login; `None` where a jump or a command carries it.
pub async fn ssh_host_name(reach: &SshReach, run: &SshLocalRun) -> Option<String> {
    let config = run("ssh".to_string(), dial_config_args(reach), SSH_LOCAL_READ_MS).await;
    let values = if config.exit_code.get() == 0 { read_values(&config.stdout) } else { IndexMap::new() };
    if proxied_by(&values).is_some() {
        return None;
    }
    let host = values.get("hostname").map_or("", String::as_str);
    Some(if host.is_empty() { reach.host.clone() } else { host.to_string() })
}

/// ssh-backend.ts:536. The key this machine is known by.
pub async fn known_host_key(reach: &SshReach, run: &SshLocalRun) -> Option<String> {
    let config = run("ssh".to_string(), dial_config_args(reach), SSH_LOCAL_READ_MS).await;
    if config.exit_code.get() != 0 {
        return None;
    }
    let values = read_values(&config.stdout);
    let target = known_host_target(&values)?;
    let mut found = String::new();
    for file in known_host_files(&values, None) {
        let args = vec!["-F".to_string(), target.clone(), "-f".to_string(), file];
        let read = run("ssh-keygen".to_string(), args, SSH_LOCAL_READ_MS).await;
        if read.exit_code.get() == 0 {
            found.push_str(&read.stdout);
        }
    }
    let algorithms = values.get("hostkeyalgorithms").map_or("", String::as_str);
    let held = host_key_found(&found, algorithms);
    if held.is_some() || !trusted_by_authority(&found) {
        return held;
    }
    host_key_found(&offered_host_keys(&values, run).await, algorithms)
}

/// ssh-backend.ts:560's answer: the file accept-new writes into and the name the entry is written under. Both `None`
/// is the TS's `{}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownHostsEntry {
    pub file: Option<String>,
    pub target: Option<String>,
}

/// ssh-backend.ts:560.
pub async fn known_hosts_written(reach: &SshReach, run: &SshLocalRun) -> KnownHostsEntry {
    let config = run("ssh".to_string(), dial_config_args(reach), SSH_LOCAL_READ_MS).await;
    if config.exit_code.get() != 0 {
        return KnownHostsEntry::default();
    }
    let values = read_values(&config.stdout);
    KnownHostsEntry { file: known_host_files(&values, None).into_iter().next(), target: known_host_target(&values) }
}

/// ssh-backend.ts:572.
pub fn ssh_identity(host_key: &str, user: &str) -> String {
    format!("{host_key} as {user}")
}

/// ssh-backend.ts:587. The client's own words with its debug log, its known_hosts note and its local forward warnings
/// taken out.
pub fn client_words(text: &str) -> String {
    let kept: Vec<&str> =
        text.split('\n').filter(|line| !debug_line(line) && !wrote_known_hosts(line) && !local_forward_noise(line)).collect();
    js_trim(&kept.join("\n")).to_string()
}

/// ssh-backend.ts:599. What ssh itself said when a login would not stand.
pub fn ssh_refusal_line(said: &ExecResult, reach: &SshReach) -> String {
    let lines = client_words(&said.stderr);
    if lines.is_empty() {
        format!("{}@{} refused the login over ssh (exit {})", reach.user, reach.host, said.exit_code.get())
    } else {
        utf16_tail(&lines, SSH_LINE_CAP)
    }
}

/// ssh-backend.ts:615.
pub const SSH_DIAL_MS: u64 = 15_000;

/// ssh-backend.ts:607. One dial and nothing else: `exit 0` as the login.
pub async fn ssh_dial(reach: &SshReach, transport: &SshTransport) -> Result<()> {
    let opts = SshTransportOptions { timeout_ms: Some(SSH_DIAL_MS), ride: SshRide::Login, ..Default::default() };
    let said = transport(reach.clone(), "exit 0".to_string(), opts).await;
    if said.exit_code.get() == 0 {
        return Ok(());
    }
    Err(plain(ssh_refusal_line(&said, reach)))
}

/// An error the way a TS test throws a plain `new Error(message)`.
pub(crate) fn plain(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

/// The argv of the client's read of a machine's config: the dial's own arguments, then the typed reach.
fn dial_config_args(reach: &SshReach) -> Vec<String> {
    let mut args = vec!["-G".to_string()];
    args.extend(ssh_dial_args(reach));
    args.push(format!("{}@{}", reach.user, reach.host));
    args
}

fn non_empty(values: &IndexMap<String, String>, key: &str) -> bool {
    values.get(key).is_some_and(|value| !value.is_empty())
}

/// ssh-backend.ts:493. The option that carries a dial through another computer, if the config sets one.
fn proxied_by(values: &IndexMap<String, String>) -> Option<&'static str> {
    if non_empty(values, "proxyjump") {
        Some("ProxyJump")
    } else if non_empty(values, "proxycommand") {
        Some("ProxyCommand")
    } else {
        None
    }
}

/// ssh-backend.ts:466. The scan of the machine's own ssh port, `""` where none is asked or the scan fails.
async fn offered_host_keys(values: &IndexMap<String, String>, run: &SshLocalRun) -> String {
    let host = values.get("hostname").map_or("", String::as_str);
    if host.is_empty() || non_empty(values, "proxyjump") || non_empty(values, "proxycommand") {
        return String::new();
    }
    let read = values.get("port").map_or(f64::NAN, |port| to_number(port));
    let port = if read.is_nan() || read == 0.0 { f64::from(SSH_DEFAULT_PORT) } else { read };
    let args = vec!["-T".to_string(), SSH_KEYSCAN_S.to_string(), "-p".to_string(), number_string(port), host.to_string()];
    let scan = run("ssh-keyscan".to_string(), args, SSH_KEYSCAN_MS).await;
    if scan.exit_code.get() == 0 {
        scan.stdout
    } else {
        String::new()
    }
}

/// Node's `path.join(home, rest)`: the non-empty parts joined with `/`, then normalized.
pub(crate) fn posix_join(home: &str, rest: &str) -> String {
    let joined = match (home.is_empty(), rest.is_empty()) {
        (_, true) => home.to_string(),
        (true, false) => rest.to_string(),
        (false, false) => format!("{home}/{rest}"),
    };
    posix_normalize(&joined)
}

/// Node's `path.posix.normalize`: `.` and empty segments fold, `..` pops where it can, a trailing slash stays.
fn posix_normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = path.starts_with('/');
    let trailing = path.ends_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => match parts.last() {
                Some(last) if *last != ".." => {
                    parts.pop();
                }
                _ if !absolute => parts.push(".."),
                _ => {}
            },
            kept => parts.push(kept),
        }
    }
    let mut out = parts.join("/");
    if out.is_empty() && !absolute {
        out.push('.');
    }
    if !out.is_empty() && trailing {
        out.push('/');
    }
    if absolute {
        format!("/{out}")
    } else {
        out
    }
}

/// The home directory the passwd entry names for this user: what node's `homedir()` and ssh's tilde read, never `$HOME`.
pub(crate) fn passwd_home() -> Option<String> {
    // SAFETY: getpwuid_r fills the zeroed entry and the buffer it is given; the directory is copied out before either
    // goes out of scope.
    unsafe {
        let mut entry: libc::passwd = std::mem::zeroed();
        let mut buf = vec![0u8; 16_384];
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let rc = libc::getpwuid_r(libc::getuid(), &mut entry, buf.as_mut_ptr() as *mut libc::c_char, buf.len(), &mut found);
        if rc != 0 || found.is_null() {
            return None;
        }
        Some(CStr::from_ptr(entry.pw_dir).to_string_lossy().into_owned())
    }
}

/// `^rsa-sha2-\d+$`.
fn is_rsa_sha2(name: &str) -> bool {
    name.strip_prefix("rsa-sha2-").is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// `^\w[\w.-]*$`, with JavaScript's ASCII `\w`.
fn is_word_name(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// `^(0x[0-9a-f]*|\d+)(\.(0x[0-9a-f]*|\d+)){0,3}$` with the case flag: one to four numeric parts joined by dots.
fn is_numeric_word(word: &str) -> bool {
    let parts: Vec<&str> = word.split('.').collect();
    (1..=4).contains(&parts.len()) && parts.iter().all(|part| numeric_part(part))
}

fn numeric_part(part: &str) -> bool {
    let lower = part.to_ascii_lowercase();
    if lower.strip_prefix("0x").is_some_and(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit())) {
        return true;
    }
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
}

/// True where none of JavaScript's line breaks for `.` is in the text.
fn no_break(text: &str) -> bool {
    !text.chars().any(|c| matches!(c, '\r' | '\n' | '\u{2028}' | '\u{2029}'))
}

/// `^debug\d+:`.
fn debug_line(line: &str) -> bool {
    line.strip_prefix("debug").is_some_and(|rest| {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        digits > 0 && rest[digits..].starts_with(':')
    })
}

/// `^Warning: Permanently added .* to the list of known hosts\.?$`.
fn wrote_known_hosts(line: &str) -> bool {
    line.strip_prefix("Warning: Permanently added ").is_some_and(|rest| {
        [" to the list of known hosts.", " to the list of known hosts"].iter().any(|tail| rest.strip_suffix(tail).is_some_and(no_break))
    })
}

/// The three warnings ssh prints for a LocalForward in the person's config that cannot bind on this computer.
fn local_forward_noise(line: &str) -> bool {
    if let Some(rest) = line.strip_prefix("bind [") {
        return rest.match_indices("]:").any(|(at, _)| {
            let after = &rest[at + 2..];
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            no_break(&rest[..at]) && digits > 0 && after[digits..].starts_with(": ") && no_break(&after[digits + 2..])
        });
    }
    if let Some(port) = line.strip_prefix("channel_setup_fwd_listener_tcpip: cannot listen to port: ") {
        return !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit());
    }
    line == "Could not request local forwarding."
}
