// SPDX-License-Identifier: AGPL-3.0-only
//! The pieces of Settings the app's frame lays in its own places, and a page's dialog on the dialogs layer.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::{
    canvas, div, prelude::*, AnyElement, App, Context, Entity, FocusHandle, Pixels, StyleRefinement, Subscription, WeakEntity, Window,
};

use super::model::{At, GroupId};
use super::{frame, page, Acts, Ctx, SettingsScreen};
use crate::screens::dialogs::layer;

/// A piece of Settings the app's frame lays in its own place: the page list in its sidebar column under its frame row,
/// the crumbs in its header row, and Restore defaults or a group's tabs between them and the panel toggles.
#[derive(Clone, Copy, PartialEq)]
pub enum Part {
    Sidebar,
    Crumbs,
    Actions,
}

/// One of a framed screen's parts as a view of its own, drawn from the screen and again whenever the screen moves.
pub struct SettingsPart {
    screen: Entity<SettingsScreen>,
    part: Part,
    actions: Option<Entity<Actions>>,
    _moved: Subscription,
}

impl SettingsPart {
    pub fn new(screen: &Entity<SettingsScreen>, part: Part, cx: &mut Context<Self>) -> Self {
        let actions = (part == Part::Actions).then(|| cx.new(|cx| Actions::new(screen, cx)));
        SettingsPart { screen: screen.clone(), part, actions, _moved: cx.observe(screen, |_, _, cx| cx.notify()) }
    }
}

/// The Actions part as a view of its own, drawn cached at the width it was last laid out at while nothing it draws
/// moved, so a control easing its ink (Activity's view picker) draws itself alone and not the shell's header row.
pub struct Actions {
    screen: Entity<SettingsScreen>,
    width: Rc<Cell<Pixels>>,
    moved: Rc<Cell<bool>>,
    viewport: Cell<Pixels>,
    /// Whether it was last placed cached, in a box of the width it measured before.
    boxed: Rc<Cell<bool>>,
    _moved: Subscription,
}

impl Actions {
    fn new(screen: &Entity<SettingsScreen>, cx: &mut Context<Self>) -> Self {
        let moved = Rc::new(Cell::new(true));
        let observed = moved.clone();
        let _moved = cx.observe(screen, move |_, _, cx| {
            observed.set(true);
            cx.notify();
        });
        Actions {
            screen: screen.clone(),
            width: Rc::new(Cell::new(Pixels::ZERO)),
            moved,
            viewport: Cell::new(Pixels::ZERO),
            boxed: Rc::new(Cell::new(false)),
            _moved,
        }
    }

    /// Cached at its last width, or whole to be measured again after a change or at a new window width.
    fn drawn(actions: &Entity<Actions>, window: &Window, cx: &mut App) -> AnyElement {
        let viewport = window.viewport_size().width;
        let width = actions.update(cx, |a, _| {
            let moved = a.moved.replace(false) | (a.viewport.replace(viewport) != viewport);
            let width = Some(a.width.get()).filter(|w| *w > Pixels::ZERO && !moved);
            a.boxed.set(width.is_some());
            width
        });
        match width {
            Some(width) => actions.clone().cached(StyleRefinement::default().flex_none().h_full().w(width)).into_any_element(),
            None => actions.clone().into_any_element(),
        }
    }
}

impl Render for Actions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("SettingsActions");
        let weak = self.screen.downgrade();
        let content = self.screen.update(cx, |s, cx| {
            let at = s.resolved();
            match s.usage.as_ref().filter(|_| s.search.is_empty() && at == At::Group(GroupId::Usage)) {
                Some((page, _)) => page.update(cx, |page, cx| page.tabs(cx).into_any_element()),
                None => {
                    let acts = Acts::new(weak, s.host.clone(), s.runtime.clone());
                    let ctx =
                        Ctx::new(&s.data, &at, s.level.as_ref(), acts, s.activity.clone(), s.agents.clone(), s.computers_menu.clone());
                    frame::actions(&ctx, window, cx).unwrap_or_else(|| div().into_any_element())
                }
            }
        });
        let (width, moved, boxed) = (self.width.clone(), self.moved.clone(), self.boxed.clone());
        // Drawn again in its box at a width it no longer has (a hover that shows words), it asks to be measured whole.
        let measure = move |bounds: gpui_kit::Bounds<Pixels>, window: &mut Window, _: &mut App| {
            if boxed.get() && (width.get() - bounds.size.width).abs() > gpui_kit::px(0.5) {
                moved.set(true);
                window.request_animation_frame();
            }
            width.set(bounds.size.width);
        };
        div()
            .flex()
            .flex_none()
            .h_full()
            .items_center()
            .child(div().relative().flex_none().child(content).child(canvas(measure, |_, _, _, _| {}).absolute().size_full()))
    }
}

impl Render for SettingsPart {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("SettingsPart");
        if let Some(actions) = &self.actions {
            return Actions::drawn(actions, window, cx);
        }
        let part = self.part;
        let weak = self.screen.downgrade();
        self.screen.update(cx, |s, cx| {
            let at = s.resolved();
            let acts = Acts::new(weak.clone(), s.host.clone(), s.runtime.clone());
            let ctx = Ctx::new(&s.data, &at, s.level.as_ref(), acts, s.activity.clone(), s.agents.clone(), s.computers_menu.clone());
            match part {
                Part::Sidebar => {
                    let (hits, results) = (s.drawn.hits.as_deref(), s.drawn.results.as_deref());
                    let height = window.viewport_size().height;
                    frame::sidebar(&ctx, &s.search_field, hits, results, &s.drawn.trail, false, height, window, cx).into_any_element()
                }
                Part::Crumbs => frame::crumbs(&ctx, !s.search.is_empty() && !crate::ui::narrow::width(window).md, cx).into_any_element(),
                Part::Actions => div().into_any_element(),
            }
        })
    }
}

/// The layer's name for the dialog a Settings page or Usage stands on the window, one at a time.
pub const DIALOG: &str = "settings-dialog";

/// A page's dialog as a view on the dialogs layer, built each frame from the screen's records and drawn again whenever
/// the screen moves.
pub(super) struct SettingsDialog {
    screen: WeakEntity<SettingsScreen>,
    build: page::Build,
    /// Held around what it builds, so focus can be moved to its first tab stop as Base UI's popups take it.
    focus: FocusHandle,
    focused: bool,
    _moved: Subscription,
}

impl SettingsDialog {
    pub(super) fn new(screen: &Entity<SettingsScreen>, build: page::Build, cx: &mut Context<Self>) -> Self {
        let _moved = cx.observe(screen, |_, _, cx| cx.notify());
        SettingsDialog { screen: screen.downgrade(), build, focus: cx.focus_handle(), focused: false, _moved }
    }
}

impl Render for SettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(screen) = self.screen.upgrade() else {
            window.defer(cx, |window, cx| layer::close(DIALOG, window, cx));
            return gpui_kit::div().into_any_element();
        };
        if !std::mem::replace(&mut self.focused, true) {
            let focus = self.focus.clone();
            // Its tab stops are known once a frame has drawn it; a field it focused itself keeps the focus.
            window.on_next_frame(move |window, cx| {
                if focus.contains_focused(window, cx) {
                    return;
                }
                focus.focus(window, cx);
                window.focus_next(cx);
                if !focus.contains_focused(window, cx) {
                    focus.focus(window, cx);
                }
            });
        }
        let (build, weak) = (self.build.clone(), self.screen.clone());
        let built = screen.update(cx, |s, cx| {
            let at = s.resolved();
            let acts = Acts::new(weak, s.host.clone(), s.runtime.clone());
            let ctx = Ctx::new(&s.data, &at, s.level.as_ref(), acts, s.activity.clone(), s.agents.clone(), s.computers_menu.clone());
            build(&ctx, window, cx)
        });
        gpui_kit::div().track_focus(&self.focus).child(built).into_any_element()
    }
}
