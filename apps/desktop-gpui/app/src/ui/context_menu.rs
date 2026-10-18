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

use gpui_kit::component::menu::PopupMenu;
use gpui_kit::{
    anchored, deferred, point, prelude::*, px, Anchor, AnyElement, Context, DismissEvent, Entity, Focusable, Keystroke, Pixels, Point,
    SharedString, Size, Subscription, Window,
};

#[cfg(target_os = "macos")]
use gpui_kit::App;

use super::menu::{menu_of, MenuAct};
use super::theme::ActivePalette;

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

/// The page's menu, gpui-kit's popup filled as a row's menu is (`ui::menu`), where it stands and whether it closed.
struct Open {
    menu: Entity<PopupMenu>,
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
            let mut group = 0;
            let acts: Vec<MenuAct> = items
                .iter()
                .enumerate()
                .map(|(ix, it)| {
                    group += u8::from(ix > 0 && items[ix - 1].group != it.group);
                    let picked = picked.clone();
                    MenuAct {
                        group,
                        glyph: None,
                        title: it.label.clone(),
                        destructive: it.destructive,
                        held: it.refusal.is_some(),
                        why: it.refusal.clone(),
                        keys: it.keys.as_ref().map(|k| keys_label(k, false).into()),
                        run: Rc::new(move |_, _| picked.set(Some(ix))),
                    }
                })
                .collect();
            let (danger, muted) = (cx.palette().error_foreground, cx.palette().muted_foreground);
            let held = window.focused(cx);
            let menu = PopupMenu::build(window, cx, move |menu, _, _| {
                let menu = menu_of(menu.min_w(px(MENU_MIN_WIDTH)), &acts, danger, muted);
                match held {
                    Some(held) => menu.action_context(held),
                    None => menu,
                }
            });
            // gpui-kit's menu takes its height from its rows; a menu with no room below opens above the pointer.
            let rules = items.windows(2).filter(|w| w[0].group != w[1].group).count();
            let height = px(items.len() as f32 * ROW + rules as f32 * RULE + PADDING);
            let placed = place_menu(at, gpui_kit::size(px(MENU_WIDTH), height), window.viewport_size());
            let (above, left) = (placed.y < at.y, placed.x < at.x);
            let closed = Rc::new(Cell::new(false));
            let mut chose = Some(chose);
            let dismissed = cx.subscribe_in(&menu, window, {
                let closed = closed.clone();
                move |_, _, _: &DismissEvent, window, cx| {
                    closed.set(true);
                    cx.notify();
                    // The menu hands the keys back as it closes, after this; the pick runs once it has, so a pick
                    // that opens a field keeps them.
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

/// The width the placement reckons with: the menu's rows fit it in the words the sidebar's menus hold.
const MENU_WIDTH: f32 = 200.;
/// `min-w-44`, the web menu's floor.
const MENU_MIN_WIDTH: f32 = 176.;
const MENU_MARGIN: f32 = 8.;
/// gpui-kit's popup row, its rule with the space round it, and its padding and edge, as the height is reckoned.
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
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        assert_eq!(owner.read_with(cx, |o, _| o.picks.clone()), [Some(1)], "the second Down passes the refused row and Enter picks Pin");
        assert!(owner.read_with(cx, |o, _| !o.menu.is_open()));
        let held = owner.read_with(cx, |o, _| o.focus.clone().unwrap());
        assert!(cx.update(|window, _| held.is_focused(window)), "the keys go back to whoever held them");

        open(&owner, cx);
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        open(&owner, cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        open(&owner, cx);
        cx.simulate_click(point(px(900.), px(700.)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            owner.read_with(cx, |o, _| o.picks.clone()),
            [Some(1), None, None, None],
            "Enter on the refused row picks nothing; Escape and a press outside close it"
        );
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
