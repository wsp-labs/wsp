// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Activity (`activity/`): every thread's processes on one computer, with their memory, CPU, energy and
//! ports. Drawing the page holds that computer's daemon link and its process watch; leaving it drops the page's state,
//! which closes the link, so nothing samples while it is closed. The totals carry a line of the last minutes, kept for
//! as long as the page stands and dropped with it.

mod live;
mod model;
mod pieces;
mod place;
mod tables;
mod wire;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::StreamExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Entity, FontWeight, SharedString, Subscription, Task, Window};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{ActivityEndRequest, PlaceView, SessionsInterruptRequest};
use wsp_host_client::HostClient;

use crate::screens::settings::model::req;
use crate::screens::settings::page::{CardData, Ctx};
use crate::screens::settings::Acts;
use crate::ui::select::{Select, SelectItem, SelectState};
use crate::ui::{text, ActivePalette, AlertDialog, Button, Card, CardItem, Glyph, Segment, SegmentedControl, Skeleton, Status, Switch};
use live::{Arrival, Held, Listing, Target};
use model::{
    age_word, cpu_tint, energy_tint, energy_word, filter_groups, flat_rows, fmt_bytes, fmt_elapsed, kind_groups, memory_share_tint,
    pick_choices, port_rows, ports_by_pid, push_history, scope_of, sort_groups, total_spent, words as w, Group, HistoryPoint, Picks, Proc,
    Scope, ScopeInput, Sort, SubState, SubagentIn, Thread, Tint, View, HISTORY_POINTS,
};
use pieces::{computer_glyph, sparkline};
use place::{thread_cgroup, THREAD_CGROUPS};
use tables::{tint_ink, Hand, ThreadLooks};
use wire::{apply_changes, apply_port_event, AppPart, Frame, Kind, KnownPort, Pid, Snapshot};

/// The page's view and the computer it reads (`activityView`, `activityPlace`).
#[derive(Default)]
pub struct Pick {
    view: View,
    place: Option<String>,
}

fn pick_state(window: &mut Window, cx: &mut App) -> Entity<Pick> {
    window.use_keyed_state("activity-pick", cx, |_, _| Pick::default())
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let pick = pick_state(window, cx);
    let look = ctx.data.preferences().and_then(|p| serde_json::to_value(p).ok()).and_then(|p| p.get("computerLook").cloned());
    let places: Vec<(PlaceView, Glyph)> = ctx.data.places.iter().map(|p| (p.clone(), computer_glyph(p, look.as_ref()))).collect();
    let models: Vec<(String, String, String)> = ctx
        .data
        .harnesses
        .iter()
        .flat_map(|h| h.models.iter().map(move |m| (h.harness.to_string(), m.value.clone(), m.label.clone())))
        .collect();
    vec![CardData::new("activity").body(Page { pick, places, models, acts: ctx.acts.clone() })]
}

/// The page's views at the top bar's right end.
pub fn head(_ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let pick = pick_state(window, cx);
    let view = pick.read(cx).view;
    let segments = View::ALL.into_iter().map(|v| Segment::new(v, w::views(v))).collect();
    let control = SegmentedControl::new("activity-views", view, segments).large().on_change(move |v, _, cx| {
        let v = *v;
        pick.update(cx, |p, cx| {
            p.view = v;
            cx.notify();
        })
    });
    Some(control.into_any_element())
}

/// A computer Activity reads: the one the host runs on over its own link, or a box over a link of one of its folders.
#[derive(Clone, Debug)]
struct Computer {
    place: PlaceView,
    glyph: Glyph,
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

const HERE: &str = "here";

fn field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// `workspacePlaceId`: the row a workspace stands on, by the place its record names, this computer for a local one.
fn place_of(places: &[(PlaceView, Glyph)], w: &Value) -> Option<String> {
    match field(w, "place") {
        Some(named) => places.iter().find(|(p, _)| p.id == named).map(|(p, _)| p.id.clone()),
        None if field(w, "kind") == Some("local") => {
            places.iter().find(|(p, _)| p.id == HERE).or(places.first()).map(|(p, _)| p.id.clone())
        }
        None => None,
    }
}

/// `activityComputers`: this computer, and each box a folder of which is on it.
fn computers(places: &[(PlaceView, Glyph)], workspaces: &[Value]) -> Vec<Computer> {
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
                .find(|w| field(w, "kind") == Some("place") && place_of(places, w).as_deref() == Some(place.id.as_str()))?;
            let id = field(folder, "id")?.to_owned();
            Some(Computer { place: place.clone(), glyph: *glyph, key: id.clone(), workspace_id: Some(id) })
        })
        .collect()
}

/// `titleLine` and `openingTitle`: the first line, and an opening turn's first sentence cut to 48 characters.
fn title_line(text: &str) -> String {
    text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn opening_title(text: &str) -> String {
    let line = title_line(text);
    let sentence =
        line.char_indices().find(|(i, c)| ".!?".contains(*c) && line[i + c.len_utf8()..].chars().next().is_none_or(char::is_whitespace));
    let first = sentence.map_or(line.as_str(), |(i, c)| &line[..i + c.len_utf8()]);
    cut_line(first, 48)
}

fn cut_line(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_owned();
    }
    let head: String = text.chars().take(room).collect();
    let trim = |s: &str| s.trim_end_matches(|c: char| c.is_whitespace() || ",;:".contains(c)).to_owned();
    let words = head.rfind(' ').filter(|at| *at > 0).map(|at| trim(&head[..at]));
    let room = room - 1;
    let words = words.or_else(|| Some(trim(&text.chars().take(room).collect::<String>())));
    let mut out = words.unwrap_or_default();
    if out.chars().count() > room {
        out = out.chars().take(room).collect();
    }
    format!("{out}\u{2026}")
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.)
}

/// `useActivityThreads` with `foldThreads`: the threads of every folder on the computer, and each one's status mark.
fn threads_on(listing: &Listing, places: &[(PlaceView, Glyph)], computer: &Computer) -> (Vec<Thread>, Latest) {
    let mut threads = Vec::new();
    let mut latests = Vec::new();
    for w in &listing.workspaces {
        if place_of(places, w).as_deref() != Some(computer.place.id.as_str()) {
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
            let title = match (field(latest, "harnessTitle"), field(first, "prompt")) {
                (Some(t), _) => title_line(t),
                (None, Some(prompt)) => opening_title(prompt),
                _ => field(first, "claudeSessionId").or(field(first, "id")).unwrap_or_default().to_owned(),
            };
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

/// The threads and the computer a page reads, kept while the page is drawn.
/// Each thread's newest turn, which its status mark reads.
type Latest = Vec<(String, Value)>;

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
    fn threads(&mut self, places: &[(PlaceView, Glyph)], computer: &Computer) -> (Vec<Thread>, Latest) {
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
    /// The cgroups the last proc.watch named, none for every process.
    asked: Option<Vec<String>>,
    _link: Option<Held>,
    _receive: Task<()>,
    _filter: Subscription,
}

/// Two seconds, the sampler's cadence, until two frames say what it is.
const FIRST_INTERVAL_MS: f64 = 2000.;

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
        let asked = reads(computer.here(), view, false);
        let (tx, mut rx) = mpsc::unbounded();
        let target = match &computer.workspace_id {
            Some(id) => Target::Workspace(id.clone()),
            None => Target::Place(HERE.into()),
        };
        let link = host.as_ref().map(|(h, rt)| live::follow_link(h.clone(), rt, target, computer.here(), asked.clone(), tx));
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
            asked,
            _link: link,
            _receive: receive,
            _filter: sub,
        }
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        match arrival {
            Arrival::Opened(channel) => self.channel = Some(channel),
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
                    let used = event.pointer("/mem/used").and_then(Value::as_f64);
                    let total = event.pointer("/mem/total").and_then(Value::as_f64);
                    if let (Some(used), Some(total)) = (used, total) {
                        self.mem = Some((used, total));
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
                let frame = live::watch_frame(self.asked.as_deref());
                self.daemon(frame, cx, |_, _, _| {});
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
    fn set_threads(&mut self, threads: Vec<Thread>) {
        if threads != self.threads {
            self.threads = threads;
            self.recompute();
        }
    }

    /// What the daemon reads each tick: on a box the threads' cgroups, and the whole table once Every process asks.
    fn set_reads(&mut self, view: View, cx: &mut Context<Self>) {
        let want = reads(self.computer.here(), view, self.every);
        if want == self.asked || self.channel.is_none() {
            return;
        }
        self.asked = want.clone();
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

    /// The process and everything under it, TERM then KILL in the daemon, checked against the row's identity.
    pub fn end(&mut self, p: &Proc, cx: &mut Context<Self>) {
        let pid = p.pid;
        self.ending.insert(pid);
        cx.notify();
        let mut frame = json!({ "op": "proc.kill", "pid": pid, "signal": "TERM", "mode": "tree" });
        if let Some(at) = p.started_at {
            frame["startedAt"] = json!(at.round() as i64);
        }
        if let Some(id) = &p.id {
            frame["identity"] = json!(id);
        }
        self.daemon(frame, cx, move |this, result, cx| {
            if let Err(said) = result {
                this.ending.remove(&pid);
                this.acts.failed(said, cx);
                cx.notify();
            }
        });
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
        let line = w::stop_sentence(&place::agent_name(&t.harness), t.cgroup.is_some());
        let session = t.session_id.clone();
        self.acts.open_dialog(
            Rc::new(move |_, _, _| {
                let (cancel, stop) = (acts.clone(), acts.clone());
                let session = session.clone();
                AlertDialog::new(title.clone())
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
        let line = w::end_all_sentence(ends.len(), &fmt_bytes(tables::held_by(heading)));
        let acts = self.acts.clone();
        let this = cx.entity().downgrade();
        self.acts.open_dialog(
            Rc::new(move |_, _, _| {
                let (cancel, close) = (acts.clone(), acts.clone());
                let (this, ends) = (this.clone(), ends.clone());
                AlertDialog::new(title.clone())
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
    places: Vec<(PlaceView, Glyph)>,
    models: Vec<(String, String, String)>,
    acts: Acts,
}

impl RenderOnce for Page {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
            s.set_reads(view, cx);
            let gone = s.picks.thread.as_ref().is_some_and(|t| s.scope.as_ref().is_some_and(|sc| !sc.groups.iter().any(|g| g.id == *t)));
            if gone {
                s.picks.thread = None;
            }
            s.refresh_shown();
        });
        let selects = selects(&computers, &computer, &scoped, window, cx);
        body(&self, selects, &computer, view, &scoped, &looks, cx).into_any_element()
    }
}

/// The computer's state when it is a box that is not answering (`useAbsentComputer`).
fn absent(computer: &Computer) -> Option<(String, String)> {
    (!computer.here() && computer.place.present == Some(false))
        .then(|| (format!("{} is not answering", computer.name()), "it connects on its own when it is on".to_owned()))
}

fn note(word: impl Into<SharedString>, cx: &App) -> gpui_kit::Div {
    div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(cx.palette().muted_foreground).child(word.into())
}

fn body(
    page: &Page,
    selects: Selects,
    computer: &Computer,
    view: View,
    scoped: &Entity<Scoped>,
    looks: &HashMap<String, Status>,
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
    };
    let tiles = match (&away, scope) {
        (Some(_), _) => None,
        (None, None) => Some(tiles_skeleton(cx)),
        (None, Some(sc)) => Some(tiles(sc, &s.history, s.mem, cx)),
    };
    let bar = controls(page, selects, view, scoped, s, cx);
    let unavailable = s.unavailable.clone().filter(|_| s.snapshot.is_none() && away.is_none());
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
        rows_skeleton(cx).into_any_element()
    };
    v_flex().id("activity").debug_selector(|| "activity".into()).w_full().gap(px(40.)).children(tiles).child(bar).child(main)
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
    first: bool,
    cx: &App,
) -> AnyElement {
    let p = cx.palette();
    v_flex()
        .id(id)
        .flex_1()
        .min_w_0()
        .gap(px(6.))
        .px_5()
        .py_5()
        .when(!first, |el| el.border_l_1().border_color(p.border.opacity(0.5)))
        .child(div().text_size(text::FACT).line_height(px(19.5)).text_color(p.muted_foreground).child(label))
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
        .children(line.map(|(points, ink)| div().mt_2().child(sparkline(points, ink, p.border))))
        .into_any_element()
}

/// The last minutes as a line, scaled to the highest point in the window and never below the floor.
fn line_points(history: &[HistoryPoint], y: impl Fn(&HistoryPoint) -> f64, floor: f64) -> Vec<(f32, f32)> {
    let to = history.last().map_or(0., |h| h.at);
    let from = to - (HISTORY_POINTS as f64 - 1.) * FIRST_INTERVAL_MS;
    let top = history.iter().map(&y).fold(floor, f64::max) * 1.15;
    history.iter().map(|h| (((h.at - from) / (to - from).max(1.)).clamp(0., 1.) as f32, (y(h) / top).clamp(0., 1.) as f32)).collect()
}

/// wsp's totals on this computer, each figure over a line of the last minutes; the process count stands alone.
fn tiles(scope: &Scope, history: &[HistoryPoint], mem: Option<(f64, f64)>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let t = total_spent(&scope.groups);
    let threads = scope.groups.iter().filter(|g| g.kind == model::GroupKind::Thread).count();
    let mem_note = mem.map_or_else(|| w::PROCESSES.to_owned(), |(used, _)| w::of(&fmt_bytes(used)));
    // CPU sky, memory violet and power teal on the web, inks the palette does not hold.
    let mut row = vec![
        tile(
            "activity-mem",
            w::WSP,
            fmt_bytes(t.mem),
            mem_note,
            memory_share_tint(t.mem, mem.map(|m| m.1)),
            Some((line_points(history, |h| h.mem, 1024f64.powi(3)), p.primary)),
            true,
            cx,
        ),
        tile(
            "activity-cpu",
            w::CPU,
            format!("{}%", t.cpu.round()),
            w::ONE_CORE.into(),
            cpu_tint(t.cpu),
            Some((line_points(history, |h| h.cpu, 100.), p.info_foreground)),
            false,
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
            Some((line_points(history, |h| h.energy.unwrap_or(0.), 1000.), p.success)),
            false,
            cx,
        ));
    }
    row.push(tile("activity-procs", w::PROCESSES, t.count.to_string(), w::threads(threads), None, None, false, cx));
    let tiles = h_flex().debug_selector(|| "activity-totals".into()).items_stretch().children(row);
    Card::new("activity-totals").locked(true).item(CardItem::any(tiles)).into_any_element()
}

/// A bar in the skeleton's tier at the width of whatever holds it.
fn bar(height: f32, cx: &App) -> impl IntoElement {
    div().w_full().h(px(height)).rounded(px(6.)).bg(cx.palette().border)
}

fn tiles_skeleton(cx: &App) -> AnyElement {
    let p = cx.palette();
    let cell = |ix: usize| {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(6.))
            .px_5()
            .py_5()
            .when(ix > 0, |el| el.border_l_1().border_color(p.border.opacity(0.5)))
            .child(div().my_1().child(Skeleton::new(px(64.), px(12.))))
            .child(div().my(px(2.)).child(Skeleton::new(px(112.), px(28.))))
            .child(div().my(px(2.)).child(Skeleton::new(px(96.), px(12.))))
            .child(div().mt_2().child(bar(40., cx)))
    };
    Card::new("activity-totals-loading").locked(true).item(CardItem::any(h_flex().children((0..4).map(cell)))).into_any_element()
}

fn rows_skeleton(cx: &App) -> impl IntoElement {
    let row = |ix: usize| {
        CardItem::placed(move |corners| {
            corners
                .apply(h_flex())
                .id(("activity-loading", ix))
                .min_h(px(60.))
                .gap_3()
                .px(corners.inset)
                .py_3()
                .child(Skeleton::new(px(32.), px(32.)))
                .child(Skeleton::new(px(192.), px(16.)))
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

type Items<T> = Vec<(T, SharedString, Option<Glyph>)>;

fn select_items<T: Clone + PartialEq + 'static>(items: &Items<T>) -> Vec<SelectItem<T>> {
    items
        .iter()
        .map(|(v, label, glyph)| {
            let item = SelectItem::new(v.clone(), label.clone());
            match *glyph {
                Some(g) => item.mark(move |cx| {
                    Icon::new(g).with_size(px(14.)).flex_none().text_color(cx.palette().foreground.opacity(0.8)).into_any_element()
                }),
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

/// The page's own pickers and its filter, at the height of the Agents page's computer picker.
fn controls(page: &Page, selects: Selects, view: View, scoped: &Entity<Scoped>, s: &Scoped, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let pick = page.pick.clone();
    let (kinds, threads) = (scoped.downgrade(), scoped.downgrade());
    let left = h_flex()
        .min_w_0()
        .flex_wrap()
        .gap_2()
        .child(Select::new(&selects.computer).width(px(224.)).on_change(move |id, _, cx| {
            let id = id.clone();
            pick.update(cx, |p, cx| {
                p.place = Some(id);
                cx.notify();
            })
        }))
        .child(Select::new(&selects.kind).width(px(160.)).on_change(move |kind, _, cx| {
            let kind = *kind;
            _ = kinds.update(cx, |s, cx| {
                s.picks.kind = kind;
                cx.notify();
            });
        }))
        .child(Select::new(&selects.thread).width(px(192.)).on_change(move |thread, _, cx| {
            let thread = thread.clone();
            _ = threads.update(cx, |s, cx| {
                s.picks.thread = thread;
                cx.notify();
            });
        }));
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
        .h(px(32.))
        .w(px(256.))
        .rounded(crate::ui::size::CONTROL_RADIUS)
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::FACT);
    h_flex()
        .debug_selector(|| "activity-controls".into())
        .flex_wrap()
        .justify_between()
        .gap_x_4()
        .gap_y_2()
        .child(left)
        .child(h_flex().gap_4().children(every).child(filter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_opening_turn_titles_by_its_first_sentence() {
        assert_eq!(opening_title("Fix the login bug. Then add tests."), "Fix the login bug.");
        assert_eq!(opening_title("\n  port   the page\nsecond line"), "port the page");
        let long = "word ".repeat(20);
        let cut = opening_title(&long);
        assert!(cut.ends_with('\u{2026}') && cut.chars().count() <= 48, "{cut}");
    }

    #[test]
    fn a_box_reads_its_threads_cgroups_until_every_process_is_asked() {
        assert_eq!(reads(true, View::Threads, false), None);
        assert_eq!(reads(false, View::Threads, true), Some(vec![THREAD_CGROUPS.to_owned()]));
        assert_eq!(reads(false, View::Processes, true), None);
    }
}
