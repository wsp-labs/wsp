// SPDX-License-Identifier: AGPL-3.0-only
//! The window's content: the frame of `screens::app_shell` around the sidebar, the open thread or screen and the right
//! panel, the host connection behind them, and bench mode's run.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    div, prelude::*, px, AnyView, App, Context, DragMoveEvent, Empty, Entity, FocusHandle, KeyBinding, KeyContext, KeyDownEvent,
    MouseButton, MouseDownEvent, Pixels, SharedString, StyleRefinement, Subscription, Task, Window,
};
use tokio::runtime::Runtime;
use wsp_host_client::ops::settings::PreferencesReply;
use wsp_host_client::protocol::{PreferencesGetRequest, WorkspaceOutPhase, WorkspacesListRequest};
use wsp_host_client::{Env, Failure, HostClient};

use crate::bench;
use crate::glass::composer_glass;
use crate::screens::app_shell::frame;
use crate::screens::app_shell::keys::{self, Bound};
use crate::screens::app_shell::material::{self, Look};
use crate::screens::app_shell::panel::{At, RightPanel};
use crate::screens::app_shell::vibrancy::Vibrancy;
use crate::screens::composer::screen::ComposerScreen;
use crate::screens::palette;
use crate::screens::thread_view::ThreadScreen;
use crate::screens::{self, app_shell, dialogs, Entry, Host, Mounted, Opened, Pane, Place, Route, Screen, THREAD};
use crate::sidebar::{Asked, Link, Selected, Sidebar, ThreadItem};
use crate::thread_view::ThreadView;
use crate::ui::{self, ActivePalette};

mod actions;
mod bench_hold;
#[cfg(target_os = "macos")]
mod bench_post;
mod bench_run;
mod bench_session;
mod preferences;

use bench_run::{BenchRun, Phase, BENCH_LIMIT};

/// `transition-[left,right,width] duration-200 ease-linear` on the sidebar.
const SIDEBAR_SLIDE: Duration = Duration::from_millis(200);

/// The screen `mod+,` opens; the shell knows it by name because it hides the right panel and closes on Escape.
pub const SETTINGS: &str = "settings";

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

/// A panel's tabs and its active pane.
type PanelTabs = (Vec<(Pane, SharedString)>, Option<Pane>);

pub struct Shell {
    sidebar: Entity<Sidebar>,
    thread_view: Entity<ThreadView>,
    panel: Entity<RightPanel>,
    host: Option<Arc<HostClient>>,
    runtime: Arc<Runtime>,
    route: Route,
    /// The composer route on screen is a project's home (`openProjectHome`), not its workspace's New thread here.
    home: bool,
    /// Where closing Settings goes back to.
    behind: Route,
    /// `projectHome`: the project whose home was last opened, where the window lands with nothing open, until a
    /// workspace is picked.
    home_project: Option<String>,
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
    /// Under 768 px the sidebar is a sheet over the window, shut until asked for (`openMobile` in `sidebar.tsx`).
    sheet_open: bool,
    /// The sheet's own focus, which a press inside it takes (the popup's `tabIndex={-1}`), and its toggle's, which
    /// takes the focus as the sheet opens: Base UI's dialog focuses the popup's first tabbable, and the sheet's first
    /// is its toggle (`SidebarChromeHeader`'s `SidebarTrigger`).
    sheet_focus: FocusHandle,
    sheet_toggle: FocusHandle,
    sheet_focus_due: bool,
    /// The rail a drag holds, until its release, and how far from its centre line the press took hold of it, so a rail
    /// held off its centre does not jump to the pointer.
    dragging: Option<Side>,
    grab: Pixels,
    /// Whether the route's terminal drawer is open, as the terminal keeps it per thread; read at each draw.
    drawer_open: bool,
    /// The panel's tabs as its sheet began fading shut; it lets its panes go once the fade ends unless one opened since.
    panel_fading: Option<PanelTabs>,
    /// The chord rules bound now, so a change of the person's overrides can unbind exactly them.
    rules: Vec<Bound>,
    /// The theme and look this window's material was last set for.
    theme: Option<&'static str>,
    look: Look,
    vibrancy: Vibrancy,
    focus: FocusHandle,
    blank: AnyView,
    bench: Option<BenchRun>,
    glass: bool,
    /// The centre while the window shows nothing, which is first run while the host holds no project.
    first_run: Option<Entity<dialogs::first_run::FirstRun>>,
    _connect: Task<()>,
    _events: Option<Task<()>>,
    _prefs: Subscription,
    _selected: Subscription,
    /// The sidebar's pushes, which carry each workspace's pull request and a New thread page's first thread.
    _followed: Subscription,
    _asked: Subscription,
    _focus_lost: Subscription,
    _appearance: Subscription,
    _activation: Subscription,
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
        let panel = cx.new(|cx| RightPanel::new(crate::desktop::hosts::target(cx), cx));
        let selected = cx.subscribe_in(&sidebar, window, |this, _, Selected(item), window, cx| {
            let route = Route::Thread { workspace_id: item.workspace_id.clone(), thread_id: item.thread_id.clone() };
            this.go(route, window, cx);
        });
        screens::terminal::read_workspaces_from(&sidebar, cx);
        let followed = cx.observe_in(&sidebar, window, |this, _, window, cx| {
            this.follow_panel(cx);
            this.land(window, cx);
        });
        let asked = cx.subscribe_in(&sidebar, window, |this, _, asked: &Asked, window, cx| match asked {
            Asked::Settings => this.go(Route::Screen(SETTINGS.into()), window, cx),
            Asked::Usage => this.go(Route::Screen(format!("{SETTINGS}:usage").into()), window, cx),
            Asked::Activity => this.go(Route::Screen(format!("{SETTINGS}:activity").into()), window, cx),
            Asked::NewThread => this.new_thread(window, cx),
            Asked::NewThreadIn(workspace) => this.go(Route::Screen(format!("{}:{workspace}", ComposerScreen::NAME).into()), window, cx),
            Asked::Search => palette::palette::open(window, cx),
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
            Asked::OpenTerminal(workspace) => screens::terminal::open_terminal(this, workspace, window, cx),
            // `openSurface(workspace, "preview")`: the panel on the browser's first tab, else its placeholder, never shut.
            Asked::OpenBrowser(workspace) => {
                let here = this.workspace_on_screen(cx).as_deref() == Some(workspace.as_str());
                let shown = this.panel.read(cx).is_open() && this.panel.read(cx).active() == Some(Pane::Browser);
                if here && shown {
                    this.panel_fading = None;
                } else if here {
                    this.panel.update(cx, |p, cx| p.toggle_pane(Pane::Browser, window, cx));
                }
            }
            Asked::ProjectSettings(id) => this.go(Route::Screen(format!("{SETTINGS}:project:{id}").into()), window, cx),
            Asked::ComputerSettings(id) => this.go(Route::Screen(format!("{SETTINGS}:computer:{id}").into()), window, cx),
        });
        let page = crate::sidebar::host_page(&state);
        screens::terminal::boot::read_from(page.clone(), cx);
        let (pair_state, pair_env) = (state.clone(), env.clone());
        let (bought, alias) = (dialogs::pair::bought(cx), crate::desktop::hosts::picked(cx));
        let connecting = runtime.spawn(async move {
            let host = HostClient::connect_on(&state, &env, alias.as_deref(), bought).await?;
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
                        this.running.insert(workspace.id.clone(), matches!(workspace.phase, WorkspaceOutPhase::Running));
                    }
                    let host = Arc::new(host);
                    this.host = Some(host.clone());
                    let runtime = this.runtime.clone();
                    this.panel.update(cx, |p, cx| p.set_host(Host::new(host.clone(), runtime.clone()), window, cx));
                    palette::attach(Host::new(host.clone(), runtime.clone()), &this.sidebar, window, cx);
                    let (center, first) = (this.thread_view.clone().into(), Host::new(host.clone(), runtime.clone()));
                    this.first_run = Some(cx.new(|cx| dialogs::first_run::FirstRun::new(first, page.clone(), center, cx)));
                    crate::desktop::app_procs::offer_once(host.clone(), &runtime, cx);
                    this.sidebar.update(cx, |s, cx| s.attach(Link { host: host.clone(), runtime, page }, cx));
                    let prefs = crate::prefs::store(cx);
                    if let Some(preferences) = &preferences {
                        prefs.update(cx, |store, cx| store.landed(preferences, cx));
                    }
                    // Another window may have landed the same record, which moves nothing to hear.
                    if let Some(shown) = prefs.read(cx).shown().cloned() {
                        this.apply(&shown, window, cx);
                    }
                    this._events = Some(this.follow_preferences(host, window, cx));
                    if let (Route::Screen(route), None) = (this.route.clone(), &this.screen) {
                        this.screen = this.mount(&route, window, cx);
                    }
                    this.sidebar.update(cx, |s, cx| s.set_items(threads.iter().filter_map(ThreadItem::of).collect(), cx));
                    if let Some(thread) = &open {
                        let Some(item) = this.sidebar.read(cx).items().iter().find(|i| i.thread_id == *thread).cloned() else {
                            eprintln!("desktop-gpui: the host lists no thread {thread}");
                            std::process::exit(1);
                        };
                        this.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx);
                    }
                    if let Some(name) = &screen {
                        this.go(Route::Screen(name.clone().into()), window, cx);
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
        if let Some(args) = &bench {
            let limit = if args.session.is_empty() {
                BENCH_LIMIT
            } else {
                BENCH_LIMIT + bench_session::RESTING + bench_session::WALKING + bench_session::PER_THREAD * args.session.len() as u32
            };
            // A window that stops drawing (covered, minimized) never ticks, so the limit runs on a timer of its own.
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(limit).await;
                let stuck = this.read_with(cx, |this, cx| this.bench_stuck(cx)).unwrap_or_else(|_| "its window gone".to_owned());
                eprintln!("desktop-gpui: the bench did not finish in {} s, {stuck}", limit.as_secs());
                std::process::exit(2);
            })
            .detach();
        }
        // A measured window stands in front; a bench stops on one covered or not key, which draws what no one sees.
        if bench.is_some() || std::env::var_os("DESKTOP_GPUI_FRAMES").is_some_and(|path| !path.is_empty()) {
            crate::in_front::watch(bench.is_some(), window, cx);
        }
        let bench = bench.map(|args| BenchRun {
            args,
            began: Instant::now(),
            phase: Phase::Opening,
            rows: 0,
            frame_ms: Vec::new(),
            switch: Vec::new(),
            timed: Vec::new(),
        });
        let mac = cfg!(target_os = "macos");
        let rules = keys::bound(mac, &BTreeMap::new());
        if !std::mem::replace(&mut cx.default_global::<KeysBound>().0, true) {
            cx.bind_keys(keys::bindings_of(mac, &rules));
            cx.bind_keys([KeyBinding::new("escape", keys::EscapePage, Some(&format!("{} && {}", keys::SHELL, keys::SETTINGS_OPEN)))]);
            cx.bind_keys(screens::terminal::view::key_bindings());
        }
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        // Focus on a view that is no longer drawn reaches nothing, and keys then start at the window's root, above the
        // shell's bindings.
        let focus_lost = cx.on_focus_lost(window, |this, window, cx| {
            if !dialogs::layer::refocus(window, cx) {
                this.focus.focus(window, cx);
            }
        });
        let appearance = cx.observe_window_appearance(window, |this, window, cx| this.pick_theme(window, cx));
        // Reduce transparency has no window callback and is switched in System Settings, so it is read again on return.
        let activation = cx.observe_window_activation(window, |this, window, cx| this.pick_theme(window, cx));
        let store = crate::prefs::store(cx);
        let prefs = cx.observe_in(&store, window, |this, prefs, window, cx| {
            if let Some(p) = prefs.read(cx).shown().cloned() {
                this.apply(&p, window, cx);
            }
        });
        let mut shell = Shell {
            sidebar,
            thread_view,
            panel,
            host: None,
            runtime,
            route: Route::Empty,
            home: false,
            behind: Route::Empty,
            home_project: None,
            screen: None,
            thread: None,
            running: HashMap::new(),
            sidebar_open: true,
            sidebar_moved: None,
            sidebar_width: frame::SIDEBAR_DEFAULT,
            sheet_open: false,
            sheet_focus: cx.focus_handle(),
            sheet_toggle: cx.focus_handle(),
            sheet_focus_due: false,
            dragging: None,
            grab: px(0.),
            drawer_open: false,
            panel_fading: None,
            rules,
            theme: None,
            look: Look { mac: false, glass: false },
            vibrancy: Vibrancy::default(),
            focus,
            blank: cx.new(|_| Blank).into(),
            bench,
            glass,
            first_run: None,
            _connect: connect,
            _events: None,
            _prefs: prefs,
            _selected: selected,
            _followed: followed,
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

    /// The chord rules bound now, in the order they were bound.
    pub fn rules(&self) -> &[Bound] {
        &self.rules
    }

    pub fn route(&self) -> &Route {
        &self.route
    }

    /// The page mounted in the main column, as a test reaches into it.
    pub fn screen_view(&self) -> Option<AnyView> {
        self.screen.as_ref().map(|m| m.view.clone())
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
    /// `Place::Main` says, and it is blank until its screen mounts. The New thread page is not one: it keeps its
    /// workspace's sidebar, panel and drawer, as today's does.
    fn settings_open(&self) -> bool {
        matches!(&self.route, Route::Screen(route) if app_shell::split(route).0 != ComposerScreen::NAME)
    }

    /// Where the panel stands for the route, the drawer's own key (`stateKeyOf`): the computer's, a thread's, or the
    /// New thread page's of its workspace; none under a page.
    fn panel_at(&self, cx: &App) -> Option<At> {
        Some(match screens::terminal::drawer_key(&self.place(), cx)? {
            screens::terminal::hub::Key::Thread { workspace_id, thread_id } => {
                let running = self.running.get(&workspace_id).copied().unwrap_or(false);
                // `isPullRequestNamed`: the workspace status's `pr`, when it names a number rather than a refusal.
                let pr = self.sidebar.read(cx).workspace(&workspace_id).1.and_then(|s| s.pointer("/pr/number")).is_some();
                At::Thread { workspace_id, thread_id, running, pr }
            }
            _ => At::Here,
        })
    }

    /// The name of `workspace`'s project, as the host lists the workspace, else as its threads name it.
    fn project_of(&self, workspace: &str, cx: &App) -> Option<String> {
        let sidebar = self.sidebar.read(cx);
        let listed = sidebar.workspace(workspace).0.and_then(|w| w.pointer("/project/name")).and_then(|n| n.as_str()).map(str::to_owned);
        listed.or_else(|| sidebar.items().iter().find(|i| i.workspace_id == workspace).map(|i| i.project.clone()))
    }

    /// The route the panel and the drawer stand for: the computer's own (no route) on a New thread page opened as a
    /// project's home, else the route.
    fn place(&self) -> Route {
        if self.home {
            Route::Empty
        } else {
            self.route.clone()
        }
    }

    /// The route's panel moved while the route stood: its workspace runs, its pull request landed, or the New thread
    /// page's first send made the thread its panel goes on as.
    fn follow_panel(&mut self, cx: &mut Context<Self>) {
        if let Some(at) = self.panel_at(cx) {
            self.panel.update(cx, |p, cx| p.follow(at, cx));
        }
    }

    /// Moves the window to `route`; [`app_shell::navigate`] is the way in for everything outside the shell. A screen
    /// that lives in the right panel opens there, on its tab, and leaves the route where it was.
    pub fn go(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.go_on(route, false, window, cx);
    }

    /// `openProjectHome`: the composer on `workspace` as New thread opens it, the sidebar's row, mod+t and the
    /// palette's project, whose panel and drawer are this computer's own (`selectedId: null`), where New thread here
    /// keeps the workspace's.
    pub fn open_home(&mut self, workspace: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.go_on(Route::Screen(format!("{}:{workspace}", ComposerScreen::NAME).into()), true, window, cx);
    }

    fn go_on(&mut self, route: Route, home: bool, window: &mut Window, cx: &mut Context<Self>) {
        if route == self.route && home == self.home {
            return;
        }
        if let Route::Screen(route) = &route {
            let (name, arg) = app_shell::split(route);
            if let Some(Entry { place: Place::Panel(pane), .. }) = screens::screen(name) {
                self.panel_fading = None;
                let tab = arg.map(|arg| SharedString::from(arg.to_owned()));
                self.panel.update(cx, |p, cx| p.add(pane, tab, window, cx));
                return;
            }
        }
        self.end_panel_fade(window, cx);
        match &route {
            Route::Empty => {
                self.screen = None;
                self.sidebar.update(cx, |s, cx| s.unmark(cx));
                if let Some(thread) = &self.thread {
                    (thread.show.clone())(Opened::default(), window, cx);
                }
            }
            Route::Thread { workspace_id, thread_id } => {
                self.screen = None;
                screens::warm::switched(workspace_id, thread_id, window, cx);
                self.sidebar.update(cx, |s, cx| s.mark(thread_id, cx));
                if let Some(item) = self.sidebar.read(cx).items().iter().find(|i| i.thread_id == *thread_id).cloned() {
                    crate::screens::thread_view::header::seed(thread_id, &item.project, &item.title, cx);
                }
                let opened = Opened { workspace_id: Some(workspace_id.clone()), thread_id: Some(thread_id.clone()), ..Opened::default() };
                match (&self.thread, self.host.clone(), screens::screen(THREAD)) {
                    (Some(thread), _, _) => (thread.show.clone())(opened, window, cx),
                    (None, Some(host), Some(entry)) => {
                        self.thread = Some((entry.mount)(&Host::new(host, self.runtime.clone()), opened, window, cx));
                    }
                    _ => {}
                }
            }
            Route::Screen(name) => {
                if self.home {
                    self.behind = Route::Empty;
                } else if !matches!(self.route, Route::Screen(_)) {
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
                if app_shell::split(name).0 == ComposerScreen::NAME {
                    self.sidebar.update(cx, |s, cx| s.unmark(cx));
                }
                if let (screens::thread_view::cards::SUBAGENT, Some((lead, call))) =
                    (app_shell::split(name).0, app_shell::split(name).1.and_then(screens::thread_view::subagent::parse))
                {
                    self.sidebar.update(cx, |s, cx| s.mark_subagent(&lead.thread, &call, cx));
                }
            }
        }
        match &route {
            Route::Screen(r) if home => {
                let workspace = app_shell::split(r).1.unwrap_or_default();
                self.home_project =
                    self.sidebar.read(cx).workspace(workspace).0.and_then(|w| w.pointer("/project/id")?.as_str().map(str::to_owned));
            }
            Route::Thread { .. } => self.home_project = None,
            Route::Screen(r) if app_shell::split(r).0 == ComposerScreen::NAME => self.home_project = None,
            _ => {}
        }
        self.route = route;
        self.home = home;
        if let Some(page) = self.screen.as_ref().and_then(|m| m.view.clone().downcast::<ComposerScreen>().ok()) {
            page.update(cx, |page, cx| page.set_home(home, cx));
        }
        if let Some(at) = self.panel_at(cx) {
            self.panel.update(cx, |p, cx| p.show_for(at, window, cx));
        }
        let hidden = self.settings_open();
        self.panel.update(cx, |p, cx| p.set_hidden(hidden, window, cx));
        cx.notify();
        self.land(window, cx);
    }

    /// `WorkspaceCenter`: with nothing open the centre is a project's New thread page, never one that asks to pick.
    fn land(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.route != Route::Empty {
            return;
        }
        let sidebar = self.sidebar.read(cx);
        let home = self.home_project.as_deref().and_then(|p| sidebar.home_of(p)).or_else(|| sidebar.home_workspace());
        if let Some(workspace) = home {
            self.open_home(&workspace, window, cx);
        }
    }

    /// `openNewThread`: the page of projects where New thread asks for one, else the composer on the task on screen.
    fn new_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if palette::palette::new_thread(window, cx) {
            return;
        }
        if let Some(workspace) = self.workspace_on_screen(cx) {
            self.open_home(&workspace, window, cx);
        }
    }

    /// The workspace of the thread on screen or left behind a page, else of the newest thread the sidebar lists.
    fn workspace_on_screen(&self, cx: &App) -> Option<String> {
        match (&self.route, &self.behind) {
            (Route::Thread { workspace_id, .. }, _) | (_, Route::Thread { workspace_id, .. }) => Some(workspace_id.clone()),
            _ => self.sidebar.read(cx).items().first().map(|i| i.workspace_id.clone()),
        }
    }

    /// The web's `selectedId` and `selectedThreadId`, and whether a page stands over them (`settingsOpen`): the thread
    /// on screen, the task a new thread's composer is on, or the thread a page was opened over.
    pub fn selected(&self) -> Option<(String, Option<String>, bool)> {
        match (&self.route, &self.behind) {
            (Route::Thread { workspace_id, thread_id }, _) => Some((workspace_id.clone(), Some(thread_id.clone()), false)),
            (Route::Screen(_), _) if self.home => None,
            (Route::Screen(route), _) if app_shell::split(route).0 == ComposerScreen::NAME => {
                app_shell::split(route).1.map(|workspace| (workspace.to_owned(), None, false))
            }
            (Route::Screen(_), Route::Thread { workspace_id, thread_id }) => Some((workspace_id.clone(), Some(thread_id.clone()), true)),
            _ => None,
        }
    }

    /// The task mod+o opens in an editor: the one selected, never from under a page.
    pub fn editor_workspace(&self) -> Option<String> {
        self.selected().filter(|(_, _, page)| !page).map(|(workspace, _, _)| workspace)
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
    pub fn workspace_runs(&mut self, workspace_id: &str, running: bool, _: &mut Window, cx: &mut Context<Self>) {
        self.running.insert(workspace_id.to_owned(), running);
        self.follow_panel(cx);
    }

    /// Whether the sidebar is drawn now: its sheet's state under 768 px, else its column's (`useSidebarVisibility`).
    pub fn sidebar_visible(&self, window: &Window) -> bool {
        if ui::narrow::width(window).md {
            self.sheet_open
        } else {
            self.sidebar_open
        }
    }

    /// Whether the focus is on the sidebar's sheet's own toggle.
    pub fn sheet_toggle_focused(&self, window: &Window) -> bool {
        self.sheet_toggle.is_focused(window)
    }

    /// The thread screen the frame keeps across thread switches, once a thread has been opened.
    pub fn thread_screen(&self) -> Option<Entity<ThreadScreen>> {
        self.thread.as_ref().and_then(|m| m.view.clone().downcast::<ThreadScreen>().ok())
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("Shell");
        self.tick(window, cx);
        let narrow = ui::narrow::width(window).md;
        let shown = if narrow { 0. } else { self.sidebar_shown(window) };
        let sheet_shown = narrow && self.sheet_open;
        let sheet_in = if narrow { frame::sheet_in("sidebar-sheet", sheet_shown, window, cx) } else { None };
        let p = cx.palette().clone();
        let fills = material::fills(self.look, ui::theme::current(cx).id, &p);
        let viewport = window.viewport_size().width;
        let settings = self.settings_open();
        // `settingsOpen` alone swaps the sidebar and drops the New thread glyph (AppShell.tsx).
        let settings_page = matches!(&self.route, Route::Screen(route) if app_shell::split(route).0 == SETTINGS);
        let sheet = viewport <= frame::SHEET_MAX_VIEWPORT;
        let tabs_now = |p: &RightPanel| (p.tabs(), p.active());
        if self.panel_fading.as_ref().is_some_and(|at_fade| tabs_now(self.panel.read(cx)) != *at_fade) {
            self.panel_fading = None;
        }
        let panel_in = (sheet && !settings && self.panel.read(cx).is_open())
            .then(|| frame::sheet_in("panel-sheet", self.panel_fading.is_none(), window, cx))
            .flatten();
        if self.panel_fading.is_some() && panel_in.is_none() {
            self.end_panel_fade(window, cx);
        }
        let panel_open = !settings && self.panel.read(cx).is_open();
        let fading = self.panel_fading.is_some();
        let inline = panel_open && !sheet;
        let sidebar_width = frame::clamp_sidebar(self.sidebar_width, viewport, inline);
        let row = viewport - sidebar_width * shown;
        let panel_width = frame::clamp_panel(self.panel.read(cx).width(), viewport, row);

        let host = self.host.clone().map(|host| Host::new(host, self.runtime.clone()));
        let place = self.place();
        let drawer = screens::terminal::drawer(host, &place, self.look.glass, window, cx);
        self.drawer_open = screens::terminal::drawer_open(&place, cx);

        let mut context = KeyContext::new_with_defaults();
        context.add(keys::SHELL);
        // Escape leaves Settings alone, "the one centre view with no row of its own to pick" (KeybindingDispatcher.tsx).
        if settings_page {
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
            Route::Thread { workspace_id, .. } => {
                self.sidebar.read(cx).workspace(workspace_id).0.and_then(dialogs::failed_start::Failed::of)
            }
            _ => None,
        };
        // A mounted thread screen docks the real composer, whose own glass the stand-in would blur twice.
        let stand_in = self.glass && !(matches!(self.route, Route::Thread { .. }) && mounted.is_some());
        let main: AnyView = match (mounted, &self.route) {
            (Some(m), _) => m.view.clone(),
            (None, Route::Screen(_)) => self.blank.clone(),
            (None, Route::Empty) if self.thread.is_some() => self.blank.clone(),
            (None, Route::Empty) if let Some(first_run) = &self.first_run => first_run.clone().into(),
            (None, _) => self.thread_view.clone().into(),
        };
        let sidebar_content: AnyView = match parts.and_then(|p| p.sidebar.clone()) {
            Some(view) => view,
            None if settings_page => self.blank.clone(),
            None => self.sidebar.clone().into(),
        };

        let trigger = |id: &'static str, open: bool, cx: &mut Context<Self>| {
            let this = cx.entity().downgrade();
            frame::sidebar_trigger(id, open, move |_, window, cx| {
                _ = this.update(cx, |this, cx| this.toggle_sidebar(&keys::ToggleSidebar, window, cx));
            })
        };
        // Today's sheet sets its width to `calc(100vw - var(--spacing(3)))`, which CSS drops, so it is as wide as it holds.
        let in_sheet = sheet_in.is_some();
        let column = (shown > 0. || in_sheet).then(|| {
            let content = if in_sheet {
                sidebar_content.clone().into_any_element()
            } else {
                sidebar_content.cached(StyleRefinement::default().size_full()).into_any_element()
            };
            v_flex()
                .id("sidebar-column")
                .debug_selector(|| "sidebar-column".into())
                .relative()
                .flex_none()
                .map(|el| if in_sheet { el.max_w(viewport) } else { el.w(sidebar_width) })
                .h_full()
                .border_r_1()
                .border_color(p.border)
                .bg(fills.sidebar)
                .when(fills.grain, |el| el.child(frame::grain()))
                .child(
                    frame::row("sidebar-frame-row")
                        .pl(frame::frame_inset())
                        .child(trigger("sidebar-trigger", true, cx).track_focus(&self.sheet_toggle))
                        .child(frame::lockup(fills.sidebar_ink.unwrap_or_else(|| frame::sidebar_quiet(cx)))),
                )
                .child(div().flex_1().min_h_0().overflow_hidden().child(content))
                .into_any_element()
        });
        let (sidebar, sidebar_sheet) =
            match column {
                Some(column) if let Some(t) = sheet_in => {
                    if sheet_shown && std::mem::take(&mut self.sheet_focus_due) {
                        self.sheet_toggle.focus(window, cx);
                    }
                    let this = cx.entity().downgrade();
                    // `SheetPopup` over `SheetBackdrop`, each blocking the pointer while open and neither on its way out.
                    let sheet = div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .child(
                            div()
                                .id("sidebar-sheet-backdrop")
                                .debug_selector(|| "sidebar-sheet-backdrop".into())
                                .absolute()
                                .size_full()
                                .opacity(t)
                                .bg(p.background.opacity(0.6))
                                .backdrop_blur(px(4.))
                                .when(sheet_shown, |el| {
                                    el.occlude().on_click(move |_, window, cx| {
                                        _ = this.update(cx, |this, cx| this.close_sheet(window, cx));
                                    })
                                }),
                        )
                        .child(
                            div()
                                .id("sidebar-sheet")
                                .debug_selector(|| "sidebar-sheet".into())
                                .track_focus(&self.sheet_focus)
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(-frame::SHEET_SLIDE * (1. - t))
                                .opacity(t)
                                .when(sheet_shown, |el| el.occlude())
                                .shadow_lg()
                                .child(column),
                        );
                    (None, Some(sheet))
                }
                Some(column) => {
                    (
                        Some(div().relative().flex_none().w(sidebar_width * shown).h_full().child(
                            div().absolute().top_0().bottom_0().left(-(sidebar_width * (1. - shown))).w(sidebar_width).child(column),
                        )),
                        None,
                    )
                }
                None => (None, None),
            };
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
                frame::crumbs(item.as_ref().map(|i| i.project.as_str()), Some(title), window, cx)
            }
            (Route::Screen(route), None) if app_shell::split(route).0 == ComposerScreen::NAME => {
                let project = app_shell::split(route).1.and_then(|workspace| self.project_of(workspace, cx));
                frame::crumbs(project.as_deref(), Some(screens::thread_view::header::NEW_THREAD), window, cx)
            }
            (Route::Screen(_), None) => div().into_any_element(),
            (Route::Empty, None) => frame::crumbs(None, None, window, cx),
        };
        let actions = parts.and_then(|p| p.actions.clone());
        let backdrop = parts.and_then(|p| p.backdrop.clone());
        let controls = |cx: &mut Context<Self>| {
            let (on_drawer, on_panel) = (cx.entity().downgrade(), cx.entity().downgrade());
            frame::layout_controls(
                self.drawer_open,
                panel_open && !fading,
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
            .when(!self.sidebar_visible(window), |row| {
                let row = row.child(trigger("header-trigger", false, cx));
                if settings_page {
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
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .children(backdrop.map(|view| div().absolute().inset_0().child(view.cached(StyleRefinement::default().size_full()))))
                    .map(|el| match failed {
                        Some(failed) => {
                            let me = cx.entity().downgrade();
                            el.child(dialogs::failed_start::FailedStart::new(failed, move |lead, window, cx| {
                                let route = Route::Thread { workspace_id: lead.workspace_id.clone(), thread_id: lead.thread_id.clone() };
                                _ = me.update(cx, |this, cx| this.go(route, window, cx));
                            }))
                        }
                        None if settings_page => {
                            el.child(ui::theme::scoped(fills.panel_scope(&p), main.cached(StyleRefinement::default().size_full())))
                        }
                        None => el.child(main.cached(StyleRefinement::default().size_full())),
                    })
                    .when(stand_in, |el| {
                        el.child(
                            h_flex()
                                .debug_selector(|| "glass-stand-in".into())
                                .absolute()
                                .bottom_4()
                                .left_0()
                                .right_0()
                                .justify_center()
                                .child(composer_glass(cx)),
                        )
                    }),
            )
            .children(drawer);

        let panel = panel_open.then(|| {
            let controls = if inline { controls(cx).into_any_element() } else { div().into_any_element() };
            let scope = if sheet { p.clone() } else { fills.panel_scope(&p) };
            let content =
                ui::theme::within(scope.clone(), cx, |cx| self.panel.update(cx, |panel, cx| panel.draw(controls, cx).into_any_element()));
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
                .child(ui::theme::scoped(scope, content));
            if sheet {
                // Comes in and goes out as the sidebar's sheet does, holding its panes until it has gone.
                let t = panel_in.unwrap_or(1.);
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
                            .opacity(t)
                            .bg(p.background.opacity(0.6))
                            .backdrop_blur(px(4.))
                            .when(!fading, |el| {
                                el.occlude().on_click(move |_, window, cx| {
                                    _ = this.update(cx, |this, cx| this.toggle_panel(&keys::ToggleRightPanel, window, cx));
                                })
                            }),
                    )
                    .child(
                        div()
                            .debug_selector(|| "panel-sheet".into())
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(-frame::SHEET_SLIDE * (1. - t))
                            .opacity(t)
                            .when(!fading, |el| el.occlude())
                            .child(column.bg(p.background).shadow_lg()),
                    )
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
            .on_action(cx.listener(Self::escape_page))
            .on_action(cx.listener(Self::close_panel_tab))
            .on_action(cx.listener(Self::select_workspace))
            .on_action(cx.listener(|this, _: &keys::NextPanelTab, window, cx| this.panel.update(cx, |p, cx| p.step(1, window, cx))))
            .on_action(cx.listener(|this, _: &keys::PreviousPanelTab, window, cx| this.panel.update(cx, |p, cx| p.step(-1, window, cx))))
            .on_action(cx.listener(|_, _: &keys::ToggleCommandPalette, window, cx| palette::palette::toggle(window, cx)))
            .on_action(cx.listener(|this, _: &keys::SettleThread, _, cx| {
                if let Some((workspace, thread, _)) = this.selected() {
                    this.sidebar.update(cx, |s, cx| s.settle_open(&workspace, thread.as_deref(), cx));
                }
            }))
            .on_action(cx.listener(|this, _: &keys::QuickOpen, window, cx| {
                let host = this.host.clone().map(|h| Host::new(h, this.runtime.clone()));
                let mode = screens::pr_pane::files::finder::Mode::Files;
                screens::pr_pane::files::finder::chord(mode, host, this.editor_workspace(), window, cx)
            }))
            .on_action(cx.listener(|this, _: &keys::SearchFiles, window, cx| {
                let host = this.host.clone().map(|h| Host::new(h, this.runtime.clone()));
                let mode = screens::pr_pane::files::finder::Mode::Text;
                screens::pr_pane::files::finder::chord(mode, host, this.editor_workspace(), window, cx)
            }))
            .on_action(cx.listener(|this, _: &keys::OpenInEditor, window, cx| {
                if let Some(workspace) = this.editor_workspace() {
                    crate::screens::notices::open_in_editor(workspace, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &keys::NewThread, window, cx| this.new_thread(window, cx)))
            .on_action(cx.listener(|this, _: &keys::NextWorkspace, _, cx| this.step_level(1, cx)))
            .on_action(cx.listener(|this, _: &keys::PreviousWorkspace, _, cx| this.step_level(-1, cx)))
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
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.end_drag(cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| this.end_drag(cx)))
            .when((sheet && panel_open && !fading) || sheet_shown, |el| {
                el.on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                    if e.keystroke.key == "escape" {
                        cx.stop_propagation();
                        if this.close_sheet(window, cx) {
                        } else if sheet && panel_open && !fading {
                            this.toggle_panel(&keys::ToggleRightPanel, window, cx);
                        }
                    }
                }))
            })
            .children(sidebar)
            .child(div().relative().flex().flex_1().min_w_0().h_full().child(center).children(inline_panel))
            .children(rail)
            .children(sheet_panel)
            .children(sidebar_sheet)
            .children(dialogs::layer::draw(window, cx))
            .children(bench_run::listener())
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
