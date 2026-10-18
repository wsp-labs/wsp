// SPDX-License-Identifier: AGPL-3.0-only
//! The layers drawn over the whole window, as one of gpui-kit's `Root` plugins: the command palette and the thread
//! switcher, over the toasts `ui::init` registers before them, as the web keeps its toasts under its dialogs. Each layer
//! is built when it opens and dropped when it closes, so a shut palette holds nothing. The host's notices follow the
//! host here once the shell is connected.

use gpui_kit::base::{Root, RootPlugin};
use gpui_kit::{div, prelude::*, App, Context, Entity, FocusHandle, WeakEntity, Window};

use super::palette::Palette;
use super::switcher::Switcher;
use crate::screens::notices::Notices;
use crate::screens::Host;
use crate::sidebar::Sidebar;

pub struct Overlays {
    /// The connected host, once the shell has one.
    pub host: Option<Host>,
    /// The sidebar, whose projects and threads the palette lists and whose acts its rows run.
    pub sidebar: Option<WeakEntity<Sidebar>>,
    pub palette: Option<Entity<Palette>>,
    pub switcher: Option<Entity<Switcher>>,
    /// The host's notices, which raise their toasts in the window's stack.
    pub notices: Entity<Notices>,
    /// What held focus before a layer took it, given back when the layer closes.
    pub behind: Option<FocusHandle>,
}

/// Registers the layers for every window opened after it; call once after `gpui_kit::init`, before the first window.
pub fn init(cx: &mut App) {
    Root::register_plugin::<Overlays>(cx, Overlays::new);
    crate::screens::notices::init(cx);
}

/// The window's layers, where its root was built after [`init`].
pub fn of(window: &Window, cx: &App) -> Option<Entity<Overlays>> {
    window.root::<Root>().flatten()?.read(cx).plugin::<Overlays>()
}

/// Hands the layers the host and the sidebar once the shell is connected.
pub fn attach(host: Host, sidebar: &Entity<Sidebar>, window: &mut Window, cx: &mut App) {
    let Some(overlays) = of(window, cx) else { return };
    let sidebar = sidebar.downgrade();
    overlays.update(cx, |o, cx| {
        o.host = Some(host.clone());
        o.sidebar = Some(sidebar);
        o.notices.update(cx, |n, cx| n.attach(host, window, cx));
        cx.notify();
    });
    super::switcher::follow(window, cx);
}

impl Overlays {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let notices = cx.new(|cx| Notices::new(window, cx));
        Overlays { host: None, sidebar: None, palette: None, switcher: None, notices, behind: None }
    }

    /// Notes what holds focus now, for [`Overlays::give_back`]; a layer opening over another keeps the first one's.
    pub fn take_focus(&mut self, window: &Window, cx: &App) {
        if self.palette.is_none() && self.switcher.is_none() {
            self.behind = window.focused(cx);
        }
    }

    /// Puts focus back where it stood before the layers took it.
    pub fn give_back(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(behind) = self.behind.take() {
            window.focus(&behind, cx);
        }
    }
}

impl RootPlugin for Overlays {}

impl Render for Overlays {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().absolute().top_0().left_0().size_full().children(self.palette.clone()).children(self.switcher.clone())
    }
}
