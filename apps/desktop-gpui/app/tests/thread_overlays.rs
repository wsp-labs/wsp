// SPDX-License-Identifier: AGPL-3.0-only
//! The overlays over the transcript: find in thread, the Quote button over a selection in a reply, the scroll-to-end
//! pill, the rail's hover card and keys, and the shimmer on a live line.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys;
use desktop_gpui::screens::thread_view::find;
use desktop_gpui::screens::Pane;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::thread_view::{ThreadView, ThreadViewEvent};
use desktop_gpui::transcript::RowKind;
use gpui_kit::component::Root;
use gpui_kit::{
    div, point, prelude::*, px, size, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, Modifiers, MouseButton, Pixels,
    TestAppContext, VisualTestContext, Window,
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
fn the_bars_alt_chords_flip_its_toggles(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..3).flat_map(|n| turn(n, "Apples and apples, an apple.")).collect(), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("appl.");
    frames(cx, 2);
    assert_eq!(status(&view, cx).as_deref(), Some("No results"));
    let chord = |key: &str| if cfg!(target_os = "macos") { format!("cmd-alt-{key}") } else { format!("alt-{key}") };
    for (key, want, what) in [
        ("r", "9 of 9", "Use regular expression"),
        ("w", "3 of 3", "Match whole word"),
        ("w", "9 of 9", "Match whole word off"),
        ("c", "6 of 6", "Match case"),
    ] {
        cx.simulate_keystrokes(&chord(key));
        frames(cx, 2);
        assert_eq!(status(&view, cx).as_deref(), Some(want), "the chord for {what}");
    }
}

#[gpui_kit::test]
fn a_narrow_window_puts_the_toggles_on_a_row_of_their_own(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 600.);
    load(&view, apples(3), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    frames(cx, 2);
    let below = |cx: &mut VisualTestContext| {
        let toggle = cx.debug_bounds("find-toggle-0").expect("the match case toggle was not drawn");
        let count = cx.debug_bounds("find-count").expect("the count was not drawn");
        toggle.top() >= count.bottom()
    };
    assert!(below(cx), "at 600 px the toggles stand in the field's row");
    cx.simulate_resize(size(px(900.), px(600.)));
    frames(cx, 2);
    assert!(!below(cx), "at 900 px the toggles left the field's row");
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
fn switching_threads_keeps_find_and_puts_the_rail_and_quote_away(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..20).flat_map(|n| turn(n, &format!("The quick brown fox jumps over the lazy dog {n}."))).collect(), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("fox");
    frames(cx, 3);
    let case = cx.debug_bounds("find-toggle-0").expect("the match case toggle was not drawn");
    cx.simulate_click(center(case), Modifiers::none());
    frames(cx, 2);
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
        (v.find_state().is_open(), v.rail_active(), v.quote_shown())
    });
    assert_eq!(after, (true, None, None), "find did not stay open, or the last thread's rail or Quote outlived the switch");
    let next: Vec<SessionEvent> = (0..5).flat_map(|n| turn(n, if n % 2 == 0 { "A fox ran." } else { "A Fox ran." })).collect();
    view.update(cx, |v, cx| v.load(next, cx));
    frames(cx, 3);
    assert!(cx.debug_bounds("find-bar").is_some(), "the find bar was not drawn over the next thread");
    assert_eq!(
        status(&view, cx).as_deref(),
        Some("3 of 3"),
        "the next thread was not searched with the same query and Match case from its newest match"
    );
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
fn a_capped_search_keeps_the_matches_nearest_the_reader(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..40).flat_map(|n| turn(n, &"x ".repeat(300))).collect(), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("x");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("9,999 of 9,999+"));
    let newest = view.read_with(cx, |v, _| v.rows().iter().rfind(|r| matches!(r.kind, RowKind::Agent { .. })).map(|r| r.key.clone()));
    let current = view.read_with(cx, |v, _| v.find_state().current().map(|hit| hit.key));
    assert_eq!(current, newest, "the count stopped before the reply the reader sees");
}

#[gpui_kit::test]
fn a_fresh_search_picks_the_match_above_a_fold_the_reader_sees_last(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, (0..12).flat_map(|n| folded(n, &format!("An apple {n}."))).collect(), cx);
    let fold = view.read_with(cx, |v, _| {
        v.rows().iter().enumerate().filter(|(_, r)| matches!(r.kind, RowKind::Fold { .. })).nth(5).map(|(ix, _)| ix).unwrap()
    });
    view.update(cx, |v, cx| {
        let y = v.list().viewport_bounds().size.height - v.composer_inset() - px(10.);
        v.reveal(fold, y, cx);
    });
    frames(cx, 8);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("apple");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("5 of 12"), "the pick is not the newest match above turn 5's fold");
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
fn a_step_to_a_match_in_a_clamped_message_opens_it_whole(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    let long = format!("{}\n\nThe last line names the zebra.", "A line of the long message.\n".repeat(40));
    let mut events = turn(0, "Answer 0.");
    events.extend([
        event("u1", "session.start", 11, serde_json::json!({"prompt": long})),
        event("u1", "session.delta", 12, serde_json::json!({"kind": "text", "text": "Read it."})),
        event("u1", "session.done", 13, serde_json::json!({"result": {"status": "completed"}})),
        event("u1", "session.end", 14, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ]);
    load(&view, events, cx);
    let full = |v: &ThreadView| v.rows().iter().any(|r| matches!(r.kind, RowKind::Person { full: true, .. }));
    assert!(!view.read_with(cx, |v, _| full(v)), "the long message stands whole before any step");
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("zebra");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("1 of 1"), "find did not search the clamped message");
    cx.simulate_keystrokes("enter");
    frames(cx, 12);
    assert!(view.read_with(cx, |v, _| full(v)), "the step left the match under the message's clamp");
}

#[gpui_kit::test]
fn a_step_to_a_deep_match_keeps_its_line_clear_of_the_composer(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    let long = (0..80).map(|i| format!("Paragraph {i} says nothing much.")).collect::<Vec<_>>().join("\n\n");
    let mut events = apples(2);
    events.extend(turn(2, &format!("{long}\n\nThe pear is here.\n\nStill nothing.")));
    events.extend((3..9).flat_map(|n| turn(n, &format!("Answer {n} about apples."))));
    load(&view, events, cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("pear");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("1 of 1"));
    cx.simulate_keystrokes("enter");
    frames(cx, 30);
    let (row, viewport, inset) = view.read_with(cx, |v, _| {
        let ix = v.rows().iter().position(|r| r.text.contains("pear")).unwrap();
        (v.list().bounds_for_item(ix), v.list().viewport_bounds(), v.composer_inset())
    });
    let row = row.expect("the reply was not drawn");
    // The match's paragraph is the reply's last but one: below it stand one paragraph, the meta line and the row's foot.
    let line = row.bottom() - px(75.);
    assert!(
        line <= viewport.bottom() - inset,
        "the match's line ends at about {line:?}, under the composer from {:?}",
        viewport.bottom() - inset
    );
    assert!(row.top() < viewport.top(), "the reply's top stands at {:?}, so the step did not go deep", row.top());
}

#[gpui_kit::test]
fn find_searches_plans_and_subagent_lines_and_a_step_opens_what_holds_them(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    let plan: String = std::iter::once("# Fix the build\n\n".to_owned())
        .chain((0..30).map(|n| if n == 28 { "- weigh the kiwi\n".to_owned() } else { format!("- step {n}\n") }))
        .collect();
    let task = "{\"description\":\"Survey the code\",\"prompt\":\"look\"}";
    let mut events = apples(2);
    events.extend([
        event("u2", "session.start", 21, serde_json::json!({"prompt": "plan it"})),
        event("u2", "session.plan", 22, serde_json::json!({"text": plan})),
        event("u2", "session.delta", 23, serde_json::json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": task})),
        event("u2", "session.delta", 24, serde_json::json!({"kind": "text", "text": "Reading the kiwi notes.", "parentToolUseId": "t"})),
        event("u2", "session.delta", 25, serde_json::json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it."})),
        event("u2", "session.delta", 26, serde_json::json!({"kind": "text", "text": "Here it is."})),
        event("u2", "session.done", 27, serde_json::json!({"result": {"status": "completed"}})),
        event("u2", "session.end", 28, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ]);
    load(&view, events, cx);
    let open = |v: &ThreadView| {
        let plan = v.rows().iter().any(|r| matches!(r.kind, RowKind::Plan { open: true, .. }));
        let sub = v.rows().iter().any(|r| matches!(r.kind, RowKind::Subagent { open: true, .. }));
        (plan, sub)
    };
    assert_eq!(view.read_with(cx, |v, _| open(v)), (false, false), "the plan and the subagent start shut");
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("kiwi");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("2 of 2"), "the plan and the subagent's line are searched");
    cx.simulate_keystrokes("enter");
    frames(cx, 6);
    cx.simulate_keystrokes("enter");
    frames(cx, 6);
    assert_eq!(view.read_with(cx, |v, _| open(v)), (true, true), "a step opens the plan's fold and the subagent's");
    // A subagent's line is plain text, which its row paints from the marks find hands it.
    let (key, line) = view.read_with(cx, |v, _| {
        let row = v.rows().iter().find(|r| matches!(r.kind, RowKind::Subagent { .. })).unwrap();
        let RowKind::Subagent { lines, .. } = &row.kind else { unreachable!() };
        (row.key.clone(), lines.iter().position(|l| l.text.contains("kiwi")).unwrap())
    });
    let part = find::subagent_part(line, false);
    let marks = view.read_with(cx, |v, cx| find::marks(v, &key, &part, "Reading the kiwi notes.", cx));
    assert_eq!(marks.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(), vec![12..16], "the line's match is marked");
}

#[gpui_kit::test]
fn a_live_event_under_the_open_bar_reads_only_the_rows_it_changed(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, apples(50), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("apples");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("50 of 50"));
    let before = view.read_with(cx, |v, _| v.find_state().reads());
    for event in turn(50, "Answer 50 about apples.") {
        view.update(cx, |v, cx| v.push(event, cx));
        frames(cx, 1);
    }
    let read = view.read_with(cx, |v, _| v.find_state().reads()) - before;
    assert_eq!(status(&view, cx).as_deref().map(|s| s.ends_with("of 51")), Some(true), "the new reply is counted");
    assert!(read <= 12, "four live events read {read} rows again, not only the new turn's");
}

#[gpui_kit::test]
fn find_reads_the_rows_again_when_a_subagents_page_takes_their_place(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    let task = "{\"description\":\"Survey the code\",\"prompt\":\"look\"}";
    let mut events = apples(3);
    events.extend([
        event("u3", "session.start", 31, serde_json::json!({"prompt": "look around"})),
        event("u3", "session.delta", 32, serde_json::json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": task})),
        event("u3", "session.delta", 33, serde_json::json!({"kind": "text", "text": "Reading the pear notes.", "parentToolUseId": "t"})),
        event("u3", "session.delta", 34, serde_json::json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it."})),
        event("u3", "session.done", 35, serde_json::json!({"result": {"status": "completed"}})),
        event("u3", "session.end", 36, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ]);
    load(&view, events, cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    cx.simulate_input("apples");
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("3 of 3"));
    view.update(cx, |v, cx| v.set_page(Some("t".to_owned()), cx));
    frames(cx, 3);
    assert_eq!(status(&view, cx).as_deref(), Some("No results"), "find searched the thread's rows under the subagent's page");
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
    let scrolled = |cx: &mut VisualTestContext| view.read_with(cx, |v, _| -v.list().scroll_px_offset_for_scrollbar().y);
    let before = scrolled(cx);
    cx.simulate_click(center(pill), Modifiers::none());
    frames(cx, 3);
    let (moving, following, at_end) = view
        .read_with(cx, |v, _| (-v.list().scroll_px_offset_for_scrollbar().y, v.list().is_following_tail(), v.list().is_scrolled_to_end()));
    assert!(
        moving > before && !following && at_end == Some(false),
        "the pill jumped to the end instead of scrolling there: {before:?} to {moving:?}"
    );
    frames(cx, 60);
    assert!(view.read_with(cx, |v, _| v.list().is_following_tail()), "the pill did not scroll to the end");
    assert_eq!(view.read_with(cx, |v, _| v.list().is_scrolled_to_end()), Some(true));
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
    let mut now = 4_000.;
    cx.update(|_, cx| cx.set_global(desktop_gpui::ui::FrozenNow(now)));
    let mut events = apples(2);
    events.push(event("live", "session.start", 100, serde_json::json!({"prompt": "Keep going", "at": 1_000.0})));
    load(&view, events, cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    frames(cx, 2);
    let built = view.read_with(cx, |v, _| v.built());
    desktop_gpui::frames::take();
    for _ in 0..3 {
        now += 1_000.;
        cx.update(|_, cx| cx.set_global(desktop_gpui::ui::FrozenNow(now)));
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    assert_eq!(desktop_gpui::frames::take().get("Clock"), Some(&3), "the clock did not tick once a second");
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

/// Hovers `selector` past the tooltip's delay and says whether `shown` is drawn, then moves the pointer off.
fn tooltip_shows(cx: &mut VisualTestContext, selector: &'static str, shown: &'static str) -> bool {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("no {selector}")).center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(600));
    frames(cx, 2);
    let drawn = cx.debug_bounds(shown).is_some();
    cx.simulate_mouse_move(point(px(2.), px(590.)), None, Modifiers::none());
    frames(cx, 2);
    drawn
}

#[gpui_kit::test]
fn finds_tooltips_name_their_keys_as_todays_bar_does(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, 900.);
    load(&view, apples(3), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    frames(cx, 2);
    let mac = cfg!(target_os = "macos");
    let tips = [
        ("find-toggle-0", if mac { "find-chord:⌥⌘C" } else { "find-chord:Alt+C" }),
        ("find-toggle-1", if mac { "find-chord:⌥⌘W" } else { "find-chord:Alt+W" }),
        ("find-toggle-2", if mac { "find-chord:⌥⌘R" } else { "find-chord:Alt+R" }),
        ("find-older", "find-chord:Enter"),
        ("find-newer", if mac { "find-chord:⇧Enter" } else { "find-chord:Shift+Enter" }),
        ("find-close", "find-chord:Esc"),
    ];
    for (button, chord) in tips {
        assert!(tooltip_shows(cx, button, chord), "{button}'s tooltip names no {chord}");
    }
}

/// The frame with room above the thread, as the shell's header row leaves, so a tooltip over Find's bar fits either way.
struct Lowered(Entity<ThreadView>);

impl Render for Lowered {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().pt(px(120.)).key_context(format!("{} {}", keys::SHELL, keys::THREAD_OPEN).as_str()).child(self.0.clone())
    }
}

/// Each of Find's tooltips opens under its button, as today's bar puts them all (`side="bottom"` in `FindBar.tsx`),
/// though there is room above; each is found by the chord it names.
#[gpui_kit::test]
fn finds_tooltips_open_below_their_buttons(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        cx.bind_keys(keys::bindings(cfg!(target_os = "macos"), &BTreeMap::new()));
    });
    let view = cx.new(ThreadView::new);
    let frame = cx.new(|_| Lowered(view.clone()));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(frame, window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    load(&view, apples(3), cx);
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    frames(cx, 2);
    let mac = cfg!(target_os = "macos");
    let tips = [
        ("find-toggle-0", if mac { "find-chord:⌥⌘C" } else { "find-chord:Alt+C" }),
        ("find-toggle-1", if mac { "find-chord:⌥⌘W" } else { "find-chord:Alt+W" }),
        ("find-toggle-2", if mac { "find-chord:⌥⌘R" } else { "find-chord:Alt+R" }),
        ("find-older", "find-chord:Enter"),
        ("find-newer", if mac { "find-chord:⇧Enter" } else { "find-chord:Shift+Enter" }),
        ("find-close", "find-chord:Esc"),
    ];
    for (button, chord) in tips {
        let at = cx.debug_bounds(button).unwrap_or_else(|| panic!("no {button}"));
        assert!(at.top() > px(120.), "{button} has room above it: {at:?}");
        cx.simulate_mouse_move(at.center(), None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(600));
        frames(cx, 2);
        let tip = cx.debug_bounds(chord).unwrap_or_else(|| panic!("{button}'s tooltip did not show"));
        assert!(tip.top() > at.bottom(), "{button}'s tooltip opens below it: {at:?} {tip:?}");
        cx.simulate_mouse_move(point(px(2.), px(590.)), None, Modifiers::none());
        frames(cx, 2);
    }
}

/// The thread view beside a field standing for the composer, which can hold focus as Find opens.
struct WithField(Entity<ThreadView>, FocusHandle);

impl Render for WithField {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .key_context(format!("{} {}", keys::SHELL, keys::THREAD_OPEN).as_str())
            .child(div().id("field").track_focus(&self.1).h(px(20.)).child("field"))
            .child(self.0.clone())
    }
}

#[gpui_kit::test]
fn escape_from_find_gives_focus_back_to_what_held_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        cx.bind_keys(keys::bindings(cfg!(target_os = "macos"), &BTreeMap::new()));
    });
    let view = cx.new(ThreadView::new);
    let field = cx.update(|cx| cx.focus_handle());
    let frame = cx.new(|_| WithField(view.clone(), field.clone()));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(frame, window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    load(&view, apples(3), cx);
    cx.update(|window, cx| field.focus(window, cx));
    view.update_in(cx, find::open);
    frames(cx, 2);
    assert!(view.read_with(cx, |v, _| v.find_state().is_open()), "Find did not open");
    cx.simulate_keystrokes("escape");
    frames(cx, 2);
    assert!(!view.read_with(cx, |v, _| v.find_state().is_open()), "Escape did not close the bar");
    assert!(cx.update(|window, _| field.is_focused(window)), "Escape left the field that held focus as Find opened");
    focus(&view, cx);
    cx.simulate_keystrokes(FIND);
    frames(cx, 2);
    cx.simulate_keystrokes("escape");
    frames(cx, 2);
    let transcript = view.read_with(cx, |v, cx| v.focus_handle(cx));
    assert!(
        cx.update(|window, _| transcript.is_focused(window)),
        "with no screen to hand it to the composer, Find opened from the transcript did not give it focus back"
    );
}

#[gpui_kit::test]
fn the_slate_line_opens_the_slate_tab(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let home = tempfile::tempdir().unwrap();
    let env = [("HOME".to_owned(), home.path().to_string_lossy().into_owned())].into_iter().collect();
    let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap());
    let state = home.path().join("state.json");
    let mut made = None;
    cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        made = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    let shell = made.unwrap();
    let view = cx.new(ThreadView::new);
    let frame = cx.new(|_| Frame(view.clone()));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(frame, window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    let events = vec![
        event("u1", "session.start", 1, serde_json::json!({"prompt": "Build it"})),
        event("u1", "session.delta", 2, serde_json::json!({"kind": "text", "text": "Built it."})),
        event("u1", "session.slate", 3, serde_json::json!({"cause": "write", "version": 1, "by": "agent", "pieces": []})),
        event("u1", "session.done", 4, serde_json::json!({"result": {"status": "completed"}})),
        event("u1", "session.end", 5, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ];
    load(&view, events, cx);
    let active = |cx: &mut VisualTestContext| {
        cx.read(|cx| shell.read(cx).panel().read(cx).active().filter(|_| shell.read(cx).panel().read(cx).is_open()))
    };
    assert_ne!(active(cx), Some(Pane::Slate), "the Slate tab stood open before the press");
    let line = cx.debug_bounds("slate-updated").expect("no Updated the slate line");
    cx.simulate_click(line.center(), Modifiers::none());
    frames(cx, 2);
    assert_eq!(active(cx), Some(Pane::Slate), "the press on Updated the slate opened no Slate tab");
}

#[gpui_kit::test]
fn escape_from_find_with_nothing_focused_puts_the_caret_in_the_composer(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        runtime.block_on(wsp_host_client::HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let listed = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread listed");
    let (workspace_id, thread_id) = (listed.workspace_id.clone(), listed.thread_id.clone().expect("a thread id"));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, env) = (host.state(), host.env());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| Shell::new(state, env, runtime.clone(), Launch::default(), window, cx));
        made = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    let shell = made.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    let until = |cx: &mut VisualTestContext, what: &str, done: &dyn Fn(&mut VisualTestContext) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !done(cx) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
            cx.run_until_parked();
        }
    };
    until(cx, "the host", &|cx| shell.read_with(cx, |s, _| s.connected()));
    let route = desktop_gpui::screens::Route::Thread { workspace_id, thread_id };
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    until(cx, "the thread screen", &|cx| {
        shell.read_with(cx, |s, cx| s.thread_screen().is_some_and(|t| t.read(cx).view().read(cx).loaded()))
    });
    let screen = shell.read_with(cx, |s, _| s.thread_screen()).unwrap();
    let (view, composer) = screen.read_with(cx, |s, _| (s.view().clone(), s.composer().clone()));
    until(cx, "the composer's link", &|cx| composer.read_with(cx, |c, _| c.hold().is_none()));
    let field = composer.read_with(cx, |c, cx| c.focus_handle(cx));
    let transcript = view.read_with(cx, |v, cx| v.focus_handle(cx));
    // With nothing focused, and with only the transcript's page focused, as a click on it leaves today's body.
    for focus in [None, Some(transcript)] {
        cx.update(|window, cx| match &focus {
            Some(handle) => handle.focus(window, cx),
            None => window.blur(cx),
        });
        view.update_in(cx, find::open);
        frames(cx, 2);
        assert!(view.read_with(cx, |v, _| v.find_state().is_open()), "Find did not open");
        cx.simulate_keystrokes("escape");
        frames(cx, 2);
        assert!(!view.read_with(cx, |v, _| v.find_state().is_open()), "Escape did not close the bar");
        let page = focus.is_some();
        assert!(
            cx.update(|window, _| field.is_focused(window)),
            "Escape out of Find (page focused: {page}) left the composer without the caret"
        );
    }
}

#[gpui_kit::test]
fn a_reveal_stands_the_first_rows_own_top_at_its_place_as_todays_list_does(cx: &mut TestAppContext) {
    // Today's list draws its top spacer apart from the first row, so a place read off today's row is the row's own top.
    let (view, cx) = window(cx, 900.);
    load(&view, apples(30), cx);
    let stood = |ix: usize, cx: &mut VisualTestContext| {
        view.update(cx, |v, cx| v.reveal(ix, px(-40.), cx));
        frames(cx, 8);
        let selector: &'static str = Box::leak(format!("row-{ix}").into_boxed_str());
        cx.debug_bounds(selector).unwrap_or_else(|| panic!("no {selector}")).top()
    };
    let second = stood(1, cx);
    let first = stood(0, cx);
    assert_eq!(first + px(16.), second, "the first row's own top, under the list's 16 px spacer, stands off the place asked");
}
