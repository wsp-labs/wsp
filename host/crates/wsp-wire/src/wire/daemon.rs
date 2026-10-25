// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon.ts.

#![allow(unused_imports)]
use crate::types::js_number::JsInt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::ReqId;
use crate::types::pull_request::{MergeMethod, ReactionContent};
use crate::types::{Check, JsNumber, SchemaError};
pub use wsp_frames::{
    CgroupMemory, FsEntry, FsEntryType, FsFilesReply, FsHashReply, FsImageReply, FsListReply, FsSearchReply, FsWriteReply,
    GitBranchesReply, GitCheckpointDropReply, GitCheckpointReply, GitCommitReply, GitDiffFile, GitDiscardReply, GitLocalBranch,
    GitPrListReply, GitRestoreReply, GitWorktree, GitWorktreesReply, HostItem, HostItemKind, HttpBearer, HttpPostReply, KillMode, PipeDoor,
    PipeOpen, ProcKind, ProcListReply, ProcOwnedBy, ProcSignal, SshStartReply, UsageStore,
};
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
    pub port: Option<i64>,
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
    pub pid: i64,
    pub ppid: i64,
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
    pub pgid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::wire::helpers::or_none")]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonExecRequest {
    pub id: ReqId,
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub timeout_ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub stdin: Option<String>,
}

/// A member of a union tagged on `op`: the union writes and reads the tag, so the struct holds none.
impl DaemonExecRequest {
    pub const OP: &str = "exec";
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
        id: ReqId,
        cols: Option<JsNumber>,
        rows: Option<JsNumber>,
        shell: Option<String>,
        cwd: Option<String>,
        env: Option<IndexMap<String, String>>,
        run: Option<String>,
        machine_id: Option<String>,
    },
    #[serde(rename = "pty.attach", rename_all = "camelCase")]
    PtyAttach { id: ReqId, pty_id: String, machine_id: Option<String> },
    #[serde(rename = "pty.detach", rename_all = "camelCase")]
    PtyDetach { id: ReqId, pty_id: String, machine_id: Option<String> },
    #[serde(rename = "pty.write", rename_all = "camelCase")]
    PtyWrite { id: ReqId, pty_id: String, data: String, machine_id: Option<String> },
    #[serde(rename = "pty.resize", rename_all = "camelCase")]
    PtyResize { id: ReqId, pty_id: String, cols: JsNumber, rows: JsNumber, machine_id: Option<String> },
    #[serde(rename = "pty.kill", rename_all = "camelCase")]
    PtyKill { id: ReqId, pty_id: String, machine_id: Option<String> },
    #[serde(rename = "pty.list", rename_all = "camelCase")]
    PtyList { id: ReqId, machine_id: Option<String> },
    #[serde(rename = "ports.watch", rename_all = "camelCase")]
    PortsWatch { id: ReqId, roots: Option<Vec<i64>>, folder: Option<String>, cgroups: Option<Vec<String>>, watch: Option<String> },
    #[serde(rename = "manifest.get", rename_all = "camelCase")]
    ManifestGet { id: ReqId },
    #[serde(rename = "manifest.record", rename_all = "camelCase")]
    ManifestRecord { id: ReqId, cmd: String, cwd: String, port: Option<JsNumber> },
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
    SysHistory { id: ReqId, from: i64, to: i64, step_ms: i64 },
    #[serde(rename = "usage.logs", rename_all = "camelCase")]
    UsageLogs { id: ReqId, stores: Vec<UsageStore> },
    #[serde(rename = "transcripts.list", rename_all = "camelCase")]
    TranscriptsList { id: ReqId, root: String, dirs: Vec<String>, cwds: Vec<String> },
    #[serde(rename = "transcripts.read", rename_all = "camelCase")]
    TranscriptsRead { id: ReqId, root: String, dirs: Vec<String>, session: String, last: i64, tail: Option<i64> },
    #[serde(rename = "proc.watch", rename_all = "camelCase")]
    ProcWatch { id: ReqId, cgroups: Option<Vec<String>> },
    #[serde(rename = "proc.unwatch", rename_all = "camelCase")]
    ProcUnwatch { id: ReqId },
    #[serde(rename = "proc.scopes", rename_all = "camelCase")]
    ProcScopes { id: ReqId, scopes: Vec<ProcScope> },
    #[serde(rename = "proc.inspect", rename_all = "camelCase")]
    ProcInspect { id: ReqId, pid: i64 },
    #[serde(rename = "proc.list", rename_all = "camelCase")]
    ProcList { id: ReqId },
    #[serde(rename = "proc.kill", rename_all = "camelCase")]
    ProcKill {
        id: ReqId,
        pid: i64,
        signal: ProcSignal,
        mode: Option<KillMode>,
        started_at: Option<i64>,
        identity: Option<String>,
        protect: Option<Vec<i64>>,
        apps: Option<Vec<i64>>,
        agents: Option<Vec<i64>>,
        agent_bins: Option<Vec<String>>,
    },
    #[serde(rename = "container.stop", rename_all = "camelCase")]
    ContainerStop { id: ReqId, container: String },
    #[serde(rename = "ping", rename_all = "camelCase")]
    Ping { id: ReqId },
    #[serde(rename = "fs.list", rename_all = "camelCase")]
    FsList { id: ReqId, path: String, gitignore: Option<bool>, machine_id: Option<String> },
    #[serde(rename = "fs.files", rename_all = "camelCase")]
    FsFiles { id: ReqId, cwd: String, machine_id: Option<String> },
    #[serde(rename = "fs.read", rename_all = "camelCase")]
    FsRead { id: ReqId, path: String, encoding: Option<FsReadEncoding>, machine_id: Option<String> },
    #[serde(rename = "fs.image", rename_all = "camelCase")]
    FsImage { id: ReqId, path: String, machine_id: Option<String> },
    #[serde(rename = "fs.hash", rename_all = "camelCase")]
    FsHash { id: ReqId, root: String, paths: Vec<String>, machine_id: Option<String> },
    #[serde(rename = "fs.write", rename_all = "camelCase")]
    FsWrite { id: ReqId, path: String, contents: String, machine_id: Option<String> },
    #[serde(rename = "pty.tab", rename_all = "camelCase")]
    PtyTab { id: ReqId, pty_id: String, machine_id: Option<String> },
    #[serde(rename = "fs.search", rename_all = "camelCase")]
    FsSearch { id: ReqId, path: String, query: String, mode: FsSearchMode, machine_id: Option<String> },
    #[serde(rename = "git.status", rename_all = "camelCase")]
    GitStatus { id: ReqId, cwd: String, machine_id: Option<String> },
    #[serde(rename = "git.diff", rename_all = "camelCase")]
    GitDiff {
        id: ReqId,
        cwd: String,
        scope: GitDiffScope,
        path: Option<String>,
        paths: Option<Vec<String>>,
        whole: Option<bool>,
        machine_id: Option<String>,
    },
    #[serde(rename = "git.discard", rename_all = "camelCase")]
    GitDiscard { id: ReqId, cwd: String, path: String, machine_id: Option<String> },
    #[serde(rename = "git.commit", rename_all = "camelCase")]
    GitCommit { id: ReqId, cwd: String, message: String, paths: Vec<String>, machine_id: Option<String> },
    #[serde(rename = "git.snapshot", rename_all = "camelCase")]
    GitSnapshot { id: ReqId, cwd: String, machine_id: Option<String> },
    #[serde(rename = "git.range", rename_all = "camelCase")]
    GitRange { id: ReqId, cwd: String, from: String, to: String, path: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.turn", rename_all = "camelCase")]
    GitTurn { id: ReqId, cwd: String, from: String, to: String, path: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.push", rename_all = "camelCase")]
    GitPush { id: ReqId, cwd: String, base: Option<String>, token: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.pr", rename_all = "camelCase")]
    GitPr {
        id: ReqId,
        cwd: String,
        base: Option<String>,
        title: Option<String>,
        body: Option<String>,
        token: Option<String>,
        machine_id: Option<String>,
    },
    #[serde(rename = "git.prRead", rename_all = "camelCase")]
    GitPrRead {
        id: ReqId,
        cwd: String,
        remote: String,
        branch: Option<String>,
        number: Option<i64>,
        seen: Option<String>,
        machine_id: Option<String>,
    },
    #[serde(rename = "git.prView", rename_all = "camelCase")]
    GitPrView { id: ReqId, cwd: String, remote: String, number: i64, machine_id: Option<String> },
    #[serde(rename = "git.runLog", rename_all = "camelCase")]
    GitRunLog { id: ReqId, cwd: String, remote: String, run_id: i64, job_id: i64, machine_id: Option<String> },
    #[serde(rename = "git.prMerge", rename_all = "camelCase")]
    GitPrMerge {
        id: ReqId,
        cwd: String,
        remote: String,
        number: i64,
        method: MergeMethod,
        auto: bool,
        head_oid: String,
        machine_id: Option<String>,
    },
    #[serde(rename = "git.issueRead", rename_all = "camelCase")]
    GitIssueRead { id: ReqId, cwd: String, remote: String, number: i64, machine_id: Option<String> },
    #[serde(rename = "git.prCheckout", rename_all = "camelCase")]
    GitPrCheckout { id: ReqId, cwd: String, number: i64, token: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.prDiff", rename_all = "camelCase")]
    GitPrDiff { id: ReqId, cwd: String, remote: String, number: i64, max_bytes: Option<i64>, machine_id: Option<String> },
    #[serde(rename = "git.prReview", rename_all = "camelCase")]
    GitPrReview {
        id: ReqId,
        cwd: String,
        remote: String,
        number: i64,
        head_oid: String,
        event: GitPrReviewEvent,
        body: String,
        comments: Vec<GitPrReviewComment>,
        machine_id: Option<String>,
    },
    #[serde(rename = "git.prReply", rename_all = "camelCase")]
    GitPrReply {
        id: ReqId,
        cwd: String,
        remote: String,
        number: i64,
        reply_to: Option<i64>,
        thread_id: Option<String>,
        body: String,
        machine_id: Option<String>,
    },
    #[serde(rename = "git.prResolve", rename_all = "camelCase")]
    GitPrResolve { id: ReqId, cwd: String, remote: String, number: i64, thread_id: String, resolved: bool, machine_id: Option<String> },
    #[serde(rename = "git.prReact", rename_all = "camelCase")]
    GitPrReact {
        id: ReqId,
        cwd: String,
        remote: String,
        number: i64,
        subject: String,
        content: ReactionContent,
        on: bool,
        machine_id: Option<String>,
    },
    #[serde(rename = "git.repoRead", rename_all = "camelCase")]
    GitRepoRead { id: ReqId, cwd: String, remote: String, machine_id: Option<String> },
    #[serde(rename = "git.update", rename_all = "camelCase")]
    GitUpdate { id: ReqId, cwd: String, base: Option<String>, token: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.startOn", rename_all = "camelCase")]
    GitStartOn { id: ReqId, cwd: String, branch: String, token: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.branchCompare", rename_all = "camelCase")]
    GitBranchCompare { id: ReqId, cwd: String, remote: String, base: String, head: String },
    #[serde(rename = "git.mergeIn", rename_all = "camelCase")]
    GitMergeIn { id: ReqId, cwd: String, branch: String, from: Option<String>, token: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.prList", rename_all = "camelCase")]
    GitPrList { id: ReqId, cwd: String, machine_id: Option<String> },
    #[serde(rename = "git.checkpoint", rename_all = "camelCase")]
    GitCheckpoint { id: ReqId, cwd: String, thread: String, turn: String, scope: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.restore", rename_all = "camelCase")]
    GitRestore { id: ReqId, cwd: String, checkpoint: String, scope: Option<String>, machine_id: Option<String> },
    #[serde(rename = "git.checkpointDrop", rename_all = "camelCase")]
    GitCheckpointDrop { id: ReqId, cwd: String, scope: Option<String>, thread: String, machine_id: Option<String> },
    #[serde(rename = "git.worktrees", rename_all = "camelCase")]
    GitWorktrees { id: ReqId, cwd: String, machine_id: Option<String> },
    #[serde(rename = "git.branches", rename_all = "camelCase")]
    GitBranches { id: ReqId, cwd: String, machine_id: Option<String> },
    #[serde(rename = "git.switchNew", rename_all = "camelCase")]
    GitSwitchNew { id: ReqId, cwd: String, branch: String, machine_id: Option<String> },
    #[serde(rename = "git.fetchBranch", rename_all = "camelCase")]
    GitFetchBranch { id: ReqId, cwd: String, remote: String, branch: String, into: Option<String>, machine_id: Option<String> },
    #[serde(rename = "fs.folders", rename_all = "camelCase")]
    FsFolders { id: ReqId, dir: Option<String>, hidden: Option<bool>, repos: Option<bool>, projects: Option<Vec<String>> },
    #[serde(rename = "tunnel.open", rename_all = "camelCase")]
    TunnelOpen { id: ReqId, tunnel_id: String, port: i64, machine_id: Option<String> },
    #[serde(rename = "ssh.start", rename_all = "camelCase")]
    SshStart { id: ReqId, authorized_key: String, machine_id: Option<String> },
    #[serde(rename = "tunnel.write", rename_all = "camelCase")]
    TunnelWrite { id: ReqId, tunnel_id: String, data: String },
    #[serde(rename = "tunnel.close", rename_all = "camelCase")]
    TunnelClose { id: ReqId, tunnel_id: String },
    #[serde(rename = "stdio.open", rename_all = "camelCase")]
    StdioOpen { id: ReqId, stdio_id: String, cmd: String, env: Option<IndexMap<String, String>> },
    #[serde(rename = "stdio.write", rename_all = "camelCase")]
    StdioWrite { id: ReqId, stdio_id: String, data: String },
    #[serde(rename = "stdio.close", rename_all = "camelCase")]
    StdioClose { id: ReqId, stdio_id: String },
    #[serde(rename = "http.post", rename_all = "camelCase")]
    HttpPost {
        id: ReqId,
        url: String,
        headers: IndexMap<String, String>,
        body: String,
        bearer: Option<HttpBearer>,
        timeout_ms: Option<i64>,
    },
    #[serde(rename = "exec")]
    Exec(DaemonExecRequest),
    #[serde(rename = "place.leave", rename_all = "camelCase")]
    PlaceLeave { id: ReqId },
    #[serde(rename = "guest.open", rename_all = "camelCase")]
    GuestOpen { id: ReqId, kind: GuestKind, token: String, turn_token: Option<String>, argv: Vec<String>, cwd: String },
    #[serde(rename = "guest.send", rename_all = "camelCase")]
    GuestSend { id: ReqId, message: Value },
    #[serde(rename = "guest.watch", rename_all = "camelCase")]
    GuestWatch { id: ReqId },
    #[serde(rename = "guest.reply", rename_all = "camelCase")]
    GuestReply { id: ReqId, session: String, message: Value },
    #[serde(rename = "guest.close", rename_all = "camelCase")]
    GuestClose { id: ReqId, session: String, error: Option<String> },
    #[serde(rename = "place.update", rename_all = "camelCase")]
    PlaceUpdate { id: ReqId, upload_id: String, seq: i64, last: bool, data: String, sha256: String },
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GuestCliMessage {
    V0(GuestCliMessage0),
    V1(GuestCliMessage1),
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "auth"
/// - `port`: int min 1
/// - `port`: int max 65535
impl Check for DaemonAuthRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("op") { None => out.push(crate::types::check::missing::<Self>("op")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("op"))), } } }
        match obj.get("token") { None => out.push(crate::types::check::missing::<Self>("token")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("token"))), } } }
        if let Some(v) = obj.get("port") { match crate::types::check::read_present::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("port"))), } }
        if let Some(v) = obj.get("pipe") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pipe"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for FsReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("content") { None => out.push(crate::types::check::missing::<Self>("content")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("content"))), } } }
        match obj.get("size") { None => out.push(crate::types::check::missing::<Self>("size")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("size"))), } } }
        match obj.get("truncated") { None => out.push(crate::types::check::missing::<Self>("truncated")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("truncated"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitBranch {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("oid") { None => out.push(crate::types::check::missing::<Self>("oid")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("oid"))), } } }
        match obj.get("head") { None => out.push(crate::types::check::missing::<Self>("head")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("head"))), } } }
        if let Some(v) = obj.get("upstream") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("upstream"))), } }
        match obj.get("ahead") { None => out.push(crate::types::check::missing::<Self>("ahead")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ahead"))), } } }
        match obj.get("behind") { None => out.push(crate::types::check::missing::<Self>("behind")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("behind"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitStatusEntry {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("xy") { None => out.push(crate::types::check::missing::<Self>("xy")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("xy"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        if let Some(v) = obj.get("origPath") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("origPath"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `stashes`: int min 0 exclusive
impl Check for GitStatusReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("branch") { None => out.push(crate::types::check::missing::<Self>("branch")), Some(v) => { match crate::types::check::read::<GitBranch>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("branch"))), } } }
        match obj.get("entries") { None => out.push(crate::types::check::missing::<Self>("entries")), Some(v) => { match crate::types::check::read::<Vec<GitStatusEntry>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("entries"))), } } }
        match obj.get("root") { None => out.push(crate::types::check::missing::<Self>("root")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("root"))), } } }
        if let Some(v) = obj.get("editsUnread") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("editsUnread"))), } }
        if let Some(v) = obj.get("countsUnknown") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("countsUnknown"))), } }
        if let Some(v) = obj.get("stashes") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stashes"))), } }
        if let Some(v) = obj.get("defaultBranch") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("defaultBranch"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitDiffReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("base") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("base"))), } }
        match obj.get("files") { None => out.push(crate::types::check::missing::<Self>("files")), Some(v) => { match crate::types::check::read::<Vec<GitDiffFile>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("files"))), } } }
        match obj.get("truncated") { None => out.push(crate::types::check::missing::<Self>("truncated")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("truncated"))), } } }
        if let Some(v) = obj.get("moved") { match crate::types::check::read::<Vec<String>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("moved"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitSnapshotReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("commit") { None => out.push(crate::types::check::missing::<Self>("commit")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("commit"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for PtyListEntry {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("pid") { None => out.push(crate::types::check::missing::<Self>("pid")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } } }
        match obj.get("cols") { None => out.push(crate::types::check::missing::<Self>("cols")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cols"))), } } }
        match obj.get("rows") { None => out.push(crate::types::check::missing::<Self>("rows")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rows"))), } } }
        match obj.get("exited") { None => out.push(crate::types::check::missing::<Self>("exited")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exited"))), } } }
        if let Some(v) = obj.get("reply") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("reply"))), } }
        if let Some(v) = obj.get("thread") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("thread"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for PtyListReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("ptys") { None => out.push(crate::types::check::missing::<Self>("ptys")), Some(v) => { match crate::types::check::read::<Vec<PtyListEntry>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ptys"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcOwner {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("by") { None => out.push(crate::types::check::missing::<Self>("by")), Some(v) => { match crate::types::check::read::<ProcOwnedBy>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("by"))), } } }
        if let Some(v) = obj.get("thread") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("thread"))), } }
        if let Some(v) = obj.get("workspace") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspace"))), } }
        if let Some(v) = obj.get("warm") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("warm"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `owner`: catch null: a value that does not parse reads as absent
/// - `kind`: catch "other": a value that does not parse reads as that
impl Check for ProcEntry {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("pid") { None => out.push(crate::types::check::missing::<Self>("pid")), Some(v) => { match crate::types::check::read::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } } }
        match obj.get("ppid") { None => out.push(crate::types::check::missing::<Self>("ppid")), Some(v) => { match crate::types::check::read::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ppid"))), } } }
        match obj.get("user") { None => out.push(crate::types::check::missing::<Self>("user")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("user"))), } } }
        match obj.get("state") { None => out.push(crate::types::check::missing::<Self>("state")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("state"))), } } }
        match obj.get("comm") { None => out.push(crate::types::check::missing::<Self>("comm")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("comm"))), } } }
        match obj.get("cmdline") { None => out.push(crate::types::check::missing::<Self>("cmdline")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cmdline"))), } } }
        match obj.get("cpu") { None => out.push(crate::types::check::missing::<Self>("cpu")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cpu"))), } } }
        match obj.get("rss") { None => out.push(crate::types::check::missing::<Self>("rss")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("rss"))), } } }
        match obj.get("startedAt") { None => out.push(crate::types::check::missing::<Self>("startedAt")), Some(v) => { match crate::types::check::read::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("startedAt"))), } } }
        if let Some(v) = obj.get("pty") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pty"))), } }
        if let Some(v) = obj.get("cgroup") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cgroup"))), } }
        if let Some(v) = obj.get("energyMw") { match crate::types::check::read_present::<JsNumber>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("energyMw"))), } }
        if let Some(v) = obj.get("pgid") { match crate::types::check::read_present::<i64>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pgid"))), } }
        if let Some(v) = obj.get("id") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } }
        Some(out)
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
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("thread") { None => out.push(crate::types::check::missing::<Self>("thread")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("thread"))), } } }
        if let Some(v) = obj.get("workspace") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("workspace"))), } }
        if let Some(v) = obj.get("agents") { match crate::types::check::read_present::<Vec<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("agents"))), } }
        if let Some(v) = obj.get("groups") { match crate::types::check::read_present::<Vec<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("groups"))), } }
        if let Some(v) = obj.get("folder") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("folder"))), } }
        if let Some(v) = obj.get("warm") { match crate::types::check::read_present::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("warm"))), } }
        if let Some(v) = obj.get("cgroup") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cgroup"))), } }
        if let Some(v) = obj.get("pids") { match crate::types::check::read_present::<Vec<ProcScopePid>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pids"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `pid`: int min 0
impl Check for ProcScopePid {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("pid") { None => out.push(crate::types::check::missing::<Self>("pid")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } } }
        match obj.get("kind") { None => out.push(crate::types::check::missing::<Self>("kind")), Some(v) => { match crate::types::check::read::<ProcKind>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("kind"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcInspectReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("pid") { None => out.push(crate::types::check::missing::<Self>("pid")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("pid"))), } } }
        if let Some(v) = obj.get("cwd") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cwd"))), } }
        match obj.get("ports") { None => out.push(crate::types::check::missing::<Self>("ports")), Some(v) => { match crate::types::check::read::<Vec<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("ports"))), } } }
        if let Some(v) = obj.get("threads") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("threads"))), } }
        match obj.get("children") { None => out.push(crate::types::check::missing::<Self>("children")), Some(v) => { match crate::types::check::read::<Vec<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("children"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcKillReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        if let Some(v) = obj.get("signalled") { match crate::types::check::read_present::<Vec<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("signalled"))), } }
        if let Some(v) = obj.get("forced") { match crate::types::check::read_present::<Vec<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("forced"))), } }
        if let Some(v) = obj.get("left") { match crate::types::check::read_present::<Vec<JsInt>>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("left"))), } }
        Some(out)
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "exec"
/// - `cmd`: string max 16384
/// - `timeoutMs`: int min 0 exclusive
/// - `timeoutMs`: int max 600000
impl Check for DaemonExecRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<ReqId>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("cmd") { None => out.push(crate::types::check::missing::<Self>("cmd")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("cmd"))), } } }
        if let Some(v) = obj.get("timeoutMs") { match crate::types::check::read_present::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("timeoutMs"))), } }
        if let Some(v) = obj.get("stdin") { match crate::types::check::read_present::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stdin"))), } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonExecReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("exitCode") { None => out.push(crate::types::check::missing::<Self>("exitCode")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exitCode"))), } } }
        match obj.get("stdout") { None => out.push(crate::types::check::missing::<Self>("stdout")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stdout"))), } } }
        match obj.get("stderr") { None => out.push(crate::types::check::missing::<Self>("stderr")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stderr"))), } } }
        match obj.get("truncated") { None => out.push(crate::types::check::missing::<Self>("truncated")), Some(v) => { match crate::types::check::read::<bool>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("truncated"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("op") else { return Some(vec![crate::types::check::missing::<Self>("op")]); };
        match tag.as_str() {
            Some("exec") => { let mut body = obj.clone(); body.shift_remove("op"); Some(crate::types::check::issues_of::<DaemonExecRequest>(&serde_json::Value::Object(body))) }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("op", tag)]),
        }
    }

}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `line`: int min 0
impl Check for GitPrReviewComment {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("id") { None => out.push(crate::types::check::missing::<Self>("id")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("id"))), } } }
        match obj.get("path") { None => out.push(crate::types::check::missing::<Self>("path")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("path"))), } } }
        match obj.get("line") { None => out.push(crate::types::check::missing::<Self>("line")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("line"))), } } }
        match obj.get("side") { None => out.push(crate::types::check::missing::<Self>("side")), Some(v) => { match crate::types::check::read::<GitPrReviewCommentSide>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("side"))), } } }
        match obj.get("body") { None => out.push(crate::types::check::missing::<Self>("body")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("body"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestOpenReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("session") { None => out.push(crate::types::check::missing::<Self>("session")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("session"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![crate::types::check::issues_of::<GuestCliMessage0>(value), crate::types::check::issues_of::<GuestCliMessage1>(value)]))
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage0 {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("stream") { None => out.push(crate::types::check::missing::<Self>("stream")), Some(v) => { match crate::types::check::read::<GuestCliMessage0Stream>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("stream"))), } } }
        match obj.get("text") { None => out.push(crate::types::check::missing::<Self>("text")), Some(v) => { match crate::types::check::read::<String>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("text"))), } } }
        Some(out)
    }

}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage1 {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        match obj.get("exit") { None => out.push(crate::types::check::missing::<Self>("exit")), Some(v) => { match crate::types::check::read::<JsInt>(v) { Ok(_) => {}, Err(es) => out.extend(es.into_iter().map(|e| e.under("exit"))), } } }
        Some(out)
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
    todo!()
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
pub fn by_proc_column(sort: ProcSort, a: &ProcEntry, b: &ProcEntry) -> std::cmp::Ordering {
    todo!()
}

/// ProcEntry.kind as zod reads it: absent is None, a known word is that kind, null or anything else is Some(Other).
pub fn proc_kind_or_other<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<ProcKind>, D::Error> {
    let _ = d;
    todo!()
}

/// Reads one daemon request as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of DaemonRequest in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's first failing
/// field, then its Check. Unknown keys are dropped.
pub fn parse_daemon_request(frame: &Value) -> Result<DaemonRequest, SchemaError> {
    let _ = frame;
    todo!()
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
    let _ = frame;
    todo!()
}

// The shapes wsp-frames holds read every bound while deserializing, so each Check has nothing left to refuse.
impl Check for CgroupMemory {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsEntry {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsFilesReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsHashReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsImageReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsListReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsSearchReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsWriteReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitBranchesReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitCheckpointDropReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitCheckpointReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitCommitReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitDiffFile {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitDiscardReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitLocalBranch {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitPrListReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitRestoreReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitWorktree {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitWorktreesReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for HostItem {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for HttpBearer {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for HttpPostReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for PipeOpen {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for ProcListReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for SshStartReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for UsageStore {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

// A word list zod reads with z.enum: serde refuses any other word, so nothing is left to check.
impl Check for DaemonLinkStatus {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsReadEncoding {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for FsSearchMode {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitDiffScope {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GuestKind {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitPrReviewEvent {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GitPrReviewCommentSide {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
impl Check for GuestCliMessage0Stream {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
