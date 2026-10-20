// SPDX-License-Identifier: AGPL-3.0-only
//! What the host keeps for the frame: the sidebar's width, the person's chords and the theme, read and written.

use std::collections::BTreeMap;
use std::sync::Arc;

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{px, Context, Task, Window};
use wsp_host_client::protocol::{EventUnion, Preferences, PreferencesSetRequest};
use wsp_host_client::{Event, HostClient};

use super::{Picks, Shell};
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
    /// Reads what the host keeps for the frame: the sidebar's width, the person's chords and the theme.
    pub(super) fn apply(&mut self, preferences: &Preferences, window: &mut Window, cx: &mut Context<Self>) {
        let value = serde_json::to_value(preferences).unwrap_or_default();
        crate::screens::thread_view::markdown::set_sizes(&value, cx);
        self.sidebar_width = value.get("sidebarWidth").and_then(|w| w.as_f64()).map_or(frame::SIDEBAR_DEFAULT, |w| px(w as f32));
        self.rebind(&keys::overrides_of(value.get("keybindings")), cx);
        screens::terminal::read_preferences(&value, cx);
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
    pub(super) fn pick_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = cx.try_global::<Capture>().and_then(|c| c.theme.as_deref()).and_then(ui::theme::theme_by_id) {
            self.restyle(entry.id, self.picks.transparency, window, cx);
            return;
        }
        let mode = self.picks.mode.as_deref().unwrap_or("system");
        let theme = ui::theme::picked(mode, &self.picks.light, &self.picks.dark, ui::theme::is_dark(window.appearance()));
        self.restyle(theme.id, self.picks.transparency, window, cx);
    }

    /// Draws the window in `theme` and, on the Mac, on its glass or solid as `transparency` and the computer ask.
    fn restyle(&mut self, theme: &'static str, transparency: bool, window: &mut Window, cx: &mut Context<Self>) {
        let look = Look { mac: cfg!(target_os = "macos"), glass: transparency && !vibrancy::reduce_transparency() };
        if self.theme == Some(theme) && look == self.look {
            return;
        }
        ui::theme::set_theme_in(theme, look, cx);
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

    /// Follows the host's pushed preferences, as another window or the Settings page changes them.
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
                if this.update_in(cx, |this, window, cx| this.apply(&preferences, window, cx)).is_err() {
                    break;
                }
            }
        })
    }

    /// Writes the sidebar's width, as the web app writes it once a drag ends.
    pub(super) fn save_sidebar_width(&self) {
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
}
