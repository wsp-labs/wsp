// SPDX-License-Identifier: AGPL-3.0-only
//! The window's content: the frame of `screens::app_shell` around the sidebar, the open thread or screen and the right
//! panel, the host connection behind them, and bench mode's run.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Theme};
use gpui_kit::{
    div, prelude::*, px, AnyView, App, Context, DragMoveEvent, Empty, Entity, FocusHandle, KeyBinding, KeyContext, KeyDownEvent,
    MouseButton, MouseDownEvent, Pixels, SharedString, StyleRefinement, Subscription, Task, Window,
};
use tokio::runtime::Runtime;
use wsp_host_client::ops::settings::PreferencesReply;
use wsp_host_client::protocol::WorkspacesListRequest;
use wsp_host_client::protocol::{EventUnion, Preferences, PreferencesGetRequest, PreferencesSetRequest, SessionEvent};
use wsp_host_client::{Env, Event, Failure, HostClient, Live};

use crate::bench::{self, PX_PER_S};
use crate::glass::composer_glass;
use crate::screens::app_shell::frame;
use crate::screens::app_shell::keys::{self, Bound};
use crate::screens::app_shell::material::{self, Look};
use crate::screens::app_shell::panel::{At, RightPanel};
use crate::screens::app_shell::vibrancy::{self, Vibrancy};
use crate::screens::app_shell::AbortOnDrop;
use crate::screens::composer::screen::ComposerScreen;
use crate::screens::{self, app_shell, dialogs, Entry, Host, Mounted, Opened, Pane, Place, Route, Screen, THREAD};
use crate::sidebar::{Asked, Link, Selected, Sidebar, ThreadItem};
use crate::thread_view::ThreadView;
use crate::ui::theme::WspTheme;
use crate::ui::{self, ActivePalette};

/// `transition-[left,right,width] duration-200 ease-linear` on the sidebar.
const SIDEBAR_SLIDE: Duration = Duration::from_millis(200);

/// The screen `mod+,` opens; the shell knows it by name because it hides the right panel and closes on Escape.
pub const SETTINGS: &str = "settings";

/// A bench that has not finished by then is stuck, and says so rather than hang the run.
const BENCH_LIMIT: Duration = Duration::from_secs(300);
/// GPUI caps a window without focus at 30 fps, so a bench measures only a focused window and stops when it has none.
const FOCUS_WAIT: Duration = Duration::from_secs(10);
const UNFOCUSED: i32 = 3;
/// How long a bench holds still after its scroll, so the run can read the app's memory after a full scroll.
const HOLD: Duration = Duration::from_secs(10);

enum Arrival {
    History(Vec<SessionEvent>),
    Event(Box<SessionEvent>),
    Failed(String),
}

/// One thread followed live; dropping it stops the host task and the receiver.
struct Following {
    thread_id: String,
    task: tokio::task::AbortHandle,
    _receive: Task<()>,
}

impl Drop for Following {
    fn drop(&mut self) {
        self.task.abort();
    }
}

type Presented = Rc<RefCell<Option<Instant>>>;

enum Phase {
    Opening,
    Loading(Presented),
    Scrolling { start: Instant, frames: Rc<RefCell<Vec<Instant>>> },
    Holding { until: Instant, _wake: Task<()> },
    Switching { ix: usize, start: Instant, presented: Presented },
}

struct BenchRun {
    args: bench::Bench,
    began: Instant,
    phase: Phase,
    rows: usize,
    frame_ms: Vec<f64>,
    switch: Vec<(String, f64)>,
}

/// What the launch asked the window to show first.
#[derive(Default)]
pub struct Launch {
    pub bench: Option<bench::Bench>,
    /// A thread to open once the host lists it.
    pub open: Option<String>,
    /// A frosted, composer-sized panel over the bottom of the thread view.
    pub glass: bool,
    /// A screen to open by its name once the host answers.
    pub screen: Option<String>,
}

pub struct Shell {
    sidebar: Entity<Sidebar>,
    thread_view: Entity<ThreadView>,
    panel: Entity<RightPanel>,
    host: Option<Arc<HostClient>>,
    runtime: Arc<Runtime>,
    route: Route,
    /// Where closing Settings goes back to.
    behind: Route,
    /// The open page, while the route names a registered one.
    screen: Option<Mounted>,
    /// The registered thread view, built for the first thread route and kept, told of each switch after it.
    thread: Option<Mounted>,
    /// Whether each workspace the host listed runs, which the right panel's panes read.
    running: HashMap<String, bool>,
    sidebar_open: bool,
    /// When the sidebar last opened or closed, while its slide runs.
    sidebar_moved: Option<Instant>,
    sidebar_width: Pixels,
    /// The rail a drag holds, until its release, and how far from its centre line the press took hold of it, so a rail
    /// held off its centre does not jump to the pointer.
    dragging: Option<Side>,
    grab: Pixels,
    /// The terminal drawer under the thread, whose own screen draws it.
    drawer_open: bool,
    /// The chord rules bound now, so a change of the person's overrides can unbind exactly them.
    rules: Vec<Bound>,
    theme: Option<String>,
    picks: Picks,
    look: Look,
    vibrancy: Vibrancy,
    focus: FocusHandle,
    blank: AnyView,
    following: Option<Following>,
    bench: Option<BenchRun>,
    glass: bool,
    /// The centre while the window shows nothing, which is first run while the host holds no project.
    first_run: Option<Entity<dialogs::first_run::FirstRun>>,
    _connect: Task<()>,
    _events: Option<Task<()>>,
    _selected: Subscription,
    _asked: Subscription,
    _focus_lost: Subscription,
    _appearance: Subscription,
    _activation: Subscription,
}

/// The person's theme picks: light, dark or the system's (none), each side's theme, and whether the page draws glass.
struct Picks {
    mode: Option<String>,
    light: String,
    dark: String,
    transparency: bool,
}

/// The rail being dragged, which the root reads the pointer for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Sidebar,
    Panel,
}

/// What a rail's drag draws under the pointer: nothing.
struct Dragging;

impl Render for Dragging {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// Whether the shell's chords are bound: a shell built again once a pairing lands binds none twice.
#[derive(Default)]
struct KeysBound(bool);

impl gpui_kit::Global for KeysBound {}

impl Shell {
    pub fn new(state: PathBuf, env: Env, runtime: Arc<Runtime>, launch: Launch, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Launch { bench, open, glass, screen } = launch;
        app_shell::install(window.window_handle(), cx.entity().downgrade(), cx);
        let sidebar = cx.new(Sidebar::new);
        let thread_view = cx.new(ThreadView::new);
        let panel = cx.new(RightPanel::new);
        let selected = cx.subscribe_in(&sidebar, window, |this, _, Selected(item), window, cx| {
            let route = Route::Thread { workspace_id: item.workspace_id.clone(), thread_id: item.thread_id.clone() };
            this.go(route, window, cx);
        });
        let asked = cx.subscribe_in(&sidebar, window, |this, _, asked: &Asked, window, cx| match asked {
            Asked::Settings => this.go(Route::Screen(SETTINGS.into()), window, cx),
            Asked::Usage => this.go(Route::Screen(format!("{SETTINGS}:usage").into()), window, cx),
            Asked::Activity => this.go(Route::Screen(format!("{SETTINGS}:activity").into()), window, cx),
            Asked::NewThread => {
                if let Some(workspace) = this.workspace_on_screen(cx) {
                    this.go(Route::Screen(format!("{}:{workspace}", ComposerScreen::NAME).into()), window, cx);
                }
            }
            Asked::NewThreadIn(workspace) => this.go(Route::Screen(format!("{}:{workspace}", ComposerScreen::NAME).into()), window, cx),
            Asked::Search => {}
            Asked::AddProject => this.with_host(|host, window, cx| dialogs::open_add_project(&host, window, cx), window, cx),
            Asked::AddComputer => this.with_host(|host, window, cx| dialogs::open_add_computer(&host, None, window, cx), window, cx),
            Asked::AddCloud => this.with_host(|host, window, cx| dialogs::open_add_cloud(&host, window, cx), window, cx),
            Asked::Setup(id) => {
                let id = id.clone();
                this.with_host(move |host, window, cx| dialogs::open_add_computer(&host, Some(id), window, cx), window, cx)
            }
            Asked::ExportProject(workspace) => {
                let workspace = workspace.clone();
                this.with_host(move |host, window, cx| dialogs::open_export_project(&host, workspace, window, cx), window, cx)
            }
            Asked::OpenTerminal(_) => {}
            Asked::OpenBrowser(_) => {}
            Asked::ProjectSettings(_) => {}
            Asked::ComputerSettings(_) => {}
        });
        let page = crate::sidebar::host_page(&state);
        let (pair_state, pair_env) = (state.clone(), env.clone());
        let bought = dialogs::pair::bought(cx);
        let connecting = runtime.spawn(async move {
            let host = HostClient::connect_holding(&state, &env, bought).await?;
            let threads = host.threads().await?;
            let list: WorkspacesListRequest = serde_json::from_value(serde_json::json!({})).map_err(|e| Failure::new(e.to_string()))?;
            let workspaces = host.call(list).await.map(|w| w.workspaces).unwrap_or_default();
            let get: PreferencesGetRequest = serde_json::from_value(serde_json::json!({})).map_err(|e| Failure::new(e.to_string()))?;
            let preferences = host.call(get).await.map(|PreferencesReply { preferences }| preferences).ok();
            Ok::<_, Failure>((host, threads, workspaces, preferences))
        });
        let connect = cx.spawn_in(window, async move |this, cx| {
            let connected = connecting.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
            _ = this.update_in(cx, |this, window, cx| match connected {
                Ok((host, threads, workspaces, preferences)) => {
                    for workspace in &workspaces {
                        let value = serde_json::to_value(workspace).unwrap_or_default();
                        if let (Some(id), Some(phase)) = (value["id"].as_str(), value["phase"].as_str()) {
                            this.running.insert(id.to_owned(), phase == "running");
                        }
                    }
                    let host = Arc::new(host);
                    this.host = Some(host.clone());
                    let runtime = this.runtime.clone();
                    this.panel.update(cx, |p, cx| p.set_host(Host::new(host.clone(), runtime.clone()), window, cx));
                    let (center, first) = (this.thread_view.clone().into(), Host::new(host.clone(), runtime.clone()));
                    this.first_run = Some(cx.new(|cx| dialogs::first_run::FirstRun::new(first, page.clone(), center, cx)));
                    this.sidebar.update(cx, |s, cx| s.attach(Link { host: host.clone(), runtime, page }, cx));
                    if let Some(preferences) = &preferences {
                        this.apply(preferences, window, cx);
                    }
                    this._events = Some(this.follow_preferences(host, window, cx));
                    if let (Route::Screen(route), None) = (this.route.clone(), &this.screen) {
                        this.screen = this.mount(&route, window, cx);
                    }
                    this.sidebar.update(cx, |s, cx| s.set_items(threads.iter().filter_map(ThreadItem::of).collect(), cx));
                    if let Some(name) = &screen {
                        this.go(Route::Screen(name.clone().into()), window, cx);
                    }
                    if let Some(thread) = &open {
                        let Some(item) = this.sidebar.read(cx).items().iter().find(|i| i.thread_id == *thread).cloned() else {
                            eprintln!("desktop-gpui: the host lists no thread {thread}");
                            std::process::exit(1);
                        };
                        this.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx);
                    }
                }
                Err(failure) => {
                    eprintln!("desktop-gpui: {}", failure.message);
                    let unpaired = failure.kind.as_deref() == Some("auth");
                    this.sidebar.update(cx, |s, cx| s.set_note(failure.message, cx));
                    if this.bench.is_some() {
                        std::process::exit(1);
                    }
                    if unpaired {
                        dialogs::pair::open(dialogs::pair::Road::host(pair_state, pair_env, this.runtime.clone()), window, cx);
                    }
                }
            });
        });
        if bench.is_some() {
            // A window that stops drawing (covered, minimized) never ticks, so the limit runs on a timer of its own.
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(BENCH_LIMIT).await;
                let stuck = this.read_with(cx, |this, cx| this.bench_stuck(cx)).unwrap_or_else(|_| "its window gone".to_owned());
                eprintln!("desktop-gpui: the bench did not finish in {} s, {stuck}", BENCH_LIMIT.as_secs());
                std::process::exit(2);
            })
            .detach();
        }
        let bench = bench.map(|args| BenchRun {
            args,
            began: Instant::now(),
            phase: Phase::Opening,
            rows: 0,
            frame_ms: Vec::new(),
            switch: Vec::new(),
        });
        let mac = cfg!(target_os = "macos");
        let rules = keys::bound(mac, &BTreeMap::new());
        if !std::mem::replace(&mut cx.default_global::<KeysBound>().0, true) {
            cx.bind_keys(keys::bindings_of(mac, &rules));
            cx.bind_keys([KeyBinding::new("escape", keys::CloseSettings, Some(&format!("{} && {}", keys::SHELL, keys::SETTINGS_OPEN)))]);
        }
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        // Focus on a view that is no longer drawn reaches nothing, and keys then start at the window's root, above the
        // shell's bindings.
        let focus_lost = cx.on_focus_lost(window, |this, window, cx| this.focus.focus(window, cx));
        let appearance = cx.observe_window_appearance(window, |this, window, cx| this.pick_theme(window, cx));
        // Reduce transparency has no window callback and is switched in System Settings, so it is read again on return.
        let activation = cx.observe_window_activation(window, |this, window, cx| this.pick_theme(window, cx));
        let mut shell = Shell {
            sidebar,
            thread_view,
            panel,
            host: None,
            runtime,
            route: Route::Empty,
            behind: Route::Empty,
            screen: None,
            thread: None,
            running: HashMap::new(),
            sidebar_open: true,
            sidebar_moved: None,
            sidebar_width: frame::SIDEBAR_DEFAULT,
            dragging: None,
            grab: px(0.),
            drawer_open: false,
            rules,
            theme: None,
            picks: Picks { mode: None, light: ui::theme::LIGHT.to_owned(), dark: ui::theme::DARK.to_owned(), transparency: true },
            look: Look { mac: false, glass: false },
            vibrancy: Vibrancy::default(),
            focus,
            blank: cx.new(|_| Blank).into(),
            following: None,
            bench,
            glass,
            first_run: None,
            _connect: connect,
            _events: None,
            _selected: selected,
            _asked: asked,
            _focus_lost: focus_lost,
            _appearance: appearance,
            _activation: activation,
        };
        shell.pick_theme(window, cx);
        shell
    }

    /// Where the window is drawn and whether the page draws glass, as the host's transparency and the computer say.
    pub fn look(&self) -> Look {
        self.look
    }

    /// Runs `open` with the connected host; a press before the host answers does nothing, as no flow can start.
    fn with_host(&self, open: impl FnOnce(Host, &mut Window, &mut App), window: &mut Window, cx: &mut Context<Self>) {
        if let Some(host) = self.host.clone() {
            open(Host::new(host, self.runtime.clone()), window, cx);
        }
    }

    pub fn connected(&self) -> bool {
        self.host.is_some()
    }

    /// The chords bound for `command` now, in the web app's spelling.
    pub fn chords(&self, command: &str) -> Vec<String> {
        self.rules.iter().filter(|r| r.command == command).map(|r| r.chord.clone()).collect()
    }

    pub fn route(&self) -> &Route {
        &self.route
    }

    pub fn sidebar_open(&self) -> bool {
        self.sidebar_open
    }

    pub fn sidebar_width(&self) -> Pixels {
        self.sidebar_width
    }

    pub fn drawer_open(&self) -> bool {
        self.drawer_open
    }

    pub fn panel(&self) -> &Entity<RightPanel> {
        &self.panel
    }

    pub fn sidebar(&self) -> &Entity<Sidebar> {
        &self.sidebar
    }

    pub fn thread_view(&self) -> &Entity<ThreadView> {
        &self.thread_view
    }

    /// How much of the sidebar's width is on screen, 0 to 1, asking for the next frame while it slides.
    fn sidebar_shown(&mut self, window: &mut Window) -> f32 {
        let at = self.sidebar_moved.map_or(1., |moved| (moved.elapsed().as_secs_f32() / SIDEBAR_SLIDE.as_secs_f32()).min(1.));
        if at < 1. {
            window.request_animation_frame();
        } else {
            self.sidebar_moved = None;
        }
        if self.sidebar_open {
            at
        } else {
            1. - at
        }
    }

    /// A screen in the main column, Settings or another, is a page: the panel hides and Escape goes back, as
    /// `Place::Main` says, and it is blank until its screen mounts.
    fn settings_open(&self) -> bool {
        matches!(self.route, Route::Screen(_))
    }

    /// Moves the window to `route`; [`app_shell::navigate`] is the way in for everything outside the shell. A screen
    /// that lives in the right panel opens there, on its tab, and leaves the route where it was.
    pub fn go(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        if route == self.route {
            return;
        }
        if let Route::Screen(route) = &route {
            let (name, arg) = app_shell::split(route);
            if let Some(Entry { place: Place::Panel(pane), .. }) = screens::screen(name) {
                let tab = arg.map(|arg| SharedString::from(arg.to_owned()));
                self.panel.update(cx, |p, cx| p.add(pane, tab, window, cx));
                return;
            }
        }
        match &route {
            Route::Empty => {
                self.screen = None;
                self.following = None;
                self.panel.update(cx, |p, cx| p.show_for(At::Here, window, cx));
                if let Some(thread) = &self.thread {
                    (thread.show.clone())(Opened::default(), window, cx);
                }
            }
            Route::Thread { workspace_id, thread_id } => {
                self.screen = None;
                let item = self.sidebar.update(cx, |s, cx| s.mark(thread_id, cx)).unwrap_or_else(|| ThreadItem {
                    workspace_id: workspace_id.clone(),
                    thread_id: thread_id.clone(),
                    project: String::new(),
                    title: String::new(),
                });
                let running = self.running.get(workspace_id).copied().unwrap_or(false);
                let at = At::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone(), running };
                self.panel.update(cx, |p, cx| p.show_for(at, window, cx));
                let opened = Opened { workspace_id: Some(workspace_id.clone()), thread_id: Some(thread_id.clone()), ..Opened::default() };
                match (&self.thread, self.host.clone(), screens::screen(THREAD)) {
                    (Some(thread), _, _) => (thread.show.clone())(opened, window, cx),
                    (None, Some(host), Some(entry)) => {
                        self.following = None;
                        self.thread = Some((entry.mount)(&Host::new(host, self.runtime.clone()), opened, window, cx));
                    }
                    // Settings leaves the thread followed behind it, so coming back reads nothing again.
                    _ if self.following.as_ref().is_none_or(|f| f.thread_id != *thread_id) => self.open(item, cx),
                    _ => {}
                }
            }
            Route::Screen(name) => {
                if !matches!(self.route, Route::Screen(_)) {
                    self.behind = self.route.clone();
                }
                let open = match (&self.route, &self.screen) {
                    (Route::Screen(open), Some(page)) if app_shell::split(open).0 == app_shell::split(name).0 => Some(page.show.clone()),
                    _ => None,
                };
                match open {
                    Some(show) => show(self.page(name), window, cx),
                    None => self.screen = self.mount(name, window, cx),
                }
            }
        }
        self.route = route;
        cx.notify();
    }

    /// The workspace of the thread on screen or left behind a page, else of the newest thread the sidebar lists.
    fn workspace_on_screen(&self, cx: &App) -> Option<String> {
        match (&self.route, &self.behind) {
            (Route::Thread { workspace_id, .. }, _) | (_, Route::Thread { workspace_id, .. }) => Some(workspace_id.clone()),
            _ => self.sidebar.read(cx).items().first().map(|i| i.workspace_id.clone()),
        }
    }

    /// What the page `route` names is opened on: the thread the window was showing, and the route's argument.
    fn page(&self, route: &str) -> Opened {
        let (workspace_id, thread_id) = match &self.behind {
            Route::Thread { workspace_id, thread_id } => (Some(workspace_id.clone()), Some(thread_id.clone())),
            _ => (None, None),
        };
        Opened { workspace_id, thread_id, tab: None, arg: app_shell::split(route).1.map(|a| SharedString::from(a.to_owned())) }
    }

    /// Builds the page `route` names once the host has answered.
    fn mount(&self, route: &str, window: &mut Window, cx: &mut Context<Self>) -> Option<Mounted> {
        let (host, entry) = (self.host.clone()?, screens::screen(app_shell::split(route).0)?);
        Some((entry.mount)(&Host::new(host, self.runtime.clone()), self.page(route), window, cx))
    }

    /// Takes what the host says of whether `workspace_id` runs, which decides the panes its threads can open.
    pub fn workspace_runs(&mut self, workspace_id: &str, running: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.running.insert(workspace_id.to_owned(), running);
        if let Route::Thread { workspace_id: open, thread_id } = &self.route {
            if open == workspace_id {
                let at = At::Thread { workspace_id: open.clone(), thread_id: thread_id.clone(), running };
                self.panel.update(cx, |p, cx| p.show_for(at, window, cx));
            }
        }
    }

    /// Reads what the host keeps for the frame: the sidebar's width, the person's chords and the theme.
    fn apply(&mut self, preferences: &Preferences, window: &mut Window, cx: &mut Context<Self>) {
        let value = serde_json::to_value(preferences).unwrap_or_default();
        self.sidebar_width = value.get("sidebarWidth").and_then(|w| w.as_f64()).map_or(frame::SIDEBAR_DEFAULT, |w| px(w as f32));
        let overrides: BTreeMap<String, Option<String>> = value
            .get("keybindings")
            .and_then(|k| k.as_object())
            .map(|k| k.iter().map(|(command, chord)| (command.clone(), chord.as_str().map(str::to_owned))).collect())
            .unwrap_or_default();
        self.rebind(&overrides, cx);
        let side = |key: &str, default: &str| value.get(key).and_then(|t| t.as_str()).unwrap_or(default).to_owned();
        self.picks = Picks {
            mode: value.get("theme").and_then(|t| t.as_str()).map(str::to_owned),
            light: side("lightTheme", ui::theme::LIGHT),
            dark: side("darkTheme", ui::theme::DARK),
            transparency: value.get("transparency").and_then(|t| t.as_bool()).unwrap_or(true),
        };
        // Electron's nativeTheme.themeSource: the lights, the window's edge and macOS's material take the pick.
        cx.set_window_appearance(match self.picks.mode.as_deref() {
            Some("dark") => Some(gpui_kit::WindowAppearance::Dark),
            Some("light") => Some(gpui_kit::WindowAppearance::Light),
            _ => None,
        });
        self.pick_theme(window, cx);
        cx.notify();
    }

    /// The theme the person's picks name for the computer's appearance now, which a system pick follows as it moves.
    fn pick_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = match self.picks.mode.as_deref() {
            Some("dark") => true,
            Some("light") => false,
            _ => matches!(window.appearance(), gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark),
        };
        let theme = if dark { self.picks.dark.clone() } else { self.picks.light.clone() };
        if ui::theme::theme_by_id(&theme).is_some() {
            self.restyle(Some(theme), self.picks.transparency, window, cx);
        }
    }

    /// Draws the window in `theme` and, on the Mac, on its glass or solid as `transparency` and the computer ask.
    fn restyle(&mut self, theme: Option<String>, transparency: bool, window: &mut Window, cx: &mut Context<Self>) {
        let look = Look { mac: cfg!(target_os = "macos"), glass: transparency && !vibrancy::reduce_transparency() };
        if theme == self.theme && look == self.look {
            return;
        }
        if let Some(theme) = &theme {
            ui::theme::set_theme(theme, cx);
        }
        let fills = material::fills(look, ui::theme::current(cx).id, cx.palette());
        // The glass moves the page's muted ink, which every screen reads off the palette.
        if let Some(muted) = fills.muted_foreground {
            cx.global_mut::<WspTheme>().palette.muted_foreground = muted;
            Theme::update(cx, |theme| theme.muted_foreground = muted);
        }
        self.vibrancy.set(fills.vibrancy, window);
        self.theme = theme;
        self.look = look;
        window.refresh();
    }

    /// Puts the person's chords in place of the bound ones: each bound rule is unbound by its action, then the new
    /// table is bound after it, so it wins.
    fn rebind(&mut self, overrides: &BTreeMap<String, Option<String>>, cx: &mut Context<Self>) {
        let mac = cfg!(target_os = "macos");
        let next = keys::bound(mac, overrides);
        if next == self.rules {
            return;
        }
        cx.bind_keys(keys::unbindings(mac, &self.rules));
        cx.bind_keys(keys::bindings_of(mac, &next));
        self.rules = next;
    }

    /// Follows the host's pushed preferences, as another window or the Settings page changes them.
    fn follow_preferences(&mut self, host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        let (tx, mut rx) = mpsc::unbounded();
        let task = self.runtime.spawn(async move {
            let Ok(mut events) = host.events().await else { return };
            while let Some(event) = events.next().await {
                if let Ok(Event::Host(event)) = event {
                    if let EventUnion::PreferencesChanged { preferences, .. } = *event {
                        if tx.unbounded_send(preferences).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let _abort = AbortOnDrop(task.abort_handle());
            while let Some(preferences) = rx.next().await {
                if this.update_in(cx, |this, window, cx| this.apply(&preferences, window, cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn end_drag(&mut self) {
        if self.dragging.take() == Some(Side::Sidebar) {
            self.save_sidebar_width();
        }
    }

    /// Writes the sidebar's width, as the web app writes it once a drag ends.
    fn save_sidebar_width(&self) {
        let Some(host) = self.host.clone() else { return };
        let width = f32::from(self.sidebar_width).round() as i64;
        let request: Result<PreferencesSetRequest, _> = serde_json::from_value(serde_json::json!({ "patch": { "sidebarWidth": width } }));
        let Ok(request) = request else { return };
        self.runtime.spawn(async move {
            if let Err(failure) = host.call(request).await {
                eprintln!("desktop-gpui: {}", failure.message);
            }
        });
    }

    fn toggle_sidebar(&mut self, _: &keys::ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        // Turned back mid-slide, it goes back from where it stands rather than from the far end.
        let done = self.sidebar_moved.map_or(SIDEBAR_SLIDE, |moved| moved.elapsed().min(SIDEBAR_SLIDE));
        self.sidebar_moved = Some(Instant::now() - (SIDEBAR_SLIDE - done));
        cx.notify();
    }

    fn toggle_panel(&mut self, _: &keys::ToggleRightPanel, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_open() {
            self.panel.update(cx, |p, cx| p.toggle(window, cx));
        }
    }

    fn toggle_preview(&mut self, _: &keys::TogglePreview, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_open() && matches!(self.route, Route::Thread { .. }) {
            self.panel.update(cx, |p, cx| p.toggle_pane(Pane::Browser, window, cx));
        }
    }

    fn toggle_drawer(&mut self, _: &keys::ToggleTerminal, _: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_open() {
            self.drawer_open = !self.drawer_open;
            cx.notify();
        }
    }

    fn toggle_settings(&mut self, _: &keys::ToggleSettings, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(&self.route, Route::Screen(route) if app_shell::split(route).0 == SETTINGS) {
            self.close_settings(&keys::CloseSettings, window, cx);
        } else {
            self.go(Route::Screen(SETTINGS.into()), window, cx);
        }
    }

    fn close_settings(&mut self, _: &keys::CloseSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open() {
            let behind = std::mem::replace(&mut self.behind, Route::Empty);
            self.screen = None;
            self.go(behind, window, cx);
        }
    }

    fn close_panel_tab(&mut self, _: &keys::ClosePanelTab, window: &mut Window, cx: &mut Context<Self>) {
        self.panel.update(cx, |p, cx| {
            if let Some(ix) = p.active_index() {
                p.close(ix, window, cx);
            }
        });
    }

    /// The workspaces in the order the sidebar draws them, each with its first thread there.
    fn workspaces(&self, cx: &App) -> Vec<ThreadItem> {
        let mut seen: Vec<ThreadItem> = Vec::new();
        for item in self.sidebar.read(cx).items() {
            if !seen.iter().any(|s| s.workspace_id == item.workspace_id) {
                seen.push(item.clone());
            }
        }
        seen
    }

    fn select_workspace(&mut self, slot: &keys::SelectWorkspace, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.workspaces(cx).into_iter().nth(usize::from(slot.0).saturating_sub(1)) else { return };
        self.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx);
    }

    /// Moves `step` threads along `list` from the open one, wrapping.
    fn step_thread(&mut self, list: Vec<ThreadItem>, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        if list.is_empty() {
            return;
        }
        let open = match &self.route {
            Route::Thread { thread_id, .. } => list.iter().position(|i| i.thread_id == *thread_id),
            _ => None,
        };
        let n = list.len() as isize;
        let ix = open.map_or(if step > 0 { 0 } else { n - 1 }, |ix| (ix as isize + step).rem_euclid(n));
        let item = list[ix as usize].clone();
        self.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx);
    }

    fn workspace_threads(&self, cx: &App) -> Vec<ThreadItem> {
        let Route::Thread { workspace_id, .. } = &self.route else { return Vec::new() };
        self.sidebar.read(cx).items().iter().filter(|i| i.workspace_id == *workspace_id).cloned().collect()
    }

    /// Shows `item`'s transcript, read whole, then follows it live.
    fn open(&mut self, item: ThreadItem, cx: &mut Context<Self>) {
        self.following = None;
        self.thread_view.update(cx, |v, cx| v.show(&item.thread_id, cx));
        let Some(host) = self.host.clone() else { return };
        let thread_id = item.thread_id.clone();
        let (tx, mut rx) = mpsc::unbounded();
        let task = self.runtime.spawn(async move {
            let followed: Result<(), Failure> = async {
                let mut thread = host.open(&item.workspace_id, &item.thread_id).await?;
                _ = tx.unbounded_send(Arrival::History(std::mem::take(&mut thread.history)));
                while let Some(next) = thread.next().await {
                    if let Live::Event(event) = next? {
                        _ = tx.unbounded_send(Arrival::Event(event));
                    }
                }
                Ok(())
            }
            .await;
            if let Err(failure) = followed {
                _ = tx.unbounded_send(Arrival::Failed(failure.message));
            }
        });
        let receive = cx.spawn(async move |this, cx| {
            while let Some(arrival) = rx.next().await {
                if this.update(cx, |this, cx| this.arrive(arrival, cx)).is_err() {
                    break;
                }
            }
        });
        self.following = Some(Following { thread_id, task: task.abort_handle(), _receive: receive });
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        let benching = self.bench.is_some();
        self.thread_view.update(cx, |v, cx| match arrival {
            Arrival::History(events) => {
                v.load(events, cx);
                v.scroll_to_end(cx);
            }
            Arrival::Event(event) => v.push(*event, cx),
            Arrival::Failed(failure) => {
                if benching {
                    eprintln!("desktop-gpui: {failure}");
                    std::process::exit(1);
                }
                v.fail(failure, cx)
            }
        });
    }

    /// One step of bench mode, run on every frame until it writes its result and quits.
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut run) = self.bench.take() else { return };
        let measuring = matches!(run.phase, Phase::Scrolling { .. } | Phase::Switching { .. });
        if !window.is_window_active() && (measuring || run.began.elapsed() > FOCUS_WAIT) {
            eprintln!("desktop-gpui: the bench window has no focus, and GPUI caps a window without focus at 30 fps");
            std::process::exit(UNFOCUSED);
        }
        match &run.phase {
            Phase::Opening => {
                if self.host.is_some() {
                    cx.activate(true);
                    window.activate_window();
                    run.phase = Phase::Loading(self.bench_open(&run.args.thread, window, cx));
                }
            }
            Phase::Loading(presented) => {
                if presented.borrow().is_some() && window.is_window_active() {
                    let (start, frames) = (Instant::now(), Rc::new(RefCell::new(Vec::new())));
                    run.rows = self.thread_view.read(cx).rows().len();
                    self.thread_view.update(cx, |v, cx| v.drive(PX_PER_S, start, frames.clone(), cx));
                    run.phase = Phase::Scrolling { start, frames };
                }
            }
            Phase::Scrolling { start, frames } => {
                if self.thread_view.read(cx).driven_to_end() {
                    run.frame_ms = frames.borrow().iter().map(|t| t.duration_since(*start).as_secs_f64() * 1000.).collect();
                    eprintln!("bench: scrolled");
                    let wake = cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(HOLD).await;
                        _ = this.update(cx, |_, cx| cx.notify());
                    });
                    run.phase = Phase::Holding { until: Instant::now() + HOLD, _wake: wake };
                }
            }
            Phase::Holding { until, .. } => {
                if Instant::now() >= *until {
                    run.phase = self.switch_to(0, &run.args, window, cx);
                }
            }
            Phase::Switching { ix, start, presented } => {
                let shown = *presented.borrow();
                if let Some(shown) = shown {
                    run.switch.push((run.args.switch[*ix].clone(), shown.duration_since(*start).as_secs_f64() * 1000.));
                    run.phase = self.switch_to(ix + 1, &run.args, window, cx);
                }
            }
        }
        if matches!(run.phase, Phase::Switching { ix, .. } if ix >= run.args.switch.len()) {
            let json = bench::result(run.rows, &run.frame_ms, &run.switch);
            if let Err(e) = std::fs::write(&run.args.out, format!("{json}\n")) {
                eprintln!("desktop-gpui: {}: {e}", run.args.out.display());
                std::process::exit(1);
            }
            cx.quit();
            return;
        }
        let holding = matches!(run.phase, Phase::Holding { .. });
        self.bench = Some(run);
        if !holding {
            window.request_animation_frame();
        }
    }

    /// Where a bench past its limit waits, so a run on another machine says it.
    fn bench_stuck(&self, cx: &App) -> String {
        let list = self.thread_view.read(cx).list();
        match self.bench.as_ref().map(|run| &run.phase) {
            Some(Phase::Opening) => "waiting for the host or a first frame".to_owned(),
            Some(Phase::Loading(_)) => "waiting for the thread's first frame and the window's focus".to_owned(),
            Some(Phase::Scrolling { .. }) => format!(
                "scrolling at {:?} of {:?}, the end check {:?}",
                -list.scroll_px_offset_for_scrollbar().y,
                list.max_offset_for_scrollbar().y,
                list.is_scrolled_to_end()
            ),
            Some(Phase::Holding { .. }) => "holding after the scroll".to_owned(),
            Some(Phase::Switching { ix, .. }) => format!("waiting for switch {} to draw", ix + 1),
            None => "past its last step".to_owned(),
        }
    }

    fn switch_to(&mut self, ix: usize, args: &bench::Bench, window: &mut Window, cx: &mut Context<Self>) -> Phase {
        let start = Instant::now();
        let presented = args.switch.get(ix).map(|thread| self.bench_open(thread, window, cx)).unwrap_or_default();
        Phase::Switching { ix, start, presented }
    }

    /// Opens a thread as a click does and watches for the first presented frame that shows it.
    fn bench_open(&mut self, thread: &str, window: &mut Window, cx: &mut Context<Self>) -> Presented {
        let Some(item) = self.sidebar.read(cx).items().iter().find(|i| i.thread_id == thread).cloned() else {
            eprintln!("desktop-gpui: the host lists no thread {thread}");
            std::process::exit(1);
        };
        self.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx);
        let presented = Presented::default();
        self.thread_view.update(cx, |v, cx| v.watch_presented(thread, presented.clone(), cx));
        presented
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick(window, cx);
        let shown = self.sidebar_shown(window);
        let p = cx.palette().clone();
        let fills = material::fills(self.look, ui::theme::current(cx).id, &p);
        let viewport = window.viewport_size().width;
        let settings = self.settings_open();
        let panel_open = !settings && self.panel.read(cx).is_open();
        let sheet = viewport <= frame::SHEET_MAX_VIEWPORT;
        let inline = panel_open && !sheet;
        let sidebar_width = frame::clamp_sidebar(self.sidebar_width, viewport, inline);
        let row = viewport - sidebar_width * shown;
        let panel_width = frame::clamp_panel(self.panel.read(cx).width(), viewport, row);

        let mut context = KeyContext::new_with_defaults();
        context.add(keys::SHELL);
        if settings {
            context.add(keys::SETTINGS_OPEN);
        } else if matches!(self.route, Route::Thread { .. }) {
            context.add(keys::THREAD_OPEN);
        }

        let mounted = match &self.route {
            Route::Screen(_) => self.screen.as_ref(),
            Route::Thread { .. } => self.thread.as_ref(),
            Route::Empty => None,
        };
        let parts = mounted.map(|m| &m.parts);
        let failed = match &self.route {
            Route::Thread { workspace_id, .. } => self.sidebar.read(cx).workspace(workspace_id).and_then(dialogs::failed_start::Failed::of),
            _ => None,
        };
        let main: AnyView = match (mounted, &self.route) {
            (Some(m), _) => m.view.clone(),
            (None, Route::Screen(_)) => self.blank.clone(),
            (None, Route::Empty) if self.thread.is_some() => self.blank.clone(),
            (None, Route::Empty) if let Some(first_run) = &self.first_run => first_run.clone().into(),
            (None, _) => self.thread_view.clone().into(),
        };
        let sidebar_content: AnyView = match parts.and_then(|p| p.sidebar.clone()) {
            Some(view) => view,
            None if settings => self.blank.clone(),
            None => self.sidebar.clone().into(),
        };

        let trigger = |id: &'static str, open: bool, cx: &mut Context<Self>| {
            let this = cx.entity().downgrade();
            frame::sidebar_trigger(id, open, move |_, window, cx| {
                _ = this.update(cx, |this, cx| this.toggle_sidebar(&keys::ToggleSidebar, window, cx));
            })
        };
        let sidebar = (shown > 0.).then(|| {
            let column = v_flex()
                .id("sidebar-column")
                .debug_selector(|| "sidebar-column".into())
                .relative()
                .flex_none()
                .w(sidebar_width)
                .h_full()
                .border_r_1()
                .border_color(p.border)
                .bg(fills.sidebar)
                .when(fills.grain, |el| el.child(frame::grain()))
                .child(
                    frame::row("sidebar-frame-row")
                        .pl(frame::frame_inset())
                        .child(trigger("sidebar-trigger", true, cx))
                        .child(frame::lockup(fills.sidebar_ink.unwrap_or_else(|| frame::sidebar_quiet(cx)))),
                )
                .child(div().flex_1().min_h_0().overflow_hidden().child(sidebar_content.cached(StyleRefinement::default().size_full())))
                .into_any_element();
            div()
                .relative()
                .flex_none()
                .w(sidebar_width * shown)
                .h_full()
                .child(div().absolute().top_0().bottom_0().left(-(sidebar_width * (1. - shown))).w(sidebar_width).child(column))
        });
        let rail = (shown >= 1.).then(|| {
            div()
                .id("sidebar-rail")
                .debug_selector(|| "sidebar-rail".into())
                .absolute()
                .top_0()
                .bottom_0()
                .left(sidebar_width - frame::SIDEBAR_RAIL / 2.)
                .w(frame::SIDEBAR_RAIL)
                .cursor_col_resize()
                .group("sidebar-rail")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, _, _| this.grab = e.position.x - sidebar_width),
                )
                .on_drag(Side::Sidebar, |_, _, _, cx| cx.new(|_| Dragging))
                .child(div().mx_auto().w(px(2.)).h_full().group_hover("sidebar-rail", |s| s.bg(p.border)))
        });

        let crumbs = match (&self.route, parts.and_then(|p| p.crumbs.clone())) {
            (_, Some(view)) => view.into_any_element(),
            (Route::Thread { thread_id, .. }, None) => {
                let item = self.sidebar.read(cx).items().iter().find(|i| i.thread_id == *thread_id).cloned();
                let title = item.as_ref().map(|i| i.title.as_str()).filter(|t| !t.is_empty()).unwrap_or("New thread");
                frame::crumbs(item.as_ref().map(|i| i.project.as_str()), Some(title), cx)
            }
            (Route::Screen(_), None) => div().into_any_element(),
            (Route::Empty, None) => frame::crumbs(None, None, cx),
        };
        let actions = parts.and_then(|p| p.actions.clone());
        let controls = |cx: &mut Context<Self>| {
            let (on_drawer, on_panel) = (cx.entity().downgrade(), cx.entity().downgrade());
            frame::layout_controls(
                self.drawer_open,
                panel_open,
                move |_, window, cx| {
                    _ = on_drawer.update(cx, |this, cx| this.toggle_drawer(&keys::ToggleTerminal, window, cx));
                },
                move |_, window, cx| {
                    _ = on_panel.update(cx, |this, cx| this.toggle_panel(&keys::ToggleRightPanel, window, cx));
                },
                cx,
            )
        };
        let header = frame::row("header-row")
            .pl(frame::frame_inset() + (frame::HEADER_INSET - frame::frame_inset()) * shown)
            .pr(frame::HEADER_INSET)
            .when(!self.sidebar_open, |row| {
                let row = row.child(trigger("header-trigger", false, cx));
                if settings {
                    row
                } else {
                    row.child(
                        ui::Button::icon("new-thread", ui::Glyph::SquarePen)
                            .on_click(move |_, window, cx| window.dispatch_action(Box::new(keys::NewThread), cx)),
                    )
                }
            })
            .child(
                h_flex().flex_1().min_w_0().h_full().items_center().child(crumbs).child(
                    h_flex()
                        .id("header-actions")
                        .debug_selector(|| "header-actions".into())
                        .flex_none()
                        .ml_auto()
                        .mr(px(1.))
                        .h_full()
                        .items_center()
                        .gap_2()
                        .children(actions)
                        .when(!settings && !inline, |el| el.child(controls(cx))),
                ),
            );

        let center = v_flex()
            .id("center")
            .debug_selector(|| "center".into())
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(fills.centre)
            .when(fills.grain, |el| el.child(frame::grain()))
            .child(header)
            .child(div().flex_1().min_h_0().map(|el| match failed {
                Some(failed) => {
                    let me = cx.entity().downgrade();
                    el.child(dialogs::failed_start::FailedStart::new(failed, move |lead, window, cx| {
                        let route = Route::Thread { workspace_id: lead.workspace_id.clone(), thread_id: lead.thread_id.clone() };
                        _ = me.update(cx, |this, cx| this.go(route, window, cx));
                    }))
                }
                None => el.child(main.cached(StyleRefinement::default().size_full())),
            }))
            .when(self.glass, |el| el.child(h_flex().absolute().bottom_4().left_0().right_0().justify_center().child(composer_glass(cx))));

        let panel = panel_open.then(|| {
            let controls = if inline { controls(cx).into_any_element() } else { div().into_any_element() };
            let content =
                self.panel.update(cx, |panel, cx| panel.draw(controls, (fills.panel_card, fills.panel_line), cx).into_any_element());
            let column = div()
                .id("right-panel")
                .debug_selector(|| "right-panel".into())
                .relative()
                .flex_none()
                .w(if sheet { sheet_width(viewport) } else { panel_width })
                .h_full()
                .border_l_1()
                .border_color(fills.panel_line)
                .bg(fills.panel)
                .child(content);
            if sheet {
                let this = cx.entity().downgrade();
                // Over the whole window, as the web's sheet is; each layer blocks the pointer from what lies under it.
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(
                        div()
                            .id("sheet-backdrop")
                            .absolute()
                            .size_full()
                            .occlude()
                            .bg(p.background.opacity(0.6))
                            .backdrop_blur(px(4.))
                            .on_click(move |_, window, cx| {
                                _ = this.update(cx, |this, cx| this.toggle_panel(&keys::ToggleRightPanel, window, cx));
                            }),
                    )
                    .child(div().absolute().top_0().bottom_0().right_0().occlude().child(column.bg(p.popover).shadow_lg()))
                    .into_any_element()
            } else {
                column
                    .child(
                        div()
                            .id("panel-rail")
                            .debug_selector(|| "panel-rail".into())
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(-frame::PANEL_RAIL / 2.)
                            .w(frame::PANEL_RAIL)
                            .cursor_col_resize()
                            .group("panel-rail")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, e: &MouseDownEvent, _, _| this.grab = e.position.x - (viewport - panel_width)),
                            )
                            .on_drag(Side::Panel, |_, _, _, cx| cx.new(|_| Dragging))
                            .child(div().mx_auto().w(px(1.)).h_full().group_hover("panel-rail", |s| s.bg(p.border))),
                    )
                    .into_any_element()
            }
        });

        let (inline_panel, sheet_panel) = if sheet { (None, panel) } else { (panel, None) };
        h_flex()
            .id("shell")
            .key_context(context)
            .track_focus(&self.focus)
            .size_full()
            .when_some(fills.window, |el, ground| el.bg(ground))
            .text_color(p.foreground)
            .font_family(cx.theme().font_family.clone())
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_panel))
            .on_action(cx.listener(Self::toggle_preview))
            .on_action(cx.listener(Self::toggle_drawer))
            .on_action(cx.listener(Self::toggle_settings))
            .on_action(cx.listener(Self::close_settings))
            .on_action(cx.listener(Self::close_panel_tab))
            .on_action(cx.listener(Self::select_workspace))
            .on_action(cx.listener(|this, _: &keys::NextPanelTab, window, cx| this.panel.update(cx, |p, cx| p.step(1, window, cx))))
            .on_action(cx.listener(|this, _: &keys::PreviousPanelTab, window, cx| this.panel.update(cx, |p, cx| p.step(-1, window, cx))))
            .on_action(cx.listener(|this, _: &keys::NextWorkspace, window, cx| this.step_thread(this.workspaces(cx), 1, window, cx)))
            .on_action(cx.listener(|this, _: &keys::PreviousWorkspace, window, cx| this.step_thread(this.workspaces(cx), -1, window, cx)))
            .on_action(cx.listener(|this, _: &keys::NextThread, window, cx| this.step_thread(this.workspace_threads(cx), 1, window, cx)))
            .on_action(
                cx.listener(|this, _: &keys::PreviousThread, window, cx| this.step_thread(this.workspace_threads(cx), -1, window, cx)),
            )
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<Side>, window, cx| {
                let side = *e.drag(cx);
                let x = e.event.position.x - this.grab;
                let viewport = window.viewport_size().width;
                this.dragging = Some(side);
                match side {
                    // The width the person drags to, held within the sidebar's own bounds; the frame narrows it beside
                    // the panel as it draws, and gives it back once the panel goes.
                    Side::Sidebar => this.sidebar_width = x.max(frame::SIDEBAR_MIN).min(frame::SIDEBAR_MAX),
                    Side::Panel => {
                        let inline = viewport > frame::SHEET_MAX_VIEWPORT;
                        let sidebar = frame::clamp_sidebar(this.sidebar_width, viewport, inline);
                        let row = viewport - if this.sidebar_open { sidebar } else { px(0.) };
                        let width = frame::clamp_panel(viewport - x, viewport, row);
                        this.panel.update(cx, |p, cx| p.set_width(width, cx));
                    }
                }
                cx.notify();
            }))
            // A drag ends on the release wherever it falls, inside the window or past its edge.
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.end_drag()))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, _| this.end_drag()))
            .when(sheet && panel_open, |el| {
                el.on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                    if e.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.toggle_panel(&keys::ToggleRightPanel, window, cx);
                    }
                }))
            })
            .children(sidebar)
            .child(div().relative().flex().flex_1().min_w_0().h_full().child(center).children(inline_panel))
            .children(rail)
            .children(sheet_panel)
            .children(dialogs::layer::draw(window, cx))
    }
}

/// `RIGHT_PANEL_SHEET_CLASS_NAME`: 42% of the window up to 448 px and down to 320, or 88% up to 384 under 760 px.
fn sheet_width(viewport: Pixels) -> Pixels {
    if viewport <= px(760.) {
        (viewport * 0.88).min(px(384.))
    } else {
        (viewport * 0.42).min(px(448.)).max(px(320.))
    }
}

/// An empty column, where a screen that has not landed yet will mount.
struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}
