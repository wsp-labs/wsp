// SPDX-License-Identifier: AGPL-3.0-only
//! The start and review doors, the review draft and its post, the channels, port reach and probe, and the event
//! filters. workspaces.ts:1055-1247 at 65f6b89c7 (`creating` is create.rs'); spec/workspaces_review.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_core::types::api::{DaemonEventListener, ReviewAsk, ReviewDraftAsk, StartFromLinkAsk};
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::pending::DaemonChannel;
use wsp_core::types::wiring::{Detach, SysListener, WorkspaceRecord};
use wsp_frames::GitPrReviewReply;
use wsp_wire::types::pull_request::PullRequest;
use wsp_wire::types::start::{GitPrDiffReply, GithubLink, GithubLinkKind, IssueRead, ReviewDraft, WorkspaceFrom, WorkspaceFromKind};
use wsp_wire::views::harness::{ReviewPostResult, StartResult};
use wsp_wire::views::workspace::{scope_of, Caller, DaemonReachView, PortProbeView, PortReachView, ProjectView};
use wsp_wire::wire::runtime::{asker_of, workspace_id_of};
use wsp_words::exit::{HostError, Kind};
use wsp_words::start::{from_task_prompt, github_link_of, is_review_read, owner_repo_of, review_task_prompt, start_words};

use super::seam::{record_of, Base, DaemonAsk, Entry, EntryAsk};
use crate::projects::start_from::{OpenAsk, Placed, StartKind};

/// What the start and review half reads off the runtime context, past `Base`. The start pieces are start_from's.
#[async_trait(?Send)]
pub trait ReviewCtx: Base {
    fn project_by_repo(&self, repo: &str, named: Option<&str>) -> Result<ProjectView, HostError>;
    async fn issue_of(&self, remote: &str, number: i64) -> Result<IssueRead, HostError>;
    async fn pull_request_of(&self, remote: &str, number: i64) -> Result<PullRequest, HostError>;
    #[allow(clippy::wrong_self_convention)]
    fn from_of(&self, kind: WorkspaceFromKind, repo: &str, read: &IssueRead, fact: Option<&PullRequest>) -> WorkspaceFrom;
    async fn picks_hold(&self, project: &ProjectView, picks: &StartPicksAsked) -> Result<(), HostError>;
    async fn workspace_from(
        &self,
        project: &ProjectView,
        from: WorkspaceFrom,
        fact: Option<PullRequest>,
        kind: StartKind,
        origin: Option<Caller>,
    ) -> Result<Placed, HostError>;
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

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn read_as<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, HostError> {
    serde_json::from_value(v).map_err(|e| HostError::new(e.to_string()))
}

fn without_nulls(mut v: Value) -> Value {
    if let Value::Object(map) = &mut v {
        map.retain(|_, value| !value.is_null());
    }
    v
}

fn invalid(message: String) -> HostError {
    HostError::with_kind(message, Kind::Invalid)
}

/// The record of a live workspace, if this host holds it.
fn held_record(ctx: &Rc<dyn ReviewCtx>, id: &str) -> Option<WorkspaceRecord> {
    ctx.live().borrow().get(id).map(record_of)
}

/// A helper's lines are the TS's (`START_WORDS`), read off wsp-words.
fn start_link(url: &str) -> Result<GithubLink, HostError> {
    github_link_of(url).ok_or_else(|| invalid(start_words::not_a_link(url)))
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
    ctx.spawn_guard("start", origin.as_ref())?;
    ctx.ready().await?;
    let link = start_link(&ask.url)?;
    let number = link.number.0 .0 as i64;
    let project = ctx.project_by_repo(&link.repo, ask.project.as_deref())?;
    let read = ctx.issue_of(&project.remote, number).await?;
    let (kind, fact) = match link.kind {
        GithubLinkKind::PullRequest => (WorkspaceFromKind::PullRequest, Some(ctx.pull_request_of(&project.remote, number).await?)),
        GithubLinkKind::Issue => (WorkspaceFromKind::Issue, None),
    };
    let from = ctx.from_of(kind, &link.repo, &read, fact.as_ref());
    let picks = StartPicksAsked {
        harness: ask.agent.clone(),
        model: ask.model.clone(),
        effort: ask.effort.clone(),
        access: ask.access,
        permission_mode: None,
        fast: None,
    };
    ctx.picks_hold(&project, &picks).await?;
    let placed = ctx.workspace_from(&project, from.clone(), fact, StartKind::Start, origin.clone()).await?;
    let open = OpenAsk {
        prompt: from_task_prompt(&from, &read),
        harness: ask.agent,
        model: ask.model,
        effort: ask.effort,
        permission_mode: None,
        access: ask.access,
    };
    ctx.open_with(&placed, open, origin).await
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
    ctx.spawn_guard("review", origin.as_ref())?;
    ctx.ready().await?;
    let (repo, number, project) = if let Some(url) = &ask.url {
        let link = start_link(url)?;
        if link.kind != GithubLinkKind::PullRequest {
            return Err(invalid(start_words::NOT_A_PULL_REQUEST.to_owned()));
        }
        let project = ctx.project_by_repo(&link.repo, None)?;
        (link.repo.clone(), link.number.0 .0 as i64, project)
    } else {
        let entry = match &ask.workspace_id {
            None => None,
            Some(id) => Some(ctx.entry_of(id, origin.clone(), EntryAsk::default()).await?),
        };
        let Some(entry) = entry else {
            return Err(invalid(start_words::NOT_A_PULL_REQUEST.to_owned()));
        };
        let record = record_of(&entry);
        let kept_number = match (&record.pr, &record.from) {
            (Some(pr), _) => Some(pr.number.0 .0),
            (None, Some(from)) if from.kind != WorkspaceFromKind::Issue => Some(from.number.0 .0),
            _ => None,
        };
        let Some(kept_number) = kept_number else {
            return Err(invalid(start_words::NOT_A_PULL_REQUEST.to_owned()));
        };
        let project = ctx.project_held(&record.project)?;
        let repo = owner_repo_of(&project.remote).unwrap_or_else(|| project.remote.clone());
        (repo, kept_number as i64, project)
    };
    let reviewer = ask.agent.clone().unwrap_or_else(|| "codex".to_owned());
    let Some(read_only) = ctx.read_only_of(&reviewer) else {
        let others = ctx.reviewers();
        let others: Vec<&str> = others.iter().map(String::as_str).collect();
        return Err(invalid(start_words::no_read_only(&reviewer, &others)));
    };
    let fact = ctx.pull_request_of(&project.remote, number).await?;
    let read = ctx.issue_of(&project.remote, number).await?;
    let remote = project.remote.clone();
    let diff_value = ctx
        .on_this_computer(Box::new(move |ask: DaemonAsk, home: String| {
            async move { ask(json!({ "op": "git.prDiff", "cwd": home, "remote": remote, "number": number })).await }.boxed_local()
        }))
        .await?;
    let diff: GitPrDiffReply = read_as(diff_value)?;
    let from = ctx.from_of(WorkspaceFromKind::Review, &repo, &read, Some(&fact));
    let picks = StartPicksAsked {
        harness: Some(reviewer.clone()),
        model: ask.model.clone(),
        effort: ask.effort.clone(),
        access: None,
        permission_mode: Some(read_only.clone()),
        fast: None,
    };
    ctx.picks_hold(&project, &picks).await?;
    let placed = ctx.workspace_from(&project, from.clone(), Some(fact), StartKind::Review, origin.clone()).await?;
    let open = OpenAsk {
        prompt: review_task_prompt(&from, &read, &diff),
        harness: Some(reviewer),
        model: ask.model,
        effort: ask.effort,
        permission_mode: Some(read_only),
        access: None,
    };
    let opened = ctx.open_with(&placed, open, origin).await?;
    placed.entry.borrow().record.borrow_mut().review_thread = Some(opened.thread_id.clone());
    ctx.persist(record_of(&placed.entry)).await?;
    Ok(opened)
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
    let entry = ctx.entry_of(&ask.workspace_id, origin, EntryAsk::default()).await?;
    let record = record_of(&entry);
    let draft = record.review.clone();
    if is_review_read(draft.as_ref()) && (ask.summary.is_some() || ask.verdict.is_some() || ask.on.is_some()) {
        let ticks: Vec<(String, bool)> = ask.on.clone().unwrap_or_default();
        let mut value = json_of(&draft);
        if let Some(summary) = &ask.summary {
            value["summary"] = json!(summary);
        }
        if let Some(verdict) = &ask.verdict {
            value["verdict"] = json_of(verdict);
        }
        if let Some(comments) = value["comments"].as_array_mut() {
            for comment in comments.iter_mut() {
                let id = comment["id"].as_str().unwrap_or_default().to_owned();
                if let Some((_, on)) = ticks.iter().find(|(tick, _)| *tick == id) {
                    comment["on"] = json!(on);
                }
            }
        }
        let next: ReviewDraft = read_as(value)?;
        entry.borrow().record.borrow_mut().review = Some(next);
        ctx.persist(record_of(&entry)).await?;
        ctx.emit(json!({ "type": "workspace.review", "workspaceId": ask.workspace_id }))?;
    }
    Ok(record_of(&entry).review)
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
pub async fn review_post(
    ctx: &Rc<dyn ReviewCtx>,
    workspace_id: &str,
    thread_id: Option<String>,
    origin: Option<Caller>,
) -> Result<ReviewPostResult, HostError> {
    ctx.spawn_guard("review_post", origin.as_ref())?;
    let entry = ctx.entry_of(workspace_id, origin, EntryAsk { thread: thread_id.clone(), ..EntryAsk::default() }).await?;
    let record = record_of(&entry);
    let draft = record.review.clone();
    let Some(from) = record.from.clone() else {
        return Err(HostError::new(start_words::no_review_yet(&ctx.label_of(&entry, thread_id.as_deref()))));
    };
    if !is_review_read(draft.as_ref()) {
        return Err(HostError::new(start_words::no_review_yet(&ctx.label_of(&entry, thread_id.as_deref()))));
    }
    let draft_value = json_of(&draft);
    let ticked: Vec<Value> = draft_value["comments"]
        .as_array()
        .map(|comments| {
            comments
                .iter()
                .filter(|c| c["on"].as_bool() == Some(true))
                .map(|c| json!({ "id": c["id"], "path": c["path"], "line": c["line"], "side": c["side"], "body": c["body"] }))
                .collect()
        })
        .unwrap_or_default();
    let remote = ctx.project_held(&record.project)?.remote;
    let number = from.number.0 .0 as i64;
    let ask_remote = remote.clone();
    let head_oid = draft_value["headOid"].clone();
    let verdict = draft_value["verdict"].clone();
    let summary = draft_value["summary"].clone();
    let comments = ticked.clone();
    let done_value = ctx
        .on_this_computer(Box::new(move |ask: DaemonAsk, home: String| {
            async move {
                ask(json!({ "op": "git.prReview", "cwd": home, "remote": ask_remote, "number": number, "headOid": head_oid, "event": verdict, "body": summary, "comments": comments })).await
            }
            .boxed_local()
        }))
        .await?;
    let done: GitPrReviewReply = read_as(done_value)?;
    let at = ctx.clock().now();
    let mut next = draft_value;
    next["posted"] = json!({ "url": done.url, "at": at, "folded": done.folded });
    let next: ReviewDraft = read_as(next)?;
    entry.borrow().record.borrow_mut().review = Some(next);
    ctx.persist(record_of(&entry)).await?;
    ctx.emit(json!({ "type": "workspace.review", "workspaceId": workspace_id }))?;
    ctx.drop_page_cache(&remote, number);
    ctx.reread_pull_request(&entry);
    let folded = done.folded.len() as i64;
    read_as(without_nulls(json!({
        "url": done.url,
        "number": number,
        "comments": ticked.len() as i64 - folded,
        "folded": folded,
    })))
}

// ts: workspaces.ts:1142-1145
//|     async daemonReach(id, origin) {
//|       const entry = await ctx.entryOf(id, origin);
//|       return ctx.moduleOf(entry.record.kind).daemonRoad(entry);
//|     },
pub async fn daemon_reach(ctx: &Rc<dyn ReviewCtx>, id: &str, origin: Option<Caller>) -> Result<DaemonReachView, HostError> {
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    ctx.daemon_road(&entry).await
}

// ts: workspaces.ts:1147-1149
//|     async tokenRefused(id) {
//|       ctx.daemonTokens.delete((await ctx.entryOf(id)).machine.id);
//|     },
pub async fn token_refused(ctx: &Rc<dyn ReviewCtx>, id: &str) -> Result<(), HostError> {
    let entry = ctx.entry_of(id, None, EntryAsk::default()).await?;
    let machine = ctx.machine_id(&entry);
    ctx.forget_daemon_token(&machine);
    Ok(())
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
pub async fn daemon_channel(
    ctx: &Rc<dyn ReviewCtx>,
    id: &str,
    on_event: DaemonEventListener,
    origin: Option<Caller>,
) -> Result<Rc<dyn DaemonChannel>, HostError> {
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    if !ctx.shared_daemon(&entry) {
        return ctx.copy_channel(&entry, on_event).await;
    }
    let ptys = ctx.port_pids(id);
    let owner = id.to_owned();
    let roots = ctx.clone();
    let heard: DaemonEventListener = Rc::new(move |event: serde_json::Map<String, Value>| {
        if event.get("type").and_then(Value::as_str) == Some("pty.exit") {
            let pty = event.get("ptyId").and_then(Value::as_str).unwrap_or_default().to_owned();
            if ptys.borrow_mut().shift_remove(&pty).is_some() {
                roots.port_roots_moved(&owner);
            }
        }
        on_event(event);
    });
    ctx.shared_channel(&entry, heard).await
}

// ts: workspaces.ts:1164-1166
//|     async guestChannel(id, onEvent) {
//|       return ctx.copyChannel(await ctx.entryOf(id), onEvent, ctx.GUEST_ROAD_FRAMES);
//|     },
pub async fn guest_channel(ctx: &Rc<dyn ReviewCtx>, id: &str, on_event: DaemonEventListener) -> Result<Rc<dyn DaemonChannel>, HostError> {
    let entry = ctx.entry_of(id, None, EntryAsk::default()).await?;
    ctx.guest_road_channel(&entry, on_event).await
}

// ts: workspaces.ts:1168-1170
//|     async servedByItsComputer(id, origin) {
//|       return ctx.servedByItsComputer(await ctx.entryOf(id, origin)) !== undefined;
//|     },
pub async fn served_by_its_computer(ctx: &Rc<dyn ReviewCtx>, id: &str, origin: Option<Caller>) -> Result<bool, HostError> {
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    Ok(ctx.served_by_its_computer(&entry).is_some())
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    if !ctx.metrics_on_host(&entry) {
        let word = ctx.machine_word(&entry);
        return Err(HostError::new(format!("{word} reads its own load over its daemon link, not from this host")));
    }
    match ctx.sys_samples(listener) {
        Some(detach) => Ok(detach),
        None => Err(HostError::new("this host reads nothing of the computer it runs on")),
    }
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    ctx.copy_blocked(&entry).await?;
    match ctx.kind_port_reach(&entry, port, false).await {
        Some(own) => own,
        None => ctx.ws_port_reach(&entry, port).await,
    }
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
    let entry = ctx.entry_of(id, origin, EntryAsk::default()).await?;
    ctx.copy_blocked(&entry).await?;
    if let Some(own) = ctx.kind_port_reach(&entry, port, true).await {
        let reach = own?;
        return ctx.probe(&reach.url).await;
    }
    let reach = ctx.ws_port_reach(&entry, port).await?;
    let probed = ctx.probe(&reach.url).await?;
    if probed.status.0 .0 == 401.0 {
        ctx.ws_remint_port_reach(&entry, port).await?;
    }
    Ok(probed)
}

// ts: workspaces.ts:1206-1209
//|     async originRefusal(id, origin) {
//|       await ctx.ready();
//|       return ctx.refusalFor(live.get(id)?.record, origin);
//|     },
pub async fn origin_refusal(ctx: &Rc<dyn ReviewCtx>, id: &str, origin: Option<Caller>) -> Result<Option<String>, HostError> {
    ctx.ready().await?;
    let record = held_record(ctx, id);
    Ok(ctx.refusal_for(record.as_ref(), origin.as_ref()))
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
    let Some(scope) = scope_of(origin) else {
        return true;
    };
    if let Some(asked) = asker_of(event) {
        return asked.thread_id == scope.thread_id || asked.root_thread_id == scope.root_thread_id;
    }
    let Some(record) = workspace_id_of(event).and_then(|id| held_record(ctx, id.as_ref())) else {
        return false;
    };
    if let Some(thread) = event.get("threadId").and_then(Value::as_str) {
        return ctx.reaches_row(thread, &record.id, origin);
    }
    ctx.refusal_for(Some(&record), origin).is_none()
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
    let is_status = event.get("type").and_then(Value::as_str) == Some("workspace.status");
    let tree = if is_status { event.get("status").and_then(|s| s.get("tree")).cloned() } else { None };
    let (Some(tree), Some(_)) = (tree, scope_of(origin)) else {
        return event;
    };
    let all = tree.get("children").and_then(Value::as_array).cloned().unwrap_or_default();
    let children: Vec<Value> = all
        .iter()
        .filter(|child| {
            let id = child.get("workspaceId").and_then(Value::as_str).unwrap_or_default();
            held_record(ctx, id).is_some_and(|record| ctx.refusal_for(Some(&record), origin).is_none())
        })
        .cloned()
        .collect();
    if children.len() == all.len() {
        return event;
    }
    let mut status = event.get("status").cloned().unwrap_or(Value::Null);
    if let Value::Object(map) = &mut status {
        map.shift_remove("tree");
        if !children.is_empty() {
            let mut kept_tree = tree.clone();
            kept_tree["children"] = Value::Array(children);
            map.insert("tree".to_owned(), kept_tree);
        }
    }
    let mut out = event;
    out["status"] = status;
    out
}
