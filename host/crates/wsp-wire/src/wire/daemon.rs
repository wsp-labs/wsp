// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/daemon.ts.

#![allow(unused_imports)]
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::helpers::{is_cgroup_path, is_watch_name, literal_word, op_discriminator, ReqId, CGROUP_PATH_MAX};
use super::limits::{is_http_url, EXEC_BODY_MAX, GUEST_ARGV_MAX, GUEST_CWD_MAX, GUEST_TOKEN_MAX, TRANSCRIPT_DIRS_MAX, USAGE_STORES_MAX};
use crate::types::check::{hex_64, items_at_most, num_above, num_at_least, num_at_most, refine, regex_check, text_at_least, text_at_most};
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

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum DaemonRequestFrame {
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

impl Check for DaemonRequestFrame {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        let Some((op, rest)) = path.split_first() else {
            return Some(crate::types::check::Zod::Word("object"));
        };
        let op = op.as_str()?;
        if op == "exec" {
            return <DaemonExecRequest as Check>::describe(rest);
        }
        let Some((key, tail)) = rest.split_first() else {
            return Some(crate::types::check::Zod::Word("object"));
        };
        match (op, key.as_str()?) {
            ("pty.create", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.create", "cols") => <Option<JsNumber> as Check>::describe(tail),
            ("pty.create", "rows") => <Option<JsNumber> as Check>::describe(tail),
            ("pty.create", "shell") => <Option<String> as Check>::describe(tail),
            ("pty.create", "cwd") => <Option<String> as Check>::describe(tail),
            ("pty.create", "env") => <Option<IndexMap<String, String>> as Check>::describe(tail),
            ("pty.create", "run") => <Option<String> as Check>::describe(tail),
            ("pty.create", "machineId") => <Option<String> as Check>::describe(tail),
            ("pty.attach", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.attach", "ptyId") => <String as Check>::describe(tail),
            ("pty.attach", "machineId") => <Option<String> as Check>::describe(tail),
            ("pty.detach", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.detach", "ptyId") => <String as Check>::describe(tail),
            ("pty.detach", "machineId") => <Option<String> as Check>::describe(tail),
            ("pty.write", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.write", "ptyId") => <String as Check>::describe(tail),
            ("pty.write", "data") => <String as Check>::describe(tail),
            ("pty.write", "machineId") => <Option<String> as Check>::describe(tail),
            ("pty.resize", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.resize", "ptyId") => <String as Check>::describe(tail),
            ("pty.resize", "cols") => <JsNumber as Check>::describe(tail),
            ("pty.resize", "rows") => <JsNumber as Check>::describe(tail),
            ("pty.resize", "machineId") => <Option<String> as Check>::describe(tail),
            ("pty.kill", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.kill", "ptyId") => <String as Check>::describe(tail),
            ("pty.kill", "machineId") => <Option<String> as Check>::describe(tail),
            ("pty.list", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.list", "machineId") => <Option<String> as Check>::describe(tail),
            ("ports.watch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("ports.watch", "roots") => <Option<Vec<i64>> as Check>::describe(tail),
            ("ports.watch", "folder") => <Option<String> as Check>::describe(tail),
            ("ports.watch", "cgroups") => <Option<Vec<String>> as Check>::describe(tail),
            ("ports.watch", "watch") => <Option<String> as Check>::describe(tail),
            ("manifest.get", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("manifest.record", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("manifest.record", "cmd") => <String as Check>::describe(tail),
            ("manifest.record", "cwd") => <String as Check>::describe(tail),
            ("manifest.record", "port") => <Option<JsNumber> as Check>::describe(tail),
            ("manifest.restartScript", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("inbox.watch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("inbox.rescan", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("sys.watch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("sys.unwatch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("sys.history", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("sys.history", "from") => <i64 as Check>::describe(tail),
            ("sys.history", "to") => <i64 as Check>::describe(tail),
            ("sys.history", "stepMs") => <i64 as Check>::describe(tail),
            ("usage.logs", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("usage.logs", "stores") => <Vec<UsageStore> as Check>::describe(tail),
            ("transcripts.list", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("transcripts.list", "root") => <String as Check>::describe(tail),
            ("transcripts.list", "dirs") => <Vec<String> as Check>::describe(tail),
            ("transcripts.list", "cwds") => <Vec<String> as Check>::describe(tail),
            ("transcripts.read", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("transcripts.read", "root") => <String as Check>::describe(tail),
            ("transcripts.read", "dirs") => <Vec<String> as Check>::describe(tail),
            ("transcripts.read", "session") => <String as Check>::describe(tail),
            ("transcripts.read", "last") => <i64 as Check>::describe(tail),
            ("transcripts.read", "tail") => <Option<i64> as Check>::describe(tail),
            ("proc.watch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("proc.watch", "cgroups") => <Option<Vec<String>> as Check>::describe(tail),
            ("proc.unwatch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("proc.scopes", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("proc.scopes", "scopes") => <Vec<ProcScope> as Check>::describe(tail),
            ("proc.inspect", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("proc.inspect", "pid") => <i64 as Check>::describe(tail),
            ("proc.list", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("proc.kill", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("proc.kill", "pid") => <i64 as Check>::describe(tail),
            ("proc.kill", "signal") => None,
            ("proc.kill", "mode") => None,
            ("proc.kill", "startedAt") => <Option<i64> as Check>::describe(tail),
            ("proc.kill", "identity") => <Option<String> as Check>::describe(tail),
            ("proc.kill", "protect") => <Option<Vec<i64>> as Check>::describe(tail),
            ("proc.kill", "apps") => <Option<Vec<i64>> as Check>::describe(tail),
            ("proc.kill", "agents") => <Option<Vec<i64>> as Check>::describe(tail),
            ("proc.kill", "agentBins") => <Option<Vec<String>> as Check>::describe(tail),
            ("container.stop", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("container.stop", "container") => <String as Check>::describe(tail),
            ("ping", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.list", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.list", "path") => <String as Check>::describe(tail),
            ("fs.list", "gitignore") => <Option<bool> as Check>::describe(tail),
            ("fs.list", "machineId") => <Option<String> as Check>::describe(tail),
            ("fs.files", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.files", "cwd") => <String as Check>::describe(tail),
            ("fs.files", "machineId") => <Option<String> as Check>::describe(tail),
            ("fs.read", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.read", "path") => <String as Check>::describe(tail),
            ("fs.read", "encoding") => <Option<FsReadEncoding> as Check>::describe(tail),
            ("fs.read", "machineId") => <Option<String> as Check>::describe(tail),
            ("fs.image", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.image", "path") => <String as Check>::describe(tail),
            ("fs.image", "machineId") => <Option<String> as Check>::describe(tail),
            ("fs.hash", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.hash", "root") => <String as Check>::describe(tail),
            ("fs.hash", "paths") => <Vec<String> as Check>::describe(tail),
            ("fs.hash", "machineId") => <Option<String> as Check>::describe(tail),
            ("fs.write", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.write", "path") => <String as Check>::describe(tail),
            ("fs.write", "contents") => <String as Check>::describe(tail),
            ("fs.write", "machineId") => <Option<String> as Check>::describe(tail),
            ("pty.tab", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("pty.tab", "ptyId") => <String as Check>::describe(tail),
            ("pty.tab", "machineId") => <Option<String> as Check>::describe(tail),
            ("fs.search", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.search", "path") => <String as Check>::describe(tail),
            ("fs.search", "query") => <String as Check>::describe(tail),
            ("fs.search", "mode") => <FsSearchMode as Check>::describe(tail),
            ("fs.search", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.status", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.status", "cwd") => <String as Check>::describe(tail),
            ("git.status", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.diff", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.diff", "cwd") => <String as Check>::describe(tail),
            ("git.diff", "scope") => <GitDiffScope as Check>::describe(tail),
            ("git.diff", "path") => <Option<String> as Check>::describe(tail),
            ("git.diff", "paths") => <Option<Vec<String>> as Check>::describe(tail),
            ("git.diff", "whole") => <Option<bool> as Check>::describe(tail),
            ("git.diff", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.discard", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.discard", "cwd") => <String as Check>::describe(tail),
            ("git.discard", "path") => <String as Check>::describe(tail),
            ("git.discard", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.commit", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.commit", "cwd") => <String as Check>::describe(tail),
            ("git.commit", "message") => <String as Check>::describe(tail),
            ("git.commit", "paths") => <Vec<String> as Check>::describe(tail),
            ("git.commit", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.snapshot", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.snapshot", "cwd") => <String as Check>::describe(tail),
            ("git.snapshot", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.range", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.range", "cwd") => <String as Check>::describe(tail),
            ("git.range", "from") => <String as Check>::describe(tail),
            ("git.range", "to") => <String as Check>::describe(tail),
            ("git.range", "path") => <Option<String> as Check>::describe(tail),
            ("git.range", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.turn", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.turn", "cwd") => <String as Check>::describe(tail),
            ("git.turn", "from") => <String as Check>::describe(tail),
            ("git.turn", "to") => <String as Check>::describe(tail),
            ("git.turn", "path") => <Option<String> as Check>::describe(tail),
            ("git.turn", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.push", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.push", "cwd") => <String as Check>::describe(tail),
            ("git.push", "base") => <Option<String> as Check>::describe(tail),
            ("git.push", "token") => <Option<String> as Check>::describe(tail),
            ("git.push", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.pr", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.pr", "cwd") => <String as Check>::describe(tail),
            ("git.pr", "base") => <Option<String> as Check>::describe(tail),
            ("git.pr", "title") => <Option<String> as Check>::describe(tail),
            ("git.pr", "body") => <Option<String> as Check>::describe(tail),
            ("git.pr", "token") => <Option<String> as Check>::describe(tail),
            ("git.pr", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prRead", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prRead", "cwd") => <String as Check>::describe(tail),
            ("git.prRead", "remote") => <String as Check>::describe(tail),
            ("git.prRead", "branch") => <Option<String> as Check>::describe(tail),
            ("git.prRead", "number") => <Option<i64> as Check>::describe(tail),
            ("git.prRead", "seen") => <Option<String> as Check>::describe(tail),
            ("git.prRead", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prView", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prView", "cwd") => <String as Check>::describe(tail),
            ("git.prView", "remote") => <String as Check>::describe(tail),
            ("git.prView", "number") => <i64 as Check>::describe(tail),
            ("git.prView", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.runLog", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.runLog", "cwd") => <String as Check>::describe(tail),
            ("git.runLog", "remote") => <String as Check>::describe(tail),
            ("git.runLog", "runId") => <i64 as Check>::describe(tail),
            ("git.runLog", "jobId") => <i64 as Check>::describe(tail),
            ("git.runLog", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prMerge", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prMerge", "cwd") => <String as Check>::describe(tail),
            ("git.prMerge", "remote") => <String as Check>::describe(tail),
            ("git.prMerge", "number") => <i64 as Check>::describe(tail),
            ("git.prMerge", "method") => <MergeMethod as Check>::describe(tail),
            ("git.prMerge", "auto") => <bool as Check>::describe(tail),
            ("git.prMerge", "headOid") => <String as Check>::describe(tail),
            ("git.prMerge", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.issueRead", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.issueRead", "cwd") => <String as Check>::describe(tail),
            ("git.issueRead", "remote") => <String as Check>::describe(tail),
            ("git.issueRead", "number") => <i64 as Check>::describe(tail),
            ("git.issueRead", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prCheckout", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prCheckout", "cwd") => <String as Check>::describe(tail),
            ("git.prCheckout", "number") => <i64 as Check>::describe(tail),
            ("git.prCheckout", "token") => <Option<String> as Check>::describe(tail),
            ("git.prCheckout", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prDiff", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prDiff", "cwd") => <String as Check>::describe(tail),
            ("git.prDiff", "remote") => <String as Check>::describe(tail),
            ("git.prDiff", "number") => <i64 as Check>::describe(tail),
            ("git.prDiff", "maxBytes") => <Option<i64> as Check>::describe(tail),
            ("git.prDiff", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prReview", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prReview", "cwd") => <String as Check>::describe(tail),
            ("git.prReview", "remote") => <String as Check>::describe(tail),
            ("git.prReview", "number") => <i64 as Check>::describe(tail),
            ("git.prReview", "headOid") => <String as Check>::describe(tail),
            ("git.prReview", "event") => <GitPrReviewEvent as Check>::describe(tail),
            ("git.prReview", "body") => <String as Check>::describe(tail),
            ("git.prReview", "comments") => <Vec<GitPrReviewComment> as Check>::describe(tail),
            ("git.prReview", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prReply", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prReply", "cwd") => <String as Check>::describe(tail),
            ("git.prReply", "remote") => <String as Check>::describe(tail),
            ("git.prReply", "number") => <i64 as Check>::describe(tail),
            ("git.prReply", "replyTo") => <Option<i64> as Check>::describe(tail),
            ("git.prReply", "threadId") => <Option<String> as Check>::describe(tail),
            ("git.prReply", "body") => <String as Check>::describe(tail),
            ("git.prReply", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prResolve", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prResolve", "cwd") => <String as Check>::describe(tail),
            ("git.prResolve", "remote") => <String as Check>::describe(tail),
            ("git.prResolve", "number") => <i64 as Check>::describe(tail),
            ("git.prResolve", "threadId") => <String as Check>::describe(tail),
            ("git.prResolve", "resolved") => <bool as Check>::describe(tail),
            ("git.prResolve", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prReact", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prReact", "cwd") => <String as Check>::describe(tail),
            ("git.prReact", "remote") => <String as Check>::describe(tail),
            ("git.prReact", "number") => <i64 as Check>::describe(tail),
            ("git.prReact", "subject") => <String as Check>::describe(tail),
            ("git.prReact", "content") => <ReactionContent as Check>::describe(tail),
            ("git.prReact", "on") => <bool as Check>::describe(tail),
            ("git.prReact", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.repoRead", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.repoRead", "cwd") => <String as Check>::describe(tail),
            ("git.repoRead", "remote") => <String as Check>::describe(tail),
            ("git.repoRead", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.update", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.update", "cwd") => <String as Check>::describe(tail),
            ("git.update", "base") => <Option<String> as Check>::describe(tail),
            ("git.update", "token") => <Option<String> as Check>::describe(tail),
            ("git.update", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.startOn", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.startOn", "cwd") => <String as Check>::describe(tail),
            ("git.startOn", "branch") => <String as Check>::describe(tail),
            ("git.startOn", "token") => <Option<String> as Check>::describe(tail),
            ("git.startOn", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.branchCompare", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.branchCompare", "cwd") => <String as Check>::describe(tail),
            ("git.branchCompare", "remote") => <String as Check>::describe(tail),
            ("git.branchCompare", "base") => <String as Check>::describe(tail),
            ("git.branchCompare", "head") => <String as Check>::describe(tail),
            ("git.mergeIn", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.mergeIn", "cwd") => <String as Check>::describe(tail),
            ("git.mergeIn", "branch") => <String as Check>::describe(tail),
            ("git.mergeIn", "from") => <Option<String> as Check>::describe(tail),
            ("git.mergeIn", "token") => <Option<String> as Check>::describe(tail),
            ("git.mergeIn", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.prList", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.prList", "cwd") => <String as Check>::describe(tail),
            ("git.prList", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.checkpoint", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.checkpoint", "cwd") => <String as Check>::describe(tail),
            ("git.checkpoint", "thread") => <String as Check>::describe(tail),
            ("git.checkpoint", "turn") => <String as Check>::describe(tail),
            ("git.checkpoint", "scope") => <Option<String> as Check>::describe(tail),
            ("git.checkpoint", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.restore", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.restore", "cwd") => <String as Check>::describe(tail),
            ("git.restore", "checkpoint") => <String as Check>::describe(tail),
            ("git.restore", "scope") => <Option<String> as Check>::describe(tail),
            ("git.restore", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.checkpointDrop", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.checkpointDrop", "cwd") => <String as Check>::describe(tail),
            ("git.checkpointDrop", "scope") => <Option<String> as Check>::describe(tail),
            ("git.checkpointDrop", "thread") => <String as Check>::describe(tail),
            ("git.checkpointDrop", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.worktrees", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.worktrees", "cwd") => <String as Check>::describe(tail),
            ("git.worktrees", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.branches", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.branches", "cwd") => <String as Check>::describe(tail),
            ("git.branches", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.switchNew", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.switchNew", "cwd") => <String as Check>::describe(tail),
            ("git.switchNew", "branch") => <String as Check>::describe(tail),
            ("git.switchNew", "machineId") => <Option<String> as Check>::describe(tail),
            ("git.fetchBranch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("git.fetchBranch", "cwd") => <String as Check>::describe(tail),
            ("git.fetchBranch", "remote") => <String as Check>::describe(tail),
            ("git.fetchBranch", "branch") => <String as Check>::describe(tail),
            ("git.fetchBranch", "into") => <Option<String> as Check>::describe(tail),
            ("git.fetchBranch", "machineId") => <Option<String> as Check>::describe(tail),
            ("fs.folders", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("fs.folders", "dir") => <Option<String> as Check>::describe(tail),
            ("fs.folders", "hidden") => <Option<bool> as Check>::describe(tail),
            ("fs.folders", "repos") => <Option<bool> as Check>::describe(tail),
            ("fs.folders", "projects") => <Option<Vec<String>> as Check>::describe(tail),
            ("tunnel.open", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("tunnel.open", "tunnelId") => <String as Check>::describe(tail),
            ("tunnel.open", "port") => <i64 as Check>::describe(tail),
            ("tunnel.open", "machineId") => <Option<String> as Check>::describe(tail),
            ("ssh.start", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("ssh.start", "authorizedKey") => <String as Check>::describe(tail),
            ("ssh.start", "machineId") => <Option<String> as Check>::describe(tail),
            ("tunnel.write", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("tunnel.write", "tunnelId") => <String as Check>::describe(tail),
            ("tunnel.write", "data") => <String as Check>::describe(tail),
            ("tunnel.close", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("tunnel.close", "tunnelId") => <String as Check>::describe(tail),
            ("stdio.open", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("stdio.open", "stdioId") => <String as Check>::describe(tail),
            ("stdio.open", "cmd") => <String as Check>::describe(tail),
            ("stdio.open", "env") => <Option<IndexMap<String, String>> as Check>::describe(tail),
            ("stdio.write", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("stdio.write", "stdioId") => <String as Check>::describe(tail),
            ("stdio.write", "data") => <String as Check>::describe(tail),
            ("stdio.close", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("stdio.close", "stdioId") => <String as Check>::describe(tail),
            ("http.post", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("http.post", "url") => <String as Check>::describe(tail),
            ("http.post", "headers") => <IndexMap<String, String> as Check>::describe(tail),
            ("http.post", "body") => <String as Check>::describe(tail),
            ("http.post", "bearer") => <Option<HttpBearer> as Check>::describe(tail),
            ("http.post", "timeoutMs") => <Option<i64> as Check>::describe(tail),
            ("place.leave", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("guest.open", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("guest.open", "kind") => <GuestKind as Check>::describe(tail),
            ("guest.open", "token") => <String as Check>::describe(tail),
            ("guest.open", "turnToken") => <Option<String> as Check>::describe(tail),
            ("guest.open", "argv") => <Vec<String> as Check>::describe(tail),
            ("guest.open", "cwd") => <String as Check>::describe(tail),
            ("guest.send", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("guest.send", "message") => <Value as Check>::describe(tail),
            ("guest.watch", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("guest.reply", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("guest.reply", "session") => <String as Check>::describe(tail),
            ("guest.reply", "message") => <Value as Check>::describe(tail),
            ("guest.close", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("guest.close", "session") => <String as Check>::describe(tail),
            ("guest.close", "error") => <Option<String> as Check>::describe(tail),
            ("place.update", "id") => crate::types::check::leaf(tail, crate::types::check::Zod::Union),
            ("place.update", "uploadId") => <String as Check>::describe(tail),
            ("place.update", "seq") => <i64 as Check>::describe(tail),
            ("place.update", "last") => <bool as Check>::describe(tail),
            ("place.update", "data") => <String as Check>::describe(tail),
            ("place.update", "sha256") => <String as Check>::describe(tail),
            _ => None,
        }
    }
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
        literal_word(&self.op, "auth", "op")?;
        if let Some(port) = self.port {
            num_at_least(port as f64, 1.0).map_err(|e| e.under("port"))?;
            num_at_most(port as f64, 65535.0).map_err(|e| e.under("port"))?;
        }
        Ok(())
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

/// No refinement of its own; checks every nested field that has one.
impl Check for FsReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
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
            num_above(stashes as f64, 0.0).map_err(|e| e.under("stashes"))?;
        }
        Ok(())
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

/// No refinement of its own; checks every nested field that has one.
impl Check for GitDiffReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.base.check().map_err(|e| e.under("base"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        self.moved.check().map_err(|e| e.under("moved"))?;
        Ok(())
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

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "by" => None,
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
        for (key, list) in [("agents", &self.agents), ("groups", &self.groups)] {
            if let Some(list) = list {
                items_at_most(list.len(), 64).map_err(|e| e.under(key))?;
                for (i, id) in list.iter().enumerate() {
                    num_at_least(*id as f64, 0.0).map_err(|e| e.under(i).under(key))?;
                }
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

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `pid`: int min 0
impl Check for ProcScopePid {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.pid as f64, 0.0).map_err(|e| e.under("pid"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "kind" => None,
                _ => None,
            },
        }
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
            num_above(timeout as f64, 0.0).map_err(|e| e.under("timeoutMs"))?;
            num_at_most(timeout as f64, EXEC_TIMEOUT_MAX_MS as f64).map_err(|e| e.under("timeoutMs"))?;
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "cmd" => <String as Check>::describe(rest),
                "timeout_ms" => <Option<i64> as Check>::describe(rest),
                "stdin" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonExecReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
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
                num_at_least(*port as f64, 1.0).map_err(|e| e.under("port"))?;
                num_at_most(*port as f64, 65535.0).map_err(|e| e.under("port"))?;
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
                    num_above(*v as f64, 0.0).map_err(|e| e.under("timeoutMs"))?;
                    num_at_most(*v as f64, EXEC_TIMEOUT_MAX_MS as f64).map_err(|e| e.under("timeoutMs"))?;
                }
            }
            DaemonRequest::PortsWatch { cgroups, watch, .. } => {
                check_cgroups(cgroups.as_deref())?;
                check_watch(watch.as_deref())?;
            }
            DaemonRequest::ProcWatch { cgroups, .. } => check_cgroups(cgroups.as_deref())?,
            DaemonRequest::ProcScopes { scopes, .. } => {
                items_at_most(scopes.len(), PROC_SCOPES_MAX as usize).map_err(|e| e.under("scopes"))?;
                scopes.check().map_err(|e| e.under("scopes"))?;
            }
            DaemonRequest::ProcKill { pid, identity, protect, apps, agents, agent_bins, .. } => {
                num_above(*pid as f64, 0.0).map_err(|e| e.under("pid"))?;
                if let Some(identity) = identity {
                    text_at_most(identity, 32).map_err(|e| e.under("identity"))?;
                }
                for (key, list) in [("protect", protect), ("apps", apps), ("agents", agents)] {
                    if let Some(list) = list {
                        items_at_most(list.len(), KILL_PIDS_MAX as usize).map_err(|e| e.under(key))?;
                        for (i, pid) in list.iter().enumerate() {
                            num_at_least(*pid as f64, 0.0).map_err(|e| e.under(i).under(key))?;
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
                num_above(*last as f64, 0.0).map_err(|e| e.under("last"))?;
                if let Some(tail) = tail {
                    num_above(*tail as f64, 0.0).map_err(|e| e.under("tail"))?;
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
            DaemonRequest::GitPrReply { body, .. } => {
                text_at_most(body, 65536).map_err(|e| e.under("body"))?;
            }
            DaemonRequest::SshStart { authorized_key, .. } => {
                text_at_least(authorized_key, 1).map_err(|e| e.under("authorizedKey"))?;
                text_at_most(authorized_key, 1024).map_err(|e| e.under("authorizedKey"))?;
            }
            DaemonRequest::GitPush { token, .. }
            | DaemonRequest::GitPr { token, .. }
            | DaemonRequest::GitPrCheckout { token, .. }
            | DaemonRequest::GitUpdate { token, .. }
            | DaemonRequest::GitStartOn { token, .. }
            | DaemonRequest::GitMergeIn { token, .. } => {
                if let Some(token) = token {
                    text_at_most(token, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("token"))?;
                }
            }
            DaemonRequest::GitPrReview { comments, .. } => {
                comments.check().map_err(|e| e.under("comments"))?;
            }
            DaemonRequest::PlaceUpdate { upload_id, seq, sha256, .. } => {
                text_at_least(upload_id, 1).map_err(|e| e.under("uploadId"))?;
                text_at_most(upload_id, 32).map_err(|e| e.under("uploadId"))?;
                regex_check(!upload_id.is_empty() && upload_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
                    .map_err(|e| e.under("uploadId"))?;
                num_at_least(*seq as f64, 0.0).map_err(|e| e.under("seq"))?;
                regex_check(hex_64(sha256)).map_err(|e| e.under("sha256"))?;
            }
            _ => {}
        }
        Ok(())
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
        num_at_least(self.line as f64, 0.0).map_err(|e| e.under("line"))?;
        Ok(())
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

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestOpenReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
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
}

/// No refinement of its own; checks every nested field that has one.
impl Check for GuestCliMessage0 {
    fn check(&self) -> Result<(), SchemaError> {
        self.stream.check().map_err(|e| e.under("stream"))?;
        Ok(())
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
    let value: Value = serde_json::from_str(text).ok()?;
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
pub fn by_proc_column(sort: ProcSort, a: &ProcEntry, b: &ProcEntry) -> std::cmp::Ordering {
    let delta = match sort {
        ProcSort::Cpu => b.cpu.0 - a.cpu.0,
        ProcSort::Mem => b.rss.0 - a.rss.0,
    };
    delta.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal).then(a.pid.cmp(&b.pid))
}

/// ProcEntry.kind as zod reads it: absent is None, a known word is that kind, null or anything else is Some(Other).
pub fn proc_kind_or_other<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<ProcKind>, D::Error> {
    let word = Value::deserialize(d)?;
    Ok(Some(serde_json::from_value(word).unwrap_or(ProcKind::Other)))
}

/// Reads one daemon request as zod's discriminated union on `op` reads it, so a refusal's first issue is zod's: a value
/// that is not an object is one invalid_type issue at the root (expected "object"); an absent or unknown `op` is
/// one invalid_union_discriminator issue on ["op"] whose detail `options` lists every op of DaemonRequest in order and
/// whose message is "Invalid discriminator value. Expected 'a' | 'b' | ..."; otherwise the variant's first failing
/// field, then its Check. Unknown keys are dropped.
pub fn parse_daemon_request(frame: &Value) -> Result<DaemonRequest, SchemaError> {
    let Some(object) = frame.as_object() else {
        let received = match frame {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        let mut detail = IndexMap::new();
        detail.insert("expected".to_string(), Value::from("object"));
        detail.insert("received".to_string(), Value::from(received));
        return Err(SchemaError {
            code: "invalid_type".into(),
            path: Vec::new(),
            message: format!("Expected object, received {received}"),
            detail,
        });
    };
    let op = match object.get("op").and_then(Value::as_str) {
        Some(op) if DAEMON_REQUEST_OPS.contains(&op) => op,
        _ => return Err(op_discriminator("op", DAEMON_REQUEST_OPS)),
    };
    let mut payload = object.clone();
    payload.remove("op");
    let mut wrapped = serde_json::Map::new();
    wrapped.insert(op.to_string(), Value::Object(payload));
    let frame = crate::types::check::parse::<DaemonRequestFrame>(&Value::Object(wrapped)).map_err(|mut e| {
        if e.path.first() == Some(&Value::from(op)) {
            e.path.remove(0);
        }
        e
    })?;
    into_request(frame)
}

/// The request a frame read as, in the op-tagged form DaemonRequest is written in. The frame's fields were checked on
/// the way in, so the round trip cannot refuse them; only the caps are left, and those answer at their own fields.
fn into_request(frame: DaemonRequestFrame) -> Result<DaemonRequest, SchemaError> {
    let Value::Object(written) = serde_json::to_value(&frame).expect("a request writes as JSON") else {
        unreachable!("an externally tagged request writes as an object")
    };
    let Some((op, Value::Object(fields))) = written.into_iter().next() else {
        unreachable!("an externally tagged request writes its fields under its op")
    };
    let mut tagged = serde_json::Map::new();
    tagged.insert("op".to_string(), Value::from(op));
    tagged.extend(fields);
    let request: DaemonRequest = serde_json::from_value(Value::Object(tagged)).expect("a request the wire read writes back as one");
    request.check()?;
    Ok(request)
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

// The shapes wsp-frames holds read every bound while deserializing, so each Check has nothing left to refuse.
impl Check for CgroupMemory {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for FsEntry {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for FsFilesReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for FsHashReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for FsImageReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for FsListReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for FsSearchReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for FsWriteReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitBranchesReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitCheckpointDropReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitCheckpointReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitCommitReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitDiffFile {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitDiscardReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitLocalBranch {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitPrListReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitRestoreReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitWorktree {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for GitWorktreesReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for HostItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for HttpBearer {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for HttpPostReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for PipeOpen {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for ProcListReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for SshStartReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}
impl Check for UsageStore {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
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
