// SPDX-License-Identifier: AGPL-3.0-only
//! Everything that runs in or reads a workspace's checkout: commands, bring back, the folder doors, the Changes pane,
//! the pull request page and its acts, fix, merge, update and merge in. workspaces.ts:647-1053 at 65f6b89c7;
//! spec/workspaces_git.md. Daemon ops go through `Base::with_daemon` and `on_this_computer`, whose `ask` a test records.
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::{
    CommitAsk, CommitDraftAsk, DiscardAsk, FixAsk, FolderFor, FolderForAsk, MergeAsk, MergeInAsk, PullRequestReplyAsk, ViewedMark,
    WorktreeRemoveAsk,
};
use wsp_core::types::wiring::{RunningExec, WorkspaceRecord};
use wsp_frames::{GitCommitReply, GitDiscardReply};
use wsp_wire::types::bring_back::BringBackResult;
use wsp_wire::types::changes::{CheckoutReply, CommitDraft, ViewedMarks};
use wsp_wire::types::pull_request::{
    FixResult, GitPrReactReply, GitPrReplyReply, GitPrResolveReply, GitUpdateReply, MergeResult, PullRequestItem, PullRequestPage,
    PullRequestSendResult, ReactionContent,
};
use wsp_wire::types::start::GitPrDiffReply;
use wsp_wire::types::tree::{MergeInResult, TreeRecord};
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_more::{WorkspaceView, WorktreeMade};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use super::seam::{Base, Entry, Queued};
use crate::projects::folders::tree::{ListedWorktree, RemoveAsk};

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
    async fn child_of(&self, lead: &Entry, child: &str, origin: Option<Caller>, lead_thread: Option<&str>, child_thread: Option<&str>) -> Result<Entry, HostError>;
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
pub async fn exec(ctx: &Rc<dyn GitCtx>, id: &str, cmd: &str, timeout_ms: Option<u64>, origin: Option<Caller>) -> Result<ExecResult, HostError> {
    todo!()
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
pub async fn exec_stream(ctx: &Rc<dyn GitCtx>, id: &str, argv: Vec<String>, cwd: Option<String>, origin: Option<Caller>) -> Result<RunningExec, HostError> {
    todo!()
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
pub async fn bring_back(ctx: &Rc<dyn GitCtx>, workspace_id: &str, title: Option<String>, body: Option<String>, origin: Option<Caller>) -> Result<BringBackResult, HostError> {
    todo!()
}

// ts: workspaces.ts:748-752
//|     async folderFor(o, origin) {
//|       await ctx.ready();
//|       const at = await ctx.folderFor(o, origin);
//|       return { workspace: ctx.view(at.entry.record), ...(at.cwd !== undefined ? { cwd: at.cwd } : {}), ...(at.setup !== undefined ? { setup: at.setup } : {}) };
//|     },
pub async fn folder_for(ctx: &Rc<dyn GitCtx>, ask: FolderForAsk, origin: Option<Caller>) -> Result<FolderFor, HostError> {
    todo!()
}

// ts: workspaces.ts:754-757
//|     async takeSetupLine(id, origin) {
//|       await ctx.ready();
//|       return ctx.takeSetupLine((await ctx.entryOf(id, origin)).record.id);
//|     },
pub async fn take_setup_line(ctx: &Rc<dyn GitCtx>, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
    todo!()
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
pub async fn viewed(ctx: &Rc<dyn GitCtx>, workspace_id: &str, mark: Option<ViewedMark>, origin: Option<Caller>) -> Result<ViewedMarks, HostError> {
    todo!()
}

// ts: workspaces.ts:893-898
//|     async pullRequestView({ workspaceId, fresh }, origin) {
//|       const { entry, remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       const { page, here } = await ctx.readPage(entry, remote, number, fresh === true);
//|       const merge = await ctx.mergeSettings(remote).catch(() => undefined);
//|       return { ...page, ...(merge !== undefined ? { merge } : {}), sent: entry.record.prSent ?? [], postsAsYou: here };
//|     },
pub async fn pull_request_view(ctx: &Rc<dyn GitCtx>, workspace_id: &str, fresh: bool, origin: Option<Caller>) -> Result<PullRequestPage, HostError> {
    todo!()
}

// ts: workspaces.ts:900-903
//|     async pullRequestDiff({ workspaceId }, origin) {
//|       const { entry, remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       return ctx.readHost(entry, cwd => ({ op: "git.prDiff", cwd, remote, number, maxBytes: GIT_DIFF_CAP_BYTES }), r => GitPrDiffReply.parse(r));
//|     },
pub async fn pull_request_diff(ctx: &Rc<dyn GitCtx>, workspace_id: &str, origin: Option<Caller>) -> Result<GitPrDiffReply, HostError> {
    todo!()
}

// ts: workspaces.ts:905-910
//|     async pullRequestReply({ workspaceId, replyTo, threadId, body }, origin) {
//|       if (body.trim() === "") throw Object.assign(new Error(REPLY_EMPTY_LINE), { kind: "invalid" });
//|       const { remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       const where = { ...(replyTo !== undefined ? { replyTo } : {}), ...(threadId !== undefined ? { threadId } : {}) };
//|       return ctx.postAsPerson(remote, number, cwd => ({ op: "git.prReply", cwd, remote, number, ...where, body }), r => GitPrReplyReply.parse(r));
//|     },
pub async fn pull_request_reply(ctx: &Rc<dyn GitCtx>, ask: PullRequestReplyAsk, origin: Option<Caller>) -> Result<GitPrReplyReply, HostError> {
    todo!()
}

// ts: workspaces.ts:912-915
//|     async pullRequestResolve({ workspaceId, threadId, resolved }, origin) {
//|       const { remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       return ctx.postAsPerson(remote, number, cwd => ({ op: "git.prResolve", cwd, remote, number, threadId, resolved }), r => GitPrResolveReply.parse(r));
//|     },
pub async fn pull_request_resolve(ctx: &Rc<dyn GitCtx>, workspace_id: &str, thread_id: &str, resolved: bool, origin: Option<Caller>) -> Result<GitPrResolveReply, HostError> {
    todo!()
}

// ts: workspaces.ts:917-920
//|     async pullRequestReact({ workspaceId, subject, content, on }, origin) {
//|       const { remote, number } = await ctx.pullRequestOn(workspaceId, origin);
//|       return ctx.postAsPerson(remote, number, cwd => ({ op: "git.prReact", cwd, remote, number, subject, content, on }), r => GitPrReactReply.parse(r));
//|     },
pub async fn pull_request_react(ctx: &Rc<dyn GitCtx>, workspace_id: &str, subject: &str, content: ReactionContent, on: bool, origin: Option<Caller>) -> Result<GitPrReactReply, HostError> {
    todo!()
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
pub async fn pull_request_send(ctx: &Rc<dyn GitCtx>, workspace_id: &str, items: Vec<PullRequestItem>, thread_id: Option<String>, origin: Option<Caller>) -> Result<PullRequestSendResult, HostError> {
    todo!()
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
    todo!()
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
    todo!()
}

// ts: workspaces.ts:1008-1013
//|     async update({ workspaceId, threadId }, origin) {
//|       ctx.spawnGuard("update", origin);
//|       const entry = await ctx.entryOf(workspaceId, origin, { act: "update", ...(threadId !== undefined ? { thread: threadId } : {}) });
//|       await ctx.copyBlocked(entry);
//|       return ctx.updateCopy(entry);
//|     },
pub async fn update(ctx: &Rc<dyn GitCtx>, workspace_id: &str, thread_id: Option<String>, origin: Option<Caller>) -> Result<GitUpdateReply, HostError> {
    todo!()
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
    todo!()
}
