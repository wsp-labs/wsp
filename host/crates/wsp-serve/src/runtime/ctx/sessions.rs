// SPDX-License-Identifier: AGPL-3.0-only
//! What the sessions area asks of the runtime (`SessionsCtx`, sessions.ts:41-1498; runtime.ts:470): the core's own
//! maps, clock and bus, a forward to the area that holds each call, and the few lines no area has (a thread's row
//! looked up by the id an area answers, the held start's options as the queue writes them, the rename's write).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt as _;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_account::account::core_types::Entry as AccountEntry;
use wsp_account::account::rules::{like_of, TreeOf};
use wsp_adapters::port::{AsideQuestion, ForkTurn, ForkedSession, HarnessAdapter, ServerValues, SessionRenameWrite};
use wsp_core::clock::{Cancel, Clock};
use wsp_core::context::Joined;
use wsp_core::types::api::{DeleteThreadAsk, ForkOptions, StartOptions, SteerAsk, ThreadDeleted};
use wsp_core::types::internal::{EndThread, KeptLaunch, KeptProcess, SearchedHit, SubStartOpts, Taken, ThreadRecord, TreeTalk};
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_machines::machines::boot::entries::{EntryAsk, RowAt};
use wsp_machines::machines::boot::rows::Stamped;
use wsp_machines::machines::sub_queue::{HeldTold, HoldAsk};
use wsp_machines::machines::sub_spawn::machine::Resumed;
use wsp_machines::places::types::{PlaceEvent, NO_PLACE_DOOR};
use wsp_store::transcript_reader::ThreadRead;
use wsp_threads::activity::ActivityDoor;
use wsp_threads::threads::agents::launch::{AdapterAsk, BuiltAdapter, LandedImages};
use wsp_threads::threads::agents::AgentsArea;
use wsp_threads::threads::forks::{self, ForkPlan, ForksCtx, StartFn};
use wsp_threads::threads::launch::LaunchArea;
use wsp_threads::threads::sessions::{SessionsCtx, StampsOf};
use wsp_threads::threads::threads::cap::CapAt;
use wsp_threads::threads::threads::tree::TellAs;
use wsp_threads::threads::threads::turn_end::SteerSent;
use wsp_threads::threads::threads::{CapHeld, Entry, NotifyOn, Row, Sessions, ThreadRecords};
use wsp_threads::threads::transcripts::kept::ThreadsNamed;
use wsp_threads::threads::turns::TurnStart;
use wsp_threads::threads::waits::WaitsArea;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::preferences::{HistoryPage, ThreadFacts};
use wsp_wire::views::session::{SessionView, ThreadCapWait, TitleSource};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin, WorktreeSettledWhy};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;
use wsp_words::place_word::provider_key_name;

use super::Ctx;
use crate::runtime::join::spawn_detached;

/// `placeAway` (runtime.ts:853): a computer this host holds no backend and no link to, where it has a door at all.
fn sessions_place_away(ctx: &Ctx, place: &str) -> bool {
    ctx.wired_door().is_some_and(|door| ctx.places.backend(place).is_none() && door.link(place).is_none())
}

/// `ctx.placeName` (builders.ts:242): the door's name for the place, else this computer's hostname or the
/// provider's own name.
fn sessions_place_name(ctx: &Ctx, place: &str) -> String {
    match ctx.wired_door() {
        Some(door) => door.name_of(place),
        None if place == HERE_PLACE_ID => wsp_core::host_id::hostname(),
        None => provider_key_name(place),
    }
}

/// The word an act is written as, read as the enum the rules area takes: a word the table lacks is no act.
fn sessions_act<T: serde::de::DeserializeOwned>(word: &str) -> Result<T, HostError> {
    serde_json::from_value(Value::String(word.to_string())).map_err(|_| HostError::new(format!("{word} is no act a thread is refused for")))
}

/// A steered message as the threads area keeps it.
fn sessions_sent(o: &StartOptions) -> SteerSent {
    SteerSent {
        prompt: o.prompt.clone(),
        request_id: o.request_id.clone(),
        via_slate: o.via_slate,
        started_by: o.started_by,
        attachments: o.attachments.clone().unwrap_or_default(),
    }
}

/// What of a start rides a held record, as the object the queue reads (sub-queue.ts:29).
fn sessions_held_options(o: &StartOptions) -> Map<String, Value> {
    let opts = SubStartOpts {
        prompt: o.prompt.clone(),
        harness: o.harness.clone(),
        model: o.model.clone(),
        effort: o.effort.clone(),
        permission_mode: o.permission_mode.clone(),
        access: o.access,
        context_window: o.context_window.clone(),
        fast: o.fast,
        started_by: o.started_by,
        request_id: o.request_id.clone(),
        attempt: o.attempt.clone(),
        title: o.title.clone(),
        cwd: o.cwd.clone(),
        attachments: o.attachments.clone(),
    };
    match serde_json::to_value(opts) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// The engine's `HarnessExec` as the adapters take it: a command and its variables in, stdout out.
struct SessionsExec(wsp_engine::run_env::HarnessExec);

impl wsp_adapters::port::HarnessExec for SessionsExec {
    fn exec(
        &self,
        command: &str,
        env: Option<&wsp_adapters::port::Env>,
    ) -> LocalBoxFuture<'static, Result<String, wsp_adapters::port::ExecFailure>> {
        let run = (self.0)(command.to_string(), env.cloned());
        async move { run.await.map_err(|e| wsp_core::machine_exec::factory::exec_failure_of(&e)) }.boxed_local()
    }
}

impl Ctx {
    fn sessions_row_by_id(&self, row_id: &str) -> Option<Row> {
        self.sessions.borrow().get(row_id).cloned()
    }

    /// The reads of the index a workspace holds, none for a workspace with no index.
    fn sessions_in_index<R>(
        &self,
        workspace_id: &str,
        read: impl FnOnce(&wsp_core::types::internal::TranscriptIndex) -> Option<R>,
    ) -> Option<R> {
        self.transcript_state.with(|s| s.index.get(workspace_id).and_then(read))
    }
}

#[async_trait(?Send)]
impl SessionsCtx for Ctx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn sessions(&self) -> Sessions {
        self.sessions.clone()
    }
    fn records(&self) -> ThreadRecords {
        self.thread_records.clone()
    }
    fn live_entry(&self, workspace_id: &str) -> Option<Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.agents.get().rows_on(thread_id)
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        self.threads.get().thread_runs(thread_id)
    }
    fn running_on(&self, thread_id: &str) -> Option<Row> {
        let (row_id, _) = self.threads.get().running_on(thread_id)?;
        self.sessions_row_by_id(&row_id)
    }
    fn launching_on(&self, thread_id: &str) -> Option<Row> {
        let (turn_id, _) = self.threads.get().launching_on(thread_id)?;
        self.sessions
            .borrow()
            .values()
            .find(|row| {
                let row = row.borrow();
                row.turn_id == turn_id && row.view.thread_id.as_deref() == Some(thread_id)
            })
            .cloned()
    }
    async fn entry_of_row(
        &self,
        at: &RowAt,
        origin: Option<&Caller>,
        talk: Option<TreeTalk>,
        being_made: bool,
    ) -> Result<Option<Entry>, HostError> {
        self.boot.get().entry_of_row(at, origin, talk, being_made).await
    }
    async fn entry_of_with(&self, workspace_id: &str, origin: Option<&Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        self.boot.get().entry_of(workspace_id, origin, ask).await
    }
    fn record(&self, event: SessionEvent) {
        self.transcripts.get().record(event)
    }
    fn emit(&self, event: EventUnion) {
        if let Err(e) = self.bus.emit(event) {
            wsp_core::console::warn(&format!("a listener failed on an event a session sent: {}", e.message));
        }
    }
    fn pass(&self, event: EventUnion) {
        if let Err(e) = self.bus.pass(event) {
            wsp_core::console::warn(&format!("a listener failed on an event a session passed: {}", e.message));
        }
    }
    fn on_turn_settled(&self, turn_id: &str, f: Rc<dyn Fn()>) -> Rc<dyn Fn()> {
        let turn = turn_id.to_string();
        let off = self.hear("*", move |event| {
            let Ok(value) = serde_json::to_value(event) else { return };
            let settles = matches!(value["type"].as_str(), Some("session.start" | "session.end"));
            if settles && value["turnId"].as_str() == Some(turn.as_str()) {
                f();
            }
        });
        Rc::from(off)
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.transcripts.get().persist_sessions(workspace_id)
    }
    fn push_head(&self, thread_id: &str) {
        if let Err(e) = self.boot.get().push_head(thread_id) {
            wsp_core::console::warn(&format!("the head of thread {thread_id} was not pushed: {}", e.message));
        }
    }
    fn agent_label(&self, harness: &str) -> String {
        AgentsArea::agent_label(harness)
    }
    fn waits(&self) -> WaitsArea {
        (*self.waits.get()).clone()
    }
    fn activity(&self) -> Rc<dyn ActivityDoor> {
        self.activity.get()
    }

    // sessions_stop
    fn place_away(&self, place: &str) -> bool {
        sessions_place_away(self, place)
    }
    fn has_link(&self, place: &str) -> bool {
        self.wired_door().is_some_and(|door| door.link(place).is_some())
    }
    async fn ping(&self, place: &str) -> Result<(), HostError> {
        let door = self.wired_door().ok_or_else(|| HostError::new(NO_PLACE_DOOR))?;
        let link =
            door.link(place).ok_or_else(|| HostError::new(format!("{} has no link to this host", sessions_place_name(self, place))))?;
        link.request("ping", Map::new()).await.map(|_| ()).map_err(|e| HostError::new(e.message))
    }
    fn on_place_absent(&self, place: &str, f: Rc<dyn Fn()>) -> Rc<dyn Fn()> {
        let Some(door) = self.wired_door() else { return Rc::new(|| {}) };
        let place = place.to_string();
        let id = door.on(Rc::new(move |event: &PlaceEvent| {
            if let PlaceEvent::Absent(absent) = event {
                if absent.place_id == place {
                    f();
                }
            }
        }));
        Rc::new(move || door.off(id))
    }
    fn place_name_of(&self, place: &str) -> String {
        sessions_place_name(self, place)
    }
    fn workspace_place(&self, record: &WorkspaceRecord) -> Option<String> {
        record.place.clone()
    }
    fn cap_held(&self, turn_id: &str) -> Option<CapHeld> {
        self.threads.get().cap_held.borrow().get(turn_id).cloned()
    }
    fn cap_stop(&self, turn_id: &str, line: &str) {
        self.threads.get().cap_stop(turn_id, line);
    }
    fn cap_stopping(&self, row_id: &str) -> Rc<dyn Fn()> {
        Rc::from(self.threads.get().cap_stopping(row_id))
    }
    async fn drop_queued(&self, turn_id: &str) -> Result<bool, HostError> {
        Ok(self.sub_queue.get().drop_queued(turn_id).await)
    }
    async fn stop_under(&self, thread_id: &str, origin: Option<&Caller>) -> Result<Vec<String>, HostError> {
        Ok(self.threads.get().stop_under(thread_id, origin.cloned()).await)
    }
    fn try_stopped(&self, turn_id: &str) {
        self.threads.get().try_stopped(turn_id)
    }
    fn line_try(&self, row: &Row) -> bool {
        let row_id = row.borrow().view.id.clone();
        self.threads.get().line_try(&row_id)
    }
    fn end_thread(
        &self,
        entry: &Entry,
        thread_id: &str,
        o: EndThread,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, HostError>>> {
        let kind = entry.borrow().record.borrow().kind;
        let module = self.kinds.get().module_of(kind).ok()?;
        let live = entry.borrow();
        module.end_thread(&live, thread_id, o)
    }
    async fn end_owed(&self, entry: &Entry, thread_id: &str) -> Result<Option<Option<Joined>>, HostError> {
        let kind = entry.borrow().record.borrow().kind;
        let module = self.kinds.get().module_of(kind)?;
        let owed = {
            let live = entry.borrow();
            module.end_owed(&live, thread_id)
        };
        let Some(owed) = owed else { return Ok(None) };
        let paid = owed.await?;
        Ok(Some(paid.map(|paid| paid.map(|_| ()).boxed_local().shared())))
    }

    // sessions_start
    fn taken(&self, workspace_id: &str, request_id: &str) -> Option<Taken> {
        self.sessions_in_index(workspace_id, |index| index.taken.get(request_id).cloned())
    }
    async fn taken_turn(&self, workspace_id: &str, taken: Taken) -> Result<Option<Rc<dyn SessionHandle>>, HostError> {
        self.threads.get().taken_turn(workspace_id, &taken).await
    }
    async fn fork_opened(
        &self,
        workspace_id: &str,
        opened: StartOptions,
        origin: Option<&Caller>,
    ) -> Result<(StartOptions, Option<ForkPlan>), HostError> {
        let ctx: Rc<dyn ForksCtx> = self.rc();
        forks::fork_opened(&ctx, workspace_id, opened, origin.cloned()).await
    }
    fn claim_warm(&self, workspace_id: &str, harness: &str) -> Option<String> {
        self.threads.get().claim_warm(workspace_id, harness)
    }
    fn opens_in(&self, record: &WorkspaceRecord, origin: Option<&Caller>) -> Result<bool, HostError> {
        self.rules.get().opens_in(record, origin)
    }
    fn of_threads_repository(&self, origin: Option<&Caller>, project: &str) -> Result<bool, HostError> {
        self.rules.get().of_threads_repository(origin, project)
    }
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &str, name: &str) -> Result<Option<HostError>, HostError> {
        let held = self.channels.get().project_held(project)?;
        self.rules.get().elsewhere_refusal(origin, &held, name)
    }
    fn away_for(
        &self,
        record: &WorkspaceRecord,
        origin: Option<&Caller>,
        act: &str,
        workspace_id: &str,
    ) -> Result<Option<HostError>, HostError> {
        self.rules.get().away_for(&like_of(record), origin, sessions_act(act)?, workspace_id)
    }
    fn acts_on(&self, record: &WorkspaceRecord, origin: Option<&Caller>) -> bool {
        self.rules.get().acts_on(&like_of(record), origin)
    }
    fn project_computer_name(&self, project: &str) -> Result<String, HostError> {
        let held = self.channels.get().project_held(project)?;
        Ok(sessions_place_name(self, &held.computer))
    }
    async fn worktree_gone(&self, entry: &Entry, why: &str, starting: bool) -> Result<(), HostError> {
        let why: WorktreeSettledWhy = sessions_act(why)?;
        self.folders.get().worktree_gone(entry, why, starting).await
    }
    fn computer_of(&self, entry: &Entry) -> String {
        self.channels.get().computer_of(entry)
    }
    fn refuse_sub_send(&self, entry: &Entry) -> Result<(), HostError> {
        self.sub_life.get().refuse_sub_send(entry)
    }
    async fn proven(&self, entry: &Entry) -> Result<(), HostError> {
        self.machines.get().proven(entry).await.map(|_| ())
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        self.channels.get().copy_blocked(entry).await
    }
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> {
        self.folders.get().worktree_mounted(entry).await
    }
    async fn kept_check(&self, thread_id: &str) -> Result<(), HostError> {
        self.threads.get().kept_check(thread_id).await;
        Ok(())
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.rules.get().spawn_guard(sessions_act(act)?, origin).map(|_| ())
    }
    fn parent_of(&self, thread_id: &str) -> Option<String> {
        self.rules.get().parent_of(thread_id)
    }
    fn root_of(&self, thread_id: &str) -> String {
        self.rules.get().root_of(thread_id)
    }
    fn tree_of(&self, scope: Option<&ThreadScope>) -> TreeOf {
        self.rules.get().tree_of(scope)
    }
    fn thread_of_token(&self, token: &str) -> Result<String, HostError> {
        self.threads.get().thread_of_token(token)
    }
    fn reaches_row(&self, view: &SessionView, origin: Option<&Caller>, talk: Option<TreeTalk>) -> bool {
        let at = RowAt { thread_id: view.thread_id.clone(), workspace_id: view.workspace_id.clone() };
        self.boot.get().reaches_row(&at, origin, talk).unwrap_or(false)
    }
    fn notify_reach(&self, notify: &[String]) -> IndexSet<String> {
        self.threads.get().notify_reach(notify)
    }
    fn notify_on(&self, thread_id: &str) -> Option<NotifyOn> {
        self.threads.get().notify_on(thread_id)
    }
    async fn resume_sub(&self, entry: &Entry, thread_id: &str, harness: &str, prompt: &str) -> Result<Resumed, HostError> {
        self.sub_spawn.get().resume_sub(entry, thread_id, harness, Some(prompt)).await
    }
    async fn place_rows(&self) -> Option<Vec<wsp_wire::views::place_more::PlaceView>> {
        let door = self.wired_door()?;
        door.rows().await.ok()
    }
    async fn hold_queued(
        &self,
        workspace_id: &str,
        thread_id: &str,
        tree: TreeOf,
        opts: StartOptions,
        notify: Option<Vec<String>>,
        origin: Option<&Caller>,
    ) -> Result<Rc<dyn SessionHandle>, HostError> {
        let mut roots: IndexMap<String, Value> = IndexMap::new();
        if let Some(parent) = tree.parent_thread_id {
            roots.insert("parentThreadId".into(), Value::String(parent));
        }
        if let Some(root) = tree.root_thread_id {
            roots.insert("rootThreadId".into(), Value::String(root));
        }
        let on_held = opts.on_held.clone().map(|held| -> Rc<dyn Fn(HeldTold)> {
            Rc::new(move |told: HeldTold| {
                held(wsp_core::types::api::HeldStart { view: told.view, turn_id: told.turn_id, wait: told.wait });
            })
        });
        let ask = HoldAsk {
            workspace_id: workspace_id.to_string(),
            thread_id: thread_id.to_string(),
            tree: roots,
            o: sessions_held_options(&opts),
            on_held,
            notify,
        };
        let started = self.sub_queue.get().hold_queued(ask, origin).await?;
        let (held, outcome) = match self.sessions_row_by_id(&started.id) {
            Some(row) => (row.borrow().handle.clone(), wsp_core::types::internal::TakenOutcome::Started),
            None => (None, wsp_core::types::internal::TakenOutcome::Started),
        };
        if let Some(handle) = held {
            return Ok(handle);
        }
        let taken = Taken { session_id: started.id.clone(), thread_id: thread_id.to_string(), turn_id: started.turn_id.clone(), outcome };
        self.threads
            .get()
            .taken_turn(workspace_id, &taken)
            .await?
            .ok_or_else(|| HostError::new(format!("the turn {} the queue started is not held", started.turn_id)))
    }
    fn launch_area(&self) -> LaunchArea {
        (*self.launch.get()).clone()
    }
    fn fork_turn_of(&self, plan: &ForkPlan, by_count: bool) -> Result<Option<ForkTurn>, HostError> {
        forks::fork_turn_of(plan, by_count)
    }
    fn sub_done(&self, workspace_id: &str) {
        let area = self.sub_life.get();
        let workspace_id = workspace_id.to_string();
        spawn_detached(async move { area.sub_done(&workspace_id).await });
    }
    fn kept_warm(&self, thread_id: &str) -> bool {
        self.threads.get().kept_agents.borrow().get(thread_id).is_some_and(|kept| kept.borrow().warm.is_some())
    }
    async fn fork_start(&self, opts: ForkOptions, start: StartFn, origin: Option<&Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        let ctx: Rc<dyn ForksCtx> = self.rc();
        forks::fork_start(&ctx, start, opts, origin.cloned()).await
    }

    // sessions_launch
    fn carried_title(&self, thread_id: &str) -> (Option<String>, Option<TitleSource>) {
        let carried = self.agents.get().carried_title(thread_id);
        (carried.harness_title, carried.title_source)
    }
    fn cap_hold(&self, record: &WorkspaceRecord, turn_id: &str, lender: Option<&str>) {
        self.threads.get().cap_hold(record, turn_id, lender)
    }
    fn cap_full(&self, record: &WorkspaceRecord, turn_id: &str) -> Option<ThreadCapWait> {
        self.threads.get().cap_full(record, turn_id)
    }
    fn cap_wait(&self, ask: CapAt, full: ThreadCapWait) -> Joined {
        self.threads.get().cap_wait(ask, full).map(|_| ()).boxed_local().shared()
    }
    fn cap_left(&self, turn_id: &str) {
        self.threads.get().cap_left(turn_id)
    }
    fn cap_lend(&self, turn_id: &str, lender: &str) {
        self.threads.get().cap_lend(turn_id, lender)
    }
    async fn first_run_here(&self, entry: &Entry, harness: &str) -> Result<bool, HostError> {
        Ok(self.agents.get().first_run_here(entry, harness).await)
    }
    async fn limit_details_due(&self, entry: &Entry, harness: &str) -> Result<bool, HostError> {
        Ok(self.threads.get().limit_details_due(entry, harness).await)
    }
    fn host_writes(&self, session: &str) -> Option<Joined> {
        self.threads.get().host_writes.borrow().get(session).cloned()
    }
    async fn land_images(
        &self,
        entry: &Entry,
        adapter: &dyn HarnessAdapter,
        landing: &str,
        dir: &str,
        attachments: Vec<Attachment>,
    ) -> Result<LandedImages, HostError> {
        self.agents.get().land_images(entry, adapter.attachments(), landing, dir, &attachments).await
    }
    async fn land_files(&self, entry: &Entry, landing: &str, dir: &str, attachments: Vec<Attachment>) -> Result<Vec<String>, HostError> {
        self.agents.get().land_files(entry, landing, dir, &attachments).await
    }
    async fn ready_fork(
        &self,
        adapter: Arc<dyn HarnessAdapter>,
        session: &str,
        turn: ForkTurn,
        harness: &str,
    ) -> Result<ForkedSession, HostError> {
        let label = AgentsArea::agent_label(harness);
        let said = move |why: &str| {
            wsp_words::exit::refusal(
                &wsp_words::words::thread::fork_refused_line(&label, why),
                wsp_words::words::thread::FORK_REFUSED_FIX,
                Some(wsp_words::exit::Kind::Conflict),
            )
        };
        forks::ready_fork(adapter, session, turn, &said).await
    }
    async fn copy_images(&self, plan: &ForkPlan, workspace_id: &str, thread_id: &str) -> Result<(), HostError> {
        let ctx: Rc<dyn ForksCtx> = self.rc();
        forks::copy_images(&ctx, plan, workspace_id, thread_id).await
    }
    async fn pay_cut_owed(&self, adapter: Arc<dyn HarnessAdapter>, workspace_id: &str, thread_id: &str, resume: &str) -> String {
        let ctx: Rc<dyn ForksCtx> = self.rc();
        forks::pay_cut_owed(&ctx, adapter, workspace_id, thread_id, resume).await
    }
    fn snapshot_of(&self, entry: &Entry, cwd: &str) -> Shared<LocalBoxFuture<'static, Option<String>>> {
        let area = self.threads.get();
        let (entry, cwd) = (entry.clone(), cwd.to_string());
        let (sender, taken) = futures::channel::oneshot::channel::<Option<String>>();
        spawn_detached(async move {
            let _ = sender.send(area.snapshot_of(&entry, &cwd).await);
        });
        taken.map(|answer| answer.ok().flatten()).boxed_local().shared()
    }
    async fn keep_steer(&self, running: &Row, opts: &StartOptions, origin: Option<&Caller>, steer_id: &str) -> Result<(), HostError> {
        let row_id = running.borrow().view.id.clone();
        self.threads.get().keep_steer(&row_id, &sessions_sent(opts), origin, steer_id).await;
        Ok(())
    }
    fn steer_lost(&self, running: &Row, steer_id: &str) {
        let row_id = running.borrow().view.id.clone();
        self.threads.get().steer_lost(&row_id, steer_id)
    }
    fn steer_answered(
        &self,
        running: &Row,
        handle_id: &str,
        opts: &StartOptions,
        origin: Option<&Caller>,
        steer_id: &str,
        accepted: bool,
    ) -> bool {
        let row_id = running.borrow().view.id.clone();
        self.threads.get().steer_answered(&row_id, handle_id, &sessions_sent(opts), origin, steer_id, accepted)
    }
    fn cut_before(&self, workspace_id: &str, thread_id: &str) -> bool {
        self.channels.get().cut_before(workspace_id, thread_id)
    }
    fn take_kept(
        &self,
        thread_id: &str,
        ask: Option<KeptLaunch>,
        resume: Option<&str>,
        launch_key: Option<&KeptLaunch>,
    ) -> Option<Rc<RefCell<KeptProcess>>> {
        self.threads.get().take_kept(thread_id, ask.as_ref(), resume, launch_key)
    }
    fn record_copies(&self, plan: &ForkPlan, workspace_id: &str, thread_id: &str) {
        let events = forks::copied_events(plan, workspace_id, thread_id);
        self.transcripts.get().record_copies(&events)
    }
    fn run_turn(&self, t: TurnStart) -> Result<Rc<dyn SessionHandle>, HostError> {
        self.turns.get().run_turn(t)
    }
    async fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<&str>,
        attachments: Vec<Attachment>,
    ) -> Result<(), HostError> {
        self.transcripts.get().keep_sent_images(workspace_id, thread_id, request_id, &attachments).await;
        Ok(())
    }
    fn cancel_resume_timer(&self, thread_id: &str) {
        let cancel: Option<Cancel> = self.resume_timers.borrow().get(thread_id).cloned();
        if let Some(cancel) = cancel {
            cancel();
        }
    }
    async fn remember_target(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.rules.get().remember_target(record).await
    }
    fn notify_end(
        &self,
        row: &Row,
        notify: &[String],
        by: Option<&ThreadScope>,
        road: Option<&WorkspaceOrigin>,
        result: wsp_wire::views::session_events::TurnResult,
    ) {
        let named = TellAs { by: by.cloned(), road: road.copied() };
        self.threads.get().notify_end(row, notify, &named, &result, false)
    }
    fn drop_images(&self, entry: &Entry, dir: &str) {
        self.agents.get().drop_images(entry, dir)
    }
    async fn drop_sent_images(&self, workspace_id: &str, thread_ids: &[String]) -> Result<(), HostError> {
        self.transcripts.get().drop_sent_images(workspace_id, ThreadsNamed::These(thread_ids.to_vec())).await;
        Ok(())
    }
    fn end_kept(&self, thread_id: &str, kept: &Rc<RefCell<KeptProcess>>) {
        self.threads.get().end_kept(thread_id, kept.clone(), false)
    }
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>> {
        futures::future::ready(self.transcripts.get().open_thread(workspace_id, thread_id)).boxed_local()
    }

    // sessions_reads
    fn tree_stands_on(&self, workspace_id: &str, origin: Option<&Caller>, talk: Option<TreeTalk>) -> bool {
        self.boot.get().tree_stands_on(workspace_id, origin, talk).unwrap_or(false)
    }
    fn refuse_named(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.rules.get().refuse_named(workspace_id, origin)
    }
    fn title_rows(&self, rows: Vec<SessionView>) -> Vec<SessionView> {
        wsp_threads::threads::agents::lists::title_rows(&rows)
    }
    fn booting(&self, workspace_id: &str) -> bool {
        self.boot_work.borrow().contains_key(workspace_id)
    }
    async fn refresh_title(&self, view: SessionView, ended: bool) -> Result<(), HostError> {
        self.agents.get().refresh_title(&view, ended).await;
        Ok(())
    }
    fn tree_records(&self, origin: Option<&Caller>, talk: TreeTalk, workspace_id: Option<&str>) -> Vec<(String, ThreadRecord)> {
        let found = self.boot.get().tree_records(origin, Some(talk), workspace_id).unwrap_or_default();
        found.into_iter().map(|(thread, _, record)| (thread, record)).collect()
    }
    fn listed_rows(&self, rows: Vec<SessionView>) -> Vec<SessionView> {
        let held: Vec<Row> = rows.iter().filter_map(|view| self.sessions_row_by_id(&view.id)).collect();
        self.boot.get().listed_rows(&held)
    }
    fn placed_rows(&self, rows: Vec<SessionView>) -> Vec<SessionView> {
        let boot = self.boot.get();
        let placed = boot.placed_rows(rows.clone());
        placed.now_or_never().unwrap_or(rows)
    }
    fn drives_thread(&self, thread_id: Option<&str>, origin: Option<&Caller>) -> bool {
        self.threads.get().drives_thread(thread_id, origin)
    }
    async fn open_transcript(&self, workspace_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        self.transcripts.get().open_transcript(workspace_id)
    }
    fn opened_words(&self, workspace_id: &str, thread_id: &str) {
        self.transcripts.get().opened_words(workspace_id, thread_id)
    }
    fn index_pos(&self, workspace_id: &str) -> Option<u64> {
        self.sessions_in_index(workspace_id, |index| Some(index.pos))
    }
    async fn read_page(&self, workspace_id: &str, thread_id: &str, window: ThreadRead) -> Result<HistoryPage, HostError> {
        let read = self.transcripts.get().read(workspace_id, thread_id, &window)?;
        Ok(HistoryPage {
            events: read.events,
            pos: JsInt(JsNumber(read.pos as f64)),
            total: JsInt(JsNumber(read.total as f64)),
            trimmed: read.trimmed.then_some(wsp_frames::True),
        })
    }
    fn thread_facts(&self, thread_id: &str) -> Option<ThreadFacts> {
        self.boot.get().thread_facts(thread_id)
    }
    async fn facts_after_read(&self, thread_id: &str, origin: Option<&Caller>) -> Result<Option<ThreadFacts>, HostError> {
        self.boot.get().facts_after_read(thread_id, origin).await
    }
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined> {
        self.transcripts.get().kept_writes.borrow().get(workspace_id).cloned()
    }
    async fn store_get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.store.get(collection, id).await
    }
    async fn store_get_blob(&self, collection: &str, key: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.store.get_blob(collection, key).await
    }
    fn index_workspaces(&self) -> Vec<String> {
        self.transcript_state.with(|s| s.index.keys().cloned().collect())
    }
    fn search_in(&self, workspace_id: &str, words: &str, drives: &dyn Fn(&str) -> bool, found: &mut Vec<SearchedHit>) -> u32 {
        let unsearched = self.transcript_state.with(|s| {
            s.index.get(workspace_id).map(|index| wsp_core::types::internal_index::search_index(workspace_id, index, words, drives, found))
        });
        unsearched.unwrap_or(0) as u32
    }

    // sessions_acts
    fn record_steer(&self, row: &Row, row_id: &str, ask: &SteerAsk, origin: Option<&Caller>, steer_id: &str) {
        let sent = SteerSent {
            prompt: ask.prompt.clone(),
            request_id: ask.request_id.clone(),
            via_slate: false,
            started_by: None,
            attachments: ask.attachments.clone().unwrap_or_default(),
        };
        let handle_id = row_id.to_string();
        let row_key = row.borrow().view.id.clone();
        self.threads.get().record_steer(&row_key, &handle_id, &sent, origin, steer_id)
    }
    async fn launch_adapter_for(
        &self,
        entry: &Entry,
        harness: &str,
        server_values: Option<ServerValues>,
    ) -> Result<BuiltAdapter, HostError> {
        let servers = server_values.and_then(|values| values.config).unwrap_or_default();
        let ask = AdapterAsk { named: Some(harness.to_string()), servers, ..Default::default() };
        self.agents.get().launch_adapter_for(entry, ask).await
    }
    fn harness_exec(&self, entry: &Entry, timeout_ms: u64) -> Arc<dyn wsp_adapters::port::HarnessExec> {
        let machine = entry.borrow().machine.clone();
        Arc::new(SessionsExec(wsp_engine::run_env::harness_exec(machine, timeout_ms)))
    }
    async fn write_session(
        &self,
        session: &str,
        entry: &Entry,
        adapter: Arc<dyn HarnessAdapter>,
        title: &str,
    ) -> Result<SessionRenameWrite, HostError> {
        let exec = self.harness_exec(entry, wsp_core::types::events::SESSION_TITLE_TIMEOUT_MS);
        let write = adapter
            .rename_session(session.to_string(), title.to_string(), exec)
            .ok_or_else(|| HostError::new("adapter.renameSession is not a function"))?;
        let write = async move { write.await.map_err(|e| HostError::new(e.message)) }.boxed_local();
        self.threads.get().write_session(session, write).await
    }
    fn brokered(&self, record: &WorkspaceRecord) -> bool {
        self.kinds.get().brokered(record)
    }
    async fn server_values_for(&self, entry: &Entry, harness: &str, cwd: &str) -> Result<Option<ServerValues>, HostError> {
        self.agents.get().server_values_for(entry, harness, cwd).await
    }
    async fn mark(&self, thread_ids: &[String], stamps: StampsOf, origin: Option<&Caller>) -> Result<(), HostError> {
        let each = move |thread: &str| stamps(thread);
        self.boot.get().mark(thread_ids, Stamped::Each(&each), origin).await
    }
    async fn arm_resume(&self, thread_ids: &[String], on: bool, origin: Option<&Caller>) -> Result<(), HostError> {
        self.boot.get().arm_resume(thread_ids, on, origin).await
    }
    async fn aside(&self, adapter: &dyn HarnessAdapter, ask: AsideQuestion) -> Result<String, HostError> {
        let answered = adapter.aside(ask).ok_or_else(|| HostError::new("adapter.aside is not a function"))?;
        answered.await.map(|answer| answer.text).map_err(HostError::new)
    }

    // sessions_settle
    fn subagent_known(&self, id: &str) -> bool {
        self.transcript_state.with(|s| s.index.values().any(|index| index.children.values().any(|kids| kids.contains_key(id))))
    }
    fn settles_thread(&self, thread_id: &str, origin: Option<&Caller>) -> bool {
        self.threads.get().settles_thread(thread_id, origin)
    }
    fn settled_now(&self, thread_id: &str, settle_ms: Option<f64>) -> bool {
        self.boot.get().settled_now(thread_id, settle_ms)
    }
    fn tree_under(&self, thread_id: &str) -> Vec<String> {
        self.threads.get().tree_under(thread_id)
    }
    fn restarts(&self) -> IndexMap<String, String> {
        self.threads.get().restarts()
    }

    // sessions_rewind
    fn reap_kept(&self, thread_id: &str) {
        self.threads.get().reap_kept(thread_id, false)
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.channels.get().checkout_of(record)
    }
    async fn wake(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        use wsp_core::types::api::WorkspacesApi as _;
        self.workspaces.get().wake(workspace_id, origin.cloned()).await.map(|_| ())
    }
    async fn git_restore(&self, entry: &Entry, cwd: &str, checkpoint: &str) -> Result<Map<String, Value>, HostError> {
        let workspace_id = entry.borrow().record.borrow().id.clone();
        let frame = match json!({ "op": "git.restore", "cwd": cwd, "checkpoint": checkpoint, "scope": workspace_id }) {
            Value::Object(frame) => frame,
            _ => Map::new(),
        };
        let channels = self.channels.get();
        let asked = {
            let entry: AccountEntry = entry.clone();
            async move { channels.with_daemon(&entry, Box::new(move |ask| ask(frame))).await.map(Value::Object).map_err(HostError::from) }
        };
        let answer = self.folders.get().queued(&workspace_id, asked.boxed_local()).await?;
        match answer {
            Value::Object(reply) => Ok(reply),
            other => Err(HostError::new(format!("the daemon answered a restore with {other}"))),
        }
    }
    async fn flush_transcript(&self, workspace_id: &str) -> Result<(), HostError> {
        self.transcripts.get().flush_transcript(workspace_id).await;
        Ok(())
    }
    fn move_session(&self, thread_id: &str, from: &str, to: &str) {
        let ctx: Rc<dyn ForksCtx> = self.rc();
        forks::move_session(&ctx, thread_id, from, to)
    }
    async fn drop_from_transcript(&self, workspace_id: &str, thread_id: &str, turns: Option<&[String]>) -> Result<(), HostError> {
        let drops: Option<Rc<dyn Fn(&SessionEvent) -> bool>> = turns.map(|cut| {
            let cut: Vec<String> = cut.to_vec();
            Rc::new(move |event: &SessionEvent| {
                let turn = serde_json::to_value(event).ok().and_then(|v| v["turnId"].as_str().map(str::to_string)).unwrap_or_default();
                cut.contains(&turn)
            }) as Rc<dyn Fn(&SessionEvent) -> bool>
        });
        self.transcripts.get().drop_from_transcript(workspace_id, thread_id, drops).await
    }
    async fn slates_rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        self.slates.get().rewound(thread_id, turn_id, cut).await
    }

    // sessions_delete
    fn lead_of_sub(&self, workspace_id: Option<&str>, origin: Option<&Caller>) -> bool {
        self.sub_life.get().lead_of_sub(workspace_id, origin)
    }
    async fn sub_delete(
        &self,
        entry: &Entry,
        thread_id: &str,
        origin: Option<&Caller>,
        ask: DeleteThreadAsk,
    ) -> Result<ThreadDeleted, HostError> {
        let deleted = self.sub_life.get().sub_delete(entry, thread_id, origin, ask.check.unwrap_or(false)).await?;
        Ok(ThreadDeleted {
            workspace_id: deleted.workspace_id,
            worktree: None,
            threads: u64::from(deleted.threads),
            ended: None,
            machine_deleted: deleted.machine.as_deref() == Some("deleted"),
            on: Some(deleted.on),
            question: deleted.question,
        })
    }
    fn turn_runs(&self, workspace_id: &str) -> bool {
        self.daemon.get().turn_runs(workspace_id)
    }
    async fn end_thread_shells(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError> {
        self.channels.get().end_thread_shells(entry, thread_id).await.map_err(HostError::from)
    }
    async fn delete_workspace(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        use wsp_core::types::api::WorkspacesApi as _;
        self.workspaces.get().delete(workspace_id, origin.cloned()).await
    }
    async fn drop_checkpoints(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError> {
        self.folders.get().drop_checkpoints(entry, thread_id).await
    }
    async fn drop_thread_files(&self, entry: &Entry, thread_ids: &[String]) -> Result<(), HostError> {
        self.agents.get().drop_thread_files(entry, thread_ids).await;
        Ok(())
    }
    fn row_gone(&self, row_id: &str, workspace_id: &str, thread_id: &str) {
        self.boot.get().row_gone(row_id, workspace_id, Some(thread_id))
    }
    async fn slates_forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError> {
        self.slates.get().forget(thread_id, keep).await
    }
}
