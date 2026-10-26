// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/activity.ts at 65f6b89c7. Spec: spec/activity.md.
//!
//! The door's three methods are declared here; the wsp-core join copies them into `types::pending::ActivityDoor`
//! (ruling 2 of 6400658264), which the runtime's `activity` member names.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_account::account::core_types::AccountError;
use wsp_catalog::catalog::catalog_agents;
use wsp_core::clock::Clock;
use wsp_core::console;
use wsp_core::local_exec::agent_pid_of;
use wsp_core::types::api::DaemonEventListener;
use wsp_core::types::internal::EndThread;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::activity::{
    activity_of, box_behind_activity_line, box_end_line, end_heads, not_read_line, processes_left, ActivityEnded, ActivityProc,
    ActivityRead, ActivityScope, ActivityThread, ActivityView, EndPlan, LeftThread, ProcessesAsk, BOX_BEHIND_ACTIVITY_FIX, BOX_END_FIX, CLOUD_ACTIVITY_FIX,
    END_GRACE_MS, NOT_READ_FIX, SAME_START_MS,
};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::preferences::ThreadFacts;
use wsp_wire::views::session::{SessionStatus, SessionView};
use wsp_wire::views::workspace::{scope_of, Caller};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_frames::{ProcEntry, ProcKind};
use wsp_wire::wire::daemon::{ProcKillReply, ProcScope};
use wsp_wire::wire::daemon_replies::ProcListReply;
use wsp_wire::wire::machine_link::ProcSnapshot;
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_words::daemon_contract::unknown_op_line;
use wsp_words::exit::{not_found_refusal, usage_refusal, HostError};
use wsp_words::projects::folder_on_joined;
use wsp_words::thread_cgroup::thread_cgroup;
use wsp_words::views::activity::{cloud_activity_line, end_plan, not_joined_activity_line, processes_left_refusal, LeftVerb};
use wsp_wire::views::activity::not_joined_activity_fix;
use wsp_words::words::computer::{thread_word, THIS_COMPUTER};
use wsp_words::workspace_state::is_local_workspace;

use crate::threads::threads::{Entry, Sessions, ThreadRecords};

/// activity.ts:63.
pub const BOX_END_POLL_MS: i64 = 200;
/// activity.ts:66.
pub const SNAPSHOT_WAIT_MS: i64 = 10_000;
/// activity.ts:68.
pub const NO_DAEMON_HERE: &str = "this host runs no daemon on this computer, so it reads no processes";

/// What a read or an End is asked (activity.ts:73).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActivityAsk {
    pub computer: Option<String>,
    pub thread: Option<String>,
    pub thread_ids: Option<Vec<String>>,
}

/// An End's target (activity.ts:74).
#[derive(Clone, Debug, PartialEq)]
pub struct EndAsk {
    pub computer: Option<String>,
    pub pid: u32,
    pub started_at: f64,
}

/// The verb a meet runs before (activity.ts:79).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeetVerb {
    Settle,
    Delete,
}

/// activity.ts:70-80.
#[async_trait(?Send)]
pub trait ActivityDoor {
    async fn read(&self, ask: ActivityAsk, caller: Option<Caller>) -> Result<ActivityView, HostError>;
    async fn end(&self, ask: EndAsk, caller: Option<Caller>) -> Result<ActivityEnded, HostError>;
    async fn meet(&self, thread_ids: &[String], processes: ProcessesAsk, verb: MeetVerb, caller: Option<Caller>) -> Result<Vec<ActivityEnded>, HostError>;
}

/// One daemon channel with its asks (TS `DaemonChannel` read through `overChannel`'s `ask`, which throws a refusal).
pub trait DaemonAsk {
    fn ask(&self, frame: Value) -> futures::future::LocalBoxFuture<'static, Result<Map<String, Value>, AccountError>>;
    fn close(&self);
}

/// What the activity door reads of the runtime.
#[async_trait(?Send)]
pub trait ActivityCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    async fn ready(&self) -> Result<(), HostError>;
    /// Whether the host has a place door (TS `ctx.placeDoor !== undefined`).
    fn has_door(&self) -> bool;
    /// `ctx.placeDoor?.nameOf(place)`.
    fn place_name(&self, place: &str) -> Option<String>;
    async fn place_rows(&self) -> Result<Vec<PlaceView>, HostError>;
    fn joined(&self, place: &str) -> bool;
    /// `door.link(place) !== undefined`.
    fn has_link(&self, place: &str) -> bool;
    /// `ctx.opts.local?.daemonRoad !== undefined`.
    fn has_local_daemon(&self) -> bool;
    /// `ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, onEvent)`, through `ctx.killGuarded(.., true)` where
    /// `guarded`.
    async fn local_channel(&self, on_event: DaemonEventListener, guarded: bool) -> Result<Rc<dyn DaemonAsk>, HostError>;
    /// `door.channel(place, onEvent)`; None where the door has none for it.
    fn box_channel(&self, place: &str, on_event: DaemonEventListener) -> Option<futures::future::LocalBoxFuture<'static, Result<Rc<dyn DaemonAsk>, HostError>>>;
    fn sessions(&self) -> Sessions;
    fn records(&self) -> ThreadRecords;
    fn live_entry(&self, workspace_id: &str) -> Option<Entry>;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn thread_facts(&self, thread_id: &str) -> Option<ThreadFacts>;
    /// `ctx.scopesHere()`: the roots the daemon is told, the one rule.
    fn scopes_here(&self) -> Vec<ProcScope>;
    /// `ctx.keptAgents.get(threadId)?.pid`.
    fn kept_pid(&self, thread_id: &str) -> Option<u32>;
    /// `ctx.reapKept(threadId, { now: true })`.
    fn reap_kept_now(&self, thread_id: &str);
    /// `process.pid`.
    fn host_pid(&self) -> u32;
    fn drives(&self, kind: WorkspaceKind, caller: Option<&Caller>) -> bool;
    fn refusal_for(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> Option<HostError>;
    fn settles_thread(&self, thread_id: &str, caller: Option<&Caller>) -> bool;
    /// `ctx.moduleOf(entry.record.kind).endThread?.(entry, threadId, o)`; None where the kind has none.
    fn end_thread(&self, entry: &Entry, thread_id: &str, o: EndThread) -> Option<futures::future::LocalBoxFuture<'static, Result<Option<String>, HostError>>>;
    /// `ctx.withDaemon(entry, ask => ask(frame))`.
    async fn with_daemon(&self, entry: &Entry, frame: Value) -> Result<Map<String, Value>, HostError>;
    /// `ctx.slates.closeServers(threadId)`.
    fn close_servers(&self, thread_id: &str);
}

/// activity.ts:97.
#[derive(Clone)]
pub struct Activity {
    ctx: Rc<dyn ActivityCtx>,
}

/// The computer a read or an End is on: this one (`place` None), or a box the person joined by its place id.
struct Where {
    name: String,
    place: Option<String>,
}

/// What one proc.kill of a tree answered: the pids signalled, those KILL went to, and those still there.
type Killed = (Vec<u32>, Vec<u32>, Vec<u32>);

fn to_host(e: AccountError) -> HostError {
    match e {
        AccountError::Host(h) => h,
        AccountError::Refusal(r) => HostError::new(r.message),
    }
}

fn message_of(e: &AccountError) -> &str {
    match e {
        AccountError::Host(h) => &h.message,
        AccountError::Refusal(r) => &r.message,
    }
}

async fn sleep_for(clock: &Rc<dyn Clock>, ms: i64) {
    let (done, wake) = futures::channel::oneshot::channel::<()>();
    clock.schedule(Box::new(move || {
        let _ = done.send(());
    }), ms);
    let _ = wake.await;
}

fn to_left(t: ActivityThread) -> LeftThread {
    LeftThread { thread_id: t.thread_id, title: t.title, workspace_id: t.workspace_id, memory: t.memory, cpu: t.cpu, processes: t.processes, on: None }
}

fn to_frames<T: serde::Serialize, U: serde::de::DeserializeOwned>(value: &T) -> Result<U, HostError> {
    let doc = serde_json::to_value(value).map_err(|e| HostError::new(e.to_string()))?;
    serde_json::from_value(doc).map_err(|e| HostError::new(e.to_string()))
}

fn dedup_ints(values: Vec<u32>) -> Vec<JsInt> {
    values.into_iter().collect::<IndexSet<u32>>().into_iter().map(|p| JsInt::from(p as i64)).collect()
}

impl Activity {
    pub fn new(ctx: Rc<dyn ActivityCtx>) -> Self {
        Activity { ctx }
    }

    fn now(&self) -> i64 {
        self.ctx.clock().now()
    }

    /// activity.ts:99-113.
    async fn where_(&self, computer: Option<&str>) -> Result<Where, HostError> {
        let name = self.ctx.place_name(HERE_PLACE_ID).unwrap_or_else(|| THIS_COMPUTER.to_string());
        let Some(computer) = computer else { return Ok(Where { name, place: None }) };
        if computer == HERE_PLACE_ID || computer.to_lowercase() == name.to_lowercase() || computer == THIS_COMPUTER {
            return Ok(Where { name, place: None });
        }
        let rows = if self.ctx.has_door() { self.ctx.place_rows().await? } else { Vec::new() };
        let Some(row) = rows.iter().find(|r| r.id == computer || r.name.to_lowercase() == computer.to_lowercase()) else {
            let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
            return Err(usage_refusal(&no_such_place_refusal(computer, &names), "Name one wsp computers lists."));
        };
        if !(self.ctx.has_door() && self.ctx.joined(&row.id)) {
            return Err(usage_refusal(&not_read_line(&row.name), NOT_READ_FIX));
        }
        Ok(Where { name: row.name.clone(), place: Some(row.id.clone()) })
    }

    /// activity.ts:114-115. An End is on this computer only.
    async fn here(&self, computer: Option<&str>) -> Result<String, HostError> {
        let at = self.where_(computer).await?;
        if at.place.is_some() {
            return Err(usage_refusal(&box_end_line(&at.name), BOX_END_FIX));
        }
        Ok(at.name)
    }

    fn session_rows(&self) -> Vec<(SessionView, Option<String>)> {
        self.ctx.sessions().borrow().values().map(|row| {
            let r = row.borrow();
            (r.view.clone(), r.run.clone())
        }).collect()
    }

    fn workspace_of(&self, thread: &str) -> String {
        let by_record = self.ctx.records().borrow().get(thread).map(|r| r.workspace_id.clone());
        by_record.or_else(|| self.ctx.latest_on(thread).map(|v| v.workspace_id)).unwrap_or_default()
    }

    fn record_of_thread(&self, thread: &str) -> Option<WorkspaceRecord> {
        let entry = self.ctx.live_entry(&self.workspace_of(thread))?;
        let record = {
            let live = entry.borrow();
            let record = live.record.borrow().clone();
            record
        };
        Some(record)
    }

    /// activity.ts:206-218.
    fn reaches(&self, thread: Option<&str>, caller: Option<&Caller>, kind: WorkspaceKind) -> bool {
        match thread {
            None => scope_of(caller).is_none() && self.ctx.drives(kind, caller),
            Some(thread) => {
                let driven = match self.record_of_thread(thread) {
                    None => self.ctx.drives(kind, caller),
                    Some(record) => self.ctx.refusal_for(&record, caller).is_none(),
                };
                driven && self.ctx.settles_thread(thread, caller)
            }
        }
    }

    /// activity.ts:273-282: the box a thread runs on that this host has joined, by its place id.
    fn box_of(&self, thread: &str) -> Option<(String, Entry)> {
        let entry = self.ctx.live_entry(&self.workspace_of(thread))?;
        let (local, place) = {
            let live = entry.borrow();
            let record = live.record.borrow();
            (is_local_workspace(Some(&record.kind)), record.place.clone())
        };
        let place = place?;
        if local || !self.ctx.has_door() || !self.ctx.joined(&place) {
            return None;
        }
        Some((place, entry))
    }

    fn elsewhere(&self, thread: &str) -> bool {
        self.record_of_thread(thread).is_some_and(|record| !is_local_workspace(Some(&record.kind)))
    }

    /// activity.ts:283-284.
    fn unjoined_box_of(&self, thread: &str) -> Option<String> {
        let record = self.record_of_thread(thread)?;
        let place = record.place?;
        if !folder_on_joined(&record.kind) || (self.ctx.has_door() && self.ctx.joined(&place)) {
            return None;
        }
        Some(place)
    }

    fn title_of(&self, thread: &str) -> String {
        self.ctx.thread_facts(thread).map(|facts| facts.title).unwrap_or_else(|| thread_word(thread))
    }

    fn bin_of(&self, thread: &str) -> Option<String> {
        let harness = self.ctx.thread_facts(thread)?.harness;
        catalog_agents().into_iter().find(|agent| agent.id == harness).map(|agent| agent.bin.clone())
    }

    /// activity.ts:168-196.
    fn scopes_here(&self) -> Vec<ActivityScope> {
        let mut running: HashSet<String> = HashSet::new();
        let mut latest: IndexMap<String, (String, f64)> = IndexMap::new();
        let mut recorded: IndexMap<String, Vec<u32>> = IndexMap::new();
        for (view, run) in self.session_rows() {
            let Some(thread) = view.thread_id.clone() else { continue };
            let local = self.ctx.live_entry(&view.workspace_id).is_some_and(|entry| {
                let live = entry.borrow();
                let kind = live.record.borrow().kind;
                is_local_workspace(Some(&kind))
            });
            if !local {
                continue;
            }
            let is_running = view.status == SessionStatus::Running;
            if is_running {
                running.insert(thread.clone());
            }
            if let Some(agent) = run.as_deref().filter(|_| is_running).and_then(agent_pid_of).and_then(|pid| u32::try_from(pid).ok()) {
                recorded.entry(thread.clone()).or_default().push(agent);
            }
            let at = view.started_at.as_ref().map_or(0.0, |n| n.0);
            if latest.get(&thread).map_or(-1.0, |(_, t)| *t) <= at {
                latest.insert(thread, (view.workspace_id.clone(), at));
            }
        }
        let mut scopes: Vec<ActivityScope> = Vec::new();
        for s in self.ctx.scopes_here() {
            if s.warm == Some(true) {
                continue;
            }
            let kept = if running.contains(&s.thread) { None } else { self.ctx.kept_pid(&s.thread) };
            let mut roots: IndexSet<u32> = IndexSet::new();
            for pid in s.agents.iter().flatten().chain(s.groups.iter().flatten()) {
                roots.insert(pid.get() as u32);
            }
            let workspace_id = s.workspace.clone().or_else(|| latest.get(&s.thread).map(|(w, _)| w.clone())).unwrap_or_default();
            scopes.push(ActivityScope {
                thread_id: s.thread.clone(),
                title: self.title_of(&s.thread),
                workspace_id,
                roots: roots.into_iter().collect(),
                kept,
                cgroup: None,
                bin: self.bin_of(&s.thread),
                agents: recorded.get(&s.thread).cloned(),
            });
        }
        let named: HashSet<String> = scopes.iter().map(|s| s.thread_id.clone()).collect();
        for (thread, (workspace_id, _)) in &latest {
            if named.contains(thread) {
                continue;
            }
            scopes.push(ActivityScope {
                thread_id: thread.clone(),
                title: self.title_of(thread),
                workspace_id: workspace_id.clone(),
                roots: Vec::new(),
                kept: None,
                cgroup: None,
                bin: self.bin_of(thread),
                agents: None,
            });
        }
        let started = |thread: &str| latest.get(thread).map_or(0.0, |(_, t)| *t);
        scopes.sort_by(|a, b| {
            let (ra, rb) = (running.contains(&a.thread_id) as i32, running.contains(&b.thread_id) as i32);
            (rb - ra).cmp(&0).then_with(|| started(&b.thread_id).partial_cmp(&started(&a.thread_id)).unwrap_or(std::cmp::Ordering::Equal))
        });
        scopes
    }

    /// activity.ts:116-156. One snapshot on one channel, which the caller closes (`over_channel`). The fields the
    /// caller fills (computer, host, scopes, agent bins) are left empty.
    async fn snapshot(&self, place: Option<&str>, once: bool, watch: bool) -> Result<ActivityRead, HostError> {
        let (sent, got) = futures::channel::oneshot::channel::<ProcSnapshot>();
        let sent = Rc::new(RefCell::new(Some(sent)));
        let listener: DaemonEventListener = Rc::new(move |event: Map<String, Value>| {
            if event.get("type") != Some(&json!("proc.snapshot")) {
                return;
            }
            let Ok(snap) = serde_json::from_value::<ProcSnapshot>(Value::Object(event)) else { return };
            if let Some(sent) = sent.borrow_mut().take() {
                let _ = sent.send(snap);
            }
        });
        let channel = match place {
            None => {
                if !self.ctx.has_local_daemon() {
                    return Err(HostError::new(NO_DAEMON_HERE));
                }
                self.ctx.local_channel(listener, false).await?
            }
            Some(place) => match self.ctx.box_channel(place, listener) {
                Some(open) => open.await?,
                None => return Err(HostError::new(format!("{} is away", self.ctx.place_name(place).unwrap_or_else(|| place.to_string())))),
            },
        };
        let read = self.snapshot_over(&*channel, once, watch, got).await;
        channel.close();
        read
    }

    async fn snapshot_over(&self, channel: &dyn DaemonAsk, once: bool, watch: bool, got: futures::channel::oneshot::Receiver<ProcSnapshot>) -> Result<ActivityRead, HostError> {
        let listening = channel.ask(json!({ "op": "ports.watch" })).await.map_err(to_host)?;
        let mut ports: HashMap<u32, Vec<JsInt>> = HashMap::new();
        for row in listening.get("ports").and_then(Value::as_array).into_iter().flatten() {
            let (Some(pid), Some(port)) = (row.get("pid").and_then(Value::as_u64), row.get("port").and_then(Value::as_u64)) else { continue };
            let list = ports.entry(pid as u32).or_default();
            list.push(JsInt::from(port as i64));
            list.sort_by_key(|p| p.get());
        }
        if once {
            match channel.ask(json!({ "op": "proc.list" })).await {
                Ok(listed) => {
                    let reply: ProcListReply = serde_json::from_value(Value::Object(listed)).map_err(|e| HostError::new(e.to_string()))?;
                    return Ok(ActivityRead {
                        computer: String::new(),
                        at: reply.at,
                        procs: to_frames(&reply.procs)?,
                        daemon: reply.daemon.get() as u32,
                        host: 0,
                        scopes: Vec::new(),
                        agent_bins: Vec::new(),
                        ports: Some(ports),
                        cgroups: to_frames(&reply.cgroups)?,
                    });
                }
                Err(e) if message_of(&e) == unknown_op_line("proc.list") => {}
                Err(e) => return Err(to_host(e)),
            }
        }
        if !watch {
            return Err(HostError::new(unknown_op_line("proc.list")));
        }
        channel.ask(json!({ "op": "proc.watch" })).await.map_err(to_host)?;
        let (fire, late) = futures::channel::oneshot::channel::<()>();
        let cancel = self.ctx.clock().schedule(Box::new(move || {
            let _ = fire.send(());
        }), SNAPSHOT_WAIT_MS);
        let outcome = futures::future::select(got, late).await;
        cancel();
        match outcome {
            futures::future::Either::Left((Ok(snap), _)) => Ok(ActivityRead {
                computer: String::new(),
                at: snap.at,
                procs: to_frames(&snap.procs)?,
                daemon: snap.daemon.get() as u32,
                host: 0,
                scopes: Vec::new(),
                agent_bins: Vec::new(),
                ports: Some(ports),
                cgroups: to_frames(&snap.cgroups)?,
            }),
            _ => Err(HostError::new(format!("{THIS_COMPUTER}'s daemon sent no processes in {} s", SNAPSHOT_WAIT_MS / 1000))),
        }
    }

    /// activity.ts:286-290: a computer's read, its threads from this computer's scopes.
    async fn read_of(&self, computer: &str, once: bool) -> Result<ActivityRead, HostError> {
        let mut read = self.snapshot(None, once, true).await?;
        read.computer = computer.to_string();
        read.host = self.ctx.host_pid();
        read.scopes = self.scopes_here();
        read.agent_bins = catalog_agents().iter().map(|a| a.bin.clone()).collect();
        read.cgroups = None;
        Ok(read)
    }

    /// activity.ts:222-245.
    async fn box_read(&self, place: &str, threads: &[String]) -> Result<(ActivityRead, ActivityView), HostError> {
        let name = self.ctx.place_name(place).unwrap_or_else(|| place.to_string());
        if !self.ctx.has_door() || !self.ctx.has_link(place) {
            return Err(HostError::new(format!("{name} is away")));
        }
        let mut read = match self.snapshot(Some(place), true, false).await {
            Ok(read) => read,
            Err(e) if e.message == unknown_op_line("proc.list") => {
                return Err(usage_refusal(&box_behind_activity_line(&name), BOX_BEHIND_ACTIVITY_FIX));
            }
            Err(e) => return Err(e),
        };
        read.computer = name.clone();
        read.host = 0;
        read.scopes = threads
            .iter()
            .map(|id| {
                let workspace_id = self.box_of(id).map(|(_, entry)| {
                    let live = entry.borrow();
                    let record = live.record.borrow();
                    record.id.clone()
                });
                ActivityScope {
                    thread_id: id.clone(),
                    title: self.title_of(id),
                    workspace_id: workspace_id.unwrap_or_default(),
                    roots: Vec::new(),
                    kept: None,
                    cgroup: Some(thread_cgroup(id)),
                    bin: self.bin_of(id),
                    agents: None,
                }
            })
            .collect();
        read.agent_bins = catalog_agents().iter().map(|a| a.bin.clone()).collect();
        let view = activity_of(&read);
        Ok((read, view))
    }

    /// activity.ts:246-253: a settle's or a delete's read of a box never fails the verb.
    async fn box_read_quiet(&self, place: &str, threads: &[String]) -> Option<(ActivityRead, ActivityView)> {
        match self.box_read(place, threads).await {
            Ok(read) => Some(read),
            Err(e) => {
                let name = self.ctx.place_name(place).unwrap_or_else(|| place.to_string());
                console::warn(&format!("{name}'s processes were not read for a settle or a delete: {}", e.message));
                None
            }
        }
    }

    /// activity.ts:254-261: every thread a box runs, newest first.
    fn threads_on(&self, place: &str) -> Vec<String> {
        let mut latest: IndexMap<String, f64> = IndexMap::new();
        for (view, _) in self.session_rows() {
            let Some(thread) = view.thread_id.clone() else { continue };
            if self.box_of(&thread).map(|(p, _)| p).as_deref() != Some(place) {
                continue;
            }
            let at = view.started_at.as_ref().map_or(0.0, |n| n.0);
            let known = latest.get(&thread).copied().unwrap_or(0.0);
            latest.insert(thread, known.max(at));
        }
        let mut ordered: Vec<(String, f64)> = latest.into_iter().collect();
        ordered.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ordered.into_iter().map(|(thread, _)| thread).collect()
    }

    /// activity.ts:262-271: the processes of a thread on a box still standing after its end.
    async fn standing_on(&self, place: &str, t: &ActivityThread) -> Vec<i64> {
        let after = self.box_read_quiet(place, std::slice::from_ref(&t.thread_id)).await;
        t.processes
            .iter()
            .filter(|p| match &after {
                None => true,
                Some((read, _)) => read.procs.iter().any(|q| q.pid as i64 == p.pid.get() && (q.started_at as f64 - p.started_at.0).abs() <= SAME_START_MS as f64),
            })
            .map(|p| p.pid.get())
            .collect()
    }

    /// activity.ts:294-313: a box thread's end.
    async fn end_on_box(&self, place: &str, entry: &Entry, t: &ActivityThread, read: &ActivityRead) -> Result<ActivityEnded, HostError> {
        if let Some(ended) = self.ctx.end_thread(entry, &t.thread_id, EndThread::default()) {
            ended.await?;
        }
        let listed = |q: &ProcEntry| t.processes.iter().any(|p| p.pid.get() == q.pid as i64 && (q.started_at as f64 - p.started_at.0).abs() <= SAME_START_MS as f64);
        let ptys: Vec<String> = read.procs.iter().filter(|q| listed(q)).filter_map(|q| q.pty.clone()).collect();
        for pty in ptys {
            if let Err(e) = self.ctx.with_daemon(entry, json!({ "op": "pty.kill", "ptyId": pty })).await {
                console::warn(&format!("shell {pty} of thread {} was not ended: {}", thread_word(&t.thread_id), e.message));
            }
        }
        self.ctx.close_servers(&t.thread_id);
        let mut left = self.standing_on(place, t).await;
        let by = self.now() + END_GRACE_MS as i64;
        while !left.is_empty() && self.now() < by {
            sleep_for(&self.ctx.clock(), BOX_END_POLL_MS).await;
            left = self.standing_on(place, t).await;
        }
        Ok(ActivityEnded {
            pid: t.processes.first().map(|p| p.pid).unwrap_or_default(),
            ended: t.processes.clone(),
            forced: Vec::new(),
            left: left.into_iter().map(JsInt::from).collect(),
            freed: None,
        })
    }

    /// activity.ts:338-347: the views a read answers, with the caller's reach applied.
    async fn view_of(&self, o: &ActivityAsk, caller: Option<&Caller>, once: bool) -> Result<(ActivityView, ActivityRead), HostError> {
        self.ctx.ready().await?;
        let asked: Option<Vec<String>> = match &o.thread {
            Some(thread) => Some(vec![thread.clone()]),
            None => o.thread_ids.clone(),
        };
        for id in asked.iter().flatten() {
            let known = self.ctx.records().borrow().get(id).is_some() || self.ctx.thread_facts(id).is_some();
            if !known || !self.reaches(Some(id.as_str()), caller, WorkspaceKind::Local) {
                return Err(not_found_refusal(&format!("no thread {}", thread_word(id))));
            }
        }
        let box_of = o.thread.as_deref().and_then(|thread| self.box_of(thread));
        let unjoined = match &o.thread {
            Some(thread) if box_of.is_none() => self.unjoined_box_of(thread),
            _ => None,
        };
        if let (Some(thread), Some(place)) = (&o.thread, &unjoined) {
            let name = self.ctx.place_name(place).unwrap_or_else(|| place.clone());
            return Err(usage_refusal(&not_joined_activity_line(thread, &name), &not_joined_activity_fix(&name)));
        }
        if let Some(thread) = &o.thread {
            if box_of.is_none() && self.elsewhere(thread) {
                return Err(usage_refusal(&cloud_activity_line(thread), CLOUD_ACTIVITY_FIX));
            }
        }
        let named = self.where_(o.computer.as_deref()).await?;
        let at = match (&o.computer, &box_of) {
            (None, Some((place, _))) => Where { name: self.ctx.place_name(place).unwrap_or_else(|| place.clone()), place: Some(place.clone()) },
            _ => named,
        };
        let box_place = box_of.as_ref().map(|(place, _)| place.clone());
        if let Some(thread) = &o.thread {
            if at.place != box_place {
                return Err(not_found_refusal(&format!("no thread {} on {}", thread_word(thread), at.name)));
            }
        }
        let on_box = at.place.is_some();
        let wanted: Option<IndexSet<String>> = asked.as_ref().map(|ids| {
            ids.iter()
                .filter(|id| if on_box { self.box_of(id).map(|(p, _)| p) == at.place } else { !self.elsewhere(id) })
                .cloned()
                .collect()
        });
        let (read, view) = match &at.place {
            Some(place) => {
                let ids: Vec<String> = match &wanted {
                    Some(w) => w.iter().cloned().collect(),
                    None => self.threads_on(place),
                };
                self.box_read(place, &ids).await?
            }
            None => {
                let read = self.read_of(&at.name, once).await?;
                let view = activity_of(&read);
                (read, view)
            }
        };
        let kind = if on_box { WorkspaceKind::Place } else { WorkspaceKind::Local };
        let threads = view
            .threads
            .iter()
            .filter(|t| wanted.as_ref().is_none_or(|w| w.contains(&t.thread_id)) && self.reaches(Some(t.thread_id.as_str()), caller, kind))
            .cloned()
            .collect();
        let whole = wanted.is_none() && self.reaches(None, caller, kind);
        let view = ActivityView {
            threads,
            wsp: if whole { view.wsp.clone() } else { Vec::new() },
            unowned: if whole { view.unowned.clone() } else { Vec::new() },
            ..view
        };
        Ok((view, read))
    }

    /// activity.ts:298-303: one proc.kill per head, all sent at once; a head already gone is not an error.
    async fn kill_trees(&self, heads: &[ProcEntry]) -> Result<Vec<Killed>, HostError> {
        if !self.ctx.has_local_daemon() {
            return Err(HostError::new(NO_DAEMON_HERE));
        }
        let channel = self.ctx.local_channel(Rc::new(|_: Map<String, Value>| {}), true).await?;
        let answers = futures::future::join_all(heads.iter().map(|p| self.kill_one(&*channel, p))).await;
        channel.close();
        answers.into_iter().collect()
    }

    async fn kill_one(&self, channel: &dyn DaemonAsk, p: &ProcEntry) -> Result<Killed, HostError> {
        let mut frame = json!({ "op": "proc.kill", "pid": p.pid, "signal": "TERM", "mode": "tree", "startedAt": p.started_at });
        if let Some(identity) = &p.id {
            frame["identity"] = json!(identity);
        }
        let reply = match channel.ask(frame).await {
            Ok(reply) => Some(reply),
            Err(AccountError::Refusal(r)) if r.code.as_deref() == Some("not-found") => None,
            Err(e) => return Err(to_host(e)),
        };
        let Some(reply) = reply else { return Ok((vec![], vec![], vec![])) };
        let r: ProcKillReply = serde_json::from_value(Value::Object(reply)).map_err(|e| HostError::new(e.to_string()))?;
        let ints = |v: Option<Vec<JsInt>>| v.unwrap_or_default().iter().map(|n| n.get() as u32).collect::<Vec<u32>>();
        let signalled = r.signalled.map(|v| v.iter().map(|n| n.get() as u32).collect()).unwrap_or_else(|| vec![p.pid]);
        Ok((signalled, ints(r.forced), ints(r.left)))
    }

    /// activity.ts:325-336.
    fn ended_of(view: &ActivityView, plan: &[ProcEntry], pid: u32, done: &Killed) -> ActivityEnded {
        let rows: Vec<&ActivityProc> = view.threads.iter().flat_map(|t| t.processes.iter()).chain(view.unowned.iter()).collect();
        let row_of = |q: &ProcEntry| -> ActivityProc {
            match rows.iter().find(|r| r.pid.get() == q.pid as i64) {
                Some(r) => (*r).clone(),
                None => ActivityProc {
                    pid: JsInt::from(q.pid as i64),
                    ppid: JsInt::from(q.ppid as i64),
                    depth: JsInt::from(0),
                    started_at: JsNumber(q.started_at as f64),
                    kind: ProcKind::Other,
                    name: q.comm.clone(),
                    command: if q.cmdline.is_empty() { q.comm.clone() } else { q.cmdline.clone() },
                    memory: JsNumber(q.rss as f64),
                    cpu: JsNumber(q.cpu),
                    ports: Vec::new(),
                },
            }
        };
        let ended = plan.iter().filter(|q| done.0.contains(&q.pid)).map(row_of).collect();
        ActivityEnded { pid: JsInt::from(pid as i64), ended, forced: dedup_ints(done.1.clone()), left: dedup_ints(done.2.clone()), freed: None }
    }

    /// activity.ts:348-365: an End off a read already taken, its plan made before any signal.
    async fn end_on(&self, read: &ActivityRead, pid: u32, started_at: f64, caller: Option<&Caller>) -> Result<ActivityEnded, HostError> {
        let may = |thread: Option<&str>| self.reaches(thread, caller, WorkspaceKind::Local);
        let plan = end_plan(read, pid as f64, started_at, &may)?;
        let view = activity_of(read);
        match plan {
            EndPlan::Kept(kept) => {
                self.ctx.reap_kept_now(&kept.thread_id);
                let freed = view
                    .threads
                    .iter()
                    .find(|t| t.thread_id == kept.thread_id)
                    .map(|t| t.processes.iter().filter(|p| p.kind == ProcKind::Agent).cloned().collect())
                    .unwrap_or_default();
                Ok(ActivityEnded { pid: JsInt::from(pid as i64), ended: freed, forced: Vec::new(), left: Vec::new(), freed: Some(kept.thread_id) })
            }
            EndPlan::Targets(targets) => {
                let done = self.kill_trees(&targets[..targets.len().min(1)]).await?.into_iter().next().unwrap_or_default();
                Ok(Self::ended_of(&view, &targets, pid, &done))
            }
        }
    }

    /// activity.ts:367-421.
    async fn meet_impl(&self, thread_ids: &[String], processes: ProcessesAsk, verb: MeetVerb, caller: Option<&Caller>) -> Result<Vec<ActivityEnded>, HostError> {
        if matches!(processes, ProcessesAsk::Keep) {
            return Ok(Vec::new());
        }
        let local: Vec<String> = thread_ids.iter().filter(|id| !self.elsewhere(id)).cloned().collect();
        let here = if local.is_empty() {
            None
        } else {
            Some(self.view_of(&ActivityAsk { computer: None, thread: None, thread_ids: Some(local.clone()) }, caller, true).await?)
        };
        let mut left: Vec<LeftThread> = match &here {
            Some((view, _)) => processes_left(view, &local).into_iter().map(to_left).collect(),
            None => Vec::new(),
        };
        let mut boxes: IndexMap<String, Vec<String>> = IndexMap::new();
        for id in thread_ids {
            let Some((place, _)) = self.box_of(id) else { continue };
            if !self.reaches(Some(id.as_str()), caller, WorkspaceKind::Local) {
                return Err(not_found_refusal(&format!("no thread {}", thread_word(id))));
            }
            boxes.entry(place).or_default().push(id.clone());
        }
        let mut on_boxes: Vec<(String, ActivityThread, ActivityRead)> = Vec::new();
        for (place, ids) in &boxes {
            let Some((read, view)) = self.box_read_quiet(place, ids).await else { continue };
            for t in processes_left(&view, ids) {
                let lt = to_left(t.clone());
                left.push(LeftThread { on: Some(read.computer.clone()), ..lt });
                on_boxes.push((place.clone(), t, read.clone()));
            }
        }
        if left.is_empty() {
            return Ok(Vec::new());
        }
        let mut theirs: IndexSet<u32> = IndexSet::new();
        let mut reach: Option<HostError> = None;
        let mut plans: Vec<(u32, Vec<ProcEntry>)> = Vec::new();
        if let Some((view, read)) = &here {
            for t in processes_left(view, &local) {
                for p in end_heads(&t.processes) {
                    let pid = p.pid.get() as u32;
                    let unreached = Cell::new(false);
                    let may = |thread: Option<&str>| {
                        let ok = self.reaches(thread, caller, WorkspaceKind::Local);
                        if !ok {
                            unreached.set(true);
                        }
                        ok
                    };
                    let planned = match end_plan(read, pid as f64, p.started_at.0, &may) {
                        Ok(EndPlan::Targets(targets)) => Ok(targets),
                        Ok(EndPlan::Kept(kept)) => Err(HostError::new(format!(
                            "{pid} is the agent thread {} keeps, which a settle frees itself",
                            thread_word(&kept.thread_id)
                        ))),
                        Err(e) => Err(e),
                    };
                    match planned {
                        Ok(targets) => plans.push((pid, targets)),
                        Err(e) => {
                            if !unreached.get() {
                                return Err(e);
                            }
                            theirs.insert(pid);
                            reach.get_or_insert(e);
                        }
                    }
                }
            }
        }
        if matches!(processes, ProcessesAsk::Ask) {
            let leftverb = match verb {
                MeetVerb::Settle => LeftVerb::Settle,
                MeetVerb::Delete => LeftVerb::Delete,
            };
            return Err(processes_left_refusal(leftverb, &left, Some(&theirs)));
        }
        if let Some(e) = reach {
            return Err(e);
        }
        let mut ended: Vec<ActivityEnded> = Vec::new();
        if let Some((view, _)) = &here {
            if !plans.is_empty() {
                let heads: Vec<ProcEntry> = plans.iter().filter_map(|(_, targets)| targets.first().cloned()).collect();
                let done = self.kill_trees(&heads).await?;
                for (i, (pid, targets)) in plans.iter().enumerate() {
                    let answer = done.get(i).cloned().unwrap_or_default();
                    ended.push(Self::ended_of(view, targets, *pid, &answer));
                }
            }
        }
        for (place, t, read) in on_boxes {
            let Some((_, entry)) = self.box_of(&t.thread_id) else { continue };
            ended.push(self.end_on_box(&place, &entry, &t, &read).await?);
        }
        Ok(ended)
    }
}

#[async_trait(?Send)]
impl ActivityDoor for Activity {
    /// activity.ts:358. Rules AC1, AC2, AC4-AC20, AC22-AC25.
    async fn read(&self, ask: ActivityAsk, caller: Option<Caller>) -> Result<ActivityView, HostError> {
        Ok(self.view_of(&ask, caller.as_ref(), false).await?.0)
    }

    /// activity.ts:362. Rules AC3, AC26, AC27.
    async fn end(&self, ask: EndAsk, caller: Option<Caller>) -> Result<ActivityEnded, HostError> {
        self.ctx.ready().await?;
        let computer = self.here(ask.computer.as_deref()).await?;
        let read = self.read_of(&computer, false).await?;
        self.end_on(&read, ask.pid, ask.started_at, caller.as_ref()).await
    }

    /// activity.ts:367. Rules AC28-AC32, AC21.
    async fn meet(&self, thread_ids: &[String], processes: ProcessesAsk, verb: MeetVerb, caller: Option<Caller>) -> Result<Vec<ActivityEnded>, HostError> {
        self.meet_impl(thread_ids, processes, verb, caller.as_ref()).await
    }
}
