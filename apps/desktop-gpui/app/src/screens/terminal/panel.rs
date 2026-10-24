// SPDX-License-Identifier: AGPL-3.0-only
//! The terminal as a right panel tab (`WorkspaceTerminalPanel.tsx`): one surface of up to four shells, found again by
//! its tab's id after a thread switch, and ended only when the tab closes for good.

use std::rc::Rc;

use gpui_kit::{prelude::*, App, Context, Entity, SharedString, Window};

use super::drawer::{Drawer, Mode};
use super::hub::{self, Key};
use super::link::Link;
use crate::screens::{Host, Opened, Pane, Parts, Place, Screen};

pub struct TerminalScreen {
    drawer: Entity<Drawer>,
}

fn key(opened: &Opened) -> Key {
    opened.workspace_id.clone().map_or(Key::Here, Key::Workspace)
}

fn tab(opened: &Opened) -> SharedString {
    opened.tab.clone().unwrap_or_else(|| SharedString::new_static(TerminalScreen::NAME))
}

impl TerminalScreen {
    pub fn drawer(&self) -> &Entity<Drawer> {
        &self.drawer
    }

    /// `panes.ts`: the active shell's name, else "Terminal".
    fn title(&self, cx: &App) -> SharedString {
        let drawer = self.drawer.read(cx);
        let Mode::Panel { tab, .. } = drawer.mode() else { return "Terminal".into() };
        match hub::surface_read(tab, cx).map(|ui| ui.active_terminal_id).filter(|id| !id.is_empty()) {
            Some(active) => drawer.label(&active, cx),
            None => "Terminal".into(),
        }
    }
}

impl Screen for TerminalScreen {
    const NAME: &'static str = "terminal";
    const PLACE: Place = Place::Panel(Pane::Terminal);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = Mode::Panel { key: key(&opened), tab: tab(&opened) };
        TerminalScreen { drawer: cx.new(|cx| Drawer::new(host, mode, window, cx)) }
    }

    /// The tab closed for good: its shells end with it.
    fn closed(host: &Host, opened: Opened, cx: &mut App) {
        let Some(surface) = hub::drop_surface(&tab(&opened), cx) else { return };
        let link = Link::get(host, key(&opened).target(), cx);
        link.update(cx, |link, cx| {
            for pty in &surface.terminal_ids {
                link.kill(pty, cx);
            }
        });
    }

    fn parts(this: &Entity<Self>, _window: &mut Window, _cx: &mut App) -> Parts {
        let this = this.downgrade();
        Parts {
            title: Some(Rc::new(move |cx: &App| this.upgrade().map_or_else(|| "Terminal".into(), |s| s.read(cx).title(cx)))),
            ..Parts::default()
        }
    }
}

impl Render for TerminalScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.drawer.clone()
    }
}
