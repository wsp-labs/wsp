// SPDX-License-Identifier: AGPL-3.0-only
//! Everything that runs in or reads a workspace's checkout: commands, bring back, the folder doors, the Changes pane,
//! the pull request page and its acts, fix, merge, update and merge in. workspaces.ts:647-1053 at 65f6b89c7;
//! spec/workspaces_git.md. Daemon ops go through `Base::with_daemon` and `on_this_computer`, whose `ask` a test records.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::FutureExt;
use serde_json::{json, Value};
use wsp_core::types::api::{
    CommitAsk, CommitDraftAsk, DiscardAsk, FixAsk, FolderFor, FolderForAsk, MergeAsk, MergeInAsk, PullRequestReplyAsk, ViewedMark,
    WorktreeRemoveAsk,
};
use wsp_core::types::internal::{default_branch_refused, is_no_git_credential, is_no_host_cli, is_on_default_branch, DaemonRefusal};
use wsp_core::types::wiring::{RunningExec, WorkspaceRecord};
use wsp_frames::{GitCommitReply, GitDiffReply, GitDiscardReply, GitPrMergeReply, GitRunLogReply, GitStatusReply, MergeMethod};
use wsp_wire::types::bring_back::{BringBackResult, GitPrReply, GitPushReply};
use wsp_wire::types::changes::{CheckoutReply, CommitDraft, ViewedMarks, DRAFT_NOTES};
use wsp_wire::types::daemon_contract::GIT_DIFF_CAP_BYTES;
use wsp_wire::types::projects::WORKTREE_FORCE_LINE;
use wsp_wire::types::pull_request::PullRequestFact;
use wsp_wire::types::pull_request::{
    FixResult, GitPrReactReply, GitPrReplyReply, GitPrResolveReply, GitUpdateReply, MergeResult, PullRequestItem, PullRequestPage,
    PullRequestSeen, PullRequestSendResult, PullRequestSent, ReactionContent, AUTO_MERGE_OFF_LINE, REPLY_EMPTY_LINE,
};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::types::start::{GitPrDiffReply, WorkspaceFromKind};
use wsp_wire::types::tree::{MergeInResult, TreeRecord, FIX_CHECK_OR_CHILD};
use wsp_wire::views::session::{SessionStatus, SessionView};
use wsp_wire::views::workspace::{scope_of, Caller, ProjectView};
use wsp_wire::views::workspace_more::{WorkspaceView, WorktreeMade};
use wsp_wire::wire::daemon::DETACHED_HEAD;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::changes::{clean_checkout_line, commit_message, cut_diff, draft_prompt};
use wsp_words::daemon_contract::no_change_line;
use wsp_words::exit::{not_found_refusal, HostError, Kind};
use wsp_words::projects::{
    copies_folder, no_branches_line, not_made_worktree_line, not_on_this_computer_line, place_branch_line, runs_in_folder,
};
use wsp_words::pull_request::{
    check_failed_prompt, check_not_failed_refusal, child_pushed_line, conflicts_prompt, merge_method_refusal, no_pull_request_refusal,
    no_such_check_refusal, not_open_refusal, pull_request_send_prompt, CheckLog, CheckedCommit, PageItem,
};
use wsp_words::start::with_closes;
use wsp_words::tree::{
    child_on_no_branch_refusal, lead_busy_refusal, merge_child_prompt, merge_into_own_refusal, no_remote_for_tree_line, MergeChild,
};
use wsp_words::words::computer::{AcrossAct, DefaultBranchRoad};
use wsp_words::words::thread::thread_elsewhere_line;
use wsp_words::words::turn::opening_cut;

use super::create::id_of;
use super::seam::{record_of, Base, DaemonAsk, DaemonWork, Entry, EntryAsk, Queued};
use crate::projects::folders::tree::{ListedWorktree, RemoveAsk};

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn read_as<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, HostError> {
    serde_json::from_value(v).map_err(|e| HostError::new(e.to_string()))
}

fn refusal_of(e: &HostError) -> DaemonRefusal {
    let code = e.code.clone().or_else(|| match &e.kind {
        Some(Kind::Other(word)) => Some(word.clone()),
        _ => None,
    });
    DaemonRefusal { code, message: e.message.clone() }
}

/// A reply built by hand leaves its absent optional keys out, as the TS object literal does.
fn without_nulls(mut v: Value) -> Value {
    if let Value::Object(map) = &mut v {
        map.retain(|_, value| !value.is_null());
    }
    v
}

fn with_base(mut v: Value, base: &Option<String>) -> Value {
    if let Some(base) = base {
        v["base"] = json!(base);
    }
    v
}

fn home_cwd(ctx: &Rc<dyn GitCtx>) -> String {
    ctx.home_dir()
}

/// One daemon op on an entry's computer (`ctx.withDaemon(entry, ask => ask(op))`).
async fn ask_daemon(ctx: &Rc<dyn GitCtx>, entry: &Entry, op: Value) -> Result<Value, HostError> {
    let work: DaemonWork = Box::new(move |ask: DaemonAsk| async move { ask(op).await }.boxed_local());
    ctx.with_daemon(entry, work).await
}

fn tree_or_empty(tree: Option<TreeRecord>) -> TreeRecord {
    tree.unwrap_or(TreeRecord { merged: None, conflicts: None, push_refused: None })
}

fn draft_note(key: &str) -> String {
    DRAFT_NOTES.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned()).unwrap_or_default()
}

fn draft_empty(note: &str) -> CommitDraft {
    CommitDraft { message: None, note: Some(note.to_owned()) }
}

fn same_item(a: &Value, b: &Value) -> bool {
    a["kind"] == b["kind"] && a["id"] == b["id"]
}

/// The sessions of a workspace's thread, as the TS lists them (`sessions.values()` filtered).
fn thread_has_session(ctx: &Rc<dyn GitCtx>, workspace_id: &str, thread_id: &str) -> bool {
    ctx.session_views().iter().any(|s| s.workspace_id == workspace_id && s.thread_id.as_deref() == Some(thread_id))
}

/// The pull request fact the window drew, held on the live entry (`isPullRequestFact(entry.pr)`).
fn live_pr_fact(entry: &Entry) -> Option<PullRequestFact> {
    match entry.borrow().pr.clone() {
        Some(PullRequestSeen::PullRequestFact(fact)) => Some(fact),
        _ => None,
    }
}

#[derive(serde::Deserialize)]
struct MergeInReply {
    merged: bool,
    commits: u64,
    #[serde(default)]
    oid: Option<String>,
    #[serde(default)]
    head: Option<String>,
    #[serde(default)]
    conflicts: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergeSettings {
    methods: Vec<MergeMethod>,
    default_method: MergeMethod,
    auto_merge: bool,
}

/// What a send to a thread answered (`toFirstThread`, `sendDetached`).
#[derive(Clone, Debug, PartialEq)]
pub struct SentTo {
    pub outcome: String,
    pub thread_id: String,
    pub harness: String,
    pub capped: Option<serde_json::Value>,
}

/// `ctx.pullRequestOn(id, origin)`.
#[derive(Clone)]
pub struct PullRequestOn {
    pub entry: Entry,
    pub remote: String,
    pub number: i64,
}

/// What the checkout half reads off the runtime context, past `Base`.
#[async_trait(?Send)]
pub trait GitCtx: Base {
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError>;
    /// `entry.machine.exec(cmd, o)`.
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: Option<u64>) -> Result<ExecResult, HostError>;
    /// The adapter's env, the folder the command runs in, and the unbounded factory's stream, registered in `execs`.
    async fn exec_stream_of(&self, entry: &Entry, argv: Vec<String>, cwd: Option<String>) -> Result<RunningExec, HostError>;
    /// `ctx.queued(id, work)` (folders').
    async fn queued(&self, id: &str, work: Queued<serde_json::Value>) -> Result<serde_json::Value, HostError>;
    async fn keep_tree(&self, entry: &Entry, tree: TreeRecord) -> Result<(), HostError>;
    async fn take_pull_request(&self, entry: &Entry, pr: serde_json::Value) -> Result<(), HostError>;
    fn read_lead_of(&self, entry: &Entry);
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<(Entry, Option<String>, Option<String>), HostError>;
    fn take_setup_line(&self, id: &str) -> Option<String>;
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError>;
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    fn refuse_relayed(&self, record: &WorkspaceRecord, origin: Option<&Caller>) -> Result<(), HostError>;
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError>;
    async fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str, made_for: Option<String>) -> Result<Entry, HostError>;
    async fn worktrees_of(&self, top: &str) -> Result<Vec<ListedWorktree>, HostError>;
    fn folders_of(&self, project_id: &str) -> Vec<Entry>;
    async fn remove_worktree(&self, entry: &Entry, force: bool, ask: RemoveAsk) -> Result<(), HostError>;
    async fn read_checkout(&self, entry: &Entry, fresh: bool) -> Result<Option<wsp_wire::types::changes::Checkout>, HostError>;
    /// `ctx.readPullRequest(entry, force, whole?)`: the fact, as JSON, none where none.
    async fn read_pull_request(&self, entry: &Entry, force: bool, whole: bool) -> Result<Option<serde_json::Value>, HostError>;
    async fn read_tree(&self, entry: &Entry) -> Result<(), HostError>;
    fn label_of(&self, entry: &Entry, thread_id: Option<&str>) -> String;
    /// The first `session.start` event of the thread, read off its transcript.
    async fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<serde_json::Value>, HostError>;
    fn default_agent_of(&self, entry: &Entry) -> String;
    fn agent_off(&self, entry: &Entry, agent: &str) -> bool;
    fn has_adapter(&self, agent: &str) -> bool;
    /// `launchAdapterFor`, `catalogOn` and `smallestModel`: whether the adapter drafts, and the model.
    async fn drafter(&self, entry: &Entry, agent: &str) -> Result<Option<Option<String>>, HostError>;
    async fn put_files(&self, entry: &Entry, path: &str, text: &str) -> Result<ExecResult, HostError>;
    async fn draft_for(&self, entry: &Entry, agent: &str, prompt_file: &str, model: Option<&str>) -> Result<Option<String>, HostError>;
    fn viewed_marks(&self) -> Rc<std::cell::RefCell<indexmap::IndexMap<String, ViewedMarks>>>;
    async fn persist_sessions(&self, workspace_id: &str) -> Result<(), HostError>;
    async fn pull_request_on(&self, workspace_id: &str, origin: Option<Caller>) -> Result<PullRequestOn, HostError>;
    async fn read_page(&self, entry: &Entry, remote: &str, number: i64, fresh: bool) -> Result<(PullRequestPage, bool), HostError>;
    async fn merge_settings(&self, remote: &str) -> Result<serde_json::Value, HostError>;
    async fn read_host(&self, entry: &Entry, ask: serde_json::Value) -> Result<serde_json::Value, HostError>;
    async fn post_as_person(&self, remote: &str, number: i64, ask: serde_json::Value) -> Result<serde_json::Value, HostError>;
    async fn to_first_thread(&self, workspace_id: &str, prompt: &str, origin: Option<Caller>) -> Result<SentTo, HostError>;
    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str, origin: Option<Caller>) -> Result<SentTo, HostError>;
    async fn child_of(
        &self,
        lead: &Entry,
        child: &str,
        origin: Option<Caller>,
        lead_thread: Option<&str>,
        child_thread: Option<&str>,
    ) -> Result<Entry, HostError>;
    async fn update_copy(&self, entry: &Entry) -> Result<GitUpdateReply, HostError>;
    fn drop_page_cache(&self, remote: &str, number: i64);
    fn is_local_workspace(&self, record: &WorkspaceRecord) -> bool;
}

// ts: workspaces.ts:647-652
//|     async exec(id, cmd, o, origin) {
//|       const entry = await ctx.entryOf(id, origin, { act: "exec" });
//|       await ctx.copyBlocked(entry);
//|       await ctx.worktreeMounted(entry);
//|       return entry.machine.exec(cmd, o);
//|     },
pub async fn exec(
    ctx: &Rc<dyn GitCtx>,
    id: &str,
    cmd: &str,
    timeout_ms: Option<u64>,
    origin: Option<Caller>,
) -> Result<ExecResult, HostError> {
    let entry = ctx.entry_of(id, origin, EntryAsk { act: Some(AcrossAct::Exec), ..EntryAsk::default() }).await?;
    ctx.copy_blocked(&entry).await?;
    ctx.worktree_mounted(&entry).await?;
    ctx.machine_exec(&entry, cmd, timeout_ms).await
}

// ts: workspaces.ts:654-691
//|     async execStream(id, argv, cwd, origin) {
//|       const entry = await ctx.entryOf(id, origin, { act: "exec" });
//|       await ctx.copyBlocked(entry);
//|       await ctx.worktreeMounted(entry);
//|       const { adapter } = await ctx.launchAdapterFor(entry);
//|       // Only the socket or the machine going away ends a command; a build may outlive the deadline a harness turn gets.
//|       const ranIn = await ctx.threadFolder(entry, { cwd });
//|       const inner = ctx.execFactoryFor(entry, { idleMs: Number.POSITIVE_INFINITY, deadlineMs: Number.POSITIVE_INFINITY })(inFolder(ranIn, argv.map(shellQuote).join(" ")), { env: { ...adapter.env } });
//|       let endWith: (reason: string) => void = () => {};
//|       const ended = new Promise<{ reason: string }>(resolve => {
//|         endWith = reason => resolve({ reason });
//|       });
//|       const running = {
//|         workspaceId: id,
//|         end: (reason: string): void => {
//|           endWith(reason);
//|           inner.kill();
//|         },
//|       };
//|       execs.add(running);
//|       // The inner poll loop notices the kill one poll late; the reason reaches the reader as soon as it is known.
//|       const lines = async function* (): AsyncGenerator<string> {
//|         const it = inner.lines[Symbol.asyncIterator]();
//|         try {
//|           while (true) {
//|             const next = await Promise.race([it.next(), ended]);
//|             if ("reason" in next) throw new Error(next.reason);
//|             if (next.done) return;
//|             yield next.value;
//|           }
//|         } finally {
//|           execs.delete(running);
//|           // Lets go of the reader too: left at the line it last handed out, it keeps that line and its buffer.
//|           void it.return?.(undefined).catch(() => undefined);
//|         }
//|       };
//|       return { ...inner, lines: lines(), exited: Promise.race([inner.exited, ended.then(() => null)]), ...(ranIn !== undefined ? { ranIn } : {}) };
//|     },
pub async fn exec_stream(
    ctx: &Rc<dyn GitCtx>,
    id: &str,
    argv: Vec<String>,
    cwd: Option<String>,
    origin: Option<Caller>,
) -> Result<RunningExec, HostError> {
    let entry = ctx.entry_of(id, origin, EntryAsk { act: Some(AcrossAct::Exec), ..EntryAsk::default() }).await?;
    ctx.copy_blocked(&entry).await?;
    ctx.worktree_mounted(&entry).await?;
    ctx.exec_stream_of(&entry, argv, cwd).await
}

// ts: workspaces.ts:693-746
//|     async bringBack({ workspaceId, title, body: given }, origin) {
//|       let body = given;
//|       ctx.spawnGuard("bring_back", origin);
//|       const entry = await ctx.entryOf(workspaceId, origin);
//|       await ctx.copyBlocked(entry);
//|       const cwd = ctx.checkoutOf(entry.record);
//|       // The branch this copy started from, off its own record: for a child that is the branch its parent was on at
//|       // the fork, which is the code it was cut from and so where its work goes back, and nothing is asked of the
//|       // parent's machine, so a child whose parent has gone to sleep brings its work back without a wake nobody
//|       // named. A record written before that fact was kept reads the branch its project starts from, as it did.
//|       const base = entry.record.base ?? ctx.projectHeld(entry.record.project).base;
//|       const against = base === undefined ? {} : { base };
//|       // A pull request off someone's fork takes a push only where its author allowed maintainers to push; refused
//|       // before anything is pushed.
//|       const fork = entry.record.from?.head?.fork;
//|       if (entry.record.from?.kind === "pull_request" && fork !== undefined && !fork.pushable) throw new Error(START_WORDS.forkNotPushable(fork.owner));
//|       // Work on an issue closes it once its pull request merges: the body says so, once, whoever wrote the rest of it.
//|       const issue = entry.record.from?.kind === "issue" ? entry.record.from.number : undefined;
//|       if (issue !== undefined) body = withCloses(body ?? "", issue);
//|       const brought = await ctx.queued(entry.record.id, () => ctx.withDaemon(entry, async (ask): Promise<BringBackResult> => {
//|         const push = GitPushReply.parse(
//|           await ask({ op: "git.push", cwd, ...against }).catch(async (e: unknown) => {
//|             if (isOnDefaultBranch(e)) throw defaultBranchRefused("bring back", entry.record.name, GitStatusReply.parse(await ask({ op: "git.status", cwd })).branch.head);
//|             const said = e instanceof Error ? e.message : String(e);
//|             if (entry.record.parentWorkspaceId !== undefined && isNoGitCredential(e)) await ctx.keepTree(entry, { ...entry.record.tree, pushRefused: said });
//|             throw e;
//|           }),
//|         );
//|         if (entry.record.tree?.pushRefused !== undefined) {
//|           const { pushRefused: _gone, ...rest } = entry.record.tree;
//|           await ctx.keepTree(entry, rest);
//|         }
//|         // A workspace a thread opened under a lead pushes its branch and opens nothing: the lead's own pull request is
//|         // where its work lands, which the lead merges its branch into.
//|         if (entry.record.parentThreadId !== undefined) {
//|           return { branch: push.branch, base: push.base, ahead: push.ahead, uncommitted: push.uncommitted, stat: push.stat, note: childPushedLine(push.branch) };
//|         }
//|         const asked = { op: "git.pr", cwd, ...against, ...(title !== undefined ? { title } : {}), ...(body !== undefined ? { body } : {}) };
//|         // The push has landed by here, so nothing the pull request half says makes this a failed bring back: the
//|         // branch is on the remote either way and the two halves are answered apart. A machine with no signed-in
//|         // command line for the host is the note it always was; any other refusal rides beside the push as its own,
//|         // which the verb above prints under the push lines and then exits on.
//|         const opened = await ask(asked).catch((e: unknown) => {
//|           const said = e instanceof Error ? e.message : String(e);
//|           return isNoHostCli(e) ? { note: said } : { refused: said };
//|         });
//|         const half = opened as { note?: string; refused?: string };
//|         const apart = half.note !== undefined ? { note: half.note } : half.refused !== undefined ? { refused: half.refused } : { pr: GitPrReply.parse(opened).pr };
//|         return { branch: push.branch, base: push.base, ahead: push.ahead, uncommitted: push.uncommitted, stat: push.stat, ...apart };
//|       }));
//|       if (brought.pr !== undefined) await ctx.takePullRequest(entry, { ...brought.pr, readAt: clock.now() });
//|       ctx.readLeadOf(entry);
//|       return brought;
//|     },
pub async fn bring_back(
    ctx: &Rc<dyn GitCtx>,
    workspace_id: &str,
    title: Option<String>,
    body: Option<String>,
    origin: Option<Caller>,
) -> Result<BringBackResult, HostError> {
    ctx.spawn_guard("bring_back", origin.as_ref())?;
    let entry = ctx.entry_of(workspace_id, origin, EntryAsk::default()).await?;
    ctx.copy_blocked(&entry).await?;
    let record = record_of(&entry);
    let cwd = ctx.checkout_of(&record);
    let base = match &record.base {
        Some(base) => Some(base.clone()),
        None => ctx.project_held(&record.project)?.base,
    };
    let fork = record.from.as_ref().and_then(|f| f.head.as_ref()).and_then(|h| h.fork.clone());
    if let (Some(from), Some(fork)) = (&record.from, &fork) {
        if from.kind == WorkspaceFromKind::PullRequest && !fork.pushable {
            return Err(HostError::new(wsp_words::start::start_words::fork_not_pushable(&fork.owner)));
        }
    }
    let body = match &record.from {
        Some(from) if from.kind == WorkspaceFromKind::Issue => Some(with_closes(body.as_deref().unwrap_or(""), from.number.0 .0)),
        _ => body,
    };
    let pr_title = title;
    let name = record.name.clone();
    let has_parent = record.parent_workspace_id.is_some();
    let child = record.parent_thread_id.is_some();
    let c = ctx.clone();
    let e = entry.clone();
    let id = record.id.clone();
    let queued_work: Queued<Value> = Box::pin(async move {
        let inner = c.clone();
        let entry_inner = e.clone();
        let work: DaemonWork = Box::new(move |ask: DaemonAsk| {
            async move {
                let push_ask = with_base(json!({ "op": "git.push", "cwd": cwd }), &base);
                let pushed = match ask(push_ask).await {
                    Ok(v) => v,
                    Err(err) => {
                        let refusal = refusal_of(&err);
                        if is_on_default_branch(&refusal) {
                            let status: GitStatusReply = read_as(ask(json!({ "op": "git.status", "cwd": cwd })).await?)?;
                            return Err(default_branch_refused(DefaultBranchRoad::BringBack, &name, &status.branch.head));
                        }
                        if has_parent && is_no_git_credential(&refusal) {
                            let mut tree = tree_or_empty(record_of(&entry_inner).tree);
                            tree.push_refused = Some(err.message.clone());
                            inner.keep_tree(&entry_inner, tree).await?;
                        }
                        return Err(err);
                    }
                };
                if let Some(mut tree) = record_of(&entry_inner).tree.filter(|t| t.push_refused.is_some()) {
                    tree.push_refused = None;
                    inner.keep_tree(&entry_inner, tree).await?;
                }
                let push: GitPushReply = read_as(pushed)?;
                if child {
                    let note = Some(child_pushed_line(&push.branch));
                    return Ok(json_of(&BringBackResult {
                        branch: push.branch,
                        base: push.base,
                        ahead: push.ahead,
                        uncommitted: push.uncommitted,
                        stat: push.stat,
                        pr: None,
                        note,
                        refused: None,
                    }));
                }
                let mut asked = with_base(json!({ "op": "git.pr", "cwd": cwd }), &base);
                if let Some(title) = &pr_title {
                    asked["title"] = json!(title);
                }
                if let Some(body) = &body {
                    asked["body"] = json!(body);
                }
                let (pr, note, refused) = match ask(asked).await {
                    Ok(v) => {
                        let reply: GitPrReply = read_as(v)?;
                        (Some(reply.pr), None, None)
                    }
                    Err(err) => {
                        if is_no_host_cli(&refusal_of(&err)) {
                            (None, Some(err.message), None)
                        } else {
                            (None, None, Some(err.message))
                        }
                    }
                };
                Ok(json_of(&BringBackResult {
                    branch: push.branch,
                    base: push.base,
                    ahead: push.ahead,
                    uncommitted: push.uncommitted,
                    stat: push.stat,
                    pr,
                    note,
                    refused,
                }))
            }
            .boxed_local()
        });
        c.with_daemon(&e, work).await
    });
    let value = ctx.queued(&id, queued_work).await?;
    let brought: BringBackResult = read_as(value)?;
    if let Some(pr) = brought.pr.clone() {
        let mut pr_json = json_of(&pr);
        pr_json["readAt"] = json!(ctx.clock().now());
        ctx.take_pull_request(&entry, pr_json).await?;
    }
    ctx.read_lead_of(&entry);
    Ok(brought)
}

// ts: workspaces.ts:748-752
//|     async folderFor(o, origin) {
//|       await ctx.ready();
//|       const at = await ctx.folderFor(o, origin);
//|       return { workspace: ctx.view(at.entry.record), ...(at.cwd !== undefined ? { cwd: at.cwd } : {}), ...(at.setup !== undefined ? { setup: at.setup } : {}) };
//|     },
pub async fn folder_for(ctx: &Rc<dyn GitCtx>, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
    ctx.ready().await?;
    let (entry, cwd, setup) = ctx.folder_for(ask, origin).await?;
    Ok(FolderFor { workspace: ctx.view(&record_of(&entry)), cwd, setup })
}

// ts: workspaces.ts:754-757
//|     async takeSetupLine(id, origin) {
//|       await ctx.ready();
//|       return ctx.takeSetupLine((await ctx.entryOf(id, origin)).record.id);
//|     },
pub async fn take_setup_line(ctx: &Rc<dyn GitCtx>, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
    ctx.ready().await?;
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    Ok(ctx.take_setup_line(&id_of(&entry)))
}

// ts: workspaces.ts:759-765
//|     async folder({ project: named }, origin) {
//|       await ctx.ready();
//|       const project = await ctx.projectsDoor.resolve(named, origin);
//|       if (!runsInFolder(ctx.kindOf(project.computer))) throw Object.assign(new Error(notOnThisComputerLine(project.name)), { kind: "usage" });
//|       ctx.refuseRecording(project.name, origin);
//|       return ctx.view((await ctx.projectFolder(project)).record);
//|     },
pub async fn folder(ctx: &Rc<dyn GitCtx>, project: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    ctx.ready().await?;
    let project = ctx.resolve_project(project, origin.clone()).await?;
    if !runs_in_folder(&ctx.kind_of(&project.computer)) {
        return Err(HostError::with_kind(not_on_this_computer_line(&project.name), Kind::Usage));
    }
    ctx.refuse_recording(&project.name, origin.as_ref())?;
    let entry = ctx.project_folder(&project).await?;
    Ok(ctx.view(&record_of(&entry)))
}

// ts: workspaces.ts:767-778
//|     async worktree({ project: named, branch }, origin) {
//|       await ctx.ready();
//|       const project = await ctx.projectsDoor.resolve(named, origin);
//|       const top = project.git?.top;
//|       const kind = ctx.kindOf(project.computer);
//|       if (!runsInFolder(kind)) throw Object.assign(new Error(notOnThisComputerLine(project.name)), { kind: "usage" });
//|       if (!copiesFolder(kind)) throw Object.assign(new Error(placeBranchLine(ctx.placeName(project.computer))), { kind: "usage" });
//|       if (top === undefined) throw Object.assign(new Error(noBranchesLine(project.name)), { kind: "usage" });
//|       const entry = await ctx.worktreeFolder(project, top, branch, undefined, scopeOf(origin)?.rootThreadId);
//|       const tree = entry.record.worktree;
//|       return { path: tree?.path ?? top, branch, made: tree?.made === true };
//|     },
pub async fn worktree(ctx: &Rc<dyn GitCtx>, project: &str, branch: &str, origin: Option<Caller>) -> Result<WorktreeMade, HostError> {
    ctx.ready().await?;
    let project = ctx.resolve_project(project, origin.clone()).await?;
    let top = project.git.as_ref().map(|g| g.top.clone());
    let kind = ctx.kind_of(&project.computer);
    if !runs_in_folder(&kind) {
        return Err(HostError::with_kind(not_on_this_computer_line(&project.name), Kind::Usage));
    }
    if !copies_folder(&kind) {
        return Err(HostError::with_kind(place_branch_line(&ctx.place_name(&project.computer)), Kind::Usage));
    }
    let Some(top) = top else {
        return Err(HostError::with_kind(no_branches_line(&project.name), Kind::Usage));
    };
    let root = scope_of(origin.as_ref()).map(|s| s.root_thread_id);
    let entry = ctx.worktree_folder(&project, &top, branch, root).await?;
    let tree = record_of(&entry).worktree;
    Ok(WorktreeMade {
        path: tree.as_ref().map(|t| t.path.clone()).unwrap_or(top),
        branch: branch.to_owned(),
        made: tree.as_ref().is_some_and(|t| t.made),
    })
}

// ts: workspaces.ts:780-794
//|     async worktreeRemove({ project: named, branch, force, check }, origin) {
//|       await ctx.ready();
//|       const project = await ctx.projectsDoor.resolve(named, origin);
//|       const top = project.git?.top;
//|       if (top === undefined) throw Object.assign(new Error(noBranchesLine(project.name)), { kind: "usage" });
//|       const holding = (await ctx.worktreesOf(top)).find(t => t.branch === branch)?.path;
//|       const entry = ctx.foldersOf(project.id).find(e => {
//|         const tree = e.record.worktree;
//|         return tree !== undefined && tree.gone !== true && (holding !== undefined ? tree.path === holding : tree.branch === branch);
//|       });
//|       if (entry !== undefined) ctx.refuseRelayed(entry.record, origin);
//|       if (entry?.record.worktree?.made !== true) throw Object.assign(new Error(notMadeWorktreeLine(branch)), { kind: "invalid" });
//|       if (force === true && scopeOf(origin) !== undefined) throw Object.assign(new Error(WORKTREE_FORCE_LINE), { kind: "usage" });
//|       await ctx.removeWorktree(entry, force === true, check === true ? { check: true } : {});
//|     },
pub async fn worktree_remove(ctx: &Rc<dyn GitCtx>, ask: WorktreeRemoveAsk, origin: Option<Caller>) -> Result<(), HostError> {
    ctx.ready().await?;
    let project = ctx.resolve_project(&ask.project, origin.clone()).await?;
    let Some(top) = project.git.as_ref().map(|g| g.top.clone()) else {
        return Err(HostError::with_kind(no_branches_line(&project.name), Kind::Usage));
    };
    let holding = ctx.worktrees_of(&top).await?.into_iter().find(|t| t.branch.as_deref() == Some(ask.branch.as_str())).map(|t| t.path);
    let entry = ctx.folders_of(&project.id).into_iter().find(|e| {
        record_of(e).worktree.is_some_and(|tree| {
            tree.gone.is_none()
                && match &holding {
                    Some(path) => tree.path == *path,
                    None => tree.branch.as_deref() == Some(ask.branch.as_str()),
                }
        })
    });
    if let Some(entry) = &entry {
        ctx.refuse_relayed(&record_of(entry), origin.as_ref())?;
    }
    let entry = match entry {
        Some(entry) if record_of(&entry).worktree.is_some_and(|t| t.made) => entry,
        _ => return Err(HostError::with_kind(not_made_worktree_line(&ask.branch), Kind::Invalid)),
    };
    let force = ask.force == Some(true);
    if force && scope_of(origin.as_ref()).is_some() {
        return Err(HostError::with_kind(WORKTREE_FORCE_LINE.to_owned(), Kind::Usage));
    }
    let check = ask.check == Some(true);
    ctx.remove_worktree(&entry, force, RemoveAsk { check, ..RemoveAsk::default() }).await
}

// ts: workspaces.ts:796-807
//|     async checkout(id, origin, fresh = false) {
//|       const entry = await ctx.entryOf(id, origin);
//|       const checkout = await ctx.readCheckout(entry, fresh);
//|       // A fresh ask is a composer reading the branch again on a timer; the pull request and the children are not its.
//|       if (!fresh) {
//|         // The tile asks as it mounts, and its word rides the status once the git host answers.
//|         void ctx.readPullRequest(entry, false);
//|         // A lead's thread opening reads its children again.
//|         if ([...live.values()].some(e => e.record.parentWorkspaceId === entry.record.id)) void ctx.readTree(entry);
//|       }
//|       return checkout === undefined ? {} : { checkout };
//|     },
pub async fn checkout(ctx: &Rc<dyn GitCtx>, id: &str, origin: Option<Caller>, fresh: bool) -> Result<CheckoutReply, HostError> {
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    let checkout = ctx.read_checkout(&entry, fresh).await?;
    if !fresh {
        let _ = ctx.read_pull_request(&entry, false, false).await;
        let own = record_of(&entry).id;
        let has_children = ctx.live().borrow().values().any(|e| record_of(e).parent_workspace_id.as_deref() == Some(own.as_str()));
        if has_children {
            let _ = ctx.read_tree(&entry).await;
        }
    }
    Ok(CheckoutReply { checkout })
}

// ts: workspaces.ts:809-820
//|     async discard({ workspaceId, path, check, threadId }, origin) {
//|       const entry = await ctx.entryOf(workspaceId, origin, threadId === undefined ? {} : { thread: threadId });
//|       await ctx.copyBlocked(entry);
//|       if (check === true) {
//|         const said = GitStatusReply.parse(await ctx.withDaemon(entry, ask => ask({ op: "git.status", cwd: ctx.checkoutOf(entry.record) })));
//|         if (said.editsUnread !== true && !said.entries.some(e => e.xy !== "!!" && e.path === path)) throw new Error(noChangeLine(path));
//|         return { path };
//|       }
//|       const put = GitDiscardReply.parse(await ctx.queued(entry.record.id, () => ctx.withDaemon(entry, ask => ask({ op: "git.discard", cwd: ctx.checkoutOf(entry.record), path }))));
//|       await ctx.readCheckout(entry, true);
//|       return put;
//|     },
pub async fn discard(ctx: &Rc<dyn GitCtx>, ask: DiscardAsk, origin: Option<Caller>) -> Result<GitDiscardReply, HostError> {
    let thread = ask.thread_id.clone();
    let entry = ctx.entry_of(&ask.workspace_id, origin, EntryAsk { thread: thread.clone(), ..EntryAsk::default() }).await?;
    ctx.copy_blocked(&entry).await?;
    let cwd = ctx.checkout_of(&record_of(&entry));
    if ask.check == Some(true) {
        let status: GitStatusReply = read_as(ask_daemon(ctx, &entry, json!({ "op": "git.status", "cwd": cwd })).await?)?;
        let unread = status.edits_unread;
        if !unread && !status.entries.iter().any(|e| e.xy != "!!" && e.path == ask.path) {
            return Err(HostError::new(no_change_line(&ask.path)));
        }
        return read_as(json!({ "path": ask.path }));
    }
    let path = ask.path.clone();
    let work: Queued<Value> = {
        let c = ctx.clone();
        let e = entry.clone();
        Box::pin(async move { ask_daemon(&c, &e, json!({ "op": "git.discard", "cwd": cwd, "path": path })).await })
    };
    let put: GitDiscardReply = read_as(ctx.queued(&record_of(&entry).id, work).await?)?;
    ctx.read_checkout(&entry, true).await?;
    Ok(put)
}

// ts: workspaces.ts:822-839
//|     async commit({ workspaceId, message, paths, threadId }, origin) {
//|       ctx.spawnGuard("commit", origin);
//|       const entry = await ctx.entryOf(workspaceId, origin, { act: "commit", ...(threadId !== undefined ? { thread: threadId } : {}) });
//|       await ctx.copyBlocked(entry);
//|       const cwd = ctx.checkoutOf(entry.record);
//|       const made = GitCommitReply.parse(
//|         await ctx.queued(entry.record.id, () =>
//|           ctx.withDaemon(entry, async ask => {
//|             // Every changed file, each untracked one on its own, as the Changes pane lists them.
//|             const named = paths ?? GitDiffReply.parse(await ask({ op: "git.diff", cwd, scope: "head" })).files.map(f => f.path);
//|             if (paths === undefined && named.length === 0) throw new Error(cleanCheckoutLine(labelOf(entry, threadId)));
//|             return ask({ op: "git.commit", cwd, message, paths: named });
//|           }),
//|         ),
//|       );
//|       await ctx.readCheckout(entry, true);
//|       return made;
//|     },
pub async fn commit(ctx: &Rc<dyn GitCtx>, ask: CommitAsk, origin: Option<Caller>) -> Result<GitCommitReply, HostError> {
    ctx.spawn_guard("commit", origin.as_ref())?;
    let thread = ask.thread_id.clone();
    let entry = ctx
        .entry_of(&ask.workspace_id, origin, EntryAsk { act: Some(AcrossAct::Commit), thread: thread.clone(), ..EntryAsk::default() })
        .await?;
    ctx.copy_blocked(&entry).await?;
    let cwd = ctx.checkout_of(&record_of(&entry));
    let label = ctx.label_of(&entry, thread.as_deref());
    let paths = ask.paths.clone();
    let message = ask.message.clone();
    let c = ctx.clone();
    let e = entry.clone();
    let work: Queued<Value> = Box::pin(async move {
        let work: DaemonWork = Box::new(move |ask: DaemonAsk| {
            async move {
                let named = match &paths {
                    Some(paths) => paths.clone(),
                    None => {
                        let diff: GitDiffReply = read_as(ask(json!({ "op": "git.diff", "cwd": cwd, "scope": "head" })).await?)?;
                        diff.files.iter().map(|f| f.path.clone()).collect()
                    }
                };
                if paths.is_none() && named.is_empty() {
                    return Err(HostError::new(clean_checkout_line(&label)));
                }
                ask(json!({ "op": "git.commit", "cwd": cwd, "message": message, "paths": named })).await
            }
            .boxed_local()
        });
        c.with_daemon(&e, work).await
    });
    let made: GitCommitReply = read_as(ctx.queued(&record_of(&entry).id, work).await?)?;
    ctx.read_checkout(&entry, true).await?;
    Ok(made)
}

// ts: workspaces.ts:841-879
//|     async commitDraft({ workspaceId, paths, threadId }, origin) {
//|       ctx.spawnGuard("commit", origin);
//|       if (paths !== undefined && paths.length === 0) return { message: null, note: DRAFT_NOTES.nothing };
//|       const entry = await ctx.entryOf(workspaceId, origin, { act: "commit" });
//|       await ctx.copyBlocked(entry);
//|       // The thread named drafts on its own agent and from the message it was opened with; with none named, the
//|       // default agent drafts from the diff alone, since every thread of a project folder shares its workspace.
//|       const rows = threadId === undefined ? [] : [...sessions.values()].map(s => s.view).filter(v => v.workspaceId === workspaceId && v.threadId === threadId).sort((a, b) => (a.startedAt ?? 0) - (b.startedAt ?? 0));
//|       if (threadId !== undefined && rows.length === 0) throw notFoundRefusal(threadElsewhereLine(threadId));
//|       const newest = rows.at(-1);
//|       // The row keeps the head of the opening; the whole of it rides the thread's first start while the transcript
//|       // still holds it, and that start's prompt begins with the head.
//|       const head = rows[0]?.prompt;
//|       const start = head === undefined || threadId === undefined || !openingCut(head) ? undefined : (await ctx.openThread(workspaceId, threadId)).find(e => e.type === "session.start" && e.prompt?.startsWith(head) === true);
//|       const opening = start?.type === "session.start" ? start.prompt : head;
//|       const named = newest?.harness ?? ctx.defaultAgentOf(await ctx.preferences.get(), entry);
//|       if (adapters[named] === undefined || ctx.agentOff(entry, named)) return { message: null, note: DRAFT_NOTES.noAgent };
//|       const { harness, adapter } = await ctx.launchAdapterFor(entry, named);
//|       if (adapter.draftFor === undefined) return { message: null, note: DRAFT_NOTES.noAgent };
//|       const diff = GitDiffReply.parse(await ctx.withDaemon(entry, ask => ask({ op: "git.diff", cwd: ctx.checkoutOf(entry.record), scope: "head", ...(paths !== undefined ? { paths } : {}) })));
//|       const table = harnessCatalog(harness);
//|       const model = smallestModel(table === undefined ? undefined : await ctx.catalogOn(table, entry, adapter));
//|       // The question rides a file on the machine, since a diff is longer than one exec may carry, and only the
//|       // login that wrote it may read it; it goes once the answer is in, whatever the answer was.
//|       const promptFile = `/tmp/wsp-draft-${randomBytes(6).toString("hex")}.txt`;
//|       const question = draftPrompt(cutDiff(diff.files.map(f => f.patch).join("\n")), opening);
//|       const put = await putFiles(entry.machine, [{ path: promptFile, text: question }], { before: ["umask 077"] });
//|       if (put.exitCode !== 0) return { message: null, note: DRAFT_NOTES.noAnswer };
//|       try {
//|         const answer = await adapter.draftFor(
//|           { promptFile, ...(model !== undefined ? { model } : {}) },
//|           harnessExec(entry.machine, TITLE_MAKE_TIMEOUT_MS),
//|         );
//|         const message = answer === null ? null : commitMessage(answer);
//|         return message === null ? { message: null, note: DRAFT_NOTES.noAnswer } : { message };
//|       } finally {
//|         await entry.machine.exec(`rm -f ${shellQuote(promptFile)}`).catch(() => undefined);
//|       }
//|     },
pub async fn commit_draft(ctx: &Rc<dyn GitCtx>, ask: CommitDraftAsk, origin: Option<Caller>) -> Result<CommitDraft, HostError> {
    ctx.spawn_guard("commit", origin.as_ref())?;
    if ask.paths.as_ref().is_some_and(|p| p.is_empty()) {
        return Ok(draft_empty(&draft_note("nothing")));
    }
    let entry = ctx.entry_of(&ask.workspace_id, origin, EntryAsk { act: Some(AcrossAct::Commit), ..EntryAsk::default() }).await?;
    ctx.copy_blocked(&entry).await?;
    let workspace_id = ask.workspace_id.clone();
    let thread = ask.thread_id.clone();
    let mut rows: Vec<SessionView> = match &thread {
        None => Vec::new(),
        Some(thread) => ctx
            .session_views()
            .into_iter()
            .filter(|v| v.workspace_id == workspace_id && v.thread_id.as_deref() == Some(thread.as_str()))
            .collect(),
    };
    rows.sort_by(|a, b| {
        let at = |v: &SessionView| v.started_at.as_ref().map_or(0.0, |n| n.0);
        at(a).partial_cmp(&at(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    if let (Some(thread), true) = (&thread, rows.is_empty()) {
        return Err(not_found_refusal(&thread_elsewhere_line(thread)));
    }
    let head = rows.first().and_then(|r| r.prompt.clone());
    let opening = match (&head, &thread) {
        (Some(head), Some(thread)) if opening_cut(head) => {
            let events = ctx.open_thread(&workspace_id, thread).await?;
            events
                .iter()
                .find(|e| e["type"] == "session.start" && e["prompt"].as_str().is_some_and(|p| p.starts_with(head.as_str())))
                .and_then(|e| e["prompt"].as_str().map(str::to_owned))
                .or_else(|| Some(head.clone()))
        }
        _ => head.clone(),
    };
    let named = match rows.last().map(|r| r.harness.clone()) {
        Some(harness) => harness,
        None => ctx.default_agent_of(&entry),
    };
    if !ctx.has_adapter(&named) || ctx.agent_off(&entry, &named) {
        return Ok(draft_empty(&draft_note("noAgent")));
    }
    let model = match ctx.drafter(&entry, &named).await? {
        Some(model) => model,
        None => return Ok(draft_empty(&draft_note("noAgent"))),
    };
    let cwd = ctx.checkout_of(&record_of(&entry));
    let paths = ask.paths.clone();
    let diff: GitDiffReply = {
        let mut op = json!({ "op": "git.diff", "cwd": cwd, "scope": "head" });
        if let Some(paths) = &paths {
            op["paths"] = json!(paths);
        }
        read_as(ask_daemon(ctx, &entry, op).await?)?
    };
    let patches: Vec<String> = diff.files.iter().map(|f| f.patch.clone()).collect();
    let question = draft_prompt(&cut_diff(&patches.join("\n"), None), opening.as_deref());
    let prompt_file = format!("/tmp/wsp-draft-{}.txt", hex::encode(ctx.ids().bytes(6)));
    let put = ctx.put_files(&entry, &prompt_file, &question).await?;
    if put.exit_code.0 .0 != 0.0 {
        return Ok(draft_empty(&draft_note("noAnswer")));
    }
    let answer = ctx.draft_for(&entry, &named, &prompt_file, model.as_deref()).await;
    let removed = ctx.machine_exec(&entry, &format!("rm -f {}", shell_quote(&prompt_file)), None).await;
    let _ = removed;
    let answer = answer?;
    let message = answer.as_deref().and_then(commit_message);
    match message {
        None => Ok(draft_empty(&draft_note("noAnswer"))),
        Some(message) => Ok(CommitDraft { message: Some(message), note: None }),
    }
}

// ts: workspaces.ts:881-891
//|     async viewed({ workspaceId, path, blob }, origin) {
//|       await ctx.entryOf(workspaceId, origin);
//|       const marks = { ...(ctx.viewedMarks.get(workspaceId) ?? {}) };
//|       if (path === undefined) return { viewed: marks };
//|       if (typeof blob === "string") marks[path] = blob;
//|       else delete marks[path];
//|       ctx.viewedMarks.set(workspaceId, marks);
//|       await ctx.persistSessions(workspaceId);
//|       bus.emit({ type: "workspace.viewed", workspaceId, viewed: marks });
//|       return { viewed: marks };
//|     },
pub async fn viewed(
    ctx: &Rc<dyn GitCtx>,
    workspace_id: &str,
    mark: Option<ViewedMark>,
    origin: Option<Caller>,
) -> Result<ViewedMarks, HostError> {
    let _entry = ctx.entry_of(workspace_id, origin, EntryAsk::default()).await?;
    let mut marks: indexmap::IndexMap<String, String> =
        ctx.viewed_marks().borrow().get(workspace_id).map(|m| m.viewed.clone()).unwrap_or_default();
    let Some(mark) = mark else {
        return Ok(ViewedMarks { viewed: marks });
    };
    match mark.blob {
        Some(blob) => {
            marks.insert(mark.path, blob);
        }
        None => {
            marks.shift_remove(&mark.path);
        }
    }
    let answer = ViewedMarks { viewed: marks };
    ctx.viewed_marks().borrow_mut().insert(workspace_id.to_owned(), answer.clone());
    ctx.persist_sessions(workspace_id).await?;
    ctx.emit(json!({ "type": "workspace.viewed", "workspaceId": workspace_id, "viewed": json_of(&answer.viewed) }))?;
    Ok(answer)
}

// ts: workspaces.ts:893-898
//|     async pullRequestView({ workspaceId, fresh }, origin) {
//|       const { entry, remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       const { page, here } = await ctx.readPage(entry, remote, number, fresh === true);
//|       const merge = await ctx.mergeSettings(remote).catch(() => undefined);
//|       return { ...page, ...(merge !== undefined ? { merge } : {}), sent: entry.record.prSent ?? [], postsAsYou: here };
//|     },
pub async fn pull_request_view(
    ctx: &Rc<dyn GitCtx>,
    workspace_id: &str,
    fresh: bool,
    origin: Option<Caller>,
) -> Result<PullRequestPage, HostError> {
    let on = ctx.pull_request_on(workspace_id, origin).await?;
    let (page, here) = ctx.read_page(&on.entry, &on.remote, on.number, fresh).await?;
    let merge = ctx.merge_settings(&on.remote).await.ok();
    let sent = record_of(&on.entry).pr_sent.unwrap_or_default();
    let mut value = json_of(&page);
    if let Some(merge) = merge {
        value["merge"] = merge;
    }
    value["sent"] = json_of(&sent);
    value["postsAsYou"] = json!(here);
    read_as(value)
}

// ts: workspaces.ts:900-903
//|     async pullRequestDiff({ workspaceId }, origin) {
//|       const { entry, remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       return ctx.readHost(entry, cwd => ({ op: "git.prDiff", cwd, remote, number, maxBytes: GIT_DIFF_CAP_BYTES }), r => GitPrDiffReply.parse(r));
//|     },
pub async fn pull_request_diff(ctx: &Rc<dyn GitCtx>, workspace_id: &str, origin: Option<Caller>) -> Result<GitPrDiffReply, HostError> {
    let on = ctx.pull_request_on(workspace_id, origin).await?;
    let cwd = ctx.checkout_of(&record_of(&on.entry));
    let value = ctx
        .read_host(
            &on.entry,
            json!({ "op": "git.prDiff", "cwd": cwd, "remote": on.remote, "number": on.number, "maxBytes": GIT_DIFF_CAP_BYTES }),
        )
        .await?;
    read_as(value)
}

// ts: workspaces.ts:905-910
//|     async pullRequestReply({ workspaceId, replyTo, threadId, body }, origin) {
//|       if (body.trim() === "") throw Object.assign(new Error(REPLY_EMPTY_LINE), { kind: "invalid" });
//|       const { remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       const where = { ...(replyTo !== undefined ? { replyTo } : {}), ...(threadId !== undefined ? { threadId } : {}) };
//|       return ctx.postAsPerson(remote, number, cwd => ({ op: "git.prReply", cwd, remote, number, ...where, body }), r => GitPrReplyReply.parse(r));
//|     },
pub async fn pull_request_reply(
    ctx: &Rc<dyn GitCtx>,
    ask: PullRequestReplyAsk,
    origin: Option<Caller>,
) -> Result<GitPrReplyReply, HostError> {
    if ask.body.trim().is_empty() {
        return Err(HostError::with_kind(REPLY_EMPTY_LINE.to_owned(), Kind::Invalid));
    }
    let on = ctx.pull_request_on(&ask.workspace_id, origin).await?;
    let mut op = json!({ "op": "git.prReply", "cwd": home_cwd(ctx), "remote": on.remote, "number": on.number, "body": ask.body });
    if let Some(reply_to) = ask.reply_to {
        op["replyTo"] = json!(reply_to);
    }
    if let Some(thread) = &ask.thread_id {
        op["threadId"] = json!(thread);
    }
    let value = ctx.post_as_person(&on.remote, on.number, op).await?;
    read_as(value)
}

// ts: workspaces.ts:912-915
//|     async pullRequestResolve({ workspaceId, threadId, resolved }, origin) {
//|       const { remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       return ctx.postAsPerson(remote, number, cwd => ({ op: "git.prResolve", cwd, remote, number, threadId, resolved }), r => GitPrResolveReply.parse(r));
//|     },
pub async fn pull_request_resolve(
    ctx: &Rc<dyn GitCtx>,
    workspace_id: &str,
    thread_id: &str,
    resolved: bool,
    origin: Option<Caller>,
) -> Result<GitPrResolveReply, HostError> {
    let on = ctx.pull_request_on(workspace_id, origin).await?;
    let op = json!({ "op": "git.prResolve", "cwd": home_cwd(ctx), "remote": on.remote, "number": on.number, "threadId": thread_id, "resolved": resolved });
    let value = ctx.post_as_person(&on.remote, on.number, op).await?;
    read_as(value)
}

// ts: workspaces.ts:917-920
//|     async pullRequestReact({ workspaceId, subject, content, on }, origin) {
//|       const { remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       return ctx.postAsPerson(remote, number, cwd => ({ op: "git.prReact", cwd, remote, number, subject, content, on }), r => GitPrReactReply.parse(r));
//|     },
pub async fn pull_request_react(
    ctx: &Rc<dyn GitCtx>,
    workspace_id: &str,
    subject: &str,
    content: ReactionContent,
    on: bool,
    origin: Option<Caller>,
) -> Result<GitPrReactReply, HostError> {
    let target = ctx.pull_request_on(workspace_id, origin).await?;
    let op = json!({ "op": "git.prReact", "cwd": home_cwd(ctx), "remote": target.remote, "number": target.number, "subject": subject, "content": json_of(&content), "on": on });
    let value = ctx.post_as_person(&target.remote, target.number, op).await?;
    read_as(value)
}

// ts: workspaces.ts:922-935
//|     async pullRequestSend({ workspaceId, items, threadId }, origin) {
//|       const { entry, remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       if (threadId !== undefined && ![...sessions.values()].some(s => s.view.workspaceId === workspaceId && s.view.threadId === threadId)) throw notFoundRefusal(threadElsewhereLine(threadId));
//|       await ctx.copyBlocked(entry);
//|       const { page } = await ctx.readPage(entry, remote, number, false);
//|       const same = (a: PullRequestItem, b: PullRequestItem): boolean => a.kind === b.kind && a.id === b.id;
//|       const asked = items.filter((item, n) => items.findIndex(other => same(item, other)) === n);
//|       const prompt = pullRequestSendPrompt(page, asked, number);
//|       const said = threadId === undefined ? await ctx.toFirstThread(workspaceId, prompt, origin) : await ctx.sendDetached(workspaceId, { prompt, thread: threadId }, origin);
//|       const at = clock.now();
//|       entry.record.prSent = [...(entry.record.prSent ?? []).filter(s => !asked.some(i => same(i, s))), ...asked.map(i => ({ kind: i.kind, id: i.id, at }))];
//|       await ctx.persist(entry.record);
//|       return { outcome: said.outcome, threadId: said.threadId, agent: said.harness, ...(said.capped !== undefined ? { capped: said.capped } : {}), sent: entry.record.prSent };
//|     },
pub async fn pull_request_send(
    ctx: &Rc<dyn GitCtx>,
    workspace_id: &str,
    items: Vec<PullRequestItem>,
    thread_id: Option<String>,
    origin: Option<Caller>,
) -> Result<PullRequestSendResult, HostError> {
    let on = ctx.pull_request_on(workspace_id, origin.clone()).await?;
    if let Some(thread) = &thread_id {
        if !thread_has_session(ctx, workspace_id, thread) {
            return Err(not_found_refusal(&thread_elsewhere_line(thread)));
        }
    }
    ctx.copy_blocked(&on.entry).await?;
    let (page, _) = ctx.read_page(&on.entry, &on.remote, on.number, false).await?;
    let values: Vec<Value> = items.iter().map(json_of).collect();
    let keep: Vec<usize> = (0..values.len()).filter(|n| values.iter().position(|o| same_item(&values[*n], o)) == Some(*n)).collect();
    let asked_values: Vec<Value> = keep.iter().map(|n| values[*n].clone()).collect();
    let page_items: Vec<PageItem> = asked_values
        .iter()
        .map(|item| Ok(PageItem { kind: read_as(item["kind"].clone())?, id: item["id"].as_f64().unwrap_or_default() }))
        .collect::<Result<_, HostError>>()?;
    let prompt = pull_request_send_prompt(&page.comments, &page.reviews, &page.review_comments, &page_items, on.number as f64)?;
    let said = match &thread_id {
        None => ctx.to_first_thread(workspace_id, &prompt, origin).await?,
        Some(thread) => ctx.send_detached(workspace_id, &prompt, thread, origin).await?,
    };
    let at = ctx.clock().now();
    let old: Vec<Value> = json_of(&record_of(&on.entry).pr_sent.unwrap_or_default()).as_array().cloned().unwrap_or_default();
    let mut next: Vec<Value> = old.into_iter().filter(|s| !asked_values.iter().any(|i| same_item(i, s))).collect();
    for item in &asked_values {
        next.push(json!({ "kind": item["kind"], "id": item["id"], "at": at }));
    }
    let sent: Vec<PullRequestSent> = read_as(Value::Array(next))?;
    on.entry.borrow().record.borrow_mut().pr_sent = Some(sent.clone());
    ctx.persist(record_of(&on.entry)).await?;
    read_as(without_nulls(json!({
        "outcome": said.outcome,
        "threadId": said.thread_id,
        "agent": said.harness,
        "capped": said.capped,
        "sent": json_of(&sent),
    })))
}

// ts: workspaces.ts:937-979
//|     async fix({ workspaceId, check, child, threadId, childThreadId }, origin) {
//|       ctx.spawnGuard("fix", origin);
//|       if (check !== undefined && child !== undefined) throw Object.assign(new Error(FIX_CHECK_OR_CHILD), { kind: "invalid" });
//|       const entry = await ctx.entryOf(workspaceId, origin, { act: "fix", ...(threadId !== undefined ? { thread: threadId } : {}) });
//|       if (threadId !== undefined && ![...sessions.values()].some(s => s.view.workspaceId === workspaceId && s.view.threadId === threadId)) throw notFoundRefusal(threadElsewhereLine(threadId));
//|       await ctx.copyBlocked(entry);
//|       const project = ctx.projectHeld(entry.record.project);
//|       let prompt: string;
//|       let base: string;
//|       if (child !== undefined) {
//|         // A merge that stopped goes to the lead's agent as a message, and nothing is merged here.
//|         const kid = await ctx.childOf(entry, child, origin, { ...(threadId !== undefined ? { lead: threadId } : {}), ...(childThreadId !== undefined ? { child: childThreadId } : {}) });
//|         const leadBranch = (await ctx.readCheckout(entry, false))?.branch ?? entry.record.base ?? project.base ?? project.defaultBranch;
//|         const childBranch = (await ctx.readCheckout(kid, false))?.branch ?? kid.record.worktree?.branch ?? "";
//|         // With no remote the child's branch is in its folder, which is where the merge took it from.
//|         const remote = remoteHost(project.remote) === undefined ? ctx.checkoutOf(kid.record) : "origin";
//|         prompt = mergeChildPrompt({ leadBranch, childBranch, remote, conflicts: kid.record.tree?.conflicts ?? [] });
//|         base = leadBranch;
//|       } else if (check === undefined) {
//|         const updated = await ctx.updateCopy(entry);
//|         base = updated.base;
//|         if (updated.merged) return { outcome: "updated", base };
//|         const branch = entry.checkout?.branch ?? (isPullRequestFact(entry.pr) ? entry.pr.branch : base);
//|         prompt = conflictsPrompt({ base, branch, files: updated.conflicts });
//|       } else {
//|         // Read whole: a check that failed since the last read moves nothing a lighter read compares.
//|         const fact = await ctx.readPullRequest(entry, true, true);
//|         if (!isPullRequestFact(fact)) throw new Error(noPullRequestRefusal(labelOf(entry, threadId)));
//|         const failed = fact.checks.find(c => c.name === check);
//|         if (failed === undefined) throw new Error(noSuchCheckRefusal(check, fact.checks.map(c => c.name)));
//|         if (failed.state !== "fail") throw new Error(checkNotFailedRefusal(check, failed.state));
//|         const run = failed.run;
//|         // A log the host no longer holds, or will not hand over, leaves the message with the check's link alone.
//|         const log =
//|           run === undefined
//|             ? undefined
//|             : await ctx.readHost(entry, cwd => ({ op: "git.runLog", cwd, remote: project.remote, runId: run.runId, jobId: run.jobId }), r => GitRunLogReply.parse(r)).catch(() => undefined);
//|         prompt = checkFailedPrompt({ check: failed, commit: { oid: fact.headOid, subject: fact.headSubject }, ...(log !== undefined ? { log } : {}) });
//|         base = fact.base;
//|       }
//|       const said = threadId === undefined ? await ctx.toFirstThread(workspaceId, prompt, origin) : await ctx.sendDetached(workspaceId, { prompt, thread: threadId }, origin);
//|       return { outcome: said.outcome, threadId: said.threadId, ...(check !== undefined ? { check } : {}), ...(child !== undefined ? { child } : {}), base, agent: said.harness, ...(said.capped !== undefined ? { capped: said.capped } : {}) };
//|     },
pub async fn fix(ctx: &Rc<dyn GitCtx>, ask: FixAsk, origin: Option<Caller>) -> Result<FixResult, HostError> {
    ctx.spawn_guard("fix", origin.as_ref())?;
    if ask.check.is_some() && ask.child.is_some() {
        return Err(HostError::with_kind(FIX_CHECK_OR_CHILD.to_owned(), Kind::Invalid));
    }
    let thread = ask.thread_id.clone();
    let entry = ctx
        .entry_of(&ask.workspace_id, origin.clone(), EntryAsk { act: Some(AcrossAct::Fix), thread: thread.clone(), ..EntryAsk::default() })
        .await?;
    if let Some(thread) = &thread {
        if !thread_has_session(ctx, &ask.workspace_id, thread) {
            return Err(not_found_refusal(&thread_elsewhere_line(thread)));
        }
    }
    ctx.copy_blocked(&entry).await?;
    let record = record_of(&entry);
    let project = ctx.project_held(&record.project)?;
    let (prompt, base) = if let Some(child) = ask.child.clone() {
        let kid = ctx.child_of(&entry, &child, origin.clone(), thread.as_deref(), ask.child_thread_id.as_deref()).await?;
        let lead_branch = match ctx.read_checkout(&entry, false).await?.map(|c| c.branch) {
            Some(branch) => branch,
            None => record.base.clone().or(project.base.clone()).unwrap_or(project.default_branch.clone()),
        };
        let kid_record = record_of(&kid);
        let child_branch = match ctx.read_checkout(&kid, false).await?.map(|c| c.branch) {
            Some(branch) => branch,
            None => kid_record.worktree.as_ref().and_then(|w| w.branch.clone()).unwrap_or_default(),
        };
        let remote =
            if wsp_catalog::git_hosts::remote_host(&project.remote).is_none() { ctx.checkout_of(&kid_record) } else { "origin".to_owned() };
        let conflicts = kid_record.tree.as_ref().and_then(|t| t.conflicts.clone()).unwrap_or_default();
        let prompt = merge_child_prompt(&MergeChild { lead_branch: lead_branch.clone(), child_branch, remote, conflicts });
        (prompt, lead_branch)
    } else if ask.check.is_none() {
        let updated = ctx.update_copy(&entry).await?;
        if updated.merged {
            return read_as(json!({ "outcome": "updated", "base": updated.base }));
        }
        let branch = match entry.borrow().checkout.as_ref().map(|c| c.branch.clone()) {
            Some(branch) => branch,
            None => match live_pr_fact(&entry).map(|f| f.branch) {
                Some(branch) => branch,
                None => updated.base.clone(),
            },
        };
        let files: Vec<&str> = updated.conflicts.iter().map(String::as_str).collect();
        (conflicts_prompt(&updated.base, &branch, &files), updated.base.clone())
    } else {
        let check_name = ask.check.clone().unwrap_or_default();
        let fact = ctx.read_pull_request(&entry, true, true).await?.map(read_as::<PullRequestSeen>).transpose()?;
        let fact = match fact {
            Some(PullRequestSeen::PullRequestFact(fact)) => fact,
            _ => return Err(HostError::new(no_pull_request_refusal(&ctx.label_of(&entry, thread.as_deref())))),
        };
        let names: Vec<String> = fact.checks.iter().map(|c| c.name.clone()).collect();
        let failed = match fact.checks.iter().find(|c| c.name == check_name) {
            Some(failed) => failed.clone(),
            None => return Err(HostError::new(no_such_check_refusal(&check_name, &names))),
        };
        if json_of(&failed.state) != json!("fail") {
            return Err(HostError::new(check_not_failed_refusal(&check_name, json_of(&failed.state).as_str().unwrap_or_default())));
        }
        let log: Option<CheckLog> = match &failed.run {
            None => None,
            Some(run) => {
                let cwd = ctx.checkout_of(&record);
                let value = ctx
                    .read_host(
                        &entry,
                        json!({ "op": "git.runLog", "cwd": cwd, "remote": project.remote, "runId": run.run_id, "jobId": run.job_id }),
                    )
                    .await;
                match value {
                    Ok(v) => read_as::<GitRunLogReply>(v).ok().map(|r| CheckLog { lines: r.lines, truncated: r.truncated }),
                    Err(_) => None,
                }
            }
        };
        let commit = CheckedCommit { oid: fact.head_oid.clone(), subject: Some(fact.head_subject.clone()) };
        (check_failed_prompt(&failed, &commit, log.as_ref()), fact.base.clone())
    };
    let said = match &thread {
        None => ctx.to_first_thread(&ask.workspace_id, &prompt, origin).await?,
        Some(thread) => ctx.send_detached(&ask.workspace_id, &prompt, thread, origin).await?,
    };
    read_as(without_nulls(json!({
        "outcome": said.outcome,
        "threadId": said.thread_id,
        "check": ask.check,
        "child": ask.child,
        "base": base,
        "agent": said.harness,
        "capped": said.capped,
    })))
}

// ts: workspaces.ts:981-1006
//|     async merge({ workspaceId, method, whenChecksPass, head, threadId }, origin) {
//|       ctx.spawnGuard("merge", origin);
//|       const entry = await ctx.entryOf(workspaceId, origin, threadId === undefined ? {} : { thread: threadId });
//|       // The fact the host holds is the one every window drew, however old it is: a fresh read here would hand the
//|       // guard below whatever the agent pushed since the person looked. Read only where nothing was ever read.
//|       const fact = isPullRequestFact(entry.pr) ? entry.pr : await ctx.readPullRequest(entry, false);
//|       if (!isPullRequestFact(fact)) {
//|         const kept = entry.record.pr;
//|         throw new Error(kept !== undefined && kept.state !== "open" ? notOpenRefusal(kept.number, kept.state) : noPullRequestRefusal(labelOf(entry, threadId)));
//|       }
//|       if (fact.state !== "open") throw new Error(notOpenRefusal(fact.number, fact.state));
//|       const remote = ctx.projectHeld(entry.record.project).remote;
//|       const settings = await ctx.mergeSettings(remote);
//|       const by = method ?? settings.defaultMethod;
//|       if (!settings.methods.includes(by)) throw new Error(mergeMethodRefusal(by, settings.methods));
//|       if (whenChecksPass === true && !settings.autoMerge) throw new Error(AUTO_MERGE_OFF_LINE);
//|       // Merged on this computer as the person, and only while the head is the commit the person was shown, which the
//|       // window that drew it names: a push between the drawing and the press fails the merge in the host's own words
//|       // rather than landing unseen code.
//|       const done = GitPrMergeReply.parse(
//|         await ctx.onThisComputer((ask, home) => ask({ op: "git.prMerge", cwd: home, remote, number: fact.number, method: by, auto: whenChecksPass === true, headOid: head ?? fact.headOid })),
//|       );
//|       githubCache.delete(ctx.pageKey(remote, fact.number));
//|       await ctx.readPullRequest(entry, true);
//|       return { number: fact.number, method: by, merged: done.merged, autoArmed: done.autoArmed };
//|     },
pub async fn merge(ctx: &Rc<dyn GitCtx>, ask: MergeAsk, origin: Option<Caller>) -> Result<MergeResult, HostError> {
    ctx.spawn_guard("merge", origin.as_ref())?;
    let entry = ctx.entry_of(&ask.workspace_id, origin.clone(), EntryAsk { thread: ask.thread_id.clone(), ..EntryAsk::default() }).await?;
    let held = live_pr_fact(&entry);
    let fact = match held {
        Some(fact) => fact,
        None => match ctx.read_pull_request(&entry, false, false).await?.map(read_as::<PullRequestSeen>).transpose()? {
            Some(PullRequestSeen::PullRequestFact(fact)) => fact,
            _ => {
                let kept = record_of(&entry).pr;
                let label = ctx.label_of(&entry, ask.thread_id.as_deref());
                return Err(HostError::new(match kept {
                    Some(kept) if json_of(&kept.state) != json!("open") => not_open_refusal(
                        kept.number.0 .0,
                        &serde_json::from_value(json_of(&kept.state)).map_err(|e| HostError::new(e.to_string()))?,
                    ),
                    _ => no_pull_request_refusal(&label),
                }));
            }
        },
    };
    if json_of(&fact.state) != json!("open") {
        return Err(HostError::new(not_open_refusal(fact.number.0 .0, &read_as(json_of(&fact.state))?)));
    }
    let remote = ctx.project_held(&record_of(&entry).project)?.remote;
    let settings: MergeSettings = read_as(ctx.merge_settings(&remote).await?)?;
    let by = ask.method.unwrap_or(settings.default_method);
    if !settings.methods.contains(&by) {
        return Err(HostError::new(merge_method_refusal(&by, &settings.methods)));
    }
    let auto = ask.when_checks_pass == Some(true);
    if auto && !settings.auto_merge {
        return Err(HostError::new(AUTO_MERGE_OFF_LINE.to_owned()));
    }
    let head = ask.head.clone().unwrap_or(fact.head_oid.clone());
    let number_json = fact.number.0 .0 as i64;
    let method = by;
    let host_remote = remote.clone();
    let done: GitPrMergeReply = read_as(
        ctx.on_this_computer(Box::new(move |ask: DaemonAsk, home: String| {
            async move { ask(json!({ "op": "git.prMerge", "cwd": home, "remote": host_remote, "number": number_json, "method": method, "auto": auto, "headOid": head })).await }.boxed_local()
        }))
        .await?,
    )?;
    ctx.drop_page_cache(&remote, number_json);
    let _ = ctx.read_pull_request(&entry, true, false).await;
    read_as(json!({ "number": number_json, "method": by, "merged": done.merged, "autoArmed": done.auto_armed }))
}

// ts: workspaces.ts:1008-1013
//|     async update({ workspaceId, threadId }, origin) {
//|       ctx.spawnGuard("update", origin);
//|       const entry = await ctx.entryOf(workspaceId, origin, { act: "update", ...(threadId !== undefined ? { thread: threadId } : {}) });
//|       await ctx.copyBlocked(entry);
//|       return ctx.updateCopy(entry);
//|     },
pub async fn update(
    ctx: &Rc<dyn GitCtx>,
    workspace_id: &str,
    thread_id: Option<String>,
    origin: Option<Caller>,
) -> Result<GitUpdateReply, HostError> {
    ctx.spawn_guard("update", origin.as_ref())?;
    let entry = ctx
        .entry_of(workspace_id, origin, EntryAsk { act: Some(AcrossAct::Update), thread: thread_id.clone(), ..EntryAsk::default() })
        .await?;
    ctx.copy_blocked(&entry).await?;
    ctx.update_copy(&entry).await
}

// ts: workspaces.ts:1015-1053
//|     async mergeIn({ workspaceId, child, threadId, childThreadId }, origin) {
//|       ctx.spawnGuard("merge_in", origin);
//|       const lead = await ctx.entryOf(workspaceId, origin, threadId === undefined ? {} : { thread: threadId });
//|       // A thread merges only into the workspace it runs on: a child merging into its lead is a merge nobody there asked.
//|       const asked = scopeOf(origin);
//|       if (asked !== undefined && asked.workspaceId !== lead.record.id) throw new Error(mergeIntoOwnRefusal(asked.threadId));
//|       const kid = await ctx.childOf(lead, child, origin, { ...(threadId !== undefined ? { lead: threadId } : {}), ...(childThreadId !== undefined ? { child: childThreadId } : {}) });
//|       // The lead's copy is its agent's working tree, and a merge under a turn is one that agent was never asked about;
//|       // the asking thread's own turn is the one a tool call runs inside, so it is the one turn left out.
//|       const busy = [...sessions.values()]
//|         .map(s => s.view)
//|         .filter(v => v.workspaceId === lead.record.id && v.status === "running" && v.threadId !== undefined && v.threadId !== asked?.threadId);
//|       if (busy.length > 0) throw new Error(leadBusyRefusal(labelOf(lead, threadId), busy.map(v => v.threadId!)));
//|       await ctx.copyBlocked(lead);
//|       // A sub-thread's branch is its record's, so a napping machine is not woken and a released one still merges.
//|       const sub = kid.record.sub;
//|       const branch = sub?.branch ?? (await ctx.readCheckout(kid, true))?.branch ?? kid.record.worktree?.branch ?? "";
//|       if (branch === "" || branch === DETACHED_HEAD) throw new Error(childOnNoBranchRefusal(labelOf(kid, childThreadId)));
//|       const project = ctx.projectHeld(lead.record.project);
//|       // Through the remote, the one place both copies always reach; with none, from the child's folder only where
//|       // both copies sit on this computer, since a copy on a box is visible to nothing but its own workspace.
//|       let from: string | undefined;
//|       if (remoteHost(project.remote) === undefined) {
//|         if (!isLocalWorkspace(lead.record) || !isLocalWorkspace(kid.record)) throw new Error(noRemoteForTreeLine(project.name));
//|         from = ctx.checkoutOf(kid.record);
//|       }
//|       const done = GitMergeInReply.parse(
//|         await ctx.queued(lead.record.id, () => ctx.withDaemon(lead, ask => ask({ op: "git.mergeIn", cwd: ctx.checkoutOf(lead.record), branch, ...(from !== undefined ? { from } : {}) }))),
//|       );
//|       const kept: TreeRecord = { ...kid.record.tree };
//|       if (done.merged) {
//|         delete kept.conflicts;
//|         if (done.commits > 0 && done.oid !== undefined) kept.merged = { oid: done.oid, at: clock.now(), ...(done.head !== undefined ? { head: done.head } : {}) };
//|       } else kept.conflicts = done.conflicts;
//|       await ctx.readCheckout(lead, true);
//|       await ctx.keepTree(kid, kept);
//|       const unpushed = sub?.result?.unpushed ?? 0;
//|       return { lead: lead.record.name, child: kid.record.name, branch, merged: done.merged, commits: done.commits, conflicts: done.conflicts, ...(unpushed > 0 ? { unpushed } : {}) };
//|     },
pub async fn merge_in(ctx: &Rc<dyn GitCtx>, ask: MergeInAsk, origin: Option<Caller>) -> Result<MergeInResult, HostError> {
    ctx.spawn_guard("merge_in", origin.as_ref())?;
    let lead = ctx.entry_of(&ask.workspace_id, origin.clone(), EntryAsk { thread: ask.thread_id.clone(), ..EntryAsk::default() }).await?;
    let lead_record = record_of(&lead);
    if let Some(scope) = scope_of(origin.as_ref()) {
        if scope.workspace_id != lead_record.id {
            return Err(HostError::new(merge_into_own_refusal(&scope.thread_id)));
        }
    }
    let kid = ctx.child_of(&lead, &ask.child, origin.clone(), ask.thread_id.as_deref(), ask.child_thread_id.as_deref()).await?;
    let busy: Vec<String> = ctx
        .session_views()
        .into_iter()
        .filter(|v| {
            v.workspace_id == lead_record.id
                && v.status == SessionStatus::Running
                && v.thread_id.is_some()
                && v.thread_id != scope_of(origin.as_ref()).map(|s| s.thread_id)
        })
        .filter_map(|v| v.thread_id)
        .collect();
    if !busy.is_empty() {
        let label = ctx.label_of(&lead, ask.thread_id.as_deref());
        return Err(HostError::new(lead_busy_refusal(&label, &busy)));
    }
    ctx.copy_blocked(&lead).await?;
    let kid_record = record_of(&kid);
    let sub_branch = kid_record.sub.as_ref().map(|s| s.branch.clone());
    let branch = match sub_branch {
        Some(branch) => branch,
        None => match ctx.read_checkout(&kid, true).await?.map(|c| c.branch) {
            Some(branch) => branch,
            None => kid_record.worktree.as_ref().and_then(|w| w.branch.clone()).unwrap_or_default(),
        },
    };
    if branch.is_empty() || branch == DETACHED_HEAD {
        let label = ctx.label_of(&kid, ask.child_thread_id.as_deref());
        return Err(HostError::new(child_on_no_branch_refusal(&label)));
    }
    let project = ctx.project_held(&lead_record.project)?;
    let mut from: Option<String> = None;
    if wsp_catalog::git_hosts::remote_host(&project.remote).is_none() {
        if !ctx.is_local_workspace(&lead_record) || !ctx.is_local_workspace(&kid_record) {
            return Err(HostError::new(no_remote_for_tree_line(&project.name)));
        }
        from = Some(ctx.checkout_of(&kid_record));
    }
    let mut op = json!({ "op": "git.mergeIn", "cwd": ctx.checkout_of(&lead_record), "branch": branch });
    if let Some(from) = &from {
        op["from"] = json!(from);
    }
    let lead_id = lead_record.id.clone();
    let c = ctx.clone();
    let l = lead.clone();
    let work: Queued<Value> = Box::pin(async move { ask_daemon(&c, &l, op).await });
    let done: MergeInReply = read_as(ctx.queued(&lead_id, work).await?)?;
    let mut kept: TreeRecord = tree_or_empty(kid_record.tree.clone());
    if done.merged {
        kept.conflicts = None;
        if done.commits > 0 {
            if let Some(oid) = &done.oid {
                kept.merged = Some(read_as(without_nulls(json!({ "oid": oid, "at": ctx.clock().now(), "head": done.head })))?);
            }
        }
    } else {
        kept.conflicts = Some(done.conflicts.clone());
    }
    ctx.read_checkout(&lead, true).await?;
    ctx.keep_tree(&kid, kept).await?;
    let unpushed = kid_record.sub.as_ref().and_then(|s| s.result.as_ref()).map(|r| r.unpushed.0 .0).unwrap_or(0.0);
    read_as(without_nulls(json!({
        "lead": lead_record.name,
        "child": kid_record.name,
        "branch": branch,
        "merged": done.merged,
        "commits": done.commits,
        "conflicts": done.conflicts,
        "unpushed": if unpushed > 0.0 { json!(unpushed) } else { Value::Null },
    })))
}
