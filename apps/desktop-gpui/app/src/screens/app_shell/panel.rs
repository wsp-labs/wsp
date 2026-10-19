// SPDX-License-Identifier: AGPL-3.0-only
//! The right panel: its tabs, the launcher it shows with none open, and its width. Whether it is open, its tabs and
//! the active one are kept per thread, as the web app's `rightPanelStore.ts` keeps them, and with its one width in the
//! app's store. Each tab's screen lives only while the panel shows on its thread and finds its session again by the
//! tab's id.

use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnchoredPositionMode, AnyView, App, Context, FocusHandle, Focusable, Hsla,
    KeyDownEvent, MouseButton, Pixels, ScrollHandle, SharedString, StyleRefinement, TestSupportExt as _, Window,
};

use serde_json::{json, Value};

use super::{Entry, Host, Opened, Pane, Place, Title};
use crate::local_store::LocalStore;
use crate::screens;
use crate::screens::terminal::hub;
use crate::ui::card::keycap;
use crate::ui::{motion, ActivePalette};

/// `PreviewPanelShell`'s width: 540 at first, never under 360.
pub const DEFAULT_WIDTH: Pixels = px(540.);
pub const MIN_WIDTH: Pixels = px(360.);
/// The tab bar, the header row's `--workspace-topbar-height`.
pub const BAR_HEIGHT: Pixels = px(52.);
/// How long after the add menu closes on an outside press a click still belongs to that press.
const MENU_CLOSE: Duration = Duration::from_millis(500);
/// How many threads keep their panel's tabs; the oldest is forgotten past it, as `KEPT_THREAD_PANELS` in the web app.
const KEPT: usize = 100;

/// What the panel is open on, for one thread or for the computer itself: each tab's pane and id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Saved {
    pub open: bool,
    pub tabs: Vec<(Pane, SharedString)>,
    pub active: Option<usize>,
}

/// `RIGHT_PANEL_STORAGE_KEY` and its version, and `RIGHT_PANEL_WIDTH_STORAGE_KEY`: the keys today's app keeps the
/// panel under in localStorage, which the app's store keeps here.
pub const STATE_KEY: &str = "wsp:right-panel-state:v1";
const STATE_VERSION: u32 = 2;
pub const WIDTH_KEY: &str = "wsp:right-panel-width";

/// `key` as the store keeps it for the host the window is on: today's page loads from each host's own address, so
/// its localStorage is that host's alone; this computer's keeps the bare key.
pub fn stored_key(key: &str, host: Option<&str>) -> String {
    host.map_or_else(|| key.to_owned(), |alias| format!("host:{alias}:{key}"))
}

/// A pane's `RightPanelKind`.
fn kind(pane: Pane) -> &'static str {
    match pane {
        Pane::Browser => "preview",
        Pane::Terminal => "terminal",
        Pane::Changes => "diff",
        Pane::PullRequest => "pr",
        Pane::Files => "files",
        Pane::Computer => "machine",
        Pane::Processes => "processes",
        Pane::Agents => "agents",
        Pane::Aside => "aside",
        Pane::Slate => "slate",
    }
}

/// A place's record as `WorkspaceRightPanelState` keeps it; the side question is `unkept`. A terminal tab's shells are
/// the host's ids its surface holds, every shell of a split, and none is written for a tab not built since launch.
fn record_of(saved: &Saved, cx: &App) -> Value {
    let tabs: Vec<&(Pane, SharedString)> = saved.tabs.iter().filter(|(pane, _)| *pane != Pane::Aside).collect();
    let surfaces: Vec<Value> = tabs
        .iter()
        .map(|(pane, id)| {
            let rest = id.split_once(':').map(|(_, rest)| rest);
            match pane {
                Pane::Browser => json!({ "id": id, "kind": "preview", "resourceId": rest.filter(|r| *r != "new") }),
                Pane::Terminal => {
                    let mut record = json!({ "id": id, "kind": "terminal", "resourceId": rest.unwrap_or_default() });
                    if let Some(ui) = hub::surface_read(id, cx).filter(|ui| !ui.terminal_ids.is_empty()) {
                        record["terminalIds"] = json!(ui.terminal_ids);
                        record["activeTerminalId"] = json!(ui.active_terminal_id);
                    }
                    record
                }
                Pane::Files => match id.strip_prefix("file:") {
                    Some(path) => json!({ "id": id, "kind": "files", "path": path, "line": null, "reveal": 0 }),
                    None => json!({ "id": "files", "kind": "files", "path": null }),
                },
                pane => json!({ "id": kind(*pane), "kind": kind(*pane) }),
            }
        })
        .collect();
    let active = saved.active.and_then(|ix| saved.tabs.get(ix)).and_then(|t| tabs.iter().position(|k| *k == t));
    let active = active.and_then(|ix| surfaces[ix].get("id").cloned()).unwrap_or(Value::Null);
    json!({ "isOpen": saved.open, "activeSurfaceId": active, "surfaces": surfaces })
}

/// Each place's record read back as `migratePersistedRightPanelState` reads it: a surface of a kind or shape it does
/// not know is dropped, and the active one is the first when the kept id names none.
fn kept_of(value: &Value) -> Vec<(String, Saved)> {
    let Some(by) = value.pointer("/state/byWorkspaceId").and_then(Value::as_object) else { return Vec::new() };
    by.iter()
        .map(|(key, record)| {
            let surfaces = record.get("surfaces").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
            let read: Vec<(Pane, SharedString, &str)> = surfaces
                .iter()
                .filter_map(|surface| {
                    let pane = Pane::ALL.into_iter().find(|p| Some(kind(*p)) == surface.get("kind").and_then(Value::as_str))?;
                    let id: SharedString = match pane {
                        Pane::Aside => return None,
                        Pane::Browser => match surface.get("resourceId") {
                            Some(Value::String(tab)) => format!("browser:{tab}").into(),
                            None | Some(Value::Null) => "browser:new".into(),
                            _ => return None,
                        },
                        Pane::Terminal => {
                            let pty = surface.get("resourceId").and_then(Value::as_str);
                            let first = surface.get("terminalIds").and_then(Value::as_array).and_then(|t| t.first()?.as_str());
                            format!("terminal:{}", pty.or(first)?).into()
                        }
                        Pane::Files => match surface.get("path") {
                            Some(Value::String(path)) => format!("file:{path}").into(),
                            None | Some(Value::Null) => "files".into(),
                            _ => return None,
                        },
                        pane => pane.name().into(),
                    };
                    Some((pane, id, surface.get("id").and_then(Value::as_str).unwrap_or_default()))
                })
                .collect();
            let wanted = record.get("activeSurfaceId").and_then(Value::as_str);
            let active = read.iter().position(|(_, _, id)| Some(*id) == wanted).or((!read.is_empty()).then_some(0));
            let open = record.get("isOpen").and_then(Value::as_bool).unwrap_or(false);
            (key.clone(), Saved { open, tabs: read.into_iter().map(|(pane, id, _)| (pane, id)).collect(), active })
        })
        .collect()
}

/// Where the panel is: a thread on a workspace that runs or not and has a pull request or not, the workspace's New
/// thread page as the thread with no id (`threadStateKey(ws, "")`), or the computer with no thread picked.
#[derive(Clone, Debug, PartialEq)]
pub enum At {
    Here,
    Thread { workspace_id: String, thread_id: String, running: bool, pr: bool },
}

impl Pane {
    /// The launcher's line under the label.
    pub fn description(self) -> &'static str {
        match self {
            Pane::Browser => "Open your dev server or a URL.",
            Pane::Terminal => "Start a shell here.",
            Pane::Changes => "Review the changes here.",
            Pane::PullRequest => "Checks, review and comments.",
            Pane::Files => "Browse and read the files here.",
            Pane::Computer => "Load, memory and disk.",
            Pane::Processes => "Inspect and kill what runs.",
            Pane::Agents => "Agents, skills and servers here.",
            Pane::Aside => "A question the thread never keeps.",
            Pane::Slate => "What the agent built for this thread.",
        }
    }

    pub fn glyph(self) -> IconName {
        match self {
            Pane::Browser => IconName::Earth,
            Pane::Terminal => IconName::SquareTerminal,
            Pane::Changes => IconName::FileDiff,
            Pane::PullRequest => IconName::GitPullRequest,
            Pane::Files => IconName::Folder,
            Pane::Computer => IconName::Cpu,
            Pane::Processes => IconName::Activity,
            Pane::Agents => IconName::Bot,
            Pane::Aside => IconName::MessageSquareDashed,
            Pane::Slate => IconName::PanelsTopLeft,
        }
    }

    /// The name a screen mounted in this pane registers under in [`crate::screens::screen`].
    pub fn name(self) -> &'static str {
        match self {
            Pane::Browser => "browser",
            Pane::Terminal => "terminal",
            Pane::Changes => "changes",
            Pane::PullRequest => "pull-request",
            Pane::Files => "files",
            Pane::Computer => "computer",
            Pane::Processes => "processes",
            Pane::Agents => "agents",
            Pane::Aside => "aside",
            Pane::Slate => "slate",
        }
    }

    /// Listed by the launcher only while it can open: the pull request and the side question.
    fn only_when_available(self) -> bool {
        matches!(self, Pane::PullRequest | Pane::Aside)
    }

    /// Whether the pane opens here, and the line it carries when it does not, as `panes.ts` decides both.
    pub fn availability(self, at: &At) -> Result<(), &'static str> {
        let (here, running, thread, pr) = match at {
            At::Here => (true, false, false, false),
            At::Thread { running, thread_id, pr, .. } => (false, *running, !thread_id.is_empty(), *pr),
        };
        let open = match self {
            Pane::Browser | Pane::Terminal | Pane::Processes => here || running,
            Pane::Changes | Pane::Files => running,
            Pane::Computer => true,
            Pane::Agents => !here,
            Pane::Slate => thread,
            Pane::PullRequest => pr,
            Pane::Aside => false,
        };
        if open {
            return Ok(());
        }
        Err(match self {
            Pane::Changes if here => "Pick a project to review its changes.",
            Pane::Files if here => "Pick a thread to read its files.",
            Pane::Slate if !thread => "Pick a thread to see its slate.",
            Pane::Browser | Pane::Terminal | Pane::Files | Pane::Processes => "Available while this runs.",
            Pane::Changes => "Review the changes once it runs.",
            Pane::PullRequest => "Available once the work has a pull request.",
            Pane::Computer | Pane::Agents => "Pick a thread to use it.",
            Pane::Aside => "Ask one with /btw in the composer.",
            Pane::Slate => "The agent's panel for the open thread.",
        })
    }
}

struct Tab {
    pane: Pane,
    id: SharedString,
    /// The pane's screen, or None until a screen registers under the pane's name.
    view: Option<AnyView>,
    title: Option<Title>,
}

impl Tab {
    /// A file's tab, which `panes.ts` titles by the file's name under a file glyph.
    fn file(&self) -> Option<&str> {
        self.id.strip_prefix("file:").filter(|_| self.pane == Pane::Files)
    }

    /// The name and glyph the strip draws: the screen's own title once built, else the file's name or the pane's.
    fn face(&self, cx: &App) -> (SharedString, IconName) {
        let named = || self.file().map_or_else(|| SharedString::new_static(self.pane.label()), screens::pr_pane::files::file_tab::title);
        let label = self.title.as_ref().map_or_else(named, |title| title(cx));
        (label, if self.file().is_some() { IconName::File } else { self.pane.glyph() })
    }
}

pub struct RightPanel {
    at: At,
    open: bool,
    /// Hidden by a page over the thread, as Settings hides it, whatever `open` keeps for the thread.
    hidden: bool,
    tabs: Vec<Tab>,
    active: Option<usize>,
    width: Pixels,
    kept: Vec<(String, Saved)>,
    /// The store's keys for the panel's records and its width, on the host the window is on.
    keys: (String, String),
    /// The last tab this panel numbered, so each Browser and Terminal opened from the launcher is a page or a shell of
    /// its own.
    minted: u64,
    strip: ScrollHandle,
    host: Option<Host>,
    /// The add menu under the bar's plus, open.
    adding: bool,
    /// The launcher's highlighted row among the panes that open here, which the arrows and the pointer move.
    lit: Option<usize>,
    closed_at: Option<Instant>,
    focus: FocusHandle,
    /// The host's agent writes, each of which opens its thread's Slate tab once.
    _slates: Option<gpui_kit::Task<()>>,
}

impl Focusable for RightPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl RightPanel {
    /// A panel on the computer's own place, with the width and each place's tabs the app's store keeps for `host`, the
    /// alias of the host the window is on (none for this computer).
    pub fn new(host: Option<String>, cx: &mut Context<Self>) -> Self {
        let keys = (stored_key(STATE_KEY, host.as_deref()), stored_key(WIDTH_KEY, host.as_deref()));
        let store = cx.try_global::<LocalStore>();
        let width = store.and_then(|s| s.get_as::<f64>(&keys.1)).filter(|w| w.is_finite()).map_or(DEFAULT_WIDTH, |w| px(w as f32));
        let mut kept = store.and_then(|s| s.get(&keys.0)).map(kept_of).unwrap_or_default();
        let minted =
            kept.iter().flat_map(|(_, s)| &s.tabs).filter_map(|(_, id)| id.rsplit(':').next()?.parse::<u64>().ok()).max().unwrap_or(0);
        let here = kept.iter().position(|(k, _)| k == hub::HERE).map(|ix| kept.remove(ix).1).unwrap_or_default();
        RightPanel {
            at: At::Here,
            open: here.open,
            hidden: false,
            active: here.active.filter(|&ix| ix < here.tabs.len()),
            tabs: here.tabs.into_iter().map(|(pane, id)| Tab { pane, id, view: None, title: None }).collect(),
            width,
            kept,
            keys,
            minted,
            strip: ScrollHandle::new(),
            host: None,
            adding: false,
            lit: None,
            closed_at: None,
            focus: cx.focus_handle(),
            _slates: None,
        }
    }

    pub fn set_host(&mut self, host: Host, window: &mut Window, cx: &mut Context<Self>) {
        let (follow, mut writes) = screens::slate::agent_writes(&host);
        self._slates = Some(cx.spawn_in(window, async move |this, cx| {
            let _follow = follow;
            while let Some(thread) = futures::StreamExt::next(&mut writes).await {
                let on = |at: &At| matches!(at, At::Thread { thread_id, .. } if *thread_id == thread);
                if this
                    .update_in(cx, |this, window, cx| {
                        if on(&this.at) {
                            this.show_slate_once(window, cx)
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.host = Some(host);
        self.mount_active(window, cx);
        self.show_slate_once(window, cx);
        cx.notify();
    }

    /// Opens the Slate tab once on the thread here when its slate was never shown, as the web's `showOnce` does as a
    /// thread is picked and as its agent writes.
    fn show_slate_once(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (At::Thread { thread_id, .. }, Some(host)) = (&self.at, &self.host) else { return };
        if thread_id.is_empty() {
            return;
        }
        let (thread, never) = (thread_id.clone(), screens::slate::never_shown(host, thread_id.clone()));
        cx.spawn_in(window, async move |this, cx| {
            if never.await {
                _ = this.update_in(cx, |this, window, cx| {
                    let here = matches!(&this.at, At::Thread { thread_id, .. } if *thread_id == thread);
                    if here && screens::slate::first_showing(&thread, cx) {
                        this.add(Pane::Slate, None, window, cx);
                    }
                });
            }
        })
        .detach();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Hides the panel under a page or shows it again; hidden, it holds no pane's screen, as a shut one holds none.
    pub fn set_hidden(&mut self, hidden: bool, window: &mut Window, cx: &mut Context<Self>) {
        if hidden == self.hidden {
            return;
        }
        self.hidden = hidden;
        if hidden {
            self.unmount();
        } else {
            self.mount_active(window, cx);
        }
        cx.notify();
    }

    /// The active tab's screen while it is built, as a test reaches into it.
    pub fn view(&self) -> Option<AnyView> {
        self.active.and_then(|ix| self.tabs.get(ix)).and_then(|t| t.view.clone())
    }

    /// Each tab's name and glyph, as the strip draws them.
    pub fn faces(&self, cx: &App) -> Vec<(SharedString, IconName)> {
        self.tabs.iter().map(|t| t.face(cx)).collect()
    }

    pub fn panes(&self) -> Vec<Pane> {
        self.tabs.iter().map(|t| t.pane).collect()
    }

    /// Each tab's pane and id, in the strip's order.
    pub fn tabs(&self) -> Vec<(Pane, SharedString)> {
        self.tabs.iter().map(|t| (t.pane, t.id.clone())).collect()
    }

    pub fn active(&self) -> Option<Pane> {
        self.active.map(|ix| self.tabs[ix].pane)
    }

    pub fn width(&self) -> Pixels {
        self.width
    }

    pub fn set_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.width = width;
        cx.notify();
    }

    /// Writes the width a drag ended on to the app's store, as `useResizableWidth` writes it on the pointer's release.
    pub fn keep_width(&self, cx: &mut App) {
        if cx.has_global::<LocalStore>() {
            if let Err(e) = cx.global_mut::<LocalStore>().set(&self.keys.1, serde_json::json!(f32::from(self.width))) {
                crate::desktop::log::error(&format!("panel width not kept: {e}"));
            }
        }
    }

    /// Writes every place's panel to the app's store in the web's persisted shape, each change as zustand's `persist`
    /// writes it; a fresh place is left out, as `updatePanel` drops an empty record.
    fn keep(&self, cx: &mut App) {
        if !cx.has_global::<LocalStore>() {
            return;
        }
        let here = (Self::key(&self.at), self.saved());
        let places = self.kept.iter().filter(|(k, _)| *k != here.0).chain([&here]).filter(|(_, s)| *s != Saved::default());
        let by: serde_json::Map<String, serde_json::Value> = places.map(|(k, s)| (k.clone(), record_of(s, cx))).collect();
        let value = serde_json::json!({ "state": { "byWorkspaceId": by }, "version": STATE_VERSION });
        if cx.global::<LocalStore>().get(&self.keys.0) == Some(&value) {
            return;
        }
        if let Err(e) = cx.global_mut::<LocalStore>().set(&self.keys.0, value) {
            crate::desktop::log::error(&format!("panel not kept: {e}"));
        }
    }

    fn saved(&self) -> Saved {
        Saved { open: self.open, tabs: self.tabs(), active: self.active }
    }

    /// `stateKeyOf`: `place:here` for the computer's own panel, which the cap does not count, `<ws>#<thread>` for a
    /// thread's, `<ws>#` for the New thread page's.
    fn key(at: &At) -> String {
        match at {
            At::Here => hub::HERE.to_owned(),
            At::Thread { workspace_id, thread_id, .. } => format!("{workspace_id}#{thread_id}"),
        }
    }

    /// The New thread page's first send: the page's panel becomes the new thread's, as `carryNewThreadPanel` moves it.
    /// A move of `at` that keeps the key, its workspace starting to run or its pull request landing, only redraws.
    pub fn follow(&mut self, at: At, cx: &mut Context<Self>) {
        if at == self.at {
            return;
        }
        let key = Self::key(&at);
        self.kept.retain(|(k, _)| *k != key);
        self.at = at;
        self.keep(cx);
        cx.notify();
    }

    /// Moves the panel to `at`, keeping what the last place had open and restoring what `at` had; a place it has not
    /// seen starts closed, as a thread's and the computer's do in the web app, and so does the New thread page each
    /// time it is shown (`followThreadPanels`).
    pub fn show_for(&mut self, at: At, window: &mut Window, cx: &mut Context<Self>) {
        if Self::key(&at) == Self::key(&self.at) {
            self.at = at;
            cx.notify();
            return;
        }
        let key = Self::key(&self.at);
        let saved = self.saved();
        self.kept.retain(|(k, _)| *k != key);
        // A panel left as a fresh one starts is not kept, as the web app drops an empty record.
        if saved != Saved::default() {
            self.kept.push((key, saved));
            if self.kept.iter().filter(|(k, _)| k.contains('#')).count() > KEPT {
                if let Some(oldest) = self.kept.iter().position(|(k, _)| k.contains('#')) {
                    self.kept.remove(oldest);
                }
            }
        }
        self.keep_focus(window, cx);
        let key = Self::key(&at);
        if matches!(&at, At::Thread { thread_id, .. } if thread_id.is_empty()) {
            self.kept.retain(|(k, _)| *k != key);
        }
        let saved = self.kept.iter().find(|(k, _)| *k == key).map(|(_, s)| s.clone()).unwrap_or_default();
        self.at = at;
        self.open = saved.open;
        self.tabs = saved.tabs.into_iter().map(|(pane, id)| Tab { pane, id, view: None, title: None }).collect();
        self.active = saved.active.filter(|&ix| ix < self.tabs.len());
        self.mount_active(window, cx);
        self.show_slate_once(window, cx);
        self.keep(cx);
        cx.notify();
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    /// `toggle` of `rightPanelStore.ts`: closes the panel when it is open on `pane`, else opens `pane`, the browser on
    /// its first page or a new one, as `openedSurface` picks it.
    pub fn toggle_pane(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        if self.open && self.active() == Some(pane) {
            self.open = false;
            self.unmount();
            self.keep(cx);
            cx.notify();
        } else {
            let page = (pane == Pane::Browser)
                .then(|| self.tabs.iter().find(|t| t.pane == pane).map_or_else(|| "browser:new".into(), |t| t.id.clone()));
            self.add(pane, page, window, cx);
        }
    }

    /// Focus inside a tab's screen that is about to go stays in the panel, where the panel's own keys still hear it.
    fn keep_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus.contains_focused(window, cx) {
            self.focus.focus(window, cx);
        }
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = !self.open;
        self.lit = None;
        if self.open {
            self.mount_active(window, cx);
            // The launcher takes focus as it opens, so its letters answer at once.
            if self.active.is_none() {
                self.focus.focus(window, cx);
            }
        } else {
            self.unmount();
        }
        self.keep(cx);
        cx.notify();
    }

    /// A pane's letter typed while the launcher shows and focus is in the panel opens that pane, as the web app's
    /// launcher answers it; a letter with a modifier is a chord, not the launcher's.
    fn launch_by_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let m = &event.keystroke.modifiers;
        if (self.active.is_some() && !self.adding) || m.control || m.alt || m.platform || m.function {
            return;
        }
        if self.active.is_none() && self.step_launcher(&event.keystroke.key, window, cx) {
            cx.stop_propagation();
            return;
        }
        let Some(pane) = Pane::ALL.into_iter().find(|p| {
            p.key().is_some_and(|k| event.keystroke.key.eq_ignore_ascii_case(&k.to_string())) && p.availability(&self.at).is_ok()
        }) else {
            return;
        };
        cx.stop_propagation();
        self.add(pane, None, window, cx);
    }

    /// The launcher's arrows and Enter, as `RightPanelTabs.tsx` reads them: down and right step the highlight on over
    /// the panes that open here, up and left back, wrapping, and Enter opens the highlighted pane.
    fn step_launcher(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let open: Vec<Pane> = Pane::ALL.into_iter().filter(|p| p.availability(&self.at).is_ok()).collect();
        let n = open.len();
        if n == 0 {
            return false;
        }
        let lit = self.lit.map(|ix| ix.min(n - 1));
        match key {
            "down" | "right" => self.lit = Some(lit.map_or(0, |ix| (ix + 1) % n)),
            "up" | "left" => self.lit = Some(lit.map_or(n - 1, |ix| (ix + n - 1) % n)),
            "enter" => match lit {
                Some(ix) => self.add(open[ix], None, window, cx),
                None => return false,
            },
            _ => return false,
        }
        cx.notify();
        true
    }

    /// The id a tab opened from the launcher or the add menu takes, as the panes' openers make it: a new page for each
    /// Browser and a new shell for each Terminal, as `openNewBrowser` and `openPanelTerminal` do, the files tree, and
    /// the pane's own name for the rest.
    fn mint(&mut self, pane: Pane) -> SharedString {
        match pane {
            Pane::Browser | Pane::Terminal => loop {
                self.minted += 1;
                let id = SharedString::from(format!("{}:{}", pane.name(), self.minted));
                if !self.tabs.iter().any(|t| t.id == id) {
                    break id;
                }
            },
            Pane::Files => "files".into(),
            _ => pane.name().into(),
        }
    }

    /// Opens `pane` on the tab `id` names, or on the one [`Self::mint`] gives, moving to that tab when it is open: a
    /// tab is the same tab only when its pane and its id both are.
    pub fn add(&mut self, pane: Pane, id: Option<SharedString>, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.adding = false;
        self.lit = None;
        let id = id.unwrap_or_else(|| self.mint(pane));
        // `openBrowser`: a browser tab on a page takes the place of the one placeholder.
        if pane == Pane::Browser && id != "browser:new" {
            if let Some(ix) = self.tabs.iter().position(|t| t.pane == pane && t.id == "browser:new") {
                self.tabs.remove(ix);
                self.active = self.active.filter(|&a| a != ix).map(|a| if a > ix { a - 1 } else { a });
            }
        }
        let ix = match self.tabs.iter().position(|t| t.pane == pane && t.id == id) {
            Some(ix) => ix,
            None => {
                self.tabs.push(Tab { pane, id, view: None, title: None });
                self.tabs.len() - 1
            }
        };
        self.activate(ix, window, cx);
    }

    pub fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        self.active = Some(ix);
        self.strip.scroll_to_item(ix);
        self.mount_active(window, cx);
        self.keep(cx);
        cx.notify();
    }

    /// Moves by `step` tabs, wrapping, as `stepPanelTab` does.
    pub fn step(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.tabs.len() as isize;
        if n == 0 {
            return;
        }
        let from = self.active.map_or(0, |ix| ix as isize);
        self.activate((from + step).rem_euclid(n) as usize, window, cx);
    }

    /// Closes the tab at `ix`; closing the last leaves the panel open on the launcher.
    pub fn close(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        self.keep_focus(window, cx);
        let tab = self.tabs.remove(ix);
        if let (Some(host), Some(entry)) = (&self.host, Self::entry(tab.pane)) {
            (entry.closed)(host, self.opened(&tab), cx);
        }
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(a) if a > ix || a == self.tabs.len() => Some(a.saturating_sub(1)),
            other => other,
        };
        self.mount_active(window, cx);
        self.keep(cx);
        cx.notify();
    }

    /// The screen registered for `pane`'s tabs.
    fn entry(pane: Pane) -> Option<Entry> {
        screens::screen(pane.name()).filter(|e| e.place == Place::Panel(pane))
    }

    /// What `tab` is opened on: the panel's thread and workspace, and the tab's id.
    fn opened(&self, tab: &Tab) -> Opened {
        let (workspace_id, thread_id) = match &self.at {
            At::Here => (None, None),
            At::Thread { workspace_id, thread_id, .. } => (Some(workspace_id.clone()), Some(thread_id.clone()).filter(|t| !t.is_empty())),
        };
        Opened { workspace_id, thread_id, tab: Some(tab.id.clone()), arg: None }
    }

    /// Builds the active tab's screen while the panel shows; the web app mounts no pane while its panel is shut or hidden.
    fn mount_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.active.filter(|_| self.open && !self.hidden) else { return };
        let (Some(host), Some(tab)) = (self.host.clone(), self.tabs.get(ix)) else { return };
        if tab.view.is_some() {
            return;
        }
        let Some(entry) = Self::entry(tab.pane) else { return };
        let mounted = (entry.mount)(&host, self.opened(tab), window, cx);
        let tab = &mut self.tabs[ix];
        tab.view = Some(mounted.view);
        tab.title = mounted.parts.title;
    }

    /// Lets every tab's screen go; each finds its session again by its tab when it is built next.
    fn unmount(&mut self) {
        for tab in &mut self.tabs {
            tab.view = None;
            tab.title = None;
        }
    }

    fn bar(&self, controls: impl IntoElement, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(ix, tab)| {
                let active = self.active == Some(ix);
                let (label, glyph) = tab.face(cx);
                h_flex()
                    .id(("panel-tab", ix))
                    .debug_selector(move || format!("panel-tab-{ix}"))
                    .group("panel-tab")
                    .flex_none()
                    .h(px(24.))
                    .max_w(px(144.))
                    .gap(px(2.))
                    .rounded(px(6.))
                    .pl(px(6.))
                    .pr(px(8.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .cursor_pointer()
                    .map(|el| {
                        if active {
                            el.bg(p.accent).text_color(p.foreground)
                        } else {
                            el.text_color(p.muted_foreground).hover(|s| s.bg(p.accent.opacity(0.6)).text_color(p.foreground))
                        }
                    })
                    .on_mouse_down(MouseButton::Middle, cx.listener(move |this, _, window, cx| this.close(ix, window, cx)))
                    .on_click(cx.listener(move |this, _, window, cx| this.activate(ix, window, cx)))
                    .child(
                        div()
                            .id(("panel-tab-close", ix))
                            .debug_selector(move || format!("panel-tab-close-{ix}"))
                            .flex()
                            .flex_none()
                            .size(px(16.))
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .hover(|s| s.bg(p.muted))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close(ix, window, cx)
                            }))
                            .child(div().group_hover("panel-tab", |s| s.invisible()).child(Icon::new(glyph).with_size(px(12.))))
                            .child(
                                div()
                                    .absolute()
                                    .invisible()
                                    .group_hover("panel-tab", |s| s.visible())
                                    .child(Icon::new(IconName::X).with_size(px(12.))),
                            ),
                    )
                    .child(div().debug_selector(move || format!("panel-tab-label-{ix}")).min_w_0().truncate().child(label))
            })
            .collect::<Vec<_>>();
        let add = (!self.tabs.is_empty()).then(|| {
            div()
                .relative()
                .flex_none()
                .child(
                    div()
                        .id("panel-add")
                        .flex()
                        .size(px(28.))
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(|s| s.bg(p.accent))
                        .child(Icon::new(IconName::Plus).with_size(px(14.)).text_color(p.muted_foreground))
                        .on_click(cx.listener(|this, _, window, cx| {
                            // A press on the plus while the menu is open has just closed it through its outside press.
                            if this.closed_at.take().is_some_and(|at| at.elapsed() < MENU_CLOSE) {
                                return;
                            }
                            this.adding = true;
                            this.focus.focus(window, cx);
                            cx.notify();
                        })),
                )
                .when(self.adding, |el| {
                    el.child(deferred(
                        anchored().position_mode(AnchoredPositionMode::Local).position(point(px(0.), px(34.))).child(self.add_menu(cx)),
                    ))
                })
        });
        h_flex()
            .id("panel-bar")
            .debug_selector(|| "panel-bar".into())
            .flex_none()
            .h(BAR_HEIGHT)
            .items_center()
            .gap_1()
            .pl_2()
            .pr_3()
            .child(
                h_flex()
                    .id("panel-tabs")
                    .debug_selector(|| "panel-tabs".into())
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .gap_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.strip)
                    .children(tabs)
                    .children(add),
            )
            .child(controls)
    }

    /// The plus's menu: each pane the launcher lists, with its glyph and letter, 176 px at the least.
    fn add_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let rows = Pane::ALL.into_iter().filter(|pane| !(pane.only_when_available() && pane.availability(&self.at).is_err())).map(|pane| {
            let open = pane.availability(&self.at).is_ok();
            h_flex()
                .id(SharedString::from(format!("add-{}", pane.name())))
                .h(px(32.))
                .px_2()
                .gap_2()
                .rounded(px(8.))
                .text_size(px(14.))
                .text_color(p.foreground)
                .when(!open, |el| el.opacity(0.64))
                .when(open, |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(p.accent))
                        .on_click(cx.listener(move |this, _, window, cx| this.add(pane, None, window, cx)))
                })
                .child(Icon::new(pane.glyph()).with_size(px(16.)).text_color(p.muted_foreground))
                .child(div().flex_1().child(SharedString::new_static(pane.label())))
                .children(
                    pane.key().map(|k| div().text_size(px(12.)).text_color(p.muted_foreground).child(SharedString::from(k.to_string()))),
                )
        });
        v_flex()
            .id("panel-add-menu")
            .occlude()
            .min_w(px(176.))
            .p_1()
            .rounded(px(12.))
            .border_1()
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.popover_shadow.clone())
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.adding = false;
                this.closed_at = Some(Instant::now());
                cx.notify();
            }))
            .children(rows)
    }

    fn launcher(&self, (card, edge): (Hsla, Hsla), cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let mut opens = 0;
        let tiles = Pane::ALL.into_iter().filter_map(|pane| {
            let open = pane.availability(&self.at);
            if pane.only_when_available() && open.is_err() {
                return None;
            }
            let row = open.is_ok().then(|| {
                opens += 1;
                opens - 1
            });
            let line = SharedString::new_static(match open {
                Ok(()) => pane.description(),
                Err(reason) => reason,
            });
            let dim = if open.is_ok() { 1. } else { 0.64 };
            let key = pane.key().map(|k| SharedString::from(k.to_string()));
            let tile = h_flex()
                .id(SharedString::new_static(pane.name()))
                .debug_selector(move || format!("launcher-{}", pane.name()))
                .w_full()
                .gap_3()
                .px_3()
                .py_2()
                .rounded(px(10.))
                .border_1()
                .border_color(edge)
                .when_some(row, |el, row| {
                    el.cursor_pointer().on_click(cx.listener(move |this, _, window, cx| this.add(pane, None, window, cx))).on_hover(
                        cx.listener(move |this, over: &bool, _, cx| {
                            this.lit = over.then_some(row);
                            cx.notify();
                        }),
                    )
                })
                .child(Icon::new(pane.glyph()).with_size(px(16.)).text_color(p.foreground.opacity(0.8)).opacity(dim))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .text_color(p.foreground)
                                .opacity(dim)
                                .child(SharedString::new_static(pane.label())),
                        )
                        .child(div().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(line)),
                )
                .children(key.map(|k| div().opacity(dim).child(keycap(k, cx))));
            let lit = row.is_some() && row == self.lit;
            Some(Tile { name: pane.name(), rest: card, fill: lit.then_some(p.card_hover), tile })
        });
        div().flex().flex_1().min_h_0().items_center().justify_center().px_6().pt_6().pb(BAR_HEIGHT + px(24.)).child(
            div()
                .relative()
                .debug_selector(|| "panel-launcher".into())
                .w_full()
                .max_w(px(384.))
                .child(
                    v_flex()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_full()
                        .mb_5()
                        .items_center()
                        .text_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(20.))
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .child("Open a panel"),
                        )
                        .child(
                            div()
                                .id("panel-launcher-lede")
                                .mt_1()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(p.muted_foreground)
                                .child("A browser, a terminal, the changes, the files, the computer or what runs on it.")
                                .test_support(),
                        ),
                )
                .child(v_flex().gap_2().children(tiles)),
        )
    }

    /// The panel's bar and content, with `controls` at the bar's right end.
    /// `ink` is the card fill and the line the panel's surface takes, which the Mac's glass moves.
    pub fn draw(&mut self, controls: impl IntoElement, ink: (Hsla, Hsla), cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.active.and_then(|ix| self.tabs.get(ix)) {
            Some(Tab { view: Some(view), .. }) => {
                div().flex_1().min_h_0().child(view.clone().cached(StyleRefinement::default().size_full())).into_any_element()
            }
            Some(_) => v_flex()
                .debug_selector(|| "pane-empty".into())
                .flex_1()
                .items_center()
                .justify_center()
                .p_6()
                .text_center()
                .child(div().text_size(px(13.)).line_height(px(20.)).text_color(cx.palette().muted_foreground).child("Nothing open here."))
                .child(
                    div()
                        .mt_1()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(cx.palette().muted_foreground)
                        .child("Pick a panel from the tab strip above."),
                )
                .into_any_element(),
            None => self.launcher(ink, cx).into_any_element(),
        };
        // `previewFocus` holds for focus anywhere in the panel, as `data-preview-panel-mode` on its root says today.
        let mut context = gpui_kit::KeyContext::default();
        context.add(super::keys::PREVIEW);
        if self.tabs.len() > 1 {
            context.add(super::keys::PANEL_TABS);
        }
        v_flex()
            .track_focus(&self.focus)
            .key_context(context)
            .on_key_down(cx.listener(Self::launch_by_key))
            .size_full()
            .child(self.bar(controls, cx))
            .child(content)
    }
}

/// A launcher tile, its fill eased to the highlight's over `COLORS` as `transition-colors duration-150` does.
#[derive(IntoElement)]
struct Tile {
    name: &'static str,
    rest: Hsla,
    /// The highlight's fill, while the tile is the lit row, under the pointer or reached by the arrows.
    fill: Option<Hsla>,
    tile: gpui_kit::Stateful<gpui_kit::Div>,
}

impl RenderOnce for Tile {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let fill = motion::ease_ink(self.name, "fill", self.fill.unwrap_or(self.rest), motion::COLORS, window, cx);
        self.tile.bg(fill)
    }
}
