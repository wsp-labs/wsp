// SPDX-License-Identifier: AGPL-3.0-only
//! `workspaces-help.ts:311-598` at 65f6b89c7: renaming a workspace, waking, committing, fixing, merging, starting and
//! reviewing, what a dead launch owes the machine it woke, stopping, forgetting, settling, restoring and renaming a
//! thread. Spec: `spec/ws-life.md`.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wsp_catalog::catalog::agent_name;
use wsp_wire::types::changes::CommitDraft;
use wsp_wire::types::pull_request::{FixResult, FixResultV0Outcome, GitUpdateReply, MergeMethod, MergeResult};
use wsp_wire::types::tree::MergeInResult;
use wsp_wire::views::activity::ProcessesAsk;
use wsp_wire::views::harness::{ReviewPostResult, StartResult};
use wsp_wire::views::session::{SessionStatus, ThreadView};
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView, WorktreeFolder};
use wsp_wire::wire::daemon::GitCommitReply;
use wsp_wire::wire::runtime::{
    SessionInterruptOutcome, SessionInterruptResult, SessionRenameOutcome, SessionRenameResult, SessionRestoreResult, SessionSettleResult,
};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::place_state::cap_wait_line;
use wsp_words::projects::home_shortened;
use wsp_words::pull_request::{fix_asked_line, fix_conflicts_line, fix_nothing_line, update_conflicts_line, updated_line};
use wsp_words::start::start_words::made;
use wsp_words::start::ReviewVerdict;
use wsp_words::tree::fix_merge_child_line;
use wsp_words::words::computer::{thread_without_id_refusal, thread_word};
use wsp_words::words::thread::{shared_folder_line, workspace_asleep_again_line, workspace_stays_awake_line};
use wsp_words::workspace_state::{gone_refusal, workspace_state, WorkspaceState, WorkspaceStateInput};

use super::reads::{params, reply_field, thread_of, threads, workspace_of, workspace_statuses};
use crate::verbs::client::{access_word_of, opened_thread_said, other_version, pick, thread_id_of, under, Frame, HostClient};
use crate::verbs::turns_help::Turn;

fn parsed<T: DeserializeOwned>(reply: Frame, op: &str) -> Result<T, HostError> {
    serde_json::from_value(Value::Object(reply)).map_err(|_| HostError::new(other_version(op)))
}

fn word_of<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(word)) => word,
        _ => String::new(),
    }
}

fn view_of(out: &WorkspaceOut) -> Result<WorkspaceView, HostError> {
    serde_json::to_value(out)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(|| HostError::new(other_version("workspaces")))
}

fn out_of(view: &WorkspaceView) -> Result<WorkspaceOut, HostError> {
    serde_json::to_value(view)
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(|| HostError::new(other_version("workspaces")))
}

fn commit_scope(mut body: Frame, files: &[String], thread_id: Option<&str>) -> Frame {
    if !files.is_empty() {
        body.insert("paths".to_owned(), json!(files));
    }
    if let Some(thread_id) = thread_id {
        body.insert("threadId".to_owned(), json!(thread_id));
    }
    body
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0)
}

fn said_of(thread_id: &str, thread: Option<&(dyn Fn(&str) -> String + Send + Sync)>) -> String {
    match thread {
        Some(line) => line(thread_id),
        None => opened_thread_said(thread_id, None, None),
    }
}

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
    let source = workspace_of(client, r#ref).await?;
    let reply = client.request("workspaces.rename", params(json!({ "workspaceId": source.id, "name": name }))).await?;
    Ok(RenamedWorkspace { was: source.name, workspace: reply_field(&reply, "workspaces.rename", "workspace")? })
}

/// `renamedWorkspaceLine(r)` (workspaces-help.ts:325): `<was> is now <name> <id>`.
pub fn renamed_workspace_line(r: &RenamedWorkspace) -> String {
    format!("{} is now {} {}", r.was, r.workspace.name, r.workspace.id)
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
pub async fn awake(
    client: &dyn HostClient,
    workspace: &WorkspaceView,
    action: &str,
    tell: &(dyn Fn(&str) + Send + Sync),
) -> Result<Woken, HostError> {
    let before = workspace_state(&WorkspaceStateInput { phase: workspace.phase, machine_state: None, reach: None, wake_refused: None });
    if before == WorkspaceState::Gone {
        return Err(HostError::new(gone_refusal(Some(&workspace.name), action, workspace.gone.as_deref())));
    }
    if before == WorkspaceState::Released && action == "send" {
        return Ok(Woken { workspace: out_of(workspace)?, woke: false, said: None });
    }
    if before != WorkspaceState::Running {
        tell(&format!("waking {}", workspace.name));
    }
    let reply = client.request("workspaces.wake", params(json!({ "workspaceId": workspace.id }))).await?;
    let woken: WorkspaceOut = reply_field(&reply, "workspaces.wake", "workspace")?;
    let down = matches!(before, WorkspaceState::Paused | WorkspaceState::Pausing);
    let said = if down { woken.disk_gone.clone() } else { None };
    if let Some(line) = &said {
        tell(line);
    }
    let now = workspace_state(&WorkspaceStateInput { phase: woken.phase, machine_state: None, reach: None, wake_refused: None });
    let woke = down && now == WorkspaceState::Running;
    Ok(Woken { workspace: woken, woke, said })
}

/// `noDraftLine(note)` (workspaces-help.ts:360): `no message was drafted: <note>`, `the agent gave none` where absent
/// (an empty note stays empty).
pub fn no_draft_line(note: Option<&str>) -> String {
    format!("no message was drafted: {}", note.unwrap_or("the agent gave none"))
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
    let text = match message {
        Some(text) => text.to_owned(),
        None => {
            let body = commit_scope(params(json!({ "workspaceId": workspace_id })), files, thread_id);
            let draft: CommitDraft = parsed(client.request("workspaces.commitDraft", body).await?, "workspaces.commitDraft")?;
            let Some(text) = draft.message else {
                return Err(usage_refusal(&no_draft_line(draft.note.as_deref()), NO_DRAFT_FIX));
            };
            say(&text);
            text
        }
    };
    let body = commit_scope(params(json!({ "workspaceId": workspace_id, "message": text })), files, thread_id);
    parsed(client.request("workspaces.commit", body).await?, "workspaces.commit")
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
pub async fn asked_to_fix(
    client: &dyn HostClient,
    workspace_id: &str,
    check: Option<&str>,
    child: Option<&ChildRef>,
    thread_id: &str,
) -> Result<FixResult, HostError> {
    let body = pick(vec![
        ("workspaceId", Some(json!(workspace_id))),
        ("check", check.map(|check| json!(check))),
        ("child", child.map(|child| json!(child.workspace_id))),
        ("childThreadId", child.map(|child| json!(child.thread_id))),
        ("threadId", Some(json!(thread_id))),
    ]);
    parsed(client.request("workspaces.fix", body).await?, "workspaces.fix")
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
pub async fn merged_in(
    client: &dyn HostClient,
    lead: &MergeLead,
    child: &ChildRef,
    said: &(dyn Fn(&str) + Send + Sync),
) -> Result<MergeInResult, HostError> {
    let awoken = awake(client, &view_of(&lead.workspace)?, "merge in", said).await?.workspace;
    let body = params(
        json!({ "workspaceId": awoken.id, "child": child.workspace_id, "threadId": lead.thread_id, "childThreadId": child.thread_id }),
    );
    parsed(client.request("workspaces.mergeIn", body).await?, "workspaces.mergeIn")
}

/// `fixLine(thread, asked, child?)` (workspaces-help.ts:394-399): `updated` is `fix_nothing_line(thread, base)`. Any
/// other outcome names the agent by `agent_name` and reads `fix_merge_child_line(thread, agent, child ?? asked.child)`
/// where the answer names a child, else `fix_asked_line(thread, agent, check)` where it names a check, else
/// `fix_conflicts_line(thread, agent, base)`; a `held` answer with a `capped` wait adds `\n` and `cap_wait_line`.
pub fn fix_line(thread: &str, asked: &FixResult, child: Option<&str>) -> String {
    match asked {
        FixResult::V1(v) => fix_nothing_line(thread, &v.base),
        FixResult::V0(v) => {
            let agent = agent_name(&v.agent);
            let line = if let Some(child_name) = &v.child {
                fix_merge_child_line(thread, &agent, child.unwrap_or(child_name.as_str()))
            } else if let Some(check) = &v.check {
                fix_asked_line(thread, &agent, check)
            } else {
                fix_conflicts_line(thread, &agent, &v.base)
            };
            match &v.capped {
                Some(capped) if matches!(v.outcome, FixResultV0Outcome::Held) => format!("{line}\n{}", cap_wait_line(capped)),
                _ => line,
            }
        }
    }
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
    let access = match &o.access {
        Some(given) => Some(word_of(&access_word_of(given)?)),
        None => None,
    };
    let body = pick(vec![
        ("url", Some(json!(o.url))),
        ("project", o.project.as_ref().map(|v| json!(v))),
        ("agent", o.agent.as_ref().map(|v| json!(v))),
        ("model", o.model.as_ref().map(|v| json!(v))),
        ("effort", o.effort.as_ref().map(|v| json!(v))),
        ("access", access.map(|v| json!(v))),
    ]);
    parsed(client.request("workspaces.start", body).await?, "workspaces.start")
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
    let body = pick(vec![
        ("url", o.url.as_ref().map(|v| json!(v))),
        ("workspaceId", o.workspace_id.as_ref().map(|v| json!(v))),
        ("agent", o.agent.as_ref().map(|v| json!(v))),
        ("model", o.model.as_ref().map(|v| json!(v))),
        ("effort", o.effort.as_ref().map(|v| json!(v))),
    ]);
    parsed(client.request("workspaces.review", body).await?, "workspaces.review")
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
pub async fn review_posted(
    client: &dyn HostClient,
    workspace_id: &str,
    edits: &ReviewEdits,
    thread_id: &str,
) -> Result<ReviewPostResult, HostError> {
    if edits.verdict.is_some() || edits.summary.is_some() {
        let body = pick(vec![
            ("workspaceId", Some(json!(workspace_id))),
            ("verdict", edits.verdict.as_ref().map(|v| json!(v))),
            ("summary", edits.summary.as_ref().map(|v| json!(v))),
        ]);
        client.request("workspaces.reviewDraft", body).await?;
    }
    parsed(
        client.request("workspaces.reviewPost", params(json!({ "workspaceId": workspace_id, "threadId": thread_id }))).await?,
        "workspaces.reviewPost",
    )
}

/// `startedLines(started, thread?)` (workspaces-help.ts:423-425): `START_WORDS.made(workspace name, from)`, `\n`, then
/// the caller's line for the thread id, `opened_thread_said(id, None)` by default.
pub fn started_lines(started: &StartResult, thread: Option<&(dyn Fn(&str) -> String + Send + Sync)>) -> String {
    let made_line = made(&started.workspace.name, started.workspace.from.as_ref());
    format!("{made_line}\n{}", said_of(&started.thread_id, thread))
}

/// `reviewLines(started, thread?)` (workspaces-help.ts:428-430): the thread line first, then
/// `START_WORDS.made("thread <thread_word(id)>", from)`.
pub fn review_lines(started: &StartResult, thread: Option<&(dyn Fn(&str) -> String + Send + Sync)>) -> String {
    let made_line = made(&format!("thread {}", thread_word(&started.thread_id)), started.workspace.from.as_ref());
    format!("{}\n{made_line}", said_of(&started.thread_id, thread))
}

/// `threadLabel(thread)` (workspaces-help.ts:433): `thread <thread_word(thread_id_of(thread))>`.
pub fn thread_label(thread: &ThreadView) -> String {
    format!("thread {}", thread_word(&thread_id_of(thread)))
}

/// `threadCwd(t, workspace)` (workspaces-help.ts:437): the thread's folder `cwd`, absent where the workspace's
/// worktree is gone and the folder is the worktree's path or inside it (`under`), since the next turn runs where the
/// record answers now. `worktree` is `workspace?.worktree`.
pub fn thread_cwd(cwd: Option<&str>, worktree: Option<&WorktreeFolder>) -> Option<String> {
    let moved = match worktree {
        Some(w) if w.gone.is_some() => cwd.is_some_and(|c| under(c, &w.path)),
        _ => false,
    };
    if moved {
        None
    } else {
        cwd.map(str::to_owned)
    }
}

/// `folderOf(workspace)` (workspaces-help.ts:443): `home_shortened(folder ?? project.path, home)`. `home` is what
/// `os.homedir()` answers; the caller passes it so a test needs no process environment.
pub fn folder_of(workspace: &WorkspaceOut, home: Option<&str>) -> String {
    home_shortened(workspace.folder.as_deref().unwrap_or(&workspace.project.path), home)
}

/// `withShared(line, shared)` (workspaces-help.ts:446): the line, and where threads share the folder, `\n` and
/// `shared_folder_line` of their `thread_word`s.
pub fn with_shared(line: &str, shared: &[String]) -> String {
    if shared.is_empty() {
        line.to_owned()
    } else {
        format!("{line}\n{}", shared_folder_line(&shared.iter().map(|id| thread_word(id)).collect::<Vec<_>>()))
    }
}

/// `sharedOf(shared)` (workspaces-help.ts:449): the ids whole and in order, for the `sharedWith` field of an answer;
/// `None` where none share it (the TS answers `{}` or `{sharedWith: [...]}`).
pub fn shared_of(shared: &[String]) -> Option<Vec<String>> {
    if shared.is_empty() {
        None
    } else {
        Some(shared.to_vec())
    }
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
pub async fn merged_pr(
    client: &dyn HostClient,
    workspace_id: &str,
    method: Option<MergeMethod>,
    when_checks_pass: bool,
    thread_id: &str,
) -> Result<MergeResult, HostError> {
    let mut body = pick(vec![
        ("workspaceId", Some(json!(workspace_id))),
        ("method", method.map(|m| json!(m))),
        ("whenChecksPass", when_checks_pass.then(|| json!(true))),
    ]);
    body.insert("threadId".to_owned(), json!(thread_id));
    parsed(client.request("workspaces.merge", body).await?, "workspaces.merge")
}

/// `updateLine(workspace, done)` (workspaces-help.ts:460): `updated_line(workspace, base, commits)` where merged,
/// else `update_conflicts_line(workspace, base, conflicts)` (the files listed are not read where merged).
pub fn update_line(workspace: &str, done: &GitUpdateReply) -> String {
    if done.merged {
        updated_line(workspace, &done.base, done.commits.0 .0)
    } else {
        update_conflicts_line(workspace, &done.base, &done.conflicts)
    }
}

/// `napAfterDeadLaunch(client, woken, turn)` (workspaces-help.ts:469-490): nothing where this call did not wake the
/// machine, or where the turn's result names a refusal (the agent ran). Else `sessions.list {workspaceId}`; nothing
/// where the turn's thread (matched by `thread_id_of`) did work (`ran`); where any other thread is `running`,
/// `status.list` and `workspace_stays_awake_line(name, idleAt - now, floored at 0, or none)`; else `workspaces.nap
/// {workspaceId}` and `workspace_asleep_again_line(name)`. Any failure after the first checks answers
/// `workspace_stays_awake_line(name, None)`: this call never fails.
pub async fn nap_after_dead_launch(client: &dyn HostClient, woken: &Woken, turn: Option<&Turn>) -> Option<String> {
    if !woken.woke {
        return None;
    }
    if turn.and_then(|t| t.result.as_ref()).is_some_and(|result| result.refusal.is_some()) {
        return None;
    }
    match launch_owed(client, woken, turn).await {
        Ok(line) => line,
        Err(_) => Some(workspace_stays_awake_line(&woken.workspace.name, None)),
    }
}

/// `withLine(e, line)` (workspaces-help.ts:494): the same refusal with `line` under its message (`\n`), carrying only
/// the `kind` stamp over (not the fix, the problem lists or any mark). No line leaves it as it was. (The TS also
/// passes a thrown value that is no error through; a `HostError` is always one.)
pub fn with_line(e: HostError, line: Option<&str>) -> HostError {
    let Some(line) = line else { return e };
    let message = format!("{}\n{line}", e.message);
    match e.kind {
        Some(kind) => HostError::with_kind(message, kind),
        None => HostError::new(message),
    }
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
    let thread = thread_of(client, r#ref).await?;
    let body = pick(vec![("sessionId", Some(json!(thread.session_id))), ("task", subagent.map(|task| json!(task)))]);
    let result: SessionInterruptResult = parsed(client.request("sessions.interrupt", body).await?, "sessions.interrupt")?;
    Ok(Stopped {
        thread_id: thread.id,
        task: subagent.map(str::to_owned),
        outcome: result.outcome,
        under: result.under.filter(|under| !under.is_empty()),
        error: result.error,
        left: result.left,
    })
}

/// `stopLine(stopped)` (workspaces-help.ts:525-531): `thread <id>[ subagent <task>]`, then `<named>: <error>` where
/// an error stands; else the outcome's words (accepted `stopped`, not-running `not running`, not-found `not found by
/// the host`, refused and unsupported `not stopped`), then where threads are under it `, and with it <n> thread[s]
/// its agents spawned: <thread_word, ...>`, then `, but <left>` where the stop left something.
pub fn stop_line(stopped: &Stopped) -> String {
    let mut named = format!("thread {}", stopped.thread_id);
    if let Some(task) = &stopped.task {
        named = format!("{named} subagent {task}");
    }
    if let Some(error) = &stopped.error {
        return format!("{named}: {error}");
    }
    let under = stopped.under.as_deref().unwrap_or_default();
    let tree = if under.is_empty() {
        String::new()
    } else {
        let count = under.len();
        let noun = if count == 1 { "thread" } else { "threads" };
        let ids: Vec<String> = under.iter().map(|id| thread_word(id)).collect();
        format!(", and with it {count} {noun} its agents spawned: {}", ids.join(", "))
    };
    let words = match stopped.outcome {
        SessionInterruptOutcome::Accepted => "stopped",
        SessionInterruptOutcome::NotRunning => "not running",
        SessionInterruptOutcome::NotFound => "not found by the host",
        SessionInterruptOutcome::Refused | SessionInterruptOutcome::Unsupported => "not stopped",
    };
    let left = stopped.left.as_ref().map(|left| format!(", but {left}")).unwrap_or_default();
    format!("{named} {words}{tree}{left}")
}

/// `forgetThread(client, thread, o)` (workspaces-help.ts:537): a row with no `threadId` fails a plain error
/// `thread_without_id_refusal(id)` before any frame; else `sessions.forget {threadId, check?: true}` (`check` only
/// where true).
pub async fn forget_thread(client: &dyn HostClient, thread: &ThreadView, check: bool) -> Result<(), HostError> {
    let Some(thread_id) = thread.thread_id.as_deref() else {
        return Err(HostError::new(thread_without_id_refusal(&thread.id)));
    };
    let body = pick(vec![("threadId", Some(json!(thread_id))), ("check", check.then(|| json!(true)))]);
    client.request("sessions.forget", body).await?;
    Ok(())
}

/// `threadForgotLine(thread)` (workspaces-help.ts:543): `forgot thread <id>: no turn ever ran on it, so nothing of
/// its work is gone`.
pub fn thread_forgot_line(thread: &ThreadView) -> String {
    format!("forgot thread {}: no turn ever ran on it, so nothing of its work is gone", thread.id)
}

/// `settleThreads(client, threads, finished, processes?)` (workspaces-help.ts:549): `sessions.settle {threadIds (each
/// row's `id`, not its thread id), finished?: true, processes?}`, parsed as a `SessionSettleResult`.
pub async fn settle_threads(
    client: &dyn HostClient,
    threads: &[ThreadView],
    finished: bool,
    processes: Option<ProcessesAsk>,
) -> Result<SessionSettleResult, HostError> {
    let ids: Vec<&str> = threads.iter().map(|t| t.id.as_str()).collect();
    let body = pick(vec![
        ("threadIds", Some(json!(ids))),
        ("finished", finished.then(|| json!(true))),
        ("processes", processes.as_ref().map(|p| json!(p))),
    ]);
    parsed(client.request("sessions.settle", body).await?, "sessions.settle")
}

/// `restoreThreads(client, threads)` (workspaces-help.ts:554): `sessions.restore {threadIds}`, parsed as a
/// `SessionRestoreResult`.
pub async fn restore_threads(client: &dyn HostClient, threads: &[ThreadView]) -> Result<SessionRestoreResult, HostError> {
    let ids: Vec<&str> = threads.iter().map(|t| t.id.as_str()).collect();
    parsed(client.request("sessions.restore", params(json!({ "threadIds": ids }))).await?, "sessions.restore")
}

/// `settledLines(answer)` (workspaces-help.ts:559): one line per settled thread `settled <thread_word> <title>`, then
/// one per thread left `left <thread_word>: <why>`, joined by `\n`; `nothing settled` where there are none.
pub fn settled_lines(answer: &SessionSettleResult) -> String {
    let mut lines: Vec<String> = answer.settled.iter().map(|t| format!("settled {} {}", thread_word(&t.thread_id), t.title)).collect();
    lines.extend(answer.left.iter().map(|t| format!("left {}: {}", thread_word(&t.thread_id), t.why)));
    if lines.is_empty() {
        "nothing settled".to_owned()
    } else {
        lines.join("\n")
    }
}

/// `restoredLines(answer)` (workspaces-help.ts:565): `restored <thread_word> <title>` per thread, `\n` between;
/// `nothing restored` where none.
pub fn restored_lines(answer: &SessionRestoreResult) -> String {
    if answer.restored.is_empty() {
        "nothing restored".to_owned()
    } else {
        answer.restored.iter().map(|t| format!("restored {} {}", thread_word(&t.thread_id), t.title)).collect::<Vec<_>>().join("\n")
    }
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
    let reply = client.request("sessions.rename", params(json!({ "sessionId": thread.session_id, "title": title }))).await?;
    let result: SessionRenameResult = parsed(reply, "sessions.rename")?;
    Ok(Renamed {
        thread_id: thread.id.clone(),
        title: title.to_owned(),
        harness: thread.harness.clone(),
        outcome: result.outcome,
        error: result.error,
    })
}

/// `renameLine(renamed)` (workspaces-help.ts:588-598): `thread <threadId> <words>` where the words are `named
/// <title>, in <agent> too` (renamed), `not named: <agent> keeps no name of a person's for a session` (unsupported),
/// `not named: <agent> on the machine has no such session` (no-session), `not named: <error, or the machine said
/// nothing about the write>` (failed), `not found by the host` (not-found); `<agent>` is `agent_name(harness)`.
pub fn rename_line(renamed: &Renamed) -> String {
    let agent = agent_name(&renamed.harness);
    let words = match renamed.outcome {
        SessionRenameOutcome::Renamed => format!("named {}, in {agent} too", renamed.title),
        SessionRenameOutcome::Unsupported => format!("not named: {agent} keeps no name of a person's for a session"),
        SessionRenameOutcome::NoSession => format!("not named: {agent} on the machine has no such session"),
        SessionRenameOutcome::Failed => {
            format!("not named: {}", renamed.error.as_deref().unwrap_or("the machine said nothing about the write"))
        }
        SessionRenameOutcome::NotFound => "not found by the host".to_owned(),
    };
    format!("thread {} {words}", renamed.thread_id)
}

async fn launch_owed(client: &dyn HostClient, woken: &Woken, turn: Option<&Turn>) -> Result<Option<String>, HostError> {
    let name = woken.workspace.name.as_str();
    let rows = threads(client, Some(&woken.workspace.id)).await?;
    let mine = turn.and_then(|t| rows.iter().position(|row| thread_id_of(row) == t.thread_id));
    if mine.is_some_and(|i| rows[i].ran) {
        return Ok(None);
    }
    if rows.iter().enumerate().any(|(i, row)| Some(i) != mine && matches!(row.status, SessionStatus::Running)) {
        let listed = workspace_statuses(client).await?.into_iter().find(|w| w.id == woken.workspace.id);
        let naps = listed.and_then(|w| w.idle_at).map(|at| (at.0 - now_ms()).max(0.0));
        return Ok(Some(workspace_stays_awake_line(name, naps)));
    }
    client.request("workspaces.nap", params(json!({ "workspaceId": woken.workspace.id }))).await?;
    Ok(Some(workspace_asleep_again_line(name)))
}
