// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon.ts.

#![allow(unused_imports)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    pub ahead: JsNumber,
    pub behind: JsNumber,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusEntry {
    pub xy: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orig_path: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusReply {
    pub branch: GitBranch,
    pub entries: Vec<GitStatusEntry>,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits_unread: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts_unknown: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stashes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgroup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_mw: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pgid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::wire::helpers::or_none")]
    pub owner: Option<ProcOwner>,
    /// zod `ProcKind.catch("other").optional()`: absent stays absent; any other value that is no kind reads as other.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "proc_kind_or_other")]
    pub kind: Option<ProcKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcScopePid {
    pub pid: i64,
    pub kind: ProcKind,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcScope {
    pub thread: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warm: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgroup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pids: Option<Vec<ProcScopePid>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcInspectReply {
    pub pid: i64,
    pub cwd: Option<String>,
    pub ports: Vec<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<i64>,
    pub children: Vec<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcKillReply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signalled: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forced: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Vec<i64>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonExecRequest {
    pub id: ReqId,
    pub cmd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin: Option<String>,
}

/// A member of a union tagged on `op`: the union writes and reads the tag, so the struct holds none.
impl DaemonExecRequest {
    pub const OP: &str = "exec";
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonExecReply {
    pub exit_code: i64,
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
    pub line: i64,
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
    pub exit: i64,
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
}

/// No refinement of its own; checks every nested field that has one.
impl Check for FsReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitBranch {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitStatusEntry {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `stashes`: int min 0 exclusive
impl Check for GitStatusReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitDiffReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GitSnapshotReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PtyListEntry {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PtyListReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcOwner {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `owner`: catch null: a value that does not parse reads as absent
/// - `kind`: catch "other": a value that does not parse reads as that
impl Check for ProcEntry {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `pid`: int min 0
impl Check for ProcScopePid {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcInspectReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcKillReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonExecReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonRequest {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `line`: int min 0
impl Check for GitPrReviewComment {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestOpenReply {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage0 {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage1 {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
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
