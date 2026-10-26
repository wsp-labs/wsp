// SPDX-License-Identifier: AGPL-3.0-only
//! Events, status, the workspace verbs, daemon channels, readings and exec. serve.ts:369-373, 1170-1473, 1634-1678.
//! Spec: spec/serve_workspaces.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::StreamExt;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use serde_json::{json, Map, Value};

use wsp_core::daemon_channel::DaemonChannel;
use wsp_core::forwards::ForwardEvent;
use wsp_core::status::{cost_moved, CostMoved, Reconcile, StatusListOptions, StatusReader, StatusWatchOptions};
use wsp_core::sys_socket::{sys_socket, Send, SysAsk};
use wsp_core::types::api::{
    CommitAsk, CommitDraftAsk, DaemonEventListener, DiscardAsk, FixAsk, Landing, MergeAsk, MergeInAsk, PullRequestReplyAsk, ReviewAsk,
    ReviewDraftAsk, StartFromLinkAsk, ViewedMark, WorktreeRemoveAsk,
};
use wsp_core::types::events::ANY_EVENT;
use wsp_core::types::wiring::{CreateWorkspaceOptions, IdleWindow, OriginStatusApi, SizeAsk, SubAsk, WorkspaceSpec};
use wsp_machines::places::types::NO_PLACE_DOOR;
use wsp_wire::types::check::parse;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::release::ReleaseChangedEvent;
use wsp_wire::types::workspace_look::WorkspaceLook;
use wsp_wire::views::place::{DoctorLineEvent, ForwardCloseEvent, ForwardOpenEvent};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_more::{WorkspaceListing, WorkspaceStatus, WorkspaceView};
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::place_link::DAEMON_OPEN_ONE_OF;
use wsp_wire::wire::runtime::WorkspacesReviewDraftVerdict;
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::HostError;
use wsp_words::start::ReviewVerdict;
use wsp_words::workspace_state::absent_computer;

use super::answer::{ok_reply, Handled};
use super::places::refused_off_own_road;
use super::served::Payload;
use super::socket::{Socket, SysRoad};
use super::{handed, started_out};

/// The length in bytes of a daemon channel's id and an exec's id before they are written in lower hex. serve.ts:1408, 1658.
pub const CHANNEL_ID_BYTES: usize = 6;
/// What `daemon.send` and `daemon.close` fail with for an id this socket did not open. serve.ts:1455, 1462.
pub const NO_SUCH_CHANNEL: &str = "no such daemon channel on this socket";

/// Answers the ops of this unit and sends their replies and pushes; `NotMine` for any other op.
pub async fn answer_workspaces(
    socket: &Rc<Socket>,
    frame: &serde_json::Value,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
) -> Result<Handled, HostError> {
    let served = socket.served.clone();
    let rt = &*served.rt;
    let id = serde_json::to_value(msg.op.id()).unwrap_or(Value::Null);
    let reply = match &msg.op {
        RuntimeOp::EventsSubscribe { after, stream, creates, .. } => {
            let after = after.as_ref().and_then(|after| u64::try_from(after.get()).ok());
            subscribe_events(socket, &id, after, stream.as_deref(), *creates == Some(true), origin);
            return Ok(Handled::Answered);
        }
        RuntimeOp::StatusSubscribe { .. } => {
            let status = rt.status();
            let release = status.watch(StatusWatchOptions::default());
            socket.detach(Box::new(move || release()));
            let statuses = OriginStatusApi::list(&*status, None, origin).await?;
            fields([("statuses", value_of(&statuses)?)])
        }
        RuntimeOp::StatusList { .. } => {
            let opts = StatusListOptions {
                reconcile: Some(Reconcile::OnFailure),
                zombie_probe: Some(false),
                reader: Some(StatusReader::Table),
                ..Default::default()
            };
            let status = rt.status();
            let statuses = OriginStatusApi::list(&*status, Some(opts), origin).await?;
            let rows = statuses.iter().map(listing).collect::<Result<Vec<Value>, HostError>>()?;
            fields([("statuses", Value::Array(rows))])
        }
        RuntimeOp::WorkspacesCreate { golden, project, name, cpu, mem_mb, envs, labels, agents, idle_window_ms, parent, sub, .. } => {
            let opts = CreateWorkspaceOptions {
                spec: WorkspaceSpec { cpu: *cpu, mem_mb: *mem_mb, envs: envs.clone(), labels: labels.clone(), binds: None, copy: None },
                project: project.clone(),
                golden: golden.clone(),
                name: name.clone(),
                sub: sub.as_ref().map(|sub| SubAsk {
                    on: sub.on.clone(),
                    size: sub.size.clone(),
                    from: sub.from.clone(),
                    branch: sub.branch.clone(),
                    harness: sub.harness.clone(),
                }),
                opens_thread: None,
                agents: agents.as_ref().map(spread).transpose()?,
                idle_window_ms: idle_window(idle_window_ms),
                parent: parent.clone(),
                branch_of_record: None,
            };
            let made = rt.workspaces().create(opts, origin).await?;
            let mut out = workspace_of(made.view);
            if let Some(notice) = made.notice {
                out.insert("notice".into(), Value::from(notice));
            }
            if let Some(sub) = made.sub {
                out.insert("sub".into(), value_of(&sub)?);
            }
            out
        }
        RuntimeOp::WorkspacesLanding { project, sized, .. } => {
            let landing = rt.workspaces().landing(project, sized.is_some(), origin).await?;
            landing_fields(&landing)?
        }
        RuntimeOp::WorkspacesList { .. } => {
            let views = rt.workspaces().list(origin).await?;
            fields([("workspaces", Value::Array(views.into_iter().map(handed).collect()))])
        }
        RuntimeOp::WorkspacesResolve { r#ref, .. } => workspace_of(rt.workspaces().resolve(r#ref, origin).await?),
        RuntimeOp::WorkspacesGet { workspace_id, thread_id, .. } => {
            workspace_of(rt.workspaces().get(workspace_id, origin, thread_id.as_deref()).await?)
        }
        RuntimeOp::WorkspacesNap { workspace_id, .. } => workspace_of(rt.workspaces().nap(workspace_id, origin).await?),
        RuntimeOp::WorkspacesWake { workspace_id, .. } => workspace_of(rt.workspaces().wake(workspace_id, origin).await?),
        RuntimeOp::WorkspacesResize { workspace_id, size, .. } => {
            let size = SizeAsk { cpu: size.cpu.0, mem_mb: size.mem_mb.0 };
            workspace_of(rt.workspaces().resize(workspace_id, size, origin).await?)
        }
        RuntimeOp::WorkspacesStopWake { workspace_id, .. } => workspace_of(rt.workspaces().stop_wake(workspace_id, origin).await?),
        RuntimeOp::WorkspacesUpgrade { workspace_id, .. } => workspace_of(rt.workspaces().upgrade(workspace_id, origin).await?),
        RuntimeOp::WorkspacesRename { workspace_id, name, .. } => workspace_of(rt.workspaces().rename(workspace_id, name, origin).await?),
        RuntimeOp::WorkspacesLook { workspace_id, theme, glyph, .. } => {
            let look = WorkspaceLook { theme: theme.clone(), glyph: *glyph };
            workspace_of(rt.workspaces().look(workspace_id, look, origin).await?)
        }
        RuntimeOp::WorkspacesRestartDaemon { workspace_id, .. } => {
            rt.workspaces().restart_daemon(workspace_id, origin).await?;
            Payload::new()
        }
        RuntimeOp::WorkspacesDelete { workspace_id, .. } => {
            rt.workspaces().delete(workspace_id, origin).await?;
            Payload::new()
        }
        RuntimeOp::WorkspacesForget { workspace_id, .. } => {
            rt.workspaces().forget(workspace_id, origin).await?;
            Payload::new()
        }
        RuntimeOp::WorkspacesTouch { workspace_id, .. } => {
            rt.workspaces().touch(workspace_id, origin).await?;
            Payload::new()
        }
        RuntimeOp::WorkspacesRebuild { workspace_id, .. } => workspace_of(rt.workspaces().rebuild(workspace_id, origin).await?),
        RuntimeOp::WorkspacesBringBack { workspace_id, title, body, .. } => {
            let brought = rt.workspaces().bring_back(workspace_id, title.clone(), body.clone(), origin).await?;
            spread(&brought)?
        }
        RuntimeOp::WorktreeMake { project, branch, .. } => spread(&rt.workspaces().worktree(project, branch, origin).await?)?,
        RuntimeOp::FolderMake { project, .. } => workspace_of(rt.workspaces().folder(project, origin).await?),
        RuntimeOp::WorktreeRemove { project, branch, force, check, .. } => {
            let ask = WorktreeRemoveAsk { project: project.clone(), branch: branch.clone(), force: *force, check: sent_true(*check) };
            rt.workspaces().worktree_remove(ask, origin).await?;
            Payload::new()
        }
        RuntimeOp::WorkspacesCheckout { workspace_id, fresh, .. } => {
            spread(&rt.workspaces().checkout(workspace_id, origin, Some(*fresh == Some(true))).await?)?
        }
        RuntimeOp::WorkspacesDiscard { workspace_id, path, check, thread_id, .. } => {
            let ask = DiscardAsk {
                workspace_id: workspace_id.clone(),
                path: path.clone(),
                check: sent_true(*check),
                thread_id: thread_id.clone(),
            };
            spread(&rt.workspaces().discard(ask, origin).await?)?
        }
        RuntimeOp::WorkspacesCommit { workspace_id, message, paths, thread_id, .. } => {
            let ask = CommitAsk {
                workspace_id: workspace_id.clone(),
                message: message.clone(),
                paths: paths.clone(),
                thread_id: thread_id.clone(),
            };
            spread(&rt.workspaces().commit(ask, origin).await?)?
        }
        RuntimeOp::WorkspacesCommitDraft { workspace_id, paths, thread_id, .. } => {
            let ask = CommitDraftAsk { workspace_id: workspace_id.clone(), paths: paths.clone(), thread_id: thread_id.clone() };
            spread(&rt.workspaces().commit_draft(ask, origin).await?)?
        }
        RuntimeOp::WorkspacesViewed { workspace_id, path, blob, .. } => {
            let mark = path.as_ref().map(|path| ViewedMark { path: path.clone(), blob: blob.clone().flatten() });
            spread(&rt.workspaces().viewed(workspace_id, mark, origin).await?)?
        }
        RuntimeOp::WorkspacesPullRequestView { workspace_id, fresh, .. } => {
            spread(&rt.workspaces().pull_request_view(workspace_id, sent_true(*fresh), origin).await?)?
        }
        RuntimeOp::WorkspacesPullRequestDiff { workspace_id, .. } => {
            spread(&rt.workspaces().pull_request_diff(workspace_id, origin).await?)?
        }
        RuntimeOp::WorkspacesPullRequestSend { workspace_id, items, thread_id, .. } => {
            let sent = rt.workspaces().pull_request_send(workspace_id, items.clone(), thread_id.clone(), origin).await?;
            spread(&sent)?
        }
        RuntimeOp::WorkspacesPullRequestReply { workspace_id, reply_to, thread_id, body, .. } => {
            let ask = PullRequestReplyAsk {
                workspace_id: workspace_id.clone(),
                reply_to: reply_to.and_then(|reply| u64::try_from(reply.get()).ok()),
                thread_id: thread_id.clone(),
                body: body.clone(),
            };
            spread(&rt.workspaces().pull_request_reply(ask, origin).await?)?
        }
        RuntimeOp::WorkspacesPullRequestResolve { workspace_id, thread_id, resolved, .. } => {
            spread(&rt.workspaces().pull_request_resolve(workspace_id, thread_id, *resolved, origin).await?)?
        }
        RuntimeOp::WorkspacesPullRequestReact { workspace_id, subject, content, on, .. } => {
            spread(&rt.workspaces().pull_request_react(workspace_id, subject, *content, *on, origin).await?)?
        }
        RuntimeOp::WorkspacesFix { workspace_id, check, child, thread_id, child_thread_id, .. } => {
            let ask = FixAsk {
                workspace_id: workspace_id.clone(),
                check: check.clone(),
                child: child.clone(),
                thread_id: thread_id.clone(),
                child_thread_id: child_thread_id.clone(),
            };
            spread(&rt.workspaces().fix(ask, origin).await?)?
        }
        RuntimeOp::WorkspacesMerge { workspace_id, method, when_checks_pass, head, thread_id, .. } => {
            let ask = MergeAsk {
                workspace_id: workspace_id.clone(),
                method: *method,
                when_checks_pass: *when_checks_pass,
                head: head.clone(),
                thread_id: thread_id.clone(),
            };
            spread(&rt.workspaces().merge(ask, origin).await?)?
        }
        RuntimeOp::WorkspacesUpdate { workspace_id, thread_id, .. } => {
            spread(&rt.workspaces().update(workspace_id, thread_id.clone(), origin).await?)?
        }
        RuntimeOp::WorkspacesMergeIn { workspace_id, child, thread_id, child_thread_id, .. } => {
            let ask = MergeInAsk {
                workspace_id: workspace_id.clone(),
                child: child.clone(),
                thread_id: thread_id.clone(),
                child_thread_id: child_thread_id.clone(),
            };
            spread(&rt.workspaces().merge_in(ask, origin).await?)?
        }
        RuntimeOp::WorkspacesStart { url, project, agent, model, effort, access, .. } => {
            let ask = StartFromLinkAsk {
                url: url.clone(),
                project: project.clone(),
                agent: agent.clone(),
                model: model.clone(),
                effort: effort.clone(),
                access: *access,
            };
            spread(&started_out(rt.workspaces().start(ask, origin).await?))?
        }
        RuntimeOp::WorkspacesReview { url, workspace_id, agent, model, effort, .. } => {
            let ask = ReviewAsk {
                url: url.clone(),
                workspace_id: workspace_id.clone(),
                agent: agent.clone(),
                model: model.clone(),
                effort: effort.clone(),
            };
            spread(&started_out(rt.workspaces().review(ask, origin).await?))?
        }
        RuntimeOp::WorkspacesReviewDraft { workspace_id, summary, verdict, on, .. } => {
            let ask = ReviewDraftAsk {
                workspace_id: workspace_id.clone(),
                summary: summary.clone(),
                verdict: verdict.map(review_verdict),
                on: on.as_ref().map(|items| items.iter().map(|item| (item.id.clone(), item.on)).collect()),
            };
            match rt.workspaces().review_draft(ask, origin).await? {
                Some(draft) => fields([("review", value_of(&draft)?)]),
                None => Payload::new(),
            }
        }
        RuntimeOp::WorkspacesReviewPost { workspace_id, thread_id, .. } => {
            spread(&rt.workspaces().review_post(workspace_id, thread_id.clone(), origin).await?)?
        }
        RuntimeOp::WorkspacesPortReach { workspace_id, port, .. } => {
            let reach = rt.workspaces().port_reach(workspace_id, port_of(port)?, origin).await?;
            fields([("reach", value_of(&reach)?)])
        }
        RuntimeOp::WorkspacesPortProbe { workspace_id, port, .. } => {
            let probe = rt.workspaces().port_probe(workspace_id, port_of(port)?, origin).await?;
            fields([("probe", value_of(&probe)?)])
        }
        RuntimeOp::DaemonOpen { workspace_id, place_id, .. } => {
            open_daemon(socket, frame, msg, &id, workspace_id, place_id, origin).await?;
            return Ok(Handled::Answered);
        }
        RuntimeOp::SysSubscribe { workspace_id, place_id, .. } => {
            sys_op(socket, frame, msg, workspace_id, place_id, false, origin).await?;
            return Ok(Handled::Answered);
        }
        RuntimeOp::SysUnsubscribe { workspace_id, place_id, .. } => {
            sys_op(socket, frame, msg, workspace_id, place_id, true, origin).await?;
            return Ok(Handled::Answered);
        }
        RuntimeOp::DaemonSend { channel, frame: sent, .. } => {
            let open = socket.channels.borrow().get(channel).cloned();
            let Some(ch) = open else {
                return Err(HostError::new(NO_SUCH_CHANNEL));
            };
            let answer = ch.send(spread(sent)?).await.map_err(|e| HostError::new(e.to_string()))?;
            fields([("reply", Value::Object(answer))])
        }
        RuntimeOp::DaemonClose { channel, .. } => {
            let held = socket.channels.borrow_mut().shift_remove(channel);
            let Some(ch) = held else {
                return Err(HostError::new(NO_SUCH_CHANNEL));
            };
            ch.close();
            Payload::new()
        }
        RuntimeOp::WorkspacesExec { workspace_id, argv, cwd, .. } => {
            start_exec(socket, &id, workspace_id, argv, cwd, origin).await?;
            return Ok(Handled::Answered);
        }
        _ => return Ok(Handled::NotMine),
    };
    socket.answer(Some(frame), ok_reply(&id, reply), None);
    Ok(Handled::Answered)
}

/// One socket's filter over the events it is sent: serve.ts:1176-1184. A workspace event goes out when `seen_by` lets
/// the caller see it and its spend moved, worded by `shown_to`; a line or a create goes out when `seen_by` lets it.
struct Pass {
    socket: Rc<Socket>,
    origin: Option<Caller>,
    moved: RefCell<CostMoved>,
}

impl Pass {
    fn event(&self, event: &EventUnion) {
        let Ok(value) = serde_json::to_value(event) else {
            return;
        };
        if self.seen(&value) && self.moved.borrow_mut().admit(event) {
            self.show(value);
        }
    }

    fn value(&self, value: Value) {
        if self.seen(&value) {
            self.show(value);
        }
    }

    fn seen(&self, value: &Value) -> bool {
        self.socket.served.rt.workspaces().seen_by(value, self.origin.as_ref())
    }

    fn show(&self, value: Value) {
        if let Value::Object(shown) = self.socket.served.rt.workspaces().shown_to(value, self.origin.as_ref()) {
            self.socket.send(shown);
        }
    }
}

/// `events.subscribe`: the reply, then the replay, with the listeners taken in the same step (serve.ts:1170-1188).
fn subscribe_events(socket: &Rc<Socket>, id: &Value, after: Option<u64>, stream: Option<&str>, creates: bool, origin: Option<Caller>) {
    let served = socket.served.clone();
    let bus = served.rt.events();
    let since = bus.since(after, stream);
    let pass = Rc::new(Pass { socket: socket.clone(), origin, moved: RefCell::new(cost_moved()) });

    let heard = pass.clone();
    let off = bus.on(
        ANY_EVENT,
        Rc::new(move |event: &EventUnion| -> Result<(), HostError> {
            heard.event(event);
            Ok(())
        }),
    );
    socket.detach(Box::new(off));

    if let Some(source) = served.forwards.get().cloned().or_else(|| served.opts.forwards.clone()) {
        let heard = pass.clone();
        let off = source.on(Rc::new(move |event: &ForwardEvent| {
            if let Ok(frame) = forward_frame(event) {
                heard.value(frame);
            }
        }));
        socket.detach(off);
    }

    if let Some(doctor) = served.opts.doctor.clone() {
        let heard = pass.clone();
        let off = doctor.on(Rc::new(move |event: &DoctorLineEvent| {
            if let Ok(frame) = tagged(DoctorLineEvent::TYPE, event) {
                heard.value(frame);
            }
        }));
        socket.detach(off);
    }
    if let Some(release) = served.opts.release.clone() {
        let heard = pass.clone();
        let off = release.on(Rc::new(move |event: &ReleaseChangedEvent| {
            if let Ok(frame) = tagged(ReleaseChangedEvent::TYPE, event) {
                heard.value(frame);
            }
        }));
        socket.detach(off);
    }

    let mut reply = fields([("seq", json!(since.head)), ("stream", json!(since.stream))]);
    if since.gap {
        reply.insert("gap".into(), Value::Bool(true));
    }
    socket.send(ok_reply(id, reply));
    for event in &since.events {
        pass.event(event);
    }
    if creates {
        for created in served.rt.workspaces().creating() {
            if let Ok(frame) = tagged(WorkspaceCreatingEvent::TYPE, &created) {
                pass.value(frame);
            }
        }
    }
}

/// `daemon.open`: a channel to a workspace's daemon, or to a computer's (serve.ts:1407-1467). The reply goes before any
/// frame the daemon pushes, which wait in `queued` until it is written.
async fn open_daemon(
    socket: &Rc<Socket>,
    frame: &Value,
    msg: &RuntimeRequest,
    id: &Value,
    workspace_id: &Option<String>,
    place_id: &Option<String>,
    origin: Option<Caller>,
) -> Result<(), HostError> {
    let served = socket.served.clone();
    let rt = &*served.rt;
    let channel = random_id();
    let queued: Queue = Rc::new(RefCell::new(Some(Vec::new())));
    let on_event: DaemonEventListener = {
        let socket = socket.clone();
        let queued = queued.clone();
        let channel = channel.clone();
        Rc::new(move |event: Map<String, Value>| {
            if let Some(held) = queued.borrow_mut().as_mut() {
                held.push(event);
                return;
            }
            socket.send(daemon_event(&channel, event));
        })
    };

    let ch: Rc<dyn DaemonChannel> = if let Some(place) = place_id {
        if workspace_id.is_some() {
            return Err(HostError::new(DAEMON_OPEN_ONE_OF));
        }
        if refused_off_own_road(socket, frame, msg) {
            return Ok(());
        }
        if place.as_str() == HERE_PLACE_ID {
            rt.here_channel(on_event).await?
        } else {
            let places = rt.places().ok_or_else(|| HostError::new(NO_PLACE_DOOR))?;
            let Some(link) = places.channel(place, on_event) else {
                return Err(HostError::new(absent_computer(&places.name_of(place), None).sentence));
            };
            rt.place_kills(link)
        }
    } else {
        let Some(workspace) = workspace_id else {
            return Err(HostError::new(DAEMON_OPEN_ONE_OF));
        };
        rt.workspaces().daemon_channel(workspace, on_event, origin).await?
    };

    if !socket.sink.is_open() {
        ch.close();
        return Ok(());
    }
    socket.channels.borrow_mut().insert(channel.clone(), ch.clone());
    {
        let socket = socket.clone();
        let channel = channel.clone();
        let ch = ch.clone();
        tokio::task::spawn_local(async move {
            let end = ch.closed().await;
            let still_ours = socket.channels.borrow().get(&channel).is_some_and(|held| Rc::ptr_eq(held, &ch));
            if !still_ours {
                return;
            }
            socket.channels.borrow_mut().shift_remove(&channel);
            socket.send(fields([
                ("type", json!("daemon.closed")),
                ("channel", json!(channel)),
                ("code", json!(end.code)),
                ("reason", json!(end.reason)),
            ]));
        });
    }
    socket.send(ok_reply(id, fields([("channel", json!(channel))])));
    let held = queued.borrow_mut().take().unwrap_or_default();
    for event in held {
        socket.send(daemon_event(&channel, event));
    }
    Ok(())
}

/// `sys.subscribe` and `sys.unsubscribe`: the socket's sys road (serve.ts:1468-1473). A computer's readings are read on
/// the own road only, as every places op is.
async fn sys_op(
    socket: &Rc<Socket>,
    frame: &Value,
    msg: &RuntimeRequest,
    workspace_id: &Option<String>,
    place_id: &Option<String>,
    unsubscribe: bool,
    origin: Option<Caller>,
) -> Result<(), HostError> {
    if !unsubscribe && place_id.is_some() && refused_off_own_road(socket, frame, msg) {
        return Ok(());
    }
    let answering = socket.clone();
    let answer_frame = frame.clone();
    let send: Send = Rc::new(move |payload: Payload| answering.answer(Some(&answer_frame), payload, None));
    let ask = SysAsk { id: msg.op.id().clone(), unsubscribe, workspace_id: workspace_id.clone(), place_id: place_id.clone() };
    sys_road(socket)(ask, send, origin).await
}

/// The socket's sys road, built the first time a sys op asks (sys-socket.ts:15, serve.ts:563).
fn sys_road(socket: &Rc<Socket>) -> SysRoad {
    if let Some(road) = socket.sys_road.borrow().clone() {
        return road;
    }
    let road = sys_socket(socket.served.rt.clone(), socket.clone());
    *socket.sys_road.borrow_mut() = Some(road.clone());
    road
}

/// `workspaces.exec`: the command's lines and exit go to this socket alone (serve.ts:1656-1678). The reply carries the
/// folder the command ran in where the stream names one; a close of the socket tears the stream down until it exits.
async fn start_exec(
    socket: &Rc<Socket>,
    id: &Value,
    workspace_id: &str,
    argv: &[String],
    cwd: &Option<String>,
    origin: Option<Caller>,
) -> Result<(), HostError> {
    let served = socket.served.clone();
    let running = served.rt.workspaces().exec_stream(workspace_id, argv.to_vec(), cwd.clone(), origin).await?;
    let exec_id = random_id();
    let live = Rc::new(Cell::new(true));
    let stream = running.stream.clone();
    let still_live = live.clone();
    socket.detach(Box::new(move || {
        if still_live.get() {
            stream.teardown();
        }
    }));
    let mut reply = fields([("execId", json!(exec_id))]);
    if let Some(ran_in) = &running.ran_in {
        reply.insert("cwd".into(), json!(ran_in));
    }
    socket.send(ok_reply(id, reply));

    let socket = socket.clone();
    tokio::task::spawn_local(async move {
        let mut error = None;
        let mut lines = running.stream.lines();
        while let Some(line) = lines.next().await {
            match line {
                Ok(text) => socket.send(fields([("type", json!("exec.output")), ("execId", json!(exec_id)), ("text", json!(text))])),
                Err(failure) => {
                    error = Some(failure.message);
                    break;
                }
            }
        }
        let code = running.stream.exited().await;
        live.set(false);
        let mut exit = fields([("type", json!("exec.exit")), ("execId", json!(exec_id)), ("exitCode", json!(code))]);
        if let Some(error) = error {
            exit.insert("error".into(), json!(error));
        }
        socket.send(exit);
    });
    Ok(())
}

/// `workspaces.landing`'s answer: `place` where the landing has one, then `name`, `capabilities`, `kind` (serve.ts:1215).
fn landing_fields(landing: &Landing) -> Result<Payload, HostError> {
    let mut out = Payload::new();
    if let Some(place) = &landing.place {
        out.insert("place".into(), json!(place));
    }
    out.insert("name".into(), json!(landing.name));
    out.insert("capabilities".into(), value_of(&landing.capabilities)?);
    if let Some(kind) = &landing.kind {
        out.insert("kind".into(), value_of(kind)?);
    }
    Ok(out)
}

/// `status.list`'s row: the listing's own fields, so the route a reach carries is dropped (serve.ts:1195-1206).
fn listing(status: &WorkspaceStatus) -> Result<Value, HostError> {
    let parsed: WorkspaceListing = parse(&value_of(status)?).map_err(|e| HostError::new(e.issue_text()))?;
    value_of(&parsed)
}

/// A workspace verb's answer: `workspace`, through `handed`.
fn workspace_of(view: WorkspaceView) -> Payload {
    fields([("workspace", handed(view))])
}

/// A stream's event as the page reads it: its `type`, then its fields (the TS literal's order).
fn tagged<T: Serialize>(kind: &str, event: &T) -> Result<Value, HostError> {
    let mut out = Payload::new();
    out.insert("type".into(), Value::from(kind));
    out.extend(spread(event)?);
    Ok(Value::Object(out))
}

fn forward_frame(event: &ForwardEvent) -> Result<Value, HostError> {
    match event {
        ForwardEvent::Open(open) => tagged(ForwardOpenEvent::TYPE, open),
        ForwardEvent::Close(close) => tagged(ForwardCloseEvent::TYPE, close),
    }
}

fn daemon_event(channel: &str, event: Map<String, Value>) -> Payload {
    fields([("type", json!("daemon.event")), ("channel", json!(channel)), ("event", Value::Object(event))])
}

/// The id a channel or an exec is named by: `CHANNEL_ID_BYTES` random bytes in lower hex.
fn random_id() -> String {
    let mut bytes = [0u8; CHANNEL_ID_BYTES];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// `workspaces.create`'s idle window: absent takes the default, null turns auto-nap off (wiring.ts:126).
fn idle_window(sent: &Option<Option<JsNumber>>) -> IdleWindow {
    match sent {
        None => IdleWindow::Default,
        Some(None) => IdleWindow::Off,
        Some(Some(ms)) => IdleWindow::Ms(*ms),
    }
}

/// A daemon channel's pushed events, held until its open reply is written; `None` once they go out.
type Queue = Rc<RefCell<Option<Vec<Map<String, Value>>>>>;

/// A flag the TS sends only when it is `true`: `Some(true)` or nothing.
fn sent_true(flag: Option<bool>) -> Option<bool> {
    flag.filter(|set| *set)
}

fn review_verdict(verdict: WorkspacesReviewDraftVerdict) -> ReviewVerdict {
    match verdict {
        WorkspacesReviewDraftVerdict::Comment => ReviewVerdict::Comment,
        WorkspacesReviewDraftVerdict::Approve => ReviewVerdict::Approve,
        WorkspacesReviewDraftVerdict::RequestChanges => ReviewVerdict::RequestChanges,
    }
}

/// A port the schema holds to 1 to 65535 (runtime_op_check.rs), so the conversion is the only check left.
fn port_of(port: &JsInt) -> Result<u16, HostError> {
    u16::try_from(port.get()).map_err(|_| HostError::new("port must be between 1 and 65535"))
}

/// `{id, ok: true, ...fields}`'s fields: the pairs given, in their order.
fn fields<const N: usize>(pairs: [(&str, Value); N]) -> Payload {
    pairs.into_iter().map(|(key, value)| (key.to_string(), value)).collect()
}

/// The fields of an answer spread beside id and ok, in the answer's own order.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match value_of(answer)? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}

fn value_of<T: Serialize>(answer: &T) -> Result<Value, HostError> {
    serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))
}
