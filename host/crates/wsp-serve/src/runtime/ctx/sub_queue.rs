// SPDX-License-Identifier: AGPL-3.0-only
//! What the sub-queue, sub-life and pull-request areas ask of the runtime (`SubQueueCtx`, sub-queue.ts:49-498;
//! `SubLifeCtx`, sub-life.ts:156-597; `PullRequestsCtx`, pull-requests.ts:9-300): the core's own maps and clock, a
//! forward to the area that holds each call, and the one line no area has (`forget_marks`, sub-life.ts:319-321).
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::Value;
use wsp_account::account::core_types::{AccountError, AccountResult, Ask, Entry as AccountEntry, Work};
use wsp_account::account::pull_requests::{CacheEntry, PullRequestsCtx};
use wsp_core::clock::Cancel;
use wsp_core::github_cache::GitHubCacheEntry;
use wsp_core::pipe::area::{cut_off as pipe_cut_off, PipeAreaContext};
use wsp_core::types::api::{SessionsApi as _, StartAs, StartOptions, WorkspacesApi as _};
use wsp_core::types::internal::{absent_machine, dead_machine, SessionEntry, SubStartRecord, SUB_STARTS};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::box_backend::BoxLimits;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::MachineKind;
use wsp_machines::machines::area_error::AreaResult;
use wsp_machines::machines::boot::entries::EntryAsk;
use wsp_machines::machines::boot::rows::Stamped;
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_life::{SessionFile, SubLifeCtx};
use wsp_machines::machines::sub_queue::{StartedTurn, SubQueueCtx};
use wsp_store::transcript_reader::ThreadRead;
use wsp_threads::threads::agents::launch::AdapterAsk;
use wsp_threads::threads::threads::tree::TellAs;
use wsp_wire::types::changes::Checkout;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::{Caller, ProjectView, ReachState};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

use super::Ctx;
use crate::runtime::join::spawn_detached;

/// `{ type, ... }` as the bus's event, or the warning a shape that is no event leaves where the TS would have thrown.
fn sub_queue_event(event: Value) -> Option<EventUnion> {
    match serde_json::from_value::<EventUnion>(event) {
        Ok(event) => Some(event),
        Err(e) => {
            wsp_core::console::warn(&format!("an event of a shape the host does not know was not sent: {e}"));
            None
        }
    }
}

/// What a start the queue held reads as when it failed to launch: a row of its own, for the threads area to tell
/// the thread it was to report to.
fn sub_queue_held_row(view: &SessionView, turn_id: &str) -> Rc<RefCell<SessionEntry>> {
    Rc::new(RefCell::new(SessionEntry {
        view: view.clone(),
        turn_id: turn_id.to_string(),
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
    }))
}

/// The options a held start was written with, as the sessions area takes them.
fn sub_queue_options(o: &serde_json::Map<String, Value>) -> Result<StartOptions, HostError> {
    fn typed<T: serde::de::DeserializeOwned>(o: &serde_json::Map<String, Value>, key: &str) -> Result<Option<T>, HostError> {
        match o.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => serde_json::from_value(value.clone()).map(Some).map_err(|e| HostError::new(format!("{key}: {e}"))),
        }
    }
    let r#as = match o.get("as") {
        Some(Value::Object(held)) => Some(StartAs {
            thread_id: held.get("threadId").and_then(Value::as_str).unwrap_or_default().to_string(),
            turn_id: held.get("turnId").and_then(Value::as_str).unwrap_or_default().to_string(),
        }),
        _ => None,
    };
    Ok(StartOptions {
        prompt: typed::<String>(o, "prompt")?.unwrap_or_default(),
        harness: typed(o, "harness")?,
        thread: typed(o, "thread")?,
        cwd: typed(o, "cwd")?,
        model: typed(o, "model")?,
        effort: typed(o, "effort")?,
        permission_mode: typed(o, "permissionMode")?,
        access: typed(o, "access")?,
        context_window: typed(o, "contextWindow")?,
        fast: typed(o, "fast")?,
        started_by: typed(o, "startedBy")?,
        r#as,
        request_id: typed(o, "requestId")?,
        attempt: typed(o, "attempt")?,
        notify: typed(o, "notify")?,
        title: typed(o, "title")?,
        attachments: typed(o, "attachments")?,
        ..Default::default()
    })
}

#[async_trait(?Send)]
impl SubQueueCtx for Ctx {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn live_all(&self) -> Vec<Entry> {
        self.live.borrow().values().cloned().collect()
    }
    fn session(&self, turn_id: &str) -> Option<SessionEntry> {
        let row = self.sessions.borrow().get(turn_id).cloned()?;
        let copy = row.borrow().clone();
        Some(copy)
    }
    fn set_session(&self, turn_id: &str, entry: SessionEntry) {
        self.sessions.borrow_mut().insert(turn_id.to_string(), Rc::new(RefCell::new(entry)));
    }
    fn update_session(&self, turn_id: &str, f: &mut dyn FnMut(&mut SessionEntry)) {
        let row = self.sessions.borrow().get(turn_id).cloned();
        if let Some(row) = row {
            f(&mut row.borrow_mut());
        }
    }
    fn remove_session(&self, turn_id: &str) {
        self.sessions.borrow_mut().shift_remove(turn_id);
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool {
        self.rules.get().holds_slot(record)
    }
    fn place_name_of(&self, place_id: &str) -> Option<String> {
        Some(self.wired_door()?.name_of(place_id))
    }
    fn machines_at(&self, place_id: &str) -> Option<f64> {
        self.wired_door()?.machines_at(place_id).map(|n| n as f64)
    }
    async fn limits_of(&self, record: &WorkspaceRecord) -> AreaResult<Option<BoxLimits>> {
        let backend = self.kinds.get().backend_for(record)?;
        Ok(backend.limits().await.ok())
    }
    fn view(&self, record: &WorkspaceRecord) -> Result<Value, HostError> {
        let view = self.records.get().view(record)?;
        serde_json::to_value(view).map_err(|e| HostError::new(e.to_string()))
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        let records = self.records.get();
        Ok(records.persist(record).await?)
    }
    async fn persist_sessions(&self, workspace_id: &str) -> AreaResult<()> {
        self.transcripts.get().persist_sessions(workspace_id).await;
        Ok(())
    }
    fn emit(&self, event: Value) {
        let Some(event) = sub_queue_event(event) else { return };
        if let Err(e) = self.bus.emit(event) {
            wsp_core::console::warn(&format!("a listener failed on an event the queue sent: {}", e.message));
        }
    }
    fn record(&self, event: Value) {
        match serde_json::from_value::<SessionEvent>(event) {
            Ok(event) => self.transcripts.get().record(event),
            Err(e) => wsp_core::console::warn(&format!("a transcript event of a shape the host does not know was not kept: {e}")),
        }
    }
    fn attach_absent(&self, record: &Rc<RefCell<WorkspaceRecord>>, said: &str) -> Result<Entry, HostError> {
        let said = said.to_string();
        let machine = absent_machine("", MachineKind::Sandbox, Arc::new(move || EngineError::Other { message: said.clone(), code: None }));
        self.machines.get().attach(record, machine)
    }
    fn attach_dead(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Result<Entry, HostError> {
        self.machines.get().attach(record, dead_machine(""))
    }
    fn record_cell(&self, workspace_id: &str) -> Option<Rc<RefCell<WorkspaceRecord>>> {
        Some(self.live.borrow().get(workspace_id)?.borrow().record.clone())
    }
    async fn drop_workspace(&self, workspace_id: &str) -> AreaResult<()> {
        let machines = self.machines.get();
        Ok(machines.drop(workspace_id).await?)
    }
    fn notify_failed(
        &self,
        view: &SessionView,
        turn_id: &str,
        notify: &[String],
        by: Option<&ThreadScope>,
        road: Option<&Value>,
        error: &str,
    ) {
        let road = road.and_then(|road| serde_json::from_value::<WorkspaceOrigin>(road.clone()).ok());
        let named = TellAs { by: by.cloned(), road };
        let result = match serde_json::from_value(serde_json::json!({ "status": "failed", "error": error })) {
            Ok(result) => result,
            Err(_) => return,
        };
        let held = sub_queue_held_row(view, turn_id);
        self.threads.get().notify_end(&held, notify, &named, &result, false);
    }
    fn push_head(&self, thread_id: &str) {
        if let Err(e) = self.boot.get().push_head(thread_id) {
            wsp_core::console::warn(&format!("the head of thread {thread_id} was not pushed: {}", e.message));
        }
    }
    async fn rows_sent(&self) {
        let boot = self.boot.get();
        boot.rows_sent().await;
    }
    async fn put_start(&self, record: &SubStartRecord) -> AreaResult<()> {
        let row = serde_json::to_value(record).map_err(|e| HostError::new(e.to_string()))?;
        Ok(self.store.put(SUB_STARTS, &record.workspace_id, row).await?)
    }
    async fn delete_start(&self, workspace_id: &str) -> AreaResult<()> {
        Ok(self.store.delete(SUB_STARTS, workspace_id).await?)
    }
    async fn list_starts(&self) -> AreaResult<Vec<SubStartRecord>> {
        let rows = self.store.list(SUB_STARTS).await?;
        Ok(rows.into_iter().filter_map(|row| serde_json::from_value(row).ok()).collect())
    }
    async fn stand_up_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>, posted: Rc<dyn Fn()>, waited: bool) -> AreaResult<()> {
        let spawn = self.sub_spawn.get();
        spawn.stand_up_queued(record, posted, waited).await
    }
    async fn start_turn(&self, workspace_id: &str, o: serde_json::Map<String, Value>, origin: Option<Caller>) -> AreaResult<StartedTurn> {
        let opts = sub_queue_options(&o)?;
        let sessions = self.sessions_area.get();
        let handle = sessions.start(workspace_id, opts, origin).await?;
        Ok(StartedTurn { id: handle.id(), workspace_id: handle.workspace_id(), turn_id: handle.turn_id() })
    }
    async fn cut_off(&self, entry: &Entry) {
        let (id, name) = {
            let held = entry.borrow();
            let record = held.record.borrow();
            (record.id.clone(), record.name.clone())
        };
        pipe_cut_off(self as &dyn PipeAreaContext, &id, &name).await;
    }
    async fn unfork(&self, entry: &Entry) -> AreaResult<()> {
        let machines = self.machines.get();
        Ok(machines.unfork(entry).await?)
    }
    fn now(&self) -> f64 {
        self.clock.now() as f64
    }
    fn uuid(&self) -> String {
        self.ids.uuid()
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms as i64)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
    fn on_event(&self, kind: &str, f: Rc<dyn Fn(&Value)>) {
        self.hear_json(kind, f)
    }
}

#[async_trait(?Send)]
impl SubLifeCtx for Ctx {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn live_all(&self) -> Vec<Entry> {
        self.live.borrow().values().cloned().collect()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_records.borrow().iter().map(|(id, record)| (id.clone(), record.workspace_id.clone())).collect()
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.sessions.borrow().values().map(|row| row.borrow().view.clone()).collect()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        self.threads.get().thread_runs(thread_id)
    }
    fn launching_on(&self, thread_id: &str) -> bool {
        self.threads.get().launching_on(thread_id).is_some()
    }
    fn push_head(&self, thread_id: &str) {
        SubQueueCtx::push_head(self, thread_id)
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.channels.get().place_id_of(record)
    }
    fn delete_after_ms(&self, place_id: Option<&str>) -> Option<f64> {
        self.channels.get().settings_at(place_id)?.delete_after_ms.map(|ms| ms.0 .0)
    }
    fn session_file(&self, entry: &Entry, harness: &str, session_id: &str) -> Option<SessionFile> {
        let ask = AdapterAsk { named: Some(harness.to_string()), ..Default::default() };
        let built = self.agents.get().adapter_for(entry, ask).ok()?;
        let file = built.adapter.session_file(session_id)?;
        Some(SessionFile { folder: file.folder, name: file.name, depth: file.depth })
    }
    async fn read_transcript(&self, workspace_id: &str, thread_id: &str, limit: usize, bytes: usize) -> AreaResult<Vec<Value>> {
        let ask = ThreadRead { before: None, limit, bytes: bytes as u64, strict: false, shape: None };
        let read = self.transcripts.get().read(workspace_id, thread_id, &ask)?;
        Ok(read.events.iter().filter_map(|event| serde_json::to_value(event).ok()).collect())
    }
    async fn put_blob(&self, collection: &str, key: &str, bytes: Vec<u8>) -> AreaResult<()> {
        Ok(self.store.put_blob(collection, key, bytes).await?)
    }
    async fn delete_blob(&self, collection: &str, key: &str) -> AreaResult<()> {
        Ok(self.store.delete_blob(collection, key).await?)
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        SubQueueCtx::persist(self, record).await
    }
    async fn nap_with(&self, workspace_id: &str, reason: &str) -> AreaResult<()> {
        let machines = self.machines.get();
        machines.nap_with(workspace_id, Some(reason)).await?;
        Ok(())
    }
    fn end_sessions(&self, workspace_id: &str, reason: &str) {
        self.machines.get().end_sessions(workspace_id, reason);
    }
    fn attach_dead(&self, entry: &Entry, machine_id: &str) -> Result<Entry, HostError> {
        let record = entry.borrow().record.clone();
        self.machines.get().attach(&record, dead_machine(machine_id))
    }
    fn forget_marks(&self, workspace_id: &str) {
        self.reach.get().nap_refusals.borrow_mut().shift_remove(workspace_id);
        self.reach.get().stop_refusals.borrow_mut().shift_remove(workspace_id);
        self.daemon.get().unreached().borrow_mut().shift_remove(workspace_id);
    }
    fn idle_forget(&self, workspace_id: &str) {
        self.idle.get().forget(workspace_id);
    }
    async fn emit_status(&self, entry: &Entry, reach: ReachState) -> AreaResult<()> {
        let area = self.reach.get();
        area.emit_status(entry, reach, None).await
    }
    fn emit(&self, event: Value) {
        SubQueueCtx::emit(self, event)
    }
    async fn unfork(&self, entry: &Entry) -> AreaResult<()> {
        SubQueueCtx::unfork(self, entry).await
    }
    async fn cut_off(&self, entry: &Entry) {
        SubQueueCtx::cut_off(self, entry).await
    }
    fn broker_lift(&self, workspace_id: &str) {
        if let Some(broker) = &self.opts.broker {
            broker.lift(workspace_id);
        }
    }
    async fn delete_workspace(&self, workspace_id: &str) -> AreaResult<()> {
        let workspaces = self.workspaces.get();
        Ok(workspaces.delete(workspace_id, None).await?)
    }
    fn now(&self) -> f64 {
        self.clock.now() as f64
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms as i64)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
    fn on_event(&self, kind: &str, f: Rc<dyn Fn(&Value)>) {
        self.hear_json(kind, f)
    }
}

#[async_trait(?Send)]
impl PullRequestsCtx for Ctx {
    fn now(&self) -> i64 {
        self.clock.now()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms)
    }
    fn cache_get(&self, key: &str) -> Option<CacheEntry> {
        let held = self.github_cache.get(key)?;
        Some(CacheEntry { body: held.body, tag: held.tag, fetched_at: held.fetched_at })
    }
    fn cache_set(&self, key: &str, entry: CacheEntry) {
        self.github_cache.set(key, GitHubCacheEntry { body: entry.body, tag: entry.tag, fetched_at: entry.fetched_at });
    }
    fn cache_delete(&self, key: &str) {
        self.github_cache.delete(key);
    }
    fn live(&self, workspace_id: &str) -> Option<AccountEntry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    fn session_views(&self) -> Vec<SessionView> {
        SubLifeCtx::session_views(self)
    }
    async fn on_this_computer(
        &self,
        work: Box<dyn FnOnce(Ask, String) -> LocalBoxFuture<'static, AccountResult<Value>>>,
    ) -> AccountResult<Value> {
        let channels = self.channels.get();
        channels.on_this_computer(work).await
    }
    async fn with_daemon(&self, entry: &AccountEntry, work: Work<Value>) -> AccountResult<Value> {
        let channels = self.channels.get();
        channels.with_daemon(entry, work).await
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.channels.get().checkout_of(record)
    }
    async fn entry_of(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<AccountEntry, HostError> {
        let boot = self.boot.get();
        boot.entry_of(workspace_id, origin, EntryAsk::default()).await
    }
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError> {
        self.channels.get().project_held(id)
    }
    fn status_watched(&self) -> bool {
        self.status_tracker.get().watched()
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()> {
        let records = self.records.get();
        Ok(records.persist(record).await?)
    }
    async fn status_now(&self, entry: &AccountEntry) -> AccountResult<()> {
        let reach = self.reach.get();
        reach.status_now(entry).await.map_err(|e| AccountError::Host(e.into_host()))
    }
    async fn read_checkout(&self, entry: &AccountEntry, force: bool) -> AccountResult<Option<Checkout>> {
        let reach = self.reach.get();
        Ok(reach.read_checkout(entry, force).await)
    }
    fn slates_watch_pr(&self, workspace_id: &str) -> bool {
        self.slates.get().watches_pr(workspace_id)
    }
    async fn mark_settled(&self, threads: Vec<String>, at: i64) -> AccountResult<()> {
        let mut stamps = serde_json::Map::new();
        stamps.insert("readAt".into(), serde_json::json!(at));
        stamps.insert("settledAt".into(), serde_json::json!(at));
        let boot = self.boot.get();
        Ok(boot.mark(&threads, Stamped::Same(stamps), None).await?)
    }
    fn pauses(&self, record: &WorkspaceRecord) -> bool {
        self.rules.get().pauses(record)
    }
    async fn nap_with(&self, workspace_id: &str) -> AccountResult<()> {
        let machines = self.machines.get();
        machines.nap_with(workspace_id, None).await?;
        Ok(())
    }
    fn read_lead_of(&self, child: &AccountEntry) {
        self.reach.get().read_lead_of(child);
    }
    async fn queued(&self, workspace_id: &str, work: LocalBoxFuture<'static, AccountResult<Value>>) -> AccountResult<Value> {
        let kept: Rc<RefCell<Option<AccountError>>> = Rc::default();
        let stash = kept.clone();
        let through = async move {
            work.await.map_err(|e| {
                let host: HostError = e.clone().into();
                *stash.borrow_mut() = Some(e);
                host
            })
        }
        .boxed_local();
        let folders = self.folders.get();
        match folders.queued(workspace_id, through).await {
            Ok(value) => Ok(value),
            Err(host) => Err(kept.borrow_mut().take().unwrap_or(AccountError::Host(host))),
        }
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
}
