// SPDX-License-Identifier: AGPL-3.0-only
//! The browser pane's More menu (`PreviewMoreMenu.tsx`): Hard reload, Copy URL, Open in system browser and New tab,
//! a rule, then the Zoom row, whose buttons keep the menu open. Whether it is open is the screen's.

use std::sync::Arc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, Anchor, AnchoredPositionMode, App, ClickEvent, Div, FocusHandle, FontFeatures, Hsla,
    KeyDownEvent, Pixels, Point, Stateful, Window,
};

use super::bars::percent;
use super::chrome::{Act, IconKey};
use crate::ui::menu::glass;
use crate::ui::{ActivePalette, Button, Glyph};

/// `min-w-56`.
pub const MENU_WIDTH: Pixels = px(224.);

/// The More menu: its button, and while `open` its popup aligned to the button's end, 6 px below. The page rows and
/// the Zoom row are held while the tab has no page.
#[derive(IntoElement)]
pub struct MoreMenu {
    pub open: bool,
    pub has_page: bool,
    pub zoom: f32,
    pub on_toggle: Act,
    pub on_close: Act,
    pub on_hard_reload: Act,
    pub on_copy: Act,
    pub on_open_outside: Act,
    pub on_new_tab: Act,
    pub on_zoom_in: Act,
    pub on_zoom_out: Act,
    pub on_zoom_reset: Act,
}

/// `MenuItem`: 28 px at the least, 8 by 4 px, the accent ground under the pointer, held at 64%.
fn row(selector: &'static str, held: bool, accent: Hsla) -> Stateful<Div> {
    h_flex().id(selector).debug_selector(move || selector.into()).min_h(px(28.)).px_2().py_1().gap_2().rounded(px(8.)).map(|el| {
        if held {
            el.opacity(0.64)
        } else {
            el.cursor_pointer().hover(move |s| s.bg(accent))
        }
    })
}

/// A row that runs its act and closes the menu.
fn item(selector: &'static str, label: &'static str, held: bool, act: &Act, close: &Act, accent: Hsla) -> impl IntoElement {
    let (act, close) = (act.clone(), close.clone());
    row(selector, held, accent)
        .when(!held, |el| {
            el.on_click(move |_, window, cx| {
                act(window, cx);
                close(window, cx);
            })
        })
        .child(label)
}

impl RenderOnce for MoreMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        // Where the press that last closed the popup from outside went down, so the button does not open it again at once.
        let closed_by = window.use_keyed_state("browser-more-closed", cx, |_, _| None::<Point<Pixels>>);
        let focus: FocusHandle = window.use_keyed_state("browser-more-focus", cx, |_, cx| cx.focus_handle()).read(cx).clone();
        // The popup takes focus as it opens, however it was opened, so Escape reaches it.
        let armed = window.use_keyed_state("browser-more-armed", cx, |_, _| false);
        if *armed.read(cx) != self.open {
            armed.update(cx, |armed, _| *armed = self.open);
            if self.open {
                focus.focus(window, cx);
            }
        }
        let toggle = self.on_toggle.clone();
        let guard = closed_by.clone();
        // The tooltip stands down while the menu it opens is up, as Base UI's does.
        let trigger = Button::icon_xs("browser-more", Glyph::EllipsisVertical);
        let trigger = if self.open { trigger } else { trigger.tooltip("More") };
        let trigger = trigger.on_click(move |event, window, cx| {
            let closer = guard.update(cx, |at, _| at.take());
            if let ClickEvent::Mouse(click) = event {
                if closer == Some(click.down.position) {
                    return;
                }
            }
            toggle(window, cx)
        });
        let held = !self.has_page;
        let popup = self.open.then(|| {
            let (escape, outside) = (self.on_close.clone(), self.on_close.clone());
            let zoom_key = |id: &'static str, glyph| IconKey::new(id, id, glyph).held(held);
            let zoom = row("browser-more-zoom", held, p.accent).justify_between().child("Zoom").child(
                h_flex()
                    .gap_1()
                    .child(zoom_key("browser-zoom-out", Glyph::Minus).outline().on_click(self.on_zoom_out.clone()))
                    .child(
                        div()
                            .debug_selector(|| "browser-zoom-percent".into())
                            .min_w(px(48.))
                            .text_center()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
                            .text_color(p.muted_foreground)
                            .child(percent(self.zoom)),
                    )
                    .child(zoom_key("browser-zoom-in", Glyph::Plus).outline().on_click(self.on_zoom_in.clone()))
                    .child(
                        zoom_key("browser-zoom-reset", Glyph::RotateCcw)
                            .hover(p.foreground.opacity(0.1))
                            .on_click(self.on_zoom_reset.clone()),
                    ),
            );
            let surface = v_flex()
                .id("browser-more-menu")
                .debug_selector(|| "browser-more-menu".into())
                .occlude()
                .min_w(MENU_WIDTH)
                .p_1()
                .rounded(px(12.))
                .track_focus(&focus)
                .text_size(px(14.))
                .line_height(px(20.))
                .text_color(p.foreground)
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        escape(window, cx);
                    }
                })
                .on_mouse_down_out(move |event, window, cx| {
                    let at = event.position;
                    closed_by.update(cx, |by, _| *by = Some(at));
                    outside(window, cx);
                })
                .child(item("browser-more-hard-reload", "Hard reload", held, &self.on_hard_reload, &self.on_close, p.accent))
                .child(item("browser-more-copy", "Copy URL", held, &self.on_copy, &self.on_close, p.accent))
                .child(item("browser-more-outside", "Open in system browser", held, &self.on_open_outside, &self.on_close, p.accent))
                .child(item("browser-more-new-tab", "New tab", false, &self.on_new_tab, &self.on_close, p.accent))
                .child(div().mx_2().my_1().h(px(1.)).bg(p.border))
                .child(zoom);
            deferred(
                anchored()
                    .position_mode(AnchoredPositionMode::Local)
                    .anchor(Anchor::TopRight)
                    .position(point(px(24.), px(30.)))
                    .child(glass(surface, &p)),
            )
            .with_priority(1)
        });
        div().debug_selector(|| "browser-more".into()).relative().flex().flex_none().child(trigger).children(popup)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui_kit::component::Root;
    use gpui_kit::{px, size, Context, Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};

    use super::*;
    use crate::ui::{self, Side};

    #[derive(Default)]
    struct Counts {
        reload: Cell<u32>,
        copy: Cell<u32>,
        new_tab: Cell<u32>,
        zoom_in: Cell<u32>,
        zoom_out: Cell<u32>,
        reset: Cell<u32>,
        beneath: Cell<u32>,
    }

    struct Host {
        open: bool,
        has_page: bool,
        counts: Rc<Counts>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let me: Entity<Host> = cx.entity();
            let c = self.counts.clone();
            let act = |f: fn(&Counts) -> &Cell<u32>| -> Act {
                let c = c.clone();
                Rc::new(move |_, _| f(&c).set(f(&c).get() + 1))
            };
            let set = |open: Option<bool>| -> Act {
                let me = me.clone();
                Rc::new(move |_, cx| {
                    me.update(cx, |host, cx| {
                        host.open = open.unwrap_or(!host.open);
                        cx.notify();
                    })
                })
            };
            let beneath = self.counts.clone();
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().flex().justify_end().p(px(16.)).child(MoreMenu {
                    open: self.open,
                    has_page: self.has_page,
                    zoom: 1.2,
                    on_toggle: set(None),
                    on_close: set(Some(false)),
                    on_hard_reload: act(|c| &c.reload),
                    on_copy: act(|c| &c.copy),
                    on_open_outside: act(|c| &c.copy),
                    on_new_tab: act(|c| &c.new_tab),
                    on_zoom_in: act(|c| &c.zoom_in),
                    on_zoom_out: act(|c| &c.zoom_out),
                    on_zoom_reset: act(|c| &c.reset),
                }))
                .child(
                    div()
                        .debug_selector(|| "beneath".into())
                        .flex_1()
                        .on_mouse_down(MouseButton::Left, move |_, _, _| beneath.beneath.set(beneath.beneath.get() + 1)),
                )
        }
    }

    fn open(cx: &mut TestAppContext, has_page: bool) -> (&mut VisualTestContext, Entity<Host>, Rc<Counts>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cx.set_global(ui::Still(true));
        });
        let counts = Rc::new(Counts::default());
        let kept = counts.clone();
        let host = cx.new(|_| Host { open: false, has_page, counts: kept });
        let shown = host.clone();
        let (_, cx) = cx.add_window_view(move |window, cx| Root::new(shown, window, cx));
        cx.simulate_resize(size(px(480.), px(400.)));
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let trigger = cx.debug_bounds("browser-more").expect("the button is drawn");
        cx.simulate_click(trigger.center(), Modifiers::none());
        cx.run_until_parked();
        (cx, host, counts)
    }

    fn is_open(cx: &mut VisualTestContext, host: &Entity<Host>) -> bool {
        let open = host.read_with(cx, |h, _| h.open);
        assert_eq!(open, cx.debug_bounds("browser-more-menu").is_some(), "the popup is drawn exactly while open");
        open
    }

    fn press(cx: &mut VisualTestContext, selector: &'static str) {
        let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"));
        cx.simulate_click(at.center(), Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn the_menu_is_224_wide_at_the_buttons_end_6_px_below(cx: &mut TestAppContext) {
        let (cx, host, _) = open(cx, true);
        assert!(is_open(cx, &host), "a press on the button opens it");
        let (trigger, menu) = (cx.debug_bounds("browser-more").unwrap(), cx.debug_bounds("browser-more-menu").unwrap());
        assert_eq!((trigger.size.width, trigger.size.height), (px(24.), px(24.)));
        assert_eq!(menu.size.width, MENU_WIDTH);
        assert_eq!((menu.right(), menu.top() - trigger.bottom()), (trigger.right(), px(6.)));
        let rows = cx.debug_bounds("browser-more-hard-reload").unwrap();
        assert_eq!((rows.size.height, rows.left() - menu.left()), (px(28.), px(5.)), "28 px rows, 4 px in from the 1 px edge");
        let percent = cx.debug_bounds("browser-zoom-percent").unwrap();
        assert_eq!(percent.size.width, px(48.));
        for key in ["browser-zoom-out", "browser-zoom-in", "browser-zoom-reset"] {
            let at = cx.debug_bounds(key).unwrap();
            assert_eq!((at.size.width, at.size.height), (px(24.), px(24.)), "{key}");
        }
    }

    #[gpui_kit::test]
    fn the_zoom_buttons_keep_the_menu_open(cx: &mut TestAppContext) {
        let (cx, host, counts) = open(cx, true);
        press(cx, "browser-zoom-in");
        press(cx, "browser-zoom-in");
        press(cx, "browser-zoom-out");
        press(cx, "browser-zoom-reset");
        assert_eq!((counts.zoom_in.get(), counts.zoom_out.get(), counts.reset.get()), (2, 1, 1));
        assert!(is_open(cx, &host), "a press inside does not close it");
        assert_eq!(counts.beneath.get(), 0);
    }

    #[gpui_kit::test]
    fn a_row_runs_its_act_and_closes(cx: &mut TestAppContext) {
        let (cx, host, counts) = open(cx, true);
        press(cx, "browser-more-hard-reload");
        assert_eq!(counts.reload.get(), 1);
        assert!(!is_open(cx, &host));
    }

    #[gpui_kit::test]
    fn escape_closes_the_menu(cx: &mut TestAppContext) {
        let (cx, host, _) = open(cx, true);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!is_open(cx, &host));
    }

    #[gpui_kit::test]
    fn a_press_outside_closes_it_and_the_button_does_not_reopen_it(cx: &mut TestAppContext) {
        let (cx, host, counts) = open(cx, true);
        let beneath = cx.debug_bounds("beneath").unwrap();
        cx.simulate_click(gpui_kit::point(beneath.left() + px(20.), beneath.bottom() - px(20.)), Modifiers::none());
        cx.run_until_parked();
        assert!(!is_open(cx, &host), "a press outside closes it");
        assert_eq!(counts.beneath.get(), 1, "and lands where it was aimed");
        press(cx, "browser-more");
        assert!(is_open(cx, &host), "the button opens it again");
        press(cx, "browser-more");
        assert!(!is_open(cx, &host), "a press on the button while open closes it once, not shut and open again");
    }

    #[gpui_kit::test]
    fn the_menu_covers_what_is_under_it(cx: &mut TestAppContext) {
        let (cx, host, counts) = open(cx, true);
        let (menu, beneath) = (cx.debug_bounds("browser-more-menu").unwrap(), cx.debug_bounds("beneath").unwrap());
        let separator = gpui_kit::point(menu.center().x, cx.debug_bounds("browser-more-zoom").unwrap().top() - px(4.));
        assert!(beneath.contains(&separator), "the press lands where the surface lies");
        cx.simulate_click(separator, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(counts.beneath.get(), 0, "the popup occludes");
        assert!(is_open(cx, &host));
    }

    #[gpui_kit::test]
    fn with_no_page_the_page_rows_and_zoom_are_held(cx: &mut TestAppContext) {
        let (cx, host, counts) = open(cx, false);
        press(cx, "browser-more-hard-reload");
        press(cx, "browser-more-copy");
        press(cx, "browser-zoom-in");
        assert_eq!((counts.reload.get(), counts.copy.get(), counts.zoom_in.get()), (0, 0, 0));
        assert!(is_open(cx, &host), "a held row takes no press and the menu stays");
        press(cx, "browser-more-new-tab");
        assert_eq!(counts.new_tab.get(), 1, "New tab never needs a page");
        assert!(!is_open(cx, &host));
    }
}
