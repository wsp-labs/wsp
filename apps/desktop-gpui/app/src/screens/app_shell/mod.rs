// SPDX-License-Identifier: AGPL-3.0-only
//! The frame every screen lives in, and the contract a screen signs to be mounted in it: a [`Screen`] is built from a
//! [`Host`] and what it is [`Opened`] on, says where it goes with a [`Place`], and may hand the shell a sidebar column,
//! crumbs, header actions or a tab title of its own.

pub mod assets;
pub mod frame;
pub mod keys;
pub mod material;
pub mod menu;
pub mod panel;
pub mod title;
pub mod vibrancy;

use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{AnyView, AnyWindowHandle, App, AppContext, Context, Entity, Global, Render, SharedString, WeakEntity, Window};
use tokio::runtime::Runtime;
use wsp_host_client::{Failure, HostClient};

use crate::shell::Shell;

/// The connected host, as every screen reads and writes it. Cheap to clone; the shell builds it once the host answers.
#[derive(Clone)]
pub struct Host {
    client: Arc<HostClient>,
    runtime: Arc<Runtime>,
}

impl Host {
    pub fn new(client: Arc<HostClient>, runtime: Arc<Runtime>) -> Self {
        Host { client, runtime }
    }

    pub fn client(&self) -> &Arc<HostClient> {
        &self.client
    }

    /// Runs `work` on the host's tokio runtime; await the result from a GPUI task (`cx.spawn`), which runs on the
    /// window's thread, where the client's socket does not live.
    pub fn run<T, Fut>(&self, work: impl FnOnce(Arc<HostClient>) -> Fut) -> impl Future<Output = Result<T, Failure>> + 'static
    where
        T: Send + 'static,
        Fut: Future<Output = Result<T, Failure>> + Send + 'static,
    {
        let handle = self.runtime.spawn(work(self.client.clone()));
        let abort = AbortOnDrop(handle.abort_handle());
        async move {
            let _abort = abort;
            handle.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())))
        }
    }
}

/// Stops a host task when the GPUI task that reads it is dropped.
pub(crate) struct AbortOnDrop(pub(crate) tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The right panel's kinds, in the order and with the names and letters the web app's `panes.ts` gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Browser,
    Terminal,
    Changes,
    PullRequest,
    Files,
    Computer,
    Processes,
    Agents,
    Aside,
    Slate,
}

impl Pane {
    pub const ALL: [Pane; 10] = [
        Pane::Browser,
        Pane::Terminal,
        Pane::Changes,
        Pane::PullRequest,
        Pane::Files,
        Pane::Computer,
        Pane::Processes,
        Pane::Agents,
        Pane::Aside,
        Pane::Slate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Pane::Browser => "Browser",
            Pane::Terminal => "Terminal",
            Pane::Changes => "Changes",
            Pane::PullRequest => "Pull request",
            Pane::Files => "Files",
            Pane::Computer => "Computer",
            Pane::Processes => "Processes",
            Pane::Agents => "Agents",
            Pane::Aside => "Side question",
            Pane::Slate => "Slate",
        }
    }

    /// The letter that opens the pane from the panel's add menu; the side question opens only from the composer.
    pub fn key(self) -> Option<char> {
        match self {
            Pane::Browser => Some('B'),
            Pane::Terminal => Some('T'),
            Pane::Changes => Some('D'),
            Pane::PullRequest => Some('R'),
            Pane::Files => Some('F'),
            Pane::Computer => Some('M'),
            Pane::Processes => Some('P'),
            Pane::Agents => Some('A'),
            Pane::Aside => None,
            Pane::Slate => Some('S'),
        }
    }
}

/// Where the shell mounts a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// The main column, under the header row and beside the sidebar; the right panel closes while it shows, as it
    /// does for Settings today.
    Main,
    /// A tab of the right panel.
    Panel(Pane),
}

/// What a screen hands the shell besides its own view. Each is drawn only while the screen is the open one.
#[derive(Default)]
pub struct Parts {
    /// Drawn in the sidebar column under its 52 px frame row, in place of the threads, as Settings' page list is.
    pub sidebar: Option<AnyView>,
    /// A page's crumbs, in place of the shell's, as `SettingsCrumbs` stands in the web app's breadcrumb.
    pub crumbs: Option<AnyView>,
    /// The header's right-hand group, between the crumbs and the panel toggles (`AppShell.tsx`): the thread's context
    /// ring and its git and open splits, or Settings' own actions.
    pub actions: Option<AnyView>,
    /// For a screen in the right panel, its tab's name, read on every draw; the pane's label without one.
    pub title: Option<Title>,
}

/// What a screen is opened on: the thread the window shows and its workspace, a pane's own tab, and a page's argument.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opened {
    pub workspace_id: Option<String>,
    pub thread_id: Option<String>,
    /// A pane's tab, as `rightPanelStore.ts` names it: `browser:<page>`, `terminal:<shell>`, `file:<path>`, `files`, or
    /// the pane's name for a pane that holds one tab. A screen finds its session again by it after a thread switch. A
    /// route opens a pane on the tab its argument names: `terminal:terminal:7` is the Terminal on `terminal:7`.
    pub tab: Option<SharedString>,
    /// What follows the colon in a page's route: `settings:usage` opens Settings on its `usage` page.
    pub arg: Option<SharedString>,
}

/// A tab's name as its screen says it now, such as a terminal's shell.
pub type Title = Rc<dyn Fn(&App) -> SharedString>;

/// A screen the shell can route to. Implement it on the screen's root view and add its arm to [`crate::screens::screen`].
pub trait Screen: Render + Sized {
    /// The name the route and `desktop-gpui --screen <name>[:<arg>]` open it by. A pane's screen takes its pane's
    /// [`Pane::name`]; the thread view takes [`THREAD`].
    const NAME: &'static str;
    const PLACE: Place;

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self;

    /// Moves a kept screen. The shell keeps the [`THREAD`] screen across thread switches and tells it each one here,
    /// with no thread when the window shows none; it tells an open page its new argument, as `settings` going to
    /// `settings:usage`. Either may be told what it already shows, as on the way back from a page. Every other screen
    /// is built again for each place it opens.
    fn show(&mut self, _opened: Opened, _window: &mut Window, _cx: &mut Context<Self>) {}

    /// A pane's tab closed for good, whether or not its screen is built now: what the screen keeps for `opened.tab`,
    /// such as a terminal's shell, can go. A thread switch drops the screen as well, and is not a close.
    fn closed(_host: &Host, _opened: Opened, _cx: &mut App) {}

    /// The views the screen adds to the frame; none by default.
    fn parts(_this: &Entity<Self>, _window: &mut Window, _cx: &mut App) -> Parts {
        Parts::default()
    }
}

/// The name the thread view registers under, which the shell mounts for every thread route.
pub const THREAD: &str = "thread";

/// A screen built and ready for the frame.
pub struct Mounted {
    pub place: Place,
    pub view: AnyView,
    pub parts: Parts,
    /// [`Screen::show`] on the screen.
    pub show: Show,
}

/// [`Screen::show`] on a screen the shell holds only as a view.
pub type Show = Rc<dyn Fn(Opened, &mut Window, &mut App)>;

/// A registered screen: where it goes, how it is built, and [`Screen::closed`].
#[derive(Clone, Copy)]
pub struct Entry {
    pub place: Place,
    pub mount: fn(&Host, Opened, &mut Window, &mut App) -> Mounted,
    pub closed: fn(&Host, Opened, &mut App),
}

/// The entry for `S`; each arm of [`crate::screens::screen`] is one call of this.
pub fn entry<S: Screen>() -> Entry {
    Entry { place: S::PLACE, mount: mount::<S>, closed: S::closed }
}

fn mount<S: Screen>(host: &Host, opened: Opened, window: &mut Window, cx: &mut App) -> Mounted {
    let host = host.clone();
    let entity = cx.new(|cx| S::new(host, opened, window, cx));
    let parts = S::parts(&entity, window, cx);
    let kept = entity.clone();
    let show = Rc::new(move |opened: Opened, window: &mut Window, cx: &mut App| kept.update(cx, |s, cx| s.show(opened, window, cx)));
    Mounted { place: S::PLACE, view: entity.into(), parts, show }
}

/// Where the window is: nothing open, a thread, or a screen by its [`Screen::NAME`], with `:<arg>` after it for one
/// opened on a part of itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Empty,
    Thread { workspace_id: String, thread_id: String },
    Screen(SharedString),
}

/// A screen route's name and argument: `settings:usage` is `settings` on `usage`.
pub fn split(route: &str) -> (&str, Option<&str>) {
    match route.split_once(':') {
        Some((name, arg)) => (name, Some(arg)),
        None => (route, None),
    }
}

struct Router {
    window: AnyWindowHandle,
    shell: WeakEntity<Shell>,
}

impl Global for Router {}

/// Registers the window's shell as the one [`navigate`] moves.
pub fn install(window: AnyWindowHandle, shell: WeakEntity<Shell>, cx: &mut App) {
    cx.set_global(Router { window, shell });
}

/// Moves the window to `route`. Safe to call from inside any view's update: the move runs once that update ends.
pub fn navigate(route: Route, cx: &mut App) {
    let Some(router) = cx.try_global::<Router>() else { return };
    let (handle, shell) = (router.window, router.shell.clone());
    cx.defer(move |cx| {
        _ = handle.update(cx, |_, window, cx| shell.update(cx, |shell, cx| shell.go(route, window, cx)));
    });
}
