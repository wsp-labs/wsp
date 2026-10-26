// SPDX-License-Identifier: AGPL-3.0-only
//! The shells of one drawer or one panel tab (`ThreadTerminalDrawer.tsx`, which today's app draws in both places):
//! their splits, the toolbar, the tab column, the font card, the overlays over a pane that cannot be typed into, and
//! in a drawer the strip that drags its height.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, DragMoveEvent, Empty, Entity, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    MouseDownEvent, Pixels, Point, Render, SharedString, Subscription, Task, Window,
};
use wsp_host_client::protocol::{self, CapabilitiesGetRequest, WorkspaceOut, WorkspaceStatus, WorkspacesListRequest};
use wsp_host_client::Failure;

use super::font_card;
use super::groups::{Direction, Place as SplitAt, Ui};
use super::hub::{self, Key};
use super::link::{Link, LinkEvent, LinkHold, Status, Tag};
use super::links;
use super::look::{self, Choices, Look};
use super::menu;
use super::overlay;
use super::session::{Changed, Session};
use super::state::{self, Absent, PaneInput, PaneState, PauseMode, WorkspaceRead, WorkspaceState};
use super::tabs::{self, Pick};
use super::toolbar::{self, Act};
use super::view::{TerminalView, ViewEvent};
use super::words;
use crate::screens::app_shell::{self, keys};
use crate::screens::sidebar::Sidebar;
use crate::screens::{Host, Route, Screen};
use crate::ui::context_menu::{ContextMenu, MenuItem};
use crate::ui::{ActivePalette, Button};

/// `DEFAULT_THREAD_TERMINAL_HEIGHT`'s bounds in `ThreadTerminalDrawer.tsx`: 180 px up to three quarters of the window.
pub const MIN_HEIGHT: f32 = 180.;
/// How long an adopted shell may wait for its replay before its grid shows as it stands.
const REPLAY_BOUND: Duration = Duration::from_secs(2);
/// The drag strip over the drawer's top border, `h-1.5`.
pub const STRIP: f32 = 6.;

/// The drawer's height within the window's: at least 180 px, at most 75% of the window, whole pixels.
pub fn clamp_height(height: f32, window: f32) -> f32 {
    let max = (window * 0.75).floor().max(MIN_HEIGHT);
    height.clamp(MIN_HEIGHT, max).round()
}

/// Where the shells sit: under the thread, by its drawer's key, or in a right panel tab, by the tab's id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Drawer(Key),
    Panel { key: Key, tab: SharedString },
}

impl Mode {
    pub fn key(&self) -> &Key {
        match self {
            Mode::Drawer(key) | Mode::Panel { key, .. } => key,
        }
    }

    fn tag(&self) -> Tag {
        match self {
            Mode::Drawer(Key::Thread { thread_id, .. }) if thread_id.is_empty() => Tag::Thread(super::new_thread_page().to_owned()),
            Mode::Drawer(Key::Thread { thread_id, .. }) => Tag::Thread(thread_id.clone()),
            Mode::Drawer(_) => Tag::None,
            Mode::Panel { tab, .. } => Tag::Panel(tab.clone()),
        }
    }
}

/// The computer a drawer's shells run on as the places list has it: this computer's name, the workspace's own
/// computer's, and why that computer is not there.
#[derive(Clone, Default, PartialEq)]
struct Placed {
    here: String,
    computer: Option<String>,
    absent: Option<Absent>,
}

/// What the drag strip carries while it is held.
#[derive(Clone, Copy)]
struct Resize;

struct Nothing;

impl Render for Nothing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

pub struct Drawer {
    host: Host,
    mode: Mode,
    link: Entity<Link>,
    views: Vec<(String, Entity<TerminalView>)>,
    config: Option<protocol::TerminalConfig>,
    dark: bool,
    /// The theme the look's colours were read from.
    theme: &'static str,
    choices: Choices,
    look: Look,
    record: Option<WorkspaceOut>,
    /// The sizes the host's provider offers (`capabilities.sizes`), which the next size up is read from.
    offers: Option<Vec<words::Offer>>,
    status: Option<WorkspaceStatus>,
    /// The record and status as the sidebar last held them, so a sidebar change elsewhere redraws nothing here.
    seen: Option<(serde_json::Value, Option<serde_json::Value>)>,
    /// What the places list says of the computer the shells run on.
    place: Placed,
    font_input: Option<Entity<InputState>>,
    /// Where a drag of the strip took hold: the pointer's height and the drawer's, and the height it is at now.
    drag: Option<(Pixels, f32, f32)>,
    /// The overlay's title and when it went up, which its "for Ns" counts from.
    overlay: Option<(String, Instant)>,
    key_refused: Option<String>,
    starting: bool,
    /// Why the last New or Split was refused, in red in the pane.
    refused: Option<String>,
    /// Why the last Wake or Start again was refused, in red under its button.
    asked_refused: Option<String>,
    focus: FocusHandle,
    overlay_focus: FocusHandle,
    focus_active: bool,
    menu: ContextMenu,
    _tick: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
    /// Each view's events, dropped with the view.
    view_events: HashMap<String, Subscription>,
    /// The adopted shells whose replay has not landed, covered by the bare ground until it does or [`REPLAY_BOUND`] passes.
    held: HashMap<String, (Subscription, Task<()>)>,
    _font_events: Option<Subscription>,
    _hold: LinkHold,
}

impl Focusable for Drawer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Drawer {
    pub fn new(host: Host, mode: Mode, window: &mut Window, cx: &mut Context<Self>) -> Drawer {
        let link = Link::get(&host, mode.key().target(), cx);
        let hold = link.update(cx, |link, _| link.hold());
        let subscriptions = vec![
            cx.subscribe_in(&link, window, |this, _, event: &LinkEvent, window, cx| {
                if *event == LinkEvent::Changed {
                    this.sync(window, cx);
                    cx.notify();
                }
            }),
            cx.observe_window_bounds(window, |this, window, cx| this.sync_height(window, cx)),
        ];
        let dark = cx.theme().is_dark();
        let choices = Choices {
            family: hub::font(cx),
            size_from_file: look::size_from_file(cx),
            zoom: mode.key().workspace_id().and_then(|w| hub::zoom(w, cx)).unwrap_or(0.),
            ..Choices::default()
        };
        let look = Look::with(None, &choices, cx);
        let mut this = Drawer {
            host,
            mode,
            link,
            views: Vec::new(),
            config: None,
            dark,
            theme: crate::ui::theme::current(cx).id,
            choices,
            look,
            record: None,
            offers: None,
            status: None,
            seen: None,
            place: Placed::default(),
            font_input: None,
            drag: None,
            overlay: None,
            key_refused: None,
            starting: false,
            refused: None,
            asked_refused: None,
            focus: cx.focus_handle(),
            overlay_focus: cx.focus_handle(),
            focus_active: true,
            menu: ContextMenu::default(),
            _tick: None,
            _subscriptions: subscriptions,
            view_events: HashMap::new(),
            held: HashMap::new(),
            _font_events: None,
            _hold: hold,
        };
        this.read_config(window, cx);
        this.read_workspace(window, cx);
        if let Some(sidebar) = super::workspaces_sidebar(cx) {
            this._subscriptions.push(cx.observe(&sidebar, |this, sidebar, cx| this.follow(&sidebar, cx)));
            this.follow(&sidebar, cx);
        }
        this.sync(window, cx);
        this
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn link(&self) -> &Entity<Link> {
        &self.link
    }

    /// The arrangement this drawer or tab shows.
    pub fn ui(&self, cx: &mut App) -> Ui {
        match &self.mode {
            Mode::Drawer(key) => hub::drawer(key, cx),
            Mode::Panel { tab, .. } => hub::surface(tab, cx).unwrap_or_default(),
        }
    }

    fn set_ui(&mut self, ui: Ui, cx: &mut Context<Self>) {
        match &self.mode {
            Mode::Drawer(key) => hub::set_drawer(key, ui, cx),
            Mode::Panel { tab, .. } => hub::set_surface(tab, ui, cx),
        }
        cx.notify();
    }

    /// The shells' views, in the order the arrangement lists them.
    pub fn views(&self) -> &[(String, Entity<TerminalView>)] {
        &self.views
    }

    fn view(&self, pty: &str) -> Option<&Entity<TerminalView>> {
        self.views.iter().find(|(id, _)| id == pty).map(|(_, v)| v)
    }

    /// The name a shell's tab reads: its title, "(exited)" once it ended, else "Terminal".
    pub fn label(&self, pty: &str, cx: &App) -> SharedString {
        let Some(view) = self.view(pty) else { return "Terminal".into() };
        let session = view.read(cx).session().read(cx);
        let title = session.title();
        match (title.is_empty(), session.exited().is_some()) {
            (true, _) => "Terminal".into(),
            (false, true) => format!("{title} (exited)").into(),
            (false, false) => title.clone(),
        }
    }

    /// Asks for the focused split to take focus on the next draw, as opening the drawer and lifting an overlay do.
    pub fn focus_active(&mut self, cx: &mut Context<Self>) {
        self.focus_active = true;
        cx.notify();
    }

    /// Draws the pane's ground at the config's opacity where the page draws glass, opaque where it does not.
    pub fn set_glass(&mut self, glass: bool, cx: &mut Context<Self>) {
        if glass != self.choices.glass {
            self.choices.glass = glass;
            self.relook(cx);
        }
    }

    /// Whether the drawer drops its ground for the window's glass (`useTerminalGround`): the page draws glass and the
    /// config's opacity is below 1, which the look's opacity holds only then.
    pub fn see_through(&self) -> bool {
        self.look.opacity < 1.
    }

    fn read_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let reading = look::read_config(&self.host, self.dark);
        let dark = self.dark;
        cx.spawn_in(window, async move |this, cx| {
            let read = reading.await;
            _ = this.update_in(cx, |this, _, cx| match read {
                Ok(config) if this.dark == dark => {
                    this.config = Some(config);
                    this.relook(cx);
                }
                Ok(_) => {}
                Err(failure) => config_failed(&failure),
            });
        })
        .detach();
    }

    /// The workspace's record, which the pane reads its state from.
    fn read_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.mode.key().workspace_id().map(str::to_owned) else { return };
        let listing = self.host.run(|client| async move {
            let list: WorkspacesListRequest = serde_json::from_value(serde_json::json!({})).map_err(|e| Failure::new(e.to_string()))?;
            client.call(list).await.map(|w| w.workspaces)
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(workspaces) = listing.await else { return };
            let record =
                workspaces.into_iter().find(|w| serde_json::to_value(w).ok().and_then(|v| v["id"].as_str().map(|s| s == id)) == Some(true));
            _ = this.update(cx, |this, cx| {
                if this.seen.is_none() {
                    this.record = record;
                    this.watch_metrics(cx);
                    cx.notify();
                }
            });
        })
        .detach();
        let asking = self.host.run(|client| async move {
            let ask: CapabilitiesGetRequest = serde_json::from_value(serde_json::json!({})).map_err(|e| Failure::new(e.to_string()))?;
            client.call(ask).await.map(|c| c.capabilities)
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(capabilities) = asking.await else { return };
            let sizes = serde_json::to_value(capabilities).unwrap_or_default()["sizes"].as_array().cloned();
            let offers = sizes.map(|sizes| {
                sizes
                    .iter()
                    .filter_map(|o| {
                        Some(words::Offer {
                            cpu: o["cpu"].as_f64()?,
                            mem_mb: o["memMb"].as_f64()?,
                            rate_usd_per_hour: o["rateUsdPerHour"].as_f64()?,
                        })
                    })
                    .collect()
            });
            _ = this.update(cx, |this, _| this.offers = offers);
        })
        .detach();
    }

    /// A machine's figures ride its daemon's channel (`readingRoad(kind, "metrics")`); this computer's ride the host.
    fn watch_metrics(&mut self, cx: &mut Context<Self>) {
        let local = self.record.as_ref().and_then(|r| serde_json::to_value(r).ok()).is_some_and(|r| r["kind"] == "local");
        if self.record.is_some() && !local {
            self.link.update(cx, |link, cx| link.watch_sys(cx));
        }
    }

    /// The record's field as the live status says it where one has arrived.
    fn field(&self, key: &str) -> Option<String> {
        let read = |v: Option<serde_json::Value>| v.and_then(|v| v.get(key)?.as_str().map(str::to_owned));
        read(self.status.as_ref().and_then(|s| serde_json::to_value(s).ok()))
            .or_else(|| read(self.record.as_ref().and_then(|r| serde_json::to_value(r).ok())))
    }

    /// `outOfMemoryReading`: the channel's last reading, where memory was near full, while the workspace runs and the
    /// channel is down.
    fn out_of_memory(&self, cx: &App) -> Option<String> {
        let link = self.link.read(cx);
        let memory = link.memory().filter(|m| m.near_full())?;
        let running = self.field("phase").is_none_or(|phase| phase == "running");
        (running && *link.status() != Status::Live).then(|| words::out_of_memory_line(&memory))
    }

    /// `biggerSizeLine` for the machine's size, once both it and the provider's sizes are known.
    fn bigger(&self) -> Option<String> {
        let status = serde_json::to_value(self.status.as_ref()?).ok()?;
        Some(words::bigger_size_line(status["size"]["memMb"].as_f64()?, self.offers.as_ref()?))
    }

    /// The workspace's record and live status as the sidebar holds them, which move the pane's state as they change.
    fn follow(&mut self, sidebar: &Entity<Sidebar>, cx: &mut Context<Self>) {
        let s = sidebar.read(cx);
        let id = self.mode.key().workspace_id();
        let place =
            Placed { here: s.here_name(), computer: id.and_then(|id| s.own_computer_name(id)), absent: id.and_then(|id| s.absence(id)) };
        if place != self.place {
            self.place = place;
            cx.notify();
        }
        let Some(id) = self.mode.key().workspace_id() else { return };
        let (record, status) = sidebar.read(cx).workspace(id);
        let Some(record) = record.cloned() else { return };
        let seen = Some((record, status.cloned()));
        if seen == self.seen {
            return;
        }
        let napping = |pane: &PaneState| matches!(pane, PaneState::Paused { .. } | PaneState::Waking { .. });
        let was_napping = napping(&self.pane(cx));
        let (record, status) = seen.clone().unwrap();
        self.record = serde_json::from_value(record).ok().or(self.record.take());
        self.status = status.and_then(|s| serde_json::from_value(s).ok());
        self.seen = seen;
        self.watch_metrics(cx);
        let pane = self.pane(cx);
        // `feedStop`: a nap that ends processes ends the shells, so the next attach says the thread stopped.
        if matches!(pane, PaneState::Paused { pause_mode, .. } if pause_mode != Some(PauseMode::Memory)) {
            self.link.update(cx, |link, _| link.stopped());
        }
        if was_napping && !napping(&pane) {
            self.link.update(cx, |link, cx| link.redial(cx));
        }
        cx.notify();
    }

    fn relook(&mut self, cx: &mut Context<Self>) {
        self.look = Look::with(self.config.as_ref(), &self.choices, cx);
        let look = self.look.clone();
        for (_, view) in &self.views {
            view.update(cx, |v, cx| v.set_look(look.clone(), cx));
        }
        cx.notify();
    }

    /// Follows what changes a look from outside: the font card, the size preference, the zoom keys,
    /// the theme.
    fn follow_look(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wanted = Choices {
            family: hub::font(cx),
            size_from_file: look::size_from_file(cx),
            zoom: self.mode.key().workspace_id().and_then(|w| hub::zoom(w, cx)).unwrap_or(0.),
            ..self.choices.clone()
        };
        let dark = cx.theme().is_dark();
        let theme = crate::ui::theme::current(cx).id;
        if theme != self.theme {
            self.theme = theme;
            self.look = self.look.recolored(self.config.as_ref(), cx);
            let look = self.look.clone();
            for (_, view) in &self.views {
                view.update(cx, |v, cx| v.set_look(look.clone(), cx));
            }
        }
        if dark != self.dark {
            self.dark = dark;
            self.read_config(window, cx);
        }
        if wanted != self.choices {
            self.choices = wanted;
            self.relook(cx);
        }
    }

    /// The shells the channel lists for this drawer, in the arrangement's order and then the channel's.
    fn listed(&self, ui: &Ui, cx: &mut App) -> Vec<String> {
        let panel = hub::panel_shells(cx);
        let link = self.link.read(cx);
        let mine: Vec<String> = link
            .ptys()
            .iter()
            .filter(|p| match (&self.mode, &p.tag) {
                (Mode::Drawer(Key::Thread { thread_id, .. }), Tag::Thread(t)) if thread_id.is_empty() => {
                    t.starts_with(super::NEW_THREAD_PAGE_MARK)
                }
                (Mode::Drawer(Key::Thread { thread_id, .. }), Tag::Thread(t)) => t == thread_id,
                (Mode::Drawer(Key::Thread { .. }), _) => false,
                (Mode::Drawer(_), Tag::None) => true,
                (Mode::Drawer(_), _) => false,
                (Mode::Panel { tab, .. }, Tag::Panel(t)) => t == tab,
                (Mode::Panel { .. }, _) => false,
            })
            .map(|p| p.session.read(cx).pty().to_string())
            .filter(|id| matches!(self.mode, Mode::Panel { .. }) || !panel.contains(id))
            .collect();
        let mut next: Vec<String> = ui.terminal_ids.iter().filter(|id| mine.contains(id)).cloned().collect();
        next.extend(mine.into_iter().filter(|id| !ui.terminal_ids.contains(id)));
        next
    }

    /// Follows the channel: the arrangement takes the daemon's list once it is live, each listed shell gets a view,
    /// and a drawer's first open asks for its first shell.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let live = *self.link.read(cx).status() == Status::Live;
        let mut ui = self.ui(cx);
        if live {
            let next = self.listed(&ui, cx);
            let reconciled = ui.reconcile(&next);
            if reconciled != ui.normalize() {
                ui = reconciled;
                self.set_ui(ui.clone(), cx);
            }
        }
        let sessions: Vec<_> = self.link.read(cx).ptys().iter().map(|p| p.session.clone()).collect();
        self.views.retain(|(id, _)| ui.terminal_ids.contains(id));
        let views = &self.views;
        self.view_events.retain(|id, _| views.iter().any(|(v, _)| v == id));
        self.held.retain(|id, _| views.iter().any(|(v, _)| v == id));
        for id in &ui.terminal_ids {
            if self.view(id).is_some() {
                continue;
            }
            let Some(session) = sessions.iter().find(|s| s.read(cx).pty() == id).cloned() else { continue };
            if session.read(cx).awaiting_replay() {
                self.hold(id, &session, cx);
            }
            let look = self.look.clone();
            let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
            let pty = id.clone();
            let events = cx.subscribe_in(&view, window, move |this, _, event: &ViewEvent, window, cx| match event {
                ViewEvent::Activated => this.activate(&pty, false, window, cx),
                ViewEvent::OpenUrl(url) => cx.open_url(url),
                ViewEvent::OpenPath(path) => this.open_path(path, cx),
                ViewEvent::Menu(at) => this.open_menu(&pty, *at, window, cx),
            });
            self.view_events.insert(id.clone(), events);
            self.views.push((id.clone(), view));
        }
        let first = match &self.mode {
            Mode::Drawer(key) => live && ui.terminal_open && ui.terminal_ids.is_empty() && !hub::mark_spawned(key, cx),
            Mode::Panel { tab, .. } => live && hub::surface(tab, cx).is_none(),
        };
        if first {
            if let Mode::Panel { tab, .. } = &self.mode {
                hub::set_surface(tab, Ui::default(), cx);
            }
            self.open_shell(None, true, window, cx);
        }
    }

    /// Covers `pty` with the bare ground until its replay lands, so a reopen never shows its grid empty first.
    fn hold(&mut self, pty: &str, session: &Entity<Session>, cx: &mut Context<Self>) {
        let id = pty.to_owned();
        let landed = cx.subscribe(session, move |this, session, _: &Changed, cx| {
            if !session.read(cx).awaiting_replay() && this.held.remove(&id).is_some() {
                cx.notify();
            }
        });
        let id = pty.to_owned();
        let bound = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REPLAY_BOUND).await;
            _ = this.update(cx, |this, cx| {
                if this.held.remove(&id).is_some() {
                    cx.notify();
                }
            });
        });
        self.held.insert(pty.to_owned(), (landed, bound));
    }

    /// A shell's grid, under the bare ground while it waits for its replay; the view stays mounted, so its keys reach
    /// the shell all along.
    fn grid(&self, pty: &str, cx: &App) -> AnyElement {
        let Some(view) = self.view(pty) else { return div().size_full().into_any_element() };
        let held = self.held.contains_key(pty);
        let ground = view.read(cx).ground();
        div()
            .relative()
            .size_full()
            .child(view.clone())
            .when(held, |el| el.child(div().debug_selector(|| "terminal-held".into()).absolute().inset_0().bg(ground)))
            .when(!held, |el| el.debug_selector(|| "terminal-grid".into()))
            .into_any_element()
    }

    /// A new shell, as its own tab or split into the active group.
    pub fn open_shell(&mut self, split: Option<Direction>, first: bool, window: &mut Window, cx: &mut Context<Self>) {
        let ui = self.ui(cx);
        if split.is_some() && !ui.can_split() {
            return;
        }
        let size = self.views.first().map(|(_, v)| v.read(cx).session().read(cx).engine().size()).unwrap_or((0, 0));
        let tag = self.mode.tag();
        let at = if matches!(self.mode, Mode::Panel { .. }) { SplitAt::End } else { SplitAt::AfterActive };
        let creating = self.link.update(cx, |link, cx| link.create(tag, size.0, size.1, cx));
        cx.spawn_in(window, async move |this, cx| {
            let created = creating.await;
            _ = this.update_in(cx, |this, window, cx| match created {
                Ok(session) => {
                    this.refused = None;
                    let pty = session.read(cx).pty().to_string();
                    let ui = this.ui(cx);
                    let ui = match split {
                        Some(direction) => ui.split(&pty, direction, at),
                        None => ui.new_terminal(&pty),
                    };
                    this.set_ui(ui, cx);
                    this.focus_active = true;
                    this.sync(window, cx);
                }
                Err(said) => {
                    if first {
                        if let Mode::Drawer(key) = &this.mode {
                            hub::unmark_spawned(key, cx);
                        }
                    }
                    let line = refused_line(this.computer_name().as_deref(), &said);
                    eprintln!("desktop-gpui: {line}");
                    this.refused = Some(line);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Why the last New or Split was refused, as the pane shows it.
    pub fn refused(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    /// Why the last Wake or Start again was refused, as the pane shows it under the button.
    pub fn asked_refused(&self) -> Option<&str> {
        self.asked_refused.as_deref()
    }

    /// Why the last key on the overlay was not sent, while the overlay that refused it stands.
    pub fn key_refused(&self) -> Option<&str> {
        self.key_refused.as_deref()
    }

    /// `openTerminalPath`: a path clicked in a shell opens in Files; a build without a Files screen opens nothing, since
    /// a route no screen answers draws a blank page in place of the thread.
    fn open_path(&self, text: &str, cx: &mut App) {
        if let Some(route) = self.path_route(text).filter(|_| crate::screens::screen("files").is_some()) {
            app_shell::navigate(route, cx);
        }
    }

    /// Where a path clicked in a shell opens: Files, on the path read against the thread's folder (`threadFolderOf`,
    /// the copy's own folder else the project's; the port keeps no chosen folder and no daemon root, today's first and
    /// last); none while that folder is unknown.
    pub fn path_route(&self, text: &str) -> Option<Route> {
        let record = self.record.as_ref().and_then(|r| serde_json::to_value(r).ok())?;
        let folder = record["folder"].as_str().or(record["project"]["path"].as_str())?;
        Some(Route::Screen(format!("files:file:{}", links::resolve_path(text, folder)).into()))
    }

    /// The computer's name for the refused-open line, where the record says it.
    fn computer_name(&self) -> Option<String> {
        let record = serde_json::to_value(self.record.as_ref()?).ok()?;
        record["name"].as_str().map(str::to_owned)
    }

    pub fn activate(&mut self, pty: &str, focus: bool, _window: &mut Window, cx: &mut Context<Self>) {
        let ui = self.ui(cx);
        let next = ui.activate(pty);
        if next != ui.normalize() {
            self.set_ui(next, cx);
        }
        if focus {
            self.focus_active = true;
            cx.notify();
        }
    }

    /// Ends `pty` and drops it from the arrangement; the drawer closes with its last shell, and a panel tab too, as
    /// `RightPanel.tsx` closes a terminal tab left with none.
    pub fn close(&mut self, pty: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.link.update(cx, |link, cx| link.kill(pty, cx));
        let ui = self.ui(cx).close(pty);
        if matches!(self.mode, Mode::Panel { .. }) && ui.terminal_ids.is_empty() {
            window.dispatch_action(Box::new(keys::ClosePanelTab), cx);
        }
        self.views.retain(|(id, _)| id != pty);
        self.set_ui(ui, cx);
        self.focus_active = true;
    }

    pub fn close_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.ui(cx).active_terminal_id;
        if !active.is_empty() {
            self.close(&active, window, cx);
        }
    }

    fn act(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        match act {
            Act::Split => self.open_shell(Some(Direction::Horizontal), false, window, cx),
            Act::SplitVertical => self.open_shell(Some(Direction::Vertical), false, window, cx),
            Act::New => self.new_shell(window, cx),
            Act::Font => self.toggle_font_card(window, cx),
            Act::Close => self.close_active(window, cx),
        }
    }

    /// The right-click menu of the shell `pty`, which the press made the active one.
    fn open_menu(&mut self, pty: &str, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let (rows, items): (Vec<_>, Vec<_>) = self.menu_rows(pty, cx).into_iter().unzip();
        let pty = pty.to_owned();
        self.menu.open(at, items, window, cx, move |this, pick, window, cx| {
            if let Some(row) = pick.and_then(|ix| rows.get(ix).copied()) {
                this.run_menu_row(&pty, row, window, cx);
            }
        });
    }

    /// The menu's rows for the shell `pty`, as they stand now.
    pub fn menu_rows(&self, pty: &str, cx: &mut App) -> Vec<(menu::Row, MenuItem)> {
        let view = self.view(pty);
        let selection = view.is_some_and(|v| v.read(cx).has_selection(cx));
        let target = menu::Target { active: view.is_some(), selection, can_split: self.ui(cx).can_split(), composer: false };
        menu::rows(target, cfg!(target_os = "macos"))
    }

    /// A row picked in the menu of the shell `pty`.
    pub fn run_menu_row(&mut self, pty: &str, row: menu::Row, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.view(pty).cloned() else { return };
        match row {
            menu::Row::Copy => _ = view.update(cx, |v, cx| v.copy(cx)),
            menu::Row::Paste => view.update(cx, |v, cx| v.paste(cx)),
            menu::Row::Clear => view.update(cx, |v, cx| v.clear(cx)),
            menu::Row::AddToChat => {}
            menu::Row::Pane(act) => self.act(act, window, cx),
        }
        self.focus_active = true;
        cx.notify();
    }

    /// Whether the right-click menu is open, for its tests.
    pub fn menu_open(&self) -> bool {
        self.menu.is_open()
    }

    /// The labels of the rows the last right-click asked for, whoever draws the menu.
    pub fn menu_asked(&self) -> Vec<gpui_kit::SharedString> {
        self.menu.asked().to_vec()
    }

    fn pick(&mut self, pick: Pick, window: &mut Window, cx: &mut Context<Self>) {
        match pick {
            Pick::Shell(pty) => self.activate(&pty, true, window, cx),
            Pick::Close(pty) => self.close(&pty, window, cx),
            Pick::Group(group) => {
                let ui = self.ui(cx);
                let Some(group) = ui.terminal_groups.iter().find(|g| g.id == group) else { return };
                let pty = group.terminal_ids.iter().find(|t| **t == ui.active_terminal_id).or(group.terminal_ids.first()).cloned();
                if let Some(pty) = pty {
                    self.activate(&pty, true, window, cx);
                }
            }
        }
    }

    /// A new shell as a tab of its own; in a panel tab, `newPanelTerminal`: it joins the tab while its list shows, and
    /// opens a tab of its own from one shell or four.
    fn new_shell(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Mode::Panel { .. } = self.mode {
            let ui = self.ui(cx);
            let n = ui.terminal_ids.len();
            if n > 1 && n < super::groups::MAX_PER_GROUP {
                let direction = ui.active_group().map_or(Direction::Horizontal, |g| g.split_direction);
                return self.open_shell(Some(direction), false, window, cx);
            }
            return app_shell::navigate(Route::Screen(SharedString::new_static(<super::panel::TerminalScreen as Screen>::NAME)), cx);
        }
        self.open_shell(None, false, window, cx);
    }

    /// `terminal.split` and `terminal.new` act only with a workspace selected.
    fn by_key(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode.key().workspace_id().is_none() {
            cx.propagate();
            return;
        }
        self.act(act, window, cx);
    }

    /// One zoom step for this workspace, held to 6 to 32 px over the base size; none resets it.
    fn zoom(&mut self, step: Option<f32>, cx: &mut Context<Self>) {
        let Some(workspace) = self.mode.key().workspace_id().map(str::to_owned) else { return };
        let zoom = match step {
            Some(step) => {
                let base = Look::with(self.config.as_ref(), &Choices { zoom: 0., ..self.choices.clone() }, cx).font_size;
                Some(look::font_size(base, self.choices.zoom + step) - base)
            }
            None if hub::zoom(&workspace, cx).is_none() => return,
            None => None,
        };
        hub::set_zoom(&workspace, zoom, cx);
        let patch = serde_json::json!({ "patch": { "terminalZoom": { workspace: zoom } } });
        let saving = self.host.run(move |client| async move {
            let set: protocol::PreferencesSetRequest = serde_json::from_value(patch).map_err(|e| Failure::new(e.to_string()))?;
            client.call(set).await.map(|_| ())
        });
        cx.background_spawn(async move {
            if let Err(failure) = saving.await {
                eprintln!("desktop-gpui: {}", failure.message);
            }
        })
        .detach();
        cx.refresh_windows();
    }

    fn toggle_font_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.font_input.take().is_some() {
            cx.notify();
            return;
        }
        let value = hub::font(cx).unwrap_or_default();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(font_card::placeholder()).default_value(value));
        self._font_events = Some(cx.subscribe_in(&input, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                let family = input.read(cx).value().to_string();
                hub::set_font(&family, cx);
                this.follow_look_now(cx);
            }
        }));
        input.update(cx, |i, cx| i.focus(window, cx));
        self.font_input = Some(input);
        cx.notify();
    }

    fn follow_look_now(&mut self, cx: &mut Context<Self>) {
        self.choices.family = hub::font(cx);
        self.relook(cx);
        cx.refresh_windows();
    }

    fn close_font_card(&mut self, cx: &mut Context<Self>) {
        if self.font_input.take().is_some() {
            self.focus_active = true;
            cx.notify();
        }
    }

    /// How the pane stands, read off the workspace's record and the channel.
    pub fn pane(&self, cx: &App) -> PaneState {
        let socket = self.link.read(cx).status().clone();
        let mut input = match (self.mode.key(), &self.record) {
            (Key::Here, _) => state::here_input(socket, &self.place.here),
            (_, Some(record)) => {
                let read = WorkspaceRead { record, status: self.status.as_ref(), computer: self.place.computer.as_deref() };
                state::input_for(&read, socket, self.place.absent.clone())
            }
            (_, None) => PaneInput {
                state: WorkspaceState::Running,
                reach: None,
                socket,
                out_of_memory: None,
                local: false,
                absent: self.place.absent.clone(),
                where_: "the thread's computer".into(),
                computer: None,
                pause_mode: None,
            },
        };
        input.out_of_memory = self.out_of_memory(cx);
        state::pane_state(&input)
    }

    fn wake(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.mode.key().workspace_id().map(str::to_owned) else { return };
        let host = self.host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let woke = state::wake(&host, &workspace).await;
            _ = this.update(cx, |this, cx| {
                if let Err(said) = &woke {
                    eprintln!("desktop-gpui: {said}");
                }
                this.answered(woke, cx);
            });
        })
        .detach();
    }

    fn start_again(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.mode.key().workspace_id().map(str::to_owned) else { return };
        if self.starting {
            return;
        }
        self.starting = true;
        self.asked_refused = None;
        cx.notify();
        let host = self.host.clone();
        cx.spawn_in(window, async move |this, cx| {
            let started = state::restart_daemon(&host, &workspace).await;
            _ = this.update(cx, |this, cx| {
                this.starting = false;
                this.answered(started, cx);
            });
        })
        .detach();
    }

    /// The host's answer to Wake or Start again: its refusal shows under the button that asked.
    pub fn answered(&mut self, answer: Result<(), SharedString>, cx: &mut Context<Self>) {
        self.asked_refused = answer.err().map(|s| s.to_string());
        cx.notify();
    }

    fn height(&self, window: &Window, cx: &mut App) -> f32 {
        let height = self.drag.map_or_else(|| self.ui(cx).terminal_height, |(_, _, now)| now);
        clamp_height(height, f32::from(window.viewport_size().height))
    }

    /// `syncHeight` on a window resize: a drawer the window clamped keeps the clamped height, saved.
    fn sync_height(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() || !matches!(self.mode, Mode::Drawer(_)) {
            return;
        }
        let ui = self.ui(cx);
        let height = clamp_height(ui.terminal_height, f32::from(window.viewport_size().height));
        if height != ui.terminal_height {
            self.set_ui(ui.set_height(height), cx);
        }
    }

    /// Saves the height a drag left, once, when it ends.
    fn end_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, start, now)) = self.drag.take() else { return };
        let height = clamp_height(now, f32::from(window.viewport_size().height));
        if height != start {
            let ui = self.ui(cx).set_height(height);
            self.set_ui(ui, cx);
        }
        cx.notify();
    }

    fn empty(&self, pane: &PaneState, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let mut body = v_flex()
            .id("terminal-empty")
            .debug_selector(|| "terminal-empty".into())
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .gap_3()
            .px_4()
            .py_6()
            .text_center()
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(p.muted_foreground);
        match words::empty_line(pane) {
            None => {
                let new = toolbar::label(Act::New, false, cfg!(target_os = "macos"));
                body = body
                    .child("No terminals open.")
                    .child(
                        Button::outline("terminal-empty-new", new)
                            .xs()
                            .on_click(cx.listener(|this, _, window, cx| this.open_shell(None, false, window, cx))),
                    )
                    .children(self.refused.clone().map(|refused| overlay::red(refused, cx)));
            }
            Some(line) => {
                body = body.child(line).children(self.buttons(pane, cx));
            }
        }
        body.into_any_element()
    }

    /// A panel tab on a workspace the host has not listed: no link to say anything of yet.
    fn unlinked(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        v_flex()
            .id("terminal-no-link")
            .debug_selector(|| "terminal-no-link".into())
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .gap_1()
            .px_4()
            .text_center()
            .text_size(px(14.))
            .line_height(px(20.))
            .child(div().text_color(p.foreground).child(words::NO_LINK_TITLE))
            .child(div().text_color(p.muted_foreground).child(words::no_link_line(self.place.computer.as_deref())))
            .into_any_element()
    }

    /// Wake under a paused thread, Start again under a daemon this host lost, and the start's refusal in red.
    fn buttons(&self, pane: &PaneState, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = Vec::new();
        if matches!(pane, PaneState::Paused { .. }) {
            out.push(
                Button::outline("terminal-wake", "Wake")
                    .xs()
                    .on_click(cx.listener(|this, _, window, cx| this.wake(window, cx)))
                    .into_any_element(),
            );
        }
        if let PaneState::Absent(absent) = pane {
            if let Some(start) = &absent.start {
                let label = if self.starting { words::STARTING_WORD.to_owned() } else { start.clone() };
                out.push(
                    Button::outline("terminal-start", label)
                        .xs()
                        .held(self.starting)
                        .on_click(cx.listener(|this, _, window, cx| this.start_again(window, cx)))
                        .into_any_element(),
                );
            }
        }
        let asked = self.asked_refused.as_ref().filter(|_| !out.is_empty());
        out.extend([asked, self.refused.as_ref()].into_iter().flatten().map(|refused| overlay::red(refused.clone(), cx)));
        out
    }

    fn state_overlay(&mut self, pane: &PaneState, title: String, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let since = match &self.overlay {
            Some((shown, since)) if *shown == title => *since,
            _ => {
                let now = Instant::now();
                self.overlay = Some((title.clone(), now));
                now
            }
        };
        let ticking = matches!(pane, PaneState::Reconnecting { .. });
        if ticking && self._tick.is_none() {
            self._tick = Some(cx.spawn_in(window, async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }));
        } else if !ticking {
            self._tick = None;
        }
        let refusal = words::input_refusal(pane);
        overlay::cover(cx)
            .track_focus(&self.overlay_focus)
            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
                if !matches!(e.keystroke.key.as_str(), "tab" | "escape") {
                    this.key_refused = refusal.clone();
                    cx.notify();
                }
            }))
            .child(overlay::title(title, ticking.then_some(since)))
            .children(words::hints(pane, self.bigger().as_deref()).into_iter().map(|h| overlay::note(h, cx)))
            .children(self.buttons(pane, cx))
            .children(self.key_refused.clone().map(|line| overlay::note(line, cx)))
            .into_any_element()
    }

    fn gone_overlay(&self, line: SharedString, cx: &mut Context<Self>) -> AnyElement {
        let new = toolbar::label(Act::New, false, cfg!(target_os = "macos"));
        overlay::cover(cx)
            .debug_selector(|| "terminal-gone".into())
            .child(div().child(line))
            .child(
                Button::outline("terminal-gone-new", new)
                    .xs()
                    .on_click(cx.listener(|this, _, window, cx| this.open_shell(None, false, window, cx))),
            )
            .into_any_element()
    }

    fn splits(&self, ui: &Ui, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let group = ui.active_group().cloned();
        let ids: Vec<String> = group.as_ref().map(|g| g.terminal_ids.clone()).unwrap_or_default();
        if ids.len() <= 1 {
            return div().size_full().min_w_0().children(ids.first().map(|id| self.grid(id, cx))).into_any_element();
        }
        let vertical = group.is_some_and(|g| g.split_direction == Direction::Vertical);
        let cells = ids.iter().enumerate().map(|(ix, id)| {
            let active = *id == ui.active_terminal_id;
            let edge = if active { p.border } else { p.border.opacity(0.7) };
            let pty = id.clone();
            div()
                .id(SharedString::from(format!("terminal-split-{id}")))
                .debug_selector(|| "terminal-split".into())
                .relative()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .when(ix > 0 && vertical, |el| el.border_t_1().border_color(edge))
                .when(ix > 0 && !vertical, |el| el.border_l_1().border_color(edge))
                .when(!active, |el| {
                    el.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| this.activate(&pty, true, window, cx)),
                    )
                })
                .child(self.grid(id, cx))
        });
        let grid = if vertical { v_flex() } else { h_flex() };
        grid.size_full().items_stretch().overflow_hidden().children(cells).into_any_element()
    }

    fn body(&mut self, ui: &Ui, pane: &PaneState, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if matches!(self.mode, Mode::Panel { key: Key::Workspace(_), .. }) && self.record.is_none() {
            return self.unlinked(cx);
        }
        if ui.terminal_ids.is_empty() {
            return self.empty(pane, cx);
        }
        let p = cx.palette().clone();
        let can_split = ui.can_split();
        let column = ui.terminal_ids.len() > 1;
        let on_act = {
            let this = cx.entity().downgrade();
            std::rc::Rc::new(move |act: Act, window: &mut Window, cx: &mut App| {
                _ = this.update(cx, |this, cx| this.act(act, window, cx));
            }) as toolbar::OnAct
        };
        let card = self.font_input.as_ref().map(|input| {
            div()
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if e.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.close_font_card(cx);
                    }
                }))
                .child(font_card::card(input, cx))
        });
        let active_lost = self.view(&ui.active_terminal_id).and_then(|v| v.read(cx).session().read(cx).lost().cloned());
        let cover = match words::title(pane) {
            Some(title) => Some(self.state_overlay(pane, title, window, cx)),
            None => {
                self.overlay = None;
                self.key_refused = None;
                self._tick = None;
                active_lost.map(|line| self.gone_overlay(line, cx))
            }
        };
        let area = div()
            .id("terminal-area")
            .debug_selector(|| "terminal-area".into())
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.splits(ui, cx))
            .children(cover)
            .children(self.refused.clone().filter(|_| words::title(pane).is_none()).map(|line| {
                h_flex()
                    .debug_selector(|| "terminal-refused".into())
                    .absolute()
                    .bottom_2()
                    .left_0()
                    .right_0()
                    .justify_center()
                    .child(div().px_2().py_1().rounded(px(6.)).bg(p.background).child(overlay::red(line, cx)))
            }))
            .when(!column, |el| {
                el.child(
                    v_flex()
                        .absolute()
                        .top_2()
                        .right_2()
                        .items_end()
                        .gap(px(6.))
                        .child(toolbar::floating(can_split, on_act.clone(), cx))
                        .children(card),
                )
            });
        let labels: Vec<(String, SharedString)> = ui.terminal_ids.iter().map(|id| (id.clone(), self.label(id, cx))).collect();
        let tab_column = column.then(|| {
            let this = cx.entity().downgrade();
            let on_pick: tabs::OnPick = std::rc::Rc::new(move |pick, window, cx| {
                _ = this.update(cx, |this, cx| this.pick(pick, window, cx));
            });
            let label = move |id: &str| labels.iter().find(|(i, _)| i == id).map_or_else(|| "Terminal".into(), |(_, l)| l.clone());
            let card = self.font_input.as_ref().map(|input| {
                div()
                    .absolute()
                    .right_0()
                    .top(px(22.))
                    .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                        if e.keystroke.key == "escape" {
                            cx.stop_propagation();
                            this.close_font_card(cx);
                        }
                    }))
                    .child(font_card::card(input, cx))
            });
            div()
                .relative()
                .flex_none()
                .h_full()
                .child(tabs::column(toolbar::header(can_split, on_act.clone(), cx), ui, &label, on_pick, window, cx))
                .children(card)
        });
        h_flex()
            .size_full()
            .min_h_0()
            .bg(look::hsla_of(self.look.colors.background).opacity(self.look.opacity))
            .text_color(p.foreground)
            .child(area)
            .children(tab_column)
            .into_any_element()
    }

    /// Focus moves with what shows: to the active split when asked, onto the overlay when it rises over a focused
    /// pane, and back to the split when it lifts while holding focus.
    fn place_focus(&mut self, covered: bool, ui: &Ui, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.view(&ui.active_terminal_id).map(|v| v.focus_handle(cx));
        let held = self.views.iter().any(|(_, v)| v.focus_handle(cx).contains_focused(window, cx));
        if covered {
            if held || (self.focus_active && active.is_some()) {
                self.overlay_focus.focus(window, cx);
                self.focus_active = false;
            }
            return;
        }
        if self.overlay_focus.is_focused(window) {
            self.focus_active = true;
        }
        if self.focus_active {
            if let Some(active) = active {
                active.focus(window, cx);
                self.focus_active = false;
            }
        }
    }
}

/// "Terminal config not read from the host: <said>. The pane keeps its defaults." (`ThreadTerminalDrawer.tsx`).
fn config_failed(failure: &Failure) {
    eprintln!("desktop-gpui: Terminal config not read from the host: {}. The pane keeps its defaults.", failure.message);
}

/// `terminalRefusedLine` in `actions/format.ts`.
pub fn refused_line(computer: Option<&str>, said: &str) -> String {
    match computer {
        Some(name) => format!("No terminal on {name}: {said}"),
        None => format!("No terminal: {said}"),
    }
}

impl Render for Drawer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.follow_look(window, cx);
        let ui = self.ui(cx);
        let pane = self.pane(cx);
        let covered = words::title(&pane).is_some() && !ui.terminal_ids.is_empty();
        self.place_focus(covered, &ui, window, cx);
        let p = cx.palette().clone();
        let body = self.body(&ui, &pane, window, cx);
        let menu = self.menu.overlay();
        let root = v_flex()
            .id("terminal-drawer")
            .debug_selector(|| "terminal-drawer".into())
            .track_focus(&self.focus)
            .relative()
            .min_w_0()
            .overflow_hidden()
            .when(!self.see_through(), |el| el.bg(p.background))
            .on_action(cx.listener(|this, _: &keys::SplitTerminal, window, cx| this.by_key(Act::Split, window, cx)))
            .on_action(cx.listener(|this, _: &keys::NewTerminal, window, cx| this.by_key(Act::New, window, cx)))
            .on_action(cx.listener(|this, _: &keys::ZoomInTerminal, _, cx| this.zoom(Some(1.), cx)))
            .on_action(cx.listener(|this, _: &keys::ZoomOutTerminal, _, cx| this.zoom(Some(-1.), cx)))
            .on_action(cx.listener(|this, _: &keys::ResetTerminalZoom, _, cx| this.zoom(None, cx)));
        match &self.mode {
            Mode::Panel { .. } => root.size_full().flex_1().child(body).children(menu),
            Mode::Drawer(_) => {
                let height = self.height(window, cx);
                root.flex_none()
                    .w_full()
                    .h(px(height))
                    .border_t_1()
                    .border_color(p.border)
                    .on_drag_move(cx.listener(|this, e: &DragMoveEvent<Resize>, window, cx| {
                        if let Some((grab, start, _)) = this.drag {
                            let now = start + f32::from(grab - e.event.position.y);
                            let clamped = clamp_height(now, f32::from(window.viewport_size().height));
                            this.drag = Some((grab, start, clamped));
                            cx.notify();
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, window, cx| this.end_drag(window, cx)))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, window, cx| this.end_drag(window, cx)))
                    .child(body)
                    .children(menu)
                    .child(
                        div()
                            .id("terminal-drawer-strip")
                            .debug_selector(|| "terminal-drawer-strip".into())
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(STRIP))
                            .cursor_row_resize()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, e: &MouseDownEvent, _, _| this.drag = Some((e.position.y, height, height))),
                            )
                            .on_drag(Resize, |_, _, _, cx| cx.new(|_| Nothing)),
                    )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_height_holds_between_180_px_and_three_quarters_of_the_window() {
        assert_eq!(clamp_height(100., 800.), 180.);
        assert_eq!(clamp_height(700., 800.), 600.);
        assert_eq!(clamp_height(280.4, 800.), 280.);
        assert_eq!(clamp_height(300., 200.), 180., "a window too short for both keeps the floor");
    }

    #[test]
    fn a_refused_open_names_the_computer() {
        assert_eq!(refused_line(Some("Box"), "no daemon"), "No terminal on Box: no daemon");
        assert_eq!(refused_line(None, "no daemon"), "No terminal: no daemon");
    }
}
