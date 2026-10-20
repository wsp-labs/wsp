// SPDX-License-Identifier: AGPL-3.0-only
//! The shell's own actions: its toggles and steps, Escape on a page, and the end of a rail's drag.

use std::time::Instant;

use gpui_kit::{App, Context, Window};

use super::{Shell, Side, SETTINGS, SIDEBAR_SLIDE};
use crate::screens::app_shell::panel::At;
use crate::screens::app_shell::{frame, keys};
use crate::screens::sidebar::walk;
use crate::screens::{self, app_shell, dialogs, Pane, Route};
use crate::sidebar::ThreadItem;
use crate::thread_view::ThreadViewEvent;
use crate::ui;

impl Shell {
    pub(super) fn end_drag(&mut self, cx: &mut App) {
        match self.dragging.take() {
            Some(Side::Sidebar) => self.save_sidebar_width(),
            Some(Side::Panel) => self.panel.update(cx, |p, cx| p.keep_width(cx)),
            None => {}
        }
    }

    pub(super) fn toggle_sidebar(&mut self, _: &keys::ToggleSidebar, window: &mut Window, cx: &mut Context<Self>) {
        if ui::narrow::width(window).md {
            self.sheet_open = !self.sheet_open;
            self.sheet_focus_due = self.sheet_open;
            cx.notify();
            return;
        }
        self.sidebar_open = !self.sidebar_open;
        // Turned back mid-slide, it goes back from where it stands rather than from the far end.
        let done = self.sidebar_moved.map_or(SIDEBAR_SLIDE, |moved| moved.elapsed().min(SIDEBAR_SLIDE));
        self.sidebar_moved = Some(Instant::now() - (SIDEBAR_SLIDE - done));
        cx.notify();
    }

    pub(super) fn toggle_panel(&mut self, _: &keys::ToggleRightPanel, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open() || (self.panel.read(cx).is_open() && self.fade_panel_sheet(window, cx)) {
            return;
        }
        self.panel.update(cx, |p, cx| p.toggle(window, cx));
    }

    pub(super) fn toggle_preview(&mut self, _: &keys::TogglePreview, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.panel_at(cx), Some(At::Thread { .. })) {
            return;
        }
        let shuts = self.panel.read(cx).is_open() && self.panel.read(cx).active() == Some(Pane::Browser);
        if !(shuts && self.fade_panel_sheet(window, cx)) {
            self.panel.update(cx, |p, cx| p.toggle_pane(Pane::Browser, window, cx));
        }
    }

    /// Starts the sheet's fade on its way shut, or brings it back if asked on the way out; false where it shuts at once.
    fn fade_panel_sheet(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        if window.viewport_size().width > frame::SHEET_MAX_VIEWPORT {
            return false;
        }
        self.panel_fading = match self.panel_fading.take() {
            Some(_) => None,
            None => Some({
                let p = self.panel.read(cx);
                (p.tabs(), p.active())
            }),
        };
        cx.notify();
        true
    }

    /// Ends the sheet's fade: the panel shuts and lets its panes go.
    pub(super) fn end_panel_fade(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(at_fade) = self.panel_fading.take() else { return };
        let p = self.panel.read(cx);
        if p.is_open() && (p.tabs(), p.active()) == at_fade {
            self.panel.update(cx, |p, cx| p.toggle(window, cx));
        }
    }

    pub(super) fn toggle_drawer(&mut self, _: &keys::ToggleTerminal, _: &mut Window, cx: &mut Context<Self>) {
        screens::terminal::toggle_drawer(&self.place(), cx);
        cx.notify();
    }

    pub(super) fn toggle_settings(&mut self, _: &keys::ToggleSettings, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(&self.route, Route::Screen(route) if app_shell::split(route).0 == SETTINGS) {
            self.close_settings(&keys::CloseSettings, window, cx);
        } else {
            self.go(Route::Screen(SETTINGS.into()), window, cx);
        }
    }

    pub(super) fn close_settings(&mut self, _: &keys::CloseSettings, window: &mut Window, cx: &mut Context<Self>) {
        // Escape's binding outranks the layer's key handler, so a dialog over the page passes it on to close itself.
        if dialogs::layer::top(cx).is_some() {
            cx.propagate();
            return;
        }
        if self.settings_open() {
            let behind = std::mem::replace(&mut self.behind, Route::Empty);
            self.screen = None;
            // A theme a page showed under the pointer goes with the page.
            if let Some(theme) = self.theme.filter(|theme| ui::theme::current(cx).id != *theme) {
                ui::theme::set_theme(theme, cx);
            }
            self.go(behind, window, cx);
        }
    }

    /// Escape on a page: the page takes it first for something of its own, and only then does the page close.
    pub(super) fn escape_page(&mut self, _: &keys::EscapePage, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(escape) = self.screen.as_ref().map(|screen| screen.escape.clone()) {
            if escape(window, cx) {
                return;
            }
        }
        if self.close_sheet(window, cx) {
            return;
        }
        self.close_settings(&keys::CloseSettings, window, cx);
    }

    /// Shuts the sidebar's sheet when it shows; false when there was none to shut.
    pub fn close_sheet(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        if !(self.sheet_open && ui::narrow::width(window).md) || dialogs::layer::top(cx).is_some() {
            return false;
        }
        self.sheet_open = false;
        cx.notify();
        true
    }

    pub(super) fn close_panel_tab(&mut self, _: &keys::ClosePanelTab, window: &mut Window, cx: &mut Context<Self>) {
        self.panel.update(cx, |p, cx| {
            if let Some(ix) = p.active_index() {
                p.close(ix, window, cx);
            }
        });
    }

    /// The workspaces in the order the sidebar draws them, each with its first thread there.
    pub(super) fn workspaces(&self, cx: &App) -> Vec<ThreadItem> {
        let mut seen: Vec<ThreadItem> = Vec::new();
        for item in self.sidebar.read(cx).items() {
            if !seen.iter().any(|s| s.workspace_id == item.workspace_id) {
                seen.push(item.clone());
            }
        }
        seen
    }

    pub(super) fn select_workspace(&mut self, slot: &keys::SelectWorkspace, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.workspaces(cx).into_iter().nth(usize::from(slot.0).saturating_sub(1)) else { return };
        self.go(Route::Thread { workspace_id: item.workspace_id, thread_id: item.thread_id }, window, cx);
    }

    /// The step along the open thread's level of the sidebar (`stepThreadLevel`): the sidebar opens the thread it lands
    /// on, and the caret goes to that thread's composer once the frame has moved the thread screen onto it.
    pub(super) fn step_level(&mut self, by: isize, cx: &mut Context<Self>) {
        if !self.sidebar.update(cx, |s, cx| s.step_level(by, cx)) {
            return;
        }
        let shell = cx.entity().downgrade();
        cx.defer(move |cx| {
            let Some(screen) = shell.upgrade().and_then(|s| s.read(cx).thread_screen()) else { return };
            let view = screen.read(cx).view().clone();
            view.update(cx, |_, cx| cx.emit(ThreadViewEvent::FocusComposer));
        });
    }

    /// Moves `step` threads along `list` from the open one, wrapping.
    pub(super) fn step_thread(&mut self, list: Vec<ThreadItem>, step: isize, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn workspace_threads(&self, cx: &App) -> Vec<ThreadItem> {
        let Route::Thread { workspace_id, .. } = &self.route else { return Vec::new() };
        let sidebar = self.sidebar.read(cx);
        let item = |t: walk::Target| ThreadItem {
            title: sidebar.title_of(&t.thread_id).unwrap_or_default(),
            project: String::new(),
            workspace_id: t.workspace_id,
            thread_id: t.thread_id,
        };
        walk::thread_walk(sidebar, workspace_id).into_iter().map(item).collect()
    }
}
