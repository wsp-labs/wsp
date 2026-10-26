// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:476-895: from a checked start to a running turn. Spec: spec/sessions_launch.md.
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::{select, Either, FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_adapters::port::{AdapterError, ForkedSession, HarnessAdapter, HarnessStartOptions, KeptTurn, OnEvent, Settle, TurnImage};
use wsp_core::clock::Cancel;
use wsp_core::console;
use wsp_core::types::api::{HeldStart, SettleAsk, StartOptions};
use wsp_core::types::internal::{KeptLaunch, KeptProcess, SessionEntry, ThreadRecord, TurnAsked};
use wsp_core::types::wiring::{SessionHandle, SteerTaken, WorkspaceRecord};
use wsp_wire::types::attachments::Attachment;
use wsp_wire::views::harness::HarnessCatalogSource;
use wsp_wire::views::session::{thread_ran, SessionOrigin, SessionStatus, SessionView, TitleSource};
use wsp_wire::views::session_events::{TurnResult, AGENT_STARTING_MS};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::scope_of;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::attachments::{attached_files_prompt, is_image, send_files_dir, steer_files_blocked, turn_images_dir};
use wsp_words::exit::{HostError, Kind};
use wsp_words::projects::{home_shortened, no_cwd_line};
use wsp_words::thread_read::{thread_messages, thread_seed};
use wsp_words::words::computer::{send_given_up_line, thread_word};
use wsp_words::words::turn::{kept_opening, listed_failure, title_line};
use wsp_words::workspace_state::is_local_workspace;

use super::start::{drop_scope, refuse_now, Prepared};
use super::{HeldSend, SessionsArea, SessionsCtx};
use crate::threads::agents::launch::LaunchServers;
use crate::threads::launch::{LaunchAsked, LaunchPicks};
use crate::threads::threads::{CapAt, Entry, Row};
use crate::threads::turns::{Open, Opening, SnapshotFrom, TurnSnapshot, TurnStart};

/// The flags a send's timers and its bus listener share (TS `quiet`, `slow`, `offStarted`, `outcome`).
struct Saying {
    quiet: std::cell::Cell<bool>,
    outcome: std::cell::Cell<SessionStartOutcome>,
    slow: RefCell<Option<Cancel>>,
    off_started: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Saying {
    fn new() -> Rc<Saying> {
        Rc::new(Saying {
            quiet: std::cell::Cell::new(false),
            outcome: std::cell::Cell::new(SessionStartOutcome::Started),
            slow: RefCell::new(None),
            off_started: RefCell::new(None),
        })
    }

    fn hush(&self) {
        self.quiet.set(true);
        let slow = self.slow.borrow_mut().take();
        if let Some(cancel) = slow {
            cancel();
        }
        let off = self.off_started.borrow_mut().take();
        if let Some(off) = off {
            off();
        }
    }
}

/// A started handle with its outcome or notice set over (TS `{ ...handle, outcome }`, `Object.assign(handle, { notice })`).
struct Decorated {
    inner: Rc<dyn SessionHandle>,
    outcome: Option<SessionStartOutcome>,
    notice: Option<String>,
}

impl SessionHandle for Decorated {
    fn id(&self) -> String {
        self.inner.id()
    }
    fn workspace_id(&self) -> String {
        self.inner.workspace_id()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.inner.finished()
    }
    fn turn_id(&self) -> String {
        self.inner.turn_id()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.outcome.unwrap_or_else(|| self.inner.outcome())
    }
    fn notice(&self) -> Option<String> {
        self.notice.clone().or_else(|| self.inner.notice())
    }
    fn view(&self) -> SessionView {
        self.inner.view()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.inner.interrupt()
    }
    fn steer(
        &self,
        prompt: String,
        id: Option<String>,
        images: Vec<TurnImage>,
    ) -> Option<wsp_core::types::wiring::Local<'static, SteerTaken>> {
        self.inner.steer(prompt, id, images)
    }
    fn tells_unread(&self) -> Option<bool> {
        self.inner.tells_unread()
    }
    fn answer(
        &self,
        ask_id: String,
        option_id: String,
        reason: Option<String>,
    ) -> Option<wsp_core::types::wiring::Local<'static, wsp_wire::wire::runtime::SessionAnswerResult>> {
        self.inner.answer(ask_id, option_id, reason)
    }
    fn set_access(&self, mode: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_core::types::wiring::AccessMove>> {
        self.inner.set_access(mode)
    }
    fn stop_task(&self, task: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_adapters::port::TaskStop>> {
        self.inner.stop_task(task)
    }
}

/// The locals of the TS `launchPrepared` that live across its awaits.
struct Run {
    p: Prepared,
    turn_id: String,
    minted: String,
    row: Row,
    saying: Rc<Saying>,
    launched: RefCell<Option<oneshot::Sender<()>>>,
    lender: Option<String>,
    resume: Option<String>,
    held: bool,
    to_held: bool,
    to_box: bool,
    cleared: bool,
    limit_details: bool,
    catalog_version: Option<Option<String>>,
    harness_version: Option<String>,
    images: Vec<TurnImage>,
    images_dir: Option<String>,
    file_paths: Vec<String>,
    files_folder: Option<String>,
    launch_servers: LaunchServers,
    landed: bool,
    through: bool,
    snapshot: Option<TurnSnapshot>,
    handed_over: bool,
    kept_taken: Option<Rc<RefCell<KeptProcess>>>,
    readied: Option<ForkedSession>,
    failure: Option<String>,
    folder: String,
    picks_for: Option<Box<dyn Fn(Option<&str>) -> Result<LaunchPicks, HostError>>>,
}

fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn union_event(doc: Value) -> Result<EventUnion, HostError> {
    serde_json::from_value(doc).map_err(|e| HostError::new(e.to_string()))
}

fn session_event(doc: Value) -> Result<SessionEvent, HostError> {
    serde_json::from_value(doc).map_err(|e| HostError::new(e.to_string()))
}

fn turn_of(event: &SessionEvent) -> Option<String> {
    serde_json::to_value(event).ok().and_then(|v| v.get("turnId").and_then(|t| t.as_str().map(str::to_string)))
}

fn adapter_error(e: AdapterError) -> HostError {
    HostError::new(e.0)
}

fn set_opt(doc: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        doc.insert(key.to_string(), value);
    }
}

fn port_servers(
    servers: Option<&IndexMap<String, wsp_wire::views::harness::McpServerSpec>>,
) -> Option<IndexMap<String, wsp_adapters::port::McpServerSpec>> {
    servers.map(|servers| {
        servers
            .iter()
            .map(|(name, spec)| {
                (
                    name.clone(),
                    wsp_adapters::port::McpServerSpec {
                        command: spec.command.clone(),
                        args: spec.args.clone(),
                        no_slate: spec.no_slate.map(|_| true),
                    },
                )
            })
            .collect()
    })
}

fn inline_images(attachments: &[Attachment]) -> Vec<TurnImage> {
    attachments.iter().map(|a| TurnImage { media_type: a.media_type.clone(), bytes: a.bytes.clone(), path: None }).collect()
}

fn cap_at(ws: &str, thread: &str, turn: &str, session: &str, prompt: &str, request_id: Option<String>) -> CapAt {
    CapAt {
        workspace_id: ws.to_string(),
        thread_id: thread.to_string(),
        turn_id: turn.to_string(),
        session_id: session.to_string(),
        request_id,
        prompt: prompt.to_string(),
    }
}

/// The picks a launch carries, keyed as the TS object is (`{ model?, effort?, permissionMode?, contextWindow?, fast? }`).
fn picks_map(picks: &LaunchPicks) -> IndexMap<String, Value> {
    let mut map = IndexMap::new();
    if let Some(model) = &picks.picks.model {
        map.insert("model".to_string(), json!(model));
    }
    if let Some(effort) = &picks.picks.effort {
        map.insert("effort".to_string(), json!(effort));
    }
    if let Some(mode) = &picks.picks.permission_mode {
        map.insert("permissionMode".to_string(), json!(mode));
    }
    if let Some(window) = &picks.context_window {
        map.insert("contextWindow".to_string(), json!(window));
    }
    if let Some(fast) = picks.picks.fast {
        map.insert("fast".to_string(), json!(fast));
    }
    map
}

/// The picks a send named, kept for the match a kept process is taken on (TS `asked` in the handover).
fn asked_map(o: &StartOptions, picks: &LaunchPicks) -> IndexMap<String, Value> {
    let mut map = IndexMap::new();
    if o.model.is_some() {
        if let Some(model) = &picks.picks.model {
            map.insert("model".to_string(), json!(model));
        }
    }
    if o.effort.is_some() {
        if let Some(effort) = &picks.picks.effort {
            map.insert("effort".to_string(), json!(effort));
        }
    }
    if o.context_window.is_some() {
        if let Some(window) = &picks.context_window {
            map.insert("contextWindow".to_string(), json!(window));
        }
    }
    if let Some(mode) = &picks.picks.permission_mode {
        map.insert("permissionMode".to_string(), json!(mode));
    }
    if picks.picks.fast == Some(true) {
        map.insert("fast".to_string(), json!(true));
    }
    map
}

/// What a `session.starting` line names.
struct SayingCall {
    workspace_id: String,
    thread_id: String,
    harness: String,
    request_id: Option<String>,
}

/// sessions.ts:524-534 `sayStarting`, run detached by the timer and by the handover.
async fn say_starting(ctx: Rc<dyn SessionsCtx>, saying: Rc<Saying>, entry: Entry, said: SayingCall) {
    if saying.quiet.get() || saying.outcome.get() == SessionStartOutcome::Queued {
        saying.hush();
        return;
    }
    let Ok(installs) = ctx.first_run_here(&entry, &said.harness).await else { return };
    if saying.quiet.get() {
        return;
    }
    saying.hush();
    let mut doc =
        json!({ "type": "session.starting", "workspaceId": said.workspace_id, "threadId": said.thread_id, "harness": said.harness });
    if installs {
        doc["installs"] = json!(true);
    }
    if let Some(request_id) = &said.request_id {
        doc["requestId"] = json!(request_id);
    }
    if let Ok(event) = union_event(doc) {
        ctx.emit(event);
    }
}

/// sessions.ts:524-534: the timer (`AGENT_STARTING_MS`) and the bus listener that hush the line.
fn start_saying(ctx: &Rc<dyn SessionsCtx>, saying: &Rc<Saying>, entry: &Entry, said: SayingCall, turn_id: &str) {
    let (ctx_t, saying_t, entry_t) = (ctx.clone(), saying.clone(), entry.clone());
    let cancel = ctx.clock().schedule(
        Box::new(move || {
            tokio::task::spawn_local(say_starting(ctx_t, saying_t, entry_t, said));
        }),
        AGENT_STARTING_MS as i64,
    );
    *saying.slow.borrow_mut() = Some(cancel);
    let weak: Weak<Saying> = Rc::downgrade(saying);
    let off = ctx.on_turn_settled(
        turn_id,
        Rc::new(move || {
            if let Some(saying) = weak.upgrade() {
                saying.hush();
            }
        }),
    );
    *saying.off_started.borrow_mut() = Some(off);
}

/// The seed a resuming turn hands its agent: the thread's earlier turns, this one left out (TS `seedAfterLoss` seed).
fn seed_for(
    ctx: &Rc<dyn SessionsCtx>,
    ws: &str,
    thread: &str,
    turn: &str,
) -> Arc<dyn Fn() -> LocalBoxFuture<'static, Result<String, String>>> {
    let (ctx, ws, thread, turn) = (ctx.clone(), ws.to_string(), thread.to_string(), turn.to_string());
    Arc::new(move || {
        let (ctx, ws, thread, turn) = (ctx.clone(), ws.clone(), thread.clone(), turn.clone());
        async move {
            let events = ctx.open_thread(&ws, &thread).await.map_err(|e| e.message)?;
            let others: Vec<SessionEvent> = events.into_iter().filter(|e| turn_of(e).as_deref() != Some(turn.as_str())).collect();
            Ok(thread_seed(&thread_messages(&others, &thread)))
        }
        .boxed_local()
    })
}

impl SessionsArea {
    /// sessions.ts:476-895. Rules L1-L21, Z1-Z6.
    pub async fn launch_prepared(&self, prepared: Prepared) -> Result<Rc<dyn SessionHandle>, HostError> {
        let ctx = self.ctx.clone();
        let turn_id = match prepared.opts.r#as.as_ref() {
            Some(a) => a.turn_id.clone(),
            None => ctx.ids().uuid(),
        };
        let minted = ctx.ids().uuid();
        let view = {
            let mut doc = Map::new();
            doc.insert("id".into(), json!(turn_id));
            doc.insert("workspaceId".into(), json!(prepared.workspace_id));
            doc.insert("harness".into(), json!(prepared.built.harness));
            doc.insert("status".into(), json!("running"));
            doc.insert("startedBy".into(), json!(prepared.opts.started_by.unwrap_or(SessionOrigin::Person)));
            doc.insert("threadId".into(), json!(prepared.thread_id));
            set_opt(&mut doc, "parentThreadId", prepared.tree.parent_thread_id.clone().map(Value::from));
            set_opt(&mut doc, "rootThreadId", prepared.tree.root_thread_id.clone().map(Value::from));
            set_opt(&mut doc, "attempt", prepared.opts.attempt.clone().map(Value::from));
            doc.insert("prompt".into(), json!(kept_opening(&prepared.opts.prompt)));
            doc.insert("startedAt".into(), json!(ctx.clock().now() as f64));
            let (title, title_source) = match (&prepared.title, &prepared.opts.outside) {
                (Some(title), _) => (Some(title.clone()), Some(TitleSource::Person)),
                (None, Some(outside)) => (Some(title_line(&outside.title)), Some(TitleSource::Auto)),
                (None, None) => ctx.carried_title(&prepared.thread_id),
            };
            set_opt(&mut doc, "harnessTitle", title.map(Value::from));
            set_opt(&mut doc, "titleSource", title_source.map(|s| json!(s)));
            set_opt(&mut doc, "claudeSessionId", prepared.resume.clone().map(Value::from));
            set_opt(&mut doc, "contextWindow", prepared.opts.context_window.clone().map(Value::from));
            serde_json::from_value::<SessionView>(Value::Object(doc)).map_err(|e| HostError::new(e.to_string()))?
        };
        let row: Row = Rc::new(RefCell::new(SessionEntry {
            view,
            turn_id: turn_id.clone(),
            replaces: None,
            notify: None,
            notify_by: None,
            notify_road: None,
            turn_token: None,
            scope_device_id: None,
            handle: None,
            end: None,
            turn_live: None,
            run: None,
            from: None,
            asked: None,
            snapshot: None,
            pid: None,
            launch: None,
            calls: None,
            unanswered: false,
            queued: false,
        }));
        let lender = {
            let asker = scope_of(prepared.origin.as_ref()).map(|s| s.thread_id);
            match asker {
                Some(asker) if asker != prepared.thread_id && (prepared.opts.followed == Some(true) || prepared.opts.on_held.is_none()) => {
                    ctx.running_on(&asker).and_then(|r| r.borrow().handle.as_ref().map(|h| h.turn_id()))
                }
                _ => None,
            }
        };
        let (launched_tx, launched_rx) = oneshot::channel::<()>();
        let launch_fut: Shared<LocalBoxFuture<'static, ()>> = launched_rx.map(|_| ()).boxed_local().shared();
        row.borrow_mut().launch = Some(launch_fut);
        let resume = prepared.resume.clone();
        let cleared = prepared.cleared;
        let folder_seed = String::new();
        let mut st = Run {
            p: prepared,
            turn_id,
            minted,
            row,
            saying: Saying::new(),
            launched: RefCell::new(Some(launched_tx)),
            lender,
            resume,
            held: false,
            to_held: false,
            to_box: false,
            cleared,
            limit_details: false,
            catalog_version: None,
            harness_version: None,
            images: Vec::new(),
            images_dir: None,
            file_paths: Vec::new(),
            files_folder: None,
            launch_servers: LaunchServers::default(),
            landed: false,
            through: false,
            snapshot: None,
            handed_over: false,
            kept_taken: None,
            readied: None,
            failure: None,
            folder: folder_seed,
            picks_for: None,
        };
        let body = self.launch_body(&mut st).await;
        if let Err(e) = &body {
            st.failure = Some(e.message.clone());
        }
        self.launch_finally(&mut st).await;
        body
    }

    /// sessions.ts:501-523: the thread's row is held, once, while the thread runs no turn.
    fn hold(&self, st: &mut Run) {
        let ctx = self.ctx.clone();
        let thread_id = st.p.thread_id.clone();
        if st.held || ctx.thread_runs(&thread_id) {
            return;
        }
        if let Some(resume) = ctx.latest_on(&thread_id).and_then(|view| view.claude_session_id) {
            st.resume = Some(resume);
        }
        st.held = true;
        ctx.cap_hold(&record_of(&st.p.entry), &st.turn_id, st.lender.as_deref());
        {
            let mut row = st.row.borrow_mut();
            row.replaces = st.p.replaces.clone();
            row.notify = st.p.notify.clone();
            row.notify_by = st.p.notify_by.clone();
            row.notify_road = st.p.notify_road.clone();
        }
        ctx.sessions().borrow_mut().insert(st.turn_id.clone(), st.row.clone());
        let mut doc = json!({ "type": "session.held", "workspaceId": st.p.workspace_id, "threadId": thread_id });
        if let Some(request_id) = &st.p.opts.request_id {
            doc["requestId"] = json!(request_id);
        }
        if let Ok(event) = union_event(doc) {
            ctx.emit(event);
        }
    }

    /// sessions.ts:569-741: `session.queued`, with the computer it waits on where a stop holds it.
    fn say_queued(&self, st: &Run, waits_for: Option<&str>) -> Result<(), HostError> {
        let mut doc = json!({
            "type": "session.queued", "workspaceId": st.p.workspace_id, "threadId": st.p.thread_id,
            "harness": st.p.built.harness, "prompt": st.p.opts.prompt,
        });
        if let Some(request_id) = &st.p.opts.request_id {
            doc["requestId"] = json!(request_id);
        }
        if let Some(waits_for) = waits_for {
            doc["waitsFor"] = json!(waits_for);
        }
        self.ctx.emit(union_event(doc)?);
        Ok(())
    }

    /// The TS try-block of `launchPrepared`: the picks, the wait loop and the landing, then the handover.
    async fn launch_body(&self, st: &mut Run) -> Result<Rc<dyn SessionHandle>, HostError> {
        let ctx = self.ctx.clone();
        let ws = st.p.workspace_id.clone();
        let thread_id = st.p.thread_id.clone();
        let turn_id = st.turn_id.clone();
        let entry = st.p.entry.clone();
        let harness = st.p.built.harness.clone();
        let adapter: Arc<dyn HarnessAdapter> = st.p.built.adapter.clone();
        let o = st.p.opts.clone();
        let origin = st.p.origin.clone();
        let outside = o.outside.clone();

        self.hold(st);
        let said = SayingCall {
            workspace_id: ws.clone(),
            thread_id: thread_id.clone(),
            harness: harness.clone(),
            request_id: o.request_id.clone(),
        };
        start_saying(&ctx, &st.saying, &entry, said, &turn_id);

        let read = ctx
            .launch_area()
            .launch_picks(
                &entry,
                &harness,
                adapter.as_ref(),
                &st.p.prefs,
                &thread_id,
                &LaunchAsked {
                    model: o.model.clone(),
                    effort: o.effort.clone(),
                    permission_mode: o.permission_mode.clone(),
                    access: o.access,
                    context_window: o.context_window.clone(),
                    fast: o.fast,
                },
            )
            .await?;
        st.catalog_version = read.catalog.as_ref().map(|c| c.version.clone());
        st.harness_version = read.catalog.as_ref().filter(|c| c.source == HarnessCatalogSource::Harness).and_then(|c| c.version.clone());
        let picks_for = read.picks_for;
        picks_for(if outside.is_some() { None } else { st.resume.as_deref() })?;
        st.picks_for = Some(picks_for);
        st.folder = ctx.thread_folder(&entry, o.cwd.as_deref()).await?;
        if o.cwd.is_some() && is_local_workspace(Some(&record_of(&entry).kind)) && !ctx.folder_exists(&st.folder) {
            let home = std::env::var("HOME").ok();
            return Err(HostError::with_kind(no_cwd_line(&home_shortened(&st.folder, home.as_deref())), Kind::Usage));
        }
        st.limit_details = ctx.limit_details_due(&entry, &harness).await?;
        let owed = if self.stop_holds(&thread_id) { None } else { ctx.end_owed(&entry, &thread_id).await? };
        let at = ctx.workspace_place(&record_of(&entry));
        if let (Some(Some(paid)), Some(at)) = (owed, at) {
            let name = ctx.place_name_of(&at);
            self.hold_next(&thread_id, paid, Some(&name));
        }

        loop {
            let Some(running) = ctx.running_on(&thread_id) else {
                let ending = self.stops.borrow().ending.get(&thread_id).cloned();
                if let Some(stopping) = ending {
                    let box_name = self.stops.borrow().waits_for.get(&thread_id).cloned();
                    if st.saying.outcome.get() == SessionStartOutcome::Started || (box_name.is_some() && !st.to_box) {
                        self.say_queued(st, box_name.as_deref())?;
                        st.to_box = box_name.is_some();
                    }
                    st.saying.outcome.set(SessionStartOutcome::Queued);
                    if box_name.is_some() {
                        self.hold(st);
                    }
                    let waiting_row = if box_name.is_none() { None } else { ctx.sessions().borrow().get(&turn_id).cloned() };
                    if let Some(row) = &waiting_row {
                        row.borrow_mut().view.waits_for = box_name.clone();
                    }
                    let holder = if box_name.is_none() || waiting_row.is_some() {
                        None
                    } else {
                        ctx.launching_on(&thread_id).and_then(|r| r.borrow().launch.clone())
                    };
                    match box_name {
                        None => {
                            stopping.await;
                            tokio::task::yield_now().await;
                        }
                        Some(name) => {
                            let (give_tx, give_rx) = oneshot::channel::<String>();
                            let give_tx = RefCell::new(Some(give_tx));
                            let give_up: Rc<dyn Fn() -> String> = Rc::new(move || {
                                let line = send_given_up_line(&name);
                                let tx = give_tx.borrow_mut().take();
                                if let Some(tx) = tx {
                                    let _ = tx.send(line.clone());
                                }
                                line
                            });
                            self.stops.borrow_mut().held_sends.insert(turn_id.clone(), HeldSend { thread_id: thread_id.clone(), give_up });
                            let waited = async move {
                                match holder {
                                    Some(holder) => {
                                        let _ = select(stopping, holder).await;
                                    }
                                    None => {
                                        stopping.await;
                                    }
                                }
                            };
                            let given = select(Box::pin(waited), give_rx).await;
                            self.stops.borrow_mut().held_sends.shift_remove(&turn_id);
                            if let Some(row) = &waiting_row {
                                row.borrow_mut().view.waits_for = None;
                            }
                            if let Either::Right((Ok(line), _)) = given {
                                return Err(HostError::new(line));
                            }
                            tokio::task::yield_now().await;
                        }
                    }
                    refuse_now(&ctx, &ws, &entry)?;
                    st.cleared = false;
                    continue;
                }
                if let Some(launching) = ctx.launching_on(&thread_id).filter(|r| r.borrow().turn_id != turn_id) {
                    let (launching_turn, launch) = {
                        let r = launching.borrow();
                        (r.turn_id.clone(), r.launch.clone())
                    };
                    if let Some(lender) = st.lender.clone() {
                        ctx.cap_lend(&launching_turn, &lender);
                    }
                    if st.to_box || (st.saying.outcome.get() == SessionStartOutcome::Started && !adapter.steers()) {
                        self.say_queued(st, None)?;
                    }
                    if !adapter.steers() {
                        st.saying.outcome.set(SessionStartOutcome::Queued);
                    }
                    st.to_box = false;
                    if let Some(launch) = launch {
                        launch.await;
                    }
                    refuse_now(&ctx, &ws, &entry)?;
                    st.cleared = false;
                    continue;
                }
                if !st.cleared {
                    st.cleared = true;
                    ctx.copy_blocked(&entry).await?;
                    ctx.worktree_mounted(&entry).await?;
                    ctx.kept_check(&thread_id).await?;
                    continue;
                }
                if let Some(writing) = st.resume.clone().and_then(|session| ctx.host_writes(&session)) {
                    writing.await;
                    refuse_now(&ctx, &ws, &entry)?;
                    continue;
                }
                self.hold(st);
                if !st.through {
                    let cap = ctx.cap_held(&turn_id);
                    if let Some(stops) = cap.as_ref().map(|c| c.stops.clone()).filter(|stops| !stops.is_empty()) {
                        for stop in stops {
                            stop.await;
                        }
                        refuse_now(&ctx, &ws, &entry)?;
                        st.cleared = false;
                        continue;
                    }
                    if let Some(stopped) = cap.as_ref().and_then(|c| c.stopped.clone()) {
                        return Err(HostError::new(stopped));
                    }
                    let full = if o.wakes_lead { None } else { ctx.cap_full(&record_of(&entry), &turn_id) };
                    if let Some(full) = full {
                        st.saying.hush();
                        let session = st.resume.clone().unwrap_or_else(|| turn_id.clone());
                        let waits =
                            ctx.cap_wait(cap_at(&ws, &thread_id, &turn_id, &session, &o.prompt, o.request_id.clone()), full.clone());
                        if !st.to_held {
                            st.to_held = true;
                            if let Some(on_held) = o.on_held.clone() {
                                let mut view = st.row.borrow().view.clone();
                                view.capped = Some(full.clone());
                                on_held(HeldStart { view, turn_id: turn_id.clone(), wait: full.clone() });
                            }
                        }
                        waits.await;
                        refuse_now(&ctx, &ws, &entry)?;
                        st.cleared = false;
                        continue;
                    }
                    st.through = true;
                    ctx.cap_left(&turn_id);
                }
                if st.landed {
                    break;
                }
                st.landed = true;
                let ran_in = match st.resume.clone() {
                    Some(session) => ctx.folder_of(&ws, &session),
                    None => None,
                };
                let landing = ctx.runs_in(&entry, ran_in.as_deref(), &st.folder);
                let attachments = o.attachments.clone().unwrap_or_default();
                let images_in: Vec<Attachment> = attachments.iter().filter(|a| is_image(&a.media_type)).cloned().collect();
                let landed = ctx
                    .land_images(
                        &entry,
                        adapter.as_ref(),
                        &landing,
                        &turn_images_dir(&landing, &thread_id, o.request_id.as_deref(), &st.minted),
                        images_in,
                    )
                    .await?;
                st.images = landed.images;
                st.images_dir = landed.dir;
                let files: Vec<Attachment> = attachments.iter().filter(|a| !is_image(&a.media_type)).cloned().collect();
                st.file_paths = ctx
                    .land_files(&entry, &landing, &send_files_dir(&landing, &thread_id, o.request_id.as_deref(), &st.minted), files)
                    .await?;
                if !st.file_paths.is_empty() {
                    st.files_folder = Some(landing.clone());
                }
                if let Some(plan) = st.p.plan.clone() {
                    if let (Some(fork_turn), Some(session)) = (st.p.fork_turn.clone(), plan.source.session.clone()) {
                        let readied = ctx.ready_fork(adapter.clone(), &session, fork_turn, &harness).await?;
                        if let ForkedSession::Resume { resume: copied, .. } = &readied {
                            st.resume = Some(copied.clone());
                        }
                        st.readied = Some(readied);
                    }
                    ctx.copy_images(&plan, &ws, &thread_id).await?;
                }
                if !st.p.opens {
                    if let Some(session) = st.resume.clone() {
                        st.resume = Some(ctx.pay_cut_owed(adapter.clone(), &ws, &thread_id, &session).await);
                    }
                }
                let taking = ctx.snapshot_of(&entry, &landing);
                if adapter.mcp_servers() || adapter.denies_servers() {
                    st.launch_servers = ctx.launch_servers_for(&entry, &harness, adapter.as_ref(), &landing).await?;
                }
                st.snapshot = if adapter.waits_for_prompt() {
                    Some(TurnSnapshot { from: SnapshotFrom::Taking(taking), cwd: landing.clone() })
                } else {
                    taking.await.map(|commit| TurnSnapshot { from: SnapshotFrom::Known(Some(commit)), cwd: landing.clone() })
                };
                refuse_now(&ctx, &ws, &entry)?;
                continue;
            };

            // A turn that has already answered takes no message; a steering harness steers into one that has not.
            let steer_id = ctx.ids().uuid();
            let (reply_seen, handle) = {
                let r = running.borrow();
                (r.turn_live.as_ref().is_some_and(|t| t.reply.is_some()), r.handle.clone())
            };
            let steer_road =
                !reply_seen && adapter.steers() && steer_files_blocked(&st.p.records, adapter.steers_images(), &harness).is_none();
            if let (true, Some(handle)) = (steer_road, handle.clone()) {
                ctx.keep_steer(&running, &o, origin.as_ref(), &steer_id).await?;
                let answer = match handle.steer(
                    o.prompt.clone(),
                    Some(steer_id.clone()),
                    inline_images(o.attachments.as_deref().unwrap_or_default()),
                ) {
                    Some(steer) => Some(steer.await),
                    None => None,
                };
                if let Some(answer) = answer {
                    if let Err(e) = &answer {
                        if handle.tells_unread() != Some(true) {
                            ctx.steer_lost(&running, &steer_id);
                            return Err(e.clone());
                        }
                    }
                    let accepted = !matches!(answer, Ok(SteerTaken::NotRunning));
                    if ctx.steer_answered(&running, &handle.id(), &o, origin.as_ref(), &steer_id, accepted) {
                        return Ok(Rc::new(Decorated { inner: handle, outcome: Some(SessionStartOutcome::Steered), notice: None }));
                    }
                }
            }
            if st.saying.outcome.get() == SessionStartOutcome::Started || st.to_box {
                self.say_queued(st, None)?;
            }
            st.saying.outcome.set(SessionStartOutcome::Queued);
            st.to_box = false;
            let (wake_tx, wake_rx) = oneshot::channel::<()>();
            let wake_tx = RefCell::new(Some(wake_tx));
            let wake: Rc<dyn Fn()> = Rc::new(move || {
                let tx = wake_tx.borrow_mut().take();
                if let Some(tx) = tx {
                    let _ = tx.send(());
                }
            });
            let wake_id = Rc::as_ptr(&wake) as *const () as usize as u64;
            self.stops.borrow_mut().behind_turn.entry(thread_id.clone()).or_default().push((wake_id, wake.clone()));
            let finished: LocalBoxFuture<'static, ()> = match handle {
                Some(handle) => handle.finished().map(|_| ()).boxed_local(),
                None => futures::future::pending::<()>().boxed_local(),
            };
            let _ = select(finished, wake_rx).await;
            {
                let mut stops = self.stops.borrow_mut();
                if let Some(sends) = stops.behind_turn.get_mut(&thread_id) {
                    sends.retain(|(id, _)| *id != wake_id);
                    if sends.is_empty() {
                        stops.behind_turn.shift_remove(&thread_id);
                    }
                }
            }
            refuse_now(&ctx, &ws, &entry)?;
            st.cleared = false;
        }

        self.handover(st).await
    }

    /// sessions.ts:742-848: the picks and the folder the turn runs at, then `run_turn`, the record and the settle.
    async fn handover(&self, st: &mut Run) -> Result<Rc<dyn SessionHandle>, HostError> {
        let ctx = self.ctx.clone();
        let ws = st.p.workspace_id.clone();
        let thread_id = st.p.thread_id.clone();
        let turn_id = st.turn_id.clone();
        let entry = st.p.entry.clone();
        let harness = st.p.built.harness.clone();
        let adapter: Arc<dyn HarnessAdapter> = st.p.built.adapter.clone();
        let o = st.p.opts.clone();
        let outside = o.outside.clone();
        let ran_in = match st.resume.clone() {
            Some(session) => ctx.folder_of(&ws, &session),
            None => None,
        };
        let cwd = ctx.runs_in(&entry, ran_in.as_deref(), &st.folder);
        let session_id = st.resume.clone().unwrap_or_else(|| turn_id.clone());
        if let Some(text) = o.behind.clone() {
            ctx.record(session_event(json!({ "type": "session.behind", "workspaceId": ws, "sessionId": session_id, "turnId": turn_id, "threadId": thread_id, "text": text }))?);
        }
        if let Some(text) = o.setup.clone() {
            ctx.record(session_event(json!({ "type": "session.behind", "workspaceId": ws, "sessionId": session_id, "turnId": turn_id, "threadId": thread_id, "text": text }))?);
        }
        let record = record_of(&entry);
        let moved_from = ran_in.clone().or_else(|| st.p.gone_now.clone());
        if let Some(from) = moved_from {
            let gone = record.worktree.as_ref().is_some_and(|w| w.gone.is_some());
            if cwd != from && gone {
                let fresh = ran_in.is_some()
                    && wsp_catalog::catalog::catalog_agents()
                        .iter()
                        .find(|a| a.id == harness)
                        .map_or(true, |a| a.project_key_env.is_none());
                let mut doc = json!({ "type": "session.moved", "workspaceId": ws, "sessionId": session_id, "turnId": turn_id, "threadId": thread_id, "from": from, "to": cwd });
                if let Some(branch) = record.worktree.as_ref().and_then(|w| w.branch.clone()) {
                    doc["branch"] = json!(branch);
                }
                if fresh {
                    doc["fresh"] = json!(true);
                }
                ctx.record(session_event(doc)?);
                if fresh {
                    st.resume = None;
                    st.row.borrow_mut().view.claude_session_id = None;
                }
            }
        }
        let picks_for = st.picks_for.take().ok_or_else(|| HostError::new("the picks were not read"))?;
        let picks = picks_for(if outside.is_some() { None } else { st.resume.as_deref() })?;
        let after_cut = st.resume.is_some() && ctx.cut_before(&ws, &thread_id);
        {
            let mut row = st.row.borrow_mut();
            row.view.model = picks.picks.model.clone();
            row.view.effort = picks.picks.effort.clone();
            row.view.permission_mode = picks.picks.permission_mode.clone();
            row.view.context_window = picks.context_window.clone();
            row.view.fast = picks.picks.fast;
            row.view.cwd = Some(cwd.clone());
            row.view.claude_session_id = st.resume.clone();
        }
        let handed = attached_files_prompt(&o.prompt, &st.file_paths);
        let fork_on_start = match st.readied.as_ref() {
            Some(ForkedSession::Fork { session, turn }) => Some((session.clone(), turn.clone())),
            _ => None,
        };
        let launch_key: Option<KeptLaunch> = if ctx.keeps_agents(&entry) && fork_on_start.is_none() {
            let mcp = o.mcp_servers.as_ref().map(|servers| serde_json::to_value(servers).unwrap_or(Value::Null));
            let version = st.catalog_version.as_ref().map(|v| v.as_deref());
            let fixed = ctx.launch_area().launch_fixed(&entry, &harness, &cwd, mcp.as_ref(), version);
            Some(KeptLaunch { fixed, picks: picks_map(&picks).into_iter().collect() })
        } else {
            None
        };
        let asked = asked_map(&o, &picks);
        let kept = ctx.take_kept(
            &thread_id,
            launch_key.as_ref().map(|k| KeptLaunch { fixed: k.fixed.clone(), picks: asked.clone().into_iter().collect() }),
            st.resume.as_deref(),
            launch_key.as_ref(),
        );
        if kept.is_some() {
            drop_scope(&ctx, st.p.launch.scoped_device_id.as_deref(), &thread_id);
        }
        let prompt_after: Option<Settle> = match &st.snapshot {
            Some(TurnSnapshot { from: SnapshotFrom::Taking(taking), .. }) => {
                Some(taking.clone().map(|_| Ok::<(), String>(())).boxed_local())
            }
            _ => None,
        };
        st.kept_taken = kept.clone();
        if let Some(plan) = st.p.plan.clone() {
            ctx.record_copies(&plan, &ws, &thread_id);
        }
        if let Some(outside) = outside.as_ref() {
            for row in &outside.earlier {
                let mut doc = serde_json::to_value(row).map_err(|e| HostError::new(e.to_string()))?;
                if let Value::Object(map) = &mut doc {
                    map.insert("type".into(), json!("session.earlier"));
                    map.insert("workspaceId".into(), json!(ws));
                    map.insert("sessionId".into(), json!(outside.id));
                    map.insert("threadId".into(), json!(thread_id));
                }
                ctx.record(session_event(doc)?);
            }
        }
        let opening = Opening {
            prompt: o.prompt.clone(),
            request_id: o.request_id.clone(),
            via_slate: o.via_slate,
            after_cut,
            after_limit: o.after_limit,
            opens_thread: st.p.opens,
            title: st.p.title.clone(),
            attachments: if st.p.records.is_empty() { None } else { Some(st.p.records.clone()) },
        };
        st.row.borrow_mut().asked = Some(TurnAsked {
            prompt: handed.clone(),
            effort: picks.picks.effort.clone(),
            typed: if handed != o.prompt { Some(o.prompt.clone()) } else { None },
            request_id: o.request_id.clone(),
            owed: if o.owed { Some(true) } else { None },
            awaits_prompt: None,
        });
        let turn_token = Some(match kept.as_ref() {
            Some(kept) => kept.borrow().turn_token.clone(),
            None => st.p.turn_token.clone(),
        });
        let scope_device = match kept.as_ref() {
            Some(kept) => kept.borrow().scope_device_id.clone(),
            None => st.p.launch.scoped_device_id.clone(),
        };
        let waiting = match kept.as_ref() {
            Some(kept) => kept.borrow().waiting.clone(),
            None => st.p.waiting.clone(),
        };
        let keep: Option<KeptLaunch> = launch_key.as_ref().map(|k| match kept.as_ref() {
            Some(kept) => kept.borrow().launch.clone(),
            None => k.clone(),
        });
        let has_seed = st.resume.is_some();
        let seed = if has_seed { Some(seed_for(&ctx, &ws, &thread_id, &turn_id)) } else { None };
        let images = st.images.clone();
        let copy = outside.as_ref().is_some_and(|out| out.copy);
        let open: Open = match kept.clone() {
            Some(kept) => Box::new(move |on_event: OnEvent| {
                let turn = KeptTurn { prompt: handed, images, after: prompt_after, on_event };
                let result = kept.borrow().agent.next(turn);
                result.map_err(adapter_error)
            }),
            None => {
                let start = HarnessStartOptions {
                    prompt: handed,
                    resume: if copy { outside.as_ref().map(|out| out.id.clone()) } else { st.resume.clone() },
                    copy,
                    cwd: Some(cwd.clone()),
                    model: picks.picks.model.clone(),
                    effort: picks.picks.effort.clone(),
                    permission_mode: picks.picks.permission_mode.clone(),
                    context_window: picks.context_window.clone(),
                    fast: picks.picks.fast,
                    title: st.p.title.clone(),
                    images,
                    fork: fork_on_start.clone(),
                    mcp_servers: port_servers(st.p.mcp_servers.as_ref()),
                    server_values: st.launch_servers.server_values.clone(),
                    off_servers: st.launch_servers.off_servers.clone(),
                    seed,
                    limit_details: st.limit_details,
                    prompt_after,
                    version: st.harness_version.clone(),
                    keep: launch_key.is_some(),
                    on_event: None,
                };
                let adapter = adapter.clone();
                Box::new(move |on_event: OnEvent| {
                    let mut start = start;
                    start.on_event = Some(on_event);
                    adapter.start(start).map_err(adapter_error)
                })
            }
        };
        let handle = ctx.run_turn(TurnStart {
            entry: entry.clone(),
            row: st.row.clone(),
            written: None,
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            notify: st.p.notify.clone(),
            notify_by: st.p.notify_by.clone(),
            notify_road: st.p.notify_road.clone(),
            turn_token,
            scope_device_id: scope_device,
            outcome: st.saying.outcome.get(),
            opening,
            resume: st.resume.clone(),
            images_dir: st.images_dir.clone(),
            snapshot: st.snapshot.clone(),
            reports_edits: adapter.reports_edits(),
            waiting: Some(waiting),
            keep,
            open,
        })?;
        st.handed_over = true;
        if kept.is_none() {
            let said = SayingCall {
                workspace_id: ws.clone(),
                thread_id: thread_id.clone(),
                harness: harness.clone(),
                request_id: o.request_id.clone(),
            };
            tokio::task::spawn_local(say_starting(ctx.clone(), st.saying.clone(), entry.clone(), said));
        }
        {
            let (ctx2, ws2, thread2, request_id, attachments) =
                (ctx.clone(), ws.clone(), thread_id.clone(), o.request_id.clone(), o.attachments.clone().unwrap_or_default());
            tokio::task::spawn_local(async move {
                let _ = ctx2.keep_sent_images(&ws2, &thread2, request_id.as_deref(), attachments).await;
            });
        }
        if !ctx.records().borrow().contains_key(&thread_id) {
            let views: Vec<SessionView> = ctx.rows_on(&thread_id).into_iter().filter(|v| v.status != SessionStatus::Running).collect();
            let mut doc = Map::new();
            doc.insert("workspaceId".into(), json!(ws));
            doc.insert("harness".into(), json!(harness));
            set_opt(&mut doc, "parentThreadId", st.p.tree.parent_thread_id.clone().map(Value::from));
            set_opt(&mut doc, "rootThreadId", st.p.tree.root_thread_id.clone().map(Value::from));
            set_opt(&mut doc, "permissionMode", picks.picks.permission_mode.clone().map(Value::from));
            set_opt(&mut doc, "replaces", st.p.replaces.clone().map(Value::from));
            set_opt(&mut doc, "forkedFrom", st.p.plan.as_ref().and_then(|p| serde_json::to_value(&p.forked_from).ok()));
            doc.insert("worked".into(), json!(thread_ran(&views)));
            let record: ThreadRecord = serde_json::from_value(Value::Object(doc)).map_err(|e| HostError::new(e.to_string()))?;
            ctx.records().borrow_mut().insert(thread_id.clone(), record);
        }
        if let Some(clouds) = st.p.clouds.clone() {
            if let Some(record) = ctx.records().borrow_mut().get_mut(&thread_id) {
                record.clouds = Some(clouds);
            }
        }
        let limit_resume = {
            let records = ctx.records();
            let mut records = records.borrow_mut();
            match records.get_mut(&thread_id) {
                Some(record) if record.limit_resume.is_some() => {
                    record.limit_resume = None;
                    true
                }
                _ => false,
            }
        };
        if limit_resume {
            ctx.cancel_resume_timer(&thread_id);
        }
        if let Some(folder) = st.files_folder.clone() {
            if let Some(record) = ctx.records().borrow_mut().get_mut(&thread_id) {
                let files = record.files_in.get_or_insert_with(Vec::new);
                if !files.contains(&folder) {
                    files.push(folder);
                }
            }
        }
        if let Some(launched) = st.launched.borrow_mut().take() {
            let _ = launched.send(());
        }
        if let Some(replaces) = st.p.replaces.clone() {
            if let Err(e) = self.settle_threads(vec![replaces.clone()], None, SettleAsk::default()).await {
                console::warn(&format!("thread {} was not settled after its restart: {}", thread_word(&replaces), e.message));
            }
            ctx.push_head(&replaces);
        }
        if st.resume.is_none() {
            if let Err(e) = ctx.remember_target(&record_of(&entry)).await {
                console::warn(&format!("last target for {ws} not remembered: {}", e.message));
            }
        }
        Ok(match st.p.notice.clone() {
            None => handle,
            Some(notice) => Rc::new(Decorated { inner: handle, outcome: None, notice: Some(notice) }),
        })
    }

    /// The TS `finally` of `launchPrepared` (Z1-Z6), run after the body whether it ran or failed.
    async fn launch_finally(&self, st: &mut Run) {
        if st.handed_over {
            return;
        }
        let ctx = self.ctx.clone();
        let ws = st.p.workspace_id.clone();
        let thread_id = st.p.thread_id.clone();
        st.saying.hush();
        ctx.cap_left(&st.turn_id);
        drop_scope(&ctx, st.p.launch.scoped_device_id.as_deref(), &thread_id);
        if st.held {
            if let (true, Some(notify), Some(failure)) = (st.to_held && !st.p.opts.owed, st.p.notify.clone(), st.failure.clone()) {
                if let Ok(result) = serde_json::from_value::<TurnResult>(json!({ "status": "failed", "error": failure })) {
                    ctx.notify_end(&st.row, &notify, st.p.notify_by.as_ref(), st.p.notify_road.as_ref(), result);
                }
            }
            ctx.sessions().borrow_mut().shift_remove(&st.turn_id);
            if let Some(failure) = st.failure.clone() {
                if !ctx.thread_runs(&thread_id) {
                    let session_id = st.row.borrow().view.id.clone();
                    if let Ok(event) = union_event(json!({
                        "type": "session.end", "workspaceId": ws, "sessionId": session_id, "turnId": st.turn_id,
                        "threadId": thread_id, "exitCode": null, "sawResult": false, "reason": failure, "at": ctx.clock().now(),
                    })) {
                        ctx.emit(event);
                    }
                    if let (true, Some(before)) = (st.to_held && st.p.opts.on_held.is_some(), ctx.latest_on(&thread_id)) {
                        let listed = listed_failure(Some(&failure));
                        let rows: Vec<Row> = ctx.sessions().borrow().values().cloned().collect();
                        for row in rows {
                            let same = {
                                let r = row.borrow();
                                r.view.id == before.id && r.view.workspace_id == before.workspace_id
                            };
                            if same {
                                row.borrow_mut().view.failure = Some(listed.clone());
                                break;
                            }
                        }
                        let _ = ctx.persist_sessions(&before.workspace_id).await;
                    }
                }
            }
        }
        if let Some(launched) = st.launched.borrow_mut().take() {
            let _ = launched.send(());
        }
        if let Some(dir) = st.images_dir.clone() {
            ctx.drop_images(&st.p.entry, &dir);
        }
        if let Some(plan) = st.p.plan.clone() {
            if let Some(ForkedSession::Resume { drop: drop_copy, .. }) = st.readied.take() {
                let copy = drop_copy();
                let source = thread_word(&plan.source.thread_id);
                tokio::task::spawn_local(async move {
                    if let Err(why) = copy.await {
                        console::warn(&format!("the copy a fork of thread {source} made was not removed: {why}"));
                    }
                });
            }
            let (ctx2, ws2, thread2) = (ctx.clone(), ws.clone(), thread_id.clone());
            tokio::task::spawn_local(async move {
                let _ = ctx2.drop_sent_images(&ws2, &[thread2]).await;
            });
        }
        if let Some(kept) = st.kept_taken.take() {
            ctx.end_kept(&thread_id, &kept);
        }
    }
}
