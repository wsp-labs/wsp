// SPDX-License-Identifier: AGPL-3.0-only
//! The terminal: a shell per tab over the host's daemon channel, drawn from libghostty-vt's screen.

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

use gpui_kit::{prelude::*, AnyView, App, Entity, Global, WeakEntity, Window};

use drawer::{Drawer, Mode};
use hub::Key;
pub use panel::TerminalScreen;

use crate::screens::sidebar::Sidebar;
use crate::screens::{Host, Route};
use crate::shell::Shell;

/// The drawer `route` shows under the thread view: the thread's own, this computer's with no thread, none on a page.
pub fn drawer_key(route: &Route) -> Option<Key> {
    match route {
        Route::Thread { workspace_id, thread_id } => Some(Key::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone() }),
        Route::Empty => Some(Key::Here),
        Route::Screen(_) => None,
    }
}

/// Whether `route`'s drawer is open, host or none.
pub fn drawer_open(route: &Route, cx: &mut App) -> bool {
    drawer_key(route).is_some_and(|key| hub::is_open(&key, cx))
}

/// The drawer built for the key on screen; another key's is dropped, its shells kept by their channel.
#[derive(Default)]
struct Shown(Option<(Key, Entity<Drawer>)>);

impl Global for Shown {}

/// The drawer under the thread view while `route`'s drawer is open, built on first show and kept while its key shows.
pub fn drawer(host: Option<Host>, route: &Route, window: &mut Window, cx: &mut App) -> Option<AnyView> {
    let key = drawer_key(route)?;
    if !hub::is_open(&key, cx) {
        return None;
    }
    if let Some((shown, view)) = &cx.default_global::<Shown>().0 {
        if *shown == key {
            return Some(view.clone().into());
        }
    }
    let host = host?;
    let view = cx.new(|cx| Drawer::new(host, Mode::Drawer(key.clone()), window, cx));
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

/// Takes what the host keeps for the terminal in its preferences: each workspace's zoom (`terminalZoom`).
pub fn read_preferences(preferences: &serde_json::Value, cx: &mut App) {
    hub::read_zoom(preferences.get("terminalZoom").unwrap_or(&serde_json::Value::Null), cx);
}

/// The drawer built for the key on screen, if one is.
pub fn shown_drawer(cx: &App) -> Option<Entity<Drawer>> {
    cx.try_global::<Shown>()?.0.as_ref().map(|(_, view)| view.clone())
}

/// `terminal.toggle`: opens or closes `route`'s drawer, focusing its active shell as it opens.
pub fn toggle_drawer(route: &Route, cx: &mut App) {
    let Some(key) = drawer_key(route) else { return };
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
