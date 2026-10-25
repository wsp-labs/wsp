// SPDX-License-Identifier: AGPL-3.0-only
//! `workspaces-help.ts:311-598` at 65f6b89c7: renaming a workspace, waking, committing, fixing, merging, starting and
//! reviewing, what a dead launch owes the machine it woke, stopping, forgetting, settling, restoring and renaming a
//! thread. Spec: `spec/ws-life.md`.

use serde::{Deserialize, Serialize};
use wsp_wire::types::pull_request::{FixResult, GitUpdateReply, MergeMethod, MergeResult};
use wsp_wire::types::tree::MergeInResult;
use wsp_wire::views::activity::ProcessesAsk;
use wsp_wire::views::harness::{ReviewPostResult, StartResult};
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView, WorktreeFolder};
use wsp_wire::wire::daemon::GitCommitReply;
use wsp_wire::wire::runtime::{SessionInterruptOutcome, SessionRenameOutcome, SessionRestoreResult, SessionSettleResult};
use wsp_words::exit::HostError;
use wsp_words::start::ReviewVerdict;

use crate::verbs::client::HostClient;
use crate::verbs::turns_help::Turn;

/// `RenamedWorkspace` (workspaces-help.ts:312): the name the workspace went in under and the record after.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenamedWorkspace {
    pub was: String,
    pub workspace: WorkspaceOut,
}

/// `renameWorkspace(client, ref, name)` (workspaces-help.ts:319): `workspace_of`, then `workspaces.rename
/// {workspaceId, name}` (the name as typed, blank or taken included: those are the runtime's refusals); answers the
/// old name and the record as the host wrote it.
pub async fn rename_workspace(client: &dyn HostClient, r#ref: &str, name: &str) -> Result<RenamedWorkspace, HostError> {
    let _ = (client, r#ref, name);
    todo!("workspaces-help.ts:319")
}

/// `renamedWorkspaceLine(r)` (workspaces-help.ts:325): `<was> is now <name> <id>`.
pub fn renamed_workspace_line(r: &RenamedWorkspace) -> String {
    let _ = r;
    todo!("workspaces-help.ts:325")
}

/// `Woken` (workspaces-help.ts:331): the workspace a verb woke, whether this call is what woke it, and the disk line
/// the wake left where the provider had deleted the machine's disk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Woken {
    pub workspace: WorkspaceOut,
    pub woke: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

/// `awake(client, workspace, action, tell)` (workspaces-help.ts:347-357). The state is read off the phase alone
/// (`workspace_state({phase})`). `gone` fails a plain error `gone_refusal(name, action, workspace.gone)` before any
/// frame. `released` with the action `send` answers the workspace as given with `woke` false and sends nothing (a
/// send makes a new machine). Any state but `running` says `waking <name>` through `tell`; then `workspaces.wake
/// {workspaceId}` is asked whatever the state (the runtime alone knows a provider-side pause). Where the state was
/// `paused` or `pausing`, the woken record's `diskGone` is told too (even empty) and kept as `said`, and `woke` is
/// true only if the record's phase now reads `running`; from any other state `woke` is false and no disk line is told.
/// The workspace answered is the record the wake returned.
pub async fn awake(client: &dyn HostClient, workspace: &WorkspaceView, action: &str, tell: &(dyn Fn(&str) + Send + Sync)) -> Result<Woken, HostError> {
    let _ = (client, workspace, action, tell);
    todo!("workspaces-help.ts:347")
}

/// `noDraftLine(note)` (workspaces-help.ts:360): `no message was drafted: <note>`, `the agent gave none` where absent
/// (an empty note stays empty).
pub fn no_draft_line(note: Option<&str>) -> String {
    let _ = note;
    todo!("workspaces-help.ts:360")
}

/// `NO_DRAFT_FIX` (workspaces-help.ts:361).
pub const NO_DRAFT_FIX: &str = "Pass one with --message \"<message>\".";

/// `committed(client, workspaceId, message, files, say, threadId?)` (workspaces-help.ts:365-375). A message (even
/// empty) goes straight to `workspaces.commit {workspaceId, message, paths?, threadId?}`. Without one,
/// `workspaces.commitDraft {workspaceId, paths?, threadId?}` is asked first (`paths` only where files are named); a
/// `null` draft message fails `usage_refusal(no_draft_line(note), NO_DRAFT_FIX)` and commits nothing; any other draft
/// (even empty) is said through `say` and committed. The reply is parsed as a `GitCommitReply`.
pub async fn committed(
    client: &dyn HostClient,
    workspace_id: &str,
    message: Option<&str>,
    files: &[String],
    say: &(dyn Fn(&str) + Send + Sync),
    thread_id: Option<&str>,
) -> Result<GitCommitReply, HostError> {
    let _ = (client, workspace_id, message, files, say, thread_id);
    todo!("workspaces-help.ts:365")
}

/// A child thread a fix or a merge names: its workspace and its thread.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildRef {
    pub workspace_id: String,
    pub thread_id: String,
}

/// `askedToFix(client, workspaceId, check, child, threadId)` (workspaces-help.ts:378-384): `workspaces.fix
/// {workspaceId, check?, child?: child.workspaceId, childThreadId?: child.threadId, threadId}`, in that order, an
/// absent `check` or `child` left out (an empty check is sent). A reply that does not read as a `FixResult` fails a
/// plain error `other_version("workspaces.fix")`.
pub async fn asked_to_fix(client: &dyn HostClient, workspace_id: &str, check: Option<&str>, child: Option<&ChildRef>, thread_id: &str) -> Result<FixResult, HostError> {
    let _ = (client, workspace_id, check, child, thread_id);
    todo!("workspaces-help.ts:378")
}

/// The lead of a merge in: its workspace and its thread.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeLead {
    pub workspace: WorkspaceOut,
    pub thread_id: String,
}

/// `mergedIn(client, lead, child, said)` (workspaces-help.ts:387-390): `awake(lead.workspace, "merge in", said)`, then
/// `workspaces.mergeIn {workspaceId: <the woken record's id>, child: child.workspaceId, threadId: lead.threadId,
/// childThreadId: child.threadId}`; the reply parsed as a `MergeInResult`. A gone lead fails before any frame.
pub async fn merged_in(client: &dyn HostClient, lead: &MergeLead, child: &ChildRef, said: &(dyn Fn(&str) + Send + Sync)) -> Result<MergeInResult, HostError> {
    let _ = (client, lead, child, said);
    todo!("workspaces-help.ts:387")
}

/// `fixLine(thread, asked, child?)` (workspaces-help.ts:394-399): `updated` is `fix_nothing_line(thread, base)`. Any
/// other outcome names the agent by `agent_name` and reads `fix_merge_child_line(thread, agent, child ?? asked.child)`
/// where the answer names a child, else `fix_asked_line(thread, agent, check)` where it names a check, else
/// `fix_conflicts_line(thread, agent, base)`; a `held` answer with a `capped` wait adds `\n` and `cap_wait_line`.
pub fn fix_line(thread: &str, asked: &FixResult, child: Option<&str>) -> String {
    let _ = (thread, asked, child);
    todo!("workspaces-help.ts:394")
}

/// What `startedFrom` asks, in the order the command line and the tool build it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StartedFromAsk {
    pub url: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub access: Option<String>,
}

/// `startedFrom(client, o)` (workspaces-help.ts:402-406): `access`, where given, is read through `access_word_of`
/// first (a word outside ask, auto-edit, full, plan fails before any frame); `workspaces.start` carries the fields
/// that are given, in the order url, project, agent, model, effort, access (an empty string is given). Answers the
/// workspace as the host wrote it, the thread id and the session id.
pub async fn started_from(client: &dyn HostClient, o: &StartedFromAsk) -> Result<StartResult, HostError> {
    let _ = (client, o);
    todo!("workspaces-help.ts:402")
}

/// What `reviewStarted` asks: a link or a workspace's own pull request, and the picks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartedAsk {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

/// `reviewStarted(client, o)` (workspaces-help.ts:409-413): `workspaces.review` with the fields given, in the order
/// url, workspaceId, agent, model, effort; the answer as `started_from`'s.
pub async fn review_started(client: &dyn HostClient, o: &ReviewStartedAsk) -> Result<StartResult, HostError> {
    let _ = (client, o);
    todo!("workspaces-help.ts:409")
}

/// The edits `reviewPosted` makes to a review's draft first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReviewEdits {
    #[serde(default)]
    pub verdict: Option<ReviewVerdict>,
    #[serde(default)]
    pub summary: Option<String>,
}

/// `reviewPosted(client, workspaceId, edits, threadId)` (workspaces-help.ts:416-419): where a verdict or a summary
/// is given (an empty summary is one) `workspaces.reviewDraft {workspaceId, verdict?, summary?}` goes first and a
/// refusal of it ends the call; then `workspaces.reviewPost {workspaceId, threadId}`, parsed as a `ReviewPostResult`.
pub async fn review_posted(client: &dyn HostClient, workspace_id: &str, edits: &ReviewEdits, thread_id: &str) -> Result<ReviewPostResult, HostError> {
    let _ = (client, workspace_id, edits, thread_id);
    todo!("workspaces-help.ts:416")
}

/// `startedLines(started, thread?)` (workspaces-help.ts:423-425): `START_WORDS.made(workspace name, from)`, `\n`, then
/// the caller's line for the thread id, `opened_thread_said(id, None)` by default.
pub fn started_lines(started: &StartResult, thread: Option<&(dyn Fn(&str) -> String + Send + Sync)>) -> String {
    let _ = (started, thread);
    todo!("workspaces-help.ts:423")
}

/// `reviewLines(started, thread?)` (workspaces-help.ts:428-430): the thread line first, then
/// `START_WORDS.made("thread <thread_word(id)>", from)`.
pub fn review_lines(started: &StartResult, thread: Option<&(dyn Fn(&str) -> String + Send + Sync)>) -> String {
    let _ = (started, thread);
    todo!("workspaces-help.ts:428")
}

/// `threadLabel(thread)` (workspaces-help.ts:433): `thread <thread_word(thread_id_of(thread))>`.
pub fn thread_label(thread: &ThreadView) -> String {
    let _ = thread;
    todo!("workspaces-help.ts:433")
}

/// `threadCwd(t, workspace)` (workspaces-help.ts:437): the thread's folder `cwd`, absent where the workspace's
/// worktree is gone and the folder is the worktree's path or inside it (`under`), since the next turn runs where the
/// record answers now. `worktree` is `workspace?.worktree`.
pub fn thread_cwd(cwd: Option<&str>, worktree: Option<&WorktreeFolder>) -> Option<String> {
    let _ = (cwd, worktree);
    todo!("workspaces-help.ts:437")
}

/// `folderOf(workspace)` (workspaces-help.ts:443): `home_shortened(folder ?? project.path, home)`. `home` is what
/// `os.homedir()` answers; the caller passes it so a test needs no process environment.
pub fn folder_of(workspace: &WorkspaceOut, home: Option<&str>) -> String {
    let _ = (workspace, home);
    todo!("workspaces-help.ts:443")
}

/// `withShared(line, shared)` (workspaces-help.ts:446): the line, and where threads share the folder, `\n` and
/// `shared_folder_line` of their `thread_word`s.
pub fn with_shared(line: &str, shared: &[String]) -> String {
    let _ = (line, shared);
    todo!("workspaces-help.ts:446")
}

/// `sharedOf(shared)` (workspaces-help.ts:449): the ids whole and in order, for the `sharedWith` field of an answer;
/// `None` where none share it (the TS answers `{}` or `{sharedWith: [...]}`).
pub fn shared_of(shared: &[String]) -> Option<Vec<String>> {
    let _ = shared;
    todo!("workspaces-help.ts:449")
}

/// `VERDICTS` (workspaces-help.ts:452): the verdict a word names, in the command line's dashed spelling or the
/// host's own, in this order.
pub const VERDICTS: [(&str, ReviewVerdict); 4] = [
    ("comment", ReviewVerdict::Comment),
    ("approve", ReviewVerdict::Approve),
    ("request-changes", ReviewVerdict::RequestChanges),
    ("request_changes", ReviewVerdict::RequestChanges),
];

/// `mergedPr(client, workspaceId, method, whenChecksPass, threadId)` (workspaces-help.ts:455): `workspaces.merge
/// {workspaceId, method?, whenChecksPass? (only where true), threadId}`, parsed as a `MergeResult`.
pub async fn merged_pr(client: &dyn HostClient, workspace_id: &str, method: Option<MergeMethod>, when_checks_pass: bool, thread_id: &str) -> Result<MergeResult, HostError> {
    let _ = (client, workspace_id, method, when_checks_pass, thread_id);
    todo!("workspaces-help.ts:455")
}

/// `updateLine(workspace, done)` (workspaces-help.ts:460): `updated_line(workspace, base, commits)` where merged,
/// else `update_conflicts_line(workspace, base, conflicts)` (the files listed are not read where merged).
pub fn update_line(workspace: &str, done: &GitUpdateReply) -> String {
    let _ = (workspace, done);
    todo!("workspaces-help.ts:460")
}

/// `napAfterDeadLaunch(client, woken, turn)` (workspaces-help.ts:469-490): nothing where this call did not wake the
/// machine, or where the turn's result names a refusal (the agent ran). Else `sessions.list {workspaceId}`; nothing
/// where the turn's thread (matched by `thread_id_of`) did work (`ran`); where any other thread is `running`,
/// `status.list` and `workspace_stays_awake_line(name, idleAt - now, floored at 0, or none)`; else `workspaces.nap
/// {workspaceId}` and `workspace_asleep_again_line(name)`. Any failure after the first checks answers
/// `workspace_stays_awake_line(name, None)`: this call never fails.
pub async fn nap_after_dead_launch(client: &dyn HostClient, woken: &Woken, turn: Option<&Turn>) -> Option<String> {
    let _ = (client, woken, turn);
    todo!("workspaces-help.ts:469")
}

/// `withLine(e, line)` (workspaces-help.ts:494): the same refusal with `line` under its message (`\n`), carrying only
/// the `kind` stamp over (not the fix, the problem lists or any mark). No line leaves it as it was. (The TS also
/// passes a thrown value that is no error through; a `HostError` is always one.)
pub fn with_line(e: HostError, line: Option<&str>) -> HostError {
    let _ = (e, line);
    todo!("workspaces-help.ts:494")
}

/// `Stopped` (workspaces-help.ts:501): what a stop came to, none of it an error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stopped {
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub outcome: SessionInterruptOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
}

/// `stop(client, ref, subagent?)` (workspaces-help.ts:517): `thread_of`, then `sessions.interrupt {sessionId (the
/// thread's latest turn), task?}`; the reply parsed as a `SessionInterruptResult`. The answer is `{threadId, task?,
/// outcome, under? (only where not empty), error?, left?}`.
pub async fn stop(client: &dyn HostClient, r#ref: &str, subagent: Option<&str>) -> Result<Stopped, HostError> {
    let _ = (client, r#ref, subagent);
    todo!("workspaces-help.ts:517")
}

/// `stopLine(stopped)` (workspaces-help.ts:525-531): `thread <id>[ subagent <task>]`, then `<named>: <error>` where
/// an error stands; else the outcome's words (accepted `stopped`, not-running `not running`, not-found `not found by
/// the host`, refused and unsupported `not stopped`), then where threads are under it `, and with it <n> thread[s]
/// its agents spawned: <thread_word, ...>`, then `, but <left>` where the stop left something.
pub fn stop_line(stopped: &Stopped) -> String {
    let _ = stopped;
    todo!("workspaces-help.ts:525")
}

/// `forgetThread(client, thread, o)` (workspaces-help.ts:537): a row with no `threadId` fails a plain error
/// `thread_without_id_refusal(id)` before any frame; else `sessions.forget {threadId, check?: true}` (`check` only
/// where true).
pub async fn forget_thread(client: &dyn HostClient, thread: &ThreadView, check: bool) -> Result<(), HostError> {
    let _ = (client, thread, check);
    todo!("workspaces-help.ts:537")
}

/// `threadForgotLine(thread)` (workspaces-help.ts:543): `forgot thread <id>: no turn ever ran on it, so nothing of
/// its work is gone`.
pub fn thread_forgot_line(thread: &ThreadView) -> String {
    let _ = thread;
    todo!("workspaces-help.ts:543")
}

/// `settleThreads(client, threads, finished, processes?)` (workspaces-help.ts:549): `sessions.settle {threadIds (each
/// row's `id`, not its thread id), finished?: true, processes?}`, parsed as a `SessionSettleResult`.
pub async fn settle_threads(client: &dyn HostClient, threads: &[ThreadView], finished: bool, processes: Option<ProcessesAsk>) -> Result<SessionSettleResult, HostError> {
    let _ = (client, threads, finished, processes);
    todo!("workspaces-help.ts:549")
}

/// `restoreThreads(client, threads)` (workspaces-help.ts:554): `sessions.restore {threadIds}`, parsed as a
/// `SessionRestoreResult`.
pub async fn restore_threads(client: &dyn HostClient, threads: &[ThreadView]) -> Result<SessionRestoreResult, HostError> {
    let _ = (client, threads);
    todo!("workspaces-help.ts:554")
}

/// `settledLines(answer)` (workspaces-help.ts:559): one line per settled thread `settled <thread_word> <title>`, then
/// one per thread left `left <thread_word>: <why>`, joined by `\n`; `nothing settled` where there are none.
pub fn settled_lines(answer: &SessionSettleResult) -> String {
    let _ = answer;
    todo!("workspaces-help.ts:559")
}

/// `restoredLines(answer)` (workspaces-help.ts:565): `restored <thread_word> <title>` per thread, `\n` between;
/// `nothing restored` where none.
pub fn restored_lines(answer: &SessionRestoreResult) -> String {
    let _ = answer;
    todo!("workspaces-help.ts:565")
}

/// `Renamed` (workspaces-help.ts:571): what a thread rename came to, none of it an error. `error` rides whatever the
/// outcome when the answer carried it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Renamed {
    pub thread_id: String,
    pub title: String,
    pub harness: String,
    pub outcome: SessionRenameOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `rename(client, thread, title)` (workspaces-help.ts:581): `sessions.rename {sessionId (the thread's latest
/// turn), title}` (the title as given, empty included), parsed as a `SessionRenameResult`; answers `{threadId: the
/// row's id, title, harness, outcome, error?}`.
pub async fn rename(client: &dyn HostClient, thread: &ThreadView, title: &str) -> Result<Renamed, HostError> {
    let _ = (client, thread, title);
    todo!("workspaces-help.ts:581")
}

/// `renameLine(renamed)` (workspaces-help.ts:588-598): `thread <threadId> <words>` where the words are `named
/// <title>, in <agent> too` (renamed), `not named: <agent> keeps no name of a person's for a session` (unsupported),
/// `not named: <agent> on the machine has no such session` (no-session), `not named: <error, or the machine said
/// nothing about the write>` (failed), `not found by the host` (not-found); `<agent>` is `agent_name(harness)`.
pub fn rename_line(renamed: &Renamed) -> String {
    let _ = renamed;
    todo!("workspaces-help.ts:588")
}
