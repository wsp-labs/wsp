// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/target-line.ts. Spec: spec/machine_reads.md.
//!
//! Who a line on a computer runs as. A box's daemon runs as root, so every line that reads or writes the agents' files
//! comes through here and runs as the owner of the home instead.
use std::future::Future;

use indexmap::IndexMap;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::agents_report::{no_home_refusal, no_runuser_refusal};
use wsp_words::js::js_trim;
use wsp_words::project_path::under_project;

use crate::daemon_targets::platform_of_system;
pub use crate::daemon_targets::Platform;
use crate::errors::{EngineError, Result};
use crate::land_bytes::{land_bytes, LandBytesOptions};
use crate::machine::{ExecOptions, ExecResult, Machine};

/// The exec options a line of this module carries: only its timeout.
fn bound(timeout_ms: Option<u64>) -> ExecOptions {
    ExecOptions { timeout_ms, idempotency_key: None, stdin: None }
}

/// The first line of the trimmed stderr, else stdout, else `exit <code>`.
fn first_line(res: &ExecResult) -> String {
    let text = if res.stderr.is_empty() { &res.stdout } else { &res.stderr };
    let line = js_trim(text).split('\n').next().unwrap_or("");
    if line.is_empty() {
        format!("exit {}", res.exit_code.get())
    } else {
        line.to_string()
    }
}

/// node's `path.posix.dirname` for the folders a land names: the text before the last separator, a lone root kept as
/// `//`, and `/` or `.` where there is none.
fn dirname(p: &str) -> String {
    let trimmed = p.trim_end_matches('/');
    let has_root = p.starts_with('/');
    match trimmed.rfind('/').filter(|&at| at >= 1) {
        Some(1) if has_root => "//".into(),
        Some(at) => trimmed[..at].to_string(),
        None if has_root => "/".into(),
        None => ".".into(),
    }
}

/// target-line.ts:15.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLogin {
    pub platform: Platform,
    pub home: String,
    /// Absent where neither the computer nor its shell gave one (an empty PATH is absent).
    pub path: Option<String>,
    pub user: String,
    /// The user every line is handed to, where the road is root and the home is somebody else's.
    pub run_as: Option<String>,
}

/// target-line.ts:30 `given`: the home and PATH the computer reported for its login, which win over the road's own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GivenLogin {
    pub home: Option<String>,
    pub path: Option<String>,
}

/// target-line.ts:23.
pub const PROBE_MS: u64 = 20_000;
/// target-line.ts:71.
pub const LOGIN_SHELL_S: u64 = 15;

/// target-line.ts:30. One exec of the probe (exact text in spec/machine_reads.md) under `PROBE_MS`. Non-zero exit is
/// `the computer did not say who its lines run as: <first line of trimmed stderr, else stdout>`. Seven lines, trimmed:
/// system, uid, own name, the home's owner, runuser (1/0), the shell's HOME, the shell's PATH. Platform by
/// `platform_of_system`, Linux when unknown; user is the owner, else the own name. Not Linux or uid not 0: answered as
/// is. Root with no owner: `no_home_refusal(home)`. Root owning the home: as is. Root, somebody else's home, no runuser:
/// `no_runuser_refusal(user)`. Else `run_as` is that user.
pub async fn target_login(machine: &dyn Machine, given: &GivenLogin) -> Result<TargetLogin> {
    let home = match &given.home {
        Some(home) => shell_quote(home),
        None => "\"$HOME\"".to_string(),
    };
    let probe = [
        "uname -s".to_string(),
        "id -u".to_string(),
        "id -un".to_string(),
        format!("stat -L -c %U {home} 2>/dev/null || stat -L -f %Su {home} 2>/dev/null || echo"),
        "command -v runuser >/dev/null 2>&1 && echo 1 || echo 0".to_string(),
        r#"printf '%s\n' "$HOME" "$PATH""#.to_string(),
    ]
    .join("; ");
    let res = machine.exec(&probe, bound(Some(PROBE_MS))).await?;
    if res.exit_code.get() != 0 {
        return Err(EngineError::Other {
            message: format!("the computer did not say who its lines run as: {}", first_line_of_failure(&res)),
            code: None,
        });
    }
    let lines: Vec<String> = res.stdout.split('\n').map(|l| js_trim(l).to_string()).collect();
    let field = |i: usize| lines.get(i).cloned().unwrap_or_default();
    let (os, uid, self_user, owner, runuser, shell_home, shell_path) =
        (field(0), field(1), field(2), field(3), field(4), field(5), field(6));
    let platform = platform_of_system(&os).unwrap_or(Platform::Linux);
    let user = if owner.is_empty() { self_user } else { owner.clone() };
    let path = given.path.clone().unwrap_or(shell_path);
    let at = TargetLogin {
        platform,
        home: given.home.clone().unwrap_or(shell_home),
        path: (!path.is_empty()).then_some(path),
        user: user.clone(),
        run_as: None,
    };
    if platform != Platform::Linux || uid != "0" {
        return Ok(at);
    }
    if owner.is_empty() {
        return Err(EngineError::Other { message: no_home_refusal(&at.home), code: None });
    }
    if user == "root" {
        return Ok(at);
    }
    if runuser != "1" {
        return Err(EngineError::Other { message: no_runuser_refusal(&user), code: None });
    }
    Ok(TargetLogin { run_as: Some(user), ..at })
}

/// The first line of a failed probe's stderr, else stdout, as the probe's refusal names it (no `exit <code>` fallback).
fn first_line_of_failure(res: &ExecResult) -> String {
    let text = if res.stderr.is_empty() { &res.stdout } else { &res.stderr };
    js_trim(text).split('\n').next().unwrap_or("").to_string()
}

/// target-line.ts:58. The stores whose lines this login reaches, in the map's order: all of them where nothing is handed
/// to another login; else only those at or under the home (`under_project`).
pub fn stores_reached(t: &TargetLogin, stores: &IndexMap<String, String>) -> IndexMap<String, String> {
    stores
        .iter()
        .filter(|(_, store)| t.run_as.is_none() || under_project(store, &t.home))
        .map(|(agent, store)| (agent.clone(), store.clone()))
        .collect()
}

/// target-line.ts:64. `export HOME=<q home>[ PATH=<q path>]; cd "$HOME" 2>/dev/null; <line>`, wrapped as
/// `runuser -u <q run_as> -- bash -c <q inner>` where `run_as` is set.
pub fn as_login(t: &TargetLogin, line: &str) -> String {
    let path = t.path.as_ref().map_or(String::new(), |p| format!(" PATH={}", shell_quote(p)));
    let inner = format!("export HOME={}{path}; cd \"$HOME\" 2>/dev/null; {line}", shell_quote(&t.home));
    match &t.run_as {
        None => inner,
        Some(user) => format!("runuser -u {} -- bash -c {}", shell_quote(user), shell_quote(&inner)),
    }
}

/// target-line.ts:76. Linux only (`None` elsewhere, nothing run). Runs the login shell line as the login under
/// `(LOGIN_SHELL_S + 5) * 1000` ms; the last stdout line trimmed, `None` where the exec failed, exited non-zero or that
/// line is empty. Never fails.
pub async fn login_shell_path(machine: &dyn Machine, t: &TargetLogin) -> Option<String> {
    if t.platform != Platform::Linux {
        return None;
    }
    let line = format!(
        "s=$(getent passwd {} | cut -d: -f7); timeout {LOGIN_SHELL_S} \"${{s:-/bin/sh}}\" -ilc 'printf %s \"$PATH\"' < /dev/null 2> /dev/null",
        shell_quote(&t.user)
    );
    let res = machine.exec(&as_login(t, &line), bound(Some((LOGIN_SHELL_S + 5) * 1000))).await.ok()?;
    if res.exit_code.get() != 0 {
        return None;
    }
    let last = res.stdout.split('\n').next_back().unwrap_or("");
    let path = js_trim(last);
    (!path.is_empty()).then(|| path.to_string())
}

/// target-line.ts:92. Makes `/tmp/wsp-land-<12 hex>` with `mkdir -m 0700` (non-zero: `no private folder to stage <what>
/// in: <first>`), lands the bytes at `<dir>/bytes` by `land_bytes` on `land`, then `chmod 0600 <file>` plus
/// ` && chown -R -- <q run_as> <q dir>` where handed (non-zero: `the staging folder for <what> could not be handed to
/// <user>: <first>`), then `use_(file, dir)`. `rm -rf -- <q dir>` runs whatever happened once the folder was made, its
/// own failure ignored. Every exec gets `timeout_ms` where given.
pub async fn stage_as_login<T, F, Fut>(
    machine: &dyn Machine,
    land: &dyn Machine,
    t: &TargetLogin,
    what: &str,
    bytes: &[u8],
    use_: F,
    timeout_ms: Option<u64>,
) -> Result<T>
where
    F: FnOnce(String, String) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let dir = format!("/tmp/wsp-land-{}", hex::encode(rand::random::<[u8; 6]>()));
    let folder = shell_quote(&dir);
    let file = format!("{dir}/bytes");
    let made = machine.exec(&format!("mkdir -m 0700 {folder}"), bound(timeout_ms)).await?;
    if made.exit_code.get() != 0 {
        return Err(EngineError::Other { message: format!("no private folder to stage {what} in: {}", first_line(&made)), code: None });
    }
    let mut cleanup = Cleanup { machine, cmd: Some(format!("rm -rf -- {folder}")), timeout_ms };
    let staged = async {
        land_bytes(land, &file, bytes, LandBytesOptions { fetch: None, timeout_ms }).await?;
        let chown = t.run_as.as_ref().map_or(String::new(), |user| format!(" && chown -R -- {} {folder}", shell_quote(user)));
        let handed = machine.exec(&format!("chmod 0600 {}{chown}", shell_quote(&file)), bound(timeout_ms)).await?;
        if handed.exit_code.get() != 0 {
            return Err(EngineError::Other {
                message: format!("the staging folder for {what} could not be handed to {}: {}", t.user, first_line(&handed)),
                code: None,
            });
        }
        use_(file.clone(), dir.clone()).await
    }
    .await;
    if let Some(cmd) = cleanup.cmd.take() {
        let _ = machine.exec(&cmd, bound(timeout_ms)).await;
    }
    staged
}

/// The `finally` of target-line.ts:107: removes the staging folder when the caller stops waiting for the stage, which a
/// TS caller cannot do. A dropped future cannot await, so the command is started with one poll of the machine's exec
/// and its answer is not waited for (`rm -rf` ignores its own failure here as it does at the end of a finished stage).
struct Cleanup<'a> {
    machine: &'a dyn Machine,
    cmd: Option<String>,
    timeout_ms: Option<u64>,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        if let Some(cmd) = self.cmd.take() {
            let _ = futures::FutureExt::now_or_never(self.machine.exec(&cmd, bound(self.timeout_ms)));
        }
    }
}

/// target-line.ts:120. Stages as above (`what` is `dest`), then as the login either
/// `mkdir -p <q dest> && tar -xzf <q file> -C <q dest>` (unpack) or `mkdir -p <q dirname(dest)> && cat <q file> > <q dest>`;
/// non-zero is `<dest> was not written as <user>: <first>`.
pub async fn land_as_login(
    machine: &dyn Machine,
    t: &TargetLogin,
    dest: &str,
    bytes: &[u8],
    unpack: bool,
    timeout_ms: Option<u64>,
) -> Result<()> {
    stage_as_login(
        machine,
        machine,
        t,
        dest,
        bytes,
        |file, _folder| async move {
            let at = shell_quote(&file);
            let to = shell_quote(dest);
            let put = if unpack {
                format!("mkdir -p {to} && tar -xzf {at} -C {to}")
            } else {
                format!("mkdir -p {} && cat {at} > {to}", shell_quote(&dirname(dest)))
            };
            let res = machine.exec(&as_login(t, &put), bound(timeout_ms)).await?;
            if res.exit_code.get() != 0 {
                return Err(EngineError::Other {
                    message: format!("{dest} was not written as {}: {}", t.user, first_line(&res)),
                    code: None,
                });
            }
            Ok(())
        },
        timeout_ms,
    )
    .await
}
