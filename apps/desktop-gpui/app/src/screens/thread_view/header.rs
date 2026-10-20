// SPDX-License-Identifier: AGPL-3.0-only
//! The thread's parts of the header: its crumbs with the rename box (ThreadBreadcrumb), and its cluster at the header's
//! right (AppShell's `ml-auto mr-px gap-2` row): ContextMeter, GitSplit and OpenSplit.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, svg, Anchor, AnyElement, App, BoxShadow, Context, Entity, FontWeight, Global, Hsla,
    KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, SharedString, Subscription, Task, WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    HarnessesListRequest, ProjectsListRequest, SessionsListRequest, SessionsRenameRequest, WorkspacesCheckoutRequest, WorkspacesWakeRequest,
};
use wsp_host_client::{Event, Op};

use crate::screens::app_shell::frame::{crumb_line, TITLE_SHARE};
use crate::screens::app_shell::title::thread_title;
use crate::screens::pr_pane::files::open_split::{self, OpenSplit};
use crate::screens::pr_pane::pull_request::git_split::GitSplit;
use crate::thread_view::ThreadView;
use crate::transcript::TurnContext;
use crate::ui::{tip, ActivePalette, Glyph, Notice, Palette};
use crate::words::fmt_tokens;

// The context window meter, contextMeter.logic.ts.

pub const CONTEXT_HEAD: &str = "Context window";
pub const COMPACT: &str = "Compact context";
pub const NOT_YET: &str = "Shows once the agent reports what it holds";
pub const FAILED_EARLY: &str = "The agent reported nothing before the turn failed";
pub const LIMIT_AT_END: &str = "The limit shows once a turn finishes";
/// TURN_IN_FLIGHT in packages/protocol/src/words/thread.ts.
pub const TURN_IN_FLIGHT: &str = "Turn in flight";

fn no_limit(agent: &str) -> String {
    format!("{agent} does not report its limit")
}

fn no_figure(agent: &str) -> String {
    format!("{agent} does not report how much context it holds")
}

/// What the model held at the newest turn that said, out of the newest window an agent named.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub used: f64,
    pub max: Option<f64>,
    /// The meter's fill from 0 to 1; none where there is no limit to measure against.
    pub share: Option<f64>,
}

pub fn snapshot(turns: &[TurnContext]) -> Option<Snapshot> {
    let mut used = None;
    for turn in turns.iter().rev() {
        let used = match used {
            Some(used) => used,
            None => match turn.tokens_context.or(turn.held_context) {
                Some(context) if context >= 0. => *used.insert(context),
                _ => continue,
            },
        };
        if let Some(window) = turn.tokens_window.or(turn.held_window).filter(|w| *w > 0.) {
            return Some(Snapshot { used, max: Some(window), share: Some((used / window).min(1.)) });
        }
    }
    used.map(|used| Snapshot { used, max: None, share: None })
}

/// Why a thread that ran a turn shows no reading, or why its limit is missing; none where nothing is.
pub fn missing(turns: &[TurnContext], agent: &str) -> Option<String> {
    let last = turns.last()?;
    let counted: Vec<&TurnContext> = turns.iter().filter(|t| !t.running && t.counted).collect();
    match snapshot(turns) {
        None if !counted.is_empty() => Some(no_figure(agent)),
        None => Some(if last.failed { FAILED_EARLY } else { NOT_YET }.to_owned()),
        Some(s) if s.max.is_some() => None,
        Some(_) if counted.iter().any(|t| t.tokens_context.is_some()) => Some(no_limit(agent)),
        Some(_) => Some(LIMIT_AT_END.to_owned()),
    }
}

/// The figure beside the meter: what was held over the limit, or what was held alone.
pub fn figure(s: &Snapshot) -> String {
    match s.max {
        Some(max) => format!("{} / {}", fmt_tokens(s.used), fmt_tokens(max)),
        None => fmt_tokens(s.used),
    }
}

/// The share as a percentage, one decimal under ten.
pub fn percent(s: &Snapshot) -> Option<String> {
    let value = s.share? * 100.;
    if value < 10. {
        let fixed = format!("{value:.1}");
        return Some(format!("{}%", fixed.strip_suffix(".0").unwrap_or(&fixed)));
    }
    Some(format!("{}%", value.round()))
}

/// The ring's label: the reading, with why a part of it is missing where one is.
pub fn title(s: Option<&Snapshot>, missing: Option<&str>) -> String {
    match s {
        None => format!("Context: {}", missing.unwrap_or(NOT_YET)),
        Some(s) => match (s.max, percent(s)) {
            (Some(max), Some(percent)) => format!("Context: {percent} used, {} of {} tokens", fmt_tokens(s.used), fmt_tokens(max)),
            _ => format!("Context: {} tokens. {}", fmt_tokens(s.used), missing.unwrap_or(LIMIT_AT_END)),
        },
    }
}

/// What the ring draws, `ContextReading`: none on a thread that never ran a turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub snapshot: Option<Snapshot>,
    pub missing: Option<String>,
    /// The agent's own compaction where it has one and there is a reading: why it cannot run now, or none.
    pub compact: Option<Option<String>>,
}

pub fn reading(turns: &[TurnContext], agent: &str, compacts: bool) -> Option<Reading> {
    let snapshot = snapshot(turns);
    let missing = missing(turns, agent);
    if snapshot.is_none() && missing.is_none() {
        return None;
    }
    let busy = turns.last().is_some_and(|t| t.running);
    let compact = (compacts && snapshot.is_some()).then(|| busy.then(|| TURN_IN_FLIGHT.to_owned()));
    Some(Reading { snapshot, missing, compact })
}

// What the cluster keeps for one thread.

#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub harness: String,
    pub label: String,
    /// The message the agent's adapter declares for its own compaction.
    pub compacts: Option<String>,
}

/// The host's label for the agent once `harnesses.list` answers, the catalog's name until then.
pub fn agent_label(catalogs: &[Catalog], harness: &str) -> String {
    catalogs.iter().find(|c| c.harness == harness).map_or_else(|| crate::ui::catalog::name(harness), |c| c.label.clone())
}

/// The cluster's state for one thread: what the host said, and which popup is up.
#[derive(Default)]
pub struct Bar {
    asked: bool,
    /// The workspace the thread was read in, whose agents' catalogs it shows.
    workspace: Option<String>,
    /// The thread's workspace's status row (`WorkspaceStatus`), as the host sent it.
    pub status: Option<Value>,
    /// The thread's newest session row (`SessionView`), as the host listed or pushed it.
    pub row: Option<Value>,
    /// Each project's base branch by project id.
    pub bases: HashMap<String, String>,
    pub catalogs: Vec<Catalog>,
    ring_hover: bool,
    card_hover: bool,
    closing: Option<Task<()>>,
    turns: Option<(usize, std::rc::Rc<Vec<TurnContext>>, String)>,
    renaming: Option<Renaming>,
}

impl Bar {
    pub fn ring_open(&self) -> bool {
        self.ring_hover || self.card_hover || self.closing.is_some()
    }

    /// The name in the open rename box, none while no box is open.
    pub fn rename_box(&self, cx: &App) -> Option<String> {
        self.renaming.as_ref().map(|r| r.input.read(cx).value().to_string())
    }
}

/// The title's name box while it is open.
struct Renaming {
    input: Entity<InputState>,
    /// The name is on its way: the box stays as it is and takes no second Enter.
    saving: bool,
    _events: Subscription,
}

#[derive(Default)]
struct Bars(HashMap<String, Entity<Bar>>);

impl Global for Bars {}

/// The cluster's state for `thread`; opening another thread's lets the last one's go.
pub fn bar(thread: &str, cx: &mut App) -> Entity<Bar> {
    if let Some(bar) = cx.default_global::<Bars>().0.get(thread) {
        return bar.clone();
    }
    let bar = cx.new(|_| Bar::default());
    let bars = cx.default_global::<Bars>();
    bars.0.clear();
    bars.0.insert(thread.to_owned(), bar.clone());
    bar
}

/// The project `thread`'s crumbs name, by its id where the row carries one and its name, while its cluster stands.
pub fn project_of(thread: &str, cx: &App) -> Option<(Option<String>, String)> {
    let bar = cx.try_global::<Bars>()?.0.get(thread)?;
    let crumb = crumb(bar.read(cx).row.as_ref()?);
    Some((crumb.project_id, crumb.project?))
}

/// Names `thread` in the crumbs with what the sidebar lists of it until the host's row lands, so a switch never draws
/// the title row blank.
pub fn seed(thread: &str, project: &str, title: &str, cx: &mut App) {
    let bar = bar(thread, cx);
    if bar.read(cx).row.is_none() && !title.is_empty() {
        let row = json!({ "threadId": thread, "harnessTitle": title, "project": { "name": project } });
        set(&bar, cx, |b| b.row = Some(row));
    }
}

fn set(bar: &Entity<Bar>, cx: &mut App, f: impl FnOnce(&mut Bar)) {
    bar.update(cx, |b, cx| {
        f(b);
        cx.notify();
    });
}

fn request<O: serde::de::DeserializeOwned>(value: Value) -> Option<O> {
    serde_json::from_value(value).ok()
}

/// Sends `op` and hands its answer, read into JSON by `read`, to `done`, or the failure's words.
fn ask<O>(
    view: &mut ThreadView,
    op: Option<O>,
    read: impl FnOnce(O::Reply) -> Value + 'static,
    cx: &mut Context<ThreadView>,
    done: impl FnOnce(&mut ThreadView, Result<Value, String>, &mut Context<ThreadView>) + 'static,
) where
    O: Op + Send + 'static,
    O::Reply: Send + 'static,
{
    let Some(op) = op else {
        done(view, Err(format!("{} could not be asked", O::NAME)), cx);
        return;
    };
    view.call(op, cx, move |view, answer, cx| done(view, answer.map(read).map_err(|f| f.message), cx));
}

/// Sends `op` on the host `view` follows and hands its answer to `done` on the app, whether or not the view still
/// stands by then: what it answers is kept for every thread, and a read left unanswered would never be asked again.
fn ask_host<O>(
    view: &ThreadView,
    op: Option<O>,
    read: impl FnOnce(O::Reply) -> Value + Send + 'static,
    cx: &mut App,
    done: impl FnOnce(Result<Value, String>, &mut App) + 'static,
) where
    O: Op + Send + 'static,
    O::Reply: Send + 'static,
{
    let (Some(host), Some(op)) = (view.host().cloned(), op) else {
        done(Err(format!("{} could not be asked", O::NAME)), cx);
        return;
    };
    let answer = host.run(move |client| async move { client.call(op).await.map(read) });
    cx.spawn(async move |cx| {
        let answer = answer.await.map_err(|f| f.message);
        cx.update(|cx| done(answer, cx));
    })
    .detach();
}

/// The projects' bases and each workspace's agents' catalogs as the host listed them, read once per host and kept
/// across switches, as the editors are; a refused read is asked again by the next thread to open.
#[derive(Default)]
struct Hosted {
    bases: Option<HashMap<String, String>>,
    bases_asked: bool,
    /// Each workspace's catalogs as `harnesses.list` answered them, which the cluster and the view's facts both read.
    catalogs: HashMap<String, Rc<Vec<Value>>>,
    catalogs_asked: HashSet<String>,
    /// The views waiting on a workspace's catalogs for their thread's facts.
    waiting: HashMap<String, Vec<WeakEntity<ThreadView>>>,
    /// The workspaces whose checkout was asked for as their thread came into view.
    checkouts_asked: HashSet<String>,
    reads: usize,
}

impl Global for Hosted {}

/// What the cluster reads off a workspace's catalogs: each agent's label and compaction message.
fn labels(catalogs: &[Value]) -> Vec<Catalog> {
    let catalog = |c: &Value| {
        Some(Catalog {
            harness: c.get("harness")?.as_str()?.to_owned(),
            label: c.get("label")?.as_str()?.to_owned(),
            compacts: c.get("compacts").and_then(Value::as_str).map(str::to_owned),
        })
    };
    catalogs.iter().filter_map(catalog).collect()
}

/// `workspace`'s agents' catalogs as the host listed them, once read: the sidebar's read where the window has one,
/// which a whole read of the host empties, else the header's own.
pub fn catalogs_of(workspace: &str, cx: &App) -> Option<Rc<Vec<Value>>> {
    match crate::screens::terminal::workspaces_sidebar(cx) {
        Some(sidebar) => sidebar.read(cx).listed_catalogs(workspace),
        None => cx.try_global::<Hosted>()?.catalogs.get(workspace).cloned(),
    }
}

/// Lists `workspace`'s agents the first time the cluster or a view's facts want them: through the sidebar where the
/// window has one, else once per host from the header; `waiter`, if any, hears when they land. The read goes on
/// whether or not the view that asked stands, and a refused read is asked again by the next to want them.
pub(crate) fn want_catalogs(view: &ThreadView, workspace: &str, waiter: Option<WeakEntity<ThreadView>>, cx: &mut App) {
    if catalogs_of(workspace, cx).is_some() {
        return;
    }
    let hosted = cx.default_global::<Hosted>();
    if let Some(waiter) = waiter {
        let waiting = hosted.waiting.entry(workspace.to_owned()).or_default();
        if !waiting.iter().any(|w| w.entity_id() == waiter.entity_id()) {
            waiting.push(waiter);
        }
    }
    if !hosted.catalogs_asked.insert(workspace.to_owned()) {
        return;
    }
    let asked = workspace.to_owned();
    if let Some(sidebar) = crate::screens::terminal::workspaces_sidebar(cx) {
        let landed = asked.clone();
        sidebar.update(cx, |s, cx| s.want_catalogs(&asked, move |ok, cx| catalogs_landed(&landed, ok, cx), cx));
        return;
    }
    cx.default_global::<Hosted>().reads += 1;
    let op = request::<HarnessesListRequest>(json!({ "workspaceId": workspace }));
    ask_host(
        view,
        op,
        |r| json!(r.harnesses),
        cx,
        move |answer, cx| {
            let listed = answer.ok().and_then(|a| a.as_array().cloned());
            if let Some(listed) = listed.clone() {
                cx.default_global::<Hosted>().catalogs.insert(asked.clone(), Rc::new(listed));
            }
            catalogs_landed(&asked, listed.is_some(), cx);
        },
    );
}

/// Asks the host for the workspace's checkout once, as today's app does for a workspace whose tile comes into view:
/// the host reads git again where what it holds is past its ten seconds, so the split stands on what the folder holds
/// now and not on a read from before a turn's writes. A refusal or a reply with no checkout leaves it to be asked again.
fn ask_checkout(view: &ThreadView, workspace: &str, cx: &mut App) {
    if !cx.default_global::<Hosted>().checkouts_asked.insert(workspace.to_owned()) {
        return;
    }
    let asked = workspace.to_owned();
    let op = request::<WorkspacesCheckoutRequest>(json!({ "workspaceId": workspace }));
    ask_host(
        view,
        op,
        |r| json!({ "checkout": r.checkout }),
        cx,
        move |answer, cx| match answer.ok().and_then(|a| a.get("checkout").filter(|c| c.is_object()).cloned()) {
            Some(checkout) => take_checkout(&asked, &checkout, cx),
            None => _ = cx.default_global::<Hosted>().checkouts_asked.remove(&asked),
        },
    );
}

/// Stands `checkout` on the workspace's status where it is newer than the one the status holds.
fn take_checkout(workspace: &str, checkout: &Value, cx: &mut App) {
    let read_at = |c: Option<&Value>| c.and_then(|c| c.get("readAt")).and_then(Value::as_f64).unwrap_or(0.);
    let newer = |status: &Value| read_at(Some(checkout)) >= read_at(status.get("checkout"));
    if let Some(status) = cx.default_global::<Statuses>().0.get_mut(workspace).filter(|s| newer(s)) {
        status["checkout"] = checkout.clone();
    }
    for bar in bars_of(Some(workspace), cx) {
        let checkout = checkout.clone();
        set(&bar, cx, |b| {
            if let Some(status) = b.status.as_mut().filter(|s| newer(s)) {
                status["checkout"] = checkout;
            }
        });
    }
}

/// A workspace's agents were read, or refused: the cluster takes their labels and each view waiting takes its facts.
fn catalogs_landed(workspace: &str, landed: bool, cx: &mut App) {
    let hosted = cx.default_global::<Hosted>();
    hosted.catalogs_asked.remove(workspace);
    let waiting = hosted.waiting.remove(workspace).unwrap_or_default();
    if let Some(catalogs) = catalogs_of(workspace, cx).filter(|_| landed) {
        for bar in bars_of(Some(workspace), cx) {
            let labels = labels(&catalogs);
            set(&bar, cx, |b| b.catalogs = labels);
        }
    }
    for waiter in waiting {
        _ = waiter.update(cx, |v, cx| v.catalogs_landed(landed, cx));
    }
}

/// How many times the header has asked the host for the projects or a workspace's agents.
pub fn reads(cx: &App) -> usize {
    cx.try_global::<Hosted>().map_or(0, |h| h.reads)
}

/// Every open bar of `workspace`, or every open bar.
fn bars_of(workspace: Option<&str>, cx: &mut App) -> Vec<Entity<Bar>> {
    let bars: Vec<Entity<Bar>> = cx.default_global::<Bars>().0.values().cloned().collect();
    bars.into_iter().filter(|b| workspace.is_none_or(|w| b.read(cx).workspace.as_deref() == Some(w))).collect()
}

/// Gives the thread the projects' bases and its workspace's catalogs as the host listed them, reading each the first
/// time; the editors and the pick once per host.
fn load(view: &mut ThreadView, bar: &Entity<Bar>, cx: &mut Context<ThreadView>) {
    let Some(workspace) = view.followed_workspace().map(str::to_owned) else { return };
    if bar.read(cx).asked {
        return;
    }
    let known = cx.try_global::<Statuses>().and_then(|s| s.0.get(&workspace).cloned());
    let catalogs = catalogs_of(&workspace, cx).map(|c| labels(&c));
    let hosted = cx.default_global::<Hosted>();
    let bases = hosted.bases.clone();
    let ask_bases = !std::mem::replace(&mut hosted.bases_asked, true);
    hosted.reads += usize::from(ask_bases);
    set(bar, cx, |b| {
        b.asked = true;
        b.workspace = Some(workspace.clone());
        b.status = b.status.take().or(known);
        b.bases = bases.unwrap_or_default();
        b.catalogs = catalogs.unwrap_or_default();
    });
    if ask_bases {
        ask_host(
            view,
            request::<ProjectsListRequest>(json!({})),
            |r| json!({ "projects": r.projects }),
            cx,
            move |answer, cx| {
                let Ok(answer) = answer else {
                    cx.default_global::<Hosted>().bases_asked = false;
                    return;
                };
                let bases = answer["projects"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| Some((p.get("id")?.as_str()?.to_owned(), p.get("base")?.as_str()?.to_owned())));
                let bases: HashMap<String, String> = bases.collect();
                cx.default_global::<Hosted>().bases = Some(bases.clone());
                for bar in bars_of(None, cx) {
                    let bases = bases.clone();
                    set(&bar, cx, |b| b.bases = bases);
                }
            },
        );
    }
    want_catalogs(view, &workspace, None, cx);
    ask_checkout(view, &workspace, cx);
    if let Some(host) = view.host() {
        open_split::want_editors(host, cx);
    }
}

/// Every workspace's status as the host last pushed it, which a thread opened later starts from.
#[derive(Default)]
struct Statuses(HashMap<String, Value>);

impl Global for Statuses {}

/// What the host pushed that the header reads, by its type: a workspace's status, a session's row, the preferences.
pub fn frame_of(event: &Event) -> Option<(String, Value)> {
    let frame = match event {
        Event::Host(e) => serde_json::to_value(e).ok()?,
        Event::Other(v) => v.clone(),
        _ => return None,
    };
    let kind = frame.get("type")?.as_str()?;
    matches!(kind, "workspace.status" | "session.row" | "preferences.changed").then(|| (kind.to_owned(), frame.clone()))
}

/// A frame [`frame_of`] kept: the open thread's workspace's status and its newest row follow the host, and the pick
/// follows the person's preferences.
pub fn heard(view: &mut ThreadView, kind: &str, frame: &Value, cx: &mut Context<ThreadView>) {
    match kind {
        "workspace.status" => {
            let Some(status) = frame.get("status").filter(|s| s.is_object()) else { return };
            let Some(id) = status.get("id").and_then(Value::as_str) else { return };
            cx.default_global::<Statuses>().0.insert(id.to_owned(), status.clone());
            if let (Some(thread), Some(workspace)) = (view.thread().map(str::to_owned), view.followed_workspace()) {
                if workspace == id {
                    let status = status.clone();
                    set(&bar(&thread, cx), cx, |b| b.status = Some(status));
                }
            }
        }
        "session.row" => {
            let (Some(thread), Some(row)) = (view.thread().map(str::to_owned), frame.get("row").filter(|r| r.is_object())) else { return };
            if row_thread(row) != Some(thread.as_str()) {
                return;
            }
            let bar = bar(&thread, cx);
            if newer(bar.read(cx).row.as_ref(), row) {
                let mut row = row.clone();
                // A push that leaves out the opening prompt keeps the one the thread's row holds (SessionRowEvent).
                if row.get("prompt").and_then(Value::as_str).is_none() {
                    if let Some(prompt) = bar.read(cx).row.as_ref().and_then(|kept| kept.get("prompt")).cloned() {
                        row["prompt"] = prompt;
                    }
                }
                set(&bar, cx, |b| b.row = Some(row));
            }
        }
        "preferences.changed" => {
            let picked = frame.pointer("/preferences/editor").and_then(Value::as_str).map(str::to_owned);
            if picked.is_some() {
                open_split::set_editors(None, picked, cx);
            }
        }
        _ => {}
    }
}

/// The thread a session row belongs to: its thread, or itself for a session from before threads.
fn row_thread(row: &Value) -> Option<&str> {
    row.get("threadId").or(row.get("id")).and_then(Value::as_str)
}

/// Whether `row` stands for the thread over `kept`: the same session again, or a newer turn's.
fn newer(kept: Option<&Value>, row: &Value) -> bool {
    let Some(kept) = kept else { return true };
    let started = |r: &Value| r.get("startedAt").and_then(Value::as_f64).unwrap_or(0.);
    kept.get("id") == row.get("id") || started(row) >= started(kept)
}

/// Reads the open thread's newest row again, as it opens and once the host answers after a lost socket.
pub fn refresh(view: &mut ThreadView, cx: &mut Context<ThreadView>) {
    let (Some(thread), Some(workspace)) = (view.thread().map(str::to_owned), view.followed_workspace().map(str::to_owned)) else {
        return;
    };
    let b = bar(&thread, cx);
    // The sidebar holds every row the host listed and pushed; the host is asked only for a thread it has no row of yet.
    let listed =
        crate::screens::terminal::workspaces_sidebar(cx).and_then(|sidebar| {
            let rows = sidebar
                .read(cx)
                .rows()
                .iter()
                .filter(|r| r.thread_id.as_deref() == Some(thread.as_str()))
                .map(|r| std::rc::Rc::new(r.as_sent()));
            rows.filter(|r| row_thread(r) == Some(thread.as_str())).fold(None, |newest: Option<std::rc::Rc<Value>>, row| {
                if newer(newest.as_deref(), &row) {
                    Some(row)
                } else {
                    newest
                }
            })
        });
    if let Some(row) = listed {
        let row = (*row).clone();
        if newer(b.read(cx).row.as_ref(), &row) {
            set(&b, cx, |b| b.row = Some(row));
        }
        return;
    }
    ask(
        view,
        request::<SessionsListRequest>(json!({ "workspaceId": workspace })),
        |r| json!({ "sessions": r.sessions }),
        cx,
        move |_, answer, cx| {
            let Ok(answer) = answer else { return };
            let rows = answer["sessions"].as_array().cloned().unwrap_or_default();
            let mut newest: Option<Value> = None;
            for row in rows.into_iter().filter(|r| row_thread(r) == Some(thread.as_str())) {
                if newer(newest.as_ref(), &row) {
                    newest = Some(row);
                }
            }
            if newest.is_some() {
                set(&b, cx, |b| b.row = newest);
            }
        },
    );
}

// Drawing.

/// Draws a piece of the header again whenever the open thread's cluster changes, which leaves the transcript as drawn.
#[derive(Default)]
pub struct BarWatch(Option<(gpui_kit::EntityId, Subscription)>);

impl BarWatch {
    /// Watches the cluster of the thread `view` shows now, once per thread.
    pub fn follow<V: 'static>(&mut self, view: &Entity<ThreadView>, cx: &mut Context<V>) {
        let Some(thread) = view.read(cx).thread().map(str::to_owned) else {
            self.0 = None;
            return;
        };
        let bar = bar(&thread, cx);
        if self.0.as_ref().is_some_and(|(id, _)| *id == bar.entity_id()) {
            return;
        }
        self.0 = Some((bar.entity_id(), cx.observe(&bar, |_, _, cx| cx.notify())));
    }
}

/// The cluster: the ring, the git split and Open, 8 px apart, at the header's right. The splits are the panes' own,
/// one each per header: told the thread on a switch and fed the status the header follows, so they follow nothing.
pub fn actions(
    view: &mut ThreadView,
    git: &Entity<GitSplit>,
    open: &Entity<OpenSplit>,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> impl IntoElement {
    let Some(thread) = view.thread().map(str::to_owned) else { return div().into_any_element() };
    let bar = bar(&thread, cx);
    load(view, &bar, cx);
    let seen = view.events_seen();
    let (turns, harness) = match &bar.read(cx).turns {
        Some((at, turns, harness)) if *at == seen => (turns.clone(), harness.clone()),
        _ => {
            let turns = std::rc::Rc::new(view.transcript().turn_contexts());
            let harness = view.transcript().harness().unwrap_or_else(|| "claude".to_owned());
            bar.update(cx, |b, _| b.turns = Some((seen, turns.clone(), harness.clone())));
            (turns, harness)
        }
    };
    let workspace = view.followed_workspace().unwrap_or_default().to_owned();
    if git.read(cx).shows() != (workspace.as_str(), Some(thread.as_str())) {
        git.update(cx, |g, cx| g.show(workspace.clone(), Some(thread.clone()), cx));
        open.update(cx, |o, cx| o.show(workspace.clone(), cx));
    }
    feed(&bar, git, open, cx);
    let (reading, compacts) = {
        let b = bar.read(cx);
        let catalog = b.catalogs.iter().find(|c| c.harness == harness);
        let agent = agent_label(&b.catalogs, &harness);
        let compacts = catalog.and_then(|c| c.compacts.clone());
        (reading(&turns, &agent, compacts.is_some()), compacts)
    };
    let me = cx.entity().downgrade();
    h_flex()
        .debug_selector(|| "header-actions".into())
        .flex_none()
        .ml_auto()
        .mr(px(1.))
        .gap(px(8.))
        .children(reading.map(|r| ring(&r, &bar, compacts, me.clone(), window, cx)))
        .child(git.clone())
        .child(open.clone())
        .into_any_element()
}

/// Hands the splits the thread's workspace status, its project's base and the computers the sidebar holds, where
/// they moved; compared in place, since the cluster draws on every frame of a scroll.
fn feed(bar: &Entity<Bar>, git: &Entity<GitSplit>, open: &Entity<OpenSplit>, cx: &mut App) {
    let none = Vec::new();
    let sidebar = crate::screens::terminal::workspaces_sidebar(cx);
    let held = {
        let b = bar.read(cx);
        let places = sidebar.as_ref().map_or(&none, |s| &s.read(cx).around().places);
        git.read(cx).holds(b.status.as_ref(), base_of(b), places)
    };
    if held {
        return;
    }
    let (status, base) = {
        let b = bar.read(cx);
        (b.status.clone(), base_of(b).map(str::to_owned))
    };
    let places = sidebar.map(|s| s.read(cx).around().places.clone()).unwrap_or_default();
    git.update(cx, |g, cx| g.feed(status.as_ref(), base.as_deref(), &places, cx));
    open.update(cx, |o, cx| o.take_status(status.as_ref(), cx));
}

/// The branch the thread's work goes back to: its project's base, where the host listed one.
fn base_of(b: &Bar) -> Option<&str> {
    let project = b.status.as_ref()?.pointer("/project/id")?.as_str()?;
    b.bases.get(project).map(String::as_str)
}

/// `KEYCAP_BEVEL`: the 1 px light inside the top edge.
fn bevel(p: &Palette) -> Vec<BoxShadow> {
    vec![BoxShadow { color: p.keycap_top, offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }]
}

/// The outline keycap (`OUTLINE_SURFACE`); held is `disabled:opacity-64`.
fn keycap(id: &'static str, held: bool, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let radius = px(8.);
    let surface = div()
        .rounded(radius)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(p.input)
        .bg(p.input_fill)
        .whitespace_nowrap()
        .text_color(p.foreground)
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(13.))
        .line_height(px(19.5))
        .child(div().rounded(radius - px(1.)).absolute().inset_0().shadow(bevel(p)));
    let hover = p.input_fill_hover;
    surface.id(id).debug_selector(move || id.into()).map(|el| {
        if held {
            el.opacity(0.64)
        } else {
            el.cursor_pointer().hover(move |s| s.bg(hover))
        }
    })
}

fn glyph(icon: Glyph, size: Pixels, p: &Palette) -> Icon {
    Icon::new(icon).with_size(size).text_color(p.muted_foreground)
}

/// The 24 unit ring at 20 px as a round quad and a glyph: a painted path makes the renderer hold window-sized textures.
fn ring_canvas(share: f64, ink: Hsla) -> impl IntoElement {
    let scale = 20. / 24.;
    let outer = px((9.75 + 1.5) * 2. * scale);
    let percent = (share.clamp(0., 1.) * 100.).round() as u32;
    div()
        .relative()
        .flex_none()
        .size(px(20.))
        .child(
            div()
                .absolute()
                .top((px(20.) - outer) / 2.)
                .left((px(20.) - outer) / 2.)
                .size(outer)
                .rounded_full()
                .border(px(3. * scale))
                .border_color(ink.opacity(0.1)),
        )
        .when(percent > 0, |el| {
            el.child(svg().absolute().inset_0().size(px(20.)).path(crate::ui::icons::ring_arc(percent)).text_color(ink.opacity(0.55)))
        })
}

fn hover_ring(bar: &Entity<Bar>, on_ring: bool, hovered: bool, delay: bool, cx: &mut App) {
    let shut = !hovered && delay;
    bar.update(cx, |b, cx| {
        if on_ring {
            b.ring_hover = hovered;
        } else {
            b.card_hover = hovered;
        }
        b.closing = None;
        if shut {
            b.closing = Some(cx.spawn(async move |b, cx| {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                _ = b.update(cx, |b, cx| {
                    b.closing = None;
                    cx.notify();
                });
            }));
        }
        cx.notify();
    });
}

/// ContextRing: the 28 px round button with its 20 px ring, and on hover the card under it, right edges together.
fn ring(
    reading: &Reading,
    bar: &Entity<Bar>,
    compacts: Option<String>,
    view: WeakEntity<ThreadView>,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let p = cx.palette().clone();
    let open = bar.read(cx).ring_open();
    let share = reading.snapshot.and_then(|s| s.share).unwrap_or(0.);
    let delay = reading.compact.is_some();
    let (on_ring, on_card) = (bar.clone(), bar.clone());
    let accent = p.accent;
    let button = div()
        .id("context-ring")
        .debug_selector(|| "context-ring".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded_full()
        .text_color(p.muted_foreground)
        .hover(move |s| s.bg(accent))
        .when(open, |el| el.bg(accent))
        .on_hover(move |hovered, _, cx| hover_ring(&on_ring, true, *hovered, delay, cx))
        .child(ring_canvas(share, p.foreground));
    let card = open.then(|| {
        div().absolute().right_0().top(px(28. + 6.)).child(
            deferred(
                anchored().anchor(Anchor::TopRight).snap_to_window().child(
                    context_card(reading, compacts, view, &p, window, cx)
                        .on_hover(move |hovered, _, cx| hover_ring(&on_card, false, *hovered, delay, cx)),
                ),
            )
            .with_priority(1),
        )
    });
    div().relative().flex_none().child(button).children(card).into_any_element()
}

/// The card in the tooltip's skin: 288 px, 13 px, the head and figures, the meter or why it is missing, and Compact.
fn context_card(
    reading: &Reading,
    compacts: Option<String>,
    view: WeakEntity<ThreadView>,
    p: &Palette,
    _: &mut Window,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let mono = cx.theme().mono_font_family.clone();
    let figures = reading.snapshot.map(|s| {
        h_flex()
            .debug_selector(|| "context-figures".into())
            .flex_none()
            .gap(px(12.))
            .font_family(mono)
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.muted_foreground)
            .children(percent(&s))
            .child(figure(&s))
    });
    let measured = reading.snapshot.and_then(|s| s.share);
    let below = match measured {
        Some(share) => div()
            .debug_selector(|| "context-meter".into())
            .h(px(4.))
            .w_full()
            .rounded_full()
            .overflow_hidden()
            .bg(p.foreground.opacity(0.1))
            .child(
                div()
                    .debug_selector(|| "context-meter-fill".into())
                    .h_full()
                    .w(gpui_kit::relative(share as f32))
                    .rounded_full()
                    .bg(p.foreground.opacity(0.55)),
            )
            .into_any_element(),
        None => div()
            .debug_selector(|| "context-missing".into())
            .text_color(p.muted_foreground)
            .child(reading.missing.clone().unwrap_or_default())
            .into_any_element(),
    };
    let compact = reading.compact.clone().map(|held| {
        let shown = held.is_some();
        let button = keycap("context-compact", shown, p)
            .mt(px(4.))
            .w_full()
            .h(px(24.))
            .px(px(8.))
            .gap(px(4.))
            .text_size(px(12.))
            .line_height(px(16.))
            .child(glyph(Glyph::Minimize2, px(14.), p))
            .child(COMPACT)
            .when(!shown, |el| {
                el.on_click(move |_, _, cx| {
                    let Some(prompt) = compacts.clone() else { return };
                    _ = view.update(cx, |view, cx| view.send(prompt, cx));
                })
            });
        v_flex().gap(px(8.)).child(button).children(held.map(|h| {
            div()
                .debug_selector(|| "context-compact-held".into())
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.muted_foreground)
                .child(h)
        }))
    });
    div()
        .id("context-card")
        .debug_selector(|| "context-card".into())
        .occlude()
        .w(px(288.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .bg(p.popover)
        .shadow(vec![
            BoxShadow {
                color: gpui_kit::black().opacity(0.05),
                offset: point(px(0.), px(4.)),
                blur_radius: px(6.),
                spread_radius: px(-1.),
                inset: false,
            },
            BoxShadow {
                color: gpui_kit::black().opacity(0.05),
                offset: point(px(0.), px(2.)),
                blur_radius: px(4.),
                spread_radius: px(-2.),
                inset: false,
            },
        ])
        .text_size(px(13.))
        .line_height(px(19.5))
        .child(
            v_flex()
                .min_w_0()
                .gap(px(8.))
                .py(px(4.))
                .child(
                    h_flex()
                        .items_baseline()
                        .justify_between()
                        .gap(px(12.))
                        .child(div().whitespace_nowrap().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(CONTEXT_HEAD))
                        .children(figures),
                )
                .child(below)
                .children(compact),
        )
}

// The crumbs, ThreadBreadcrumb.tsx.

pub const NEW_THREAD: &str = "New thread";
/// THREAD_WORDS.rename.
pub const RENAME: &str = "Rename thread";
/// A text field's `size`: the width the name box asks for, in characters of its own face.
const NAME_BOX_CHARS: f32 = 20.;

/// What the crumbs read off the thread's newest row: its project, its title, and what it waits on where it waits.
#[derive(Clone, Debug, PartialEq)]
pub struct Crumb {
    pub project: Option<String>,
    pub project_id: Option<String>,
    pub title: String,
    pub waiting: Option<String>,
}

pub fn crumb(row: &Value) -> Crumb {
    let text = |key: &str| row.get(key).and_then(Value::as_str);
    let title = thread_title(text("harnessTitle"), text("prompt")).unwrap_or_else(|| NEW_THREAD.to_owned());
    let project = row.pointer("/project/name").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_owned);
    let project_id = row.pointer("/project/id").and_then(Value::as_str).map(str::to_owned);
    Crumb { project, project_id, title, waiting: waiting_line(row) }
}

/// The thread that opened another, where an agent did (`openedBy`): its workspace, its id where the runtime stamped
/// one, and its title.
#[derive(Clone, Debug, PartialEq)]
pub struct Opener {
    pub workspace_id: String,
    pub thread_id: Option<String>,
    pub title: String,
}

/// The thread that opened `thread`, as the sidebar lists both.
pub fn opener_of(thread: &str, cx: &App) -> Option<Opener> {
    let sidebar = crate::screens::terminal::workspaces_sidebar(cx)?;
    let threads = sidebar.read(cx).threads();
    let parent = threads.iter().find(|t| t.thread_id.as_deref() == Some(thread))?.parent.as_deref()?;
    let opener = threads.iter().find(|t| t.thread_id.as_deref() == Some(parent) || t.key == parent)?;
    Some(Opener { workspace_id: opener.workspace_id.clone(), thread_id: opener.thread_id.clone(), title: opener.title.clone() })
}

/// `ProjectCrumb`: the project's glyph in its look and its name, and the slash after it; under 640 px the glyph alone,
/// its name on its hover, and no slash.
pub fn project_crumb(project_id: Option<&str>, name: String, window: &Window, cx: &App) -> AnyElement {
    let p = cx.palette();
    let narrow = crate::ui::narrow(window);
    let sidebar = crate::screens::terminal::workspaces_sidebar(cx);
    let glyph = match &sidebar {
        Some(s) => {
            let around = s.read(cx).around();
            let prefs = around.preferences.as_deref().unwrap_or(&Value::Null);
            crate::screens::sidebar::looks::project_glyph(project_id, prefs, &around.images, px(16.), p.muted_foreground, true, p)
        }
        None => Icon::new(Glyph::Folder).with_size(px(16.)).text_color(p.muted_foreground).into_any_element(),
    };
    let crumb = h_flex()
        .id("crumbs-project")
        .debug_selector(|| "crumbs-project".into())
        .min_w_0()
        .items_center()
        .gap_2()
        .text_color(p.muted_foreground)
        .child(div().flex_none().child(glyph));
    let crumb = if narrow {
        crumb.flex_none().tooltip(tip(name)).into_any_element()
    } else {
        crumb.child(div().truncate().child(SharedString::from(name))).into_any_element()
    };
    h_flex().min_w_0().gap_2().items_center().child(crumb).when(!narrow, |el| el.child(crate::ui::slash(cx))).into_any_element()
}

/// `ThreadLink`: the thread named as the way to it, in the muted ink, the foreground and an underline under the
/// pointer; a press selects it. One the runtime stamped no id on is its name in plain text.
pub fn thread_link(id: &'static str, opener: Opener, cx: &App) -> AnyElement {
    let p = cx.palette();
    let link = div().id(id).debug_selector(move || id.into()).min_w_0().truncate().text_color(p.muted_foreground);
    let Some(thread) = opener.thread_id else { return link.child(SharedString::from(opener.title)).into_any_element() };
    let (ink, workspace) = (p.foreground, opener.workspace_id);
    link.cursor_pointer()
        .hover(move |s| s.text_color(ink).underline())
        .on_click(move |_, _, cx| {
            crate::screens::navigate(crate::screens::Route::Thread { workspace_id: workspace.clone(), thread_id: thread.clone() }, cx)
        })
        .child(SharedString::from(opener.title))
        .into_any_element()
}

/// `waitingLine` for a thread `threadState` reads as waiting: its own question, else the one the thread it is behind holds.
pub fn waiting_line(row: &Value) -> Option<String> {
    if let Some(asking) = row.get("asking").and_then(Value::as_str) {
        return Some(asking.to_owned());
    }
    let prompt = row.pointer("/waitingOn/prompt").filter(|p| p.is_object())?;
    let text = |key: &str| prompt.get(key).and_then(Value::as_str);
    let line = crate::transcript::asking_line(text("toolName").unwrap_or_default(), text("input").unwrap_or_default(), text("detail"));
    Some(format!("{line} needs an answer"))
}

/// Opens the title's menu at the pointer.
pub type OnMenu = Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App)>;

/// The crumbs of an open thread: its project and a slash, the thread that opened it and a slash where an agent did,
/// its title or the name box in the title's place, and the question glyph while it waits on the person. `on_menu`
/// opens the title's menu at the pointer.
pub fn crumbs(view: &mut ThreadView, on_menu: OnMenu, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    let line = crumb_line("crumbs");
    let Some(thread) = view.thread().map(str::to_owned) else { return line.into_any_element() };
    let bar = bar(&thread, cx);
    let Some(crumb) = bar.read(cx).row.as_ref().map(crumb) else { return line.into_any_element() };
    let project = crumb.project.clone().map(|name| project_crumb(crumb.project_id.as_deref(), name, window, cx));
    let opener =
        opener_of(&thread, cx).map(|o| h_flex().min_w_0().gap_2().child(thread_link("crumbs-opener", o, cx)).child(crate::ui::slash(cx)));
    let own = project.is_some() || opener.is_some();
    let me = cx.entity().downgrade();
    let renaming = bar.read(cx).renaming.as_ref().map(|r| (r.input.clone(), r.saving));
    let title = match renaming {
        Some((input, saving)) => name_box(&input, saving, &bar, &p, window, cx),
        None => div()
            .id("crumbs-title")
            .debug_selector(|| "crumbs-title".into())
            .truncate()
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.foreground)
            .on_click(move |e, window, cx| {
                if e.click_count() == 2 {
                    _ = me.update(cx, |view, cx| open_rename(view, window, cx));
                }
            })
            .on_mouse_down(MouseButton::Right, move |e: &MouseDownEvent, window, cx| on_menu(e.position, window, cx))
            .child(SharedString::from(crumb.title.clone()))
            .into_any_element(),
    };
    let waiting = crumb.waiting.clone().map(|line| {
        div()
            .id("crumbs-waiting")
            .debug_selector(|| "crumbs-waiting".into())
            .flex()
            .flex_none()
            .items_center()
            .child(Icon::new(Glyph::MessageCircleQuestionMark).with_size(px(14.)).text_color(p.status_input))
            .tooltip(tip(line))
            .tooltip_side(gpui_kit::TooltipSide::Bottom)
    });
    line.children(project)
        .children(opener)
        .child(div().flex().min_w_0().when(own, |el| el.flex_none().max_w(gpui_kit::relative(TITLE_SHARE))).child(title))
        .children(waiting)
        .into_any_element()
}

/// The name box in the title's place, 24 px, as wide as a text field asks; Escape leaves the name as it was.
fn name_box(
    input: &Entity<InputState>,
    saving: bool,
    bar: &Entity<Bar>,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let family = cx.theme().font_family.clone();
    let asks = crate::ui::text::width("0", 14., family, window) * NAME_BOX_CHARS + px(10.);
    let shut = bar.clone();
    // `rounded-sm border border-input px-1`, the ring's edge while it has the keys.
    div()
        .debug_selector(|| "crumbs-rename".into())
        .flex()
        .items_center()
        .h(px(24.))
        .w(asks)
        .min_w_0()
        .rounded(px(6.))
        .border_1()
        .border_color(p.ring)
        .px(px(4.))
        .font_weight(FontWeight::MEDIUM)
        .capture_key_down(move |e: &KeyDownEvent, _, cx| {
            if e.keystroke.key == "escape" {
                if !saving {
                    close_rename(&shut, cx);
                }
                cx.stop_propagation();
            }
        })
        .child(Input::new(input).appearance(false).p_0().w_full().h(px(20.)).text_size(px(14.)).line_height(px(20.)))
        .into_any_element()
}

/// Opens the name box on the title, as a double-click on it or its menu's Rename does, the name selected.
pub fn open_rename(view: &mut ThreadView, window: &mut Window, cx: &mut Context<ThreadView>) {
    let Some(thread) = view.thread().map(str::to_owned) else { return };
    if view.followed_workspace().is_none() {
        return;
    }
    let bar = bar(&thread, cx);
    let Some(title) = bar.read(cx).row.as_ref().map(|r| crumb(r).title) else { return };
    let input = cx.new(|cx| InputState::new(window, cx).default_value(title.clone()));
    // Weak: the subscription lives in the bar it names.
    let weak = bar.downgrade();
    let events = cx.subscribe_in(&input, window, move |view, input, event: &InputEvent, _, cx| {
        let Some(b) = weak.upgrade() else { return };
        match event {
            InputEvent::PressEnter { .. } => {
                if b.read(cx).renaming.as_ref().is_none_or(|r| r.saving) {
                    return;
                }
                let typed = input.read(cx).value().trim().to_owned();
                // A name that is blank or unchanged closes the box, as the web's does.
                if typed.is_empty() || typed == title {
                    return close_rename(&b, cx);
                }
                save_name(view, &b, typed, cx);
            }
            InputEvent::Blur if b.read(cx).renaming.as_ref().is_some_and(|r| !r.saving) => set(&b, cx, |b| b.renaming = None),
            _ => {}
        }
    });
    input.update(cx, |state, cx| {
        state.focus(window, cx);
        state.select_all(window, cx);
    });
    set(&bar, cx, |b| b.renaming = Some(Renaming { input, saving: false, _events: events }));
}

fn close_rename(bar: &Entity<Bar>, cx: &mut App) {
    set(bar, cx, |b| b.renaming = None);
}

/// `renameThread`: the name goes to the thread's agent, after a wake where its machine naps. The box closes once the
/// agent keeps it, and stays open on the typed name with the reason raised where it does not.
fn save_name(view: &mut ThreadView, bar: &Entity<Bar>, title: String, cx: &mut Context<ThreadView>) {
    let Some(workspace) = view.followed_workspace().map(str::to_owned) else { return };
    let Some(row) = bar.read(cx).row.clone() else { return };
    let Some(session) = row.get("id").and_then(Value::as_str).map(str::to_owned) else { return };
    let harness = row.get("harness").and_then(Value::as_str).unwrap_or_default().to_owned();
    let napping = bar.read(cx).status.as_ref().and_then(|s| s.get("phase")).and_then(Value::as_str).is_some_and(|p| p != "running");
    set(bar, cx, |b| {
        if let Some(r) = b.renaming.as_mut() {
            r.saving = true;
        }
    });
    let (b, title_said) = (bar.clone(), title.clone());
    let rename = move |view: &mut ThreadView, cx: &mut Context<ThreadView>| {
        let said = title.clone();
        ask(
            view,
            request::<SessionsRenameRequest>(json!({ "sessionId": session, "title": title })),
            |r| serde_json::to_value(r).unwrap_or_default(),
            cx,
            move |_, answer, cx| {
                let refused = match answer {
                    Ok(reply) if reply["outcome"] == "renamed" => None,
                    Ok(reply) => Some(crate::screens::sidebar::rename_not_taken_line(
                        &harness,
                        reply["outcome"].as_str().unwrap_or_default(),
                        reply["error"].as_str(),
                    )),
                    Err(failure) => Some(format!("{said}: {failure}")),
                };
                match refused {
                    None => set(&b, cx, |b| b.renaming = None),
                    Some(refused) => {
                        set(&b, cx, |b| {
                            if let Some(r) = b.renaming.as_mut() {
                                r.saving = false;
                            }
                        });
                        crate::ui::notify(cx, Notice::error(refused));
                    }
                }
            },
        );
    };
    if !napping {
        return rename(view, cx);
    }
    let (b, named) = (bar.clone(), title_said);
    ask(
        view,
        request::<WorkspacesWakeRequest>(json!({ "workspaceId": workspace })),
        |_| Value::Null,
        cx,
        move |view, answer, cx| match answer {
            Ok(_) => rename(view, cx),
            Err(said) => {
                set(&b, cx, |b| {
                    if let Some(r) = b.renaming.as_mut() {
                        r.saving = false;
                    }
                });
                crate::ui::notify(cx, Notice::error(format!("{named}: {said}")));
            }
        },
    );
}
