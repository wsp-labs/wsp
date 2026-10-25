// SPDX-License-Identifier: AGPL-3.0-only
//! Everything a detector may ask the computer. Detectors are pure over this seam; unit tests hand in a fake and the
//! live host hands in the real disk and processes. Port of packages/collect/src/host.ts at a3b870a6e; the spec is
//! spec/host.md.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::FutureExt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// host.ts:4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Darwin,
    Linux,
}

/// host.ts:7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatKind {
    File,
    Dir,
}

/// host.ts:6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    pub kind: StatKind,
    /// Size of the file, or of every regular file under the dir.
    pub bytes: u64,
    /// When it was last written, milliseconds since the epoch, fraction kept as node's `mtimeMs` has it.
    pub mtime_ms: f64,
}

/// host.ts:15. Every method answers; none fails: a missing path is `None` or empty.
pub trait HostFs {
    fn stat<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<Stat>>;
    /// Names directly under dir; empty when dir is missing.
    fn list<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>>;
    /// Only for files whose content is configuration, never a credential.
    fn read_text<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<String>>;
    /// Every regular file under dir, absolute and sorted, links and the dependency trees under it left out.
    fn walk<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>>;
    /// The file one line at a time; nothing when it cannot be opened. With `holding`, a line without that text may be
    /// skipped unread. An `Err` item is the TS iterator throwing part way through.
    fn lines<'a>(&'a self, path: &'a str, holding: Option<&'a str>) -> LocalBoxStream<'a, std::io::Result<String>>;
}

/// host.ts:28.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Variables added to the collector's own environment for this child, in this order.
    pub env: Option<IndexMap<String, String>>,
    /// How long the child may run before it is killed; two minutes when unset.
    pub timeout_ms: Option<u64>,
    /// Kills the child and every process under it when cancelled; a run asked after that starts nothing.
    pub signal: Option<tokio_util::sync::CancellationToken>,
}

/// What node's `spawn` throws for a command, an argument or an environment value it cannot take: a `TypeError` with
/// the code `ERR_INVALID_ARG_VALUE`, which rejects the TS promise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnRefusal {
    pub message: String,
}

impl SpawnRefusal {
    pub const CODE: &'static str = "ERR_INVALID_ARG_VALUE";
}

impl std::fmt::Display for SpawnRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SpawnRefusal {}

/// host.ts:37.
pub trait HostExec {
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool>;
    /// stdout when the command exits 0, otherwise `None`. Every child reads end of file on stdin. A command the host
    /// refuses to start reads as `None` here; `try_run` says why.
    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>>;
    /// `run`, with the refusal where node's `spawn` throws, as the TS promise rejects. A host that never refuses answers
    /// as `run` does.
    fn try_run<'a>(
        &'a self,
        cmd: &'a str,
        args: &'a [String],
        opts: RunOptions,
    ) -> LocalBoxFuture<'a, Result<Option<String>, SpawnRefusal>> {
        async move { Ok(self.run(cmd, args, opts).await) }.boxed_local()
    }
}

/// host.ts:43. Fields in the TS key order.
#[derive(Clone)]
pub struct Host {
    pub platform: Platform,
    pub home: String,
    /// SHELL of the process running the collector.
    pub shell: Option<String>,
    /// TERM_PROGRAM of the process running the collector.
    pub terminal: Option<String>,
    /// XDG_CONFIG_HOME of the process running the collector; unset, the XDG config dir is ~/.config.
    pub xdg_config_home: Option<String>,
    /// CLAUDE_CONFIG_DIR; unset, ~/.claude.
    pub claude_config_dir: Option<String>,
    /// CODEX_HOME; unset, ~/.codex.
    pub codex_home: Option<String>,
    /// The folder each agent's store variable names for a thread there, by agent id.
    pub stores: Option<IndexMap<String, String>>,
    /// What each agent's plugin seed variable holds for a thread there, by agent id.
    pub seeds: Option<IndexMap<String, String>>,
    pub fs: Rc<dyn HostFs>,
    pub exec: Rc<dyn HostExec>,
}

/// Absolute path for a `~/`-relative one (host.ts:66).
pub fn expand(home: &str, path: &str) -> String {
    if path == "~" {
        return home.to_string();
    }
    match path.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => path.to_string(),
    }
}

/// `~`-relative form of an absolute path under home; other paths come back unchanged (host.ts:72).
pub fn tilde(home: &str, path: &str) -> String {
    if path == home {
        return "~".to_string();
    }
    match path.strip_prefix(&format!("{home}/")) {
        Some(rest) => format!("~/{rest}"),
        None => path.to_string(),
    }
}
