// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

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

crate::zod_union!(GuestCliMessage { V0(GuestCliMessage0), V1(GuestCliMessage1) }

);

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
        crate::types::keys::opt_arr_with::<JsInt>(obj, "agents", &mut out, |n| items_at_most(n, 64), Self::rule_agents);
        crate::types::keys::opt_arr_with::<JsInt>(obj, "groups", &mut out, |n| items_at_most(n, 64), Self::rule_groups);
        crate::types::keys::opt_key_with::<String>(obj, "folder", &mut out, Self::rule_folder);
        crate::types::keys::opt_key::<bool>(obj, "warm", &mut out);
        crate::types::keys::opt_key_with::<String>(obj, "cgroup", &mut out, Self::rule_cgroup);
        crate::types::keys::opt_arr_with::<ProcScopePid>(obj, "pids", &mut out, |n| items_at_most(n, 64), Self::rule_pids);
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
