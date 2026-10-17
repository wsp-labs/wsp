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
    EditorListRequest, EditorOpenRequest, HarnessesListRequest, PreferencesGetRequest, PreferencesSetRequest, ProjectsListRequest,
    SessionsListRequest, SessionsRenameRequest, WorkspacesBringBackRequest, WorkspacesCommitDraftRequest, WorkspacesCommitRequest,
    WorkspacesWakeRequest,
};
use wsp_host_client::{Event, Op};

use super::open_pane;
use crate::screens::app_shell::title::thread_title;
use crate::screens::Pane;
use crate::thread_view::ThreadView;
use crate::transcript::TurnContext;
use crate::ui::{shortcut_tooltip, spinner, tip, ActivePalette, Glyph, Palette};
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

// The git button, gitAction.logic.ts.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitAct {
    CommitPushPr,
    CommitPush,
    Push,
    CreatePr,
    ViewPr,
}

/// DETACHED_HEAD in packages/protocol/src/wire/daemon.ts.
const DETACHED_HEAD: &str = "(detached)";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Checkout {
    pub branch: String,
    pub ahead: i64,
    pub changed: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitState {
    pub checkout: Option<Checkout>,
    pub pr_open: bool,
    /// The branch the copy's work goes back to, where it is known.
    pub base: Option<String>,
    /// False on a copy a lead's thread opened, whose work lands in the lead's pull request.
    pub opens_pr: bool,
    pub busy: bool,
    /// Why the copy cannot be asked right now: every act but View PR is held on it.
    pub away: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GitQuick {
    pub label: &'static str,
    pub act: Option<GitAct>,
    pub hint: Option<String>,
    /// The act a part held for an away machine stands for, which its icon still shows.
    pub holds: Option<GitAct>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAct {
    Changes,
    Push,
    CreatePr,
    ViewPr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GitItem {
    pub id: &'static str,
    pub label: &'static str,
    pub act: MenuAct,
    pub disabled: bool,
}

pub mod git_words {
    pub const COMMIT: &str = "Commit";
    pub const COMMIT_PUSH_PR: &str = "Commit, push and PR";
    pub const COMMIT_PUSH: &str = "Commit and push";
    pub const PUSH: &str = "Push";
    pub const PUSH_CREATE_PR: &str = "Push and create PR";
    pub const CREATE_PR: &str = "Create PR";
    pub const VIEW_PR: &str = "View PR";
    pub const MENU: &str = "More git actions";
    pub const NO_BRANCH: &str = "The copy is on no branch. Check one out in the terminal to push it.";
    pub const NOTHING: &str = "Nothing to commit or push.";
    pub const UNREAD: &str = "The branch is not read yet.";
    pub const BUSY: &str = "A git action is running.";
    pub fn on_base(base: &str) -> String {
        format!("{base} is the branch the work goes back to; put the work on a branch of its own to push it.")
    }
}

fn held(label: &'static str, hint: String) -> GitQuick {
    GitQuick { label, act: None, hint: Some(hint), holds: None }
}

fn acts(label: &'static str, act: GitAct) -> GitQuick {
    GitQuick { label, act: Some(act), hint: None, holds: None }
}

pub fn git_quick(s: &GitState) -> GitQuick {
    use git_words as w;
    let Some(checkout) = &s.checkout else { return held(w::COMMIT, w::UNREAD.to_owned()) };
    if checkout.branch.is_empty() || checkout.branch == DETACHED_HEAD {
        return held(w::COMMIT, w::NO_BRANCH.to_owned());
    }
    let quick = pick(s, checkout);
    if let (Some(away), Some(act)) = (&s.away, quick.act) {
        if act != GitAct::ViewPr {
            return GitQuick { holds: Some(act), ..held(quick.label, away.clone()) };
        }
    }
    if s.busy && quick.act.is_some() {
        return held(quick.label, w::BUSY.to_owned());
    }
    quick
}

fn pick(s: &GitState, checkout: &Checkout) -> GitQuick {
    use git_words as w;
    if let Some(base) = s.base.as_deref().filter(|b| *b == checkout.branch) {
        return held(w::COMMIT, w::on_base(base));
    }
    let quiet = !s.opens_pr || s.pr_open;
    if checkout.changed > 0 {
        return if quiet { acts(w::COMMIT_PUSH, GitAct::CommitPush) } else { acts(w::COMMIT_PUSH_PR, GitAct::CommitPushPr) };
    }
    if checkout.ahead > 0 {
        return if quiet { acts(w::PUSH, GitAct::Push) } else { acts(w::PUSH_CREATE_PR, GitAct::CreatePr) };
    }
    if !s.opens_pr {
        return held(w::PUSH, w::NOTHING.to_owned());
    }
    if s.pr_open {
        return acts(w::VIEW_PR, GitAct::ViewPr);
    }
    acts(w::CREATE_PR, GitAct::CreatePr)
}

pub fn git_menu(s: &GitState) -> Vec<GitItem> {
    use git_words as w;
    let checkout = s.checkout.as_ref();
    let branched = checkout.is_some_and(|c| !c.branch.is_empty() && c.branch != DETACHED_HEAD);
    let changes = checkout.is_some_and(|c| c.changed > 0);
    let ahead = checkout.is_some_and(|c| c.ahead > 0);
    let on_base = s.base.is_some() && checkout.map(|c| &c.branch) == s.base.as_ref();
    let stopped = s.busy || s.away.is_some();
    let mut items = vec![
        GitItem { id: "commit", label: w::COMMIT, act: MenuAct::Changes, disabled: stopped || !changes },
        GitItem {
            id: "push",
            label: w::PUSH,
            act: MenuAct::Push,
            disabled: stopped || !branched || on_base || !ahead || (s.opens_pr && !s.pr_open),
        },
    ];
    if s.opens_pr {
        items.push(if s.pr_open {
            GitItem { id: "pr", label: w::VIEW_PR, act: MenuAct::ViewPr, disabled: false }
        } else {
            GitItem { id: "pr", label: w::CREATE_PR, act: MenuAct::CreatePr, disabled: stopped || !branched || changes || on_base }
        });
    }
    items
}

/// workspaceState in packages/protocol/src/workspace-state.ts, off a status row.
pub fn workspace_state(status: &Value) -> &'static str {
    let phase = status.get("phase").and_then(Value::as_str).unwrap_or("running");
    let machine = status.get("machineState").and_then(Value::as_str);
    let reach = status.pointer("/reach/state").and_then(Value::as_str);
    if phase == "released" {
        return "released";
    }
    if machine == Some("gone") {
        return "gone";
    }
    match phase {
        "pausing" => "pausing",
        "napping" => "paused",
        "waking" => "waking",
        "gone" => "gone",
        "queued" => "waiting",
        _ if machine == Some("paused") => "paused",
        _ if machine == Some("starting") => "waking",
        _ if matches!(reach, Some("unreachable" | "no-daemon" | "zombie")) => "unreachable",
        _ => "running",
    }
}

/// awayLine: actionRefusal for the git acts on a machine that is not running.
pub fn away_line(state: &str) -> Option<String> {
    let action = if matches!(state, "waking" | "unreachable" | "waiting") { "git action" } else { "push" };
    match state {
        "running" => None,
        "pausing" | "paused" => Some(format!("Workspace is {state}; wake it to {action}")),
        "waking" => Some(format!("Workspace is waking; {action}s open when it is running")),
        "unreachable" => Some(format!("Workspace is unreachable; {action}s open when the machine answers")),
        "gone" => Some(format!(
            "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to {action}, which brings back its home folder from the last saved nap"
        )),
        "waiting" => Some(format!("Workspace is waiting for a machine; {action}s open when it is running")),
        _ => Some(format!("This workspace's machine was deleted once its sub-thread was done, so there is none to {action} on")),
    }
}

fn pr_open(status: &Value) -> bool {
    status.pointer("/pr/number").is_some() && status.pointer("/pr/state").and_then(Value::as_str) == Some("open")
}

/// The git split's state off the workspace's status row, or none where GitSplit draws nothing.
pub fn git_state(status: &Value, base: Option<String>, busy: bool) -> Option<GitState> {
    let machine = workspace_state(status);
    let open = pr_open(status);
    let checkout = status.get("checkout")?;
    if machine == "released" || (machine == "gone" && !open) {
        return None;
    }
    let count = |name: &str| checkout.get(name).and_then(Value::as_i64).unwrap_or(0);
    Some(GitState {
        checkout: Some(Checkout {
            branch: checkout.get("branch").and_then(Value::as_str).unwrap_or_default().to_owned(),
            ahead: count("ahead"),
            changed: count("changed"),
        }),
        pr_open: open,
        base,
        opens_pr: status.get("parentThreadId").is_none(),
        busy,
        away: away_line(machine),
    })
}

// Open in the editor, OpenSplit.tsx.

pub const OPEN: &str = "Open";
pub const CHOOSE: &str = "Choose an editor";
pub const SMALL_LINE: &str = "An editor here takes about what one more thread does, or more.";
/// Memory at or under this, a workspace on another computer hears what an editor there costs.
const SMALL_MB: f64 = 4096.;

#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    pub id: String,
    pub name: String,
    pub remote: bool,
}

/// What Open draws: none where it draws nothing; `current` none while the host lists its editors, the slot held.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenState {
    pub editors: Vec<Editor>,
    pub current: Option<Editor>,
    pub here: bool,
    pub small: bool,
    pub opening: bool,
}

pub fn open_state(status: &Value, editors: Option<&[Editor]>, picked: Option<&str>, opening: bool) -> Option<OpenState> {
    let here = status.get("kind").and_then(Value::as_str).unwrap_or("cloud") == "local";
    let running = status.get("phase").and_then(Value::as_str) == Some("running");
    if !here && !running {
        return None;
    }
    let usable: Option<Vec<Editor>> = editors.map(|all| all.iter().filter(|e| here || e.remote).cloned().collect());
    if usable.as_ref().is_some_and(Vec::is_empty) {
        return None;
    }
    let current = usable.as_ref().and_then(|u| u.iter().find(|e| Some(e.id.as_str()) == picked).or(u.first()).cloned());
    let small = !here && status.pointer("/size/memMb").and_then(Value::as_f64).is_some_and(|mb| mb <= SMALL_MB);
    Some(OpenState { editors: usable.unwrap_or_default(), current, here, small, opening })
}

/// The `editor.open` binding's keycap.
fn open_shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘O"
    } else {
        "Ctrl+O"
    }
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
    pub busy: bool,
    pub opening: bool,
    /// What the last act said where it did not happen, shown as a notice for a while.
    pub said: Option<Said>,
    said_for: Option<Task<()>>,
    pub git_menu: bool,
    pub open_menu: bool,
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

/// A notice the cluster raised: the words, whether they say a failure, and the workspace they are about.
#[derive(Clone, Debug, PartialEq)]
pub struct Said {
    pub text: String,
    pub failed: bool,
    pub about: Option<String>,
}

/// `NOTICE_MS`: how long a notice stands.
const NOTICE: Duration = Duration::from_millis(8_000);

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

/// The project `thread`'s crumbs name, while its cluster stands.
pub fn project_of(thread: &str, cx: &App) -> Option<String> {
    let bar = cx.try_global::<Bars>()?.0.get(thread)?;
    bar.read(cx).row.as_ref().and_then(|row| crumb(row).project)
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

/// The editors the host found installed and the person's pick, read once per host: the app holds one host.
#[derive(Default)]
pub struct Editors {
    /// None until the host lists them.
    pub list: Option<Vec<Editor>>,
    pub picked: Option<String>,
    asked: bool,
}

impl Global for Editors {}

/// Takes what the host said of its editors and the pick, and draws the open thread's split again.
pub fn set_editors(list: Option<Vec<Editor>>, picked: Option<String>, cx: &mut App) {
    let editors = cx.default_global::<Editors>();
    if list.is_some() {
        editors.asked = true;
        editors.list = list;
    }
    editors.picked = picked.or(editors.picked.take());
    let bars: Vec<Entity<Bar>> = cx.default_global::<Bars>().0.values().cloned().collect();
    for bar in bars {
        bar.update(cx, |_, cx| cx.notify());
    }
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
    if cx.default_global::<Editors>().asked {
        return;
    }
    cx.default_global::<Editors>().asked = true;
    ask_host(
        view,
        request::<EditorListRequest>(json!({})),
        |r| json!({ "editors": r.editors }),
        cx,
        move |answer, cx| {
            // A refusal leaves the list unread, so the next thread to open asks again.
            let Ok(answer) = answer else {
                cx.default_global::<Editors>().asked = false;
                return;
            };
            let editors = answer["editors"].as_array().cloned().unwrap_or_default();
            let editors = editors.iter().filter_map(|e| {
                Some(Editor {
                    id: e.get("id")?.as_str()?.to_owned(),
                    name: e.get("name")?.as_str()?.to_owned(),
                    remote: e.get("remote").and_then(Value::as_bool) == Some(true),
                })
            });
            set_editors(Some(editors.collect()), None, cx);
        },
    );
    ask_host(
        view,
        request::<PreferencesGetRequest>(json!({})),
        |r| json!({ "preferences": r.preferences }),
        cx,
        move |answer, cx| {
            let picked = answer.ok().and_then(|a| a.pointer("/preferences/editor").and_then(Value::as_str).map(str::to_owned));
            set_editors(None, picked, cx);
        },
    );
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
                set_editors(None, picked, cx);
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

/// Raises a notice about the thread's workspace, which goes after a while or on its close.
fn say(bar: &Entity<Bar>, text: String, failed: bool, cx: &mut App) {
    bar.update(cx, |b, cx| {
        let about = b.status.as_ref().and_then(|s| s.get("name")).and_then(Value::as_str).map(str::to_owned);
        b.said = Some(Said { text, failed, about });
        b.said_for = Some(cx.spawn(async move |b, cx| {
            cx.background_executor().timer(NOTICE).await;
            _ = b.update(cx, |b, cx| {
                b.said = None;
                cx.notify();
            });
        }));
        cx.notify();
    });
}

fn failed(bar: &Entity<Bar>, said: String, cx: &mut App) {
    eprintln!("desktop-gpui: {said}");
    set(bar, cx, |b| {
        b.busy = false;
        b.opening = false;
    });
    say(bar, said, true, cx);
}

/// GitSplit's run: View PR opens the PR; the rest bring back, first committing all with the agent's drafted message.
fn run(view: &mut ThreadView, bar: &Entity<Bar>, act: GitAct, cx: &mut Context<ThreadView>) {
    let Some(workspace) = view.followed_workspace().map(str::to_owned) else { return };
    if act == GitAct::ViewPr {
        open_pane(Pane::PullRequest, None, cx);
        return;
    }
    set(bar, cx, |b| b.busy = true);
    if matches!(act, GitAct::CommitPushPr | GitAct::CommitPush) {
        let thread = view.followed().map(str::to_owned);
        let (b, ws, pressed) = (bar.clone(), workspace.clone(), thread.clone());
        ask(
            view,
            request::<WorkspacesCommitDraftRequest>(json!({ "workspaceId": workspace, "threadId": thread })),
            |r| json!(r),
            cx,
            move |view, answer, cx| {
                let draft = match answer {
                    Ok(draft) => draft,
                    Err(said) => return failed(&b, said, cx),
                };
                // With no draft the Changes pane opens on the uncommitted files, so the person writes one, while
                // the window still shows the thread they pressed on.
                let Some(message) = draft.get("message").and_then(Value::as_str).map(str::to_owned) else {
                    set(&b, cx, |b| b.busy = false);
                    if view.followed() != pressed.as_deref() {
                        return;
                    }
                    if let Some(note) = draft.get("note").and_then(Value::as_str) {
                        say(&b, note.to_owned(), false, cx);
                    }
                    open_pane(Pane::Changes, None, cx);
                    return;
                };
                let bb = b.clone();
                ask(
                    view,
                    request::<WorkspacesCommitRequest>(json!({ "workspaceId": ws, "message": message })),
                    |_| Value::Null,
                    cx,
                    move |view, answer, cx| match answer {
                        Ok(_) => bring_back(view, &bb, ws, cx),
                        Err(said) => failed(&bb, said, cx),
                    },
                );
            },
        );
        return;
    }
    bring_back(view, bar, workspace, cx);
}

/// Brings back the workspace the person pressed on, whatever the view follows by the time it is asked; the host pushes
/// the status the act left.
fn bring_back(view: &mut ThreadView, bar: &Entity<Bar>, workspace: String, cx: &mut Context<ThreadView>) {
    let b = bar.clone();
    ask(
        view,
        request::<WorkspacesBringBackRequest>(json!({ "workspaceId": workspace })),
        |_| Value::Null,
        cx,
        move |_, answer, cx| match answer {
            Ok(_) => set(&b, cx, |b| b.busy = false),
            Err(said) => failed(&b, said, cx),
        },
    );
}

/// openCopyInEditor: the copy's folder in the default editor, or in `pick`, which becomes the default first.
fn open_in(view: &mut ThreadView, bar: &Entity<Bar>, pick: Option<String>, cx: &mut Context<ThreadView>) {
    let Some(workspace) = view.followed_workspace().map(str::to_owned) else { return };
    let Some(status) = bar.read(cx).status.clone() else { return };
    let editors = cx.default_global::<Editors>();
    let (list, picked) = (editors.list.clone(), editors.picked.clone());
    let Some(open) = open_state(&status, list.as_deref(), picked.as_deref(), bar.read(cx).opening) else { return };
    if open.opening {
        return;
    }
    let folder = status.get("folder").or(status.pointer("/project/path")).and_then(Value::as_str).unwrap_or_default().to_owned();
    let named = if open.here { None } else { pick.clone().or(open.current.map(|c| c.id)) };
    let mut ask_open = json!({ "workspaceId": workspace, "path": folder });
    if let Some(editor) = named {
        ask_open["editor"] = editor.into();
    }
    set(bar, cx, |b| b.opening = true);
    let opening = request::<EditorOpenRequest>(ask_open);
    let b = bar.clone();
    let open_now = move |view: &mut ThreadView, cx: &mut Context<ThreadView>| {
        ask(
            view,
            opening,
            |_| Value::Null,
            cx,
            move |_, answer, cx| match answer {
                Ok(_) => set(&b, cx, |b| b.opening = false),
                Err(said) => failed(&b, said, cx),
            },
        );
    };
    let Some(pick) = pick else { return open_now(view, cx) };
    let b = bar.clone();
    ask(
        view,
        request::<PreferencesSetRequest>(json!({ "patch": { "editor": pick } })),
        |_| Value::Null,
        cx,
        move |view, answer, cx| match answer {
            Ok(_) => {
                set_editors(None, Some(pick), cx);
                open_now(view, cx);
            }
            Err(said) => failed(&b, said, cx),
        },
    );
}

/// `editor.open`: the thread's copy in the default editor, as Open's main part opens it.
pub fn open_default(view: &mut ThreadView, cx: &mut Context<ThreadView>) {
    let Some(thread) = view.thread().map(str::to_owned) else { return };
    let bar = bar(&thread, cx);
    open_in(view, &bar, None, cx);
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

/// The cluster: the ring, the git split and Open, 8 px apart, at the header's right.
pub fn actions(view: &mut ThreadView, window: &mut Window, cx: &mut Context<ThreadView>) -> impl IntoElement {
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
    // Read off the status in place: the cluster draws on every frame of a scroll.
    let (reading, compacts, git, open) = {
        let b = bar.read(cx);
        let catalog = b.catalogs.iter().find(|c| c.harness == harness);
        let agent = agent_label(&b.catalogs, &harness);
        let compacts = catalog.and_then(|c| c.compacts.clone());
        let reading = reading(&turns, &agent, compacts.is_some());
        let status = b.status.as_ref();
        let base = status.and_then(|s| s.pointer("/project/id")).and_then(Value::as_str).and_then(|id| b.bases.get(id).cloned());
        let git = status.and_then(|s| git_state(s, base, b.busy));
        let editors = cx.try_global::<Editors>();
        let (list, picked) = (editors.and_then(|e| e.list.as_deref()), editors.and_then(|e| e.picked.as_deref()));
        let open = status.and_then(|s| open_state(s, list, picked, b.opening));
        (reading, compacts, git, open)
    };
    let me = cx.entity().downgrade();
    h_flex()
        .debug_selector(|| "header-actions".into())
        .flex_none()
        .ml_auto()
        .mr(px(1.))
        .gap(px(8.))
        .children(reading.map(|r| ring(&r, &bar, compacts, me.clone(), window, cx)))
        .children(git.map(|g| git_split(&g, &bar, me.clone(), cx)))
        .children(open.map(|o| open_split(&o, &bar, me.clone(), cx)))
        .into_any_element()
}

/// `KEYCAP_BEVEL`: the 1 px light inside the top edge.
fn bevel(p: &Palette) -> Vec<BoxShadow> {
    vec![BoxShadow { color: p.keycap_top, offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }]
}

#[derive(Clone, Copy, PartialEq)]
enum Ends {
    Both,
    Left,
    Right,
}

/// The outline keycap (`OUTLINE_SURFACE`) with only the corners its split side keeps; held is `disabled:opacity-64`.
fn keycap(id: &'static str, ends: Ends, held: bool, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let radius = px(8.);
    let corners = |el: gpui_kit::Div, r: Pixels| match ends {
        Ends::Both => el.rounded(r),
        Ends::Left => el.rounded_l(r),
        Ends::Right => el.rounded_r(r),
    };
    let surface = corners(div(), radius)
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
        .child(corners(div(), radius - px(1.)).absolute().inset_0().shadow(bevel(p)));
    let hover = p.input_fill_hover;
    surface.id(id).debug_selector(move || id.into()).map(|el| {
        if held {
            el.opacity(0.64)
        } else {
            el.cursor_pointer().hover(move |s| s.bg(hover))
        }
    })
}

/// The chevron's left edge over the main part's right one, as `-ms-px` stacks them.
fn shared_edge(p: &Palette) -> impl IntoElement {
    div().absolute().left(px(-1.)).top(px(-1.)).bottom(px(-1.)).w(px(1.)).bg(p.input)
}

fn glyph(icon: Glyph, size: Pixels, p: &Palette) -> Icon {
    Icon::new(icon).with_size(size).text_color(p.muted_foreground)
}

/// `MenuPopup` under its trigger, its right edge on the trigger's (align end, 4 px down).
fn popup(id: &'static str, below: Pixels, shut: impl Fn(&mut App) + 'static, p: &Palette, items: Vec<AnyElement>) -> AnyElement {
    let surface = crate::ui::menu::surface(p);
    div()
        .absolute()
        .right_0()
        .top(below)
        .child(
            deferred(
                anchored().anchor(Anchor::TopRight).snap_to_window().child(
                    v_flex()
                        .id(id)
                        .debug_selector(move || id.into())
                        .occlude()
                        .min_w(px(128.))
                        .p(px(4.))
                        .rounded(surface.radius)
                        .border_1()
                        .border_color(surface.edge)
                        .bg(p.popover)
                        .shadow(surface.shadow)
                        .children(items)
                        .on_mouse_down_out(move |_, _, cx| shut(cx)),
                ),
            )
            .with_priority(1),
        )
        .into_any_element()
}

/// `MenuItem`: 28 px, its glyph at 16 in the muted ink at 80%, the accent under the pointer, held at 64%.
fn menu_item(id: SharedString, lead: AnyElement, label: SharedString, disabled: bool, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let accent = p.accent;
    h_flex()
        .id(id.clone())
        .debug_selector(move || id.to_string())
        .min_h(px(28.))
        .gap(px(8.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(8.))
        .text_size(px(14.))
        .line_height(px(20.))
        .text_color(p.foreground)
        .child(lead)
        .child(label)
        .map(|el| if disabled { el.opacity(0.64) } else { el.cursor_pointer().hover(move |s| s.bg(accent)) })
}

/// A menu item's glyph: 16 px in the muted ink at 80%, pulled 2 px into the gap on each side.
fn menu_glyph(icon: Glyph, p: &Palette) -> AnyElement {
    div().flex_none().mx(px(-2.)).opacity(0.8).child(glyph(icon, px(16.), p)).into_any_element()
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
        let button = keycap("context-compact", Ends::Both, shown, p)
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

/// GitSplit: the quick act on the main part, the rest in the menu under the chevron.
fn git_split(state: &GitState, bar: &Entity<Bar>, view: WeakEntity<ThreadView>, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    let quick = git_quick(state);
    let icon = match quick.act.or(quick.holds) {
        Some(GitAct::ViewPr | GitAct::CreatePr) => Glyph::GitPullRequest,
        Some(GitAct::Push) => Glyph::Upload,
        _ => Glyph::GitCommitHorizontal,
    };
    let tip = quick.hint.clone().unwrap_or_else(|| quick.label.to_owned());
    let (b, v) = (bar.clone(), view.clone());
    let main = keycap("git-quick", Ends::Left, quick.act.is_none(), &p)
        .h(px(32.))
        .px(px(12.))
        .gap(px(6.))
        .child(glyph(icon, px(14.), &p))
        .child(quick.label)
        .tooltip(shortcut_tooltip(tip, &[]))
        .when_some(quick.act, |el, act| {
            el.on_click(move |_, _, cx| {
                let b = b.clone();
                _ = v.update(cx, |view, cx| run(view, &b, act, cx));
            })
        });
    let menu_open = bar.read(cx).git_menu;
    let toggle = bar.clone();
    // `-ms-px` as a missing left edge: taffy sizes a row short around a child's negative margin.
    let more = keycap("git-more", Ends::Right, false, &p)
        .border_l_0()
        .child(shared_edge(&p))
        .h(px(32.))
        .px(px(8.))
        .child(glyph(Glyph::ChevronDown, px(14.), &p))
        .on_click(move |_, _, cx| set(&toggle, cx, |b| b.git_menu = !b.git_menu));
    let menu = menu_open.then(|| {
        let items = git_menu(state)
            .into_iter()
            .map(|item| {
                let icon = match item.id {
                    "commit" => Glyph::GitCommitHorizontal,
                    "push" => Glyph::Upload,
                    _ => Glyph::GitPullRequest,
                };
                let (b, v) = (bar.clone(), view.clone());
                let act = item.act;
                menu_item(SharedString::from(format!("git-item-{}", item.id)), menu_glyph(icon, &p), item.label.into(), item.disabled, &p)
                    .when(!item.disabled, |el| {
                        el.on_click(move |_, _, cx| {
                            set(&b, cx, |b| b.git_menu = false);
                            let act = match act {
                                MenuAct::Changes => {
                                    open_pane(Pane::Changes, None, cx);
                                    return;
                                }
                                MenuAct::Push => GitAct::Push,
                                MenuAct::CreatePr => GitAct::CreatePr,
                                MenuAct::ViewPr => GitAct::ViewPr,
                            };
                            let b = b.clone();
                            _ = v.update(cx, |view, cx| run(view, &b, act, cx));
                        })
                    })
                    .into_any_element()
            })
            .collect();
        let shut = bar.clone();
        popup("git-menu", px(32. + 4.), move |cx| set(&shut, cx, |b| b.git_menu = false), &p, items)
    });
    h_flex()
        .debug_selector(|| "git-split".into())
        .flex_none()
        .child(main)
        .child(div().relative().flex_none().child(more).children(menu))
        .into_any_element()
}

/// OpenSplit: Open in the default editor, and the editors the host found in the menu, the default with its keycap.
fn open_split(state: &OpenState, bar: &Entity<Bar>, view: WeakEntity<ThreadView>, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    let listed = state.current.is_some();
    let tip = state.current.as_ref().map_or_else(|| OPEN.to_owned(), |c| format!("Open in {}", c.name));
    let (b, v) = (bar.clone(), view.clone());
    let mark = if state.opening { spinner(cx).into_any_element() } else { div().size(px(16.)).flex_none().into_any_element() };
    let main = keycap("open-main", Ends::Left, false, &p)
        .h(px(32.))
        .px(px(12.))
        .gap(px(6.))
        .child(mark)
        .child(OPEN)
        .tooltip(shortcut_tooltip(tip, &[]))
        .on_click(move |_, _, cx| {
            let b = b.clone();
            _ = v.update(cx, |view, cx| open_in(view, &b, None, cx));
        });
    let toggle = bar.clone();
    let more = keycap("open-choose", Ends::Right, false, &p)
        .border_l_0()
        .child(shared_edge(&p))
        .h(px(32.))
        .px(px(8.))
        .child(glyph(Glyph::ChevronDown, px(14.), &p))
        .on_click(move |_, _, cx| set(&toggle, cx, |b| b.open_menu = !b.open_menu));
    let menu = bar.read(cx).open_menu.then(|| {
        let mut items: Vec<AnyElement> = Vec::new();
        if state.small {
            items.push(
                div()
                    .max_w(px(256.))
                    .px(px(8.))
                    .py(px(6.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(p.muted_foreground)
                    .child(SMALL_LINE)
                    .into_any_element(),
            );
        }
        for editor in &state.editors {
            let (b, v, id) = (bar.clone(), view.clone(), editor.id.clone());
            let current = state.current.as_ref().is_some_and(|c| c.id == editor.id);
            let item = menu_item(
                SharedString::from(format!("open-item-{}", editor.id)),
                div().size(px(16.)).flex_none().into_any_element(),
                editor.name.clone().into(),
                false,
                &p,
            )
            .when(current, |el| {
                el.child(
                    div()
                        .ml_auto()
                        .h(px(20.))
                        .min_w(px(20.))
                        .px(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .bg(p.muted)
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(p.muted_foreground)
                        .child(open_shortcut()),
                )
            })
            .on_click(move |_, _, cx| {
                set(&b, cx, |b| b.open_menu = false);
                let (b, id) = (b.clone(), id.clone());
                _ = v.update(cx, |view, cx| open_in(view, &b, Some(id), cx));
            });
            items.push(item.into_any_element());
        }
        let shut = bar.clone();
        popup("open-menu", px(32. + 4.), move |cx| set(&shut, cx, |b| b.open_menu = false), &p, items)
    });
    h_flex()
        .debug_selector(|| "open-split".into())
        .flex_none()
        .when(!listed, |el| el.invisible())
        .child(main)
        .child(div().relative().flex_none().child(more).children(menu))
        .into_any_element()
}

// The crumbs, ThreadBreadcrumb.tsx.

pub const NEW_THREAD: &str = "New thread";
/// THREAD_WORDS.rename.
pub const RENAME: &str = "Rename thread";
/// A text field's `size`: the width the name box asks for, in characters of its own face.
const NAME_BOX_CHARS: f32 = 20.;
/// `OWN_CRUMB`: beside its project the thread keeps at most this share of the line.
const OWN_SHARE: f32 = 0.7;

/// What the crumbs read off the thread's newest row: its project, its title, and what it waits on where it waits.
#[derive(Clone, Debug, PartialEq)]
pub struct Crumb {
    pub project: Option<String>,
    pub title: String,
    pub waiting: Option<String>,
}

pub fn crumb(row: &Value) -> Crumb {
    let text = |key: &str| row.get(key).and_then(Value::as_str);
    let title = thread_title(text("harnessTitle"), text("prompt")).unwrap_or_else(|| NEW_THREAD.to_owned());
    let project = row.pointer("/project/name").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_owned);
    Crumb { project, title, waiting: waiting_line(row) }
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

/// The crumbs of an open thread: its project and a slash, its title or the name box in the title's place, and the
/// question glyph while it waits on the person. `on_menu` opens the title's menu at the pointer.
pub fn crumbs(view: &mut ThreadView, on_menu: OnMenu, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    let line =
        h_flex().id("crumbs").debug_selector(|| "crumbs".into()).min_w_0().items_center().gap_2().text_size(px(14.)).line_height(px(20.));
    let Some(thread) = view.thread().map(str::to_owned) else { return line.into_any_element() };
    let bar = bar(&thread, cx);
    let Some(crumb) = bar.read(cx).row.as_ref().map(crumb) else { return line.into_any_element() };
    let project = crumb.project.clone().map(|name| {
        h_flex()
            .debug_selector(|| "crumbs-project".into())
            .min_w_0()
            .items_center()
            .gap_2()
            .text_color(p.muted_foreground)
            .child(Icon::new(Glyph::Folder).with_size(px(16.)).flex_none())
            .child(div().truncate().child(SharedString::from(name)))
    });
    let own = project.is_some();
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
    });
    line.children(project)
        .when(own, |el| el.child(crate::ui::slash(cx)))
        .child(div().flex().min_w_0().when(own, |el| el.flex_none().max_w(gpui_kit::relative(OWN_SHARE))).child(title))
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
    let b = bar.clone();
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
                        say(&b, refused, true, cx);
                    }
                }
            },
        );
    };
    if !napping {
        return rename(view, cx);
    }
    let b = bar.clone();
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
                say(&b, said, true, cx);
            }
        },
    );
}

/// The notice the cluster raised, as the app's toasts draw one: the kind's glyph, the words, the workspace, a close.
pub fn notice(view: &mut ThreadView, cx: &mut Context<ThreadView>) -> Option<AnyElement> {
    let thread = view.thread().map(str::to_owned)?;
    let bar = bar(&thread, cx);
    let said = bar.read(cx).said.clone()?;
    let p = cx.palette().clone();
    let (glyph, tone) = if said.failed { (Glyph::CircleAlert, p.status_failed) } else { (Glyph::Info, p.muted_foreground) };
    let shut = bar.clone();
    Some(
        h_flex()
            .id("thread-notice")
            .debug_selector(|| "thread-notice".into())
            .occlude()
            .group("thread-notice")
            .w(px(356.))
            .items_start()
            .gap(px(12.))
            .py(px(12.))
            .pl(px(12.))
            .pr(px(8.))
            .rounded(px(12.))
            .bg(p.popover)
            .shadow(p.popover_shadow.clone())
            .child(div().flex_none().mt(px(2.)).child(Icon::new(glyph).with_size(px(16.)).text_color(tone)))
            .child(
                v_flex()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .debug_selector(|| "thread-notice-text".into())
                            .line_clamp(2)
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.foreground)
                            .child(said.text),
                    )
                    .children(
                        said.about.map(|about| {
                            div().truncate().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(about)
                        }),
                    ),
            )
            .child(
                div()
                    .id("thread-notice-close")
                    .debug_selector(|| "thread-notice-close".into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(24.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .opacity(0.)
                    .group_hover("thread-notice", |s| s.opacity(1.))
                    .hover(|s| s.bg(p.accent))
                    .on_click(move |_, _, cx| set(&shut, cx, |b| b.said = None))
                    .tooltip(tip("Close this message"))
                    .child(Icon::new(Glyph::X).with_size(px(16.)).text_color(p.foreground)),
            )
            .into_any_element(),
    )
}

/// Takes the notice down, saying whether one stood.
pub fn dismiss(view: &mut ThreadView, cx: &mut Context<ThreadView>) -> bool {
    let Some(thread) = view.thread().map(str::to_owned) else { return false };
    let bar = bar(&thread, cx);
    if bar.read(cx).said.is_none() {
        return false;
    }
    set(&bar, cx, |b| b.said = None);
    true
}
