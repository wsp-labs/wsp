// SPDX-License-Identifier: AGPL-3.0-only
//! What the host keeps for the frame: the sidebar's width, the person's chords and the theme, read from the app's one
//! copy of the preferences and written through it.

use std::collections::BTreeMap;
use std::sync::Arc;

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{px, Context, Task, Window};
use wsp_host_client::protocol::{EventUnion, Preferences};
use wsp_host_client::{Event, HostClient};

use super::Shell;
use crate::screens;
use crate::screens::app_shell::frame;
use crate::screens::app_shell::keys;
use crate::screens::app_shell::material::{self, Look};
use crate::screens::app_shell::vibrancy;
use crate::screens::app_shell::AbortOnDrop;
use crate::screens::palette;
use crate::screens::thread_view::Capture;
use crate::ui::{self, ActivePalette};

impl Shell {
    /// Lays on what the preferences say for the frame: the sidebar's width, the person's chords and the theme.
    pub(super) fn apply(&mut self, preferences: &Preferences, window: &mut Window, cx: &mut Context<Self>) {
        let value = serde_json::to_value(preferences).unwrap_or_default();
        crate::screens::thread_view::markdown::set_sizes(&value, cx);
        self.sidebar_width = value.get("sidebarWidth").and_then(|w| w.as_f64()).map_or(frame::SIDEBAR_DEFAULT, |w| px(w as f32));
        self.rebind(&keys::overrides_of(value.get("keybindings")), cx);
        screens::terminal::read_preferences(&value, cx);
        // Electron's nativeTheme.themeSource: the lights, the window's edge and macOS's material take the pick.
        cx.set_window_appearance(match value.get("theme").and_then(|t| t.as_str()) {
            Some("dark") => Some(gpui_kit::WindowAppearance::Dark),
            Some("light") => Some(gpui_kit::WindowAppearance::Light),
            _ => None,
        });
        self.pick_theme(window, cx);
        cx.notify();
    }

    /// The theme the person's picks name for the computer's appearance now, which a system pick follows as it moves.
    pub(super) fn pick_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let system_dark = ui::theme::is_dark(window.appearance());
        let (mut theme, transparency) = match crate::prefs::shown(cx) {
            Some(p) => (ui::theme::picked(&p.theme.to_string(), &p.light_theme, &p.dark_theme, system_dark), p.transparency),
            // Before the host answers, the system's side in its default theme, on the glass.
            None => (ui::theme::picked("system", ui::theme::LIGHT, ui::theme::DARK, system_dark), true),
        };
        if let Some(entry) = cx.try_global::<Capture>().and_then(|c| c.theme.as_deref()).and_then(ui::theme::theme_by_id) {
            theme = entry;
        }
        self.restyle(theme.id, transparency, window, cx);
    }

    /// Draws the window in `theme` and, on the Mac, on its glass or solid as `transparency` and the computer ask.
    fn restyle(&mut self, theme: &'static str, transparency: bool, window: &mut Window, cx: &mut Context<Self>) {
        let look = Look { mac: cfg!(target_os = "macos"), glass: transparency && !vibrancy::reduce_transparency() };
        let drawn = cx.global::<ui::theme::WspTheme>();
        let drawn = drawn.entry.id == theme && drawn.look == look;
        if drawn && self.theme == Some(theme) && look == self.look {
            return;
        }
        // Another window, or a theme a page showed under the pointer, may have drawn it already.
        if !drawn {
            ui::theme::set_theme_in(theme, look, cx);
        }
        let fills = material::fills(look, theme, cx.palette());
        self.vibrancy.set(fills.vibrancy, window);
        self.theme = Some(theme);
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
        palette::palette::rebind(&self.rules, &next, cx);
        self.rules = next;
    }

    /// Hands the host's pushed preferences to the app's copy, as another client changes them.
    pub(super) fn follow_preferences(&mut self, host: Arc<HostClient>, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
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
                if this.update(cx, |_, cx| crate::prefs::store(cx).update(cx, |store, cx| store.landed(&preferences, cx))).is_err() {
                    break;
                }
            }
        })
    }

    /// Writes the sidebar's width, as the web app writes it once a drag ends.
    pub(super) fn save_sidebar_width(&self, cx: &mut gpui_kit::App) {
        let Some(host) = self.host.clone() else { return };
        let width = f32::from(self.sidebar_width).round() as i64;
        let host = Some((host, self.runtime.clone()));
        crate::prefs::store(cx).update(cx, |store, cx| {
            store.write(serde_json::json!({ "sidebarWidth": width }), host, cx, |answer, _| {
                if let Err(said) = answer {
                    eprintln!("desktop-gpui: {said}");
                }
            })
        });
    }
}
