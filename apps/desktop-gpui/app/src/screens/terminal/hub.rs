// SPDX-License-Identifier: AGPL-3.0-only
//! What the terminal keeps beside its shells: each drawer's arrangement by its key (`drawerStore.ts`), each panel
//! tab's surface, the person's font and each workspace's zoom. Drawers are saved in the app's store as the web saves
//! them in localStorage; a drawer left as it starts is not kept.

use std::collections::{BTreeMap, HashMap};

use gpui_kit::{App, Global, SharedString};

use super::groups::Ui;
use super::link::Target;
use crate::local_store::LocalStore;

/// `wsp:terminal-drawer:v1` in `drawerStore.ts`.
pub const DRAWERS: &str = "wsp:terminal-drawer:v1";
/// `wsp:terminal-font` in `fontSetting.ts`.
pub const FONT: &str = "wsp:terminal-font";
/// `HERE_KEY` in `terminal/computer.ts`: this computer's own drawer, in its home folder.
pub const HERE: &str = "place:here";

/// Whose drawer: a thread's (`<ws>#<thread>`), a workspace's own (`<ws>`), or this computer's.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Thread { workspace_id: String, thread_id: String },
    Workspace(String),
    Here,
}

impl Key {
    /// `stateKeyOf` in `stateKey.ts`.
    pub fn name(&self) -> String {
        match self {
            Key::Thread { workspace_id, thread_id } => format!("{workspace_id}#{thread_id}"),
            Key::Workspace(workspace_id) => workspace_id.clone(),
            Key::Here => HERE.to_owned(),
        }
    }

    pub fn target(&self) -> Target {
        match self {
            Key::Thread { workspace_id, .. } | Key::Workspace(workspace_id) => Target::Workspace(workspace_id.clone()),
            Key::Here => Target::Here,
        }
    }

    pub fn workspace_id(&self) -> Option<&str> {
        match self {
            Key::Thread { workspace_id, .. } | Key::Workspace(workspace_id) => Some(workspace_id),
            Key::Here => None,
        }
    }

    pub fn thread_id(&self) -> Option<&str> {
        match self {
            Key::Thread { thread_id, .. } => Some(thread_id),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Hub {
    read: bool,
    drawers: BTreeMap<String, Ui>,
    /// The drawers whose first shell was asked for on this run: a first open spawns once, and closing the last never
    /// spawns again (`WorkspaceTerminalDrawer.tsx`).
    spawned: Vec<String>,
    surfaces: HashMap<SharedString, Ui>,
    font: Option<String>,
    zoom: HashMap<String, f32>,
}

impl Global for Hub {}

fn hub(cx: &mut App) -> &mut Hub {
    if !cx.default_global::<Hub>().read {
        let stored = cx.try_global::<LocalStore>();
        let drawers = stored.and_then(|s| s.get_as::<BTreeMap<String, Ui>>(DRAWERS)).unwrap_or_default();
        let font = stored.and_then(|s| s.get_as::<String>(FONT)).filter(|f| !f.trim().is_empty());
        let hub = cx.global_mut::<Hub>();
        hub.read = true;
        hub.drawers = drawers.into_iter().map(|(k, ui)| (k, ui.normalize())).collect();
        hub.font = font;
    }
    cx.global_mut::<Hub>()
}

fn save(cx: &mut App) {
    let value = serde_json::to_value(&hub(cx).drawers).unwrap_or_default();
    if cx.has_global::<LocalStore>() {
        if let Err(e) = cx.global_mut::<LocalStore>().set(DRAWERS, value) {
            eprintln!("desktop-gpui: {DRAWERS}: {e}");
        }
    }
}

pub fn drawer(key: &Key, cx: &mut App) -> Ui {
    hub(cx).drawers.get(&key.name()).cloned().unwrap_or_default()
}

pub fn set_drawer(key: &Key, ui: Ui, cx: &mut App) {
    let ui = ui.normalize();
    let name = key.name();
    let drawers = &mut hub(cx).drawers;
    if drawers.get(&name) == Some(&ui) || (ui.is_default() && !drawers.contains_key(&name)) {
        return;
    }
    if ui.is_default() {
        drawers.remove(&name);
    } else {
        drawers.insert(name, ui);
    }
    save(cx);
}

pub fn is_open(key: &Key, cx: &mut App) -> bool {
    drawer(key, cx).terminal_open
}

pub fn set_open(key: &Key, open: bool, cx: &mut App) {
    let ui = drawer(key, cx).set_open(open);
    set_drawer(key, ui, cx);
}

pub fn toggle(key: &Key, cx: &mut App) {
    let open = is_open(key, cx);
    set_open(key, !open, cx);
}

/// Marks `key`'s first shell as asked for, answering whether it had been already.
pub fn mark_spawned(key: &Key, cx: &mut App) -> bool {
    let name = key.name();
    let spawned = &mut hub(cx).spawned;
    if spawned.contains(&name) {
        return true;
    }
    spawned.push(name);
    false
}

/// A refused first shell is asked for again on the next mount.
pub fn unmark_spawned(key: &Key, cx: &mut App) {
    let name = key.name();
    hub(cx).spawned.retain(|k| *k != name);
}

pub fn surface(tab: &SharedString, cx: &mut App) -> Option<Ui> {
    hub(cx).surfaces.get(tab).cloned()
}

/// The same, for a reader that cannot change the hub, as a tab's title.
pub fn surface_read(tab: &SharedString, cx: &App) -> Option<Ui> {
    cx.try_global::<Hub>()?.surfaces.get(tab).cloned()
}

pub fn set_surface(tab: &SharedString, ui: Ui, cx: &mut App) {
    hub(cx).surfaces.insert(tab.clone(), ui.normalize());
}

pub fn drop_surface(tab: &SharedString, cx: &mut App) -> Option<Ui> {
    hub(cx).surfaces.remove(tab)
}

/// Every shell a panel tab holds, which never shows in a drawer.
pub fn panel_shells(cx: &mut App) -> Vec<String> {
    hub(cx).surfaces.values().flat_map(|ui| ui.terminal_ids.iter().cloned()).collect()
}

pub fn font(cx: &mut App) -> Option<String> {
    hub(cx).font.clone()
}

/// Sets the font every terminal draws with; empty draws the config's or the default.
pub fn set_font(font: &str, cx: &mut App) {
    let font = Some(font.trim().to_owned()).filter(|f| !f.is_empty());
    if hub(cx).font == font {
        return;
    }
    hub(cx).font = font.clone();
    if cx.has_global::<LocalStore>() {
        let store = cx.global_mut::<LocalStore>();
        let written = match &font {
            Some(font) => store.set(FONT, serde_json::Value::String(font.clone())),
            None => store.remove(FONT),
        };
        if let Err(e) = written {
            eprintln!("desktop-gpui: {FONT}: {e}");
        }
    }
}

pub fn zoom(workspace_id: &str, cx: &mut App) -> Option<f32> {
    hub(cx).zoom.get(workspace_id).copied()
}

pub fn set_zoom(workspace_id: &str, size: Option<f32>, cx: &mut App) {
    let zoom = &mut hub(cx).zoom;
    match size {
        Some(size) => zoom.insert(workspace_id.to_owned(), size),
        None => zoom.remove(workspace_id),
    };
}

/// Takes the host's `terminalZoom` preference, each workspace's font size in pixels.
pub fn read_zoom(preference: &serde_json::Value, cx: &mut App) {
    let Some(sizes) = preference.as_object() else { return };
    hub(cx).zoom = sizes.iter().filter_map(|(ws, px)| Some((ws.clone(), px.as_f64()? as f32))).collect();
}
