// SPDX-License-Identifier: AGPL-3.0-only
//! The context's bodies for the transcripts, channels, process scopes, box panels, trimmed rows and waits
//! (transcripts.ts, channels.ts, proc-scopes.ts, box-panels.ts, trimmed-row.ts, waits.ts; wired at runtime.ts:433-438
//! and :469). Each method reads a core field, forwards to the area that owns it, or calls a platform function.
use std::cell::RefCell;
use std::ops::ControlFlow;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_account::account::box_panels::StartedRow;
use wsp_account::account::channels::{ChannelRow, ThreadLaunch};
use wsp_account::account::core_types::{AccountError, AccountResult, Entry, Release};
use wsp_account::account::proc_scopes::{KeptRow, LatestRow, ScopeRecord, ScopeRow};
use wsp_adapters::claude::landmines::{BrokerKind, BrokeredSignIn};
use wsp_core::clock::Cancel;
use wsp_core::daemon_channel::{open_daemon_channel, ChannelError, DaemonChannel, DaemonChannelOptions};
use wsp_core::reach::DaemonReach;
use wsp_core::types::harness::BrokeredKind;
use wsp_core::types::internal::{SessionEntry, ThreadWords};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::machines::brokered::{brokered_env, terminal_sign_in, LaunchRead, RepoOfRecord, TerminalSignIn};
use wsp_machines::places::types::PlaceDoor;
use wsp_store::sqlite_transcripts::{EventSize, Summed, Total, TranscriptRows};
use wsp_store::store::Store;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::DaemonReachView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::exit::{HostError, Kind};

use super::Ctx;
use crate::runtime::join::spawn_detached;

/// `channels.ts:518`'s link identity: one `Rc<()>` per link the door holds, so a pane's watches read the same key for
/// as long as the link stands and a new link at a reused address reads none (box-panels.ts `procAsks`).
type Identities = IndexMap<usize, (Weak<dyn DaemonReach>, Rc<()>)>;

thread_local! {
    static LINKS: RefCell<Identities> = RefCell::new(IndexMap::new());
}

/// A channel error as the area's error: a door's refusal and a token's keep their kind, the rest are plain.
fn channel_failure(e: ChannelError) -> AccountError {
    match e.kind() {
        Some("refused") => HostError::with_kind(e.to_string(), Kind::Refused).into(),
        Some("reauth") => HostError::with_kind(e.to_string(), Kind::Reauth).into(),
        _ => HostError::new(e.to_string()).into(),
    }
}

/// `store.transcripts` as the shared handle the area asks for. A store that keeps no rows (the memory store) fails each
/// call, where the TS takes the blob road that Proposal 3 dropped.
struct StoreRows(Rc<dyn Store>);

const NO_ROWS: &str = "this store keeps no transcript rows";

impl StoreRows {
    fn rows(&self) -> Result<&dyn TranscriptRows, HostError> {
        self.0.transcripts().ok_or_else(|| HostError::new(NO_ROWS))
    }
}

impl TranscriptRows for StoreRows {
    fn all(&self, workspace_id: &str) -> Result<Summed, HostError> {
        self.rows()?.all(workspace_id)
    }
    fn newest(
        &self,
        workspace_id: &str,
        thread_id: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        self.rows()?.newest(workspace_id, thread_id, before, each)
    }
    fn count(&self, workspace_id: &str, thread_id: &str) -> Result<u64, HostError> {
        self.rows()?.count(workspace_id, thread_id)
    }
    fn total(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Total, HostError> {
        self.rows()?.total(workspace_id, thread_id)
    }
    fn newest_pos(&self, workspace_id: &str) -> Result<i64, HostError> {
        self.rows()?.newest_pos(workspace_id)
    }
    fn sizes(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        self.rows()?.sizes(workspace_id, thread_id)
    }
    fn word_events(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        self.rows()?.word_events(workspace_id, thread_id)
    }
    fn thread(&self, workspace_id: &str, thread_id: &str) -> Result<Summed, HostError> {
        self.rows()?.thread(workspace_id, thread_id)
    }
    fn at(&self, workspace_id: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        self.rows()?.at(workspace_id, positions)
    }
    fn append(&self, workspace_id: &str, events: &[SessionEvent]) -> Result<(), HostError> {
        self.rows()?.append(workspace_id, events)
    }
    fn remove(&self, workspace_id: &str, positions: &[i64]) -> Result<(), HostError> {
        self.rows()?.remove(workspace_id, positions)
    }
    fn clear(&self, workspace_id: &str) -> Result<(), HostError> {
        self.rows()?.clear(workspace_id)
    }
    fn index(&self, workspace_id: &str) -> Result<Option<String>, HostError> {
        self.rows()?.index(workspace_id)
    }
    fn put_index(&self, workspace_id: &str, json: &str) -> Result<(), HostError> {
        self.rows()?.put_index(workspace_id, json)
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        self.rows()?.atomically(f)
    }
}

impl Ctx {
    fn in_index<R>(&self, workspace_id: &str, read: impl FnOnce(&wsp_core::types::internal::TranscriptIndex) -> Option<R>) -> Option<R> {
        self.transcript_state.with(|s| s.index.get(workspace_id).and_then(read))
    }

    /// Every session row's view, in the map's order.
    fn session_entries<R>(&self, f: impl Fn(&SessionEntry) -> R) -> Vec<R> {
        self.sessions.borrow().values().map(|row| f(&row.borrow())).collect()
    }

    fn thread_pairs(&self) -> Vec<(String, String)> {
        self.thread_records.borrow().iter().map(|(id, record)| (id.clone(), record.workspace_id.clone())).collect()
    }

    fn latest_view(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }

    fn module_of(&self, kind: WorkspaceKind) -> Option<Rc<dyn wsp_core::types::internal::KindModule>> {
        self.kinds.get().module_of(kind).ok()
    }
}

fn started(view: &SessionView) -> Option<f64> {
    view.started_at.map(|at| at.0)
}

fn kind_of(entry: &Entry) -> WorkspaceKind {
    entry.borrow().record.borrow().kind
}

impl wsp_threads::threads::transcripts::TranscriptsCtx for Ctx {
    fn clock(&self) -> Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn wsp_core::ids::Ids> {
        self.ids.clone()
    }
    fn bus(&self) -> Rc<dyn wsp_core::types::events::EventBus> {
        self.bus.clone()
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), HostError> {
        self.bus.emit(event).map(|_| ())
    }
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn rows(&self) -> Rc<dyn TranscriptRows> {
        Rc::new(StoreRows(self.store.clone()))
    }
    fn transcript_state(&self) -> wsp_core::context::StateCell<wsp_threads::threads::transcripts::TranscriptState> {
        self.transcript_state.clone()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        self.sessions.clone()
    }
    fn thread_records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        self.thread_records.clone()
    }
    fn index_flushes(&self) -> wsp_core::context::StateCell<IndexMap<String, wsp_core::context::Joined>> {
        self.index_flushes.clone()
    }
    fn entry(&self, workspace_id: &str) -> Option<wsp_threads::threads::threads::Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    fn is_live(&self, workspace_id: &str) -> bool {
        self.live.borrow().contains_key(workspace_id)
    }
    fn closed(&self) -> bool {
        self.state.with(|s| s.closed)
    }
    fn push_head(&self, thread_id: &str) {
        if let Err(e) = self.boot.get().push_head(thread_id) {
            wsp_core::console::warn(&e.message);
        }
    }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.agents.get().rows_on(thread_id)
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.latest_view(thread_id)
    }
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String> {
        self.channels.get().resumed_fact(workspace_id, session, fact)
    }
    fn daemon_token_seed(&self) -> Option<String> {
        self.opts.daemon_token.clone()
    }
    fn rotate_daemon_token(
        &self,
        machine: Arc<dyn wsp_engine::machine::Machine>,
        token: String,
        path: Option<String>,
    ) -> LocalBoxFuture<'static, Result<bool, HostError>> {
        Box::pin(async move { wsp_core::daemon_token::rotate_daemon_token(machine.as_ref(), &token, path.as_deref()).await })
    }
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        spawn_detached(work);
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::channels::ChannelsCtx for Ctx {
    fn index_start(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        self.in_index(workspace_id, |i| i.starts.get(thread_id).cloned())
    }
    fn index_cut(&self, workspace_id: &str, thread_id: &str) -> Option<bool> {
        self.in_index(workspace_id, |i| i.cut.get(thread_id).copied())
    }
    fn index_fact(&self, workspace_id: &str, resume: &str, fact: &str) -> Option<String> {
        self.in_index(workspace_id, |i| {
            let facts = i.facts.get(resume)?;
            match fact {
                "cwd" => facts.cwd.clone(),
                "permissionMode" => facts.permission_mode.clone(),
                "model" => facts.model.clone(),
                "effort" => facts.effort.clone(),
                _ => None,
            }
        })
    }
    fn session_rows(&self) -> Vec<ChannelRow> {
        self.session_entries(|entry| {
            let view = &entry.view;
            let mut facts = IndexMap::new();
            for (key, value) in
                [("cwd", &view.cwd), ("permissionMode", &view.permission_mode), ("model", &view.model), ("effort", &view.effort)]
            {
                if let Some(value) = value {
                    facts.insert(key.to_string(), value.clone());
                }
            }
            ChannelRow {
                workspace_id: view.workspace_id.clone(),
                claude_session_id: view.claude_session_id.clone(),
                facts,
                status: serde_json::to_value(view.status).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
                pid: entry.pid.map(i64::from),
                run: entry.run.clone(),
            }
        })
    }
    fn thread_record(&self, thread_id: &str) -> Option<wsp_core::types::internal::ThreadRecord> {
        self.thread_records.borrow().get(thread_id).cloned()
    }
    fn thread_ids(&self) -> Vec<String> {
        self.thread_records.borrow().keys().cloned().collect()
    }
    fn latest_harness(&self, thread_id: &str) -> Option<String> {
        self.latest_view(thread_id).map(|view| view.harness)
    }
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String> {
        self.module_of(record.kind).and_then(|module| module.home_dir(record))
    }
    async fn daemon_road(&self, entry: &Entry) -> AccountResult<DaemonReachView> {
        let module = self.kinds.get().module_of(kind_of(entry))?;
        let road = module.daemon_road(&entry.borrow());
        Ok(road.await?)
    }
    fn turn_road(&self, kind: WorkspaceKind) -> Option<wsp_core::devices::ScopedRoad> {
        self.module_of(kind).map(|module| module.turn_road())
    }
    fn has_door(&self) -> bool {
        self.wired_door().is_some()
    }
    fn offer_of(&self, place_id: &str) -> Option<String> {
        self.wired_door().and_then(|door| door.offer_of(place_id))
    }
    fn wired(&self) -> String {
        self.places.wired()
    }
    fn settings_at(&self, place_id: &str) -> Option<wsp_wire::views::place_more::PlaceSettings> {
        self.wired_door().map(|door| door.settings_at(place_id))
    }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<i64>> {
        self.wired_door().and_then(|door| door.turn_limit_at(place_id)).map(|limit| limit.map(|n| n as i64))
    }
    fn name_of(&self, place_id: &str) -> String {
        self.wired_door().map_or_else(|| place_id.to_string(), |door| door.name_of(place_id))
    }
    fn joined(&self, place_id: &str) -> bool {
        self.wired_door().is_some_and(|door| door.joined(place_id))
    }
    async fn report_of(&self, place_id: &str) -> Option<wsp_wire::wire::place_link::PlaceReport> {
        let door = self.wired_door()?;
        door.report_of(place_id).await.ok().flatten()
    }
    async fn rows(&self) -> Vec<PlaceView> {
        match self.wired_door() {
            Some(door) => door.rows().await.unwrap_or_default(),
            None => Vec::new(),
        }
    }
    fn link_channel(&self, place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>> {
        self.wired_door()?.channel(place_id, Rc::new(move |event| on_event(Value::Object(event))))
    }
    async fn spend_today(&self, rows: &[PlaceView], now: i64) -> Vec<(String, Option<f64>)> {
        let tracker = self.status_tracker.get();
        match tracker.spend(rows, Some(now)).await {
            Ok(spent) => spent.into_iter().map(|row| (row.place, Some(row.today_usd.0))).collect(),
            Err(said) => {
                wsp_core::console::warn(&said);
                Vec::new()
            }
        }
    }
    fn projects_held(&self) -> IndexMap<String, wsp_wire::views::workspace::ProjectView> {
        self.projects_held.borrow().clone()
    }
    async fn open_channel(&self, url: &str, token: &str, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        let ask = DaemonChannelOptions { url: url.to_string(), token: token.to_string(), on_event, connect_timeout_ms: None, clock: None };
        match self.opts.daemon_channel.clone() {
            Some(open) => Ok(open(ask).await?),
            None => open_daemon_channel(ask).await.map_err(channel_failure),
        }
    }
    fn local_has_daemon(&self) -> bool {
        self.opts.local.as_ref().is_some_and(|local| local.daemon_road.is_some())
    }
    fn local_home_dir(&self) -> String {
        self.opts.local.as_ref().map(|local| local.home_dir.clone()).unwrap_or_default()
    }
    async fn local_road(&self) -> AccountResult<DaemonReachView> {
        self.kinds.get().local_road().await.map_err(|e| e.into_host().into())
    }
    async fn adopt_pause(&self, entry: &Entry) -> AccountResult<()> {
        Ok(self.machines.get().adopt_pause(entry).await?)
    }
    fn broker_ready(&self) -> bool {
        self.opts.broker.as_ref().is_some_and(|broker| broker.ready())
    }
    fn root_of(&self, thread_id: &str) -> String {
        self.rules.get().root_of(thread_id)
    }
    fn device_door(&self) -> Rc<dyn wsp_core::devices::DeviceDoor> {
        self.device_door.clone()
    }
    fn host_pid(&self) -> i64 {
        i64::from(std::process::id())
    }
    fn host_parent_pid(&self) -> i64 {
        i64::from(std::os::unix::process::parent_id())
    }
    fn has_state_path(&self) -> bool {
        self.opts.state_path.is_some()
    }
    fn state_folder(&self) -> String {
        self.kinds.get().state_folder().unwrap_or_default()
    }
    fn read_link(&self, path: &str) -> Option<String> {
        std::fs::read_link(path).ok().map(|target| target.to_string_lossy().into_owned())
    }
    fn hostname(&self) -> String {
        self.hostname.clone()
    }
    fn pid_alive(&self, pid: i64) -> bool {
        i32::try_from(pid).is_ok_and(wsp_core::types::internal::pid_alive)
    }
    fn agent_bins(&self) -> Vec<String> {
        wsp_catalog::catalog::catalog_agents().into_iter().map(|agent| agent.bin.clone()).collect()
    }
    fn agent_pid_of(&self, run: &str) -> Option<i64> {
        wsp_core::local_exec::agent_pid_of(run).map(i64::from)
    }
    fn group_exists(&self, pgid: i64) -> bool {
        i32::try_from(pgid).is_ok_and(wsp_core::local_exec::group_exists)
    }
    fn proc_scopes_moved(&self) {
        self.proc_scopes.get().proc_scopes_moved();
    }
    fn state(&self) -> wsp_core::context::StateCell<wsp_core::context::RuntimeState> {
        self.state.clone()
    }
    fn box_panels_ctx(&self) -> Rc<dyn wsp_account::account::box_panels::BoxPanelsCtx> {
        self.rc()
    }
    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>> {
        Ok(self.store.get(collection, id).await?)
    }
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()> {
        Ok(self.store.put(collection, id, value).await?)
    }
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()> {
        Ok(self.store.delete(collection, id).await?)
    }
    async fn store_keys(&self, collection: &str) -> AccountResult<Vec<String>> {
        Ok(self.store.keys(collection).await?)
    }
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.hear_workspace_deleted(f)
    }
    fn live_has(&self, workspace_id: &str) -> bool {
        self.live.borrow().contains_key(workspace_id)
    }
    async fn thread_launch(&self, entry: &Entry, thread_id: &str, root_thread_id: &str) -> AccountResult<ThreadLaunch> {
        let (workspace_id, kind) = {
            let live = entry.borrow();
            let record = live.record.borrow();
            (record.id.clone(), record.kind)
        };
        let launch = self.core.get().thread_launch(&workspace_id, kind, thread_id, root_thread_id, false).await?;
        Ok(ThreadLaunch { scoped: launch.scoped, env: launch.env })
    }
    fn brokered_env(&self, token: &str, entry: &Entry) -> IndexMap<String, String> {
        let live = entry.borrow();
        let record = live.record.borrow();
        let repo = record.branch_of_record.as_ref().map(|b| RepoOfRecord {
            owner: b.owner.clone(),
            name: b.name.clone(),
            public: b.public.is_some(),
        });
        // No area holds the public marketplaces yet (branch-of-record.ts:80's WeakMap has no slot in `Ctx`): none read.
        brokered_env(token, repo.as_ref(), &[])
    }
    fn terminal_sign_in(&self, harness: &str, entry: &Entry, token: &str) -> Option<TerminalSignIn> {
        let kinds = self.kinds.get();
        let read = |launch_token: Option<&str>| -> Option<LaunchRead> {
            let launch = kinds.brokered_launch(&entry.borrow(), harness, launch_token).ok().flatten()?;
            let claude = launch.claude.map(|b| BrokeredSignIn {
                token: b.token,
                kind: match b.kind {
                    BrokeredKind::Token => BrokerKind::Token,
                    BrokeredKind::Key => BrokerKind::Key,
                    BrokeredKind::Gateway => BrokerKind::Gateway,
                },
            });
            Some(LaunchRead { vault: launch.vault, claude, codex: launch.codex })
        };
        match terminal_sign_in(harness, &read, token, self.clock.now()) {
            Ok(sign_in) => sign_in,
            Err(said) => {
                wsp_core::console::warn(&said);
                None
            }
        }
    }
    fn terminal_line(&self, harness: &str, program: Option<&str>) -> String {
        wsp_machines::machines::brokered::terminal_line(harness, program)
    }
    async fn thread_folder(&self, entry: &Entry) -> AccountResult<String> {
        // `thread_folder` reads the entry before its one return and never awaits, so the borrow is gone when it answers.
        let kinds = self.kinds.get();
        let folder = futures::FutureExt::now_or_never(kinds.thread_folder(&entry.borrow(), None));
        Ok(folder.expect("thread_folder does not await")?)
    }
    fn thread_env(&self, entry: &Entry) -> IndexMap<String, String> {
        self.agents.get().thread_env(entry, None)
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()> {
        Ok(self.records.get().persist(record).await?)
    }
    fn read_checkout_soon(&self, entry: &Entry) {
        let reach = self.reach.get();
        let entry = entry.clone();
        spawn_detached(async move {
            reach.read_checkout(&entry, true).await;
        });
    }
    async fn delete_workspace(&self, workspace_id: &str) -> AccountResult<()> {
        let workspaces = self.workspaces.get();
        Ok(wsp_core::types::api::WorkspacesApi::delete(&*workspaces, workspace_id, None).await?)
    }
    fn git_origin(&self, path: &str, timeout_ms: i64) -> (Option<i32>, String) {
        origin_read(path, timeout_ms)
    }
    fn now(&self) -> i64 {
        self.clock.now()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
    fn warn(&self, line: &str) {
        wsp_core::console::warn(line);
    }
}

/// `spawnSync("git", ["-C", path, "remote", "get-url", "origin"], { encoding: "utf8", timeout })` (channels.ts:914):
/// the exit status and stdout, stopped at the timeout with no status, as node's SIGTERM leaves it.
fn origin_read(path: &str, timeout_ms: i64) -> (Option<i32>, String) {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let spawned = Command::new("git")
        .args(["-C", path, "remote", "get-url", "origin"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = spawned else {
        return (None, String::new());
    };
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        out
    });
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(0) as u64);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let out = reader.join().unwrap_or_default();
    match status {
        Some(status) => (status.code(), String::from_utf8_lossy(&out).into_owned()),
        None => (None, String::new()),
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::proc_scopes::ProcScopesCtx for Ctx {
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)> {
        let groups = self.channels.get().turn_groups.clone();
        let held = groups.borrow();
        held.iter()
            .map(|(workspace, led)| (workspace.clone(), led.iter().map(|(pgid, thread)| (*pgid, thread.clone())).collect()))
            .collect()
    }
    fn group_exists(&self, pgid: i64) -> bool {
        i32::try_from(pgid).is_ok_and(wsp_core::local_exec::group_exists)
    }
    fn sessions(&self) -> Vec<ScopeRow> {
        self.session_entries(|entry| {
            let view = &entry.view;
            ScopeRow {
                workspace_id: view.workspace_id.clone(),
                thread_id: view.thread_id.clone(),
                status: serde_json::to_value(view.status).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
                started_at: started(view),
                pid: entry.pid.map(i64::from),
            }
        })
    }
    fn kept_agents(&self) -> Vec<(String, KeptRow)> {
        let threads = self.threads.get();
        let kept = threads.kept_agents.borrow();
        kept.iter()
            .map(|(thread, process)| {
                let process = process.borrow();
                let row = KeptRow {
                    workspace_id: process.workspace_id.clone(),
                    pid: process.pid.map(i64::from),
                    warm_unclaimed: process.warm.as_ref().is_some_and(|warm| !warm.claimed),
                };
                (thread.clone(), row)
            })
            .collect()
    }
    fn latest_on(&self, thread_id: &str) -> Option<LatestRow> {
        self.latest_view(thread_id).map(|view| LatestRow { workspace_id: view.workspace_id, cwd: view.cwd })
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_pairs()
    }
    fn thread_terminals(&self) -> Vec<wsp_account::account::proc_scopes::ThreadTerminal> {
        self.channels.get().thread_terminals()
    }
    fn live_record(&self, workspace_id: &str) -> Option<ScopeRecord> {
        let entry = self.live.borrow().get(workspace_id).cloned()?;
        let record = entry.borrow().record.borrow().clone();
        let checkout = self.channels.get().checkout_of(&record).unwrap_or_default();
        Some(ScopeRecord { kind: record.kind, place: record.place, checkout })
    }
    fn shared_daemon(&self, kind: WorkspaceKind) -> bool {
        self.module_of(kind).is_some_and(|module| module.shared_daemon())
    }
    async fn channel_here(&self) -> AccountResult<Rc<dyn DaemonChannel>> {
        let road = self.kinds.get().local_road().await.map_err(|e| AccountError::from(e.into_host()))?;
        self.channels.get().channel_over(&road, wsp_words::words::computer::THIS_COMPUTER, Rc::new(|_: Value| {})).await
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::box_panels::BoxPanelsCtx for Ctx {
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.hear_workspace_deleted(f)
    }
    fn session_rows(&self) -> Vec<StartedRow> {
        self.session_entries(|entry| {
            let view = &entry.view;
            StartedRow { workspace_id: view.workspace_id.clone(), thread_id: view.thread_id.clone(), started_at: started(view) }
        })
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_pairs()
    }
    fn link(&self, place_id: &str) -> Option<Rc<()>> {
        let link = self.wired_door()?.link(place_id)?;
        let address = Rc::as_ptr(&link) as *const () as usize;
        LINKS.with(|held| {
            let mut held = held.borrow_mut();
            held.retain(|_, (link, _)| link.strong_count() > 0);
            let standing = held.entry(address).or_insert_with(|| (Rc::downgrade(&link), Rc::new(())));
            Some(standing.1.clone())
        })
    }
    fn name_of(&self, place_id: &str) -> String {
        wsp_account::account::channels::ChannelsCtx::name_of(self, place_id)
    }
    async fn daemon_version(&self, place_id: &str) -> Option<i64> {
        let door = self.wired_door()?;
        door.report_of(place_id).await.ok().flatten().map(|report| report.daemon_version.get())
    }
    async fn watch_sys(&self, place_id: &str) -> Result<Release, String> {
        let door = self.wired_door().ok_or_else(|| "this runtime holds no computers".to_string())?;
        let release = door.watch_sys(place_id, Rc::new(|_| {})).await.map_err(|e| e.message)?;
        let held = RefCell::new(Some(release));
        Ok(Rc::new(move || {
            if let Some(release) = held.borrow_mut().take() {
                release();
            }
        }))
    }
    fn scopes_on(&self, place_id: &str) -> Vec<wsp_wire::wire::daemon::ProcScope> {
        self.proc_scopes.get().scopes_on(place_id)
    }
    fn thread_env(&self, entry: &Entry) -> IndexMap<String, String> {
        self.agents.get().thread_env(entry, None)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
}

impl wsp_threads::threads::trimmed_row::TrimmedCtx for Ctx {
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        self.channels.get().started_as(workspace_id, thread_id)
    }
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String> {
        self.channels.get().folder_of(workspace_id, session)
    }
    fn thread_words(&self, workspace_id: &str, thread_id: &str) -> Option<ThreadWords> {
        self.in_index(workspace_id, |i| i.words.get(thread_id).cloned())
    }
}

impl wsp_threads::threads::waits::WaitsCtx for Ctx {
    fn clock(&self) -> Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        self.sessions.clone()
    }
    fn closing(&self) -> bool {
        self.state.with(|s| s.closing)
    }
    fn record(&self, event: SessionEvent) {
        self.transcripts.get().record(event);
    }
}
