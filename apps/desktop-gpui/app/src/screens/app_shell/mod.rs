// SPDX-License-Identifier: AGPL-3.0-only
//! The frame every screen lives in, and the contract a screen signs to be mounted in it: a [`Screen`] is built from a
//! [`Host`], says where it goes with a [`Place`], and may hand the shell a sidebar column and a header of its own.

pub mod assets;
pub mod frame;
pub mod keys;
pub mod material;
pub mod menu;
pub mod panel;
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
        async move { handle.await.unwrap_or_else(|e| Err(Failure::new(e.to_string()))) }
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
    /// Drawn in the header row after the sidebar's toggle, in place of the thread's breadcrumb and the panel toggles.
    pub header: Option<AnyView>,
    /// For a screen in the right panel, its tab's name, read on every draw; the pane's label without one.
    pub title: Option<Title>,
}

/// A tab's name as its screen says it now, such as a terminal's shell.
pub type Title = Rc<dyn Fn(&App) -> SharedString>;

/// A screen the shell can route to. Implement it on the screen's root view and add its arm to [`crate::screens::screen`].
pub trait Screen: Render + Sized {
    /// The name the route and `desktop-gpui --screen <name>` open it by.
    const NAME: &'static str;
    const PLACE: Place;

    fn new(host: Host, window: &mut Window, cx: &mut Context<Self>) -> Self;

    /// The views the screen adds to the frame; none by default.
    fn parts(_this: &Entity<Self>, _window: &mut Window, _cx: &mut App) -> Parts {
        Parts::default()
    }
}

/// A screen built and ready for the frame.
pub struct Mounted {
    pub place: Place,
    pub view: AnyView,
    pub parts: Parts,
}

/// Builds `S` for the frame; each arm of [`crate::screens::screen`] names this for its type.
pub fn mount<S: Screen>(host: &Host, window: &mut Window, cx: &mut App) -> Mounted {
    let host = host.clone();
    let entity = cx.new(|cx| S::new(host, window, cx));
    let parts = S::parts(&entity, window, cx);
    Mounted { place: S::PLACE, view: entity.into(), parts }
}

/// Where the window is: nothing open, a thread, or a screen by its [`Screen::NAME`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Empty,
    Thread { workspace_id: String, thread_id: String },
    Screen(SharedString),
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
