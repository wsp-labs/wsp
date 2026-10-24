// SPDX-License-Identifier: AGPL-3.0-only
//! A right-click menu as the desktop app pops one (`apps/desktop/src/context-menu.ts`): flat rows, a rule where the
//! group changes, a refused row held with its reason. On the Mac it is an `NSMenu` at the pointer, as Electron's
//! `Menu.popup` is; elsewhere it is the page's own menu (`actions/ContextMenuHost.tsx`) over the window. The pick comes
//! back by its row's index, or none when the menu closes without one.

// The page's own menu is drawn off the Mac alone, so its pieces go unused there.
#![cfg_attr(target_os = "macos", allow(dead_code, unused_imports))]

#[cfg(target_os = "macos")]
mod mac;

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    actions, anchored, canvas, deferred, div, point, prelude::*, px, Anchor, AnyElement, App, Context, DismissEvent, DispatchPhase, Entity,
    EventEmitter, FocusHandle, Focusable, KeyBinding, Keystroke, Pixels, Point, ScrollWheelEvent, SharedString, Size, Subscription,
    TooltipSide, Window,
};

use super::dialog::tip;
use super::text;
use super::theme::ActivePalette;

actions!(page_menu, [Leave, Next, Previous, First, Last, Choose]);

const PAGE_MENU: &str = "PageMenu";

/// The page's menu's keys, bound in its own context so they win over the window's: the shell's Escape would close
/// Settings under it, and the window's Tab would move the keys on.
pub(crate) fn init(cx: &mut App) {
    let at = Some(PAGE_MENU);
    cx.bind_keys([
        KeyBinding::new("escape", Leave, at),
        KeyBinding::new("tab", Leave, at),
        KeyBinding::new("shift-tab", Leave, at),
        KeyBinding::new("down", Next, at),
        KeyBinding::new("up", Previous, at),
        KeyBinding::new("home", First, at),
        KeyBinding::new("end", Last, at),
        KeyBinding::new("enter", Choose, at),
        KeyBinding::new("space", Choose, at),
    ]);
}

pub struct MenuItem {
    pub label: SharedString,
    pub group: &'static str,
    pub refusal: Option<SharedString>,
    pub keys: Option<Keystroke>,
    pub destructive: bool,
}

#[cfg(target_os = "macos")]
type Chose = Box<dyn FnOnce(Option<usize>, &mut Window, &mut App)>;

/// The menu its owner holds: the page's own while one is open, nothing on the Mac, where AppKit draws it; and the rows
/// last asked for, on either.
#[derive(Default)]
pub struct ContextMenu {
    open: Option<Open>,
    asked: Vec<SharedString>,
}

/// The page's menu, where it stands and whether it closed.
struct Open {
    menu: Entity<PageMenu>,
    at: Point<Pixels>,
    above: bool,
    left: bool,
    closed: Rc<Cell<bool>>,
    _dismissed: Subscription,
}

impl ContextMenu {
    /// Opens `items` at `at`, in the window's points; `chose` runs on the owner with the picked row, once.
    pub fn open<V: 'static>(
        &mut self,
        at: Point<Pixels>,
        items: Vec<MenuItem>,
        window: &mut Window,
        cx: &mut Context<V>,
        chose: impl FnOnce(&mut V, Option<usize>, &mut Window, &mut Context<V>) + 'static,
    ) {
        if items.is_empty() {
            return;
        }
        self.open = None;
        self.asked = items.iter().map(|it| it.label.clone()).collect();
        #[cfg(target_os = "macos")]
        {
            let owner = cx.entity().downgrade();
            let chose: Chose = Box::new(move |pick, window, cx| _ = owner.update(cx, |v, cx| chose(v, pick, window, cx)));
            pop(at, items, chose, window, cx);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let picked = Rc::new(Cell::new(None));
            let held = window.focused(cx);
            // The page's menu takes its height from its rows; a menu with no room below opens above the pointer.
            let rules = items.windows(2).filter(|w| w[0].group != w[1].group).count();
            let height = px(items.len() as f32 * ROW + rules as f32 * RULE + PADDING);
            let placed = place_menu(at, gpui_kit::size(px(MENU_WIDTH), height), window.viewport_size());
            let (above, left) = (placed.y < at.y, placed.x < at.x);
            let menu = cx.new(|cx| PageMenu::new(items, picked.clone(), window, cx));
            let closed = Rc::new(Cell::new(false));
            let mut chose = Some(chose);
            let dismissed = cx.subscribe_in(&menu, window, {
                let closed = closed.clone();
                move |_, _, _: &DismissEvent, window, cx| {
                    if closed.replace(true) {
                        return;
                    }
                    cx.notify();
                    if let Some(held) = &held {
                        held.focus(window, cx);
                    }
                    // The keys go back to whoever held them before the pick runs, so a pick that opens a field keeps them.
                    if let Some(chose) = chose.take() {
                        let pick = picked.take();
                        cx.defer_in(window, move |v, window, cx| chose(v, pick, window, cx));
                    }
                }
            });
            // The press that opened it is still being dispatched and may move the keys, so they go to the menu after it.
            let focus = menu.focus_handle(cx);
            window.defer(cx, move |window, cx| focus.focus(window, cx));
            self.open = Some(Open { menu, at, above, left, closed, _dismissed: dismissed });
            cx.notify();
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.as_ref().is_some_and(|m| !m.closed.get())
    }

    /// The labels of the rows the last menu was opened with, whoever draws it.
    pub fn asked(&self) -> &[SharedString] {
        &self.asked
    }

    /// The page's menu over everything else, while one is open.
    pub fn overlay(&self) -> Option<AnyElement> {
        let open = self.open.as_ref().filter(|m| !m.closed.get())?;
        let anchor = match (open.above, open.left) {
            (true, true) => Anchor::BottomRight,
            (true, false) => Anchor::BottomLeft,
            (false, true) => Anchor::TopRight,
            (false, false) => Anchor::TopLeft,
        };
        let placed = anchored()
            .position(open.at)
            .anchor(anchor)
            .snap_to_window_with_margin(px(MENU_MARGIN))
            .child(gpui_kit::div().debug_selector(|| "context-menu".into()).child(open.menu.clone()));
        Some(deferred(placed).with_priority(2).into_any_element())
    }
}

/// Builds the NSMenu and pops it from a task of its own, outside any update: AppKit's tracking loop runs the main
/// queue, and with it GPUI's work, which would find the window borrowed inside one.
#[cfg(target_os = "macos")]
fn pop(at: Point<Pixels>, items: Vec<MenuItem>, chose: Chose, window: &mut Window, cx: &mut App) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let view = match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
        Ok(RawWindowHandle::AppKit(h)) => h.ns_view.as_ptr() as cocoa::base::id,
        _ => return,
    };
    let rows: Vec<mac::Row> = items
        .iter()
        .map(|it| mac::Row {
            label: it.label.to_string(),
            group: it.group,
            enabled: it.refusal.is_none(),
            tooltip: it.refusal.as_ref().map(|r| r.to_string()),
            key: it.keys.as_ref().and_then(key_equivalent),
        })
        .collect();
    let at = (f64::from(at.x), f64::from(at.y));
    window
        .spawn(cx, async move |cx| {
            // SAFETY: GPUI's own view of this window, on the main thread, in a task no update holds.
            let (target, chosen) = unsafe { mac::pop(view, at, &rows) };
            // Electron reads a close as no pick a turn later, since AppKit can send the pick after the close.
            if chosen && target.picked().is_none() {
                cx.background_executor().timer(std::time::Duration::ZERO).await;
            }
            let pick = target.picked();
            drop(target);
            _ = cx.update(|window, cx| chose(pick, window, cx));
        })
        .detach();
}

/// An item's key equivalent as `gpui-pre-apple`'s menu bar writes one: the key's characters and the modifier mask.
#[cfg(target_os = "macos")]
fn key_equivalent(k: &Keystroke) -> Option<(String, u64)> {
    let key = mac::arrow(&k.key).map_or_else(|| k.key.to_lowercase(), String::from);
    let m = &k.modifiers;
    let mask = [(m.platform, mac::COMMAND), (m.control, mac::CONTROL), (m.alt, mac::OPTION), (m.shift, mac::SHIFT)]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(0, |mask, (_, bit)| mask | bit);
    Some((key, mask))
}

/// `formatShortcutLabel` in `keybindings.ts`: `⇧⌘E` on the Mac, `Ctrl+Shift+E` elsewhere.
pub fn keys_label(k: &Keystroke, mac: bool) -> String {
    let m = &k.modifiers;
    let key = match k.key.as_str() {
        "space" => "Space".to_owned(),
        "escape" => "Esc".to_owned(),
        other => {
            let mut chars = other.chars();
            chars.next().map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
        }
    };
    if mac {
        let marks = [(m.control, "⌃"), (m.alt, "⌥"), (m.shift, "⇧"), (m.platform, "⌘")];
        return marks.into_iter().filter(|(on, _)| *on).map(|(_, mark)| mark).collect::<String>() + &key;
    }
    let words = [(m.control, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift"), (m.platform, "Meta")];
    let mut out: Vec<String> = words.into_iter().filter(|(on, _)| *on).map(|(_, w)| w.to_owned()).collect();
    out.push(key);
    out.join("+")
}

/// `InAppContextMenu`: one row per item with its chord at the right, a rule where the group changes, a refused row
/// dimmed with its refusal on its hover. The keys light one row, from the first that runs: arrows walk every row and
/// wrap, Home and End go to the ends, Enter and Space run the lit row, Escape and Tab close, as a press outside, a
/// scroll, a resize of the window and the window losing the keys do.
struct PageMenu {
    rows: Vec<MenuItem>,
    lit: usize,
    picked: Rc<Cell<Option<usize>>>,
    focus: FocusHandle,
    /// The window's size and activation watched while the menu is open; dropped as it closes.
    window: Option<[Subscription; 2]>,
}

impl PageMenu {
    fn new(rows: Vec<MenuItem>, picked: Rc<Cell<Option<usize>>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let lit = rows.iter().position(|r| r.refusal.is_none()).unwrap_or(0);
        // The window's bounds move with it too; only a change of its size closes the menu, as the page's `resize` does.
        let viewport = window.viewport_size();
        let resized = cx.observe_window_bounds(window, move |menu, window, cx| {
            if window.viewport_size() != viewport {
                menu.close(cx);
            }
        });
        let blurred = cx.observe_window_activation(window, |menu, window, cx| {
            if !window.is_window_active() {
                menu.close(cx);
            }
        });
        PageMenu { rows, lit, picked, focus: cx.focus_handle(), window: Some([resized, blurred]) }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.window.take().is_some() {
            cx.emit(DismissEvent);
        }
    }

    fn choose(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.rows.get(ix).is_some_and(|r| r.refusal.is_none()) {
            self.picked.set(Some(ix));
            self.close(cx);
        }
    }

    fn light(&mut self, lit: usize, cx: &mut Context<Self>) {
        self.lit = lit;
        cx.notify();
    }
}

impl EventEmitter<DismissEvent> for PageMenu {}

impl Focusable for PageMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PageMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let rows = self.rows.iter().enumerate().flat_map(|(ix, row)| {
            let rule =
                (ix > 0 && self.rows[ix - 1].group != row.group).then(|| div().mx_2().my_1().h(px(1.)).bg(p.border).into_any_element());
            let (lit, runs) = (ix == self.lit, row.refusal.is_none());
            let ink = if row.destructive { p.error_foreground } else { p.sidebar_hover_ink };
            let line = h_flex()
                .id(("page-menu-row", ix))
                .debug_selector(move || format!("page-menu-row-{ix}"))
                .w_full()
                .min_h(px(28.))
                .gap_3()
                .px_2()
                .py_1()
                .rounded(px(6.))
                .when(runs, |el| el.hover(move |s| s.bg(p.accent).text_color(ink)).when(lit, |el| el.bg(p.accent).text_color(ink)))
                .when(!runs, |el| el.opacity(0.5).when(lit, |el| el.bg(p.accent.opacity(0.5))))
                .on_click(cx.listener(move |menu, _, _, cx| menu.choose(ix, cx)))
                .child(div().debug_selector(move || format!("page-menu-label-{ix}")).flex_1().min_w_0().truncate().child(row.label.clone()))
                .children(
                    row.keys
                        .as_ref()
                        .map(|k| div().flex_none().font_family(mono.clone()).text_color(p.muted_foreground).child(keys_label(k, false))),
                );
            let line = match row.refusal.clone() {
                Some(why) => line.tooltip(tip(why)).tooltip_side(TooltipSide::Right).into_any_element(),
                None => line.into_any_element(),
            };
            rule.into_iter().chain([line])
        });
        // A scroll of the page under it closes the menu, as the page's `scroll` listener in the capture phase does; the
        // menu itself does not scroll.
        let menu = cx.weak_entity();
        let scrolled = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window.on_mouse_event(move |e: &ScrollWheelEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && !bounds.contains(&e.position) {
                        _ = menu.update(cx, |menu, cx| menu.close(cx));
                    }
                })
            },
        )
        .absolute()
        .size_full();
        v_flex()
            .id("page-menu")
            .key_context(PAGE_MENU)
            .track_focus(&self.focus)
            .on_action(cx.listener(|menu, _: &Leave, _, cx| menu.close(cx)))
            .on_action(cx.listener(|menu, _: &Next, _, cx| menu.light((menu.lit + 1) % menu.rows.len(), cx)))
            .on_action(cx.listener(|menu, _: &Previous, _, cx| menu.light((menu.lit + menu.rows.len() - 1) % menu.rows.len(), cx)))
            .on_action(cx.listener(|menu, _: &First, _, cx| menu.light(0, cx)))
            .on_action(cx.listener(|menu, _: &Last, _, cx| menu.light(menu.rows.len() - 1, cx)))
            .on_action(cx.listener(|menu, _: &Choose, _, cx| menu.choose(menu.lit, cx)))
            .on_mouse_down_out(cx.listener(|menu, _, _, cx| menu.close(cx)))
            .occlude()
            .min_w(px(MENU_MIN_WIDTH))
            .p_1()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .bg(p.popover)
            .shadow(p.tooltip_shadow.clone())
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .text_color(p.foreground)
            .child(scrolled)
            .children(rows)
    }
}

/// The width the placement reckons with: the menu's rows fit it in the words the sidebar's menus hold.
const MENU_WIDTH: f32 = 200.;
/// `min-w-44`, the web menu's floor.
const MENU_MIN_WIDTH: f32 = 176.;
const MENU_MARGIN: f32 = 8.;
/// The menu's row (`min-h-7`), its rule with the space round it (`my-1`), and its padding and edge, as the height is
/// reckoned.
const ROW: f32 = 28.;
const RULE: f32 = 9.;
const PADDING: f32 = 10.;

/// `actions/menuPlacement.ts` placeMenu: at the pointer, flipped to its other side where that edge of the window is
/// too near, never past the margin.
pub fn place_menu(pointer: Point<Pixels>, menu: Size<Pixels>, viewport: Size<Pixels>) -> Point<Pixels> {
    let (x, y, w, h) = (f32::from(pointer.x), f32::from(pointer.y), f32::from(menu.width), f32::from(menu.height));
    let (vw, vh) = (f32::from(viewport.width), f32::from(viewport.height));
    let left = if x + w <= vw - MENU_MARGIN { x } else { x - w };
    let top = if y + h <= vh - MENU_MARGIN { y } else { y - h };
    point(px(left.min(vw - w - MENU_MARGIN).max(MENU_MARGIN)), px(top.min(vh - h - MENU_MARGIN).max(MENU_MARGIN)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{div, FocusHandle, Modifiers, TestAppContext, VisualTestContext};

    #[derive(Default)]
    struct Owner {
        menu: ContextMenu,
        picks: Vec<Option<usize>>,
        focus: Option<FocusHandle>,
    }

    impl Render for Owner {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let focus = self.focus.get_or_insert_with(|| cx.focus_handle()).clone();
            div().size_full().track_focus(&focus).children(self.menu.overlay())
        }
    }

    fn item(label: &'static str, group: &'static str, refusal: Option<&'static str>) -> MenuItem {
        MenuItem { label: label.into(), group, refusal: refusal.map(Into::into), keys: None, destructive: false }
    }

    fn opened(cx: &mut TestAppContext) -> (Entity<Owner>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let owner = cx.new(|_| Owner::default());
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(owner.clone(), window, cx));
        cx.simulate_resize(gpui_kit::size(px(1280.), px(800.)));
        cx.run_until_parked();
        (owner, cx)
    }

    fn open(owner: &Entity<Owner>, cx: &mut VisualTestContext) {
        owner.update_in(cx, |o, window, cx| {
            let focus = o.focus.clone().unwrap();
            window.focus(&focus, cx);
            let items = vec![item("Refused", "a", Some("not now")), item("Pin", "a", None), item("Delete", "b", None)];
            o.menu.open(point(px(100.), px(100.)), items, window, cx, |o, pick, _, _| o.picks.push(pick));
        });
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn a_pick_comes_back_by_its_index_and_a_close_as_none(cx: &mut TestAppContext) {
        let (owner, cx) = opened(cx);
        open(&owner, cx);
        assert_eq!(
            owner.read_with(cx, |o, _| o.menu.asked().to_vec()),
            ["Refused", "Pin", "Delete"],
            "the menu is asked for with its rows"
        );
        // On the Mac AppKit draws the menu, which the test platform cannot drive.
        if cfg!(target_os = "macos") {
            return;
        }
        assert!(owner.read_with(cx, |o, _| o.menu.is_open()), "the menu is open over the window");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(owner.read_with(cx, |o, _| o.picks.clone()), [Some(1)], "Enter picks the lit row, Pin, the first that runs");
        assert!(owner.read_with(cx, |o, _| !o.menu.is_open()));
        let held = owner.read_with(cx, |o, _| o.focus.clone().unwrap());
        assert!(cx.update(|window, _| held.is_focused(window)), "the keys go back to whoever held them");

        open(&owner, cx);
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        assert!(owner.read_with(cx, |o, _| o.menu.is_open()), "the arrows wrap onto the refused row, where Enter runs nothing");
        cx.simulate_keystrokes("end space");
        cx.run_until_parked();
        open(&owner, cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        open(&owner, cx);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        open(&owner, cx);
        cx.simulate_click(point(px(900.), px(700.)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            owner.read_with(cx, |o, _| o.picks.clone()),
            [Some(1), Some(2), None, None, None],
            "End then Space picks Delete; Escape, Tab and a press outside close it"
        );
    }

    /// `InAppContextMenu` closes with no pick on a scroll, a resize of the window and the window's blur.
    #[cfg(not(target_os = "macos"))]
    fn closes_on(close: impl FnOnce(&mut VisualTestContext), cx: &mut TestAppContext) -> (bool, Vec<Option<usize>>) {
        let (owner, cx) = opened(cx);
        // A test window starts with nobody's keys; it takes them first, so there is a blur to have.
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        open(&owner, cx);
        assert!(owner.read_with(cx, |o, _| o.menu.is_open()), "the menu is open over the window");
        close(cx);
        cx.run_until_parked();
        owner.read_with(cx, |o, _| (o.menu.is_open(), o.picks.clone()))
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn a_scroll_closes_the_menu(cx: &mut TestAppContext) {
        let shut = closes_on(
            |cx| {
                cx.simulate_event(gpui_kit::ScrollWheelEvent {
                    position: point(px(900.), px(700.)),
                    delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-40.))),
                    modifiers: Modifiers::none(),
                    touch_phase: gpui_kit::TouchPhase::Moved,
                })
            },
            cx,
        );
        assert_eq!(shut, (false, vec![None]), "a scroll away from the menu closes it with no pick");
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn a_resize_closes_the_menu(cx: &mut TestAppContext) {
        let shut = closes_on(|cx| cx.simulate_resize(gpui_kit::size(px(1000.), px(700.))), cx);
        assert_eq!(shut, (false, vec![None]), "a resize of the window closes it with no pick");
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn the_windows_blur_closes_the_menu(cx: &mut TestAppContext) {
        let shut = closes_on(|cx| cx.deactivate_window(), cx);
        assert_eq!(shut, (false, vec![None]), "the window losing the keys closes it with no pick");
    }

    /// The page's `scroll` comes from what scrolls; the menu itself does not, so a wheel over it leaves it open.
    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn a_scroll_over_the_menu_leaves_it_open(cx: &mut TestAppContext) {
        let (owner, cx) = opened(cx);
        open(&owner, cx);
        let over = cx.debug_bounds("page-menu-row-1").expect("the menu is drawn").center();
        cx.simulate_event(gpui_kit::ScrollWheelEvent {
            position: over,
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-40.))),
            modifiers: Modifiers::none(),
            touch_phase: gpui_kit::TouchPhase::Moved,
        });
        cx.run_until_parked();
        assert!(owner.read_with(cx, |o, _| o.menu.is_open()), "a wheel over the menu leaves it open");
    }

    /// Once closed the menu no longer watches the window: a resize after Escape tells nobody it closed again.
    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn a_closed_menu_stops_watching_the_window(cx: &mut TestAppContext) {
        let (owner, cx) = opened(cx);
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        open(&owner, cx);
        let menu = page_menu(&owner, cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(owner.read_with(cx, |o, _| !o.menu.is_open()), "Escape closed the menu");
        let told = Rc::new(Cell::new(0));
        let _counting = cx.update(|_, cx| {
            let told = told.clone();
            cx.subscribe(&menu, move |_, _: &DismissEvent, _| told.set(told.get() + 1))
        });
        cx.simulate_resize(gpui_kit::size(px(1000.), px(700.)));
        cx.deactivate_window();
        cx.run_until_parked();
        assert_eq!(told.get(), 0, "neither the resize nor the blur reached the closed menu");
    }

    /// The menu the owner holds open.
    #[cfg(not(target_os = "macos"))]
    fn page_menu(owner: &Entity<Owner>, cx: &mut VisualTestContext) -> Entity<PageMenu> {
        owner.read_with(cx, |o, _| o.menu.open.as_ref().expect("a menu is open").menu.clone())
    }

    /// `InAppContextMenu`'s focus starts on the first enabled row, and the arrows walk every row, the refused one too.
    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn the_first_row_that_runs_is_lit_as_the_menu_opens(cx: &mut TestAppContext) {
        let (owner, cx) = opened(cx);
        open(&owner, cx);
        let lit = |cx: &mut VisualTestContext| page_menu(&owner, cx).read_with(cx, |m, _| m.lit);
        assert_eq!(lit(cx), 1, "Pin, past the refused row, is lit as the menu opens");
        let mut walked = Vec::new();
        for key in ["down", "down", "down", "up", "end", "home"] {
            cx.simulate_keystrokes(key);
            walked.push(lit(cx));
        }
        assert_eq!(walked, [2, 0, 1, 0, 2, 0], "down wraps onto the refused row, up wraps back, End and Home go to the ends");
    }

    /// `ROW_CLASS`'s `min-h-7` rows of 12 px words on 16, `py-1` inside a `p-1` menu with a 1 px edge, `my-1` round a rule.
    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn the_rows_are_the_page_menus_size(cx: &mut TestAppContext) {
        let (owner, cx) = opened(cx);
        open(&owner, cx);
        let row =
            |ix: usize, cx: &mut VisualTestContext| cx.debug_bounds(Box::leak(format!("page-menu-row-{ix}").into_boxed_str())).unwrap();
        let (first, second, third) = (row(0, cx), row(1, cx), row(2, cx));
        assert_eq!((first.size.height, second.size.height, third.size.height), (px(28.), px(28.), px(28.)), "min-h-7 rows");
        assert_eq!(second.top(), first.bottom(), "rows of one group stand flush");
        assert_eq!(third.top() - second.bottom(), px(9.), "a rule with 4 px above and below it between groups");
        let menu = cx.debug_bounds("context-menu").expect("the menu is drawn");
        assert_eq!(first.top() - menu.top(), px(5.), "the 1 px edge and the 4 px padding");
        assert!(menu.size.width >= px(176.), "min-w-44: {menu:?}");
        let words = cx.debug_bounds("page-menu-label-1").unwrap();
        assert_eq!(words.size.height, px(16.), "text-xs words on a 16 px line");
    }

    // The Mac opens the native menu, whose place the system picks; the popup measured here is the other platforms'.
    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn a_menu_with_no_room_right_of_the_pointer_opens_to_its_left(cx: &mut TestAppContext) {
        let (owner, cx) = opened(cx);
        owner.update_in(cx, |o, window, cx| {
            let items = vec![item("Open thread", "go", None), item("Copy PID", "copy", None)];
            o.menu.open(point(px(1200.), px(100.)), items, window, cx, |o, pick, _, _| o.picks.push(pick));
        });
        cx.run_until_parked();
        let menu = cx.debug_bounds("context-menu").expect("the menu is drawn");
        assert!(menu.right() <= px(1201.) && menu.left() < px(1100.), "the menu ends at the pointer: {menu:?}");
        assert!(menu.top() >= px(99.), "with room below it still opens down: {menu:?}");
    }

    #[test]
    fn the_menu_flips_and_stays_inside_the_margin() {
        let view = gpui_kit::size(px(1280.), px(800.));
        let menu = gpui_kit::size(px(200.), px(613.));
        assert_eq!(place_menu(point(px(120.), px(100.)), menu, view), point(px(120.), px(100.)));
        assert_eq!(place_menu(point(px(120.), px(491.)), menu, view), point(px(120.), px(8.)), "flipped up, then held at the margin");
        assert_eq!(place_menu(point(px(1200.), px(100.)), menu, view), point(px(1000.), px(100.)));
    }

    #[test]
    fn a_chord_reads_as_the_page_labels_it() {
        let label = |chord: &str, mac| keys_label(&Keystroke::parse(chord).unwrap(), mac);
        assert_eq!(label("ctrl-shift-e", false), "Ctrl+Shift+E");
        assert_eq!(label("alt-shift-up", false), "Alt+Shift+Up");
        assert_eq!(label("ctrl-j", false), "Ctrl+J");
        assert_eq!(label("escape", false), "Esc");
        assert_eq!(label("cmd-shift-e", true), "⇧⌘E");
        assert_eq!(label("ctrl-alt-space", true), "⌃⌥Space");
    }
}
