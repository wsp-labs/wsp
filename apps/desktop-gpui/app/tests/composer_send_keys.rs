// SPDX-License-Identifier: AGPL-3.0-only
//! The keys that send and move in the composer's field, against today's: Enter or Cmd or Ctrl and Enter sends by
//! `sendWith` (`enterSends`, `ChatComposer.tsx:1050`), an Enter while the input method composes is its own
//! (`plugins.ts:51`), and Home and End go to the line's ends (`plugins.ts:102`).

#[path = "composer/harness.rs"]
mod harness;
#[path = "composer/states.rs"]
mod states;

use desktop_gpui::screens::composer::Composer;
use gpui_kit::{Entity, EntityInputHandler as _, Focusable as _, TestAppContext, VisualTestContext};
use harness::{composer_of, open, starts};

fn typed<'a>(text: &str, cx: &'a mut TestAppContext) -> (&'a mut VisualTestContext, Entity<Composer>) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.editor().focus_handle(cx).focus(window, cx));
    cx.simulate_input(text);
    (cx, composer)
}

fn text(composer: &Entity<Composer>, cx: &mut VisualTestContext) -> String {
    composer.read_with(cx, |c, cx| c.text(cx))
}

#[gpui_kit::test]
fn with_send_on_enter_enter_sends_and_the_command_chord_breaks_the_line(cx: &mut TestAppContext) {
    let (cx, composer) = typed("one", cx);
    let started = starts(&composer, cx);
    composer.update(cx, |c, cx| c.set_send_with_mod(false, cx));
    cx.simulate_keystrokes("secondary-enter");
    assert!(started.borrow().is_empty(), "Cmd or Ctrl and Enter does not send");
    assert_eq!(text(&composer, cx), "one\n");
    cx.simulate_keystrokes("shift-enter");
    assert!(started.borrow().is_empty(), "Shift and Enter never sends");
    cx.simulate_input("two");
    cx.simulate_keystrokes("enter");
    assert_eq!(*started.borrow(), ["one\n\ntwo"]);
}

#[gpui_kit::test]
fn with_send_on_the_command_chord_enter_breaks_the_line(cx: &mut TestAppContext) {
    let (cx, composer) = typed("one", cx);
    let started = starts(&composer, cx);
    composer.update(cx, |c, cx| c.set_send_with_mod(true, cx));
    cx.simulate_keystrokes("enter");
    assert!(started.borrow().is_empty(), "Enter does not send");
    assert_eq!(text(&composer, cx), "one\n");
    cx.simulate_keystrokes("shift-enter");
    assert!(started.borrow().is_empty(), "Shift and Enter never sends");
    cx.simulate_input("two");
    cx.simulate_keystrokes("secondary-enter");
    assert_eq!(*started.borrow(), ["one\n\ntwo"]);
}

#[gpui_kit::test]
fn enter_while_the_input_method_composes_does_not_send(cx: &mut TestAppContext) {
    let (cx, composer) = typed("konnichiwa ", cx);
    let started = starts(&composer, cx);
    let editor = composer.read_with(cx, |c, _| c.editor().clone());
    editor.update_in(cx, |e, window, cx| e.replace_and_mark_text_in_range(None, "こん", None, window, cx));
    cx.simulate_keystrokes("enter");
    assert!(started.borrow().is_empty(), "the Enter is the input method's while text is marked");
    editor.update_in(cx, |e, window, cx| e.unmark_text(window, cx));
    cx.simulate_keystrokes("enter");
    assert_eq!(*started.borrow(), ["konnichiwa こん"], "once the text is committed Enter sends");
}

#[gpui_kit::test]
fn home_and_end_go_to_the_ends_of_the_caret_line(cx: &mut TestAppContext) {
    let (cx, composer) = typed("first line", cx);
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("second");
    let editor = composer.read_with(cx, |c, _| c.editor().clone());
    let caret = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.cursor());
    assert_eq!(caret(cx), 17);
    cx.simulate_keystrokes("home");
    assert_eq!(caret(cx), 11, "Home goes to the start of the caret's line, not of the text");
    cx.simulate_keystrokes("end");
    assert_eq!(caret(cx), 17, "End goes to its end");
    cx.simulate_keystrokes("shift-home");
    assert_eq!(editor.read_with(cx, |e, _| e.selected_range()), 11..17, "Shift and Home selects to the start");
}
