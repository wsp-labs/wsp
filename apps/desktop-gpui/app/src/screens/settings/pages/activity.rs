// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Activity (`activity/`): every thread's processes on one computer, with their memory, CPU, energy and
//! ports. Drawing the page holds that computer's daemon link and its process watch; leaving it drops the page's state,
//! which closes the link, so nothing samples while it is closed. The totals carry a line of the last minutes, kept for
//! as long as the page stands and dropped with it.

mod live;
pub mod model;
mod pieces;
mod place;
mod tables;
mod wire;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::channel::{mpsc, oneshot};
use futures::StreamExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, AnyWindowHandle, App, Context, Entity, FontWeight, SharedString, Subscription, Task, Window,
};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{ActivityEndRequest, PlaceView, SessionsInterruptRequest, SlatesGetRequest};
use wsp_host_client::HostClient;

use crate::screens::app_shell::title::thread_title;
use crate::screens::settings::model::req;
use crate::screens::settings::page::{CardData, Ctx, DialogBuild};
use crate::screens::settings::words::HERE;
use crate::screens::settings::Acts;
use crate::screens::sidebar::menu::{absent_of, place_of};
use crate::ui::catalog;
use crate::ui::context_menu::ContextMenu;
use crate::ui::fmt_bytes;
use crate::ui::select::{Select, SelectItem, SelectState};
use crate::ui::{
    narrow, text, ActivePalette, AlertDialog, Button, Card, CardItem, ComputerIcon, Glyph, Notice, NoticeAction, Segmented, Skeleton,
    Sparkline, Status, Switch, Width,
};
pub(crate) use live::wait as redial;
use live::{Arrival, Held, Listing, Reads, Target};
use model::{
    age_word, cpu_tint, energy_tint, energy_word, filter_groups, flat_rows, fmt_elapsed, kind_groups, memory_share_tint, pick_choices,
    port_rows, ports_by_pid, push_history, scope_of, sort_groups, total_spent, words as w, Group, HistoryPoint, Picks, Proc, Scope,
    ScopeInput, Sort, SubState, SubagentIn, Thread, Tint, View, HISTORY_POINTS,
};
use place::{thread_cgroup, THREAD_CGROUPS};
use tables::{tint_ink, Hand, ThreadLooks};
use wire::{apply_changes, apply_port_event, memory_of, AppPart, Frame, Kind, KnownPort, Pid, Snapshot};

/// The page's view and the computer it reads (`activityView`, `activityPlace`).
#[derive(Default)]
pub struct Pick {
    pub view: View,
    pub place: Option<String>,
}

/// Each window's pick, kept across Settings closing for as long as the window stands, as the web's store keeps it.
#[derive(Default)]
struct Kept(HashMap<gpui_kit::WindowId, Entity<Pick>>);

impl gpui_kit::Global for Kept {}

/// The pick `window` last left, or a fresh one.
pub fn kept(window: &Window, cx: &mut App) -> Entity<Pick> {
    if !cx.has_global::<Kept>() {
        cx.set_global(Kept::default());
        cx.on_window_closed(|cx, closed| {
            cx.global_mut::<Kept>().0.remove(&closed);
        })
        .detach();
    }
    let id = window.window_handle().window_id();
    if let Some(pick) = cx.global::<Kept>().0.get(&id) {
        return pick.clone();
    }
    let pick = cx.new(|_| Pick::default());
    cx.global_mut::<Kept>().0.insert(id, pick.clone());
    pick
}

pub fn cards(ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> Vec<CardData> {
    let pick = ctx.activity.clone();
    let places: Vec<(PlaceView, ComputerIcon)> =
        ctx.data.places.iter().map(|p| (p.clone(), ComputerIcon::of(p, ctx.data.preferences()))).collect();
    let models: Vec<(String, String, String)> = ctx
        .data
        .harnesses
        .iter()
        .flat_map(|h| h.models.iter().map(move |m| (h.harness.to_string(), m.value.clone(), m.label.clone())))
        .collect();
    vec![CardData::new("activity").body(Page { pick, places, models, acts: ctx.acts.clone() })]
}

/// The page's views at the top bar's right end; under 640 px, where four segments would cover the crumbs, the same views
/// in a select.
pub fn head(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let pick = ctx.activity.clone();
    let view = pick.read(cx).view;
    if narrow::width(window).sm {
        let items: Items<View> = View::ALL.into_iter().map(|v| (v, SharedString::from(w::views(v)), None)).collect();
        let state = synced("activity-views-narrow".into(), items, view, window, cx);
        let select = Select::new(&state).on_change(move |v, _, cx| {
            let v = *v;
            pick.update(cx, |p, cx| {
                p.view = v;
                cx.notify();
            })
        });
        return Some(div().debug_selector(|| "activity-views-narrow".into()).child(select).into_any_element());
    }
    let control = View::ALL
        .into_iter()
        .fold(
            Segmented::new("activity-views", View::ALL.iter().position(|v| *v == view))
                .height(px(36.))
                .segment_size((px(14.), px(20.)), px(14.)),
            |control, v| control.segment(w::views(v)),
        )
        .on_pick(move |ix, _, cx| {
            pick.update(cx, |p, cx| {
                p.view = View::ALL[ix];
                cx.notify();
            })
        });
    Some(control.into_any_element())
}

/// A computer Activity reads: the one the host runs on over its own link, or a box over a link of one of its folders.
#[derive(Clone, Debug)]
struct Computer {
    place: PlaceView,
    glyph: ComputerIcon,
    key: String,
    workspace_id: Option<String>,
}

impl Computer {
    fn here(&self) -> bool {
        self.place.id == HERE
    }

    fn name(&self) -> String {
        super::computers::place_name(&self.place)
    }
}

fn field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// The rows as the sidebar's `place_of` reads them.
fn listed(places: &[(PlaceView, ComputerIcon)]) -> Vec<Value> {
    places.iter().filter_map(|(p, _)| serde_json::to_value(p).ok()).collect()
}

impl Pick {
    /// `pickActivityPlace`: the computer the page reads, picked from outside it.
    pub fn pick_place(&mut self, place: String) {
        self.place = Some(place);
    }
}

/// The host's workspaces as a computer's page last read them, for whether it links to Activity.
#[derive(Default)]
struct LinkRead {
    asked: bool,
    workspaces: Vec<Value>,
}

/// Whether a computer's page links to Activity (`activityComputers`): this computer, or a box a folder of which is
/// on it.
pub fn lists_computer(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> bool {
    if place.id == HERE {
        return true;
    }
    if serde_json::to_value(place.kind).ok().as_ref().and_then(Value::as_str) == Some("provider") {
        return false;
    }
    let read = window.use_keyed_state(SharedString::from(format!("activity-link-{}", place.id)), cx, |_, _| LinkRead::default());
    if !read.update(cx, |r, _| std::mem::replace(&mut r.asked, true)) {
        let read = read.clone();
        ctx.acts.call(
            crate::screens::settings::model::req::<wsp_host_client::protocol::WorkspacesListRequest>(json!({})),
            cx,
            move |answer: Result<wsp_host_client::ops::sidebar::Workspaces, String>, cx| {
                let workspaces = answer.map(|w| w.workspaces.iter().filter_map(|w| serde_json::to_value(w).ok()).collect());
                read.update(cx, |r, cx| {
                    r.workspaces = workspaces.unwrap_or_default();
                    cx.notify();
                });
            },
        );
    }
    let places: Vec<(PlaceView, ComputerIcon)> =
        ctx.data.places.iter().map(|p| (p.clone(), ComputerIcon::of(p, ctx.data.preferences()))).collect();
    computers(&places, &read.read(cx).workspaces).iter().any(|c| c.place.id == place.id)
}

/// `activityComputers`: this computer, and each box a folder of which is on it.
fn computers(places: &[(PlaceView, ComputerIcon)], workspaces: &[Value]) -> Vec<Computer> {
    let listed = listed(places);
    places
        .iter()
        .filter_map(|(place, glyph)| {
            let facts = serde_json::to_value(place).ok()?;
            if place.id == HERE {
                return Some(Computer { place: place.clone(), glyph: *glyph, key: HERE.into(), workspace_id: None });
            }
            if field(&facts, "kind") == Some("provider") {
                return None;
            }
            let folder = workspaces
                .iter()
                .find(|w| field(w, "kind") == Some("place") && place_of(w, &listed).as_deref() == Some(place.id.as_str()))?;
            let id = field(folder, "id")?.to_owned();
            Some(Computer { place: place.clone(), glyph: *glyph, key: id.clone(), workspace_id: Some(id) })
        })
        .collect()
}

/// The thread's title as the sidebar gives it, else the id its first turn goes by.
fn title_of(first: &Value, latest: &Value) -> String {
    thread_title(field(latest, "harnessTitle"), field(first, "prompt"))
        .unwrap_or_else(|| field(first, "claudeSessionId").or(field(first, "id")).unwrap_or_default().to_owned())
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.)
}

/// `useActivityThreads` with `foldThreads`: the threads of every folder on the computer, and each one's status mark.
fn threads_on(listing: &Listing, places: &[(PlaceView, ComputerIcon)], computer: &Computer) -> (Vec<Thread>, Latest) {
    let mut threads = Vec::new();
    let mut latests = Vec::new();
    let listed = listed(places);
    for w in &listing.workspaces {
        if place_of(w, &listed).as_deref() != Some(computer.place.id.as_str()) {
            continue;
        }
        let Some(wid) = field(w, "id") else { continue };
        let project = w.pointer("/project/name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let mut order: Vec<String> = Vec::new();
        let mut turns: HashMap<String, Vec<&Value>> = HashMap::new();
        for s in listing.sessions.iter().filter(|s| field(s, "workspaceId") == Some(wid)) {
            let key = field(s, "threadId").or(field(s, "id")).unwrap_or_default().to_owned();
            if !turns.contains_key(&key) {
                order.push(key.clone());
            }
            turns.entry(key).or_default().push(s);
        }
        for id in order {
            let mut rows = turns.remove(&id).unwrap_or_default();
            rows.sort_by(|a, b| {
                let at = |s: &Value| s.get("startedAt").and_then(Value::as_f64).unwrap_or(0.);
                at(a).partial_cmp(&at(b)).unwrap_or(std::cmp::Ordering::Equal)
            });
            let (first, latest) = (rows[0], rows[rows.len() - 1]);
            let title = title_of(first, latest);
            let status = field(latest, "status").unwrap_or_default();
            let pid = latest.get("pid").and_then(Value::as_i64);
            let agent_pid = latest.get("agentPid").and_then(Value::as_i64);
            let subagents =
                latest.get("subagents").and_then(Value::as_array).map(|a| a.iter().filter_map(subagent_in).collect()).unwrap_or_default();
            let here = computer.here();
            threads.push(Thread {
                id: id.clone(),
                title,
                harness: field(first, "harness").unwrap_or_default().to_owned(),
                project: project.clone(),
                workspace_id: wid.to_owned(),
                session_id: field(latest, "id").unwrap_or_default().to_owned(),
                working: status == "running",
                pid: pid.filter(|_| here),
                agent_pid: agent_pid.filter(|_| here),
                cgroup: (!here).then(|| thread_cgroup(&id)),
                parent: field(first, "parentThreadId").map(str::to_owned),
                slate: None,
                subagents,
            });
            latests.push((id, latest.clone()));
        }
    }
    (threads, latests)
}

fn subagent_in(s: &Value) -> Option<SubagentIn> {
    let state = match field(s, "state")? {
        "running" => SubState::Running,
        "done" => SubState::Done,
        "failed" => SubState::Failed,
        _ => SubState::Stopped,
    };
    let tokens = s
        .pointer("/usage/tokens")
        .map(|t| t.get("input").and_then(Value::as_f64).unwrap_or(0.) + t.get("output").and_then(Value::as_f64).unwrap_or(0.));
    Some(SubagentIn {
        id: field(s, "id")?.to_owned(),
        title: field(s, "title").unwrap_or_default().to_owned(),
        state,
        started_at: s.get("startedAt").and_then(Value::as_f64).unwrap_or(0.),
        ended_at: s.get("endedAt").and_then(Value::as_f64),
        tokens,
        duration_ms: s.pointer("/usage/durationMs").and_then(Value::as_f64),
        model: field(s, "model").map(str::to_owned),
    })
}

/// A thread head's status mark: what it asks, its running clock, its failure or unread finish, else its age.
fn status_of(latest: &Value, now: f64) -> Status {
    let started = latest.get("startedAt").and_then(Value::as_f64);
    let ended = latest.get("endedAt").and_then(Value::as_f64);
    let status = field(latest, "status").unwrap_or_default();
    if latest.get("asking").is_some_and(|a| !a.is_null()) {
        return Status::NeedsYou;
    }
    if status == "running" {
        return Status::Working(fmt_elapsed(now - started.unwrap_or(now)).into());
    }
    if status == "failed" {
        return Status::Failed;
    }
    let read = latest.get("readAt").and_then(Value::as_f64);
    if status == "completed" && ended.is_some_and(|e| read.is_none_or(|r| r < e)) {
        return Status::Done;
    }
    Status::Read(ended.or(started).map_or_else(String::new, |at| age_word(at, now)).into())
}

/// Each thread's newest turn, which its status mark reads.
type Latest = Vec<(String, Value)>;

/// The threads and the computer a page reads, kept while the page is drawn.
struct Lists {
    listing: Option<Listing>,
    /// The threads of the computer last asked for, read again only when the lists or the computer move.
    cache: Option<(String, Vec<Thread>, Latest)>,
    _follow: Option<Held>,
    _receive: Task<()>,
}

impl Lists {
    fn new(acts: &Acts, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = mpsc::unbounded();
        let follow = acts.host(cx).map(|(host, runtime)| live::follow_lists(host, &runtime, tx));
        let receive = cx.spawn(async move |this, cx| {
            while let Some(listing) = rx.next().await {
                if this.update(cx, |l, cx| take_listing(l, listing, cx)).is_err() {
                    break;
                }
            }
        });
        Lists { listing: None, cache: None, _follow: follow, _receive: receive }
    }
}

fn take_listing(lists: &mut Lists, listing: Listing, cx: &mut Context<Lists>) {
    lists.listing = Some(listing);
    lists.cache = None;
    cx.notify();
}

impl Lists {
    fn threads(&mut self, places: &[(PlaceView, ComputerIcon)], computer: &Computer) -> (Vec<Thread>, Latest) {
        let fresh = self.cache.as_ref().is_some_and(|(id, _, _)| *id == computer.place.id);
        if !fresh {
            let Some(listing) = &self.listing else { return Default::default() };
            let (threads, latests) = threads_on(listing, places, computer);
            self.cache = Some((computer.place.id.clone(), threads, latests));
        }
        self.cache.as_ref().map(|(_, t, l)| (t.clone(), l.clone())).unwrap_or_default()
    }
}

/// What the shown groups were made from: the scope's read, the filter, the picks and the sort.
type ShownKey = (u64, String, Picks, Sort);

/// One computer's page: its link, its frame read into groups, the page's picks and folds, and what is ending.
pub struct Scoped {
    computer: Computer,
    acts: Acts,
    host: Option<(Arc<HostClient>, Arc<Runtime>)>,
    channel: Option<String>,
    snapshot: Option<Snapshot>,
    resyncing: bool,
    unavailable: Option<String>,
    ports: Vec<KnownPort>,
    mem: Option<(f64, f64)>,
    app: HashMap<Pid, AppPart>,
    threads: Vec<Thread>,
    scope: Option<Scope>,
    /// Counts the scope's reads, so the filtered and sorted groups are made again only when something moved.
    generation: u64,
    shown: Option<(ShownKey, Rc<Vec<Group>>)>,
    history: Vec<HistoryPoint>,
    interval: f64,
    sort: Sort,
    filter: Entity<InputState>,
    filter_text: String,
    picks: Picks,
    every: bool,
    open: HashSet<String>,
    ending: HashSet<Pid>,
    stopping: HashSet<String>,
    /// Processes a daemon too old for tree kills signalled alone, said in one notice once the wait is over.
    too_old: Vec<Proc>,
    too_old_wait: Option<Task<()>>,
    /// Each slated thread's title once `slates.get` answered, and the threads already asked.
    slates: HashMap<String, String>,
    slates_asked: HashSet<String>,
    /// The rows' right-click menu, the page's own off the Mac.
    menu: ContextMenu,
    window: AnyWindowHandle,
    /// The cgroups the last proc.watch named, none for every process; the link asks for them again on each dial.
    asked: Reads,
    /// A dial failed or the channel went, with no frame come since: the skeletons hold still while the link dials.
    waiting: bool,
    _link: Option<Held>,
    _receive: Task<()>,
    _filter: Subscription,
}

/// Two seconds, the sampler's cadence, until two frames say what it is.
const FIRST_INTERVAL_MS: f64 = 2000.;

/// `ESCALATE_MS`: how long TERM has before the too-old notice names what it reached alone.
const ESCALATE: std::time::Duration = std::time::Duration::from_millis(5000);

/// A proc.kill of the row's process, checked against its start and identity; `tree` takes everything under it too.
fn kill_frame(p: &Proc, signal: &str, tree: bool) -> Value {
    let mut frame = json!({ "op": "proc.kill", "pid": p.pid, "signal": signal });
    if tree {
        frame["mode"] = json!("tree");
    }
    if let Some(at) = p.started_at {
        frame["startedAt"] = json!(at.round() as i64);
    }
    if let Some(id) = &p.id {
        frame["identity"] = json!(id);
    }
    frame
}

/// A daemon too old for kill modes answers proc.kill with no `signalled`: TERM went to the pid alone.
fn reached_alone(reply: &Value) -> bool {
    reply.get("signalled").is_none()
}

/// The processes of `said` the frame still lists as themselves, which Force end offers KILL.
fn still_listed(said: &[Proc], snapshot: Option<&Snapshot>) -> Vec<Proc> {
    let Some(snapshot) = snapshot else { return Vec::new() };
    said.iter().filter(|q| snapshot.procs.iter().any(|r| r.pid == q.pid && (q.id.is_none() || r.id == q.id))).cloned().collect()
}

/// The slate's title off a `slates.get` answer.
fn slate_title(answer: &Value) -> Option<String> {
    answer.pointer("/slate/document/title").and_then(Value::as_str).map(str::to_owned)
}

fn reads(here: bool, view: View, every: bool) -> Option<Vec<String>> {
    let whole = here || (view == View::Processes && every);
    (!whole).then(|| vec![THREAD_CGROUPS.to_owned()])
}

impl Scoped {
    fn new(computer: Computer, acts: Acts, view: View, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(w::FILTER));
        let sub = cx.subscribe(&filter, |this: &mut Scoped, field, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.filter_text = field.read(cx).value().to_string();
                cx.notify();
            }
        });
        let host = acts.host(cx);
        let asked: Reads = Arc::new(Mutex::new(reads(computer.here(), view, false)));
        let (tx, mut rx) = mpsc::unbounded();
        let target = match &computer.workspace_id {
            Some(id) => Target::Workspace(id.clone()),
            None => Target::Place(HERE.into()),
        };
        let link = host.as_ref().map(|(h, rt)| live::follow_link(h.clone(), rt, target, computer.here(), asked.clone(), tx));
        let window_handle = window.window_handle();
        let receive = cx.spawn(async move |this, cx| {
            while let Some(arrival) = rx.next().await {
                if this.update(cx, |s, cx| s.arrive(arrival, cx)).is_err() {
                    break;
                }
            }
        });
        Scoped {
            computer,
            acts,
            host,
            channel: None,
            snapshot: None,
            resyncing: false,
            unavailable: None,
            ports: Vec::new(),
            mem: None,
            app: HashMap::new(),
            threads: Vec::new(),
            scope: None,
            generation: 0,
            shown: None,
            history: Vec::new(),
            interval: FIRST_INTERVAL_MS,
            sort: Sort::Mem,
            filter,
            filter_text: String::new(),
            picks: Picks::default(),
            every: false,
            open: HashSet::new(),
            ending: HashSet::new(),
            stopping: HashSet::new(),
            too_old: Vec::new(),
            too_old_wait: None,
            slates: HashMap::new(),
            slates_asked: HashSet::new(),
            menu: ContextMenu::default(),
            window: window_handle,
            asked,
            waiting: false,
            _link: link,
            _receive: receive,
            _filter: sub,
        }
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        match arrival {
            Arrival::Opened(channel) => {
                self.channel = Some(channel);
                self.waiting = false;
            }
            Arrival::Down => {
                // The last frame stays on screen while the link dials again, as the web keeps it.
                let moved = self.channel.is_some() || self.resyncing || !self.waiting;
                self.channel = None;
                self.resyncing = false;
                self.waiting = true;
                if !moved {
                    return;
                }
            }
            Arrival::Watch(refused) => {
                if refused.is_some() {
                    self.resyncing = false;
                }
                self.unavailable = refused;
            }
            Arrival::Ports(ports) => {
                self.ports = ports;
                self.recompute();
            }
            Arrival::App(parts) => {
                self.app = parts.into_iter().map(|p| (p.pid, p)).collect();
                self.recompute();
            }
            Arrival::Event(event) => match event.get("type").and_then(Value::as_str) {
                Some("proc.snapshot" | "proc.changes") => self.take_frame(&event, cx),
                Some("port.open" | "port.close") => {
                    apply_port_event(&mut self.ports, &event);
                    self.recompute();
                }
                Some("sys.sample") => {
                    let Some(mem) = memory_of(&event) else { return };
                    // The tile draws the used memory's words and tints by the total, so a sample that moves neither draws nothing.
                    let drawn = |(used, total): (f64, f64)| (fmt_bytes(used as u64), total);
                    let same = self.mem.is_some_and(|held| drawn(held) == drawn(mem));
                    self.mem = Some(mem);
                    if same {
                        return;
                    }
                }
                _ => return,
            },
        }
        cx.notify();
    }

    /// A whole snapshot replaces what is held; changes apply only on the frame they name, and a lost frame asks the
    /// daemon once for a whole list.
    fn take_frame(&mut self, event: &Value, cx: &mut Context<Self>) {
        let next = match Frame::read(event) {
            Some(Frame::Snapshot(s)) => {
                self.resyncing = false;
                Some(s)
            }
            Some(Frame::Changes(c)) => self.snapshot.as_ref().and_then(|s| apply_changes(s, c)),
            None => return,
        };
        let Some(next) = next else {
            if !self.resyncing {
                self.resyncing = true;
                let frame = live::watch_frame(self.asked.lock().unwrap().as_deref());
                self.daemon(frame, cx, |this, result, cx| {
                    if let Err(said) = result {
                        this.resyncing = false;
                        this.unavailable = Some(said);
                        cx.notify();
                    }
                });
            }
            return;
        };
        let at = next.at;
        if let Some(previous) = &self.snapshot {
            self.interval = (at - previous.at).max(1.);
        }
        let listed: HashSet<Pid> = next.procs.iter().map(|p| p.pid).collect();
        self.ending.retain(|pid| listed.contains(pid));
        let containers: HashSet<&str> = next.containers.iter().flatten().map(|c| c.id.as_str()).collect();
        self.stopping.retain(|id| containers.contains(id.as_str()));
        self.snapshot = Some(next);
        self.recompute();
        if let Some(scope) = &self.scope {
            let t = total_spent(&scope.groups);
            push_history(&mut self.history, HistoryPoint { at, mem: t.mem, cpu: t.cpu, energy: t.energy });
        }
    }

    fn recompute(&mut self) {
        let Some(snapshot) = &self.snapshot else { return };
        let host = if self.computer.here() {
            snapshot.procs.iter().find(|p| p.pid == snapshot.daemon).map(|p| p.ppid).filter(|ppid| *ppid > 1)
        } else {
            None
        };
        let ports = ports_by_pid(&self.ports);
        self.scope = Some(scope_of(ScopeInput { snapshot, threads: &self.threads, ports: &ports, host, app: &self.app, now: now_ms() }));
        self.generation += 1;
    }

    /// The groups the filter and the selects leave, sorted, made again only when the scope or a pick moved.
    fn refresh_shown(&mut self) {
        let Some(scope) = &self.scope else { return };
        let key = (self.generation, self.filter_text.clone(), self.picks.clone(), self.sort);
        if self.shown.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let groups = sort_groups(filter_groups(&scope.groups, &self.filter_text, &self.picks), self.sort);
        self.shown = Some((key, Rc::new(groups)));
    }

    /// Laid on from the page each frame; the scope is read again only when the threads moved.
    fn set_threads(&mut self, mut threads: Vec<Thread>) {
        for t in &mut threads {
            if let Some(title) = self.slates.get(&t.id) {
                t.slate = Some(title.clone());
            }
        }
        if threads != self.threads {
            self.threads = threads;
            self.recompute();
        }
    }

    /// `slates.get` once for each thread with a slate run whose title is not known yet, so its heading carries it.
    fn ask_slates(&mut self, cx: &mut Context<Self>) {
        let Some(scope) = &self.scope else { return };
        let slated: Vec<String> = scope
            .groups
            .iter()
            .filter(|g| g.procs.iter().any(|p| p.heading))
            .filter_map(|g| g.thread.as_ref().filter(|t| t.slate.is_none()).map(|t| t.id.clone()))
            .filter(|id| !self.slates_asked.contains(id))
            .collect();
        for id in slated {
            self.slates_asked.insert(id.clone());
            let this = cx.entity().downgrade();
            self.acts.call(req::<SlatesGetRequest>(json!({ "threadId": id })), cx, move |answer, cx| {
                let Some(title) = answer.ok().and_then(|a| serde_json::to_value(a).ok()).as_ref().and_then(slate_title) else { return };
                _ = this.update(cx, |s, cx| {
                    s.slates.insert(id.clone(), title.clone());
                    let threads = s.threads.clone();
                    s.threads.clear();
                    s.set_threads(threads);
                    cx.notify();
                });
            });
        }
    }

    /// What the daemon reads each tick: on a box the threads' cgroups, and the whole table once Every process asks.
    fn set_reads(&mut self, view: View, cx: &mut Context<Self>) {
        let want = reads(self.computer.here(), view, self.every);
        {
            let mut asked = self.asked.lock().unwrap();
            if *asked == want {
                return;
            }
            *asked = want.clone();
        }
        if self.channel.is_none() {
            return;
        }
        self.daemon(live::watch_frame(want.as_deref()), cx, |this, result, cx| {
            this.unavailable = result.err();
            cx.notify();
        });
    }

    /// One frame to this computer's daemon, its answer handed back here.
    fn daemon(
        &mut self,
        frame: Value,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<Value, String>, &mut Context<Self>) + 'static,
    ) {
        let (Some((host, runtime)), Some(channel)) = (self.host.clone(), self.channel.clone()) else {
            then(self, Err("The daemon is not answering.".into()), cx);
            return;
        };
        let (tx, rx) = oneshot::channel();
        runtime.spawn(async move {
            _ = tx.send(live::send(&host, &channel, frame).await);
        });
        cx.spawn(async move |this, cx| {
            let Ok(answer) = rx.await else { return };
            _ = this.update(cx, |s, cx| then(s, answer, cx));
        })
        .detach();
    }

    pub fn toggle(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.open.remove(id) {
            self.open.insert(id.to_owned());
        }
        cx.notify();
    }

    pub fn set_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        self.sort = sort;
        cx.notify();
    }

    /// The process and everything under it, TERM then KILL in the daemon, checked against the row's identity. A daemon
    /// too old for that sent TERM to the pid alone: once TERM has had as long as the pane waits, one notice names every
    /// such process with the computer's update line, and offers KILL to those still listed.
    pub fn end(&mut self, p: &Proc, cx: &mut Context<Self>) {
        let pid = p.pid;
        self.ending.insert(pid);
        cx.notify();
        let q = p.clone();
        self.daemon(kill_frame(p, "TERM", true), cx, move |this, result, cx| match result {
            Err(said) => {
                this.ending.remove(&pid);
                this.acts.failed(said, cx);
                cx.notify();
            }
            Ok(reply) if reached_alone(&reply) => this.wait_too_old(q, cx),
            Ok(_) => {}
        });
    }

    fn wait_too_old(&mut self, p: Proc, cx: &mut Context<Self>) {
        self.too_old.push(p);
        if self.too_old_wait.is_none() {
            self.too_old_wait = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(ESCALATE).await;
                _ = this.update(cx, |s, cx| s.say_too_old(cx));
            }));
        }
    }

    fn say_too_old(&mut self, cx: &mut Context<Self>) {
        self.too_old_wait = None;
        let said = std::mem::take(&mut self.too_old);
        for q in &said {
            self.ending.remove(&q.pid);
        }
        let still = still_listed(&said, self.snapshot.as_ref());
        let name = self.computer.name();
        let named: Vec<(Pid, &str)> = said.iter().map(|q| (q.pid, q.command.as_str())).collect();
        let mut note = Notice::error(w::kill_too_old_of(&name, &named)).place(name);
        if !still.is_empty() {
            let this = cx.entity().downgrade();
            note = note.action(NoticeAction::new(w::FORCE_END, move |_, cx| _ = this.update(cx, |s, cx| s.force_end(&still, cx))));
        }
        let window = self.window;
        cx.defer(move |cx| _ = window.update(cx, |_, window, cx| crate::ui::notify_in(window, cx, note)));
        cx.notify();
    }

    /// Force end: KILL to each process the too-old notice named that is still listed as itself.
    fn force_end(&mut self, still: &[Proc], cx: &mut Context<Self>) {
        for q in still {
            self.daemon(kill_frame(q, "KILL", false), cx, |this, result, cx| {
                if let Err(said) = result {
                    this.acts.failed(said, cx);
                }
            });
        }
    }

    /// Frees a thread's kept agent through the host, as `wsp activity end` does.
    pub fn free(&mut self, p: &Proc, cx: &mut Context<Self>) {
        let Some(started) = p.started_at else { return };
        let pid = p.pid;
        self.ending.insert(pid);
        cx.notify();
        let this = cx.entity().downgrade();
        self.acts.call(req::<ActivityEndRequest>(json!({ "pid": pid, "startedAt": started })), cx, move |answer, cx| {
            if let Err(said) = answer {
                _ = this.update(cx, |s, cx| {
                    s.ending.remove(&pid);
                    s.acts.failed(said, cx);
                    cx.notify();
                });
            }
        });
    }

    /// Stops one container through its engine, keeping its data.
    pub fn stop_container(&mut self, c: &model::ContainerRow, cx: &mut Context<Self>) {
        let id = c.id.clone();
        self.stopping.insert(id.clone());
        cx.notify();
        self.daemon(json!({ "op": "container.stop", "container": id }), cx, move |this, result, cx| {
            if let Err(said) = result {
                this.stopping.remove(&id);
                this.acts.failed(said, cx);
                cx.notify();
            }
        });
    }

    /// Stop asks before a running turn ends.
    pub fn ask_stop(&mut self, group: &Group, cx: &mut Context<Self>) {
        let Some(t) = group.thread.clone() else { return };
        let acts = self.acts.clone();
        let title = w::stop_title(&t.title);
        let line = w::stop_sentence(&catalog::name(&t.harness), t.cgroup.is_some());
        let session = t.session_id.clone();
        self.acts.open_dialog(
            DialogBuild::alert(move |_, _, _| {
                let (cancel, stop) = (acts.clone(), acts.clone());
                let session = session.clone();
                AlertDialog::new(title.clone())
                    .layered()
                    .sentence(line.clone())
                    .act(Button::outline("activity-cancel", w::CANCEL).neutral_ring().on_click(move |_, _, cx| cancel.close_dialog(cx)))
                    .act(Button::destructive("activity-confirm", w::STOP_TURN).on_click(move |_, _, cx| {
                        stop.close_dialog(cx);
                        let said = stop.clone();
                        stop.call(req::<SessionsInterruptRequest>(json!({ "sessionId": session })), cx, move |answer, cx| match answer {
                            Ok(result) => {
                                if let Some(left) = result.left {
                                    said.failed(left, cx);
                                }
                            }
                            Err(e) => said.failed(e, cx),
                        });
                    }))
                    .into_any_element()
            }),
            cx,
        );
    }

    /// End all asks, naming how many processes it ends and what they hold.
    pub fn ask_end_all(&mut self, heading: &Proc, _group: &Group, cx: &mut Context<Self>) {
        let ends: Vec<Proc> = tables::ends_of(heading).into_iter().cloned().collect();
        let title = w::end_all_title(&heading.name);
        let line = w::end_all_sentence(ends.len(), &fmt_bytes(tables::held_by(heading) as u64));
        let acts = self.acts.clone();
        let this = cx.entity().downgrade();
        self.acts.open_dialog(
            DialogBuild::alert(move |_, _, _| {
                let (cancel, close) = (acts.clone(), acts.clone());
                let (this, ends) = (this.clone(), ends.clone());
                AlertDialog::new(title.clone())
                    .layered()
                    .sentence(line.clone())
                    .act(Button::outline("activity-cancel", w::CANCEL).neutral_ring().on_click(move |_, _, cx| cancel.close_dialog(cx)))
                    .act(Button::destructive("activity-confirm", w::END_ALL_PRESS).on_click(move |_, _, cx| {
                        close.close_dialog(cx);
                        _ = this.update(cx, |s, cx| {
                            for p in &ends {
                                s.end(p, cx);
                            }
                        });
                    }))
                    .into_any_element()
            }),
            cx,
        );
    }
}

/// The page's one card body: the totals, the bar of pickers and the filter, then the open view.
#[derive(IntoElement)]
struct Page {
    pick: Entity<Pick>,
    places: Vec<(PlaceView, ComputerIcon)>,
    models: Vec<(String, String, String)>,
    acts: Acts,
}

impl RenderOnce for Page {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Lists and Scoped take the host once, when they are made, so neither is made before it answers.
        if self.acts.host(cx).is_none() {
            return div().into_any_element();
        }
        let acts = self.acts.clone();
        let lists = window.use_keyed_state("activity-lists", cx, move |_, cx| Lists::new(&acts, cx));
        let empty = Vec::new();
        let workspaces = lists.read(cx).listing.as_ref().map_or(&empty, |l| &l.workspaces);
        let computers = computers(&self.places, workspaces);
        let (view, picked) = {
            let p = self.pick.read(cx);
            (p.view, p.place.clone())
        };
        let Some(computer) = computers.iter().find(|c| c.place.id == picked.as_deref().unwrap_or(HERE)).or(computers.first()).cloned()
        else {
            return div().into_any_element();
        };
        let now = now_ms();
        let (threads, latests) = lists.update(cx, |l, _| l.threads(&self.places, &computer));
        let looks: HashMap<String, Status> = latests.iter().map(|(id, latest)| (id.clone(), status_of(latest, now))).collect();
        let (acts, start) = (self.acts.clone(), computer.clone());
        let scoped = window.use_keyed_state(SharedString::from(format!("activity-scoped-{}", computer.key)), cx, move |window, cx| {
            Scoped::new(start, acts, view, window, cx)
        });
        scoped.update(cx, |s, cx| {
            s.set_threads(threads);
            s.ask_slates(cx);
            s.set_reads(view, cx);
            let gone = s.picks.thread.as_ref().is_some_and(|t| s.scope.as_ref().is_some_and(|sc| !sc.groups.iter().any(|g| g.id == *t)));
            if gone {
                s.picks.thread = None;
            }
            s.refresh_shown();
        });
        let selects = selects(&computers, &computer, &scoped, window, cx);
        body(&self, selects, &computer, view, &scoped, &looks, narrow::width(window), cx).into_any_element()
    }
}

/// The computer's state when it is a box that is not answering (`useAbsentComputer`): none for this computer, whose
/// Activity reads no workspace of its own.
fn absent(computer: &Computer) -> Option<(String, String)> {
    if computer.here() {
        return None;
    }
    let absent = absent_of(&serde_json::to_value(&computer.place).ok()?)?;
    Some((absent.said, absent.will.unwrap_or_default()))
}

fn note(word: impl Into<SharedString>, cx: &App) -> gpui_kit::Div {
    div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(cx.palette().muted_foreground).child(word.into())
}

#[allow(clippy::too_many_arguments)]
fn body(
    page: &Page,
    selects: Selects,
    computer: &Computer,
    view: View,
    scoped: &Entity<Scoped>,
    looks: &HashMap<String, Status>,
    width: Width,
    cx: &mut App,
) -> impl IntoElement {
    let s = scoped.read(cx);
    let away = absent(computer);
    let scope = s.scope.as_ref();
    let energy = scope.is_some_and(|sc| sc.energy);
    let filtering = !s.filter_text.trim().is_empty() || s.picks.thread.is_some() || s.picks.kind.is_some();
    let hand = Hand {
        scoped: scoped.downgrade(),
        ending: Rc::new(s.ending.clone()),
        stopping: Rc::new(s.stopping.clone()),
        open: Rc::new(s.open.clone()),
        sort: s.sort,
        energy,
        filtering,
        width,
        workspace: computer.workspace_id.clone(),
    };
    let unavailable = s.unavailable.clone().filter(|_| s.snapshot.is_none() && away.is_none());
    let moving = skeletons_move(s.waiting, unavailable.is_some());
    let tiles = match (&away, scope) {
        (Some(_), _) => None,
        (None, None) => Some(tiles_skeleton(moving, width.sm, cx)),
        (None, Some(sc)) => Some(tiles(sc, &s.history, s.mem, width.sm, cx)),
    };
    let bar = controls(page, selects, view, scoped, s, width.sm, cx);
    let name = computer.name();
    let main: AnyElement = if let Some((said, will)) = away {
        v_flex().gap_1().py_2().child(note(said, cx)).child(note(will, cx)).into_any_element()
    } else if let Some(said) = unavailable {
        note(said, cx).into_any_element()
    } else if let (Some(sc), Some((_, groups))) = (scope, &s.shown) {
        let groups = groups.clone();
        match view {
            View::Threads => {
                let cost = s.snapshot.as_ref().and_then(|snap| snap.cost).map(|c| (c.cpu_ms / s.interval * 100., c.held));
                let models = &page.models;
                let status = |id: &str| looks.get(id).cloned();
                let model = |harness: &str, m: &str| {
                    let bare = m.strip_suffix("[1m]").unwrap_or(m);
                    Some(
                        models
                            .iter()
                            .find(|(h, v, _)| h == harness && v == bare)
                            .map_or_else(|| m.to_owned(), |(_, _, label)| label.clone()),
                    )
                };
                tables::threads_table(name, &groups, &ThreadLooks { status: &status, model: &model }, !computer.here(), cost, &hand, cx)
            }
            View::Processes => tables::processes_table(name, &flat_rows(&groups, &sc.rest, s.every, s.sort, &s.filter_text), &hand, cx),
            View::Kinds => tables::kinds_table(name, &kind_groups(&groups, s.sort), &hand, cx),
            View::Ports => tables::ports_table(&port_rows(&groups), &hand, cx),
        }
    } else {
        rows_skeleton(moving, cx).into_any_element()
    };
    let menu = s.menu.overlay().map(|open| div().absolute().debug_selector(|| "activity-row-menu".into()).child(open));
    v_flex().id("activity").debug_selector(|| "activity".into()).w_full().gap(px(40.)).children(tiles).child(bar).child(main).children(menu)
}

/// The figure's mark: warn and danger in their inks.
fn inked(tint: Option<Tint>, cx: &App) -> gpui_kit::Hsla {
    tint_ink(tint, cx)
}

/// One total: its label, the figure inked past its mark, a quiet note, and where it has a history, its line.
#[allow(clippy::too_many_arguments)]
fn tile(
    id: &'static str,
    label: &'static str,
    value: String,
    note_word: String,
    tint: Option<Tint>,
    line: Option<(Vec<(f32, f32)>, gpui_kit::Hsla)>,
    ruled: bool,
    cx: &App,
) -> AnyElement {
    let p = cx.palette();
    v_flex()
        .id(id)
        .debug_selector(move || id.into())
        .flex_1()
        .min_w_0()
        .gap(px(6.))
        .px_5()
        .py_5()
        .when(ruled, |el| el.border_l_1().border_color(p.border.opacity(0.5)))
        .child(div().text_size(text::FACT.0).line_height(px(19.5)).text_color(p.muted_foreground).child(label))
        .child(
            div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(26.))
                .line_height(px(32.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(inked(tint, cx))
                .child(value),
        )
        .child(div().truncate().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(note_word))
        .children(line.map(|(points, ink)| div().mt_2().child(Sparkline::new(points, ink).height(px(40.)))))
        .into_any_element()
}

/// The tile lines' inks, CPU sky, memory violet and power teal (`LINE_INKS`, `activity/page.tsx`), which these hues hold.
fn line_inks(cx: &App) -> (gpui_kit::Hsla, gpui_kit::Hsla, gpui_kit::Hsla) {
    let p = cx.palette();
    (p.hue_blue, p.hue_violet, p.hue_teal)
}

/// The last minutes as a line, scaled to the highest point in the window and never below the floor.
fn line_points(history: &[HistoryPoint], y: impl Fn(&HistoryPoint) -> f64, floor: f64) -> Vec<(f32, f32)> {
    let to = history.last().map_or(0., |h| h.at);
    let from = to - (HISTORY_POINTS as f64 - 1.) * FIRST_INTERVAL_MS;
    let top = history.iter().map(&y).fold(floor, f64::max) * 1.15;
    history.iter().map(|h| (((h.at - from) / (to - from).max(1.)).clamp(0., 1.) as f32, (y(h) / top).clamp(0., 1.) as f32)).collect()
}

/// The tiles on their grid: one row with a rule between each, or two columns with none under 640 px
/// (`max-sm:grid-cols-2 max-sm:divide-x-0`), a short last row keeping its columns' widths.
fn tile_grid(id: &'static str, tiles: Vec<AnyElement>, narrow: bool) -> gpui_kit::Div {
    let per = if narrow { 2 } else { tiles.len().max(1) };
    let mut tiles = tiles.into_iter();
    let mut grid = v_flex().debug_selector(move || id.into());
    loop {
        let mut line: Vec<AnyElement> = tiles.by_ref().take(per).collect();
        if line.is_empty() {
            break;
        }
        line.extend((line.len()..per).map(|_| div().flex_1().min_w_0().into_any_element()));
        grid = grid.child(h_flex().items_stretch().children(line));
    }
    grid
}

/// wsp's totals on this computer, each figure over a line of the last minutes; the process count stands alone.
fn tiles(scope: &Scope, history: &[HistoryPoint], mem: Option<(f64, f64)>, narrow: bool, cx: &App) -> AnyElement {
    let t = total_spent(&scope.groups);
    let threads = scope.groups.iter().filter(|g| g.kind == model::GroupKind::Thread).count();
    let mem_note = mem.map_or_else(|| w::PROCESSES.to_owned(), |(used, _)| w::of(&fmt_bytes(used as u64)));
    let (cpu_ink, mem_ink, energy_ink) = line_inks(cx);
    let mut row = vec![
        tile(
            "activity-mem",
            w::WSP,
            fmt_bytes(t.mem as u64),
            mem_note,
            memory_share_tint(t.mem, mem.map(|m| m.1)),
            Some((line_points(history, |h| h.mem, 1024f64.powi(3)), mem_ink)),
            false,
            cx,
        ),
        tile(
            "activity-cpu",
            w::CPU,
            format!("{}%", t.cpu.round()),
            w::ONE_CORE.into(),
            cpu_tint(t.cpu),
            Some((line_points(history, |h| h.cpu, 100.), cpu_ink)),
            !narrow,
            cx,
        ),
    ];
    if scope.energy {
        row.push(tile(
            "activity-energy",
            w::ENERGY,
            energy_word(t.energy),
            w::ENERGY_NOTE.into(),
            energy_tint(t.energy),
            Some((line_points(history, |h| h.energy.unwrap_or(0.), 1000.), energy_ink)),
            !narrow,
            cx,
        ));
    }
    row.push(tile("activity-procs", w::PROCESSES, t.count.to_string(), w::threads(threads), None, None, !narrow, cx));
    Card::new("activity-totals").locked(true).item(CardItem::any(tile_grid("activity-totals", row, narrow))).into_any_element()
}

/// A bar in the skeleton's tier at the width of whatever holds it.
fn bar(height: f32, cx: &App) -> impl IntoElement {
    div().w_full().h(px(height)).rounded(px(6.)).bg(cx.palette().border)
}

/// Whether the skeletons sweep: not while a dial waits to come again, nor once the watch was refused, since nothing
/// will arrive to end the sweep and each frame of it draws the whole window again.
fn skeletons_move(waiting: bool, refused: bool) -> bool {
    !waiting && !refused
}

/// A skeleton bar, held still once a dial has failed: each frame of its sweep draws the whole window again.
fn skeleton(width: f32, height: f32, moving: bool, border: gpui_kit::Hsla) -> AnyElement {
    if moving {
        return Skeleton::new(px(width), px(height)).into_any_element();
    }
    div().flex_none().w(px(width)).h(px(height)).rounded(px(6.)).bg(border).into_any_element()
}

fn tiles_skeleton(moving: bool, narrow: bool, cx: &App) -> AnyElement {
    let p = cx.palette();
    let cell = |ix: usize| {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(6.))
            .px_5()
            .py_5()
            .when(ix > 0 && !narrow, |el| el.border_l_1().border_color(p.border.opacity(0.5)))
            .child(div().my_1().child(skeleton(64., 12., moving, p.border)))
            .child(div().my(px(2.)).child(skeleton(112., 28., moving, p.border)))
            .child(div().my(px(2.)).child(skeleton(96., 12., moving, p.border)))
            .child(div().mt_2().child(bar(40., cx)))
            .into_any_element()
    };
    let grid = tile_grid("activity-totals-loading", (0..4).map(cell).collect(), narrow);
    Card::new("activity-totals-loading").locked(true).item(CardItem::any(grid)).into_any_element()
}

fn rows_skeleton(moving: bool, cx: &App) -> impl IntoElement {
    let border = cx.palette().border;
    let row = |ix: usize| {
        CardItem::placed(move |corners| {
            corners
                .apply(h_flex())
                .id(("activity-loading", ix))
                .min_h(px(60.))
                .gap_3()
                .px(corners.inset)
                .py_3()
                .child(skeleton(32., 32., moving, border))
                .child(skeleton(192., 16., moving, border))
                .into_any_element()
        })
    };
    v_flex()
        .debug_selector(|| "activity-loading".into())
        .child(note(w::PENDING, cx).mb_4())
        .child(Card::new("activity-loading").locked(true).items((0..3).map(row)))
}

/// A select's state kept in step with what the page holds: its items and its value laid on again only when they move.
struct Synced<T: Clone + PartialEq + 'static> {
    state: Entity<SelectState<T>>,
    sig: Vec<(T, SharedString)>,
    value: T,
    _watch: Subscription,
}

type Items<T> = Vec<(T, SharedString, Option<ComputerIcon>)>;

fn select_items<T: Clone + PartialEq + 'static>(items: &Items<T>) -> Vec<SelectItem<T>> {
    items
        .iter()
        .map(|(v, label, glyph)| {
            let item = SelectItem::new(v.clone(), label.clone());
            match *glyph {
                Some(icon) => item.mark(move |cx| icon.draw(cx.palette().foreground.opacity(0.8), px(14.))),
                None => item,
            }
        })
        .collect()
}

fn synced<T: Clone + PartialEq + 'static>(
    key: String,
    items: Items<T>,
    value: T,
    window: &mut Window,
    cx: &mut App,
) -> Entity<SelectState<T>> {
    let sig: Vec<(T, SharedString)> = items.iter().map(|(v, l, _)| (v.clone(), l.clone())).collect();
    let (first, v0, sig0) = (select_items(&items), value.clone(), sig.clone());
    let held = window.use_keyed_state(SharedString::from(key), cx, move |_, cx| {
        let state = cx.new(|_| SelectState::new(first, Some(&v0)));
        let watch = cx.observe(&state, |_, _, cx| cx.notify());
        Synced { state, sig: sig0, value: v0, _watch: watch }
    });
    let (state, stale_items, stale_value) = {
        let h = held.read(cx);
        (h.state.clone(), h.sig != sig, h.value != value)
    };
    if stale_items || stale_value {
        held.update(cx, |h, _| {
            h.sig = sig;
            h.value = value.clone();
        });
        state.update(cx, |st, cx| {
            if stale_items {
                st.set_items(select_items(&items), cx);
            }
            st.set_value(Some(&value), cx);
        });
    }
    state
}

/// The bar's three selects: the computer, the kind and the thread.
struct Selects {
    computer: Entity<SelectState<String>>,
    kind: Entity<SelectState<Option<Kind>>>,
    thread: Entity<SelectState<Option<String>>>,
}

fn selects(computers: &[Computer], computer: &Computer, scoped: &Entity<Scoped>, window: &mut Window, cx: &mut App) -> Selects {
    let (kinds, threads, picks) = {
        let s = scoped.read(cx);
        let (kinds, threads) = match (&s.scope, absent(computer)) {
            (Some(sc), None) => pick_choices(&sc.groups, &s.picks),
            _ => (Vec::new(), Vec::new()),
        };
        (kinds, threads, s.picks.clone())
    };
    let places: Items<String> = computers.iter().map(|c| (c.place.id.clone(), SharedString::from(c.name()), Some(c.glyph))).collect();
    let mut kind_items: Items<Option<Kind>> = vec![(None, w::ALL_KINDS.into(), None)];
    kind_items.extend(kinds.into_iter().map(|k| (Some(k), SharedString::from(k.word()), None)));
    let mut thread_items: Items<Option<String>> = vec![(None, w::ALL_THREADS.into(), None)];
    thread_items.extend(threads.into_iter().map(|(id, title)| (Some(id), SharedString::from(title), None)));
    Selects {
        computer: synced("activity-computer".into(), places, computer.place.id.clone(), window, cx),
        kind: synced(format!("activity-kind-{}", computer.key), kind_items, picks.kind, window, cx),
        thread: synced(format!("activity-thread-{}", computer.key), thread_items, picks.thread, window, cx),
    }
}

/// `BAR_SELECT`'s `h-8`, the filter's height too.
const BAR_HEIGHT: gpui_kit::Pixels = px(32.);

/// The page's own pickers and its filter, at the height of the Agents page's computer picker. Under 640 px the pickers
/// stand on a grid of two, the computer's across both, and the filter takes the line under them.
fn controls(page: &Page, selects: Selects, view: View, scoped: &Entity<Scoped>, s: &Scoped, narrow: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let pick = page.pick.clone();
    let (kinds, threads) = (scoped.downgrade(), scoped.downgrade());
    let computer = Select::new(&selects.computer).width(px(224.)).height(BAR_HEIGHT).fill(narrow).on_change(move |id, _, cx| {
        let id = id.clone();
        pick.update(cx, |p, cx| {
            p.place = Some(id);
            cx.notify();
        })
    });
    let kind = Select::new(&selects.kind).width(px(160.)).height(BAR_HEIGHT).fill(narrow).on_change(move |kind, _, cx| {
        let kind = *kind;
        _ = kinds.update(cx, |s, cx| {
            s.picks.kind = kind;
            cx.notify();
        });
    });
    let thread = Select::new(&selects.thread).width(px(192.)).height(BAR_HEIGHT).fill(narrow).on_change(move |thread, _, cx| {
        let thread = thread.clone();
        _ = threads.update(cx, |s, cx| {
            s.picks.thread = thread;
            cx.notify();
        });
    });
    let (computer, kind, thread) = (
        div().debug_selector(|| "activity-computer".into()).child(computer),
        div().debug_selector(|| "activity-kind".into()).child(kind),
        div().debug_selector(|| "activity-thread".into()).child(thread),
    );
    let left = if narrow {
        v_flex().w_full().gap_2().child(computer).child(h_flex().gap_2().child(kind.flex_1().min_w_0()).child(thread.flex_1().min_w_0()))
    } else {
        h_flex().min_w_0().flex_wrap().gap_2().child(computer).child(kind).child(thread)
    };
    let every = (view == View::Processes).then(|| {
        let scoped = scoped.downgrade();
        h_flex()
            .flex_none()
            .gap_2()
            .text_size(text::NOTE.0)
            .line_height(text::NOTE.1)
            .text_color(p.muted_foreground)
            .child(Switch::new("activity-every", s.every).on_change(move |on, _, cx| {
                let on = *on;
                _ = scoped.update(cx, |s, cx| {
                    s.every = on;
                    s.set_reads(View::Processes, cx);
                    cx.notify();
                });
            }))
            .child(w::EVERY)
    });
    let filter = Input::new(&s.filter)
        .prefix(Icon::new(Glyph::Search).with_size(px(14.)).text_color(p.muted_foreground))
        .h(BAR_HEIGHT)
        .when(!narrow, |el| el.w(px(256.)))
        .when(narrow, |el| el.flex_1().min_w_0())
        .rounded(crate::ui::size::CONTROL_RADIUS)
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::FACT.0);
    h_flex().debug_selector(|| "activity-controls".into()).flex_wrap().justify_between().gap_x_4().gap_y_2().child(left).child(
        h_flex()
            .when(narrow, |el| el.w_full())
            .gap_4()
            .children(every)
            .child(div().debug_selector(|| "activity-filter".into()).when(narrow, |el| el.flex_1().min_w_0()).child(filter)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::settings::SettingsScreen;
    use gpui_kit::{size, Bounds, Pixels, TestAppContext, VisualTestContext, WeakEntity};

    const WIDTHS: [f32; 3] = [600., 760., 1280.];

    fn proc(pid: Pid, kind: Kind) -> Proc {
        Proc {
            pid,
            name: format!("p{pid}"),
            kind,
            mem: 2e8,
            cpu: 3.,
            energy: None,
            ports: vec![],
            command: "node dev".into(),
            started_at: None,
            id: None,
            harness: None,
            needed: false,
            endable: true,
            heading: false,
            note: None,
            children: vec![],
        }
    }

    fn thread_group() -> Group {
        let thread = Thread {
            id: "t1".into(),
            title: "Fix the login".into(),
            harness: "claude".into(),
            project: "bench".into(),
            workspace_id: "w1".into(),
            session_id: "s1".into(),
            working: false,
            pid: None,
            agent_pid: None,
            cgroup: None,
            parent: None,
            slate: None,
            subagents: vec![],
        };
        Group {
            id: "t1".into(),
            kind: model::GroupKind::Thread,
            thread: Some(thread),
            procs: vec![proc(11, Kind::Dev)],
            subagents: vec![],
            containers: vec![],
            memory: None,
        }
    }

    /// The page's tiles, bar and Threads list as its body lays them, at the window's width, off one thread with no host.
    struct Drawn {
        page: Page,
        computers: Vec<Computer>,
        scoped: Entity<Scoped>,
    }

    impl Render for Drawn {
        fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let width = narrow::width(window);
            let computer = self.computers[0].clone();
            let selects = selects(&self.computers, &computer, &self.scoped, window, cx);
            let s = self.scoped.read(cx);
            let scope = Scope { groups: vec![thread_group()], rest: vec![], energy: false };
            let hand = Hand {
                scoped: self.scoped.downgrade(),
                ending: Rc::default(),
                stopping: Rc::default(),
                open: Rc::new(["t1".to_owned()].into_iter().collect()),
                sort: Sort::Mem,
                energy: false,
                filtering: false,
                width,
                workspace: None,
            };
            let looks = ThreadLooks { status: &|_| None, model: &|_, _| None };
            v_flex()
                .size_full()
                .p_4()
                .gap(px(40.))
                .child(tiles(&scope, &[], None, width.sm, cx))
                .child(controls(&self.page, selects, View::Threads, &self.scoped, s, width.sm, cx))
                .child(tables::threads_table("This computer".into(), &scope.groups, &looks, false, None, &hand, cx))
        }
    }

    fn draw(cx: &mut TestAppContext) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let place: PlaceView = serde_json::from_value(json!({ "id": HERE, "kind": "computer", "name": "here", "default": true })).unwrap();
        let computer = Computer { glyph: ComputerIcon::of(&place, None), place: place.clone(), key: HERE.into(), workspace_id: None };
        let acts = Acts::new(WeakEntity::<SettingsScreen>::new_invalid(), None, runtime);
        let (_, cx) = cx.add_window_view(|window, cx| {
            let pick = cx.new(|_| Pick::default());
            let page = Page { pick, places: vec![(place, computer.glyph)], models: vec![], acts: acts.clone() };
            let scoped = cx.new(|cx| Scoped::new(computer.clone(), acts, View::Threads, window, cx));
            let drawn = cx.new(|_| Drawn { page, computers: vec![computer], scoped });
            gpui_kit::component::Root::new(drawn, window, cx)
        });
        cx
    }

    fn at(cx: &mut VisualTestContext, width: f32) {
        cx.simulate_resize(size(px(width), px(1200.)));
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }

    fn bounds(cx: &mut VisualTestContext, selector: &'static str, width: f32) -> Bounds<Pixels> {
        cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn at {width} px"))
    }

    #[track_caller]
    fn near(what: &str, got: Pixels, want: f32) {
        let got = f32::from(got);
        assert!((got - want).abs() < 0.5, "{what}: {got} px, the web writes {want} px");
    }

    #[gpui_kit::test]
    fn the_tiles_stand_two_by_two_under_640(cx: &mut TestAppContext) {
        let cx = draw(cx);
        for width in WIDTHS {
            at(cx, width);
            let (mem, cpu, procs) =
                (bounds(cx, "activity-mem", width), bounds(cx, "activity-cpu", width), bounds(cx, "activity-procs", width));
            near("the second tile beside the first", cpu.top(), f32::from(mem.top()));
            if width < 640. {
                let grid = bounds(cx, "activity-totals", width);
                near("two columns, each half the strip", mem.size.width, f32::from(grid.size.width) / 2.);
                near("two columns, the second at the first's edge", cpu.left(), f32::from(mem.right()));
                assert!(procs.top() >= mem.bottom(), "the third tile starts the second line at {width} px: {procs:?}");
                near("the second line's first column under the first", procs.left(), f32::from(mem.left()));
            } else {
                near("one line of three", procs.top(), f32::from(mem.top()));
            }
        }
    }

    #[gpui_kit::test]
    fn the_bar_becomes_a_grid_of_two_under_640(cx: &mut TestAppContext) {
        let cx = draw(cx);
        for width in WIDTHS {
            at(cx, width);
            let (computer, kind, thread, filter) = (
                bounds(cx, "activity-computer", width),
                bounds(cx, "activity-kind", width),
                bounds(cx, "activity-thread", width),
                bounds(cx, "activity-filter", width),
            );
            near(&format!("BAR_SELECT's h-8 at {width} px"), computer.size.height, 32.);
            if width < 640. {
                near("the computer across both columns", computer.size.width, f32::from(kind.size.width + thread.size.width) + 8.);
                near("kind and thread side by side", kind.top(), f32::from(thread.top()));
                near("gap-2 down the grid", kind.top() - computer.bottom(), 8.);
                near("the filter on the line under, gap-y-2", filter.top() - kind.bottom(), 8.);
                near("the filter across the bar", filter.size.width, f32::from(computer.size.width));
            } else {
                near("the computer's w-56", computer.size.width, 224.);
                near("the kind's w-40", kind.size.width, 160.);
                near("the thread's w-48", thread.size.width, 192.);
                near("the filter's w-64", filter.size.width, 256.);
            }
        }
    }

    #[gpui_kit::test]
    fn the_lists_drop_cpu_and_energy_and_a_thread_head_carries_its_menu_under_768(cx: &mut TestAppContext) {
        let cx = draw(cx);
        for width in WIDTHS {
            at(cx, width);
            let narrow = width < 768.;
            assert_eq!(cx.debug_bounds("activity-sort-CPU").is_none(), narrow, "CPU's head at {width} px");
            assert_eq!(cx.debug_bounds("activity-head-menu").is_some(), narrow, "the thread head's menu button at {width} px");
            let mem = bounds(cx, "activity-sort-Memory", width);
            let row = bounds(cx, "activity-sub-row", width);
            // `narrow:grid-cols-[minmax(0,1fr)_72px_20px]` against the full `76px 60px 64px 20px`.
            let past = if narrow { 16. + 20. + 20. } else { 16. + 60. + 16. + 64. + 16. + 20. + 20. };
            near(&format!("memory's column against the row's end at {width} px"), row.right() - mem.right(), past);
            near(&format!("memory's column at {width} px"), mem.size.width, if narrow { 72. } else { 76. });
        }
    }

    #[test]
    fn activity_reads_this_computer_and_each_box_a_folder_stands_on() {
        let row = |v: Value| -> (PlaceView, ComputerIcon) {
            let place: PlaceView = serde_json::from_value(v).unwrap();
            let icon = ComputerIcon::of(&place, None);
            (place, icon)
        };
        let places = vec![
            row(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })),
            row(json!({ "id": "pi", "kind": "computer", "name": "pi", "default": false, "present": false })),
            row(json!({ "id": "nas", "kind": "computer", "name": "nas", "default": false })),
            row(json!({ "id": "box", "kind": "provider", "name": "box", "default": false })),
        ];
        let workspaces =
            [json!({ "id": "wpi", "kind": "place", "place": "pi" }), json!({ "id": "wcloud", "kind": "cloud", "provider": "box" })];
        let read = computers(&places, &workspaces);
        let keys: Vec<(&str, &str)> = read.iter().map(|c| (c.place.id.as_str(), c.key.as_str())).collect();
        assert_eq!(keys, [("here", HERE), ("pi", "wpi")], "no box without a folder there, and no cloud");
        assert_eq!(absent(&read[0]), None, "this computer is never absent here");
        assert_eq!(absent(&read[1]), Some(("pi is not answering".to_owned(), "it connects on its own when it is on".to_owned())));
    }

    #[test]
    fn a_thread_titles_as_the_sidebar_does() {
        let turn = |prompt: &str| serde_json::json!({ "id": "s1", "prompt": prompt });
        assert_eq!(title_of(&turn("Fix the login bug. Then add tests."), &turn("")), "Fix the login bug.");
        // 30 faces are 60 UTF-16 units, which JavaScript's length counts: past 48, so cut.
        let faces = "\u{1f600}".repeat(30);
        assert_eq!(title_of(&turn(&faces), &turn("")), format!("{}\u{2026}", "\u{1f600}".repeat(23)));
        assert_eq!(title_of(&serde_json::json!({ "id": "s1" }), &serde_json::json!({ "harnessTitle": "Named\nmore" })), "Named");
    }

    #[test]
    fn a_refused_watch_holds_the_skeletons_still() {
        assert!(skeletons_move(false, false));
        assert!(!skeletons_move(true, false));
        assert!(!skeletons_move(false, true));
    }

    fn ticked(pid: Pid, command: &str, id: Option<&str>) -> Proc {
        Proc {
            pid,
            name: "node".into(),
            kind: Kind::Dev,
            mem: 0.,
            cpu: 0.,
            energy: None,
            ports: vec![],
            command: command.into(),
            started_at: Some(1_700_000_000_000.4),
            id: id.map(str::to_owned),
            harness: None,
            needed: false,
            endable: true,
            heading: false,
            note: None,
            children: vec![],
        }
    }

    fn entry(pid: Pid, id: Option<&str>) -> wire::ProcEntry {
        wire::ProcEntry {
            pid,
            ppid: 1,
            state: "S".into(),
            comm: "node".into(),
            cmdline: String::new(),
            cpu: 0.,
            rss: 0.,
            started_at: 0.,
            pty: None,
            cgroup: None,
            energy_mw: None,
            pgid: None,
            owner: None,
            kind: None,
            id: id.map(str::to_owned),
        }
    }

    fn snapshot(procs: Vec<wire::ProcEntry>) -> Snapshot {
        Snapshot { at: 0., daemon: 1, procs, seq: None, cost: None, containers: None, cgroups: None }
    }

    #[test]
    fn end_is_a_tree_term_and_force_end_a_lone_kill_both_checked_against_the_row() {
        let p = ticked(41, "npm run dev", Some("tick-1"));
        assert_eq!(
            kill_frame(&p, "TERM", true),
            json!({ "op": "proc.kill", "pid": 41, "signal": "TERM", "mode": "tree", "startedAt": 1_700_000_000_000i64, "identity": "tick-1" })
        );
        assert_eq!(
            kill_frame(&p, "KILL", false),
            json!({ "op": "proc.kill", "pid": 41, "signal": "KILL", "startedAt": 1_700_000_000_000i64, "identity": "tick-1" })
        );
    }

    #[test]
    fn only_a_reply_with_no_signalled_list_is_a_too_old_daemons() {
        assert!(!reached_alone(&json!({ "id": 3, "ok": true, "signalled": [41, 42], "forced": [], "left": [] })));
        assert!(reached_alone(&json!({ "id": 3, "ok": true })));
    }

    #[test]
    fn force_end_offers_kill_only_to_what_the_frame_still_lists_as_itself() {
        let said = [ticked(41, "a", Some("tick-1")), ticked(42, "b", Some("tick-2")), ticked(43, "c", None)];
        let frame = snapshot(vec![entry(41, Some("tick-1")), entry(42, Some("tick-9")), entry(43, Some("tick-3"))]);
        assert_eq!(still_listed(&said, Some(&frame)).iter().map(|p| p.pid).collect::<Vec<_>>(), [41, 43], "42 is another process now");
        assert!(still_listed(&said, None).is_empty());
    }

    #[test]
    fn the_too_old_notice_says_the_webs_words() {
        assert_eq!(
            w::kill_too_old_of("mac", &[(41, "npm run dev")]),
            "TERM went to 41 (npm run dev) alone: mac runs a daemon too old to end what it started; wsp add mac --update updates it"
        );
        assert_eq!(
            w::kill_too_old_of("box", &[(41, "a"), (42, "b")]),
            "TERM went to 41 (a) and 42 (b) alone: box runs a daemon too old to end what they started; wsp add box --update updates it"
        );
    }

    #[test]
    fn a_slates_heading_takes_the_title_slates_get_answers() {
        assert_eq!(
            slate_title(&json!({ "slate": { "threadId": "t", "document": { "title": "Deploy board" } } })).as_deref(),
            Some("Deploy board")
        );
        assert_eq!(slate_title(&json!({ "slate": { "threadId": "t", "document": null } })), None);
        assert_eq!(slate_title(&json!({ "slate": null })), None);
    }

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui_kit::test]
    fn what_a_too_old_daemon_signalled_alone_is_said_once_after_the_wait(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx));
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap());
        let place: PlaceView = serde_json::from_value(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })).unwrap();
        let computer = Computer { glyph: ComputerIcon::of(&place, None), place, key: HERE.into(), workspace_id: None };
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
        let scoped = cx.update(|window, cx| cx.new(|cx| Scoped::new(computer, acts, View::Threads, window, cx)));
        scoped.update(cx, |s, cx| {
            s.snapshot = Some(snapshot(vec![entry(41, Some("tick-1"))]));
            for p in [ticked(41, "npm run dev", Some("tick-1")), ticked(42, "vite", None)] {
                s.ending.insert(p.pid);
                s.wait_too_old(p, cx);
            }
        });
        let said = |cx: &mut gpui_kit::VisualTestContext| {
            let toasts = cx.update(|window, cx| crate::ui::toast::of(window, cx).expect("ui::init registers the stack"));
            toasts.read_with(cx, |t, _| t.standing())
        };
        cx.executor().advance_clock(ESCALATE / 2);
        cx.run_until_parked();
        assert_eq!(said(cx).len(), 0, "nothing is said before TERM has had its wait");
        cx.executor().advance_clock(ESCALATE);
        cx.run_until_parked();
        let notices = said(cx);
        assert_eq!(notices.len(), 1, "one notice names both");
        let n = &notices[0];
        assert_eq!(n.text.as_ref(), w::kill_too_old_of("mac", &[(41, "npm run dev"), (42, "vite")]));
        assert_eq!(n.place.as_deref(), Some("mac"), "where it happened, as the web's notice says it");
        assert_eq!(n.action.as_ref().map(|a| a.word.as_ref()), Some(w::FORCE_END), "41 is still listed as itself");
        assert!(scoped.read_with(cx, |s, _| s.ending.is_empty() && s.too_old.is_empty() && s.too_old_wait.is_none()));
    }

    #[gpui_kit::test]
    fn a_sample_that_moves_no_word_draws_nothing(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx));
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap());
        let place: PlaceView = serde_json::from_value(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })).unwrap();
        let computer = Computer { glyph: ComputerIcon::of(&place, None), place, key: HERE.into(), workspace_id: None };
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
        let scoped = cx.update(|window, cx| cx.new(|cx| Scoped::new(computer, acts, View::Threads, window, cx)));
        let drawn = Rc::new(std::cell::Cell::new(0));
        let counted = drawn.clone();
        cx.update(|_, cx| cx.observe(&scoped, move |_, _| counted.set(counted.get() + 1)).detach());
        let sample = |used: f64, total: f64| Arrival::Event(json!({ "type": "sys.sample", "mem": { "used": used, "total": total } }));
        let take = |arrival: Arrival, cx: &mut gpui_kit::VisualTestContext| {
            scoped.update(cx, |s, cx| s.arrive(arrival, cx));
            cx.run_until_parked();
            drawn.get()
        };
        assert_eq!(take(sample(4.0e9, 16.0e9), cx), 1, "the first sample draws");
        assert_eq!(take(sample(4.0e9 + 1024., 16.0e9), cx), 1, "a few bytes more read the same words and draw nothing");
        assert_eq!(take(sample(6.0e9, 16.0e9), cx), 2, "new words draw");
        assert_eq!(take(sample(6.0e9, 32.0e9), cx), 3, "a new total moves the tint and draws");
        assert_eq!(scoped.read_with(cx, |s, _| s.mem), Some((6.0e9, 32.0e9)), "the newest sample is held either way");
    }

    #[gpui_kit::test]
    fn a_failed_resync_leaves_the_next_lost_frame_free_to_ask_again(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx));
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let place: PlaceView = serde_json::from_value(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })).unwrap();
        let computer = Computer { glyph: ComputerIcon::of(&place, None), place, key: HERE.into(), workspace_id: None };
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
        let scoped = cx.update(|window, cx| cx.new(|cx| Scoped::new(computer, acts, View::Threads, window, cx)));
        // Changes on a frame the page never held: the daemon is asked for a whole list, and with no link it refuses.
        let lost = json!({ "type": "proc.changes", "at": 1.0, "daemon": 1, "seq": 9, "base": 7 });
        scoped.update(cx, |s, cx| {
            s.snapshot = Some(Snapshot { seq: Some(5), ..snapshot(vec![]) });
            s.take_frame(&lost, cx);
            assert!(s.unavailable.is_some(), "the resync was asked and refused");
            s.unavailable = None;
            s.take_frame(&lost, cx);
            assert!(s.unavailable.is_some(), "the next lost frame asks again, the refused resync no longer standing in its way");
        });
    }

    #[test]
    fn a_box_reads_its_threads_cgroups_until_every_process_is_asked() {
        assert_eq!(reads(true, View::Threads, false), None);
        assert_eq!(reads(false, View::Threads, true), Some(vec![THREAD_CGROUPS.to_owned()]));
        assert_eq!(reads(false, View::Processes, false), Some(vec![THREAD_CGROUPS.to_owned()]));
        assert_eq!(reads(false, View::Processes, true), None);
    }
}
