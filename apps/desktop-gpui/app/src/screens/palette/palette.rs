// SPDX-License-Identifier: AGPL-3.0-only
//! The command palette, after `components/palette/CommandPalette.tsx`: one search over the commands, the tasks and the
//! threads the sidebar holds, the host's message hits once the typing rests, and the page of projects New thread
//! picks from. The rows come from [`items`]; this draws them, takes the keys and runs what is picked.

use std::time::Duration;

use gpui_kit::component::input::{Backspace, Enter, Escape, Input, InputEvent, InputState, MoveDown, MoveUp};
use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    actions, div, linear_color_stop, linear_gradient, prelude::*, px, Action, AnyElement, App, Context, Entity, KeyBinding, Pixels,
    ScrollHandle, SharedString, Subscription, Task, Unbind, WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::SessionsSearchRequest;

use super::frame::{self, Row};
use super::items::{self, Description, Go, Group, Hit, Item, Lead, Seen, Trailing};
use super::overlays::{self, Overlays};
use crate::screens::app_shell::keys::{self, Bound, Run};
use crate::screens::sidebar::menu::{self, Act};
use crate::screens::sidebar::{looks, model, tile, Sidebar};
use crate::screens::{navigate, Host, Route};
use crate::shell::Shell;
use crate::sidebar::Asked;
use crate::ui::{catalog, motion, text, ActivePalette, Glyph};

/// How long the typing rests before the words go to the host (`MESSAGE_SEARCH_WAIT_MS`).
pub const SEARCH_WAIT: Duration = Duration::from_millis(200);
/// `--fade-size`: how far the list fades at an edge it scrolls past.
const FADE_EDGE: Pixels = px(24.);
/// The key context the palette's own chords are bound in.
pub const CONTEXT: &str = "Palette";

actions!(palette, [ClosePalette, KeepFocus]);

/// Picks the page's row by its place, counted from 1, as its label's mod digit names it.
#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = palette, no_json)]
pub struct PickRow(pub usize);

/// Binds the palette's chords, once, before any shell rebinds them: mod+digit for the page's rows, and the palette's
/// own toggle, since the shell's chords are bound on the shell's element and the palette lies beside it, not under it.
pub(super) fn bind(cx: &mut App) {
    struct Bound;
    impl gpui_kit::Global for Bound {}
    if cx.has_global::<Bound>() {
        return;
    }
    cx.set_global(Bound);
    let mac = cfg!(target_os = "macos");
    let mut bindings: Vec<KeyBinding> =
        (1..=items::SLOTS).map(|n| KeyBinding::new(&format!("secondary-{n}"), PickRow(n), Some(CONTEXT))).collect();
    bindings.extend(closes(&keys::bound(mac, &Default::default()), mac, false));
    bindings.push(KeyBinding::new("escape", ClosePalette, Some(CONTEXT)));
    // The palette is modal: Tab stays in its field instead of walking to the window under it.
    bindings.push(KeyBinding::new("tab", KeepFocus, Some(CONTEXT)));
    bindings.push(KeyBinding::new("shift-tab", KeepFocus, Some(CONTEXT)));
    cx.bind_keys(bindings);
}

/// The palette's close on each chord `rules` give its toggle, or what takes it back off them.
fn closes(rules: &[Bound], mac: bool, off: bool) -> Vec<KeyBinding> {
    let typed = rules.iter().filter(|r| r.command == "commandPalette.toggle").filter_map(|r| keys::keystroke(&r.chord, mac));
    typed
        .map(|t| match off {
            true => KeyBinding::new(&t, Unbind(ClosePalette.name().into()), Some(CONTEXT)),
            false => KeyBinding::new(&t, ClosePalette, Some(CONTEXT)),
        })
        .collect()
}

/// Moves the palette's close from the toggle chords `was` bound to the ones `now` binds, as the shell rebinds them.
pub fn rebind(was: &[Bound], now: &[Bound], cx: &mut App) {
    let mac = cfg!(target_os = "macos");
    cx.bind_keys(closes(was, mac, true));
    cx.bind_keys(closes(now, mac, false));
}

/// What the host's message search found, for the words it was asked.
struct Found {
    words: String,
    hits: Vec<Hit>,
    unsearched: usize,
}

pub struct Palette {
    overlays: WeakEntity<Overlays>,
    host: Option<Host>,
    sidebar: Option<WeakEntity<Sidebar>>,
    input: Entity<InputState>,
    query: String,
    /// The page of projects is open over the root.
    page: bool,
    highlighted: Option<SharedString>,
    /// The first row runs on Enter but is drawn unlit, until a step or the pointer lights one: the web's list highlights
    /// its first row itself after a keystroke or a page opening, and the palette's own lit row stays cleared.
    unlit: bool,
    seen: Seen,
    /// The sidebar's acts the Actions list holds, each with the thread it runs on.
    acts: Vec<(String, Act)>,
    groups: Vec<Group>,
    found: Option<Found>,
    _search: Option<Task<()>>,
    scroll: ScrollHandle,
    _input: Subscription,
    _sidebar: Option<Subscription>,
}

impl Palette {
    fn new(
        overlays: WeakEntity<Overlays>,
        host: Option<Host>,
        sidebar: Option<WeakEntity<Sidebar>>,
        page: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(items::placeholder(page)));
        let changed = cx.subscribe_in(&input, window, |this, input, e: &InputEvent, window, cx| {
            if matches!(e, InputEvent::Change) {
                let query = input.read(cx).value().to_string();
                this.unlit |= query != this.query;
                this.query = query;
                this.highlighted = None;
                this.ask_host(window, cx);
                this.rebuild(cx);
            }
        });
        input.update(cx, |i, cx| i.focus(window, cx));
        // The sidebar and the shell may be mid-update where the palette opens from one of their own handlers.
        cx.defer_in(window, |this, window, cx| this.see(window, cx));
        Palette {
            overlays,
            host,
            sidebar,
            input,
            query: String::new(),
            page,
            highlighted: None,
            unlit: false,
            seen: Seen::default(),
            acts: Vec::new(),
            groups: Vec::new(),
            found: None,
            _search: None,
            scroll: ScrollHandle::new(),
            _input: changed,
            _sidebar: None,
        }
    }

    /// Reads the window again: the sidebar's projects, tasks and threads, the shell's route and chords.
    fn see(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = shell_of(window, cx);
        // The shell's own sidebar, which the layers are handed only once a host answers.
        let sidebar = shell.as_ref().map(|s| s.read(cx).sidebar().clone()).or_else(|| self.sidebar.as_ref().and_then(|s| s.upgrade()));
        if let (None, Some(watched)) = (&self._sidebar, &sidebar) {
            self.sidebar = Some(watched.downgrade());
            self._sidebar = Some(cx.observe_in(watched, window, |this, _, window, cx| this.see(window, cx)));
        }
        let mac = cfg!(target_os = "macos");
        let rules = shell.as_ref().map(|s| s.read(cx).rules().to_vec()).unwrap_or_else(|| keys::bound(mac, &Default::default()));
        let route = shell.as_ref().map(|s| s.read(cx).route().clone());
        let (seen, acts) = match &sidebar {
            Some(sidebar) => read(sidebar.read(cx), route.as_ref(), self.host.is_some(), &rules, mac),
            None => (Seen { connected: self.host.is_some(), labels: labels(&rules, mac), ..Seen::default() }, Vec::new()),
        };
        self.seen = seen;
        self.acts = acts;
        self.rebuild(cx);
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let words = self.words();
        let (hits, unsearched) = match &self.found {
            Some(found) if found.words == words => (found.hits.as_slice(), found.unsearched),
            _ => (&[][..], 0),
        };
        self.groups = items::lists(&self.seen, &self.query, self.page, hits, unsearched, cfg!(target_os = "macos"));
        cx.notify();
    }

    /// What the message search is asked: the typed words at the root, none on a page or after `>`.
    fn words(&self) -> String {
        if self.page || self.query.starts_with('>') {
            String::new()
        } else {
            self.query.trim().to_owned()
        }
    }

    /// Asks the host for the threads whose messages hold the words, once the typing has rested; a newer ask drops it.
    fn ask_host(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let words = self.words();
        self._search = None;
        let Some(host) = self.host.clone().filter(|_| words.chars().count() >= 2) else { return };
        self._search = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SEARCH_WAIT).await;
            let Ok(request) = serde_json::from_value::<SessionsSearchRequest>(json!({ "query": words })) else { return };
            let Ok(reply) = host.run(|c| async move { c.call(request).await }).await else { return };
            let reply = serde_json::to_value(reply).unwrap_or_default();
            let hits = reply["hits"]
                .as_array()
                .map(|hits| {
                    hits.iter()
                        .filter_map(|h| {
                            Some(Hit {
                                workspace: h["workspaceId"].as_str()?.to_owned(),
                                thread: h["threadId"].as_str()?.to_owned(),
                                snippet: h["snippet"].as_str()?.to_owned(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let unsearched = reply["unsearched"].as_u64().unwrap_or(0) as usize;
            _ = this.update(cx, |this, cx| {
                this.found = Some(Found { words, hits, unsearched });
                this.rebuild(cx);
            });
        }));
    }

    /// The rows a key can land on, in the order they are drawn.
    fn live(&self) -> impl Iterator<Item = &Item> {
        self.groups.iter().flat_map(|g| g.items.iter()).filter(|i| !i.held)
    }

    /// The row the keys stand on: the one picked, else the first, as `autoHighlight="always"` keeps it.
    fn active(&self) -> Option<&Item> {
        let picked = self.highlighted.as_ref().and_then(|v| self.live().find(|i| i.value == *v));
        picked.or_else(|| self.live().next())
    }

    /// The row drawn lit.
    fn lit(&self) -> Option<&Item> {
        if self.unlit && self.highlighted.is_none() {
            return None;
        }
        self.active()
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let live: Vec<SharedString> = self.live().map(|i| i.value.clone()).collect();
        if live.is_empty() {
            return;
        }
        let at = self.active().and_then(|a| live.iter().position(|v| *v == a.value)).unwrap_or(0) as isize;
        let next = (at + by).rem_euclid(live.len() as isize) as usize;
        self.highlighted = Some(live[next].clone());
        if let Some(row) = self.drawn_index(&live[next]) {
            self.scroll.scroll_to_item(row);
        }
        cx.notify();
    }

    /// Where a row stands among the list's children, its groups' labels counted.
    fn drawn_index(&self, value: &SharedString) -> Option<usize> {
        let mut at = 0;
        for group in &self.groups {
            at += 1;
            for item in &group.items {
                if item.value == *value {
                    return Some(at);
                }
                at += 1;
            }
        }
        None
    }

    fn open_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = true;
        self.reset(window, cx);
        self.unlit = true;
    }

    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = false;
        self.reset(window, cx);
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.clear();
        self.highlighted = None;
        self.unlit = false;
        self.found = None;
        self._search = None;
        let ghost = items::placeholder(self.page);
        self.input.update(cx, |i, cx| {
            i.set_value("", window, cx);
            i.set_placeholder(ghost, window, cx);
        });
        self.rebuild(cx);
    }

    /// Shuts the palette and hands the keys back to what held them before it opened.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(overlays) = self.overlays.upgrade() else { return };
        overlays.update(cx, |o, cx| {
            o.palette = None;
            o.give_back(window, cx);
            cx.notify();
        });
    }

    /// Runs a row, or opens its page; a run closes the palette first, so the act lands on the window under it.
    fn execute(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        if item.page || item.go == Go::Command(Run::NewThread) && self.seen.asks {
            self.open_page(window, cx);
            return;
        }
        if !item.keep_open {
            self.close(window, cx);
        }
        match item.go {
            Go::Command(run) => self.run(run, window, cx),
            Go::Task { workspace, thread } | Go::Thread { workspace, thread } => {
                navigate(Route::Thread { workspace_id: workspace, thread_id: thread }, cx);
            }
            Go::Project(id) => {
                if let Some(workspace) = self.seen.projects.iter().find(|p| p.id == id).and_then(|p| p.workspace.clone()) {
                    navigate(Route::Screen(format!("composer:{workspace}").into()), cx);
                }
            }
            Go::Act(index) => {
                let (Some((key, act)), Some(sidebar)) = (self.acts.get(index).cloned(), self.sidebar.as_ref().and_then(|s| s.upgrade()))
                else {
                    return;
                };
                sidebar.update(cx, |s, cx| s.run(&key, act, window, cx));
            }
            Go::Nothing => {}
        }
    }

    fn run(&mut self, run: Run, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(action) = keys::run_action(run) {
            window.dispatch_action(action, cx);
            return;
        }
        match run {
            Run::Screen(route) => navigate(Route::Screen(route.into()), cx),
            Run::Url(url) => cx.open_url(url),
            Run::CopyMarkdown => self.copy_markdown(window, cx),
            Run::AddProject => self.ask(Asked::AddProject, cx),
            Run::AddComputer => self.ask(Asked::AddComputer, cx),
            Run::ResumeConversation | Run::PickProject => {}
            Run::Command(_) | Run::StepWorkspace(_) | Run::StepThread(_) | Run::NewThread => {}
        }
    }

    /// Hands an ask to the shell through the sidebar it follows, so the flow opens where the sidebar's own button
    /// opens it.
    fn ask(&self, asked: Asked, cx: &mut Context<Self>) {
        if let Some(sidebar) = self.sidebar.as_ref().and_then(WeakEntity::upgrade) {
            sidebar.update(cx, |_, cx| cx.emit(asked));
        }
    }

    /// Copies the thread on screen as Markdown through the sidebar's runner, as its menu's Copy as Markdown does.
    fn copy_markdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(thread) = self.seen.open.as_ref().and_then(|o| o.thread.clone()) else { return };
        if let Some(sidebar) = self.sidebar.as_ref().and_then(WeakEntity::upgrade) {
            sidebar.update(cx, |s, cx| s.run(&thread, Act::CopyMarkdown, window, cx));
        }
    }

    fn on_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.step(-1, cx);
    }

    fn on_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.step(1, cx);
    }

    fn on_enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if let Some(item) = self.active().cloned() {
            self.execute(item, window, cx);
        }
    }

    fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.close(window, cx);
    }

    fn on_close(&mut self, _: &ClosePalette, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.close(window, cx);
    }

    fn on_backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.page && self.query.is_empty() {
            cx.stop_propagation();
            self.back(window, cx);
        }
    }

    /// A mod digit on the page runs its row, while that row is listed; taken here so it never reaches anything else.
    fn on_pick(&mut self, pick: &PickRow, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if !self.page {
            return;
        }
        let row = items::projects_page(&self.seen, cfg!(target_os = "macos")).into_iter().nth(pick.0.saturating_sub(1));
        let shown = row.filter(|r| self.groups.iter().any(|g| g.items.iter().any(|i| i.value == r.value)));
        if let Some(row) = shown {
            self.execute(row, window, cx);
        }
    }
}

impl Palette {
    /// The lists drawn now, each with its rows.
    pub fn lists(&self) -> &[Group] {
        &self.groups
    }

    /// The row the keys stand on, by its value.
    pub fn active_row(&self) -> Option<SharedString> {
        self.active().map(|i| i.value.clone())
    }

    /// The row drawn lit, where one is.
    pub fn lit_row(&self) -> Option<SharedString> {
        self.lit().map(|i| i.value.clone())
    }

    /// Whether the page of projects is open over the root.
    pub fn on_page(&self) -> bool {
        self.page
    }

    /// Opens the page of projects over the root, as its row does.
    pub fn show_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_page(window, cx);
    }
}

/// The window's palette, while it stands open.
pub fn palette(window: &Window, cx: &App) -> Option<Entity<Palette>> {
    overlays::of(window, cx)?.read(cx).palette.clone()
}

/// The shell the window's root holds, where it holds one.
fn shell_of(window: &Window, cx: &App) -> Option<Entity<Shell>> {
    window.root::<gpui_kit::base::Root>().flatten()?.read(cx).view().clone().downcast::<Shell>().ok()
}

fn labels(rules: &[Bound], mac: bool) -> Vec<(String, String)> {
    let commands = keys::COMMANDS.iter().filter_map(|c| c.chord).chain(["terminal.toggle", "preview.toggle"]).map(str::to_owned);
    let slots = (1..=items::SLOTS).map(|n| format!("workspace.select.{n}"));
    commands
        .chain(slots)
        .filter_map(|c| {
            let label = keys::shortcut_label(rules, &c, mac)?;
            Some((c, label))
        })
        .collect()
}

/// The palette's reading of the sidebar and the route, with the acts its Actions list runs through the sidebar.
fn read(s: &Sidebar, route: Option<&Route>, connected: bool, rules: &[Bound], mac: bool) -> (Seen, Vec<(String, Act)>) {
    let around = s.around();
    let place_name = |id: &str| around.places.iter().find(|p| p["id"].as_str() == Some(id)).map(looks::place_name);
    let workspace_name = |id: &str| around.workspaces.get(id).and_then(|w| w["name"].as_str()).map(str::to_owned);
    let computer_of = |workspace: &str| s.place_of(workspace).and_then(|p| place_name(&p)).filter(|n| !n.is_empty());
    let now = js_now();
    let threads: Vec<items::Thread> = s
        .threads()
        .iter()
        .map(|t| {
            let kind = model::kind_of(t, now);
            let slot = model::slot(t, kind, now, t.ended_at.or(t.started_at));
            items::Thread {
                id: t.key.clone(),
                workspace: t.workspace_id.clone(),
                title: t.title.clone(),
                harness: t.harness.clone(),
                task: workspace_name(&t.workspace_id).or_else(|| t.project_name.clone()).unwrap_or_default(),
                computer: t.computer.clone().filter(|c| !c.is_empty()),
                started_at: t.started_at,
                kind,
                slot: slot.text,
            }
        })
        .collect();
    let mut tasks: Vec<items::Task> = Vec::new();
    for item in s.items() {
        if tasks.iter().any(|t| t.id == item.workspace_id) {
            continue;
        }
        let record = around.workspaces.get(&item.workspace_id);
        tasks.push(items::Task {
            id: item.workspace_id.clone(),
            thread: item.thread_id.clone(),
            name: workspace_name(&item.workspace_id).unwrap_or_else(|| item.project.clone()),
            state: s.state_word(&item.workspace_id),
            computer: computer_of(&item.workspace_id),
            project: record.and_then(|w| w.pointer("/project/id")?.as_str()).map(str::to_owned),
        });
    }
    let open = match route {
        Some(Route::Thread { workspace_id, thread_id }) => Some(items::Open {
            workspace: workspace_id.clone(),
            thread: Some(thread_id.clone()),
            task: workspace_name(workspace_id).unwrap_or_default(),
            project: around.workspaces.get(workspace_id).and_then(|w| w.pointer("/project/id")?.as_str()).map(str::to_owned),
        }),
        _ => None,
    };
    let walk = open.as_ref().map_or(0, |o| super::switcher::walk::thread_walk(s, &o.workspace).len());
    let newest = |project: &str| {
        s.threads().iter().filter(|t| t.project_id.as_deref() == Some(project)).filter_map(|t| t.started_at).reduce(f64::max)
    };
    let leads: Vec<&str> = [s.prefs().project.as_deref(), open.as_ref().and_then(|o| o.project.as_deref())].into_iter().flatten().collect();
    let ordered: Vec<items::Project> = s
        .ordered_projects()
        .into_iter()
        .map(|(id, name)| {
            let at = around.project_computers.get(&id);
            let here = at.is_some_and(|c| c == "here");
            // `projectWhere`: this computer by its name and no glyph, another by its name or else its id.
            let computer = at.and_then(|c| place_name(c).or_else(|| (!here).then(|| c.clone()))).filter(|n| !n.is_empty());
            let workspace = folder_of(&around.workspaces, &id);
            items::Project { path: around.project_paths.get(&id).cloned().unwrap_or_default(), id, name, computer, here, workspace }
        })
        .collect();
    let projects = items::pick_order(ordered, &leads, newest);
    let asks = asks(s);
    let (acts, runs) = match &open {
        Some(o) => acts_of(s, o),
        None => (Vec::new(), Vec::new()),
    };
    let seen = Seen { connected, asks, projects, tasks, threads, open, walk, acts, labels: labels(rules, mac) };
    (seen, runs)
}

/// `newThreadAsks`: New thread asks for its project every time, and the host holds more than one to pick.
fn asks(s: &Sidebar) -> bool {
    let prefs = s.around().preferences.as_deref().unwrap_or(&Value::Null);
    prefs["newThreadIn"].as_str() == Some("ask") && s.around().projects.len() > 1
}

/// The workspace a project's New thread page opens on: the project's own folder, else any of its tasks.
fn folder_of(workspaces: &std::collections::HashMap<String, Value>, project: &str) -> Option<String> {
    let of = || workspaces.values().filter(|w| w.pointer("/project/id").and_then(Value::as_str) == Some(project));
    of().find(|w| matches!(w["kind"].as_str(), Some("local" | "place")) && w.get("worktree").is_none())
        .or_else(|| of().next())
        .and_then(|w| w["id"].as_str())
        .map(str::to_owned)
}

/// The open thread tree's moves and the open task's own acts, as `openRootMoves` and the registry's `actionItem` list
/// them: each a row of the Actions list, and the act the sidebar runs for it.
fn acts_of(s: &Sidebar, open: &items::Open) -> (Vec<items::Act>, Vec<(String, Act)>) {
    let mut rows = Vec::new();
    let mut runs = Vec::new();
    let Some(key) = open.thread.clone() else { return (rows, runs) };
    let mut root = key.clone();
    for _ in 0..s.threads().len() {
        match s.threads().iter().find(|t| t.key == root).and_then(|t| t.parent.clone()) {
            Some(up) => root = up,
            None => break,
        }
    }
    if let Some(place) = s.root_place(&root) {
        let first = (place.index == 0).then(|| "Already first".to_owned());
        let last = (place.index + 1 >= place.count).then(|| "Already last".to_owned());
        for (id, title, glyph, step, refusal) in [
            ("move-up", "Move up", Glyph::ArrowUp, model::Move::Up, first.clone()),
            ("move-down", "Move down", Glyph::ArrowDown, model::Move::Down, last),
            ("move-top", "Move to top", Glyph::ArrowUpToLine, model::Move::Top, first),
        ] {
            rows.push(items::Act {
                id: id.to_owned(),
                title: title.to_owned(),
                glyph,
                terms: Vec::new(),
                refusal,
                says: "The open thread's tree, in the sidebar".to_owned(),
                chord: None,
                index: runs.len(),
            });
            runs.push((root.clone(), Act::Move(step)));
        }
    }
    let folder = s.threads().iter().find(|t| t.key == key).is_some_and(|t| s.in_folder(t));
    let around = s.around();
    let state =
        around.workspaces.get(&open.workspace).map_or("running", |w| menu::workspace_state(w, around.statuses.get(&open.workspace)));
    for item in menu::workspace_items(s, &open.workspace) {
        if item.act == Act::NewThreadHere || folder && item.group == "remove" {
            continue;
        }
        let Some((id, glyph, terms, chord)) = registry(&item.act, state) else { continue };
        rows.push(items::Act {
            id: id.to_owned(),
            title: item.label.to_string(),
            glyph,
            terms: terms.iter().map(|t| (*t).to_owned()).collect(),
            refusal: item.refusal.clone(),
            says: open.task.clone(),
            chord,
            index: runs.len(),
        });
        runs.push((key.clone(), item.act));
    }
    (rows, runs)
}

/// A task act's id, glyph, search words and chord in `workspaceActions.ts`; the phase's glyph reads the state its
/// label reads, `state` as `menu::workspace_state` gives it.
fn registry(act: &Act, state: &str) -> Option<(&'static str, Glyph, &'static [&'static str], Option<&'static str>)> {
    Some(match act {
        Act::Phase => {
            let glyph = match state {
                "waking" => Glyph::Square,
                "running" | "unreachable" => Glyph::Pause,
                _ => Glyph::Play,
            };
            ("phase", glyph, &["pause", "nap", "sleep", "wake", "resume", "start", "stop waking"], None)
        }
        Act::Rebuild => ("rebuild", Glyph::RefreshCw, &["rebuild task", "zombie", "gone", "replace"], None),
        Act::Reconnect => ("start-daemon", Glyph::Play, &["start daemon", "daemon", "start it", "restart daemon", "reconnect"], None),
        Act::OpenTerminal => ("open-terminal", Glyph::SquareTerminal, &["open terminal", "new terminal", "shell"], Some("terminal.toggle")),
        Act::OpenBrowser => ("open-browser", Glyph::Globe, &["open browser", "preview", "ports"], Some("preview.toggle")),
        Act::BringBack => {
            ("bring-back", Glyph::GitPullRequestArrow, &["bring back", "pull request", "push the branch", "open a pull request"], None)
        }
        Act::ExportProject => ("export-project", Glyph::FolderOutput, &["export project", "export folder", "download", "bring home"], None),
        Act::RenameTask => ("rename", Glyph::Pencil, &["rename task"], None),
        Act::Fork => ("fork", Glyph::GitFork, &["fork task", "run a copy", "duplicate", "clone"], None),
        Act::CopyComputerId(_) => ("copy-id", Glyph::Copy, &["copy computer id", "copy id"], None),
        Act::DeleteTask => ("delete", Glyph::Trash, &["delete task", "remove task", "throw away"], None),
        Act::ForgetTask => ("forget", Glyph::Trash, &["forget task", "delete task", "remove"], None),
        _ => return None,
    })
}

fn js_now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0., |d| d.as_secs_f64() * 1000.)
}

/// A row's lead at 16 px.
fn lead(lead: &Lead, sidebar: Option<&Sidebar>, cx: &App) -> Option<AnyElement> {
    let p = cx.palette();
    Some(match lead {
        Lead::Glyph(glyph) => frame::row_icon(*glyph, cx).flex_none().into_any_element(),
        Lead::Harness(harness) => catalog::mark(harness, &catalog::name(harness), px(16.), p.muted_foreground, cx),
        Lead::Project(project) => {
            let (prefs, images) = match sidebar {
                Some(s) => (s.around().preferences.as_deref().unwrap_or(&Value::Null), Some(&s.around().images)),
                None => (&Value::Null, None),
            };
            match images {
                Some(images) => looks::project_glyph(project.as_deref(), prefs, images, px(16.), p.muted_foreground, true, p),
                None => Icon::new(Glyph::Folder).with_size(px(16.)).text_color(p.muted_foreground).into_any_element(),
            }
        }
        Lead::None => return None,
    })
}

/// What a row says under its title, 12 px muted.
fn says(says: &Description, cx: &App) -> AnyElement {
    let p = cx.palette();
    match says {
        Description::Words(words) => div().child(words.clone()).into_any_element(),
        Description::Facts(facts) => h_flex()
            .min_w_0()
            .overflow_hidden()
            .gap_x_3()
            .children(facts.iter().map(|f| div().flex_none().whitespace_nowrap().child(f.clone())))
            .into_any_element(),
        Description::Where { computer, here, path } => h_flex()
            .min_w_0()
            .overflow_hidden()
            .gap_x_3()
            .children(computer.clone().map(|name| {
                h_flex()
                    .flex_none()
                    .gap_1()
                    .when(!here, |el| el.child(Icon::new(Glyph::Monitor).with_size(px(12.)).text_color(p.muted_foreground)))
                    .child(div().max_w(px(240.)).truncate().child(name))
            }))
            .child(div().flex_none().whitespace_nowrap().child(path.clone()))
            .into_any_element(),
    }
}

fn trailing(trailing: &Trailing, value: &SharedString, cx: &App) -> AnyElement {
    let p = cx.palette();
    match trailing {
        Trailing::Current => div()
            .flex_none()
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .text_color(p.muted_foreground)
            .child("Current task")
            .into_any_element(),
        Trailing::Status(kind, slot) => h_flex()
            .flex_none()
            .min_w(px(16.))
            .justify_end()
            .child(
                crate::ui::StatusMark::new(tile::status_of(*kind, slot.clone()))
                    .glyph_ink(p.muted_foreground)
                    .key(SharedString::from(format!("palette-{value}"))),
            )
            .into_any_element(),
        Trailing::Pick(label) => div()
            .flex_none()
            .ml_auto()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .text_color(p.muted_foreground)
            .child(label.clone())
            .into_any_element(),
    }
}

impl Render for Palette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let sidebar = self.sidebar.as_ref().and_then(|s| s.upgrade());
        let sidebar = sidebar.as_ref().map(|s| s.read(cx));
        let active = self.lit().map(|i| i.value.clone());
        let me = cx.weak_entity();
        let viewport = window.viewport_size();
        let width = (viewport.width - px(32.)).min(frame::POPUP_WIDTH);
        // The popup's width less its edges and the list's `p-2`: each row's own, so no row sizes itself off its text.
        let row_width = width - px(2.) - px(16.);
        let mut rows: Vec<AnyElement> = Vec::new();
        for (at, group) in self.groups.iter().enumerate() {
            rows.push(
                frame::group_label(group.label, cx).when(at > 0, |el| el.mt(frame::GROUP_GAP)).w(row_width).flex_none().into_any_element(),
            );
            for item in &group.items {
                let mut row = Row::new(SharedString::from(format!("palette-row-{}", item.value)), item.title.clone())
                    .page(item.page)
                    .held(item.held)
                    .active(active.as_ref() == Some(&item.value));
                if let Some(icon) = lead(&item.lead, sidebar, cx) {
                    row = row.icon(icon);
                }
                if let Some(s) = &item.says {
                    row = row.description(says(s, cx));
                }
                if let Some(t) = &item.trailing {
                    row = row.trailing(trailing(t, &item.value, cx));
                }
                if let Some(shortcut) = &item.shortcut {
                    row = row.shortcut(shortcut.clone());
                }
                let (pick, hover) = (item.value.clone(), item.value.clone());
                let (me, me2) = (me.clone(), me.clone());
                rows.push(
                    row.render(cx)
                        .w(row_width)
                        .flex_none()
                        .debug_selector(|| format!("palette-row-{}", hover))
                        .when(!item.held, |el| {
                            el.on_mouse_move(move |_, _, cx| {
                                _ = me.update(cx, |this, cx| {
                                    if this.highlighted.as_ref() != Some(&hover) {
                                        this.highlighted = Some(hover.clone());
                                        cx.notify();
                                    }
                                });
                            })
                            .on_click(move |_, window, cx| {
                                // The row as listed now: an act's place moves as the sidebar's list is read again.
                                _ = me2.update(cx, |this, cx| {
                                    let item = this.live().find(|i| i.value == pick).cloned();
                                    if let Some(item) = item {
                                        this.execute(item, window, cx);
                                    }
                                });
                            })
                        })
                        .into_any_element(),
                );
            }
        }
        let list: AnyElement = if self.groups.is_empty() {
            frame::empty(items::nothing(&self.query, self.page), cx).into_any_element()
        } else {
            // `scrollFade`: the list fades out over up to 24 px at an edge with more of it past that edge, drawn as
            // the popover's own fill over the rows since this base has no masks.
            let above = (-self.scroll.offset().y).max(px(0.)).min(FADE_EDGE);
            let below = (self.scroll.max_offset().y + self.scroll.offset().y).max(px(0.)).min(FADE_EDGE);
            let fade = |angle: f32| linear_gradient(angle, linear_color_stop(p.popover, 0.), linear_color_stop(p.popover.opacity(0.), 1.));
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_shrink(1.)
                .min_h_0()
                .child(
                    div()
                        .id("palette-list")
                        .min_h_0()
                        .max_h((viewport.height * 0.7).min(px(448.)))
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .debug_selector(|| "palette-list".into())
                        .flex()
                        .flex_col()
                        .p_2()
                        .children(rows),
                )
                .when(above > px(0.), |el| el.child(div().absolute().top_0().left_0().right_0().h(above).bg(fade(180.))))
                .when(below > px(0.), |el| el.child(div().absolute().bottom_0().left_0().right_0().h(below).bg(fade(0.))))
                .into_any_element()
        };
        let back: Option<AnyElement> = self.page.then(|| {
            let me = me.clone();
            // `transition-colors duration-150 hover:bg-accent [&_svg]:hover:text-foreground`.
            let hovered = motion::hovered("palette-back", window, cx);
            let over = *hovered.read(cx);
            let fill =
                motion::ease_ink("palette-back", "fill", if over { p.accent } else { p.accent.opacity(0.) }, motion::COLORS, window, cx);
            let ink =
                motion::ease_ink("palette-back", "ink", if over { p.foreground } else { p.muted_foreground }, motion::COLORS, window, cx);
            div()
                .id("palette-back")
                .debug_selector(|| "palette-back".into())
                .flex()
                .m(px(-4.))
                .px(px(2.))
                .py_1()
                .rounded(frame::RADIUS_MD)
                .cursor_pointer()
                .bg(fill)
                .on_hover(motion::track_hover(hovered))
                .child(Icon::new(Glyph::ArrowLeft).with_size(px(16.)).text_color(ink))
                .on_click(move |_, window, cx| {
                    _ = me.update(cx, |this, cx| this.back(window, cx));
                })
                .into_any_element()
        });
        let field = Input::new(&self.input).appearance(false).p_0().h(px(34.)).text_size(text::SETTING_TITLE.0);
        let close = me.clone();
        div()
            .id("palette")
            .key_context(CONTEXT)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .capture_action(cx.listener(Self::on_up))
            .capture_action(cx.listener(Self::on_down))
            .capture_action(cx.listener(Self::on_enter))
            .capture_action(cx.listener(Self::on_escape))
            .capture_action(cx.listener(Self::on_backspace))
            .capture_action(cx.listener(Self::on_pick))
            .on_action(cx.listener(Self::on_close))
            .on_action(cx.listener(|_, _: &KeepFocus, _, _| {}))
            // `onBackdropPointerDown`: a press of any button.
            .child(motion::open_fade(
                "palette-scrim-open",
                frame::scrim("palette-scrim", cx).debug_selector(|| "palette-scrim".into()).on_any_mouse_down(move |_, window, cx| {
                    _ = close.update(cx, |this, cx| this.close(window, cx));
                }),
                cx,
            ))
            .child(
                frame::column(window).child(motion::open_fade(
                    "palette-popup-open",
                    frame::popup(cx)
                        .id("palette-popup")
                        .debug_selector(|| "palette-popup".into())
                        .w(width)
                        .occlude()
                        .child(frame::search_row(back, field, cx))
                        .child(list)
                        .child(frame::footer(frame::hints(if self.page { "Select" } else { "Run" }), None, window, cx)),
                    cx,
                )),
            )
    }
}

/// Opens the palette, at its root or on the page of projects, unless it stands open already.
fn show(page: bool, window: &mut Window, cx: &mut App) -> bool {
    let Some(overlays) = overlays::of(window, cx) else { return false };
    let weak = overlays.downgrade();
    overlays.update(cx, |o, cx| {
        if o.palette.is_some() {
            return false;
        }
        o.take_focus(window, cx);
        let (host, sidebar) = (o.host.clone(), o.sidebar.clone());
        o.palette = Some(cx.new(|cx| Palette::new(weak, host, sidebar, page, window, cx)));
        cx.notify();
        true
    })
}

/// Opens the palette at rest, or closes it where it stands open, as mod+k does.
pub fn toggle(window: &mut Window, cx: &mut App) {
    if is_open(window, cx) {
        close(window, cx);
    } else {
        show(false, window, cx);
    }
}

/// Opens the palette at rest, as the sidebar's Search does.
pub fn open(window: &mut Window, cx: &mut App) {
    show(false, window, cx);
}

/// `openNewThread`'s ask: where New thread asks for its project every time and there is more than one, opens the
/// palette on the page of projects and answers true; otherwise leaves New thread to its caller.
pub fn new_thread(window: &mut Window, cx: &mut App) -> bool {
    let Some(overlays) = overlays::of(window, cx) else { return false };
    let sidebar = overlays.read(cx).sidebar.as_ref().and_then(|s| s.upgrade());
    if !sidebar.is_some_and(|s| asks(s.read(cx))) {
        return false;
    }
    match overlays.read(cx).palette.clone() {
        Some(palette) => palette.update(cx, |p, cx| {
            if !p.page {
                p.open_page(window, cx);
            }
        }),
        None => _ = show(true, window, cx),
    }
    true
}

pub fn is_open(window: &Window, cx: &App) -> bool {
    overlays::of(window, cx).is_some_and(|o| o.read(cx).palette.is_some())
}

/// Shuts the palette, if it stands open, and gives the keys back.
pub fn close(window: &mut Window, cx: &mut App) {
    let Some(overlays) = overlays::of(window, cx) else { return };
    overlays.update(cx, |o, cx| {
        if o.palette.take().is_some() {
            o.give_back(window, cx);
            cx.notify();
        }
    });
}
