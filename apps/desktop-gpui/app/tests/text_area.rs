// SPDX-License-Identifier: AGPL-3.0-only
//! The multi-line field in a window: typing reaches its state, it grows a line at a time to its ceiling, Cmd or Ctrl
//! and Enter submits without a newline (and not while blank), Escape cancels, and a held field takes nothing.

use std::cell::Cell;
use std::rc::Rc;

use desktop_gpui::ui::text_area::MAX_ROWS;
use desktop_gpui::ui::{self, Side, TextArea, TextAreaLook};
use gpui_kit::component::input::TextareaState;
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, Context, Entity, Focusable as _, Modifiers, TestAppContext, VisualTestContext, Window};

struct Probe {
    state: Entity<TextareaState>,
    disabled: bool,
    sent: Rc<Cell<u32>>,
    cancelled: Rc<Cell<u32>>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (sent, cancelled) = (self.sent.clone(), self.cancelled.clone());
        div().p(px(24.)).w(px(480.)).child(
            TextArea::new("probe", &self.state)
                .look(TextAreaLook::Comment)
                .disabled(self.disabled)
                .on_submit(move |_, _| sent.set(sent.get() + 1))
                .on_escape(move |_, _| cancelled.set(cancelled.get() + 1)),
        )
    }
}

struct Opened<'a> {
    cx: &'a mut VisualTestContext,
    probe: Entity<Probe>,
    sent: Rc<Cell<u32>>,
    cancelled: Rc<Cell<u32>>,
}

fn open(cx: &mut TestAppContext, disabled: bool) -> Opened<'_> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let sent = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let (s, c) = (sent.clone(), cancelled.clone());
    let mut probe = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let state = TextArea::state("Add a comment…", window, cx);
        let view = cx.new(|_| Probe { state, disabled, sent: s, cancelled: c });
        probe = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(800.)));
    cx.run_until_parked();
    Opened { cx, probe: probe.unwrap(), sent, cancelled }
}

impl Opened<'_> {
    fn value(&mut self) -> String {
        let state = self.probe.read_with(self.cx, |p, _| p.state.clone());
        state.read_with(self.cx, |s, _| s.value().to_string())
    }

    fn height(&mut self) -> f32 {
        self.cx.run_until_parked();
        f32::from(self.cx.debug_bounds("text-area").expect("the field is drawn").size.height)
    }

    fn click(&mut self) {
        let at = self.cx.debug_bounds("text-area").expect("the field is drawn");
        self.cx.simulate_click(at.center(), Modifiers::none());
        self.cx.run_until_parked();
    }

    fn focused(&mut self) -> bool {
        let state = self.probe.read_with(self.cx, |p, _| p.state.clone());
        self.cx.update(|window, cx| state.read(cx).focus_handle(cx).is_focused(window))
    }
}

#[gpui_kit::test]
fn typing_reaches_the_state(cx: &mut TestAppContext) {
    let mut f = open(cx, false);
    f.click();
    assert!(f.focused(), "a press anywhere in the box gives it the caret");
    f.cx.simulate_input("looks right");
    f.cx.simulate_keystrokes("shift-enter");
    f.cx.simulate_input("ship it");
    assert_eq!(f.value(), "looks right\nship it");
}

#[gpui_kit::test]
fn it_grows_a_line_at_a_time_to_its_ceiling(cx: &mut TestAppContext) {
    let mut f = open(cx, false);
    // The comment look: a 48 px floor inside a 1 px edge, 20 px lines and 6 px above and below.
    let floor = f.height();
    assert_eq!(floor, 50., "an empty field stands at its floor");
    f.click();
    f.cx.simulate_input("one");
    for n in 2..=MAX_ROWS + 4 {
        f.cx.simulate_keystrokes("enter");
        f.cx.simulate_input(&format!("line {n}"));
    }
    let top = f.height();
    assert_eq!(top, (MAX_ROWS as f32) * 20. + 12. + 2., "past {MAX_ROWS} lines it stops growing and scrolls");
    for _ in 0..MAX_ROWS + 2 {
        f.cx.simulate_keystrokes("backspace backspace backspace backspace backspace backspace backspace backspace");
    }
    f.cx.simulate_keystrokes("secondary-a backspace");
    f.cx.simulate_input("a\nb\nc");
    assert_eq!(f.height(), 3. * 20. + 12. + 2., "three lines stand three lines high");
}

#[gpui_kit::test]
fn the_chord_submits_without_a_newline(cx: &mut TestAppContext) {
    let mut f = open(cx, false);
    f.click();
    f.cx.simulate_keystrokes("secondary-enter");
    assert_eq!(f.sent.get(), 0, "a blank field sends nothing");
    assert_eq!(f.value(), "", "and takes no newline");
    f.cx.simulate_input("nit: rename");
    f.cx.simulate_keystrokes("secondary-enter");
    assert_eq!(f.sent.get(), 1, "Cmd or Ctrl and Enter sends");
    assert_eq!(f.value(), "nit: rename", "the chord leaves no newline behind");
    f.cx.simulate_keystrokes("enter");
    assert_eq!(f.sent.get(), 1, "a plain Enter is a newline, not a send");
    assert_eq!(f.value(), "nit: rename\n");
}

#[gpui_kit::test]
fn escape_cancels(cx: &mut TestAppContext) {
    let mut f = open(cx, false);
    f.click();
    f.cx.simulate_input("draft");
    f.cx.simulate_keystrokes("escape");
    assert_eq!(f.cancelled.get(), 1);
    assert_eq!(f.sent.get(), 0);
}

#[gpui_kit::test]
fn a_held_field_takes_nothing(cx: &mut TestAppContext) {
    let mut f = open(cx, true);
    f.click();
    assert!(!f.focused(), "a press on a held field gives it no caret");
    f.cx.simulate_input("x");
    assert_eq!(f.value(), "");
    let state = f.probe.read_with(f.cx, |p, _| p.state.clone());
    f.cx.update(|window, cx| state.read(cx).focus_handle(cx).focus(window, cx));
    f.cx.simulate_keystrokes("secondary-enter");
    assert_eq!(f.sent.get(), 0);
}
