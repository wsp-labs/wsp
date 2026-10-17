// SPDX-License-Identifier: AGPL-3.0-only
//! The overlays over the transcript: find in thread, the Quote button over a selection in a reply, the scroll-to-end
//! pill, the rail's hover card and keys, and the shimmer on a live line.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys;
use desktop_gpui::thread_view::{ThreadView, ThreadViewEvent};
use desktop_gpui::transcript::RowKind;
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Bounds, Context, Entity, Focusable, Modifiers, MouseButton, Pixels, TestAppContext,
    VisualTestContext, Window,
};
use wsp_host_client::protocol::SessionEvent;

fn event(turn: &str, kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
    let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": turn, "type": kind, "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

/// A settled turn: the person's question and one reply.
fn turn(n: usize, reply: &str) -> Vec<SessionEvent> {
    let (t, pos) = (format!("u{n}"), 10 * n as u64 + 1);
    vec![
        event(&t, "session.start", pos, serde_json::json!({"prompt": format!("Question {n}")})),
        event(&t, "session.delta", pos + 1, serde_json::json!({"kind": "text", "text": reply})),
        event(&t, "session.done", pos + 2, serde_json::json!({"result": {"status": "completed"}})),
        event(&t, "session.end", pos + 3, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ]
}

/// A settled turn whose first reply its fold hides: a reply, a call, then the last reply.
fn folded(n: usize, hidden: &str) -> Vec<SessionEvent> {
    let (t, pos) = (format!("u{n}"), 10 * n as u64 + 1);
    vec![
        event(&t, "session.start", pos, serde_json::json!({"prompt": format!("Question {n}")})),
        event(&t, "session.delta", pos + 1, serde_json::json!({"kind": "text", "text": hidden, "messageId": "m1"})),
        event(
            &t,
            "session.delta",
            pos + 2,
            serde_json::json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"ls\"}"}),
        ),
        event(&t, "session.delta", pos + 3, serde_json::json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
        event(&t, "session.delta", pos + 4, serde_json::json!({"kind": "text", "text": "All done.", "messageId": "m2"})),
        event(&t, "session.done", pos + 5, serde_json::json!({"result": {"status": "completed"}})),
        event(&t, "session.end", pos + 6, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ]
}

/// `turns` settled turns, each reply naming apples once.
fn apples(turns: usize) -> Vec<SessionEvent> {
    (0..turns).flat_map(|n| turn(n, &format!("Answer {n} about apples."))).collect()
}

/// The shell's root as far as the thread's keys read it: the contexts it sets while a thread is open.
struct Frame(Entity<ThreadView>);

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().key_context(format!("{} {}", keys::SHELL, keys::THREAD_OPEN).as_str()).child(self.0.clone())
    }
}

fn window(cx: &mut TestAppContext, width: f32) -> (Entity<ThreadView>, &mut VisualTestContext) {
    window_with(cx, width, &[])
}

/// A window whose keys are the shell's with `overrides` over them.
fn window_with<'a>(cx: &'a mut TestAppContext, width: f32, overrides: &[(&str, &str)]) -> (Entity<ThreadView>, &'a mut VisualTestContext) {
    let overrides: BTreeMap<String, Option<String>> = overrides.iter().map(|(c, k)| (c.to_string(), Some(k.to_string()))).collect();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        cx.bind_keys(keys::bindings(cfg!(target_os = "macos"), &overrides));
    });
    let view = cx.new(ThreadView::new);
    let frame = cx.new(|_| Frame(view.clone()));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(frame, window, cx));
    cx.simulate_resize(size(px(width), px(600.)));
    (view, cx)
}

fn load(view: &Entity<ThreadView>, events: Vec<SessionEvent>, cx: &mut VisualTestContext) {
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
        v.scroll_to_end(cx);
    });
    frames(cx, 4);
}

/// Delivers `n` frames: what each asked of the next one, then everything that follows from it.
fn frames(cx: &mut VisualTestContext, n: usize) {
    for _ in 0..n {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        cx.run_until_parked();
    }
}

fn focus(view: &Entity<ThreadView>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.focus(&view.focus_handle(cx), cx));
    cx.run_until_parked();
}

const FIND: &str = if cfg!(target_os = "macos") { "cmd-f" } else { "ctrl-f" };
const FIND_OLDER: &str = if cfg!(target_os = "macos") { "cmd-g" } else { "ctrl-g" };

fn status(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |v, _| v.find_state().status())
}

fn center(b: Bounds<Pixels>) -> gpui_kit::Point<Pixels> {
    b.center()
}

#[gpui_kit::test]
fn find_opens_on_its_key_counts_steps_both_ways_and_closes_on_escape(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, apples(30), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.find_state().is_open()), "the find key did not open the bar");
    assert!(cx.debug_bounds("find-bar").is_some(), "the bar was not drawn");

    cx.simulate_input("apples");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("30 of 30"), "a fresh search picks the newest match the reader sees");
    cx.simulate_keystrokes("enter");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("29 of 30"), "Enter steps to the older match");
    cx.simulate_keystrokes("shift-enter");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("30 of 30"), "Shift+Enter steps to the newer match");
    cx.simulate_keystrokes("shift-enter");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("1 of 30"), "a step past the newest wraps to the oldest");
    cx.simulate_keystrokes(FIND_OLDER);
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("30 of 30"), "the shell's older chord steps while the bar is open");
    cx.simulate_keystrokes("shift-enter");
    frames(cx, 3);

    let older = cx.debug_bounds("find-older").expect("the older button was not drawn");
    cx.simulate_click(center(older), Modifiers::none());
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("30 of 30"), "the older button steps back");

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!view.read_with(cx, |v, _| v.find_state().is_open()), "Escape did not close the bar");
    assert!(cx.debug_bounds("find-bar").is_none(), "the bar is still drawn");
}

#[gpui_kit::test]
fn find_opens_on_the_chord_the_person_bound_and_not_on_the_default(cx: &mut TestAppContext) {
    let (view, cx) = window_with(cx, 900., &[("thread.find", "mod+e")]);
    load(&view, apples(3), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.run_until_parked();
    assert!(!view.read_with(cx, |v, _| v.find_state().is_open()), "the default chord opened find after it was rebound");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-e" } else { "ctrl-e" });
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.find_state().is_open()), "the rebound chord did not open find");
}

#[gpui_kit::test]
fn find_survives_the_transcript_read_again_shorter_while_open(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, apples(30), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("apples");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("30 of 30"));
    view.update(cx, |v, cx| v.load(apples(2), cx));
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("2 of 2"), "find did not count the transcript read again");
}

#[gpui_kit::test]
fn a_reveal_the_list_cannot_reach_stops_asking_for_frames(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, apples(30), cx);
    view.update(cx, |v, cx| {
        let (last, height) = (v.rows().len() - 1, v.list().viewport_bounds().size.height);
        v.reveal(last, height * 0.1, cx);
    });
    frames(cx, 8);
    let renders = view.read_with(cx, |v, _| v.renders());
    frames(cx, 10);
    assert_eq!(view.read_with(cx, |v, _| v.renders()), renders, "the reveal kept drawing the view with the list at its end");
}

#[gpui_kit::test]
fn switching_threads_puts_find_the_rail_and_quote_away(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..20).flat_map(|n| turn(n, &format!("The quick brown fox jumps over the lazy dog {n}."))).collect(), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("fox");
    frames(cx, 3);
    let ix = view.read_with(cx, |v, _| v.rows().iter().rposition(|r| matches!(r.kind, RowKind::Agent { .. })).unwrap());
    let row = cx.debug_bounds(Box::leak(format!("row-{ix}").into_boxed_str())).expect("the reply was not drawn");
    let (from, to) = (point(row.left() + px(6.), row.top() + px(12.)), point(row.left() + px(120.), row.top() + px(12.)));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    frames(cx, 2);
    let rail_focus = view.read_with(cx, |v, _| v.rail_focus()).expect("the rail has no focus");
    cx.update(|window, cx| window.focus(&rail_focus, cx));
    cx.simulate_keystrokes("end");
    frames(cx, 1);
    let before = view.read_with(cx, |v, _| (v.find_state().is_open(), v.rail_active(), v.quote_shown().is_some()));
    assert_eq!(before, (true, Some(19), true), "find, the rail and Quote were not all up before the switch");

    let after = view.update(cx, |v, cx| {
        v.show("t2", cx);
        (v.find_state().is_open(), v.find_state().status(), v.rail_active(), v.quote_shown())
    });
    assert_eq!(after, (false, None, None, None), "the last thread's find, rail or Quote outlived the switch");
    view.update(cx, |v, cx| v.load(apples(3), cx));
    frames(cx, 3);
    assert!(cx.debug_bounds("find-bar").is_none(), "the find bar is still drawn over the next thread");
}

#[gpui_kit::test]
fn find_paints_every_match_on_screen_and_the_current_one_apart(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..3).flat_map(|n| turn(n, &format!("Apples and **apples** {n}."))).collect(), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("apples");
    frames(cx, 4);
    assert_eq!(status(&view, cx).as_deref(), Some("6 of 6"));
    let (last, first) = view.read_with(cx, |v, _| {
        let replies: Vec<String> = v.rows().iter().filter(|r| matches!(r.kind, RowKind::Agent { .. })).map(|r| r.key.clone()).collect();
        (replies[2].clone(), replies[0].clone())
    });
    let painted = view.read_with(cx, |v, _| v.find_state().painted(&last));
    assert_eq!(painted, Some((vec![0..6, 11..17], Some(1))), "the last reply's matches, the second current");
    let other = view.read_with(cx, |v, _| v.find_state().painted(&first));
    assert_eq!(other, Some((vec![0..6, 11..17], None)), "another reply's matches, none current");
}

#[gpui_kit::test]
fn a_step_opens_the_fold_hiding_its_match_and_stands_the_row_at_two_fifths(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    let mut events: Vec<SessionEvent> = (0..6).flat_map(|n| turn(n, &format!("Answer {n}.\n\nMore text\n\nAnd more"))).collect();
    events.extend(folded(6, "A note about the zebra."));
    events.extend((7..14).flat_map(|n| turn(n, &format!("Answer {n}.\n\nMore text\n\nAnd more"))));
    load(&view, events, cx);
    let hidden = |v: &ThreadView| v.rows().iter().any(|r| r.text.contains("zebra"));
    assert!(!view.read_with(cx, |v, _| hidden(v)), "the fold does not hide the first reply");
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("zebra");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("1 of 1"), "find did not search the folded turn");
    cx.simulate_keystrokes("enter");
    frames(cx, 12);
    assert!(view.read_with(cx, |v, _| hidden(v)), "the step did not open the fold");
    let (ix, top, height) = view.read_with(cx, |v, _| {
        let ix = v.rows().iter().position(|r| r.text.contains("zebra")).unwrap();
        let viewport = v.list().viewport_bounds();
        (ix, v.list().bounds_for_item(ix).map(|b| b.top() - viewport.top()), viewport.size.height)
    });
    let top = top.unwrap_or_else(|| panic!("row {ix} was not drawn"));
    assert!((top - height * 0.4).abs() < px(1.), "the row stands at {top:?} of {height:?}");
}

#[gpui_kit::test]
fn a_selection_in_a_reply_shows_quote_and_quote_hands_over_the_webs_words(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..2).flat_map(|n| turn(n, &format!("The quick brown fox jumps over the lazy dog {n}."))).collect(), cx);
    let quoted: Rc<RefCell<Vec<ThreadViewEvent>>> = Rc::default();
    let got = quoted.clone();
    cx.update(|_, cx| cx.subscribe(&view, move |_, e: &ThreadViewEvent, _| got.borrow_mut().push(e.clone())).detach());
    let ix = view.read_with(cx, |v, _| v.rows().iter().rposition(|r| matches!(r.kind, RowKind::Agent { .. })).unwrap());
    let row = cx.debug_bounds(Box::leak(format!("row-{ix}").into_boxed_str())).expect("the reply was not drawn");
    let (from, to) = (point(row.left() + px(6.), row.top() + px(12.)), point(row.left() + px(120.), row.top() + px(12.)));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    frames(cx, 2);
    let shown = view.read_with(cx, |v, _| v.quote_shown()).expect("no Quote button after a selection in a reply");
    assert!(shown.starts_with("Quoting your reply to \"Question 1\":\n> The quick"), "{shown:?}");
    let button = cx.debug_bounds("quote").expect("the Quote button was not drawn");
    assert!(
        (button.left() - to.x).abs() < px(1.) && (button.top() - to.y).abs() < px(1.),
        "the button stands at {button:?}, released at {to:?}"
    );
    cx.simulate_click(center(button), Modifiers::none());
    frames(cx, 2);
    assert_eq!(quoted.borrow().as_slice(), [ThreadViewEvent::Quote(shown)], "Quote did not hand over the quote");
    assert!(view.read_with(cx, |v, _| v.quote_shown()).is_none(), "the button stayed after Quote");
}

#[gpui_kit::test]
fn a_selection_in_a_persons_message_shows_no_quote(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, turn(0, "Short reply."), cx);
    let row = cx.debug_bounds("row-0").expect("the person's message was not drawn");
    let (from, to) = (point(row.right() - px(80.), row.top() + px(30.)), point(row.right() - px(20.), row.top() + px(30.)));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    frames(cx, 2);
    assert!(view.read_with(cx, |v, _| v.quote_shown()).is_none(), "Quote showed over a person's message");
}

#[gpui_kit::test]
fn the_pill_shows_above_the_composer_once_scrolled_up_and_scrolls_to_the_end(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, apples(60), cx);
    assert!(cx.debug_bounds("scroll-to-end").is_none(), "the pill shows at the end");
    view.update(cx, |v, cx| {
        v.list().scroll_by(px(-1500.));
        cx.notify();
    });
    frames(cx, 2);
    std::thread::sleep(Duration::from_millis(250));
    frames(cx, 2);
    let pill = cx.debug_bounds("scroll-to-end").expect("no pill once scrolled up");
    assert_eq!(pill.bottom(), px(600.) - px(110.) - px(8.), "{pill:?}");
    assert_eq!(pill.size.height, px(28.));
    cx.simulate_click(center(pill), Modifiers::none());
    frames(cx, 4);
    assert!(view.read_with(cx, |v, _| v.list().is_following_tail()), "the pill did not scroll to the end");
}

#[gpui_kit::test]
fn hovering_the_rail_widens_the_ticks_and_shows_the_turn_and_the_keys_move_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..20).flat_map(|n| turn(n, &format!("Reply {n}\nsecond line"))).collect(), cx);
    let rail = cx.debug_bounds("minimap").expect("no rail");
    cx.simulate_mouse_move(point(rail.left() + px(4.), rail.top() + px(16.)), None, Modifiers::none());
    frames(cx, 2);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), Some(2), "the pointer at the third tick");
    let card = cx.debug_bounds("minimap-preview").expect("no card on hover");
    assert_eq!(card.size.width, px(320.));
    assert_eq!(card.left(), rail.left() + px(32.));
    cx.simulate_mouse_move(point(px(450.), px(300.)), None, Modifiers::none());
    frames(cx, 2);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), None, "the card stayed after the pointer left");

    let rail_focus = view.read_with(cx, |v, _| v.rail_focus()).expect("the rail has no focus");
    cx.update(|window, cx| window.focus(&rail_focus, cx));
    frames(cx, 1);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), Some(0), "focus starts at the first message");
    cx.simulate_keystrokes("down down");
    frames(cx, 1);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), Some(2));
    cx.simulate_keystrokes("end");
    frames(cx, 1);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), Some(19));
    cx.simulate_keystrokes("home down down");
    frames(cx, 1);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), Some(2));
    cx.simulate_keystrokes("enter");
    frames(cx, 8);
    let top = view.read_with(cx, |v, _| v.list().bounds_for_item(4).map(|b| b.top() - v.list().viewport_bounds().top()));
    assert_eq!(top, Some(px(24.)), "Enter did not scroll to the third message");
}

#[gpui_kit::test]
fn the_rails_keys_hold_when_the_transcript_is_read_again_shorter(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..20).flat_map(|n| turn(n, "Reply")).collect(), cx);
    let rail_focus = view.read_with(cx, |v, _| v.rail_focus()).expect("the rail has no focus");
    cx.update(|window, cx| window.focus(&rail_focus, cx));
    cx.simulate_keystrokes("end");
    frames(cx, 1);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), Some(19));
    view.update(cx, |v, cx| v.load((0..3).flat_map(|n| turn(n, "Reply")).collect(), cx));
    frames(cx, 2);
    cx.simulate_keystrokes("enter");
    frames(cx, 2);
    cx.simulate_keystrokes("end");
    frames(cx, 1);
    assert_eq!(view.read_with(cx, |v, _| v.rail_active()), Some(2), "End did not land on the last of the three messages");
}

#[gpui_kit::test]
fn a_narrow_gutter_keeps_the_rail_inert_but_drawn(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 800.);
    load(&view, (0..6).flat_map(|n| turn(n, "Reply")).collect(), cx);
    let rail = cx.debug_bounds("minimap").expect("a narrow gutter drew no rail");
    assert_eq!(rail.size.width, px(4.), "the hit strip is the gutter less its inset: {rail:?}");
}

fn thinking() -> Vec<SessionEvent> {
    let mut events = apples(2);
    events.push(event("live", "session.start", 100, serde_json::json!({"prompt": "Keep going"})));
    events
}

fn shimmer_frames(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> usize {
    view.read_with(cx, |v, cx| v.shimmer_lines().iter().map(|s| s.read(cx).frames).sum())
}

#[gpui_kit::test]
fn the_shimmer_runs_on_its_own_frames_while_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, thinking(), cx);
    assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Thinking))), "no Thinking row");
    assert_eq!(view.read_with(cx, |v, _| v.shimmer_lines().len()), 1, "Thinking has no shimmer");
    let line = cx.debug_bounds("shimmer").expect("the shimmer was not drawn");
    assert!(line.size.width < px(160.), "the shimmer spans {:?}, not its line's own width as `w-fit` keeps it", line.size.width);
    let (start, renders, built) = (shimmer_frames(&view, cx), view.read_with(cx, |v, _| v.renders()), view.read_with(cx, |v, _| v.built()));
    let began = Instant::now();
    let mut delivered = 0;
    while began.elapsed() < Duration::from_secs(2) {
        frames(cx, 1);
        delivered += 1;
        std::thread::sleep(Duration::from_millis(16));
    }
    let drawn = shimmer_frames(&view, cx) - start;
    assert!(cx.debug_bounds("shimmer-band").is_some(), "the band was not drawn over the rows");
    let renders = view.read_with(cx, |v, _| v.renders()) - renders;
    let built = view.read_with(cx, |v, _| v.built()) - built;
    println!("over 2 s, {delivered} frames: the shimmer drew {drawn} times, the thread view {renders} times, building {built} rows");
    assert_eq!(built, 0, "the shimmer's frames built rows again");
    assert!(drawn + 2 >= delivered, "the shimmer drew {drawn} of {delivered} frames");

    cx.update(|_, cx| cx.set_reduce_motion(true));
    frames(cx, 2);
    let still = shimmer_frames(&view, cx);
    frames(cx, 10);
    assert_eq!(shimmer_frames(&view, cx), still, "the shimmer kept drawing with reduced motion");
}

#[gpui_kit::test]
fn the_working_clock_ticks_over_the_rows_without_building_them(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    let mut events = apples(2);
    events.push(event("live", "session.start", 100, serde_json::json!({"prompt": "Keep going", "at": 1_000.0})));
    load(&view, events, cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    frames(cx, 2);
    let (built, renders) = view.read_with(cx, |v, _| (v.built(), v.renders()));
    cx.executor().advance_clock(Duration::from_secs(3));
    frames(cx, 2);
    let renders = view.read_with(cx, |v, _| v.renders()) - renders;
    assert!(renders > 0, "the clock did not tick");
    assert_eq!(view.read_with(cx, |v, _| v.built()), built, "a tick built rows again");
    let slot = cx.debug_bounds("clock-slot").expect("the Working line kept no slot for its clock");
    let clock = cx.debug_bounds("clock").expect("the clock was not drawn over the rows");
    assert_eq!(clock.origin, slot.origin, "the clock stands off its slot");
}

#[gpui_kit::test]
fn the_shimmer_stops_once_scrolled_off_screen(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    let mut events = apples(40);
    events.push(event("live", "session.start", 1000, serde_json::json!({"prompt": "Keep going"})));
    load(&view, events, cx);
    assert_eq!(view.read_with(cx, |v, _| v.shimmer_lines().len()), 1);
    view.update(cx, |v, cx| v.scroll_to_top(cx));
    frames(cx, 3);
    // The scroll-to-end pill fades in over 200 ms; it is let settle so only the shimmer could ask for frames.
    std::thread::sleep(Duration::from_millis(250));
    frames(cx, 3);
    let (before, renders) = (shimmer_frames(&view, cx), view.read_with(cx, |v, _| v.renders()));
    frames(cx, 10);
    assert_eq!(shimmer_frames(&view, cx), before, "the shimmer drew while off screen");
    assert_eq!(view.read_with(cx, |v, _| v.renders()), renders, "the thread view drew again while the shimmer was off screen");
}
