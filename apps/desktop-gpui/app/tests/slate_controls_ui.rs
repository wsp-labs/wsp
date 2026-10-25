// SPDX-License-Identifier: AGPL-3.0-only
//! The shared pieces the slate's controls stand on: the segmented control held whole, the switch's thumb under a
//! press, the checkbox's lift easing off as it is ticked, and the menu's surface.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use desktop_gpui::ui::{self, ActivePalette as _, Segmented, Side, Switch};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    div, prelude::*, px, size, AnyElement, Bounds, Context, Modifiers, MouseButton, Pixels, TestAppContext, VisualTestContext, Window,
};

type Draw = Box<dyn Fn(&mut Window, &mut Context<Lone>) -> AnyElement>;

struct Lone(Draw);

impl Render for Lone {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(400.)).p(px(24.)).child((self.0)(window, cx))
    }
}

fn draw(cx: &mut TestAppContext, piece: impl Fn(&mut Window, &mut Context<Lone>) -> AnyElement + 'static) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let view = cx.new(|_| Lone(Box::new(piece)));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
    cx.simulate_resize(size(px(640.), px(300.)));
    cx.run_until_parked();
    cx
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn"))
}

/// The frames one drawn frame asks for, after what earlier frames asked for is spent.
fn asked(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        window.simulate_next_frame(cx)
    })
}

/// Draws frames until the clock has run `ms` past now.
fn settle(cx: &mut VisualTestContext, ms: u64) {
    for _ in 0..ms / 50 {
        cx.executor().advance_clock(Duration::from_millis(50));
        asked(cx);
    }
}

#[gpui_kit::test]
fn a_held_group_takes_no_press_and_no_arrow(cx: &mut TestAppContext) {
    let picked = Rc::new(Cell::new(None));
    let p = picked.clone();
    let cx = draw(cx, move |_, _| {
        let p = p.clone();
        // In a flex row the group is as wide as its segments, so a press at its right end lands on Mine.
        div()
            .flex()
            .child(Segmented::new("s", 0).keycap().held(true).segment("All").segment("Mine").on_pick(move |ix, _, _| p.set(Some(ix))))
            .into_any_element()
    });
    let group = bounds(cx, "segmented");
    let second = gpui_kit::point(group.right() - px(12.), group.center().y);
    cx.simulate_click(second, Modifiers::none());
    assert_eq!(picked.get(), None, "a press on a held group picked");
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("right");
    assert_eq!(picked.get(), None, "a held group picked");
}

#[gpui_kit::test]
fn a_pressed_switch_stretches_its_thumb_from_its_side_and_lets_go(cx: &mut TestAppContext) {
    let on = Rc::new(Cell::new(true));
    let o = on.clone();
    let cx = draw(cx, move |_, _| Switch::new("sw", o.get()).on_change(|_, _, _| {}).into_any_element());
    let rest = bounds(cx, "switch-thumb");
    assert_eq!(rest.size.width, px(16.), "the thumb at rest is the 16 px --thumb-size");
    let at = bounds(cx, "switch").center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    settle(cx, 300);
    let held = bounds(cx, "switch-thumb");
    assert!((f32::from(held.size.width) - 17.6).abs() < 0.15, "a press stretches the thumb to 110%: {:?}", held.size.width);
    assert!((f32::from(held.right()) - f32::from(rest.right())).abs() <= 0.5, "an on thumb stretches from its right edge");
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    settle(cx, 300);
    assert_eq!(bounds(cx, "switch-thumb").size.width, px(16.), "let go, the thumb is its own size again");
    assert_eq!(asked(cx), 0, "settled, the switch asks for no frame");
}

#[gpui_kit::test]
fn a_ticked_checkbox_eases_its_lift_off_then_asks_for_no_frame(cx: &mut TestAppContext) {
    let checked = Rc::new(Cell::new(false));
    let c = checked.clone();
    let cx = draw(cx, move |window, cx| ui::controls::checkbox("tick", c.get(), false, window, cx).into_any_element());
    assert_eq!(asked(cx), 0, "at rest the checkbox asks for no frame");
    checked.set(true);
    assert!(asked(cx) > 0, "ticked, its lift eases off");
    settle(cx, 200);
    assert_eq!(asked(cx), 0, "past its 150 ms the ease is done");
}

#[gpui_kit::test]
fn the_menu_stands_on_the_popover_ground_at_twelve_px_under_the_popover_shadow(cx: &mut TestAppContext) {
    let cx = draw(cx, |_, _| div().into_any_element());
    cx.update(|_, cx| {
        let p = cx.palette().clone();
        let surface = ui::menu::surface(&p);
        assert_eq!(surface.radius, px(12.), "MenuPopup's rounded-[12px]");
        assert_eq!(surface.edge, p.popover_edge, "dropdown-glass's edge");
        assert_eq!(surface.shadow, p.popover_shadow, "popover-shadow");
    });
}
