// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use super::daemon_replies::{
    CgroupMemory, FsEntry, FsEntryType, FsFilesReply, FsHashReply, FsImageReply, FsListReply, FsSearchReply, FsWriteReply,
    GitBranchesReply, GitCheckpointDropReply, GitCheckpointReply, GitCommitReply, GitDiffFile, GitDiscardReply, GitLocalBranch,
    GitPrListReply, GitRestoreReply, GitWorktree, GitWorktreesReply, HostItem, HostItemKind, HttpBearer, HttpPostReply, KillMode, PipeDoor,
    PipeOpen, ProcKind, ProcListReply, ProcOwnedBy, ProcSignal, SshStartReply, UsageStore,
};
use super::helpers::{is_cgroup_path, is_watch_name, literal_word, ReqId, CGROUP_PATH_MAX};
use super::limits::{is_http_url, EXEC_BODY_MAX, GUEST_ARGV_MAX, GUEST_CWD_MAX, GUEST_TOKEN_MAX, TRANSCRIPT_DIRS_MAX, USAGE_STORES_MAX};
use crate::types::check::{hex_64, items_at_most, num_above, num_at_least, num_at_most, refine, regex_check, text_at_least, text_at_most};
use crate::types::pull_request::{MergeMethod, ReactionContent};
use crate::types::{Check, JsNumber, SchemaError};
use wsp_frames::{False, True};

/// Which column a list of processes is ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcSort {
    Cpu,
    Mem,
}

pub const DETACHED_HEAD: &str = "(detached)";
/// /^ssh-ed25519 [A-Za-z0-9+/]+={0,2}( [!-~]+)?$/
pub const SSH_HOST_KEY_LINE: &str = "^ssh-ed25519 [A-Za-z0-9+/]+={0,2}( [!-~]+)?$";
pub const PROC_KINDS: &[&str] = &["agent", "tool", "dev", "test", "build", "slate", "terminal", "tab", "app", "wsp", "other"];
pub const PROC_SCOPES_MAX: i64 = 256;
pub const PROC_SCOPE_PIDS_MAX: i64 = 64;
pub const KILL_PIDS_MAX: i64 = 256;
pub const EXEC_OUTPUT_MAX: i64 = 2097152;
pub const EXEC_TIMEOUT_DEFAULT_MS: i64 = 20000;
pub const EXEC_TIMEOUT_MAX_MS: i64 = 600000;
pub const EXEC_DEADLINE_EXIT: i64 = 124;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaemonLinkStatus {
    #[serde(rename = "opening")]
    Opening,
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "live")]
    Live,
    #[serde(rename = "reauth-needed")]
    ReauthNeeded,
    #[serde(rename = "refused")]
    Refused,
    #[serde(rename = "unanswered")]
    Unanswered,
    #[serde(rename = "dead")]
    Dead,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonAuthRequest {
    pub id: ReqId,
    pub op: String,
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub port: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pipe: Option<bool>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FsReadEncoding {
    #[serde(rename = "utf8")]
    Utf8,
    #[serde(rename = "base64")]
    Base64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadReply {
    pub content: String,
    pub size: JsNumber,
    pub truncated: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FsSearchMode {
    #[serde(rename = "files")]
    Files,
    #[serde(rename = "text")]
    Text,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranch {
    pub oid: String,
    pub head: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub upstream: Option<String>,
    pub ahead: JsNumber,
    pub behind: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusEntry {
    pub xy: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub orig_path: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusReply {
    pub branch: GitBranch,
    pub entries: Vec<GitStatusEntry>,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub edits_unread: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub counts_unknown: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub stashes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub default_branch: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GitDiffScope {
    #[serde(rename = "branch")]
    Branch,
    #[serde(rename = "unstaged")]
    Unstaged,
    #[serde(rename = "staged")]
    Staged,
    #[serde(rename = "head")]
    Head,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffReply {
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub base: Option<String>,
    pub files: Vec<GitDiffFile>,
    pub truncated: bool,
    #[serde(default)]
    pub moved: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSnapshotReply {
    pub commit: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyListEntry {
    pub id: String,
    pub pid: JsNumber,
    pub cols: JsNumber,
    pub rows: JsNumber,
    pub exited: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reply: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyListReply {
    pub ptys: Vec<PtyListEntry>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcOwner {
    pub by: ProcOwnedBy,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub warm: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcEntry {
    pub pid: JsInt,
    pub ppid: JsInt,
    pub user: String,
    pub state: String,
    pub comm: String,
    pub cmdline: String,
    pub cpu: JsNumber,
    pub rss: JsNumber,
    pub started_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cgroup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub energy_mw: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pgid: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "owner_or_none")]
    pub owner: Option<ProcOwner>,
    /// zod `ProcKind.catch("other").optional()`: absent stays absent; any other value that is no kind reads as other.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "proc_kind_or_other")]
    pub kind: Option<ProcKind>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcScopePid {
    pub pid: JsInt,
    pub kind: ProcKind,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcScope {
    pub thread: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<Vec<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub groups: Option<Vec<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub warm: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cgroup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pids: Option<Vec<ProcScopePid>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcInspectReply {
    pub pid: JsInt,
    #[serde(deserialize_with = "crate::types::check::nullable")]
    pub cwd: Option<String>,
    pub ports: Vec<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub threads: Option<JsInt>,
    pub children: Vec<JsInt>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcKillReply {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub signalled: Option<Vec<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub forced: Option<Vec<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub left: Option<Vec<JsInt>>,
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonExecRequest {
    pub id: ReqId,
    pub cmd: String,
    #[serde(default, deserialize_with = "crate::types::check::present")]
    pub timeout_ms: Option<JsInt>,
    #[serde(default, deserialize_with = "crate::types::check::present")]
    pub stdin: Option<String>,
}

crate::literal!(ExecLit, "exec");

/// A member of a union tagged on `op`: the union reads the tag, so the struct holds none and writes it where it is
/// written alone, in the order zod builds it.
impl DaemonExecRequest {
    pub const OP: &str = "exec";
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecAlone<'a> {
    id: &'a ReqId,
    op: &'static str,
    cmd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: &'a Option<JsInt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdin: &'a Option<String>,
}

impl Serialize for DaemonExecRequest {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ExecAlone { id: &self.id, op: Self::OP, cmd: &self.cmd, timeout_ms: &self.timeout_ms, stdin: &self.stdin }.serialize(s)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecCall<'a> {
    cmd: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: &'a Option<JsInt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stdin: &'a Option<String>,
    id: &'a ReqId,
}

/// Inside the `op` tag the call's keys come first and the id last, as the daemon channel writes a frame.
fn exec_call<S: serde::Serializer>(exec: &DaemonExecRequest, s: S) -> Result<S::Ok, S::Error> {
    ExecCall { cmd: &exec.cmd, timeout_ms: &exec.timeout_ms, stdin: &exec.stdin, id: &exec.id }.serialize(s)
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonExecReply {
    pub exit_code: JsInt,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GuestKind {
    #[serde(rename = "mcp")]
    Mcp,
    #[serde(rename = "cli")]
    Cli,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GitPrReviewEvent {
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "request_changes")]
    RequestChanges,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GitPrReviewCommentSide {
    #[serde(rename = "LEFT")]
    LEFT,
    #[serde(rename = "RIGHT")]
    RIGHT,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReviewComment {
    pub id: String,
    pub path: String,
    pub line: JsInt,
    pub side: GitPrReviewCommentSide,
    pub body: String,
}
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum DaemonRequest {
    #[serde(rename = "pty.create", rename_all = "camelCase")]
    PtyCreate {
        cols: Option<JsNumber>,
        rows: Option<JsNumber>,
        shell: Option<String>,
        cwd: Option<String>,
        env: Option<IndexMap<String, String>>,
        run: Option<String>,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "pty.attach", rename_all = "camelCase")]
    PtyAttach { pty_id: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "pty.detach", rename_all = "camelCase")]
    PtyDetach { pty_id: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "pty.write", rename_all = "camelCase")]
    PtyWrite { pty_id: String, data: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "pty.resize", rename_all = "camelCase")]
    PtyResize { pty_id: String, cols: JsNumber, rows: JsNumber, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "pty.kill", rename_all = "camelCase")]
    PtyKill { pty_id: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "pty.list", rename_all = "camelCase")]
    PtyList { machine_id: Option<String>, id: ReqId },
    #[serde(rename = "ports.watch", rename_all = "camelCase")]
    PortsWatch { roots: Option<Vec<JsInt>>, folder: Option<String>, cgroups: Option<Vec<String>>, watch: Option<String>, id: ReqId },
    #[serde(rename = "manifest.get", rename_all = "camelCase")]
    ManifestGet { id: ReqId },
    #[serde(rename = "manifest.record", rename_all = "camelCase")]
    ManifestRecord { cmd: String, cwd: String, port: Option<JsNumber>, id: ReqId },
    #[serde(rename = "manifest.restartScript", rename_all = "camelCase")]
    ManifestRestartScript { id: ReqId },
    #[serde(rename = "inbox.watch", rename_all = "camelCase")]
    InboxWatch { id: ReqId },
    #[serde(rename = "inbox.rescan", rename_all = "camelCase")]
    InboxRescan { id: ReqId },
    #[serde(rename = "sys.watch", rename_all = "camelCase")]
    SysWatch { id: ReqId },
    #[serde(rename = "sys.unwatch", rename_all = "camelCase")]
    SysUnwatch { id: ReqId },
    #[serde(rename = "sys.history", rename_all = "camelCase")]
    SysHistory { from: JsInt, to: JsInt, step_ms: JsInt, id: ReqId },
    #[serde(rename = "usage.logs", rename_all = "camelCase")]
    UsageLogs { stores: Vec<UsageStore>, id: ReqId },
    #[serde(rename = "transcripts.list", rename_all = "camelCase")]
    TranscriptsList { root: String, dirs: Vec<String>, cwds: Vec<String>, id: ReqId },
    #[serde(rename = "transcripts.read", rename_all = "camelCase")]
    TranscriptsRead { root: String, dirs: Vec<String>, session: String, last: JsInt, tail: Option<JsInt>, id: ReqId },
    #[serde(rename = "proc.watch", rename_all = "camelCase")]
    ProcWatch { cgroups: Option<Vec<String>>, id: ReqId },
    #[serde(rename = "proc.unwatch", rename_all = "camelCase")]
    ProcUnwatch { id: ReqId },
    #[serde(rename = "proc.scopes", rename_all = "camelCase")]
    ProcScopes { scopes: Vec<ProcScope>, id: ReqId },
    #[serde(rename = "proc.inspect", rename_all = "camelCase")]
    ProcInspect { pid: JsInt, id: ReqId },
    #[serde(rename = "proc.list", rename_all = "camelCase")]
    ProcList { id: ReqId },
    #[serde(rename = "proc.kill", rename_all = "camelCase")]
    ProcKill {
        pid: JsInt,
        signal: ProcSignal,
        mode: Option<KillMode>,
        started_at: Option<JsInt>,
        identity: Option<String>,
        protect: Option<Vec<JsInt>>,
        apps: Option<Vec<JsInt>>,
        agents: Option<Vec<JsInt>>,
        agent_bins: Option<Vec<String>>,
        id: ReqId,
    },
    #[serde(rename = "container.stop", rename_all = "camelCase")]
    ContainerStop { container: String, id: ReqId },
    #[serde(rename = "ping", rename_all = "camelCase")]
    Ping { id: ReqId },
    #[serde(rename = "fs.list", rename_all = "camelCase")]
    FsList { path: String, gitignore: Option<bool>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "fs.files", rename_all = "camelCase")]
    FsFiles { cwd: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "fs.read", rename_all = "camelCase")]
    FsRead { path: String, encoding: Option<FsReadEncoding>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "fs.image", rename_all = "camelCase")]
    FsImage { path: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "fs.hash", rename_all = "camelCase")]
    FsHash { root: String, paths: Vec<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "fs.write", rename_all = "camelCase")]
    FsWrite { path: String, contents: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "pty.tab", rename_all = "camelCase")]
    PtyTab { pty_id: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "fs.search", rename_all = "camelCase")]
    FsSearch { path: String, query: String, mode: FsSearchMode, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.status", rename_all = "camelCase")]
    GitStatus { cwd: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.diff", rename_all = "camelCase")]
    GitDiff {
        cwd: String,
        scope: GitDiffScope,
        path: Option<String>,
        paths: Option<Vec<String>>,
        whole: Option<bool>,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "git.discard", rename_all = "camelCase")]
    GitDiscard { cwd: String, path: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.commit", rename_all = "camelCase")]
    GitCommit { cwd: String, message: String, paths: Vec<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.snapshot", rename_all = "camelCase")]
    GitSnapshot { cwd: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.range", rename_all = "camelCase")]
    GitRange { cwd: String, from: String, to: String, path: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.turn", rename_all = "camelCase")]
    GitTurn { cwd: String, from: String, to: String, path: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.push", rename_all = "camelCase")]
    GitPush { cwd: String, base: Option<String>, token: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.pr", rename_all = "camelCase")]
    GitPr {
        cwd: String,
        base: Option<String>,
        title: Option<String>,
        body: Option<String>,
        token: Option<String>,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "git.prRead", rename_all = "camelCase")]
    GitPrRead {
        cwd: String,
        remote: String,
        branch: Option<String>,
        number: Option<JsInt>,
        seen: Option<String>,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "git.prView", rename_all = "camelCase")]
    GitPrView { cwd: String, remote: String, number: JsInt, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.runLog", rename_all = "camelCase")]
    GitRunLog { cwd: String, remote: String, run_id: JsInt, job_id: JsInt, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.prMerge", rename_all = "camelCase")]
    GitPrMerge {
        cwd: String,
        remote: String,
        number: JsInt,
        method: MergeMethod,
        auto: bool,
        head_oid: String,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "git.issueRead", rename_all = "camelCase")]
    GitIssueRead { cwd: String, remote: String, number: JsInt, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.prCheckout", rename_all = "camelCase")]
    GitPrCheckout { cwd: String, number: JsInt, token: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.prDiff", rename_all = "camelCase")]
    GitPrDiff { cwd: String, remote: String, number: JsInt, max_bytes: Option<JsInt>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.prReview", rename_all = "camelCase")]
    GitPrReview {
        cwd: String,
        remote: String,
        number: JsInt,
        head_oid: String,
        event: GitPrReviewEvent,
        body: String,
        comments: Vec<GitPrReviewComment>,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "git.prReply", rename_all = "camelCase")]
    GitPrReply {
        cwd: String,
        remote: String,
        number: JsInt,
        reply_to: Option<JsInt>,
        thread_id: Option<String>,
        body: String,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "git.prResolve", rename_all = "camelCase")]
    GitPrResolve { cwd: String, remote: String, number: JsInt, thread_id: String, resolved: bool, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.prReact", rename_all = "camelCase")]
    GitPrReact {
        cwd: String,
        remote: String,
        number: JsInt,
        subject: String,
        content: ReactionContent,
        on: bool,
        machine_id: Option<String>,
        id: ReqId,
    },
    #[serde(rename = "git.repoRead", rename_all = "camelCase")]
    GitRepoRead { cwd: String, remote: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.update", rename_all = "camelCase")]
    GitUpdate { cwd: String, base: Option<String>, token: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.startOn", rename_all = "camelCase")]
    GitStartOn { cwd: String, branch: String, token: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.branchCompare", rename_all = "camelCase")]
    GitBranchCompare { cwd: String, remote: String, base: String, head: String, id: ReqId },
    #[serde(rename = "git.mergeIn", rename_all = "camelCase")]
    GitMergeIn { cwd: String, branch: String, from: Option<String>, token: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.prList", rename_all = "camelCase")]
    GitPrList { cwd: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.checkpoint", rename_all = "camelCase")]
    GitCheckpoint { cwd: String, thread: String, turn: String, scope: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.restore", rename_all = "camelCase")]
    GitRestore { cwd: String, checkpoint: String, scope: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.checkpointDrop", rename_all = "camelCase")]
    GitCheckpointDrop { cwd: String, scope: Option<String>, thread: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.worktrees", rename_all = "camelCase")]
    GitWorktrees { cwd: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.branches", rename_all = "camelCase")]
    GitBranches { cwd: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.switchNew", rename_all = "camelCase")]
    GitSwitchNew { cwd: String, branch: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "git.fetchBranch", rename_all = "camelCase")]
    GitFetchBranch { cwd: String, remote: String, branch: String, into: Option<String>, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "fs.folders", rename_all = "camelCase")]
    FsFolders { dir: Option<String>, hidden: Option<bool>, repos: Option<bool>, projects: Option<Vec<String>>, id: ReqId },
    #[serde(rename = "tunnel.open", rename_all = "camelCase")]
    TunnelOpen { tunnel_id: String, port: JsInt, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "ssh.start", rename_all = "camelCase")]
    SshStart { authorized_key: String, machine_id: Option<String>, id: ReqId },
    #[serde(rename = "tunnel.write", rename_all = "camelCase")]
    TunnelWrite { tunnel_id: String, data: String, id: ReqId },
    #[serde(rename = "tunnel.close", rename_all = "camelCase")]
    TunnelClose { tunnel_id: String, id: ReqId },
    #[serde(rename = "stdio.open", rename_all = "camelCase")]
    StdioOpen { stdio_id: String, cmd: String, env: Option<IndexMap<String, String>>, id: ReqId },
    #[serde(rename = "stdio.write", rename_all = "camelCase")]
    StdioWrite { stdio_id: String, data: String, id: ReqId },
    #[serde(rename = "stdio.close", rename_all = "camelCase")]
    StdioClose { stdio_id: String, id: ReqId },
    #[serde(rename = "http.post", rename_all = "camelCase")]
    HttpPost {
        url: String,
        headers: IndexMap<String, String>,
        body: String,
        bearer: Option<HttpBearer>,
        timeout_ms: Option<JsInt>,
        id: ReqId,
    },
    #[serde(rename = "exec")]
    Exec(#[serde(serialize_with = "exec_call")] DaemonExecRequest),
    #[serde(rename = "place.leave", rename_all = "camelCase")]
    PlaceLeave { id: ReqId },
    #[serde(rename = "guest.open", rename_all = "camelCase")]
    GuestOpen { kind: GuestKind, token: String, turn_token: Option<String>, argv: Vec<String>, cwd: String, id: ReqId },
    #[serde(rename = "guest.send", rename_all = "camelCase")]
    GuestSend {
        #[serde(default, deserialize_with = "crate::types::kept::present")]
        message: Option<crate::types::kept::Kept>,
        id: ReqId,
    },
    #[serde(rename = "guest.watch", rename_all = "camelCase")]
    GuestWatch { id: ReqId },
    #[serde(rename = "guest.reply", rename_all = "camelCase")]
    GuestReply {
        session: String,
        #[serde(default, deserialize_with = "crate::types::kept::present")]
        message: Option<crate::types::kept::Kept>,
        id: ReqId,
    },
    #[serde(rename = "guest.close", rename_all = "camelCase")]
    GuestClose { session: String, error: Option<String>, id: ReqId },
    #[serde(rename = "place.update", rename_all = "camelCase")]
    PlaceUpdate { upload_id: String, seq: JsInt, last: bool, data: String, sha256: String, id: ReqId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestOpenReply {
    pub session: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GuestCliMessage0Stream {
    #[serde(rename = "out")]
    Out,
    #[serde(rename = "err")]
    Err,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestCliMessage0 {
    pub stream: GuestCliMessage0Stream,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestCliMessage1 {
    pub exit: JsInt,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum GuestCliMessage {
    V0(GuestCliMessage0),
    V1(GuestCliMessage1),
}

crate::zod_union!(GuestCliMessage { V0(GuestCliMessage0), V1(GuestCliMessage1) });

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "auth"
/// - `port`: int min 1
/// - `port`: int max 65535
impl Check for DaemonAuthRequest {
    fn check(&self) -> Result<(), SchemaError> {
        literal_word(&self.op, "auth").map_err(|e| e.under("op"))?;
        if let Some(port) = self.port {
            num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
            num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::lit_key(obj, "op", &mut out, "auth");
        crate::types::keys::req_key::<String>(obj, "token", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "port", &mut out, Self::rule_port);
        crate::types::keys::opt_key::<bool>(obj, "pipe", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "op" => <String as Check>::describe(rest),
                "token" => <String as Check>::describe(rest),
                "port" => <Option<i64> as Check>::describe(rest),
                "pipe" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl DaemonAuthRequest {
    fn rule_port(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(port) = *x {
            num_at_least(port.as_f64(), 1.0)?;
            num_at_most(port.as_f64(), 65535.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for FsReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "content", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "size", &mut out);
        crate::types::keys::req_key::<bool>(obj, "truncated", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "content" => <String as Check>::describe(rest),
                "size" => <JsNumber as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitBranch {
    fn check(&self) -> Result<(), SchemaError> {
        self.upstream.check().map_err(|e| e.under("upstream"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "oid", &mut out);
        crate::types::keys::req_key::<String>(obj, "head", &mut out);
        crate::types::keys::opt_key::<String>(obj, "upstream", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "ahead", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "behind", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "oid" => <String as Check>::describe(rest),
                "head" => <String as Check>::describe(rest),
                "upstream" => <Option<String> as Check>::describe(rest),
                "ahead" => <JsNumber as Check>::describe(rest),
                "behind" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitStatusEntry {
    fn check(&self) -> Result<(), SchemaError> {
        self.orig_path.check().map_err(|e| e.under("origPath"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "xy", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::opt_key::<String>(obj, "origPath", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "xy" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "origPath" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `stashes`: int min 0 exclusive
impl Check for GitStatusReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.branch.check().map_err(|e| e.under("branch"))?;
        self.entries.check().map_err(|e| e.under("entries"))?;
        if let Some(stashes) = self.stashes {
            num_above(stashes.as_f64(), 0.0).map_err(|e| e.under("stashes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<GitBranch>(obj, "branch", &mut out);
        crate::types::keys::req_key::<Vec<GitStatusEntry>>(obj, "entries", &mut out);
        crate::types::keys::req_key::<String>(obj, "root", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "editsUnread", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "countsUnknown", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "stashes", &mut out, Self::rule_stashes);
        crate::types::keys::opt_key::<String>(obj, "defaultBranch", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <GitBranch as Check>::describe(rest),
                "entries" => <Vec<GitStatusEntry> as Check>::describe(rest),
                "root" => <String as Check>::describe(rest),
                "edits_unread" => <Option<bool> as Check>::describe(rest),
                "counts_unknown" => <Option<bool> as Check>::describe(rest),
                "stashes" => <Option<i64> as Check>::describe(rest),
                "default_branch" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl GitStatusReply {
    fn rule_stashes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(stashes) = *x {
            num_above(stashes.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitDiffReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.base.check().map_err(|e| e.under("base"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        self.moved.check().map_err(|e| e.under("moved"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Option<String>>(obj, "base", &mut out);
        crate::types::keys::req_key::<Vec<GitDiffFile>>(obj, "files", &mut out);
        crate::types::keys::req_key::<bool>(obj, "truncated", &mut out);
        crate::types::keys::any_key::<Vec<String>>(obj, "moved", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "base" => <Option<String> as Check>::describe(rest),
                "files" => <Vec<GitDiffFile> as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                "moved" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitSnapshotReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "commit", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "commit" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PtyListEntry {
    fn check(&self) -> Result<(), SchemaError> {
        self.reply.check().map_err(|e| e.under("reply"))?;
        self.thread.check().map_err(|e| e.under("thread"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "pid", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "cols", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "rows", &mut out);
        crate::types::keys::req_key::<bool>(obj, "exited", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "reply", &mut out);
        crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "pid" => <JsNumber as Check>::describe(rest),
                "cols" => <JsNumber as Check>::describe(rest),
                "rows" => <JsNumber as Check>::describe(rest),
                "exited" => <bool as Check>::describe(rest),
                "reply" => <Option<bool> as Check>::describe(rest),
                "thread" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PtyListReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.ptys.check().map_err(|e| e.under("ptys"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<PtyListEntry>>(obj, "ptys", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "ptys" => <Vec<PtyListEntry> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcOwner {
    fn check(&self) -> Result<(), SchemaError> {
        self.thread.check().map_err(|e| e.under("thread"))?;
        self.workspace.check().map_err(|e| e.under("workspace"))?;
        self.warm.check().map_err(|e| e.under("warm"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ProcOwnedBy>(obj, "by", &mut out);
        crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
        crate::types::keys::opt_key::<String>(obj, "workspace", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "warm", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "by" => <ProcOwnedBy as Check>::describe(rest),
                "thread" => <Option<String> as Check>::describe(rest),
                "workspace" => <Option<String> as Check>::describe(rest),
                "warm" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `owner`: catch null: a value that does not parse reads as absent
/// - `kind`: catch "other": a value that does not parse reads as that
impl Check for ProcEntry {
    fn check(&self) -> Result<(), SchemaError> {
        self.pty.check().map_err(|e| e.under("pty"))?;
        self.cgroup.check().map_err(|e| e.under("cgroup"))?;
        self.energy_mw.check().map_err(|e| e.under("energyMw"))?;
        self.pgid.check().map_err(|e| e.under("pgid"))?;
        self.owner.check().map_err(|e| e.under("owner"))?;
        self.id.check().map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "pid", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "ppid", &mut out);
        crate::types::keys::req_key::<String>(obj, "user", &mut out);
        crate::types::keys::req_key::<String>(obj, "state", &mut out);
        crate::types::keys::req_key::<String>(obj, "comm", &mut out);
        crate::types::keys::req_key::<String>(obj, "cmdline", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "cpu", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "rss", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "pty", &mut out);
        crate::types::keys::opt_key::<String>(obj, "cgroup", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "energyMw", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "pgid", &mut out);
        crate::types::keys::opt_key::<String>(obj, "id", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "ppid" => <i64 as Check>::describe(rest),
                "user" => <String as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "comm" => <String as Check>::describe(rest),
                "cmdline" => <String as Check>::describe(rest),
                "cpu" => <JsNumber as Check>::describe(rest),
                "rss" => <JsNumber as Check>::describe(rest),
                "startedAt" => <JsNumber as Check>::describe(rest),
                "pty" => <Option<String> as Check>::describe(rest),
                "cgroup" => <Option<String> as Check>::describe(rest),
                "energyMw" => <Option<JsNumber> as Check>::describe(rest),
                "pgid" => <Option<i64> as Check>::describe(rest),
                "owner" => <Option<ProcOwner> as Check>::describe(rest),
                "kind" => None,
                "id" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `thread`: string min 1
/// - `thread`: string max 64
/// - `workspace`: string max 64
/// - `agents[]`: int min 0
/// - `agents`: array max 64
/// - `groups[]`: int min 0
/// - `groups`: array max 64
/// - `folder`: string max 4096
/// - `cgroup`: string max 512
/// - `pids`: array max 64
impl Check for ProcScope {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_least(&self.thread, 1).map_err(|e| e.under("thread"))?;
        text_at_most(&self.thread, 64).map_err(|e| e.under("thread"))?;
        if let Some(workspace) = &self.workspace {
            text_at_most(workspace, 64).map_err(|e| e.under("workspace"))?;
        }
        if let Some(list) = &self.agents {
            items_at_most(list.len(), 64).map_err(|e| e.under("agents"))?;
            for (i, id) in list.iter().enumerate() {
                num_at_least(id.as_f64(), 0.0).map_err(|e| e.under(i).under("agents"))?;
            }
        }
        if let Some(list) = &self.groups {
            items_at_most(list.len(), 64).map_err(|e| e.under("groups"))?;
            for (i, id) in list.iter().enumerate() {
                num_at_least(id.as_f64(), 0.0).map_err(|e| e.under(i).under("groups"))?;
            }
        }
        if let Some(folder) = &self.folder {
            text_at_most(folder, 4096).map_err(|e| e.under("folder"))?;
        }
        if let Some(cgroup) = &self.cgroup {
            text_at_most(cgroup, CGROUP_PATH_MAX).map_err(|e| e.under("cgroup"))?;
        }
        if let Some(pids) = &self.pids {
            items_at_most(pids.len(), 64).map_err(|e| e.under("pids"))?;
            pids.check().map_err(|e| e.under("pids"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "thread", &mut out, Self::rule_thread);
        crate::types::keys::opt_key_with::<String>(obj, "workspace", &mut out, Self::rule_workspace);
        crate::types::keys::opt_key_with::<Vec<JsInt>>(obj, "agents", &mut out, Self::rule_agents);
        crate::types::keys::opt_key_with::<Vec<JsInt>>(obj, "groups", &mut out, Self::rule_groups);
        crate::types::keys::opt_key_with::<String>(obj, "folder", &mut out, Self::rule_folder);
        crate::types::keys::opt_key::<bool>(obj, "warm", &mut out);
        crate::types::keys::opt_key_with::<String>(obj, "cgroup", &mut out, Self::rule_cgroup);
        crate::types::keys::opt_key_with::<Vec<ProcScopePid>>(obj, "pids", &mut out, Self::rule_pids);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "thread" => <String as Check>::describe(rest),
                "workspace" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<Vec<i64>> as Check>::describe(rest),
                "groups" => <Option<Vec<i64>> as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "warm" => <Option<bool> as Check>::describe(rest),
                "cgroup" => <Option<String> as Check>::describe(rest),
                "pids" => <Option<Vec<ProcScopePid>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ProcScope {
    fn rule_thread(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 64)?;
        Ok(())
    }
    fn rule_workspace(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(workspace) = x {
            text_at_most(workspace, 64)?;
        }
        Ok(())
    }
    fn rule_agents(x: &Option<Vec<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(list) = x {
            items_at_most(list.len(), 64)?;
            for (i, id) in list.iter().enumerate() {
                num_at_least(id.as_f64(), 0.0).map_err(|e| e.under(i))?;
            }
        }
        Ok(())
    }
    fn rule_groups(x: &Option<Vec<JsInt>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(list) = x {
            items_at_most(list.len(), 64)?;
            for (i, id) in list.iter().enumerate() {
                num_at_least(id.as_f64(), 0.0).map_err(|e| e.under(i))?;
            }
        }
        Ok(())
    }
    fn rule_folder(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(folder) = x {
            text_at_most(folder, 4096)?;
        }
        Ok(())
    }
    fn rule_cgroup(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(cgroup) = x {
            text_at_most(cgroup, CGROUP_PATH_MAX)?;
        }
        Ok(())
    }
    fn rule_pids(x: &Option<Vec<ProcScopePid>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(pids) = x {
            items_at_most(pids.len(), 64)?;
            pids.check()?;
        }
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `pid`: int min 0
impl Check for ProcScopePid {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "pid", &mut out, Self::rule_pid);
        crate::types::keys::req_key::<ProcKind>(obj, "kind", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "kind" => <ProcKind as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ProcScopePid {
    fn rule_pid(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcInspectReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.cwd.check().map_err(|e| e.under("cwd"))?;
        self.ports.check().map_err(|e| e.under("ports"))?;
        self.threads.check().map_err(|e| e.under("threads"))?;
        self.children.check().map_err(|e| e.under("children"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "pid", &mut out);
        crate::types::keys::req_key::<Option<String>>(obj, "cwd", &mut out);
        crate::types::keys::req_key::<Vec<JsInt>>(obj, "ports", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "threads", &mut out);
        crate::types::keys::req_key::<Vec<JsInt>>(obj, "children", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "ports" => <Vec<i64> as Check>::describe(rest),
                "threads" => <Option<i64> as Check>::describe(rest),
                "children" => <Vec<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcKillReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.signalled.check().map_err(|e| e.under("signalled"))?;
        self.forced.check().map_err(|e| e.under("forced"))?;
        self.left.check().map_err(|e| e.under("left"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<Vec<JsInt>>(obj, "signalled", &mut out);
        crate::types::keys::opt_key::<Vec<JsInt>>(obj, "forced", &mut out);
        crate::types::keys::opt_key::<Vec<JsInt>>(obj, "left", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "signalled" => <Option<Vec<i64>> as Check>::describe(rest),
                "forced" => <Option<Vec<i64>> as Check>::describe(rest),
                "left" => <Option<Vec<i64>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "exec"
/// - `cmd`: string max 16384
/// - `timeoutMs`: int min 0 exclusive
/// - `timeoutMs`: int max 600000
impl Check for DaemonExecRequest {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_most(&self.cmd, EXEC_BODY_MAX as usize).map_err(|e| e.under("cmd"))?;
        if let Some(timeout) = self.timeout_ms {
            num_above(timeout.as_f64(), 0.0).map_err(|e| e.under("timeoutMs"))?;
            num_at_most(timeout.as_f64(), EXEC_TIMEOUT_MAX_MS as f64).map_err(|e| e.under("timeoutMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::req_key::<ExecLit>(obj, "op", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "cmd", &mut out, Self::rule_cmd);
        crate::types::keys::opt_key_with::<JsInt>(obj, "timeoutMs", &mut out, Self::rule_timeout_ms);
        crate::types::keys::opt_key::<String>(obj, "stdin", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "op" => <ExecLit as Check>::describe(rest),
                "cmd" => <String as Check>::describe(rest),
                "timeoutMs" => <Option<JsInt> as Check>::describe(rest),
                "stdin" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl DaemonExecRequest {
    fn rule_cmd(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_most(x, EXEC_BODY_MAX as usize)?;
        Ok(())
    }
    fn rule_timeout_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(timeout) = *x {
            num_above(timeout.as_f64(), 0.0)?;
            num_at_most(timeout.as_f64(), EXEC_TIMEOUT_MAX_MS as f64)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonExecReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "exitCode", &mut out);
        crate::types::keys::req_key::<String>(obj, "stdout", &mut out);
        crate::types::keys::req_key::<String>(obj, "stderr", &mut out);
        crate::types::keys::req_key::<bool>(obj, "truncated", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "exitCode" => <i64 as Check>::describe(rest),
                "stdout" => <String as Check>::describe(rest),
                "stderr" => <String as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

impl Check for DaemonRequest {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            DaemonRequest::Exec(exec) => exec.check()?,
            DaemonRequest::TunnelOpen { port, .. } => {
                num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
                num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
            }
            DaemonRequest::StdioOpen { cmd, .. } => {
                text_at_most(cmd, EXEC_BODY_MAX as usize).map_err(|e| e.under("cmd"))?;
                text_at_least(cmd, 1).map_err(|e| e.under("cmd"))?;
            }
            DaemonRequest::HttpPost { url, headers, body, timeout_ms, .. } => {
                refine(is_http_url(url), "http or https URL").map_err(|e| e.under("url"))?;
                for (key, value) in headers {
                    text_at_most(value, 8192).map_err(|e| e.under(key.as_str()).under("headers"))?;
                }
                refine(headers.len() <= 64, "at most 64 headers").map_err(|e| e.under("headers"))?;
                text_at_most(body, EXEC_OUTPUT_MAX as usize).map_err(|e| e.under("body"))?;
                if let Some(v) = timeout_ms {
                    num_above(v.as_f64(), 0.0).map_err(|e| e.under("timeoutMs"))?;
                    num_at_most(v.as_f64(), EXEC_TIMEOUT_MAX_MS as f64).map_err(|e| e.under("timeoutMs"))?;
                }
            }
            DaemonRequest::PortsWatch { roots, cgroups, watch, .. } => {
                for (i, root) in roots.iter().flatten().enumerate() {
                    num_at_least(root.as_f64(), 0.0).map_err(|e| e.under(i).under("roots"))?;
                }
                check_cgroups(cgroups.as_deref())?;
                check_watch(watch.as_deref())?;
            }
            DaemonRequest::ProcWatch { cgroups, .. } => check_cgroups(cgroups.as_deref())?,
            DaemonRequest::ProcScopes { scopes, .. } => {
                items_at_most(scopes.len(), PROC_SCOPES_MAX as usize).map_err(|e| e.under("scopes"))?;
                scopes.check().map_err(|e| e.under("scopes"))?;
            }
            DaemonRequest::ProcKill { pid, identity, protect, apps, agents, agent_bins, .. } => {
                num_above(pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
                if let Some(identity) = identity {
                    text_at_most(identity, 32).map_err(|e| e.under("identity"))?;
                }
                for (key, list) in [("protect", protect), ("apps", apps), ("agents", agents)] {
                    if let Some(list) = list {
                        items_at_most(list.len(), KILL_PIDS_MAX as usize).map_err(|e| e.under(key))?;
                        for (i, pid) in list.iter().enumerate() {
                            num_at_least(pid.as_f64(), 0.0).map_err(|e| e.under(i).under(key))?;
                        }
                    }
                }
                if let Some(bins) = agent_bins {
                    items_at_most(bins.len(), 64).map_err(|e| e.under("agentBins"))?;
                    for (i, bin) in bins.iter().enumerate() {
                        text_at_most(bin, 255).map_err(|e| e.under(i).under("agentBins"))?;
                    }
                }
            }
            DaemonRequest::ContainerStop { container, .. } => {
                regex_check(hex_64(container)).map_err(|e| e.under("container"))?;
            }
            DaemonRequest::UsageLogs { stores, .. } => {
                items_at_most(stores.len(), USAGE_STORES_MAX as usize).map_err(|e| e.under("stores"))?;
            }
            DaemonRequest::TranscriptsList { root, dirs, cwds, .. } => {
                check_transcript_root(root)?;
                check_transcript_dirs(dirs)?;
                items_at_most(cwds.len(), TRANSCRIPT_DIRS_MAX as usize).map_err(|e| e.under("cwds"))?;
                for (i, cwd) in cwds.iter().enumerate() {
                    text_at_most(cwd, GUEST_CWD_MAX as usize).map_err(|e| e.under(i).under("cwds"))?;
                }
            }
            DaemonRequest::TranscriptsRead { root, dirs, session, last, tail, .. } => {
                check_transcript_root(root)?;
                check_transcript_dirs(dirs)?;
                text_at_least(session, 1).map_err(|e| e.under("session"))?;
                text_at_most(session, 64).map_err(|e| e.under("session"))?;
                num_above(last.as_f64(), 0.0).map_err(|e| e.under("last"))?;
                if let Some(tail) = tail {
                    num_above(tail.as_f64(), 0.0).map_err(|e| e.under("tail"))?;
                }
            }
            DaemonRequest::GuestOpen { token, turn_token, argv, cwd, .. } => {
                text_at_most(token, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("token"))?;
                if let Some(turn) = turn_token {
                    text_at_most(turn, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("turnToken"))?;
                }
                items_at_most(argv.len(), GUEST_ARGV_MAX as usize).map_err(|e| e.under("argv"))?;
                text_at_most(cwd, GUEST_CWD_MAX as usize).map_err(|e| e.under("cwd"))?;
            }
            DaemonRequest::GitPrReply { number, reply_to, body, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                if let Some(reply_to) = reply_to {
                    num_at_least(reply_to.as_f64(), 0.0).map_err(|e| e.under("replyTo"))?;
                }
                text_at_most(body, 65536).map_err(|e| e.under("body"))?;
            }
            DaemonRequest::SysHistory { step_ms, .. } => {
                num_at_least(step_ms.as_f64(), 0.0).map_err(|e| e.under("stepMs"))?;
            }
            DaemonRequest::ProcInspect { pid, .. } => {
                num_above(pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
            }
            DaemonRequest::GitPrRead { number: Some(number), .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
            }
            DaemonRequest::GitPrView { number, .. }
            | DaemonRequest::GitPrMerge { number, .. }
            | DaemonRequest::GitIssueRead { number, .. }
            | DaemonRequest::GitPrResolve { number, .. }
            | DaemonRequest::GitPrReact { number, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
            }
            DaemonRequest::GitRunLog { run_id, job_id, .. } => {
                num_at_least(run_id.as_f64(), 0.0).map_err(|e| e.under("runId"))?;
                num_at_least(job_id.as_f64(), 0.0).map_err(|e| e.under("jobId"))?;
            }
            DaemonRequest::GitPrDiff { number, max_bytes, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                if let Some(max_bytes) = max_bytes {
                    num_above(max_bytes.as_f64(), 0.0).map_err(|e| e.under("maxBytes"))?;
                }
            }
            DaemonRequest::GitPrCheckout { number, token, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                if let Some(token) = token {
                    text_at_most(token, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("token"))?;
                }
            }
            DaemonRequest::SshStart { authorized_key, .. } => {
                text_at_least(authorized_key, 1).map_err(|e| e.under("authorizedKey"))?;
                text_at_most(authorized_key, 1024).map_err(|e| e.under("authorizedKey"))?;
            }
            DaemonRequest::GitPush { token, .. }
            | DaemonRequest::GitPr { token, .. }
            | DaemonRequest::GitUpdate { token, .. }
            | DaemonRequest::GitStartOn { token, .. }
            | DaemonRequest::GitMergeIn { token, .. } => {
                if let Some(token) = token {
                    text_at_most(token, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("token"))?;
                }
            }
            DaemonRequest::GitPrReview { number, comments, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                comments.check().map_err(|e| e.under("comments"))?;
            }
            DaemonRequest::PlaceUpdate { upload_id, seq, sha256, .. } => {
                text_at_least(upload_id, 1).map_err(|e| e.under("uploadId"))?;
                text_at_most(upload_id, 32).map_err(|e| e.under("uploadId"))?;
                regex_check(!upload_id.is_empty() && upload_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
                    .map_err(|e| e.under("uploadId"))?;
                num_at_least(seq.as_f64(), 0.0).map_err(|e| e.under("seq"))?;
                regex_check(hex_64(sha256)).map_err(|e| e.under("sha256"))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "op", DAEMON_REQUEST_OPS)
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("op") else {
            return Some(vec![crate::types::check::missing::<Self>("op")]);
        };
        use crate::wire::daemon_checks as dc;
        match tag.as_str() {
            Some("pty.create") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "cols", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "rows", &mut out);
                crate::types::keys::opt_key::<String>(obj, "shell", &mut out);
                crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
                crate::types::keys::opt_key::<String>(obj, "run", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.attach") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.detach") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.resize") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::req_key::<JsNumber>(obj, "cols", &mut out);
                crate::types::keys::req_key::<JsNumber>(obj, "rows", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.kill") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("ports.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::opt_list::<JsInt>(obj, "roots", &mut out, None, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "folder", &mut out);
                dc::opt_list::<String>(obj, "cgroups", &mut out, Some(16), dc::cgroup);
                dc::opt_rules::<String>(obj, "watch", &mut out, dc::watch_name);
                Some(out)
            }
            Some("manifest.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("manifest.record") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cmd", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "port", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("manifest.restartScript") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("inbox.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("inbox.rescan") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.unwatch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.history") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<JsInt>(obj, "from", &mut out);
                crate::types::keys::req_key::<JsInt>(obj, "to", &mut out);
                dc::req_rules::<JsInt>(obj, "stepMs", &mut out, dc::nonneg);
                Some(out)
            }
            Some("usage.logs") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_list::<UsageStore>(obj, "stores", &mut out, Some(USAGE_STORES_MAX as usize), dc::none);
                Some(out)
            }
            Some("transcripts.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "root", &mut out, dc::root_path);
                dc::req_list::<String>(obj, "dirs", &mut out, Some(TRANSCRIPT_DIRS_MAX as usize), dc::name_255);
                dc::req_list::<String>(obj, "cwds", &mut out, Some(TRANSCRIPT_DIRS_MAX as usize), dc::path_4096);
                Some(out)
            }
            Some("transcripts.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "root", &mut out, dc::root_path);
                dc::req_list::<String>(obj, "dirs", &mut out, Some(TRANSCRIPT_DIRS_MAX as usize), dc::name_255);
                dc::req_rules::<String>(obj, "session", &mut out, dc::session);
                dc::req_rules::<JsInt>(obj, "last", &mut out, dc::positive);
                dc::opt_rules::<JsInt>(obj, "tail", &mut out, dc::positive);
                Some(out)
            }
            Some("proc.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::opt_list::<String>(obj, "cgroups", &mut out, Some(16), dc::cgroup);
                Some(out)
            }
            Some("proc.unwatch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("proc.scopes") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_list::<ProcScope>(obj, "scopes", &mut out, Some(PROC_SCOPES_MAX as usize), dc::none);
                Some(out)
            }
            Some("proc.inspect") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<JsInt>(obj, "pid", &mut out, dc::positive);
                Some(out)
            }
            Some("proc.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("proc.kill") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<JsInt>(obj, "pid", &mut out, dc::positive);
                crate::types::keys::req_key::<ProcSignal>(obj, "signal", &mut out);
                crate::types::keys::opt_key::<KillMode>(obj, "mode", &mut out);
                crate::types::keys::opt_key::<JsInt>(obj, "startedAt", &mut out);
                dc::opt_rules::<String>(obj, "identity", &mut out, dc::identity);
                dc::opt_list::<JsInt>(obj, "protect", &mut out, Some(KILL_PIDS_MAX as usize), dc::nonneg);
                dc::opt_list::<JsInt>(obj, "apps", &mut out, Some(KILL_PIDS_MAX as usize), dc::nonneg);
                dc::opt_list::<JsInt>(obj, "agents", &mut out, Some(KILL_PIDS_MAX as usize), dc::nonneg);
                dc::opt_list::<String>(obj, "agentBins", &mut out, Some(64), dc::name_255);
                Some(out)
            }
            Some("container.stop") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "container", &mut out, dc::container);
                Some(out)
            }
            Some("ping") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "gitignore", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.files") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<FsReadEncoding>(obj, "encoding", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.image") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.hash") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "root", &mut out);
                crate::types::keys::req_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::req_key::<String>(obj, "contents", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.tab") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.search") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::req_key::<String>(obj, "query", &mut out);
                crate::types::keys::req_key::<FsSearchMode>(obj, "mode", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.status") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.diff") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<GitDiffScope>(obj, "scope", &mut out);
                crate::types::keys::opt_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "whole", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.discard") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.commit") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "message", &mut out);
                crate::types::keys::req_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.snapshot") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.range") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "from", &mut out);
                crate::types::keys::req_key::<String>(obj, "to", &mut out);
                crate::types::keys::opt_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.turn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "from", &mut out);
                crate::types::keys::req_key::<String>(obj, "to", &mut out);
                crate::types::keys::opt_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.push") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "base", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.pr") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "base", &mut out);
                crate::types::keys::opt_key::<String>(obj, "title", &mut out);
                crate::types::keys::opt_key::<String>(obj, "body", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prRead") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
                dc::opt_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "seen", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prView") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.runLog") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "runId", &mut out, dc::nonneg);
                dc::req_rules::<JsInt>(obj, "jobId", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prMerge") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<MergeMethod>(obj, "method", &mut out);
                crate::types::keys::req_key::<bool>(obj, "auto", &mut out);
                crate::types::keys::req_key::<String>(obj, "headOid", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.issueRead") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prCheckout") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prDiff") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                dc::opt_rules::<JsInt>(obj, "maxBytes", &mut out, dc::positive);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prReview") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<String>(obj, "headOid", &mut out);
                crate::types::keys::req_key::<GitPrReviewEvent>(obj, "event", &mut out);
                crate::types::keys::req_key::<String>(obj, "body", &mut out);
                crate::types::keys::req_key::<Vec<GitPrReviewComment>>(obj, "comments", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prReply") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                dc::opt_rules::<JsInt>(obj, "replyTo", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                dc::req_rules::<String>(obj, "body", &mut out, dc::reply_body);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prResolve") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<bool>(obj, "resolved", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prReact") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<String>(obj, "subject", &mut out);
                crate::types::keys::req_key::<ReactionContent>(obj, "content", &mut out);
                crate::types::keys::req_key::<bool>(obj, "on", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.repoRead") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.update") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "base", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.startOn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.branchCompare") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::req_key::<String>(obj, "base", &mut out);
                crate::types::keys::req_key::<String>(obj, "head", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.mergeIn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                crate::types::keys::opt_key::<String>(obj, "from", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prList") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.checkpoint") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "thread", &mut out);
                crate::types::keys::req_key::<String>(obj, "turn", &mut out);
                crate::types::keys::opt_key::<String>(obj, "scope", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.restore") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "checkpoint", &mut out);
                crate::types::keys::opt_key::<String>(obj, "scope", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.checkpointDrop") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "scope", &mut out);
                crate::types::keys::req_key::<String>(obj, "thread", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.worktrees") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.branches") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.switchNew") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.fetchBranch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                crate::types::keys::opt_key::<String>(obj, "into", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.folders") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "dir", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "hidden", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "repos", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "projects", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                dc::req_rules::<JsInt>(obj, "port", &mut out, dc::port);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("ssh.start") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "authorizedKey", &mut out, dc::authorized_key);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("tunnel.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                dc::req_rules::<String>(obj, "cmd", &mut out, dc::stdio_cmd);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
                Some(out)
            }
            Some("stdio.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("http.post") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "url", &mut out, dc::http_url);
                dc::req_rules::<IndexMap<String, String>>(obj, "headers", &mut out, dc::headers);
                dc::req_rules::<String>(obj, "body", &mut out, dc::http_body);
                crate::types::keys::opt_key::<HttpBearer>(obj, "bearer", &mut out);
                dc::opt_rules::<JsInt>(obj, "timeoutMs", &mut out, dc::timeout);
                Some(out)
            }
            Some("exec") => <DaemonExecRequest as Check>::pass(value),
            Some("place.leave") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<GuestKind>(obj, "kind", &mut out);
                dc::req_rules::<String>(obj, "token", &mut out, dc::token);
                dc::opt_rules::<String>(obj, "turnToken", &mut out, dc::token);
                dc::req_list::<String>(obj, "argv", &mut out, Some(GUEST_ARGV_MAX as usize), dc::none);
                dc::req_rules::<String>(obj, "cwd", &mut out, dc::guest_cwd);
                Some(out)
            }
            Some("guest.send") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::any_key::<Value>(obj, "message", &mut out);
                Some(out)
            }
            Some("guest.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.reply") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "session", &mut out);
                crate::types::keys::any_key::<Value>(obj, "message", &mut out);
                Some(out)
            }
            Some("guest.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "session", &mut out);
                crate::types::keys::opt_key::<String>(obj, "error", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("place.update") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "uploadId", &mut out, dc::upload_id);
                dc::req_rules::<JsInt>(obj, "seq", &mut out, dc::nonneg);
                crate::types::keys::req_key::<bool>(obj, "last", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                dc::req_rules::<String>(obj, "sha256", &mut out, dc::sha256);
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("op", tag)]),
        }
    }
}

fn check_cgroups(cgroups: Option<&[String]>) -> Result<(), SchemaError> {
    let Some(cgroups) = cgroups else { return Ok(()) };
    items_at_most(cgroups.len(), 16).map_err(|e| e.under("cgroups"))?;
    for (i, path) in cgroups.iter().enumerate() {
        text_at_most(path, CGROUP_PATH_MAX).map_err(|e| e.under(i).under("cgroups"))?;
        refine(is_cgroup_path(path), "Invalid input").map_err(|e| e.under(i).under("cgroups"))?;
    }
    Ok(())
}

fn check_watch(watch: Option<&str>) -> Result<(), SchemaError> {
    match watch {
        Some(name) => regex_check(is_watch_name(name)).map_err(|e| e.under("watch")),
        None => Ok(()),
    }
}

fn check_transcript_root(root: &str) -> Result<(), SchemaError> {
    text_at_least(root, 1).map_err(|e| e.under("root"))?;
    text_at_most(root, 4096).map_err(|e| e.under("root"))
}

fn check_transcript_dirs(dirs: &[String]) -> Result<(), SchemaError> {
    items_at_most(dirs.len(), TRANSCRIPT_DIRS_MAX as usize).map_err(|e| e.under("dirs"))?;
    for (i, dir) in dirs.iter().enumerate() {
        text_at_most(dir, 255).map_err(|e| e.under(i).under("dirs"))?;
    }
    Ok(())
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `line`: int min 0
impl Check for GitPrReviewComment {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.line.as_f64(), 0.0).map_err(|e| e.under("line"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "line", &mut out, Self::rule_line);
        crate::types::keys::req_key::<GitPrReviewCommentSide>(obj, "side", &mut out);
        crate::types::keys::req_key::<String>(obj, "body", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "line" => <i64 as Check>::describe(rest),
                "side" => <GitPrReviewCommentSide as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl GitPrReviewComment {
    fn rule_line(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestOpenReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "session", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "session" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            GuestCliMessage::V0(v) => v.check(),
            GuestCliMessage::V1(v) => v.check(),
        }
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
        <GuestCliMessage0 as Check>::describe(path).or_else(|| <GuestCliMessage1 as Check>::describe(path))
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<GuestCliMessage0>(value),
            crate::types::check::issues_of::<GuestCliMessage1>(value),
        ]))
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage0 {
    fn check(&self) -> Result<(), SchemaError> {
        self.stream.check().map_err(|e| e.under("stream"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<GuestCliMessage0Stream>(obj, "stream", &mut out);
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "stream" => <GuestCliMessage0Stream as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage1 {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "exit", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "exit" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// Refinements on the variants of the unions above, each an issue when it fails:
// - DaemonRequest::PortsWatch.roots[]: int min 0
// - DaemonRequest::PortsWatch.cgroups[]: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
// - DaemonRequest::PortsWatch.cgroups[]: string max 512
// - DaemonRequest::PortsWatch.cgroups: array max 16
// - DaemonRequest::PortsWatch.watch: string regex /^[A-Za-z0-9_-]{1,64}$/
// - DaemonRequest::SysHistory.stepMs: int min 0
// - DaemonRequest::UsageLogs.stores: array max 32
// - DaemonRequest::TranscriptsList.root: string min 1
// - DaemonRequest::TranscriptsList.root: string max 4096
// - DaemonRequest::TranscriptsList.dirs[]: string max 255
// - DaemonRequest::TranscriptsList.dirs: array max 64
// - DaemonRequest::TranscriptsList.cwds[]: string max 4096
// - DaemonRequest::TranscriptsList.cwds: array max 64
// - DaemonRequest::TranscriptsRead.root: string min 1
// - DaemonRequest::TranscriptsRead.root: string max 4096
// - DaemonRequest::TranscriptsRead.dirs[]: string max 255
// - DaemonRequest::TranscriptsRead.dirs: array max 64
// - DaemonRequest::TranscriptsRead.session: string min 1
// - DaemonRequest::TranscriptsRead.session: string max 64
// - DaemonRequest::TranscriptsRead.last: int min 0 exclusive
// - DaemonRequest::TranscriptsRead.tail: int min 0 exclusive
// - DaemonRequest::ProcWatch.cgroups[]: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
// - DaemonRequest::ProcWatch.cgroups[]: string max 512
// - DaemonRequest::ProcWatch.cgroups: array max 16
// - DaemonRequest::ProcScopes.scopes: array max 256
// - DaemonRequest::ProcInspect.pid: int min 0 exclusive
// - DaemonRequest::ProcKill.pid: int min 0 exclusive
// - DaemonRequest::ProcKill.identity: string max 32
// - DaemonRequest::ProcKill.protect[]: int min 0
// - DaemonRequest::ProcKill.protect: array max 256
// - DaemonRequest::ProcKill.apps[]: int min 0
// - DaemonRequest::ProcKill.apps: array max 256
// - DaemonRequest::ProcKill.agents[]: int min 0
// - DaemonRequest::ProcKill.agents: array max 256
// - DaemonRequest::ProcKill.agentBins[]: string max 255
// - DaemonRequest::ProcKill.agentBins: array max 64
// - DaemonRequest::ContainerStop.container: string regex /^[0-9a-f]{64}$/
// - DaemonRequest::GitPush.token: string max 512
// - DaemonRequest::GitPr.token: string max 512
// - DaemonRequest::GitPrRead.number: int min 0
// - DaemonRequest::GitPrView.number: int min 0
// - DaemonRequest::GitRunLog.runId: int min 0
// - DaemonRequest::GitRunLog.jobId: int min 0
// - DaemonRequest::GitPrMerge.number: int min 0
// - DaemonRequest::GitIssueRead.number: int min 0
// - DaemonRequest::GitPrCheckout.number: int min 0
// - DaemonRequest::GitPrCheckout.token: string max 512
// - DaemonRequest::GitPrDiff.number: int min 0
// - DaemonRequest::GitPrDiff.maxBytes: int min 0 exclusive
// - DaemonRequest::GitPrReview.number: int min 0
// - DaemonRequest::GitPrReply.number: int min 0
// - DaemonRequest::GitPrReply.replyTo: int min 0
// - DaemonRequest::GitPrReply.body: string max 65536
// - DaemonRequest::GitPrResolve.number: int min 0
// - DaemonRequest::GitPrReact.number: int min 0
// - DaemonRequest::GitUpdate.token: string max 512
// - DaemonRequest::GitStartOn.token: string max 512
// - DaemonRequest::GitMergeIn.token: string max 512
// - DaemonRequest::TunnelOpen.port: int min 1
// - DaemonRequest::TunnelOpen.port: int max 65535
// - DaemonRequest::SshStart.authorizedKey: string min 1
// - DaemonRequest::SshStart.authorizedKey: string max 1024
// - DaemonRequest::StdioOpen.cmd: string min 1
// - DaemonRequest::StdioOpen.cmd: string max 16384
// - DaemonRequest::HttpPost.url: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
// - DaemonRequest::HttpPost.headers: refine: (val, ctx) => { const result = check(val); const setError = () => ctx.addIssue({ code: ZodIssueCode.custom, ...getIssueProperties(val) }); if (typeof Promise !=
// - DaemonRequest::HttpPost.headers{}: string max 8192
// - DaemonRequest::HttpPost.body: string max 2097152
// - DaemonRequest::HttpPost.timeoutMs: int min 0 exclusive
// - DaemonRequest::HttpPost.timeoutMs: int max 600000
// - DaemonRequest::GuestOpen.token: string max 512
// - DaemonRequest::GuestOpen.turnToken: string max 512
// - DaemonRequest::GuestOpen.argv: array max 256
// - DaemonRequest::GuestOpen.cwd: string max 4096
// - DaemonRequest::PlaceUpdate.uploadId: string min 1
// - DaemonRequest::PlaceUpdate.uploadId: string max 32
// - DaemonRequest::PlaceUpdate.uploadId: string regex /^[a-z0-9]+$/
// - DaemonRequest::PlaceUpdate.seq: int min 0
// - DaemonRequest::PlaceUpdate.sha256: string regex /^[0-9a-f]{64}$/

// daemon.ts:96
// /** The first text frame a daemon sends on a socket dialled as a pipe after its auth reply, read: the PipeOpen, or
//  * undefined for anything else, which is a daemon that serves no pipes greeting a control socket with daemon.hello.
//  * The caller closes that one socket and leaves the daemon's link alone. */
// export function pipeOpenOf(text: string): PipeOpen | undefined {
//   let frame: unknown;
//   try {
//     frame = JSON.parse(text);
//   } catch {
//     return undefined;
//   }
//   const open = PipeOpen.safeParse(frame);
//   return open.success ? open.data : undefined;
// }
pub fn pipe_open_of(text: &str) -> Option<PipeOpen> {
    let value = crate::wire::json_text::parse_json(text)?;
    crate::types::check::parse::<PipeOpen>(&value).ok()
}
// daemon.ts:411
// /** Processes heaviest first on that column, ties broken by pid so one snapshot always lays out the same way. The
//  * app's pane sorts each set of siblings by this and the command line's own list reads it flat; a second copy of the
//  * rule is how the two would come to disagree about which process is the busiest. */
// export const byProcColumn =
//   (sort: ProcSort) =>
//   (a: ProcEntry, b: ProcEntry): number => {
//     const d = sort === "cpu" ? b.cpu - a.cpu : b.rss - a.rss;
//     return d !== 0 ? d : a.pid - b.pid;
//   };
pub fn by_proc_column<P: ProcRow>(sort: ProcSort, a: &P, b: &P) -> std::cmp::Ordering {
    let by_difference = |d: f64| match d {
        d if d < 0.0 => std::cmp::Ordering::Less,
        d if d > 0.0 => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    };
    let d = match sort {
        ProcSort::Cpu => b.cpu() - a.cpu(),
        ProcSort::Mem => b.rss() - a.rss(),
    };
    // Array.prototype.sort reads a NaN from the comparator as 0, and `d !== 0` is true for a NaN, so the pid never breaks it.
    if d != 0.0 {
        return by_difference(d);
    }
    by_difference(a.pid() - b.pid())
}

/// The three numbers `by_proc_column` reads off a process row, as JavaScript holds them: the zod-shaped row here and
/// the daemon's own row in wsp-frames, which the views read, order by the one rule.
pub trait ProcRow {
    fn pid(&self) -> f64;
    fn cpu(&self) -> f64;
    fn rss(&self) -> f64;
}

impl ProcRow for ProcEntry {
    fn pid(&self) -> f64 {
        self.pid.as_f64()
    }
    fn cpu(&self) -> f64 {
        self.cpu.0
    }
    fn rss(&self) -> f64 {
        self.rss.0
    }
}

impl ProcRow for wsp_frames::ProcEntry {
    fn pid(&self) -> f64 {
        f64::from(self.pid)
    }
    fn cpu(&self) -> f64 {
        self.cpu
    }
    fn rss(&self) -> f64 {
        self.rss as f64
    }
}

/// ProcEntry.kind as zod reads it: absent is None, a known word is that kind, null or anything else is Some(Other).
pub fn proc_kind_or_other<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<ProcKind>, D::Error> {
    let word = Value::deserialize(d)?;
    Ok(Some(serde_json::from_value(word).unwrap_or(ProcKind::Other)))
}

/// ProcEntry.owner as zod reads it: `.optional().catch(undefined)`, so anything that is not a whole owner object reads as absent.
pub fn owner_or_none<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<ProcOwner>, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(if value.is_object() { ProcOwner::deserialize(value).ok() } else { None })
}

/// Reads one daemon request as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of DaemonRequest in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's fields in the
/// schema's order, then its Check. Unknown keys are dropped.
pub fn parse_daemon_request(frame: &Value) -> Result<DaemonRequest, SchemaError> {
    crate::types::check::parse::<DaemonRequest>(frame)
}

/// Every op DaemonRequest holds, in the TS union's order.
pub const DAEMON_REQUEST_OPS: &[&str] = &[
    "pty.create",
    "pty.attach",
    "pty.detach",
    "pty.write",
    "pty.resize",
    "pty.kill",
    "pty.list",
    "ports.watch",
    "manifest.get",
    "manifest.record",
    "manifest.restartScript",
    "inbox.watch",
    "inbox.rescan",
    "sys.watch",
    "sys.unwatch",
    "sys.history",
    "usage.logs",
    "transcripts.list",
    "transcripts.read",
    "proc.watch",
    "proc.unwatch",
    "proc.scopes",
    "proc.inspect",
    "proc.list",
    "proc.kill",
    "container.stop",
    "ping",
    "fs.list",
    "fs.files",
    "fs.read",
    "fs.image",
    "fs.hash",
    "fs.write",
    "pty.tab",
    "fs.search",
    "git.status",
    "git.diff",
    "git.discard",
    "git.commit",
    "git.snapshot",
    "git.range",
    "git.turn",
    "git.push",
    "git.pr",
    "git.prRead",
    "git.prView",
    "git.runLog",
    "git.prMerge",
    "git.issueRead",
    "git.prCheckout",
    "git.prDiff",
    "git.prReview",
    "git.prReply",
    "git.prResolve",
    "git.prReact",
    "git.repoRead",
    "git.update",
    "git.startOn",
    "git.branchCompare",
    "git.mergeIn",
    "git.prList",
    "git.checkpoint",
    "git.restore",
    "git.checkpointDrop",
    "git.worktrees",
    "git.branches",
    "git.switchNew",
    "git.fetchBranch",
    "fs.folders",
    "tunnel.open",
    "ssh.start",
    "tunnel.write",
    "tunnel.close",
    "stdio.open",
    "stdio.write",
    "stdio.close",
    "http.post",
    "exec",
    "place.leave",
    "guest.open",
    "guest.send",
    "guest.watch",
    "guest.reply",
    "guest.close",
    "place.update",
];

/// Reads a daemon's reply as zod's `z.union([DaemonOkResponse, DaemonErrorResponse])`: `ok: true` keeps every other
/// key in order; `ok: false` needs `error` and reads `code`, `kind` and `status` where present. A reply that is
/// neither is one invalid_union issue at the root.
pub fn parse_daemon_response(frame: &Value) -> Result<crate::wire::machine_link::DaemonResponse, SchemaError> {
    crate::types::check::parse(frame)
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for DaemonLinkStatus {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["opening", "connecting", "live", "reauth-needed", "refused", "unanswered", "dead"]),
        )
    }
}
impl Check for FsReadEncoding {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["utf8", "base64"]))
    }
}
impl Check for FsSearchMode {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["files", "text"]))
    }
}
impl Check for GitDiffScope {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["branch", "unstaged", "staged", "head"]))
    }
}
impl Check for GuestKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["mcp", "cli"]))
    }
}
impl Check for GitPrReviewEvent {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["comment", "approve", "request_changes"]))
    }
}
impl Check for GitPrReviewCommentSide {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["LEFT", "RIGHT"]))
    }
}
impl Check for GuestCliMessage0Stream {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["out", "err"]))
    }
}
