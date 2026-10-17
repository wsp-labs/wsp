// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's boxes in each state against the web app's, as the reference shots measured them in Chromium at
//! 1280 x 800 (the thread column 1024 px wide beside the sidebar): the box, the strip under it, the tab over it and
//! the place of the text.

#[path = "composer/states.rs"]
mod states;

use desktop_gpui::screens::composer::model::Hold;
use desktop_gpui::screens::composer::screen::column;
use desktop_gpui::screens::composer::{self, Composer, ComposerEvent, Outgoing, Popup};
use desktop_gpui::ui::{self, ActivePalette, Side};
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, Bounds, Context, Entity, Focusable, Pixels, TestAppContext, VisualTestContext, Window};

struct Column {
    composer: Entity<Composer>,
}

impl Render for Column {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fresh = self.composer.read(cx).fresh();
        column(fresh, cx.palette().background, self.composer.clone())
    }
}

fn open<'a>(state: &str, cx: &'a mut TestAppContext) -> &'a mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
    });
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let composer = cx.new(|cx| Composer::new(window, cx));
        made = Some(composer.clone());
        let column = cx.new(|_| Column { composer });
        Root::new(column, window, cx)
    });
    cx.simulate_resize(size(px(1024.), px(800.)));
    let composer = made.unwrap();
    composer.update_in(cx, |c, window, cx| states::stage(state, c, window, cx));
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
    cx
}

fn rect(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("nothing drew {selector}"))
}

/// x, y, width and height in whole pixels, the web's numbers less its 256 px sidebar.
fn xywh(b: Bounds<Pixels>) -> (f32, f32, f32, f32) {
    (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height))
}

#[gpui_kit::test]
fn one_line_on_a_thread_that_has_run(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    // Web: host 384,698 768x50; tray 406..1130 showing 32 px under it.
    assert_eq!(xywh(rect(cx, "composer-box")), (128., 698., 768., 50.));
    assert_eq!(xywh(rect(cx, "composer-tray")), (150., 748., 724., 32.));
    let send = rect(cx, "composer-send");
    assert_eq!(f32::from(send.size.width), 32.);
    assert_eq!(f32::from(send.right()), 128. + 768. - 1. - 8., "send keeps the box's 8 px right inset");
    let pickers = rect(cx, "composer-pickers");
    assert!(pickers.right() < rect(cx, "composer-attach").left(), "the pickers sit before attach and send");
}

#[gpui_kit::test]
fn a_draft_past_one_line_takes_the_tall_layout(cx: &mut TestAppContext) {
    let cx = open("typing", cx);
    // Web: shell 384,620 768x160, so the box is 128 high over the 32 px strip: 1 + 14 + 3 lines of 22.75 + 4 + 32 + 8
    // + 1 is 128.25, which Chromium draws in 128 px and GPUI rounds to 129.
    let b = xywh(rect(cx, "composer-box"));
    assert_eq!((b.0, b.2), (128., 768.));
    assert!((b.3 - 128.).abs() <= 1. && (b.1 - 620.).abs() <= 1., "box {b:?} against the web's 620 and 128");
}

#[gpui_kit::test]
fn a_new_thread_takes_the_full_layout_centred(cx: &mut TestAppContext) {
    let cx = open("new-thread", cx);
    let b = xywh(rect(cx, "composer-box"));
    assert_eq!((b.2, b.3), (768., 148.), "1 + 16 + 70 + 4 + 8 + 32 + 16 + 1");
    // Centred as ChatView.tsx centres a thread with no message, (748 - 208 - 88) / 2 from the foot of the view under
    // the 52 px header. The web's New thread page is its own page and draws the box 5.5 px higher, at 368.5.
    assert_eq!(b.1, 374.);
}

#[gpui_kit::test]
fn files_ride_in_a_row_over_the_text(cx: &mut TestAppContext) {
    let cx = open("attachments", cx);
    // Web: shell 768x150 at y 630, so the box is 118 high: 1 + 12 + 56 + 48 + 1.
    assert_eq!(xywh(rect(cx, "composer-box")), (128., 630., 768., 118.));
    assert_eq!(f32::from(rect(cx, "composer-files").size.height), 68.);
}

#[gpui_kit::test]
fn the_banner_and_the_drawer_sit_on_the_box(cx: &mut TestAppContext) {
    let cx = open("banner", cx);
    // Web: shell 384,358 768x212: the banner's 32 px over the full box.
    let banner = rect(cx, "composer-banner");
    let boxed = rect(cx, "composer-box");
    assert_eq!(f32::from(banner.size.width), 724.);
    assert_eq!(f32::from(boxed.top() - banner.top()), 32., "32 px of tab over the box's edge");
    assert!((f32::from(banner.top()) - 358.).abs() <= 1., "banner top {:?} against the web's 358", banner.top());
}

#[gpui_kit::test]
fn a_queued_message_shows_in_the_drawer(cx: &mut TestAppContext) {
    let cx = open("queued", cx);
    // Web: shell 384,662 768x118: a 36 px drawer over the one-line box and its strip.
    let drawer = rect(cx, "composer-drawer");
    assert_eq!(xywh(drawer).2, 724.);
    assert_eq!(f32::from(rect(cx, "composer-box").top() - drawer.top()), 36., "37 px of drawer, 1 px under the box's edge");
    assert_eq!(f32::from(drawer.top()), 662.);
    assert!(cx.debug_bounds("composer-stop").is_some(), "stop stands in for send while the turn runs");
}

#[gpui_kit::test]
fn the_model_picker_searches_and_enter_picks(cx: &mut TestAppContext) {
    let cx = open("model-picker-open", cx);
    let composer = composer_of(cx);
    let rows = composer.read_with(cx, |c, cx| c.popup_rows(cx));
    assert_eq!(rows.first().map(String::as_str), Some("claude-opus-5-5"));
    assert!(rows.contains(&"claude-sonnet-5".to_owned()), "the fold opens on the legacy model the thread runs");
    cx.simulate_input("haiku");
    cx.run_until_parked();
    let rows = composer.read_with(cx, |c, cx| c.popup_rows(cx));
    assert_eq!(rows, ["claude-haiku-5-5", "claude-haiku-4-5-20251001"], "the search filters models and legacy models");
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    composer.read_with(cx, |c, _| {
        assert_eq!(c.popup(), None, "a pick closes the picker");
        assert_eq!(c.picks().model.as_deref(), Some("claude-haiku-4-5-20251001"));
    });
}

fn composer_of(cx: &mut VisualTestContext) -> Entity<Composer> {
    cx.update(|window, cx| {
        let root = window.root::<Root>().flatten().unwrap();
        let column = root.read(cx).view().clone().downcast::<Column>().unwrap();
        column.read(cx).composer.clone()
    })
}

/// The prompts of every new turn the composer asks the host for.
fn starts(composer: &Entity<Composer>, cx: &mut VisualTestContext) -> std::rc::Rc<std::cell::RefCell<Vec<String>>> {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = seen.clone();
    cx.update(|_, cx| {
        cx.subscribe(composer, move |_, event: &ComposerEvent, _| {
            if let ComposerEvent::Start(out) = event {
                sink.borrow_mut().push(out.prompt.clone());
            }
        })
        .detach()
    });
    seen
}

#[gpui_kit::test]
fn a_steer_out_at_the_turns_end_is_not_sent_again(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_steer_preferred(true, cx);
        c.set_text("turn left", window, cx);
        c.submit(window, cx);
        c.set_text("and then right", window, cx);
        c.set_steer_preferred(false, cx);
        c.submit(window, cx);
        // The turn ends before the host answers the steer.
        c.set_turn(true, false, cx);
    });
    assert!(started.borrow().is_empty(), "nothing starts while the steer is out");
    composer.update(cx, |c, cx| c.steered(true, cx));
    assert_eq!(*started.borrow(), ["and then right"], "the taken steer leaves; the message queued behind it goes");
    assert!(composer.read_with(cx, |c, _| c.queue().is_empty()));
}

#[gpui_kit::test]
fn a_steer_the_turn_did_not_take_goes_as_the_next_turn(cx: &mut TestAppContext) {
    let cx = open("running", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_steer_preferred(true, cx);
        c.set_text("turn left", window, cx);
        c.submit(window, cx);
        c.set_turn(true, false, cx);
        c.steered(false, cx);
    });
    assert_eq!(*started.borrow(), ["turn left"]);
}

#[gpui_kit::test]
fn the_hosts_yes_to_a_send_lets_go_of_the_send(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_text("hi", window, cx);
        c.submit(window, cx);
    });
    assert_eq!(composer.read_with(cx, |c, _| c.hold().cloned()), Some(Hold::InFlight));
    composer.update(cx, |c, cx| c.sent(cx));
    assert_eq!(composer.read_with(cx, |c, _| c.hold().cloned()), None, "a turn that ended before its events arrived holds nothing");
}

#[gpui_kit::test]
fn a_refused_send_comes_back_ahead_of_the_draft(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| {
        c.set_text("first", window, cx);
        c.submit(window, cx);
        c.set_text("second draft", window, cx);
    });
    let sent = Outgoing { prompt: started.borrow()[0].clone(), files: Vec::new(), picks: Default::default() };
    composer.update_in(cx, |c, window, cx| c.refused_send(sent, window, cx));
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "first\nsecond draft");
}

#[gpui_kit::test]
fn enter_in_the_text_does_not_pick_from_an_open_menu(cx: &mut TestAppContext) {
    let cx = open("effort-picker-open", cx);
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.editor().focus_handle(cx).focus(window, cx));
    cx.run_until_parked();
    assert_eq!(composer.read_with(cx, |c, _| c.popup()), Some(Popup::Reasoning), "focusing the text leaves the menu open");
    cx.simulate_keystrokes("enter");
    composer.read_with(cx, |c, cx| {
        assert_eq!(c.text(cx), "", "Enter writes nothing in the text");
        assert_eq!(c.popup(), Some(Popup::Reasoning));
        assert_eq!(c.picks().effort, None, "Enter before an arrow key picks nothing");
    });
    cx.simulate_keystrokes("down enter");
    composer.read_with(cx, |c, _| {
        assert_eq!(c.popup(), None);
        assert_eq!(c.picks().effort.as_deref(), Some("low"));
    });
}

#[gpui_kit::test]
fn enter_sends_and_leaves_the_box_empty(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let started = starts(&composer, cx);
    composer.update_in(cx, |c, window, cx| c.editor().focus_handle(cx).focus(window, cx));
    cx.simulate_input("hi");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("there");
    cx.simulate_keystrokes("enter");
    assert_eq!(*started.borrow(), ["hi\nthere"], "Shift+Enter writes a newline, Enter sends");
    assert_eq!(composer.read_with(cx, |c, cx| c.text(cx)), "", "no newline is left behind in the emptied box");
}
