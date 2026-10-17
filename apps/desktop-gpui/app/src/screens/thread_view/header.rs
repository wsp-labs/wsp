// SPDX-License-Identifier: AGPL-3.0-only
//! The thread's cluster at the header's right (AppShell's `ml-auto mr-px gap-2` row): ContextMeter, GitSplit and OpenSplit.

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, svg, Anchor, AnyElement, App, BoxShadow, Context, Entity, FontWeight, Global, Hsla,
    Pixels, SharedString, Subscription, Task, WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    EditorListRequest, EditorOpenRequest, HarnessesListRequest, PreferencesGetRequest, PreferencesSetRequest, ProjectsListRequest,
    StatusSubscribeRequest, WorkspacesBringBackRequest, WorkspacesCommitDraftRequest, WorkspacesCommitRequest,
};
use wsp_host_client::Op;

use crate::thread_view::ThreadView;
use crate::transcript::TurnContext;
use crate::ui::{shortcut_tooltip, spinner, ActivePalette, Glyph, Palette};

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

/// fmtTokens in packages/protocol/src/words/units.ts.
pub fn fmt_tokens(n: f64) -> String {
    let count = if n.is_finite() && n > 0. { n.round() } else { 0. };
    if count < 1_000. {
        return format!("{count}");
    }
    let (value, unit) = if count < 999_500. {
        (count / 1_000., "k")
    } else if count < 999_500_000. {
        (count / 1_000_000., "M")
    } else {
        (count / 1_000_000_000., "B")
    };
    let places = if value < 10. {
        2
    } else if value < 100. {
        1
    } else {
        0
    };
    let fixed = format!("{value:.places$}");
    let trimmed = if fixed.contains('.') { fixed.trim_end_matches('0').trim_end_matches('.') } else { &fixed };
    format!("{trimmed}{unit}")
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
    /// The thread's workspace's status row (`WorkspaceStatus`), as the host sent it.
    pub status: Option<Value>,
    /// Each project's base branch by project id.
    pub bases: HashMap<String, String>,
    pub catalogs: Vec<Catalog>,
    /// None until the host lists its editors.
    pub editors: Option<Vec<Editor>>,
    pub picked: Option<String>,
    pub busy: bool,
    pub opening: bool,
    /// What the last act said where it did not happen.
    pub said: Option<String>,
    pub git_menu: bool,
    pub open_menu: bool,
    ring_hover: bool,
    card_hover: bool,
    closing: Option<Task<()>>,
    turns: Option<(usize, std::rc::Rc<Vec<TurnContext>>, String)>,
    watch: Option<Subscription>,
}

impl Bar {
    pub fn ring_open(&self) -> bool {
        self.ring_hover || self.card_hover || self.closing.is_some()
    }
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

/// Reads the workspace's status row, the projects' bases, the agents' catalogs, the editors and the picked editor.
fn load(view: &mut ThreadView, bar: &Entity<Bar>, cx: &mut Context<ThreadView>) {
    let Some(workspace) = view.followed_workspace().map(str::to_owned) else { return };
    if bar.read(cx).asked {
        return;
    }
    bar.update(cx, |b, _| b.asked = true);
    read_status(view, bar, workspace.clone(), cx);
    let b = bar.clone();
    ask(
        view,
        request::<ProjectsListRequest>(json!({})),
        |r| json!({ "projects": r.projects }),
        cx,
        move |_, answer, cx| {
            let Ok(answer) = answer else { return };
            let bases = answer["projects"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| Some((p.get("id")?.as_str()?.to_owned(), p.get("base")?.as_str()?.to_owned())));
            let bases: HashMap<String, String> = bases.collect();
            set(&b, cx, |b| b.bases = bases);
        },
    );
    let b = bar.clone();
    ask(
        view,
        request::<HarnessesListRequest>(json!({ "workspaceId": workspace })),
        |r| json!({ "harnesses": r.harnesses }),
        cx,
        move |_, answer, cx| {
            let Ok(answer) = answer else { return };
            let catalogs = answer["harnesses"].as_array().into_iter().flatten().filter_map(|c| {
                Some(Catalog {
                    harness: c.get("harness")?.as_str()?.to_owned(),
                    label: c.get("label")?.as_str()?.to_owned(),
                    compacts: c.get("compacts").and_then(Value::as_str).map(str::to_owned),
                })
            });
            let catalogs: Vec<Catalog> = catalogs.collect();
            set(&b, cx, |b| b.catalogs = catalogs);
        },
    );
    let b = bar.clone();
    ask(
        view,
        request::<EditorListRequest>(json!({})),
        |r| json!({ "editors": r.editors }),
        cx,
        move |_, answer, cx| {
            let editors = answer.ok().and_then(|a| a["editors"].as_array().cloned()).unwrap_or_default();
            let editors = editors.iter().filter_map(|e| {
                Some(Editor {
                    id: e.get("id")?.as_str()?.to_owned(),
                    name: e.get("name")?.as_str()?.to_owned(),
                    remote: e.get("remote").and_then(Value::as_bool) == Some(true),
                })
            });
            let editors: Vec<Editor> = editors.collect();
            set(&b, cx, |b| b.editors = Some(editors));
        },
    );
    let b = bar.clone();
    ask(
        view,
        request::<PreferencesGetRequest>(json!({})),
        |r| json!({ "preferences": r.preferences }),
        cx,
        move |_, answer, cx| {
            let picked = answer.ok().and_then(|a| a.pointer("/preferences/editor").and_then(Value::as_str).map(str::to_owned));
            set(&b, cx, |b| b.picked = picked);
        },
    );
}

/// The status row is read once per thread and after each act: the host's live status frames reach the app shell.
fn read_status(view: &mut ThreadView, bar: &Entity<Bar>, workspace: String, cx: &mut Context<ThreadView>) {
    let b = bar.clone();
    ask(
        view,
        request::<StatusSubscribeRequest>(json!({})),
        |r| json!({ "statuses": r.statuses }),
        cx,
        move |_, answer, cx| {
            let Ok(answer) = answer else { return };
            let row = answer["statuses"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|s| s.get("id").and_then(Value::as_str) == Some(workspace.as_str()))
                .cloned();
            set(&b, cx, |b| b.status = row);
        },
    );
}

fn failed(bar: &Entity<Bar>, said: String, cx: &mut App) {
    eprintln!("desktop-gpui: {said}");
    set(bar, cx, |b| {
        b.said = Some(said);
        b.busy = false;
        b.opening = false;
    });
}

/// GitSplit's run: View PR opens the PR; the rest bring back, first committing all with the agent's drafted message.
fn run(view: &mut ThreadView, bar: &Entity<Bar>, act: GitAct, cx: &mut Context<ThreadView>) {
    let Some(workspace) = view.followed_workspace().map(str::to_owned) else { return };
    if act == GitAct::ViewPr {
        let url = bar.read(cx).status.as_ref().and_then(|s| s.pointer("/pr/url")).and_then(Value::as_str).map(str::to_owned);
        if let Some(url) = url {
            cx.open_url(&url);
        }
        return;
    }
    set(bar, cx, |b| {
        b.busy = true;
        b.said = None;
    });
    if matches!(act, GitAct::CommitPushPr | GitAct::CommitPush) {
        let thread = view.followed().map(str::to_owned);
        let (b, ws) = (bar.clone(), workspace.clone());
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
                let Some(message) = draft.get("message").and_then(Value::as_str).map(str::to_owned) else {
                    let note = draft.get("note").and_then(Value::as_str).map(str::to_owned);
                    set(&b, cx, |b| {
                        b.busy = false;
                        b.said = note;
                    });
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

/// Brings back the workspace the person pressed on, whatever the view follows by the time it is asked.
fn bring_back(view: &mut ThreadView, bar: &Entity<Bar>, workspace: String, cx: &mut Context<ThreadView>) {
    let b = bar.clone();
    ask(
        view,
        request::<WorkspacesBringBackRequest>(json!({ "workspaceId": workspace })),
        |_| Value::Null,
        cx,
        move |view, answer, cx| {
            match answer {
                Ok(_) => set(&b, cx, |b| b.busy = false),
                Err(said) => failed(&b, said, cx),
            }
            read_status(view, &b, workspace, cx);
        },
    );
}

/// openCopyInEditor: the copy's folder in the default editor, or in `pick`, which becomes the default first.
fn open_in(view: &mut ThreadView, bar: &Entity<Bar>, pick: Option<String>, cx: &mut Context<ThreadView>) {
    let Some(workspace) = view.followed_workspace().map(str::to_owned) else { return };
    let b = bar.read(cx);
    let Some(status) = b.status.clone() else { return };
    let Some(open) = open_state(&status, b.editors.as_deref(), b.picked.as_deref(), b.opening) else { return };
    if open.opening {
        return;
    }
    let folder = status.get("folder").or(status.pointer("/project/path")).and_then(Value::as_str).unwrap_or_default().to_owned();
    let named = if open.here { None } else { pick.clone().or(open.current.map(|c| c.id)) };
    let mut ask_open = json!({ "workspaceId": workspace, "path": folder });
    if let Some(editor) = named {
        ask_open["editor"] = editor.into();
    }
    set(bar, cx, |b| {
        b.opening = true;
        b.said = None;
    });
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
                set(&b, cx, |b| b.picked = Some(pick));
                open_now(view, cx);
            }
            Err(said) => failed(&b, said, cx),
        },
    );
}

// Drawing.

/// The cluster: the ring, the git split and Open, 8 px apart, at the header's right.
pub fn actions(view: &mut ThreadView, window: &mut Window, cx: &mut Context<ThreadView>) -> impl IntoElement {
    let Some(thread) = view.thread().map(str::to_owned) else { return div().into_any_element() };
    let bar = bar(&thread, cx);
    if bar.read(cx).watch.is_none() {
        let watch = cx.observe(&bar, |_, _, cx| cx.notify());
        bar.update(cx, |b, _| b.watch = Some(watch));
    }
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
        let open = status.and_then(|s| open_state(s, b.editors.as_deref(), b.picked.as_deref(), b.opening));
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
                        .rounded(px(12.))
                        .border_1()
                        .border_color(p.foreground.opacity(0.1))
                        .bg(p.popover)
                        .shadow(p.popover_shadow.clone())
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
                                MenuAct::Changes => return,
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
