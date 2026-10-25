// SPDX-License-Identifier: AGPL-3.0-only
//! The start and review doors, the review draft and its post, the channels, port reach and probe, and the event
//! filters. workspaces.ts:1055-1247 at 65f6b89c7 (`creating` is create.rs'); spec/workspaces_review.md.
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::{DaemonEventListener, ReviewAsk, ReviewDraftAsk, StartFromLinkAsk};
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::pending::DaemonChannel;
use wsp_core::types::wiring::{Detach, SysListener, WorkspaceRecord};
use wsp_wire::types::pull_request::PullRequest;
use wsp_wire::types::start::{IssueRead, ReviewDraft, WorkspaceFrom, WorkspaceFromKind};
use wsp_wire::views::harness::{ReviewPostResult, StartResult};
use wsp_wire::views::workspace::{Caller, DaemonReachView, PortProbeView, PortReachView, ProjectView};
use wsp_words::exit::HostError;

use super::seam::{Base, Entry};
use crate::projects::start_from::{OpenAsk, Placed, StartKind};

/// What the start and review half reads off the runtime context, past `Base`. The start pieces are start_from's.
#[async_trait(?Send)]
pub trait ReviewCtx: Base {
    fn project_by_repo(&self, repo: &str, named: Option<&str>) -> Result<ProjectView, HostError>;
    async fn issue_of(&self, remote: &str, number: i64) -> Result<IssueRead, HostError>;
    async fn pull_request_of(&self, remote: &str, number: i64) -> Result<PullRequest, HostError>;
    fn from_of(&self, kind: WorkspaceFromKind, repo: &str, read: &IssueRead, fact: Option<&PullRequest>) -> WorkspaceFrom;
    async fn picks_hold(&self, project: &ProjectView, picks: &StartPicksAsked) -> Result<(), HostError>;
    async fn workspace_from(&self, project: &ProjectView, from: WorkspaceFrom, fact: Option<PullRequest>, kind: StartKind, origin: Option<Caller>) -> Result<Placed, HostError>;
    async fn open_with(&self, placed: &Placed, o: OpenAsk, origin: Option<Caller>) -> Result<StartResult, HostError>;
    fn read_only_of(&self, agent: &str) -> Option<String>;
    fn reviewers(&self) -> Vec<String>;
    fn drop_page_cache(&self, remote: &str, number: i64);
    /// `void ctx.readPullRequest(entry, true)`: started, not awaited.
    fn reread_pull_request(&self, entry: &Entry);
    fn label_of(&self, entry: &Entry, thread_id: Option<&str>) -> String;
    async fn daemon_road(&self, entry: &Entry) -> Result<DaemonReachView, HostError>;
    fn forget_daemon_token(&self, machine_id: &str);
    /// The kind's daemon is shared (`moduleOf(kind).sharedDaemon`).
    fn shared_daemon(&self, entry: &Entry) -> bool;
    /// The plain channel: `threadTagged(id, copyChannel(entry, onEvent, WORKSPACE_FRAMES))`.
    async fn copy_channel(&self, entry: &Entry, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError>;
    /// The shared daemon's channel wrapped as workspaces.ts:1160-1161 wraps it, over the heard events.
    async fn shared_channel(&self, entry: &Entry, heard: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError>;
    /// `ctx.portPids.get(id)`, made once: pty ids to pids.
    fn port_pids(&self, workspace_id: &str) -> Rc<std::cell::RefCell<indexmap::IndexMap<String, i64>>>;
    fn port_roots_moved(&self, workspace_id: &str);
    async fn guest_road_channel(&self, entry: &Entry, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError>;
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String>;
    /// `readingRoad(kind, "metrics") === "host"`.
    fn metrics_on_host(&self, entry: &Entry) -> bool;
    fn machine_word(&self, entry: &Entry) -> String;
    fn sys_samples(&self, listener: SysListener) -> Option<Detach>;
    /// The kind's own `portReach(entry, port, {standing})`, none where the kind has none.
    async fn kind_port_reach(&self, entry: &Entry, port: u16, standing: bool) -> Option<Result<PortReachView, HostError>>;
    async fn ws_port_reach(&self, entry: &Entry, port: u16) -> Result<PortReachView, HostError>;
    async fn ws_remint_port_reach(&self, entry: &Entry, port: u16) -> Result<PortReachView, HostError>;
    /// `fetch(url, {redirect: "manual", signal: timeout(PORT_PROBE_TIMEOUT_MS)})` and `readBodyUpTo(res, cap)`.
    async fn probe(&self, url: &str) -> Result<PortProbeView, HostError>;
    fn refusal_for(&self, record: Option<&WorkspaceRecord>, origin: Option<&Caller>) -> Option<String>;
    fn reaches_row(&self, thread_id: &str, workspace_id: &str, origin: Option<&Caller>) -> bool;
}

// ts: workspaces.ts:1055-1067
//|     async start({ url, project: named, agent, model, effort, access }, origin) {
//|       ctx.spawnGuard("start", origin);
//|       await ctx.ready();
//|       const link = githubLinkOf(url);
//|       if (link === undefined) throw Object.assign(new Error(START_WORDS.notALink(url)), { kind: "invalid" });
//|       const project = ctx.projectByRepo(link.repo, named);
//|       const read = await ctx.issueOf(project.remote, link.number);
//|       const fact = link.kind === "pull_request" ? await ctx.pullRequestOf(project.remote, link.number) : undefined;
//|       const from = ctx.fromOf(link.kind, link.repo, read, fact);
//|       await ctx.picksHold(project, { ...(agent !== undefined ? { harness: agent } : {}), ...(model !== undefined ? { model } : {}), ...(effort !== undefined ? { effort } : {}), ...(access !== undefined ? { access } : {}) });
//|       const placed = await ctx.workspaceFrom(project, from, fact, "start", origin);
//|       return ctx.openWith(placed, { prompt: fromTaskPrompt(from, read), ...(agent !== undefined ? { harness: agent } : {}), ...(model !== undefined ? { model } : {}), ...(effort !== undefined ? { effort } : {}), ...(access !== undefined ? { access } : {}) }, origin);
//|     },
pub async fn start(ctx: &Rc<dyn ReviewCtx>, ask: StartFromLinkAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
    todo!()
}

// ts: workspaces.ts:1069-1102
//|     async review({ url, workspaceId, agent, model, effort }, origin) {
//|       ctx.spawnGuard("review", origin);
//|       await ctx.ready();
//|       let repo: string;
//|       let number: number;
//|       let project: ProjectView;
//|       if (url !== undefined) {
//|         const link = githubLinkOf(url);
//|         if (link === undefined) throw Object.assign(new Error(START_WORDS.notALink(url)), { kind: "invalid" });
//|         if (link.kind !== "pull_request") throw Object.assign(new Error(START_WORDS.notAPullRequest), { kind: "invalid" });
//|         project = ctx.projectByRepo(link.repo, undefined);
//|         ({ repo, number } = link);
//|       } else {
//|         const entry = workspaceId === undefined ? undefined : await ctx.entryOf(workspaceId, origin);
//|         const kept = entry?.record.pr ?? (entry?.record.from?.kind !== "issue" ? entry?.record.from : undefined);
//|         if (entry === undefined || kept === undefined) throw Object.assign(new Error(START_WORDS.notAPullRequest), { kind: "invalid" });
//|         project = ctx.projectHeld(entry.record.project);
//|         repo = ownerRepoOf(project.remote) ?? project.remote;
//|         number = kept.number;
//|       }
//|       const reviewer = agent ?? "codex";
//|       const readOnly = ctx.readOnlyOf(reviewer);
//|       if (readOnly === undefined) throw Object.assign(new Error(START_WORDS.noReadOnly(reviewer, ctx.reviewers())), { kind: "invalid" });
//|       const fact = await ctx.pullRequestOf(project.remote, number);
//|       const read = await ctx.issueOf(project.remote, number);
//|       const diff = GitPrDiffReply.parse(await ctx.onThisComputer((ask, home) => ask({ op: "git.prDiff", cwd: home, remote: project.remote, number })));
//|       const from = ctx.fromOf("review", repo, read, fact);
//|       await ctx.picksHold(project, { harness: reviewer, ...(model !== undefined ? { model } : {}), ...(effort !== undefined ? { effort } : {}), permissionMode: readOnly });
//|       const placed = await ctx.workspaceFrom(project, from, fact, "review", origin);
//|       const opened = await ctx.openWith(placed, { prompt: reviewTaskPrompt(from, read, diff), harness: reviewer, ...(model !== undefined ? { model } : {}), ...(effort !== undefined ? { effort } : {}), permissionMode: readOnly }, origin);
//|       placed.entry.record.reviewThread = opened.threadId;
//|       await ctx.persist(placed.entry.record);
//|       return opened;
//|     },
pub async fn review(ctx: &Rc<dyn ReviewCtx>, ask: ReviewAsk, origin: Option<Caller>) -> Result<StartResult, HostError> {
    todo!()
}

// ts: workspaces.ts:1104-1119
//|     async reviewDraft({ workspaceId, summary, verdict, on }, origin) {
//|       const entry = await ctx.entryOf(workspaceId, origin);
//|       const draft = entry.record.review;
//|       if (isReviewRead(draft) && (summary !== undefined || verdict !== undefined || on !== undefined)) {
//|         const ticks = new Map((on ?? []).map(t => [t.id, t.on]));
//|         entry.record.review = {
//|           ...draft,
//|           ...(summary !== undefined ? { summary } : {}),
//|           ...(verdict !== undefined ? { verdict } : {}),
//|           comments: draft.comments.map(c => (ticks.has(c.id) ? { ...c, on: ticks.get(c.id)! } : c)),
//|         };
//|         await ctx.persist(entry.record);
//|         bus.emit({ type: "workspace.review", workspaceId });
//|       }
//|       return entry.record.review === undefined ? {} : { review: entry.record.review };
//|     },
pub async fn review_draft(ctx: &Rc<dyn ReviewCtx>, ask: ReviewDraftAsk, origin: Option<Caller>) -> Result<Option<ReviewDraft>, HostError> {
    todo!()
}

// ts: workspaces.ts:1121-1140
//|     async reviewPost({ workspaceId, threadId }, origin) {
//|       ctx.spawnGuard("review_post", origin);
//|       const entry = await ctx.entryOf(workspaceId, origin, threadId === undefined ? {} : { thread: threadId });
//|       const draft = entry.record.review;
//|       const from = entry.record.from;
//|       if (!isReviewRead(draft) || from === undefined) throw new Error(START_WORDS.noReviewYet(labelOf(entry, threadId)));
//|       const ticked = draft.comments.filter(c => c.on).map(({ id, path, line, side, body }) => ({ id, path, line, side, body }));
//|       const remote = ctx.projectHeld(entry.record.project).remote;
//|       // One call on this computer as the person, pinned to the head the review was written against, so a push since
//|       // is GitHub's to say and a half-made review is never seen.
//|       const done = GitPrReviewReply.parse(
//|         await ctx.onThisComputer((ask, home) => ask({ op: "git.prReview", cwd: home, remote, number: from.number, headOid: draft.headOid, event: draft.verdict, body: draft.summary, comments: ticked })),
//|       );
//|       entry.record.review = { ...draft, posted: { url: done.url, at: clock.now(), folded: done.folded } };
//|       await ctx.persist(entry.record);
//|       bus.emit({ type: "workspace.review", workspaceId });
//|       githubCache.delete(ctx.pageKey(remote, from.number));
//|       void ctx.readPullRequest(entry, true);
//|       return { url: done.url, number: from.number, comments: ticked.length - done.folded.length, folded: done.folded.length };
//|     },
pub async fn review_post(ctx: &Rc<dyn ReviewCtx>, workspace_id: &str, thread_id: Option<String>, origin: Option<Caller>) -> Result<ReviewPostResult, HostError> {
    todo!()
}

// ts: workspaces.ts:1142-1145
//|     async daemonReach(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       return ctx.moduleOf(entry.record.kind).daemonRoad(entry);
//|     },
pub async fn daemon_reach(ctx: &Rc<dyn ReviewCtx>, id: &str, origin: Option<Caller>) -> Result<DaemonReachView, HostError> {
    todo!()
}

// ts: workspaces.ts:1147-1149
//|     async tokenRefused(id) {
//|       ctx.daemonTokens.delete((await ctx.entryOf(id)).machine.id);
//|     },
pub async fn token_refused(ctx: &Rc<dyn ReviewCtx>, id: &str) -> Result<(), HostError> {
    todo!()
}

// ts: workspaces.ts:1151-1162
//|     async daemonChannel(id, onEvent, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       if (!ctx.moduleOf(entry.record.kind).sharedDaemon) return ctx.threadTagged(id, await ctx.copyChannel(entry, onEvent, ctx.WORKSPACE_FRAMES));
//|       const ptys = ctx.portPids.get(id) ?? new Map<string, number>();
//|       ctx.portPids.set(id, ptys);
//|       const heard = (event: Record<string, unknown>): void => {
//|         if (event["type"] === "pty.exit" && ptys.delete(String(event["ptyId"]))) ctx.portRootsMoved(id);
//|         onEvent(event);
//|       };
//|       const channel = ctx.rootedPorts(id, ctx.checkoutOf(entry.record), ptys, ctx.heldToOwner(id, ctx.threadTagged(id, await ctx.copyChannel(entry, heard, ctx.WORKSPACE_FRAMES))));
//|       return ctx.killGuarded(ctx.scopedProcs(ctx.scopesHere, channel), isLocalWorkspace(entry.record));
//|     },
pub async fn daemon_channel(ctx: &Rc<dyn ReviewCtx>, id: &str, on_event: DaemonEventListener, origin: Option<Caller>) -> Result<Rc<dyn DaemonChannel>, HostError> {
    todo!()
}

// ts: workspaces.ts:1164-1166
//|     async guestChannel(id, onEvent) {
//|       return ctx.copyChannel(await ctx.entryOf(id), onEvent, ctx.GUEST_ROAD_FRAMES);
//|     },
pub async fn guest_channel(ctx: &Rc<dyn ReviewCtx>, id: &str, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
    todo!()
}

// ts: workspaces.ts:1168-1170
//|     async servedByItsComputer(id, origin) {
//|       return ctx.servedByItsComputer(await ctx.entryOf(id, origin)) !== undefined;
//|     },
pub async fn served_by_its_computer(ctx: &Rc<dyn ReviewCtx>, id: &str, origin: Option<Caller>) -> Result<bool, HostError> {
    todo!()
}

// ts: workspaces.ts:1172-1178
//|     async watchSys(id, fn, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       const kind = entry.record.kind;
//|       if (readingRoad(kind, "metrics") !== "host") throw new Error(`${machineWord(kind)} reads its own load over its daemon link, not from this host`);
//|       if (local?.sysSamples === undefined) throw new Error("this host reads nothing of the computer it runs on");
//|       return local.sysSamples(fn);
//|     },
pub async fn watch_sys(ctx: &Rc<dyn ReviewCtx>, id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
    todo!()
}

// ts: workspaces.ts:1180-1186
//|     async portReach(id, port, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       await ctx.copyBlocked(entry);
//|       const own = ctx.moduleOf(entry.record.kind).portReach;
//|       const reach = own !== undefined ? await own(entry, port) : await entry.ws.portReach(port);
//|       return { url: reach.url, expiresAt: reach.expiresAt };
//|     },
pub async fn port_reach(ctx: &Rc<dyn ReviewCtx>, id: &str, port: u16, origin: Option<Caller>) -> Result<PortReachView, HostError> {
    todo!()
}

// ts: workspaces.ts:1188-1204
//|     async portProbe(id, port, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       await ctx.copyBlocked(entry);
//|       const own = ctx.moduleOf(entry.record.kind).portReach;
//|       if (own !== undefined) {
//|         // The pane repeats this fetch while the port is off the folder's list, so it must not open a forward the
//|         // person stopped in Ports, nor take the pane's word that it was stopped.
//|         const res = await fetch((await own(entry, port, { standing: true })).url, { redirect: "manual", signal: AbortSignal.timeout(PORT_PROBE_TIMEOUT_MS) });
//|         return { status: res.status, body: await readBodyUpTo(res, PORT_PROBE_BODY_CAP) };
//|       }
//|       const reach = await entry.ws.portReach(port);
//|       // A followed redirect would refetch without the token or the edge's cookies and report the edge's 401 for a page the frame loads fine.
//|       const res = await fetch(reach.url, { redirect: "manual", signal: AbortSignal.timeout(PORT_PROBE_TIMEOUT_MS) });
//|       const body = await readBodyUpTo(res, PORT_PROBE_BODY_CAP);
//|       if (res.status === 401) await entry.ws.remintPortReach(port);
//|       return { status: res.status, body };
//|     },
pub async fn port_probe(ctx: &Rc<dyn ReviewCtx>, id: &str, port: u16, origin: Option<Caller>) -> Result<PortProbeView, HostError> {
    todo!()
}

// ts: workspaces.ts:1206-1209
//|     async originRefusal(id, origin) {
//|       await ctx.ready();
//|       return ctx.refusalFor(live.get(id)?.record, origin);
//|     },
pub async fn origin_refusal(ctx: &Rc<dyn ReviewCtx>, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
    todo!()
}

// ts: workspaces.ts:1217-1231
//|     seenBy(event, origin) {
//|       const scope = scopeOf(origin);
//|       if (scope === undefined) return true;
//|       const asked = askerOf(event);
//|       if (asked !== undefined) return asked.threadId === scope.threadId || asked.rootThreadId === scope.rootThreadId;
//|       const id = workspaceIdOf(event);
//|       const record = id === undefined ? undefined : live.get(id)?.record;
//|       if (record === undefined) return false;
//|       // An event about a thread is that thread's tree's, whatever workspace it names: the stream shows exactly what
//|       // the listing and the transcript show, a lead's turn to the child on its copy included, and hides the rest,
//|       // so a row cannot be read going by. An event with no thread is about the workspace and reads its rule.
//|       const thread = (event as { threadId?: unknown }).threadId;
//|       if (typeof thread === "string") return ctx.reachesRow({ threadId: thread, workspaceId: record.id }, origin);
//|       return ctx.refusalFor(record, origin) === undefined;
//|     },
/// Whether a thread's socket sees an event; synchronous.
pub fn seen_by(ctx: &Rc<dyn ReviewCtx>, event: &serde_json::Value, origin: Option<&Caller>) -> bool {
    todo!()
}

// ts: workspaces.ts:1233-1244
//|     shownTo(event, origin) {
//|       const said = event as { type?: unknown; status?: WorkspaceStatus };
//|       const tree = said.type === "workspace.status" ? said.status?.tree : undefined;
//|       if (tree === undefined || scopeOf(origin) === undefined) return event;
//|       const children = tree.children.filter(c => {
//|         const record = live.get(c.workspaceId)?.record;
//|         return record !== undefined && ctx.refusalFor(record, origin) === undefined;
//|       });
//|       if (children.length === tree.children.length) return event;
//|       const { tree: _all, ...status } = said.status!;
//|       return { ...event, status: children.length === 0 ? status : { ...status, tree: { ...tree, children } } };
//|     },
/// The event as a thread's socket is shown it: a status tree pruned to the children it may see.
pub fn shown_to(ctx: &Rc<dyn ReviewCtx>, event: serde_json::Value, origin: Option<&Caller>) -> serde_json::Value {
    todo!()
}
