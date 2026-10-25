// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's boxes in each state against the web app's, as the reference shots measured them in Chromium at
//! 1280 x 800 (the thread column 1024 px wide beside the sidebar): the box, the strip under it, the tab over it and
//! the place of the text.

#[path = "composer/harness.rs"]
mod harness;
#[path = "composer/states.rs"]
mod states;

use desktop_gpui::screens::composer::model::Hold;
use desktop_gpui::screens::composer::{Outgoing, Popup};
use gpui_kit::{px, Bounds, Focusable, Pixels, TestAppContext, VisualTestContext};
use harness::{composer_of, open, starts};

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
    let sent = Outgoing {
        prompt: started.borrow()[0].clone(),
        files: Vec::new(),
        picks: Default::default(),
        fast: false,
        resume: None,
        from_queue: false,
        fork: None,
    };
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

#[gpui_kit::test]
fn a_draft_that_fits_beside_the_pickers_keeps_one_line(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    let column = f32::from(rect(cx, "composer-editor").size.width);
    // Letters until the line comes within 16 px of the text column, closer than the field once wrapped (18 short).
    let draft = cx.update(|window, cx| {
        let width = |text: &str| {
            let run = gpui_kit::TextRun {
                len: text.len(),
                font: gpui_kit::font(gpui_kit::component::ActiveTheme::theme(cx).font_family.clone()),
                color: gpui_kit::Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(window.text_system().layout_line(text, px(14.), &[run], None).width)
        };
        let sentence = "why does the host send the head twice on reconnect while the thread is busy with a long turn";
        let draft = (1..sentence.len()).map(|n| &sentence[..n]).take_while(|d| width(d) <= column - 2.).last().unwrap().to_owned();
        assert!(width(&draft) > column - 16., "{draft:?} is {} wide against {column}", width(&draft));
        draft
    });
    let composer = composer_of(cx);
    composer.update_in(cx, |c, window, cx| c.set_text(&draft, window, cx));
    cx.run_until_parked();
    assert_eq!(xywh(rect(cx, "composer-box")), (128., 698., 768., 50.), "{draft:?} in a column {column} wide");
}

#[gpui_kit::test]
fn the_one_line_triggers_stand_edge_to_edge(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    assert_eq!(rect(cx, "picker-reasoning").left(), rect(cx, "picker-model").right(), "no gap between the triggers on one line");
}

#[gpui_kit::test]
fn the_model_pickers_search_row_and_rows_keep_the_webs_pitch(cx: &mut TestAppContext) {
    let cx = open("model-picker-open", cx);
    assert_eq!(f32::from(rect(cx, "composer-model-search").size.height), 46., "py-3 around the 15 px field and its rule");
    let (first, second) = (rect(cx, "composer-model-row-0"), rect(cx, "composer-model-row-1"));
    assert_eq!(f32::from(second.top() - first.top()), 64., "each row 64 px, as the web's");
}

/// The window `width` wide, drawn again at it.
fn at_width(cx: &mut VisualTestContext, width: f32) {
    cx.simulate_resize(gpui_kit::size(px(width), px(800.)));
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

/// Under 640 px (`max-sm`): the composer's sides 12 px, the tray 36 px with 28 px items, the send 36 px; from 640 px
/// up the 20 px sides, the 32 px tray with 24 px items and the 32 px send come back.
#[gpui_kit::test]
fn under_640_px_the_composer_takes_the_phone_sizes(cx: &mut TestAppContext) {
    let cx = open("empty", cx);
    at_width(cx, 600.);
    let b = xywh(rect(cx, "composer-box"));
    assert_eq!((b.0, b.2), (12., 576.), "12 px sides, `px-3`");
    assert_eq!(xywh(rect(cx, "composer-tray")).3, 36., "the tray shows 36 px");
    assert_eq!(xywh(rect(cx, "tray-computer")).3, 28., "28 px tray items");
    let send = xywh(rect(cx, "composer-send"));
    assert_eq!((send.2, send.3), (36., 36.), "a 36 px send");
    assert_eq!(f32::from(rect(cx, "composer").bottom() - rect(cx, "composer-tray").bottom()), 16., "`pb-4` under the tray");

    at_width(cx, 640.);
    let b = xywh(rect(cx, "composer-box"));
    assert_eq!((b.0, b.2), (20., 600.), "20 px sides from 640 px");
    assert_eq!(xywh(rect(cx, "composer-tray")).3, 32.);
    assert_eq!(xywh(rect(cx, "tray-computer")).3, 24.);
    assert_eq!(xywh(rect(cx, "composer-send")).2, 32.);
}

/// The dock frame under 640 px (Dock.tsx `px-3 pb-3 sm:px-4 sm:pb-4`).
#[gpui_kit::test]
fn under_640_px_the_dock_takes_12_px_sides(cx: &mut TestAppContext) {
    let cx = open("prompt-dock", cx);
    at_width(cx, 600.);
    let dock = xywh(rect(cx, "composer-box"));
    assert_eq!((dock.0, dock.2), (12., 576.), "the dock's 12 px sides");
    assert_eq!(f32::from(rect(cx, "composer").bottom() - rect(cx, "composer-box").bottom()), 12., "`pb-3` under the dock");
}

#[gpui_kit::test]
fn under_640_px_the_full_box_takes_12_px_insets(cx: &mut TestAppContext) {
    let cx = open("new-thread", cx);
    at_width(cx, 600.);
    let boxed = rect(cx, "composer-box");
    let editor = rect(cx, "composer-editor");
    assert_eq!(f32::from(editor.left() - boxed.left()), 1. + 12., "the hairline and `px-3`");
    assert_eq!(f32::from(editor.top() - boxed.top()), 1. + 14., "the hairline and `pt-3.5`");
}

/// A pick the host would not take: its sentence under the cards in the error ink, and its fix after it in the body
/// ink, one paragraph that wraps as text does (PermissionPromptRow.tsx `PromptRefusal`, a `<p>` with the fix a span).
#[gpui_kit::test]
fn a_refusal_under_the_dock_carries_the_hosts_fix_after_its_sentence(cx: &mut TestAppContext) {
    use desktop_gpui::screens::thread_view::cards::Refusal;
    let cx = open("prompt-dock", cx);
    let composer = composer_of(cx);
    let ask = composer.read_with(cx, |c, _| c.dock().unwrap().prompt.ask.clone());
    let sentence = "The host could not take this answer because the thread it was asked on has ended since the question \
        was put, and a question outlives its turn only while that turn's agent still waits on it for an answer of its own.";
    let said = |fix: Option<&str>| Refusal { said: sentence.to_owned(), fix: fix.map(str::to_owned) };
    composer.update(cx, |c, cx| c.prompt_refused(&ask, Some(said(None)), cx));
    cx.run_until_parked();
    let alone = rect(cx, "refusal");
    assert!(alone.size.height > px(20.), "a long sentence wraps inside the dock: {alone:?}");
    composer.update(cx, |c, cx| c.prompt_refused(&ask, Some(said(Some("Try again."))), cx));
    cx.run_until_parked();
    let with_fix = rect(cx, "refusal");
    assert_eq!(with_fix.size.height, alone.size.height, "the fix runs on after the sentence's last line: {with_fix:?}");
}

/// Under 640 px the dock's foot stacks (Dock.tsx `StepFoot`, `flex-col-reverse`): the acts full width with the primary
/// on top, and Write instead centred under them; from 640 px up one row, the acts at the right.
#[gpui_kit::test]
fn under_640_px_the_dock_foot_stacks_its_acts_with_the_primary_on_top(cx: &mut TestAppContext) {
    let cx = open("prompt-dock", cx);
    at_width(cx, 600.);
    let dock = rect(cx, "prompt-dock");
    let (answer, write) = (rect(cx, "prompt-answer"), rect(cx, "prompt-write"));
    assert_eq!((answer.left() - dock.left(), dock.right() - answer.right()), (px(20.), px(20.)), "the primary across the foot");
    assert!(answer.bottom() <= write.top(), "the primary over Write instead: {answer:?} {write:?}");
    assert_eq!(write.center().x, dock.center().x, "Write instead centred");
    assert_eq!(dock.bottom() - write.bottom(), px(20.), "`pb-5` under the foot");

    at_width(cx, 640.);
    let dock = rect(cx, "prompt-dock");
    let (answer, write) = (rect(cx, "prompt-answer"), rect(cx, "prompt-write"));
    assert_eq!(dock.right() - answer.right(), px(20.), "the primary at the foot's right");
    assert_eq!(answer.center().y, write.center().y, "one row");
    assert_eq!(write.left() - dock.left(), px(20.));
}

/// A bar's foot in the dock's frame stacks the same way: Resume at reset over Cancel, both full width.
#[gpui_kit::test]
fn under_640_px_a_bars_foot_stacks_the_same_way(cx: &mut TestAppContext) {
    use desktop_gpui::screens::composer::model::{Drawer, Limit};
    use desktop_gpui::screens::composer::Bar;
    let cx = open("empty", cx);
    let composer = composer_of(cx);
    let limit = Limit { title: "Usage limit reached".into(), resets: true, ..Limit::default() };
    composer.update(cx, |c, cx| {
        c.set_drawer(Drawer { limit: Some(limit), ..Drawer::default() }, cx);
        c.open_bar(Bar::Usage, cx);
    });
    at_width(cx, 600.);
    let foot = rect(cx, "dock-foot");
    let (resume, cancel, back) = (rect(cx, "limit-resume"), rect(cx, "limit-cancel"), rect(cx, "dock-back"));
    assert_eq!((resume.left() - foot.left(), foot.right() - resume.right()), (px(20.), px(20.)));
    assert_eq!(cancel.size.width, resume.size.width, "both across the foot");
    assert!(resume.bottom() <= cancel.top() && cancel.bottom() <= back.top(), "Resume, Cancel, then Write a message");
    assert_eq!(back.center().x, foot.center().x);
}

/// Under 640 px a dock row's field stands under its words across the row (PickLists.tsx `Choice`: the slot under the
/// words with `max-sm:pl-7`, the field `max-sm:w-full`).
#[gpui_kit::test]
fn under_640_px_a_dock_rows_field_stands_under_its_words_full_width(cx: &mut TestAppContext) {
    let cx = open("prompt-dock", cx);
    at_width(cx, 600.);
    cx.simulate_keystrokes("2");
    cx.run_until_parked();
    let (field, dock) = (rect(cx, "control"), rect(cx, "prompt-dock"));
    // The body's 20 px side, the card's 1 px edge, the row's 20 px inset, then the slot's 28 px indent.
    assert_eq!(field.left() - dock.left(), px(20. + 1. + 20. + 28.), "under the words, past the radio");
    assert_eq!(dock.right() - field.right(), px(41.), "to the row's right edge");
}

/// The full box's bar holds the access beside the model and the effort while the surface inside the box's 1 px ring is
/// wider than 640 px (ChatComposer.tsx `useWiderThan(surfaceRef, ACCESS_IN_BAR_PX)`), so a box 642 px wide or narrower,
/// as the right panel leaves the centre column, puts it in the tray under the box.
#[gpui_kit::test]
fn the_access_picker_waits_under_a_box_642_px_wide_or_narrower(cx: &mut TestAppContext) {
    let cx = open("new-thread", cx);
    // The window's width, the box it draws, and whether the picker stands in the box's bar.
    for (window, boxed, in_bar) in
        [(1024., 768., true), (700., 660., true), (683., 643., true), (682., 642., false), (681., 641., false), (600., 576., false)]
    {
        at_width(cx, window);
        let b = rect(cx, "composer-box");
        assert_eq!(f32::from(b.size.width), boxed, "the box in a {window} px window");
        if in_bar {
            let picker = rect(cx, "picker-access");
            assert!(
                picker.left() >= b.left() && picker.right() <= b.right() && picker.top() >= b.top() && picker.bottom() <= b.bottom(),
                "a {boxed} px box holds the access {picker:?} inside {b:?}"
            );
            assert!(cx.debug_bounds("tray-access").is_none(), "a {boxed} px box draws no access in the tray");
        } else {
            let tray = rect(cx, "tray-access");
            assert!(tray.top() >= b.bottom(), "a {boxed} px box puts the access {tray:?} under its foot at {:?}", b.bottom());
            assert!(cx.debug_bounds("picker-access").is_none(), "a {boxed} px box draws no access in its bar");
        }
    }
}

/// Catches: the docked composer's blur on the Mac laid over nothing, which on the glass lets the transcript under the
/// box show through sharp (`.desktop-mac`'s `--glass-ground`), or off its radius and saturation.
#[gpui_kit::test]
fn on_the_mac_the_box_blurs_what_is_under_it_over_the_glass_ground(cx: &mut TestAppContext) {
    use desktop_gpui::screens::app_shell::material::Look;
    let cx = open("empty", cx);
    let glass = Look { mac: true, glass: true };
    for (theme, look, radius, saturation, ground) in [
        ("graphite", glass, 16., 1.08, Some(0x232929)),
        ("paper", glass, 12., 1.14, Some(0xecedec)),
        ("graphite", Look { mac: true, glass: false }, 16., 1.08, Some(0x232929)),
        ("graphite", Look { mac: false, glass: true }, 16., 1.08, None),
    ] {
        cx.update(|window, cx| {
            desktop_gpui::ui::theme::set_theme_in(theme, look, cx);
            window.refresh();
        });
        cx.run_until_parked();
        let shown = rect(cx, "composer-box");
        let blurs = cx.update(|window, _| {
            let shown = shown.scale(window.scale_factor());
            window
                .painted_backdrop_blurs()
                .into_iter()
                .filter(|b| b.bounds.intersect(&shown) == b.bounds && b.bounds.size.height > shown.size.height / 2.)
                .collect::<Vec<_>>()
        });
        let [blur] = blurs[..] else { panic!("{theme} {look:?}: the box paints {} blurs", blurs.len()) };
        let scale = cx.update(|window, _| window.scale_factor());
        assert_eq!((blur.blur_radius.0, blur.saturation), (radius * scale, saturation), "{theme} {look:?}: today's radius and saturation");
        let want = ground.map_or(0, |g| gpui_kit::pack_ground(gpui_kit::rgb(g).into()));
        assert_eq!(blur.ground, want, "{theme} {look:?}: the copy's ground is {:#010x}", blur.ground);
    }
}
