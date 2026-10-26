// SPDX-License-Identifier: AGPL-3.0-only
//! The page Settings stands on, kept for its next opening as the web keeps `wsp:settings-at` (`settingsStore.ts`): each
//! window reads the app's store once and keeps its own page from then on, and every move writes the store, so a new
//! window opens where any window last was. Never on the host.

use std::collections::HashMap;

use gpui_kit::{App, Global, WindowId};
use serde_json::Value;

use super::model::At;
use crate::local_store::LocalStore;

pub const AT_KEY: &str = "wsp:settings-at";

/// Each window's page; a window's goes when it closes.
#[derive(Default)]
struct Kept(HashMap<WindowId, At>);

impl Global for Kept {}

fn kept(cx: &mut App) -> &mut Kept {
    if !cx.has_global::<Kept>() {
        cx.set_global(Kept::default());
        cx.on_window_closed(|cx, window| {
            cx.global_mut::<Kept>().0.remove(&window);
        })
        .detach();
    }
    cx.global_mut::<Kept>()
}

/// Where `window` opens Settings with no page named: its own last page, else the store's, else the first page.
pub fn last(window: WindowId, cx: &mut App) -> At {
    if let Some(at) = kept(cx).0.get(&window) {
        return at.clone();
    }
    let stored = cx.try_global::<LocalStore>().and_then(|s| s.get_as::<String>(AT_KEY)).and_then(|word| At::parse(&word));
    let at = stored.unwrap_or(At::FIRST);
    kept(cx).0.insert(window, at.clone());
    at
}

/// Keeps `at` as `window`'s page and the store's; a store that refuses the write leaves it kept for this window alone.
pub fn keep(window: WindowId, at: &At, cx: &mut App) {
    kept(cx).0.insert(window, at.clone());
    if cx.has_global::<LocalStore>() {
        let word = Value::from(at.id());
        if let Err(e) = cx.global_mut::<LocalStore>().set(AT_KEY, word) {
            eprintln!("desktop-gpui: settings page not kept: {e}");
        }
    }
}
