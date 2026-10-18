// SPDX-License-Identifier: AGPL-3.0-only
//! The terminal: a shell per tab over the host's daemon channel, drawn from libghostty-vt's screen.

pub mod boot;
pub mod drawer;
pub mod engine;
pub mod font_card;
pub mod groups;
pub mod hub;
pub mod input;
pub mod link;
pub mod links;
pub mod look;
pub mod menu;
pub mod overlay;
pub mod panel;
pub mod session;
pub mod state;
pub mod tabs;
pub mod toolbar;
pub mod view;
pub mod words;

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::{prelude::*, AnyView, App, Entity, Global, WeakEntity, Window};

use drawer::{Drawer, Mode};
use hub::Key;
pub use panel::TerminalScreen;

use crate::screens::composer::screen::ComposerScreen;
use crate::screens::sidebar::Sidebar;
use crate::screens::{Host, Route, Screen};
use crate::shell::Shell;

/// The drawer `route` shows under the thread view: the thread's own, this computer's with no thread, none on a page
/// but the New thread page. That page's is its own under the empty thread (`threadStateKey(ws, "")` in
/// `drawerStore.ts`) until its first send, then the thread the send started, which the page goes on showing.
pub fn drawer_key(route: &Route, cx: &App) -> Option<Key> {
    match route {
        Route::Thread { workspace_id, thread_id } => Some(Key::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone() }),
        Route::Empty => Some(Key::Here),
        Route::Screen(screen) => {
            let workspace_id = screen.strip_prefix(ComposerScreen::NAME)?.strip_prefix(':')?;
            let started = cx.try_global::<PageThreads>().and_then(|p| p.0.get(workspace_id)).cloned();
            Some(Key::Thread { workspace_id: workspace_id.to_owned(), thread_id: started.unwrap_or_default() })
        }
    }
}

/// The thread each workspace's New thread page started with its first send, by workspace.
#[derive(Default)]
struct PageThreads(HashMap<String, String>);

impl Global for PageThreads {}

/// A New thread page opened afresh on `workspace_id`: its drawer is a page's own again, with no thread yet.
pub fn page_starts_over(workspace_id: &str, cx: &mut App) {
    cx.default_global::<PageThreads>().0.remove(workspace_id);
}

/// `NEW_THREAD_PAGE` in `drawerStore.ts`: this run's New thread page as its shells' thread names it on the host, so a
/// first send carries this window's page shells alone.
pub fn new_thread_page() -> &'static str {
    static PAGE: OnceLock<String> = OnceLock::new();
    PAGE.get_or_init(|| {
        let made = |seed: u128| RandomState::new().hash_one(seed);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        format!("{NEW_THREAD_PAGE_MARK}{:016x}{:016x}", made(now), made(u128::from(std::process::id())))
    })
}

/// The mark every New thread page's tag starts with, any window's.
pub const NEW_THREAD_PAGE_MARK: &str = "new:";

/// `carryNewThreadDrawer`: the New thread page's shells and drawer go to the thread its first send opened, which the
/// page's drawer is from then on; a workspace with no channel open has no shells to hand on, and none is dialled.
pub fn carry_new_thread(workspace_id: &str, thread_id: &str, cx: &mut App) {
    let draft = Key::Thread { workspace_id: workspace_id.to_owned(), thread_id: String::new() };
    let thread = Key::Thread { workspace_id: workspace_id.to_owned(), thread_id: thread_id.to_owned() };
    cx.default_global::<PageThreads>().0.insert(workspace_id.to_owned(), thread_id.to_owned());
    hub::carry(&draft, &thread, cx);
    let Some(link) = link::Link::find(&draft.target(), cx) else { return };
    link.update(cx, |link, cx| {
        let page: Vec<String> = link
            .ptys()
            .iter()
            .filter(|p| p.tag == link::Tag::Thread(new_thread_page().to_owned()))
            .map(|p| p.session.read(cx).pty().to_string())
            .collect();
        for pty in page {
            link.retag(&pty, link::Tag::Thread(thread_id.to_owned()), cx);
        }
    });
}

/// Whether `route`'s drawer is open, host or none.
pub fn drawer_open(route: &Route, cx: &mut App) -> bool {
    drawer_key(route, cx).is_some_and(|key| hub::is_open(&key, cx))
}

/// The drawer built for the key on screen; another key's is dropped, its shells kept by their channel.
#[derive(Default)]
struct Shown(Option<(Key, Entity<Drawer>)>);

impl Global for Shown {}

/// The drawer under the thread view while `route`'s drawer is open, built on first show and kept while its key shows;
/// see-through where the page draws `glass`.
pub fn drawer(host: Option<Host>, route: &Route, glass: bool, window: &mut Window, cx: &mut App) -> Option<AnyView> {
    let Some(key) = drawer_key(route, cx).filter(|key| hub::is_open(key, cx)) else {
        // A drawer off screen lets go of its channel, which then closes after 30 s; its shells stay on the daemon.
        cx.default_global::<Shown>().0 = None;
        return None;
    };
    if let Some((shown, view)) = cx.default_global::<Shown>().0.clone() {
        if shown == key {
            view.update(cx, |d, cx| d.set_glass(glass, cx));
            return Some(view.into());
        }
    }
    let host = host?;
    let view = cx.new(|cx| Drawer::new(host, Mode::Drawer(key.clone()), window, cx));
    view.update(cx, |d, cx| d.set_glass(glass, cx));
    cx.global_mut::<Shown>().0 = Some((key, view.clone()));
    Some(view.into())
}

/// The sidebar whose workspace records and live statuses the panes read their state from.
struct Workspaces(WeakEntity<Sidebar>);

impl Global for Workspaces {}

/// Hands the panes the window's sidebar, once, for the state of the workspaces their shells run on.
pub fn read_workspaces_from(sidebar: &Entity<Sidebar>, cx: &mut App) {
    cx.set_global(Workspaces(sidebar.downgrade()));
}

pub(crate) fn workspaces_sidebar(cx: &App) -> Option<Entity<Sidebar>> {
    cx.try_global::<Workspaces>()?.0.upgrade()
}

/// Takes what the host keeps for the terminal in its preferences: each workspace's zoom (`terminalZoom`) and where
/// the size it adds to comes from (`terminalSize`).
pub fn read_preferences(preferences: &serde_json::Value, cx: &mut App) {
    hub::read_zoom(preferences.get("terminalZoom").unwrap_or(&serde_json::Value::Null), cx);
    look::read_size_source(preferences.get("terminalSize").unwrap_or(&serde_json::Value::Null), cx);
}

/// The drawer built for the key on screen, if one is.
pub fn shown_drawer(cx: &App) -> Option<Entity<Drawer>> {
    cx.try_global::<Shown>()?.0.as_ref().map(|(_, view)| view.clone())
}

/// `terminal.toggle`: opens or closes `route`'s drawer, focusing its active shell as it opens.
pub fn toggle_drawer(route: &Route, cx: &mut App) {
    let Some(key) = drawer_key(route, cx) else { return };
    hub::toggle(&key, cx);
    focus_on_open(&key, cx);
}

fn focus_on_open(key: &Key, cx: &mut App) {
    if !hub::is_open(key, cx) {
        return;
    }
    if let Some((shown, view)) = cx.default_global::<Shown>().0.clone() {
        if shown == *key {
            view.update(cx, |d, cx| d.focus_active(cx));
        }
    }
}

/// The sidebar's "Open terminal" (`showTerminal`): opens the drawer, never the panel, and spawns nothing itself. On
/// the workspace on screen it is the open thread's drawer, else the workspace's own.
pub fn open_terminal(shell: &mut Shell, workspace_id: &str, _window: &mut Window, cx: &mut gpui_kit::Context<Shell>) {
    let key = match shell.route() {
        Route::Thread { workspace_id: open, thread_id } if open == workspace_id => {
            Key::Thread { workspace_id: open.clone(), thread_id: thread_id.clone() }
        }
        _ => Key::Workspace(workspace_id.to_owned()),
    };
    hub::set_open(&key, true, cx);
    focus_on_open(&key, cx);
    cx.notify();
}
