// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/box-panels.ts at 65f6b89c7: the panes of a folder on a joined computer over
//! the one link that computer holds. Spec: spec/box_panels.md.
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::{Rc, Weak};

use async_trait::async_trait;
use futures::future::{join_all, FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use wsp_wire::types::thread_cgroup::in_cgroup;
use wsp_wire::wire::daemon::ProcScope;
use wsp_wire::wire::daemon_version::{named_watch_gap, place_behind_line};
use wsp_words::js::utf16;
use wsp_words::thread_cgroup::thread_cgroup;

use crate::account::core_types::{AccountResult, Entry, Frame, Release, Served};
use crate::account::proc_scopes::{newest_first, poll_now};

/// box-panels.ts:21: the most cgroups one watch names, the daemon's own cap.
pub const WATCH_CGROUPS_MAX: usize = 16;

/// box-panels.ts:37: the ops a pane's channel into a folder there answers itself, in the TS set's order.
pub const PANEL_OPS: [&str; 9] =
    ["ports.watch", "sys.watch", "sys.unwatch", "proc.watch", "proc.unwatch", "proc.inspect", "proc.kill", "container.stop", "pty.create"];

/// B1, box-panels.ts:26. `None` in an ask is "every process"; the answer `None` is the whole table.
pub fn proc_watch_scope(asks: &[Option<Vec<String>>]) -> Option<Vec<String>> {
    let mut named: Vec<String> = Vec::new();
    for ask in asks {
        let cgroups = ask.as_ref()?;
        for cgroup in cgroups {
            if !named.contains(cgroup) {
                named.push(cgroup.clone());
            }
        }
    }
    let mut kept: Vec<String> = named
        .iter()
        .filter(|cgroup| !named.iter().any(|other| other != *cgroup && in_cgroup(Some(cgroup.as_str()), other)))
        .cloned()
        .collect();
    kept.sort_by(|a, b| utf16::cmp(a, b));
    (kept.len() <= WATCH_CGROUPS_MAX).then_some(kept)
}

/// B2, box-panels.ts:50: the refusal frame, or None where the daemon names watches.
pub fn named_watch_refusal(name: &str, daemon_version: Option<i64>) -> Option<Frame> {
    let gap = named_watch_gap(daemon_version)?;
    Some(obj(json!({ "ok": false, "code": "unsupported", "error": place_behind_line(name, &gap) })))
}

/// One thread's session row as the cgroup order reads it (box-panels.ts:70).
#[derive(Clone, Debug, PartialEq)]
pub struct StartedRow {
    pub workspace_id: String,
    pub thread_id: Option<String>,
    pub started_at: Option<f64>,
}

/// What the opener reads off the runtime's context.
#[async_trait(?Send)]
pub trait BoxPanelsCtx {
    /// `bus.on("workspace.deleted", ...)`: `f` gets the deleted workspace's id.
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>);
    /// `ctx.sessions.values()`' views, in map order.
    fn session_rows(&self) -> Vec<StartedRow>;
    /// `ctx.threadRecords`: thread id and workspace id, in map order.
    fn thread_records(&self) -> Vec<(String, String)>;
    /// `placeDoor.link(placeId)`: the link handle as an identity, None where none stands.
    fn link(&self, place_id: &str) -> Option<Rc<()>>;
    /// `placeDoor.nameOf(placeId)`.
    fn name_of(&self, place_id: &str) -> String;
    /// `(await placeDoor.reportOf(placeId))?.daemonVersion`.
    async fn daemon_version(&self, place_id: &str) -> Option<i64>;
    /// `placeDoor.watchSys(placeId, () => {})`: the hold's release, or the reason it was refused.
    async fn watch_sys(&self, place_id: &str) -> Result<Release, String>;
    /// `ctx.scopesOn(placeId)` (proc_scopes P6).
    fn scopes_on(&self, place_id: &str) -> Vec<ProcScope>;
    /// `ctx.threadEnv(entry)`, keys in order.
    fn thread_env(&self, entry: &Entry) -> IndexMap<String, String>;
    /// A `void` promise: runs on the host's thread, nobody waits for it.
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
}

/// box-panels.ts:39 `BoxPanels`: one pane's channel parts.
pub trait BoxPanels {
    fn answers(&self, op: &str) -> bool;
    fn send(&self, frame: Frame) -> futures::future::LocalBoxFuture<'static, AccountResult<Frame>>;
    /// Whether an event the link carried is this pane's.
    fn heard(&self, event: &Value) -> bool;
    fn close(&self);
}

/// The pushes each folder's panes register to name their ports again (`watchers`), by workspace.
pub type Watchers = Rc<RefCell<IndexMap<String, Vec<Rc<dyn Fn()>>>>>;

/// The watch each thread's cgroup is in, per folder (`placements`).
pub type Placements = Rc<RefCell<IndexMap<String, IndexMap<String, usize>>>>;

/// Each link's watching panes and what each asked its process watch to read (`procAsks`), keyed by the link's
/// address and holding the link weakly so an address reused by a new link starts empty.
type ProcAsks = Rc<RefCell<IndexMap<usize, (Weak<()>, IndexMap<u64, Option<Vec<String>>>)>>>;

/// The hold on the computer's readings watch, shared by every reader of the pane, with the ticket that names it.
type Reading = Shared<LocalBoxFuture<'static, Result<Release, String>>>;

/// box-panels.ts:60 `boxPanelsOpener`: one per runtime.
pub struct BoxPanelsOpener {
    pub(crate) ctx: Rc<dyn BoxPanelsCtx>,
    pub(crate) watchers: Watchers,
    pub(crate) placements: Placements,
    proc_asks: ProcAsks,
    next_pane: Cell<u64>,
}

impl BoxPanelsOpener {
    /// Registers the `workspace.deleted` listener that drops a folder's placements (B11).
    pub fn new(ctx: Rc<dyn BoxPanelsCtx>, watchers: Watchers, placements: Placements) -> Rc<BoxPanelsOpener> {
        let opener = Rc::new(BoxPanelsOpener {
            ctx: ctx.clone(),
            watchers,
            placements: placements.clone(),
            proc_asks: Rc::new(RefCell::new(IndexMap::new())),
            next_pane: Cell::new(0),
        });
        ctx.on_workspace_deleted(Rc::new(move |workspace_id: &str| {
            placements.borrow_mut().shift_remove(workspace_id);
        }));
        opener
    }

    /// box-panels.ts:78: one pane on `place_id`'s link, its frames served by `served`, its terminals in `folder`.
    pub fn open(self: &Rc<Self>, entry: &Entry, place_id: &str, served: Served, folder: &str) -> Rc<dyn BoxPanels> {
        let workspace_id = entry.borrow().record.borrow().id.clone();
        let link = self.ctx.link(place_id);
        self.placements.borrow_mut().entry(workspace_id.clone()).or_default();
        let id = self.next_pane.get();
        self.next_pane.set(id + 1);
        let state = Rc::new_cyclic(|weak: &Weak<PaneState>| {
            let weak = weak.clone();
            let push: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(pane) = weak.upgrade() {
                    pane.on_push();
                }
            });
            PaneState {
                id,
                ctx: self.ctx.clone(),
                entry: entry.clone(),
                place_id: place_id.to_string(),
                folder: folder.to_string(),
                workspace_id: workspace_id.clone(),
                served,
                link,
                watchers: self.watchers.clone(),
                placements: self.placements.clone(),
                proc_asks: self.proc_asks.clone(),
                push,
                state: RefCell::new(PaneMut::default()),
            }
        });
        let push = state.push.clone();
        self.watchers.borrow_mut().entry(workspace_id).or_default().push(push);
        Rc::new(OpenPane { state })
    }
}

/// What a pane changes as it runs: its watch flags, what its ports watch last named, and the folder's watches.
#[derive(Default)]
struct PaneMut {
    watching_procs: bool,
    closed: bool,
    named: Option<Named>,
    scoped: Option<String>,
    watches: Vec<String>,
    holders: IndexMap<u64, BTreeSet<String>>,
    readings: Option<(u64, Reading)>,
    next_ticket: u64,
}

/// The roots and cgroups a pane's ports watch last named (box-panels.ts:96).
#[derive(Clone)]
struct Named {
    roots: Vec<Value>,
    cgroups: String,
}

/// One pane's channel into its folder: the state every op and push reads and writes.
struct PaneState {
    id: u64,
    ctx: Rc<dyn BoxPanelsCtx>,
    entry: Entry,
    place_id: String,
    folder: String,
    workspace_id: String,
    served: Served,
    link: Option<Rc<()>>,
    watchers: Watchers,
    placements: Placements,
    proc_asks: ProcAsks,
    push: Rc<dyn Fn()>,
    state: RefCell<PaneMut>,
}

/// The `BoxPanels` a caller holds: the pane's state behind the trait.
struct OpenPane {
    state: Rc<PaneState>,
}

impl BoxPanels for OpenPane {
    fn answers(&self, op: &str) -> bool {
        PANEL_OPS.contains(&op)
    }
    fn send(&self, frame: Frame) -> LocalBoxFuture<'static, AccountResult<Frame>> {
        let state = self.state.clone();
        async move { state.answer(frame).await }.boxed_local()
    }
    fn heard(&self, event: &Value) -> bool {
        self.state.heard(event)
    }
    fn close(&self) {
        self.state.close();
    }
}

fn obj(v: Value) -> Frame {
    match v {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// `watchName(n)`: the folder's id for the first watch, `<id>-<n+1>` after.
fn watch_name(workspace_id: &str, n: usize) -> String {
    if n == 0 {
        workspace_id.to_string()
    } else {
        format!("{workspace_id}-{}", n + 1)
    }
}

/// `String(v)` for an event's watch name.
fn js_string(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Null) => "null".to_string(),
        None => "undefined".to_string(),
        Some(other) => other.to_string(),
    }
}

/// `Number(v)` as a Map key: every NaN one key, and +0 and -0 one key.
fn port_key(v: Option<&Value>) -> u64 {
    let n = match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Null) => 0.0,
        _ => f64::NAN,
    };
    if n.is_nan() {
        f64::NAN.to_bits()
    } else {
        (n + 0.0).to_bits()
    }
}

/// `Number.isInteger(r) ? r : (dropped)` for a root, kept as the JSON number the page sent.
fn integer_of(v: &Value) -> Option<Value> {
    if let Some(n) = v.as_i64() {
        return Some(json!(n));
    }
    let n = v.as_f64().filter(|n| n.fract() == 0.0)?;
    if (i64::MIN as f64..i64::MAX as f64).contains(&n) {
        Some(json!(n as i64))
    } else {
        Some(json!(n))
    }
}

fn bump(room: &mut Vec<usize>, n: usize) {
    if room.len() <= n {
        room.resize(n + 1, 0);
    }
    room[n] += 1;
}

/// `procAsks.get(link)`, created on first use, read and written through `f`, and gone once no pane asks of it.
fn with_asks<T>(asks: &ProcAsks, link: &Rc<()>, f: impl FnOnce(&mut IndexMap<u64, Option<Vec<String>>>) -> T) -> T {
    let key = Rc::as_ptr(link) as usize;
    let mut all = asks.borrow_mut();
    let entry = all.entry(key).or_insert_with(|| (Rc::downgrade(link), IndexMap::new()));
    if !entry.0.upgrade().is_some_and(|live| Rc::ptr_eq(&live, link)) {
        *entry = (Rc::downgrade(link), IndexMap::new());
    }
    let out = f(&mut entry.1);
    if entry.1.is_empty() {
        all.shift_remove(&key);
    }
    out
}

/// `procWatchScope(asksOn(link).values())`.
fn proc_watch_scope_on(asks: &ProcAsks, link: &Rc<()>) -> Option<Vec<String>> {
    with_asks(asks, link, |asks| {
        let all: Vec<Option<Vec<String>>> = asks.values().cloned().collect();
        proc_watch_scope(&all)
    })
}

impl PaneState {
    fn heard(&self, event: &Value) -> bool {
        match event.get("type").and_then(Value::as_str) {
            Some("port.open") | Some("port.close") => self.port_moved(event),
            Some("sys.sample") => self.state.borrow().readings.is_some(),
            Some("proc.snapshot") | Some("proc.changes") => self.state.borrow().watching_procs,
            _ => false,
        }
    }

    /// box-panels.ts:111 `cgroupsOf`: the folder's threads' cgroups, newest first.
    fn cgroups(&self) -> Vec<String> {
        let mut newest: IndexMap<String, f64> = IndexMap::new();
        for row in self.ctx.session_rows() {
            if row.workspace_id != self.workspace_id {
                continue;
            }
            let Some(thread) = row.thread_id else {
                continue;
            };
            let seen = newest.get(&thread).copied().unwrap_or(0.0);
            newest.insert(thread, seen.max(row.started_at.unwrap_or(0.0)));
        }
        for (thread, workspace) in self.ctx.thread_records() {
            if workspace == self.workspace_id && !newest.contains_key(&thread) {
                newest.insert(thread, 0.0);
            }
        }
        let mut ordered: Vec<(String, f64)> = newest.into_iter().collect();
        ordered.sort_by(|a, b| newest_first(a.1, b.1));
        ordered.iter().map(|(thread, _)| thread_cgroup(thread)).collect()
    }

    /// The link this pane rides is still the one the computer holds (`door.link(placeId) !== link`).
    fn link_is_current(&self) -> bool {
        match (&self.link, self.ctx.link(&self.place_id)) {
            (None, None) => true,
            (Some(held), Some(now)) => Rc::ptr_eq(held, &now),
            _ => false,
        }
    }

    /// box-panels.ts:86 `watchOnLink`: the link's one process watch, asked again with what its panes read.
    fn watch_on_link(&self) -> LocalBoxFuture<'static, AccountResult<Frame>> {
        let scope = self.link.as_ref().and_then(|link| proc_watch_scope_on(&self.proc_asks, link));
        let mut frame = obj(json!({ "op": "proc.watch" }));
        if let Some(cgroups) = scope {
            frame.insert("cgroups".to_string(), json!(cgroups));
        }
        (self.served)(frame)
    }

    /// box-panels.ts:129 `watchPorts`: places the folder's cgroups in watches and asks each for its ports.
    async fn watch_ports(&self, roots: Vec<Value>) -> AccountResult<Frame> {
        let cgroups = self.cgroups();
        self.state.borrow_mut().named = Some(Named { roots: roots.clone(), cgroups: cgroups.join(",") });
        let (shares, placed) = self.place(&cgroups);
        let count = shares.max(self.state.borrow().watches.len());
        let names: Vec<String> = (0..count).map(|n| watch_name(&self.workspace_id, n)).collect();
        self.state.borrow_mut().watches = names.iter().take(shares).cloned().collect();
        let asks = names
            .iter()
            .enumerate()
            .map(|(n, watch)| {
                let share: Vec<String> = cgroups.iter().filter(|cgroup| placed.get(*cgroup) == Some(&n)).cloned().collect();
                let frame = if n == 0 {
                    obj(json!({ "op": "ports.watch", "roots": roots, "folder": self.folder, "cgroups": share, "watch": watch }))
                } else {
                    obj(json!({ "op": "ports.watch", "roots": [], "cgroups": share, "watch": watch }))
                };
                (self.served)(frame)
            })
            .collect::<Vec<_>>();
        let said = join_all(asks).await.into_iter().collect::<AccountResult<Vec<Frame>>>()?;
        if let Some(refused) = said.iter().find(|reply| reply.get("ok") != Some(&Value::Bool(true))) {
            return Ok(refused.clone());
        }
        let mut holders: IndexMap<u64, BTreeSet<String>> = IndexMap::new();
        let mut ports: IndexMap<u64, Value> = IndexMap::new();
        for (n, reply) in said.iter().take(shares).enumerate() {
            let rows = match reply.get("ports") {
                Some(Value::Array(rows)) => rows.clone(),
                _ => Vec::new(),
            };
            for row in rows {
                let port = port_key(row.get("port"));
                ports.entry(port).or_insert_with(|| row.clone());
                holders.entry(port).or_default().insert(watch_name(&self.workspace_id, n));
            }
        }
        self.state.borrow_mut().holders = holders;
        let mut out = said.first().cloned().unwrap_or_default();
        out.insert("ports".to_string(), Value::Array(ports.into_values().collect()));
        Ok(out)
    }

    /// B4 placement (box-panels.ts:114-124): keeps each cgroup's watch while its thread stands, gives a new one the
    /// first watch with room, and returns the shares the folder needs with each cgroup's watch.
    fn place(&self, cgroups: &[String]) -> (usize, IndexMap<String, usize>) {
        let mut all = self.placements.borrow_mut();
        let placed = all.entry(self.workspace_id.clone()).or_default();
        placed.retain(|cgroup, _| cgroups.contains(cgroup));
        let mut room: Vec<usize> = Vec::new();
        for n in placed.values() {
            bump(&mut room, *n);
        }
        for cgroup in cgroups {
            if placed.contains_key(cgroup) {
                continue;
            }
            let mut n = 0;
            while room.get(n).copied().unwrap_or(0) >= WATCH_CGROUPS_MAX {
                n += 1;
            }
            placed.insert(cgroup.clone(), n);
            bump(&mut room, n);
        }
        (room.len().max(1), placed.clone())
    }

    /// box-panels.ts:147 `portMoved`: whether a port event from one of the folder's watches is news to the pane.
    fn port_moved(&self, event: &Value) -> bool {
        let watch = js_string(event.get("watch"));
        let mut state = self.state.borrow_mut();
        if !state.watches.contains(&watch) {
            return false;
        }
        let port = port_key(event.get("port"));
        let mut held = state.holders.get(&port).cloned().unwrap_or_default();
        if event.get("type") == Some(&json!("port.open")) {
            let elsewhere = held.iter().any(|other| other != &watch);
            held.insert(watch);
            state.holders.insert(port, held);
            return !elsewhere;
        }
        if !held.remove(&watch) {
            return held.is_empty();
        }
        if !held.is_empty() {
            state.holders.insert(port, held);
            return false;
        }
        state.holders.shift_remove(&port);
        true
    }

    /// box-panels.ts:164 `scopeProcs`: sends `proc.scopes` only where the pane last told something else.
    async fn scope_procs(&self) {
        let scopes = self.ctx.scopes_on(&self.place_id);
        let said = serde_json::to_string(&scopes).unwrap_or_default();
        if self.state.borrow().scoped.as_deref() == Some(said.as_str()) {
            return;
        }
        self.state.borrow_mut().scoped = Some(said);
        let frame = obj(json!({ "op": "proc.scopes", "scopes": serde_json::to_value(&scopes).unwrap_or(Value::Null) }));
        let _ = (self.served)(frame).await;
    }

    /// box-panels.ts:172 `push`: a thread that started since names its cgroup in the watches again.
    fn on_push(self: &Rc<Self>) {
        if self.state.borrow().watching_procs {
            let me = self.clone();
            if let Some(rest) = poll_now(async move { me.scope_procs().await }.boxed_local()) {
                self.ctx.spawn(rest);
            }
        }
        let named = self.state.borrow().named.clone();
        let Some(named) = named else {
            return;
        };
        if self.cgroups().join(",") == named.cgroups {
            return;
        }
        let me = self.clone();
        let ports = async move {
            let _ = me.watch_ports(named.roots).await;
        }
        .boxed_local();
        if let Some(rest) = poll_now(ports) {
            self.ctx.spawn(rest);
        }
    }

    /// box-panels.ts:181 `unwatchProcs`: takes the pane's ask back; the last pane on the link unwatches it.
    fn unwatch_procs(&self) {
        if !self.state.borrow().watching_procs {
            return;
        }
        self.state.borrow_mut().watching_procs = false;
        let Some(link) = self.link.clone() else {
            return;
        };
        let before = proc_watch_scope_on(&self.proc_asks, &link);
        let empty = with_asks(&self.proc_asks, &link, |asks| {
            asks.shift_remove(&self.id);
            asks.is_empty()
        });
        if !self.link_is_current() {
            return;
        }
        if empty {
            let said = (self.served)(obj(json!({ "op": "proc.unwatch" })));
            self.ctx.spawn(
                async move {
                    let _ = said.await;
                }
                .boxed_local(),
            );
        } else if proc_watch_scope_on(&self.proc_asks, &link) != before {
            let said = self.watch_on_link();
            self.ctx.spawn(
                async move {
                    let _ = said.await;
                }
                .boxed_local(),
            );
        }
    }

    /// box-panels.ts:193 `unwatchReadings`: lets the hold go once it has landed, if it ever did.
    fn unwatch_readings(&self) {
        let held = self.state.borrow_mut().readings.take();
        if let Some((_, holding)) = held {
            self.ctx.spawn(
                async move {
                    if let Ok(release) = holding.await {
                        release();
                    }
                }
                .boxed_local(),
            );
        }
    }

    /// box-panels.ts:198 `answer`: what the pane answers itself, and what passes up the link untouched.
    async fn answer(&self, frame: Frame) -> AccountResult<Frame> {
        match frame.get("op").and_then(Value::as_str) {
            Some("ports.watch") => {
                let version = self.ctx.daemon_version(&self.place_id).await;
                let name = self.ctx.name_of(&self.place_id);
                if let Some(refused) = named_watch_refusal(&name, version) {
                    return Ok(refused);
                }
                let roots: Vec<Value> = match frame.get("roots") {
                    Some(Value::Array(items)) => items.iter().filter_map(integer_of).collect(),
                    _ => Vec::new(),
                };
                self.watch_ports(roots).await
            }
            Some("sys.watch") => self.watch_readings().await,
            Some("sys.unwatch") => {
                self.unwatch_readings();
                Ok(obj(json!({ "ok": true })))
            }
            Some("proc.watch") => {
                let cgroups: Option<Vec<String>> = match frame.get("cgroups") {
                    Some(Value::Array(items)) => Some(items.iter().filter_map(|c| c.as_str().map(str::to_string)).collect()),
                    _ => None,
                };
                self.state.borrow_mut().watching_procs = true;
                if let Some(link) = &self.link {
                    with_asks(&self.proc_asks, link, |asks| {
                        asks.insert(self.id, cgroups);
                    });
                }
                self.state.borrow_mut().scoped = None;
                self.scope_procs().await;
                let said = self.watch_on_link().await?;
                if said.get("ok") != Some(&Value::Bool(true)) {
                    self.unwatch_procs();
                }
                Ok(said)
            }
            Some("proc.unwatch") => {
                self.unwatch_procs();
                Ok(obj(json!({ "ok": true })))
            }
            Some("pty.create") => {
                let asked: Map<String, Value> = match frame.get("env") {
                    Some(Value::Object(env)) => env.clone(),
                    Some(Value::Array(items)) => items.iter().enumerate().map(|(i, v)| (i.to_string(), v.clone())).collect(),
                    _ => Map::new(),
                };
                let mut env: Map<String, Value> =
                    self.ctx.thread_env(&self.entry).into_iter().map(|(k, v)| (k, Value::String(v))).collect();
                for (k, v) in asked {
                    env.insert(k, v);
                }
                let cwd = match frame.get("cwd") {
                    Some(Value::String(cwd)) => cwd.clone(),
                    _ => self.folder.clone(),
                };
                let mut out = frame.clone();
                out.insert("cwd".to_string(), Value::String(cwd));
                out.insert("env".to_string(), Value::Object(env));
                (self.served)(out).await
            }
            _ => (self.served)(frame).await,
        }
    }

    /// box-panels.ts:206 `sys.watch`: one hold per pane, reused by a second ask; a failed hold is let go.
    async fn watch_readings(&self) -> AccountResult<Frame> {
        let existing = self.state.borrow().readings.clone();
        let (ticket, holding) = match existing {
            Some(pair) => pair,
            None => {
                let ticket = {
                    let mut state = self.state.borrow_mut();
                    state.next_ticket += 1;
                    state.next_ticket
                };
                let ctx = self.ctx.clone();
                let place = self.place_id.clone();
                let holding: Reading = async move { ctx.watch_sys(&place).await }.boxed_local().shared();
                (ticket, holding)
            }
        };
        self.state.borrow_mut().readings = Some((ticket, holding.clone()));
        match holding.await {
            Ok(_) => Ok(obj(json!({ "ok": true }))),
            Err(message) => {
                let mut state = self.state.borrow_mut();
                if state.readings.as_ref().is_some_and(|(held, _)| *held == ticket) {
                    state.readings = None;
                }
                Ok(obj(json!({ "ok": false, "error": message })))
            }
        }
    }

    /// box-panels.ts:225 `close`: removes the push, takes back the ask and lets the readings go, once.
    fn close(&self) {
        {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return;
            }
            state.closed = true;
        }
        let push = self.push.clone();
        if let Some(pushes) = self.watchers.borrow_mut().get_mut(&self.workspace_id) {
            pushes.retain(|registered| !Rc::ptr_eq(registered, &push));
        }
        self.unwatch_procs();
        self.unwatch_readings();
    }
}
