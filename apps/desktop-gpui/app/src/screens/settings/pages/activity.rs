// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Activity (`activity/`): every thread's processes on one computer, with their memory, CPU, energy and
//! ports. Drawing the page holds that computer's daemon link and its process watch; leaving it drops the page's state,
//! which closes the link, so nothing samples while it is closed. The totals carry a line of the last minutes, kept for
//! as long as the page stands and dropped with it.

pub mod live;
pub mod model;
mod pickers;
mod pieces;
pub(crate) mod place;
pub(crate) mod tables;
mod totals;
pub(crate) mod wire;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::channel::{mpsc, oneshot};
use futures::StreamExt as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::v_flex;
use gpui_kit::{div, prelude::*, px, AnyElement, AnyWindowHandle, App, Context, Entity, SharedString, Subscription, Task, Window};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{ActivityEndRequest, PlaceView, SessionsInterruptRequest, SlatesGetRequest};
use wsp_host_client::HostClient;

use crate::screens::app_shell::title::thread_title;
use crate::screens::dialogs::layer;
use crate::screens::settings::model::req;
use crate::screens::settings::page::{CardData, Ctx, DialogBuild};
use crate::screens::settings::words::HERE;
use crate::screens::settings::Acts;
use crate::screens::sidebar::menu::{absent_of, place_of};
use crate::screens::sidebar::model::{Row, Status as RowStatus, Subagent, SubagentState};
use crate::screens::Host;
use crate::ui::catalog;
use crate::ui::context_menu::ContextMenu;
use crate::ui::fmt_bytes;
use crate::ui::select::Select;
use crate::ui::{narrow, text, ActivePalette, AlertDialog, Button, ComputerIcon, Notice, NoticeAction, Segmented, Status, Width};
pub(crate) use live::wait as redial;
use live::{Arrival, Held, Listing, Reads, Target};
use model::{
    age_word, filter_groups, flat_rows, fmt_elapsed, kind_groups, port_rows, ports_by_pid, push_history, scope_of, sort_groups,
    total_spent, words as w, Group, HistoryPoint, Picks, Proc, Scope, ScopeInput, Sort, SubState, SubagentIn, Thread, View,
};
use pickers::{controls, selects, synced, Items, Selects};
use place::{thread_cgroup, THREAD_CGROUPS};
use tables::{Hand, ThreadLooks};
use totals::{rows_skeleton, skeletons_move, tiles, tiles_skeleton};
use wire::{apply_changes, apply_port_event, memory_of, AppPart, Frame, KnownPort, Pid, Snapshot};

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
pub(crate) struct Computer {
    place: PlaceView,
    glyph: ComputerIcon,
    key: String,
    workspace_id: Option<String>,
}

impl Computer {
    /// `place` read over its own link, or over the link of its folder `workspace_id`.
    pub(crate) fn new(place: PlaceView, glyph: ComputerIcon, workspace_id: Option<String>) -> Self {
        let key = workspace_id.clone().unwrap_or_else(|| HERE.into());
        Computer { place, glyph, key, workspace_id }
    }

    pub(crate) fn here(&self) -> bool {
        self.place.id == HERE
    }

    pub(crate) fn name(&self) -> String {
        super::computers::place_name(&self.place)
    }

    pub(crate) fn place_id(&self) -> &str {
        &self.place.id
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
fn title_of(first: &Row, latest: &Row) -> String {
    thread_title(latest.harness_title.as_deref(), first.prompt.as_deref())
        .unwrap_or_else(|| first.claude_session_id.clone().unwrap_or_else(|| first.id.clone()))
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.)
}

/// `useActivityThreads` with `foldThreads`: the threads of every folder on the computer, and each one's status mark.
/// Each keeps the pids its turn recorded where `pids` asks, which name processes only on the computer they ran on.
pub(crate) fn threads_on(
    listing: &Listing,
    places: &[(PlaceView, ComputerIcon)],
    computer: &Computer,
    pids: bool,
) -> (Vec<Thread>, Latest) {
    let mut threads = Vec::new();
    let mut latests = Vec::new();
    let listed = listed(places);
    for w in listing.workspaces.iter() {
        if place_of(w, &listed).as_deref() != Some(computer.place.id.as_str()) {
            continue;
        }
        let Some(wid) = field(w, "id") else { continue };
        let project = w.pointer("/project/name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let mut order: Vec<String> = Vec::new();
        let mut turns: HashMap<String, Vec<&Arc<Row>>> = HashMap::new();
        for s in listing.sessions.iter().filter(|s| s.workspace_id == wid) {
            let key = s.thread_id.clone().unwrap_or_else(|| s.id.clone());
            if !turns.contains_key(&key) {
                order.push(key.clone());
            }
            turns.entry(key).or_default().push(s);
        }
        for id in order {
            let mut rows = turns.remove(&id).unwrap_or_default();
            rows.sort_by(|a, b| {
                let at = |s: &Row| s.started_at.unwrap_or(0.);
                at(a).partial_cmp(&at(b)).unwrap_or(std::cmp::Ordering::Equal)
            });
            let (first, latest) = (rows[0], rows[rows.len() - 1]);
            let title = title_of(first, latest);
            let (pid, agent_pid) = (latest.pid, latest.agent_pid);
            let subagents = latest.subagents.iter().map(subagent_in).collect();
            let here = computer.here();
            threads.push(Thread {
                id: id.clone(),
                title,
                harness: first.harness.clone(),
                project: project.clone(),
                workspace_id: wid.to_owned(),
                session_id: latest.id.clone(),
                working: latest.status == RowStatus::Running,
                pid: pid.filter(|_| pids),
                agent_pid: agent_pid.filter(|_| pids),
                cgroup: (!here).then(|| thread_cgroup(&id)),
                parent: first.parent_thread_id.clone(),
                slate: None,
                subagents,
            });
            latests.push((id, latest.clone()));
        }
    }
    (threads, latests)
}

fn subagent_in(s: &Subagent) -> SubagentIn {
    let state = match s.state {
        SubagentState::Running => SubState::Running,
        SubagentState::Done => SubState::Done,
        SubagentState::Failed => SubState::Failed,
        SubagentState::Stopped => SubState::Stopped,
    };
    SubagentIn {
        id: s.id.clone(),
        title: s.title.clone(),
        state,
        started_at: s.started_at,
        ended_at: s.ended_at,
        tokens: s.tokens,
        duration_ms: s.duration_ms,
        model: s.model.clone(),
    }
}

/// A thread head's status mark: what it asks, its running clock, its failure or unread finish, else its age.
pub(crate) fn status_of(latest: &Row, now: f64) -> Status {
    let (started, ended) = (latest.started_at, latest.ended_at);
    if latest.asking {
        return Status::NeedsYou;
    }
    if latest.status == RowStatus::Running {
        return Status::Working(fmt_elapsed(now - started.unwrap_or(now)).into());
    }
    if latest.status == RowStatus::Failed {
        return Status::Failed;
    }
    let read = latest.read_at;
    if latest.status == RowStatus::Completed && ended.is_some_and(|e| read.is_none_or(|r| r < e)) {
        return Status::Done;
    }
    Status::Read(ended.or(started).map_or_else(String::new, |at| age_word(at, now)).into())
}

/// Each thread's newest turn, which its status mark reads.
pub(crate) type Latest = Vec<(String, Arc<Row>)>;

/// The threads and the computer a page reads, kept while the page is drawn.
pub(crate) struct Lists {
    pub(crate) listing: Option<Listing>,
    /// The threads of the computer last asked for, read again only when the lists or the computer move.
    cache: Option<(String, Vec<Thread>, Latest)>,
    _follow: Option<Held>,
    _receive: Task<()>,
}

impl Lists {
    pub(crate) fn new(acts: &Acts, cx: &mut Context<Self>) -> Self {
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
    pub(crate) fn threads(&mut self, places: &[(PlaceView, ComputerIcon)], computer: &Computer) -> (Vec<Thread>, Latest) {
        let fresh = self.cache.as_ref().is_some_and(|(id, _, _)| *id == computer.place.id);
        if !fresh {
            let Some(listing) = &self.listing else { return Default::default() };
            let (threads, latests) = threads_on(listing, places, computer, computer.here());
            self.cache = Some((computer.place.id.clone(), threads, latests));
        }
        self.cache.as_ref().map(|(_, t, l)| (t.clone(), l.clone())).unwrap_or_default()
    }
}

/// What the shown groups were made from: the scope's read, the filter, the picks and the sort.
type ShownKey = (u64, String, Picks, Sort);

/// The key a pane's ask stands under on the window's dialogs layer.
const ASK: &str = "activity-ask";

/// Where Activity's asks and refusals stand: Settings' own dialog and notice, or, for the Processes pane, the window's
/// dialogs layer and its toasts, as the web's `addNotice` says them on either.
#[derive(Clone)]
pub(crate) struct Voice {
    pub(crate) acts: Acts,
    pane: Option<AnyWindowHandle>,
}

/// What draws an ask, handed the voice its buttons close it with.
type AskBuild = Rc<dyn Fn(&Voice, &mut Window, &mut App) -> AnyElement>;

/// A pane's ask, drawn off the builder its act left.
struct Asked(AskBuild, Voice);

impl Render for Asked {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        (self.0)(&self.1, window, cx)
    }
}

impl Voice {
    pub(crate) fn failed(&self, said: impl Into<String>, cx: &mut App) {
        let Some(window) = self.pane else { return self.acts.failed(said, cx) };
        let note = Notice::error(said.into());
        cx.defer(move |cx| _ = window.update(cx, |_, window, cx| crate::ui::notify_in(window, cx, note)));
    }

    fn ask(&self, build: impl Fn(&Voice, &mut Window, &mut App) -> AnyElement + 'static, cx: &mut App) {
        let Some(window) = self.pane else {
            let voice = self.clone();
            return self.acts.open_dialog(DialogBuild::alert(move |_, window, cx| build(&voice, window, cx)), cx);
        };
        let view = cx.new(|_| Asked(Rc::new(build), self.clone()));
        cx.defer(move |cx| {
            _ = window.update(cx, |_, window, cx| layer::open(ASK, view.into(), layer::Closes::Escape, None, window, cx));
        });
    }

    fn close(&self, cx: &mut App) {
        let Some(window) = self.pane else { return self.acts.close_dialog(cx) };
        cx.defer(move |cx| _ = window.update(cx, |_, window, cx| layer::close(ASK, window, cx)));
    }
}

/// One computer's page: its link, its frame read into groups, the page's picks and folds, and what is ending.
pub struct Scoped {
    pub(crate) computer: Computer,
    pub(crate) voice: Voice,
    host: Option<(Arc<HostClient>, Arc<Runtime>)>,
    pub(crate) channel: Option<String>,
    pub(crate) snapshot: Option<Snapshot>,
    resyncing: bool,
    pub(crate) unavailable: Option<String>,
    ports: Vec<KnownPort>,
    mem: Option<(f64, f64)>,
    pub(crate) app: HashMap<Pid, AppPart>,
    threads: Vec<Thread>,
    pub(crate) scope: Option<Scope>,
    /// Counts the scope's reads, so the filtered and sorted groups are made again only when something moved.
    generation: u64,
    shown: Option<(ShownKey, Rc<Vec<Group>>)>,
    history: Vec<HistoryPoint>,
    interval: f64,
    pub(crate) sort: Sort,
    filter: Entity<InputState>,
    filter_text: String,
    picks: Picks,
    every: bool,
    pub(crate) open: HashSet<String>,
    pub(crate) ending: HashSet<Pid>,
    pub(crate) stopping: HashSet<String>,
    /// Processes a daemon too old for tree kills signalled alone, said in one notice once the wait is over.
    too_old: Vec<Proc>,
    too_old_wait: Option<Task<()>>,
    /// Each slated thread's title once `slates.get` answered, and the threads already asked.
    slates: HashMap<String, String>,
    slates_asked: HashSet<String>,
    /// The rows' right-click menu, the page's own off the Mac.
    pub(crate) menu: ContextMenu,
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
    kill_of(p.pid, p.started_at, p.id.as_deref(), signal, tree)
}

/// A proc.kill of `pid`, checked against the start and the identity its row was listed with.
pub(crate) fn kill_of(pid: Pid, started_at: Option<f64>, id: Option<&str>, signal: &str, tree: bool) -> Value {
    let mut frame = json!({ "op": "proc.kill", "pid": pid, "signal": signal });
    if tree {
        frame["mode"] = json!("tree");
    }
    if let Some(at) = started_at {
        frame["startedAt"] = json!(at.round() as i64);
    }
    if let Some(id) = id {
        frame["identity"] = json!(id);
    }
    frame
}

/// A daemon too old for kill modes answers proc.kill with no `signalled`: TERM went to the pid alone.
pub(crate) fn reached_alone(reply: &Value) -> bool {
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
        let reads = reads(computer.here(), view, false);
        Scoped::build(computer, Voice { acts, pane: None }, reads, window, cx)
    }

    /// One computer's state for a pane of the right panel: its every process, or on a box only its threads', with the
    /// pane's refusals said as toasts and its asks on the window's dialogs layer. Dropping it closes the link.
    pub(crate) fn pane(computer: Computer, host: &Host, every: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), Some(host.client().clone()), host.runtime().clone());
        let reads = (!every && !computer.here()).then(|| vec![THREAD_CGROUPS.to_owned()]);
        Scoped::build(computer, Voice { acts, pane: Some(window.window_handle()) }, reads, window, cx)
    }

    fn build(computer: Computer, voice: Voice, reads: Option<Vec<String>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(w::FILTER));
        let sub = cx.subscribe(&filter, |this: &mut Scoped, field, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.filter_text = field.read(cx).value().to_string();
                cx.notify();
            }
        });
        let host = voice.acts.host(cx);
        let asked: Reads = Arc::new(Mutex::new(reads));
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
            voice,
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
    pub(crate) fn set_threads(&mut self, mut threads: Vec<Thread>) {
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
    pub(crate) fn ask_slates(&mut self, cx: &mut Context<Self>) {
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
            self.voice.acts.call(req::<SlatesGetRequest>(json!({ "threadId": id })), cx, move |answer, cx| {
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
    pub(crate) fn daemon(
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
                this.voice.failed(said, cx);
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
                    this.voice.failed(said, cx);
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
        self.voice.acts.call(req::<ActivityEndRequest>(json!({ "pid": pid, "startedAt": started })), cx, move |answer, cx| {
            if let Err(said) = answer {
                _ = this.update(cx, |s, cx| {
                    s.ending.remove(&pid);
                    s.voice.failed(said, cx);
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
                this.voice.failed(said, cx);
                cx.notify();
            }
        });
    }

    /// Stop asks before a running turn ends.
    pub fn ask_stop(&mut self, group: &Group, cx: &mut Context<Self>) {
        let Some(t) = group.thread.clone() else { return };
        let title = w::stop_title(&t.title);
        let line = w::stop_sentence(&catalog::name(&t.harness), t.cgroup.is_some());
        let session = t.session_id.clone();
        self.voice.ask(
            move |voice, _, _| {
                let (cancel, stop) = (voice.clone(), voice.clone());
                let session = session.clone();
                AlertDialog::new(title.clone())
                    .layered()
                    .sentence(line.clone())
                    .act(Button::outline("activity-cancel", w::CANCEL).neutral_ring().on_click(move |_, _, cx| cancel.close(cx)))
                    .act(Button::destructive("activity-confirm", w::STOP_TURN).on_click(move |_, _, cx| {
                        stop.close(cx);
                        let said = stop.clone();
                        stop.acts.call(
                            req::<SessionsInterruptRequest>(json!({ "sessionId": session })),
                            cx,
                            move |answer, cx| match answer {
                                Ok(result) => {
                                    if let Some(left) = result.left {
                                        said.failed(left, cx);
                                    }
                                }
                                Err(e) => said.failed(e, cx),
                            },
                        );
                    }))
                    .into_any_element()
            },
            cx,
        );
    }

    /// End all asks, naming how many processes it ends and what they hold.
    pub fn ask_end_all(&mut self, heading: &Proc, _group: &Group, cx: &mut Context<Self>) {
        let ends: Vec<Proc> = tables::ends_of(heading).into_iter().cloned().collect();
        let title = w::end_all_title(&heading.name);
        let line = w::end_all_sentence(ends.len(), &fmt_bytes(tables::held_by(heading) as u64));
        let this = cx.entity().downgrade();
        self.voice.ask(
            move |voice, _, _| {
                let (cancel, close) = (voice.clone(), voice.clone());
                let (this, ends) = (this.clone(), ends.clone());
                AlertDialog::new(title.clone())
                    .layered()
                    .sentence(line.clone())
                    .act(Button::outline("activity-cancel", w::CANCEL).neutral_ring().on_click(move |_, _, cx| cancel.close(cx)))
                    .act(Button::destructive("activity-confirm", w::END_ALL_PRESS).on_click(move |_, _, cx| {
                        close.close(cx);
                        _ = this.update(cx, |s, cx| {
                            for p in &ends {
                                s.end(p, cx);
                            }
                        });
                    }))
                    .into_any_element()
            },
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
                    let bare = crate::screens::settings::pages::agents::model::base_model(m);
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

#[cfg(test)]
mod tests;
