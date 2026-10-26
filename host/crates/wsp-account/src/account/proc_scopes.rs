// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/proc-scopes.ts at 65f6b89c7: the threads this host names to a computer's
//! daemon in proc.scopes. Spec: spec/proc_scopes.md.
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::rc::{Rc, Weak};

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::daemon::{ProcKind, ProcScope, ProcScopePid, PROC_SCOPES_MAX, PROC_SCOPE_PIDS_MAX};
use wsp_words::thread_cgroup::thread_cgroup;
use wsp_words::workspace_state::is_local_workspace;

use crate::account::core_types::{AccountResult, Frame, Release};

/// One session row as the scopes read it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeRow {
    pub workspace_id: String,
    pub thread_id: Option<String>,
    /// `view.status`.
    pub status: String,
    pub started_at: Option<f64>,
    /// The agent process the host launched for the turn.
    pub pid: Option<i64>,
}

/// A kept agent (`ctx.keptAgents`): its workspace, its pid, and whether it is a warm one no send has claimed.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptRow {
    pub workspace_id: String,
    pub pid: Option<i64>,
    /// `kept.warm?.claimed === false`.
    pub warm_unclaimed: bool,
}

/// A shell a thread's pane opened (`ctx.threadTerminals()`).
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadTerminal {
    pub thread: String,
    pub workspace: String,
    pub pid: i64,
}

/// A live record as the scopes read it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeRecord {
    pub kind: WorkspaceKind,
    pub place: Option<String>,
    /// `ctx.checkoutOf(record)`.
    pub checkout: String,
}

/// `ctx.latestOn(threadId)`: the thread's latest row's workspace and cwd.
#[derive(Clone, Debug, PartialEq)]
pub struct LatestRow {
    pub workspace_id: String,
    pub cwd: Option<String>,
}

/// What the area reads off the runtime's context.
#[async_trait(?Send)]
pub trait ProcScopesCtx {
    /// `ctx.turnGroups`: workspace, then group pid and thread, in map order.
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)>;
    /// `groupExists(pid)` (wsp_core::local_exec::group_exists).
    fn group_exists(&self, pgid: i64) -> bool;
    /// `ctx.sessions.values()`, in map order.
    fn sessions(&self) -> Vec<ScopeRow>;
    /// `ctx.keptAgents`, in map order.
    fn kept_agents(&self) -> Vec<(String, KeptRow)>;
    fn latest_on(&self, thread_id: &str) -> Option<LatestRow>;
    /// `ctx.threadRecords`: thread and workspace, in map order.
    fn thread_records(&self) -> Vec<(String, String)>;
    fn thread_terminals(&self) -> Vec<ThreadTerminal>;
    /// `ctx.live.get(id)?.record`.
    fn live_record(&self, workspace_id: &str) -> Option<ScopeRecord>;
    /// `ctx.moduleOf(kind).sharedDaemon`.
    fn shared_daemon(&self, kind: WorkspaceKind) -> bool;
    /// `ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, () => {})`: fails where no daemon is wired here.
    async fn channel_here(&self) -> AccountResult<Rc<dyn DaemonChannel>>;
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
}

/// The dial to this computer's daemon that a standing root holds, shared by every move while it stands.
type Standing = Shared<LocalBoxFuture<'static, Option<Rc<dyn DaemonChannel>>>>;

/// proc-scopes.ts:28 `procScopesArea(ctx)`.
pub struct ProcScopesArea {
    pub(crate) ctx: Rc<dyn ProcScopesCtx>,
    /// proc-scopes.ts:30 `owned`: pid → thread and kind.
    pub(crate) owned: RefCell<IndexMap<i64, (String, ProcKind)>>,
    /// proc-scopes.ts:32 `pushes`.
    pub(crate) pushes: RefCell<Vec<Rc<dyn Fn()>>>,
    /// proc-scopes.ts:35 `standing`: the dial to this computer's daemon, shared by every move while it stands, and
    /// the scopes' JSON it was last told.
    pub(crate) standing:
        RefCell<Option<(futures::future::Shared<futures::future::LocalBoxFuture<'static, Option<Rc<dyn DaemonChannel>>>>, String)>>,
    /// The identity of the standing dial, so a dial that ends clears only itself (`standing === held`).
    standing_id: Cell<u64>,
    standing_seq: Cell<u64>,
}

/// One draft of a thread's scope (proc-scopes.ts:17).
#[derive(Default)]
struct Draft {
    workspace: Option<String>,
    agents: IndexSet<i64>,
    groups: IndexSet<i64>,
    pids: Vec<ProcScopePid>,
    warm: bool,
    live: bool,
}

/// `draft(threadId, workspace, live)` (proc-scopes.ts:94): the thread's draft, made on first sight.
fn draft<'a>(drafts: &'a mut IndexMap<String, Draft>, thread: &str, workspace: Option<&str>, live: bool) -> &'a mut Draft {
    let d = drafts.entry(thread.to_string()).or_insert_with(|| Draft { live, ..Draft::default() });
    if d.workspace.is_none() {
        d.workspace = workspace.map(str::to_string);
    }
    d.live |= live;
    d
}

/// The one frame the scopes go out in (proc-scopes.ts:42, 66, 180).
fn scopes_frame(scopes: &[ProcScope]) -> Frame {
    let mut frame = Frame::new();
    frame.insert("id".to_string(), Value::Null);
    frame.insert("op".to_string(), json!("proc.scopes"));
    frame.insert("scopes".to_string(), serde_json::to_value(scopes).unwrap_or(Value::Null));
    frame
}

/// A scope list cut to the daemon's cap, the first ones kept (`cap`, proc-scopes.ts:134).
fn cap<T: Clone>(list: impl Iterator<Item = T>, max: i64) -> Vec<T> {
    list.take(max as usize).collect()
}

/// What `scopesOn` keeps of a thread (proc-scopes.ts:150).
struct Placed {
    workspace: String,
    folder: String,
    started_at: f64,
}

/// The state a watching channel keeps (proc-scopes.ts:176): what it last told the daemon.
struct ScopedState {
    ctx: Rc<dyn ProcScopesCtx>,
    scopes: Rc<dyn Fn() -> Vec<ProcScope>>,
    channel: Rc<dyn DaemonChannel>,
    told: RefCell<Option<String>>,
}

impl ScopedState {
    /// proc-scopes.ts:179 `tell`: the scopes, sent only where their JSON moved.
    async fn tell(&self) {
        let now = (self.scopes)();
        let said = serde_json::to_string(&now).unwrap_or_default();
        if self.told.borrow().as_deref() == Some(said.as_str()) {
            return;
        }
        *self.told.borrow_mut() = Some(said);
        let _ = self.channel.send(scopes_frame(&now)).await;
    }
}

/// The channel `scopedProcs` returns (proc-scopes.ts:192).
struct ScopedChannel {
    state: Rc<ScopedState>,
    stop: Rc<dyn Fn()>,
}

#[async_trait(?Send)]
impl DaemonChannel for ScopedChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        match frame.get("op").and_then(Value::as_str) {
            Some("proc.watch") => {
                *self.state.told.borrow_mut() = Some(String::new());
                self.state.tell().await;
            }
            Some("proc.unwatch") => *self.state.told.borrow_mut() = None,
            _ => {}
        }
        self.state.channel.send(frame).await
    }
    fn close(&self) {
        (self.stop)();
        self.state.channel.close();
    }
    async fn closed(&self) -> ChannelEnd {
        self.state.channel.closed().await
    }
}

impl ProcScopesArea {
    pub fn new(ctx: Rc<dyn ProcScopesCtx>) -> Rc<ProcScopesArea> {
        Rc::new(ProcScopesArea {
            ctx,
            owned: RefCell::new(IndexMap::new()),
            pushes: RefCell::new(Vec::new()),
            standing: RefCell::new(None),
            standing_id: Cell::new(0),
            standing_seq: Cell::new(0),
        })
    }

    /// P5, proc-scopes.ts:74.
    pub fn own_process(self: &Rc<Self>, thread_id: &str, pid: i64, kind: ProcKind) -> Release {
        self.owned.borrow_mut().insert(pid, (thread_id.to_string(), kind));
        self.proc_scopes_moved();
        let area = self.clone();
        let thread = thread_id.to_string();
        Rc::new(move || {
            let held = area.owned.borrow().get(&pid).map(|(held, _)| held.clone());
            if held.as_deref() != Some(thread.as_str()) {
                return;
            }
            area.owned.borrow_mut().shift_remove(&pid);
            area.proc_scopes_moved();
        })
    }

    /// P8, proc-scopes.ts:69: the standing channel told, then every watching channel's push.
    pub fn proc_scopes_moved(self: &Rc<Self>) {
        self.stand_here();
        let pushes: Vec<Rc<dyn Fn()>> = self.pushes.borrow().clone();
        for push in pushes {
            push();
        }
    }

    /// P1-P4, proc-scopes.ts:92.
    pub fn scopes_here(&self) -> Vec<ProcScope> {
        let mut drafts: IndexMap<String, Draft> = IndexMap::new();
        for (workspace_id, groups) in self.ctx.turn_groups() {
            for (group, thread_id) in groups {
                if self.ctx.group_exists(group) {
                    draft(&mut drafts, &thread_id, Some(&workspace_id), true).groups.insert(group);
                }
            }
        }
        for row in self.ctx.sessions() {
            if row.status != "running" {
                continue;
            }
            if let (Some(pid), Some(thread)) = (row.pid, row.thread_id.as_deref()) {
                draft(&mut drafts, thread, Some(&row.workspace_id), true).agents.insert(pid);
            }
        }
        for (thread_id, kept) in self.ctx.kept_agents() {
            let warm = kept.warm_unclaimed;
            if kept.pid.is_none() && !warm {
                continue;
            }
            let d = draft(&mut drafts, &thread_id, Some(&kept.workspace_id), true);
            if let Some(pid) = kept.pid {
                d.agents.insert(pid);
            }
            d.warm |= warm;
        }
        let owned: Vec<(i64, (String, ProcKind))> = self.owned.borrow().iter().map(|(pid, owner)| (*pid, owner.clone())).collect();
        for (pid, (thread_id, kind)) in owned {
            let workspace = self.ctx.latest_on(&thread_id).map(|latest| latest.workspace_id).or_else(|| self.thread_workspace(&thread_id));
            draft(&mut drafts, &thread_id, workspace.as_deref(), true).pids.push(ProcScopePid { pid: JsInt::from(pid), kind });
        }
        for terminal in self.ctx.thread_terminals() {
            let local = self.ctx.live_record(&terminal.workspace).is_some_and(|record| is_local_workspace(Some(&record.kind)));
            if local {
                draft(&mut drafts, &terminal.thread, Some(&terminal.workspace), true)
                    .pids
                    .push(ProcScopePid { pid: JsInt::from(terminal.pid), kind: ProcKind::Terminal });
            }
        }
        let mut newest: IndexMap<String, (String, f64)> = IndexMap::new();
        for row in self.ctx.sessions() {
            let Some(thread) = row.thread_id.clone() else {
                continue;
            };
            let Some(record) = self.ctx.live_record(&row.workspace_id) else {
                continue;
            };
            if !self.ctx.shared_daemon(record.kind) {
                continue;
            }
            let at = row.started_at.unwrap_or(0.0);
            let best = newest.get(&row.workspace_id).map_or(-1.0, |(_, best)| *best);
            if best < at {
                newest.insert(row.workspace_id.clone(), (thread, at));
            }
        }
        for (workspace_id, (thread_id, _)) in newest {
            draft(&mut drafts, &thread_id, Some(&workspace_id), false);
        }
        let mut ordered: Vec<(String, Draft)> = drafts.into_iter().collect();
        ordered.sort_by_key(|(_, d)| !d.live);
        ordered.truncate(PROC_SCOPES_MAX as usize);
        ordered
            .into_iter()
            .map(|(thread, d)| {
                let folder = self.folder_of(&thread, d.workspace.as_deref());
                ProcScope {
                    thread,
                    workspace: d.workspace,
                    agents: (!d.agents.is_empty()).then(|| cap(d.agents.iter().map(|pid| JsInt::from(*pid)), PROC_SCOPE_PIDS_MAX)),
                    groups: (!d.groups.is_empty()).then(|| cap(d.groups.iter().map(|pid| JsInt::from(*pid)), PROC_SCOPE_PIDS_MAX)),
                    folder,
                    warm: d.warm.then_some(true),
                    cgroup: None,
                    pids: (!d.pids.is_empty()).then(|| cap(d.pids.into_iter(), PROC_SCOPE_PIDS_MAX)),
                }
            })
            .collect()
    }

    /// P6, proc-scopes.ts:149.
    pub fn scopes_on(&self, place_id: &str) -> Vec<ProcScope> {
        let consider = |at: &mut IndexMap<String, Placed>, thread: &str, workspace: &str, started_at: f64| {
            let Some(record) = self.ctx.live_record(workspace) else {
                return;
            };
            if record.place.as_deref() != Some(place_id) {
                return;
            }
            if at.get(thread).map_or(-1.0, |placed| placed.started_at) >= started_at {
                return;
            }
            let folder = self.ctx.latest_on(thread).and_then(|latest| latest.cwd).unwrap_or_else(|| record.checkout.clone());
            at.insert(thread.to_string(), Placed { workspace: workspace.to_string(), folder, started_at });
        };
        let mut at: IndexMap<String, Placed> = IndexMap::new();
        for row in self.ctx.sessions() {
            if let Some(thread) = row.thread_id.as_deref() {
                consider(&mut at, thread, &row.workspace_id, row.started_at.unwrap_or(0.0));
            }
        }
        for (thread, workspace) in self.ctx.thread_records() {
            if !at.contains_key(&thread) {
                consider(&mut at, &thread, &workspace, 0.0);
            }
        }
        let mut terminals: IndexMap<String, Vec<ProcScopePid>> = IndexMap::new();
        for terminal in self.ctx.thread_terminals() {
            if self.ctx.live_record(&terminal.workspace).and_then(|record| record.place).as_deref() != Some(place_id) {
                continue;
            }
            if !at.contains_key(&terminal.thread) {
                consider(&mut at, &terminal.thread, &terminal.workspace, 0.0);
            }
            terminals.entry(terminal.thread).or_default().push(ProcScopePid { pid: JsInt::from(terminal.pid), kind: ProcKind::Terminal });
        }
        let mut ordered: Vec<(String, Placed)> = at.into_iter().collect();
        ordered.sort_by(|a, b| b.1.started_at.partial_cmp(&a.1.started_at).unwrap_or(Ordering::Equal));
        ordered.truncate(PROC_SCOPES_MAX as usize);
        ordered
            .into_iter()
            .map(|(thread, placed)| {
                let pids = terminals.get(&thread).map(|list| cap(list.iter().cloned(), PROC_SCOPE_PIDS_MAX));
                ProcScope {
                    cgroup: Some(thread_cgroup(&thread)),
                    thread,
                    workspace: Some(placed.workspace),
                    agents: None,
                    groups: None,
                    folder: Some(placed.folder),
                    warm: None,
                    pids,
                }
            })
            .collect()
    }

    /// P7, proc-scopes.ts:176.
    pub fn scoped_procs(self: &Rc<Self>, scopes: Rc<dyn Fn() -> Vec<ProcScope>>, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        let state = Rc::new(ScopedState { ctx: self.ctx.clone(), scopes, channel: channel.clone(), told: RefCell::new(None) });
        let push: Rc<dyn Fn()> = {
            let state = state.clone();
            Rc::new(move || {
                if state.told.borrow().is_some() {
                    let me = state.clone();
                    state.ctx.spawn(async move { me.tell().await }.boxed_local());
                }
            })
        };
        self.pushes.borrow_mut().push(push.clone());
        let stop: Rc<dyn Fn()> = {
            let weak: Weak<ProcScopesArea> = Rc::downgrade(self);
            Rc::new(move || {
                if let Some(area) = weak.upgrade() {
                    area.pushes.borrow_mut().retain(|registered| !Rc::ptr_eq(registered, &push));
                }
            })
        };
        {
            let stop = stop.clone();
            self.ctx.spawn(
                async move {
                    let _ = channel.closed().await;
                    stop();
                }
                .boxed_local(),
            );
        }
        Rc::new(ScopedChannel { state, stop })
    }

    /// The workspace a thread's record names (`ctx.threadRecords.get(threadId)?.workspaceId`).
    fn thread_workspace(&self, thread_id: &str) -> Option<String> {
        self.ctx.thread_records().into_iter().find(|(thread, _)| thread == thread_id).map(|(_, workspace)| workspace)
    }

    /// proc-scopes.ts:84 `folderOf`: the thread's latest cwd, else the workspace's checkout.
    fn folder_of(&self, thread_id: &str, workspace_id: Option<&str>) -> Option<String> {
        let record = self.ctx.live_record(workspace_id?)?;
        Some(self.ctx.latest_on(thread_id).and_then(|latest| latest.cwd).unwrap_or(record.checkout))
    }

    /// proc-scopes.ts:36 `standHere`: the standing channel is told the scopes while any of them names a process, and
    /// closed once none does.
    fn stand_here(self: &Rc<Self>) {
        let now = self.scopes_here();
        if !now.iter().any(|scope| scope.agents.is_some() || scope.groups.is_some() || scope.pids.is_some()) {
            let was = self.standing.borrow_mut().take();
            self.standing_id.set(0);
            if let Some((dial, _)) = was {
                self.ctx.spawn(
                    async move {
                        if let Some(channel) = dial.await {
                            let _ = channel.send(scopes_frame(&now)).await;
                            channel.close();
                        }
                    }
                    .boxed_local(),
                );
            }
            return;
        }
        let said = serde_json::to_string(&now).unwrap_or_default();
        if self.standing.borrow().as_ref().is_some_and(|(_, told)| *told == said) {
            return;
        }
        if self.standing.borrow().is_none() {
            let id = self.standing_seq.get() + 1;
            self.standing_seq.set(id);
            let dial = self.dial_here(id);
            *self.standing.borrow_mut() = Some((dial, String::new()));
            self.standing_id.set(id);
        }
        let dial = {
            let mut standing = self.standing.borrow_mut();
            let dial = standing.as_mut().map(|(dial, told)| {
                *told = said;
                dial.clone()
            });
            dial
        };
        if let Some(dial) = dial {
            self.ctx.spawn(
                async move {
                    if let Some(channel) = dial.await {
                        let _ = channel.send(scopes_frame(&now)).await;
                    }
                }
                .boxed_local(),
            );
        }
    }

    /// The dial the standing channel holds: a channel that closes, or a dial that fails, clears the standing one that
    /// is still `id`.
    fn dial_here(self: &Rc<Self>, id: u64) -> Standing {
        let weak = Rc::downgrade(self);
        let ctx = self.ctx.clone();
        async move {
            match ctx.channel_here().await {
                Ok(channel) => {
                    let closing = channel.clone();
                    let watch = weak.clone();
                    ctx.spawn(
                        async move {
                            let _ = closing.closed().await;
                            if let Some(area) = watch.upgrade() {
                                area.clear_standing(id);
                            }
                        }
                        .boxed_local(),
                    );
                    Some(channel)
                }
                Err(_) => {
                    if let Some(area) = weak.upgrade() {
                        area.clear_standing(id);
                    }
                    None
                }
            }
        }
        .boxed_local()
        .shared()
    }

    fn clear_standing(&self, id: u64) {
        if self.standing_id.get() == id {
            self.standing.borrow_mut().take();
            self.standing_id.set(0);
        }
    }
}
