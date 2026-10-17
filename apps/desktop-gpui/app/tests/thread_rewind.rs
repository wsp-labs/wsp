// SPDX-License-Identifier: AGPL-3.0-only
//! Copy, Rewind to here and Fork from here under the messages, and the Rewind dialog's words and what it sends.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use desktop_gpui::screens::thread_view::rewind::{self, as_sentence, rewind_note};
use desktop_gpui::thread_view::{ThreadView, ThreadViewEvent};
use desktop_gpui::transcript::{Can, Listing, RowKind};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Bounds, Entity, Modifiers, Pixels, TestAppContext, VisualTestContext};
use serde_json::{json, Value};
use wsp_host_client::protocol::SessionEvent;

fn event(turn: &str, kind: &str, pos: u64, extra: Value) -> SessionEvent {
    let mut v = json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": turn, "type": kind, "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

/// A settled turn on Codex, its question, one reply, and the checkpoint it kept.
fn turn(n: usize, checkpoint: Value) -> Vec<SessionEvent> {
    let (t, pos) = (format!("u{n}"), 10 * n as u64 + 1);
    vec![
        event(&t, "session.start", pos, json!({"prompt": format!("Question {n}"), "agent": "codex"})),
        event(&t, "session.delta", pos + 1, json!({"kind": "text", "text": format!("Answer {n}.")})),
        event(&t, "session.done", pos + 2, json!({"result": {"status": "completed"}})),
        event(&t, "session.checkpoint", pos + 3, checkpoint),
        event(&t, "session.end", pos + 4, json!({"exitCode": 0, "sawResult": true})),
    ]
}

/// Three settled turns, each keeping the files and an anchor; `second` is the middle turn's checkpoint.
fn three(second: Value) -> Vec<SessionEvent> {
    let kept = json!({"ref": "c", "anchor": "a"});
    [turn(0, kept.clone()), turn(1, second), turn(2, kept)].concat()
}

fn window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    cx.simulate_resize(size(px(900.), px(800.)));
    (view, cx)
}

fn load(view: &Entity<ThreadView>, events: Vec<SessionEvent>, can: Can, cx: &mut VisualTestContext) {
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
        v.set_can(can, cx);
        v.scroll_to_top(cx);
    });
    frames(cx, 4);
}

fn frames(cx: &mut VisualTestContext, n: usize) {
    for _ in 0..n {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
}

fn bounds(cx: &mut VisualTestContext, selector: String) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(Box::leak(selector.into_boxed_str()))
}

fn click(cx: &mut VisualTestContext, selector: String) {
    let at = bounds(cx, selector.clone()).unwrap_or_else(|| panic!("{selector} was not drawn")).center();
    cx.simulate_click(at, Modifiers::none());
    frames(cx, 3);
}

/// The dialog's Rewind, the last button it draws, or its Cancel, 8 px to the left of it.
fn act(cx: &mut VisualTestContext, rewind: bool) {
    assert!(bounds(cx, "alert-dialog".into()).is_some(), "the dialog is not ui::AlertDialog");
    let confirm = bounds(cx, "button".into()).expect("the dialog drew no button");
    let at = if rewind { confirm.center() } else { gpui_kit::point(confirm.left() - px(12.), confirm.center().y) };
    cx.simulate_click(at, Modifiers::none());
    frames(cx, 3);
}

fn key_of(view: &Entity<ThreadView>, text: &str, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |v, _| v.rows().iter().find(|r| r.text == text).map(|r| r.key.clone()).unwrap())
}

fn shown(cx: &mut VisualTestContext) -> Option<rewind::Shown> {
    cx.update(|_, cx| rewind::shown(cx))
}

const FORKS: Can = Can { forks: true, forks_by_count: true, rewinds_conversation: false, rewinds_by_count: false };
const CUTS: Can = Can { forks: false, forks_by_count: false, rewinds_conversation: true, rewinds_by_count: false };

#[gpui_kit::test]
fn the_buttons_stand_where_the_rows_say_in_order(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), FORKS, cx);
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    let mut rewinds = 0;
    for row in rows.iter().filter(|r| matches!(r.kind, RowKind::Person { .. } | RowKind::Agent { .. })) {
        let (rewind, fork) = match &row.kind {
            RowKind::Agent { rewind, fork, .. } => (rewind.is_some(), fork.is_some()),
            RowKind::Person { fork, .. } => (false, fork.is_some()),
            _ => unreachable!(),
        };
        rewinds += rewind as usize;
        assert_eq!(bounds(cx, format!("rewind-to-here-{}", row.key)).is_some(), rewind, "Rewind to here on {:?}", row.text);
        assert_eq!(bounds(cx, format!("fork-from-here-{}", row.key)).is_some(), fork, "Fork from here on {:?}", row.text);
        assert!(bounds(cx, format!("copy-to-clipboard-{}", row.key)).is_some(), "no Copy on {:?}", row.text);
    }
    assert_eq!(rewinds, 2, "the two earlier replies offer Rewind to here, the latest none");

    let reply = key_of(&view, "Answer 1.", cx);
    let copy = bounds(cx, format!("copy-to-clipboard-{reply}")).unwrap();
    let back = bounds(cx, format!("rewind-to-here-{reply}")).unwrap();
    let fork = bounds(cx, format!("fork-from-here-{reply}")).unwrap();
    assert_eq!(copy.size, size(px(24.), px(24.)));
    assert_eq!((back.left() - copy.right(), fork.left() - back.right()), (px(2.), px(2.)), "Copy, Rewind, Fork 2 px apart");
}

#[gpui_kit::test]
fn fork_from_here_hands_the_turn_to_the_composer(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), FORKS, cx);
    let forked = Rc::new(RefCell::new(Vec::new()));
    let heard = forked.clone();
    cx.update(|_, cx| cx.subscribe(&view, move |_, e: &ThreadViewEvent, _| heard.borrow_mut().push(e.clone())).detach());
    let (reply, first, second) = (key_of(&view, "Answer 0.", cx), key_of(&view, "Question 0", cx), key_of(&view, "Question 1", cx));
    click(cx, format!("fork-from-here-{reply}"));
    click(cx, format!("fork-from-here-{first}"));
    click(cx, format!("fork-from-here-{second}"));
    assert_eq!(
        forked.borrow().as_slice(),
        [
            ThreadViewEvent::Fork { thread: "t".into(), turn: Some("u0".into()) },
            ThreadViewEvent::Fork { thread: "t".into(), turn: None },
            ThreadViewEvent::Fork { thread: "t".into(), turn: Some("u0".into()) }
        ]
    );
}

#[gpui_kit::test]
fn copy_stands_as_a_check_for_a_second_and_a_half(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c"})), Can::default(), cx);
    let reply = key_of(&view, "Answer 1.", cx);
    click(cx, format!("copy-to-clipboard-{reply}"));
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("Answer 1."));
    assert!(bounds(cx, format!("copied-{reply}")).is_some(), "Copy did not turn into a check");
    cx.executor().advance_clock(Duration::from_millis(1_400));
    frames(cx, 1);
    assert!(bounds(cx, format!("copied-{reply}")).is_some(), "the check did not hold");
    cx.executor().advance_clock(Duration::from_millis(200));
    frames(cx, 1);
    assert!(bounds(cx, format!("copied-{reply}")).is_none(), "the check did not go back to Copy");
}

#[gpui_kit::test]
fn an_agent_that_keeps_its_history_offers_the_files_alone(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c"})), Can::default(), cx);
    let reply = key_of(&view, "Answer 1.", cx);
    click(cx, format!("rewind-to-here-{reply}"));
    let dialog = shown(cx).expect("the dialog did not open");
    assert_eq!(dialog.title, "Rewind to here?");
    assert!(dialog.choices.is_empty(), "an agent that keeps its own history offers no choice");
    assert_eq!(dialog.slot, rewind_note(1, true, false, "Codex", None), "the catalog names the agent before the host does");
    assert!(bounds(cx, "rewind-conversation".into()).is_none());
}

#[gpui_kit::test]
fn an_agent_that_cuts_its_conversation_offers_the_choice(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), CUTS, cx);
    let reply = key_of(&view, "Answer 0.", cx);
    click(cx, format!("rewind-to-here-{reply}"));
    let dialog = shown(cx).expect("the dialog did not open");
    let lines = |picked: bool| vec![("Conversation only".to_owned(), !picked, false), ("Conversation and files".to_owned(), picked, false)];
    assert_eq!(dialog.choices, lines(false));
    assert_eq!(dialog.slot, "The 2 turns after this reply leave the conversation; the files stay as they are.");
    click(cx, "rewind-files".into());
    let dialog = shown(cx).unwrap();
    assert_eq!(dialog.choices, lines(true));
    assert_eq!(dialog.slot, rewind_note(2, true, true, "Codex", None));
}

#[gpui_kit::test]
fn a_turn_that_kept_no_files_holds_the_files_line(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"anchor": "a"})), CUTS, cx);
    let reply = key_of(&view, "Answer 1.", cx);
    click(cx, format!("rewind-to-here-{reply}"));
    click(cx, "rewind-files".into());
    let dialog = shown(cx).expect("the dialog did not open");
    assert_eq!(dialog.choices[1], ("Conversation and files".to_owned(), false, true), "the held line took the press");
    assert_eq!(dialog.slot, "The turn after this reply leaves the conversation; the files stay as they are.");
}

#[gpui_kit::test]
fn a_kept_reason_says_why_the_conversation_stays(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "kept": "codex cannot cut a resumed thread"})), CUTS, cx);
    let reply = key_of(&view, "Answer 1.", cx);
    click(cx, format!("rewind-to-here-{reply}"));
    let dialog = shown(cx).expect("the dialog did not open");
    assert!(dialog.choices.is_empty(), "a thread the agent cannot cut offers no choice");
    assert!(dialog.slot.starts_with("Codex cannot cut a resumed thread, so the conversation stays"), "{}", dialog.slot);
}

#[gpui_kit::test]
fn rewind_sends_sessions_rewind_and_shows_the_refusal_in_the_slot(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), CUTS, cx);
    let reply = key_of(&view, "Answer 1.", cx);
    click(cx, format!("rewind-to-here-{reply}"));
    let opened = shown(cx).unwrap();
    assert!(!opened.refused && !opened.busy, "the dialog sent before Rewind was pressed");
    click(cx, "rewind-files".into());
    act(cx, true);
    let dialog = shown(cx).expect("a refused rewind closed the dialog");
    assert_eq!(dialog.request, json!({"threadId": "t", "turnId": "u1", "files": true}));
    assert!(dialog.refused && !dialog.busy);
    assert_eq!(dialog.slot, "The thread view follows no host.");
    act(cx, false);
    assert!(shown(cx).is_none(), "Cancel did not close the dialog");
}

#[gpui_kit::test]
fn conversation_only_sends_files_false(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), CUTS, cx);
    let reply = key_of(&view, "Answer 0.", cx);
    click(cx, format!("rewind-to-here-{reply}"));
    act(cx, true);
    let dialog = shown(cx).unwrap();
    assert_eq!(dialog.request, json!({"threadId": "t", "turnId": "u0", "files": false}));
    assert!(dialog.refused, "Rewind did not ask the host");
}

#[test]
fn the_note_says_each_case_in_the_protocol_s_words() {
    let shared = "If another thread has worked in this folder since this reply, the files stay as they are.";
    let undo = "Undo rewind puts them back until the next turn in this folder ends";
    assert_eq!(
        rewind_note(3, true, true, "Codex", Some("codex cannot cut a resumed thread")),
        format!(
            "Codex cannot cut a resumed thread, so the conversation stays and the files go back to how they stood at this reply. {shared} {undo}."
        )
    );
    assert_eq!(
        rewind_note(3, true, false, "Codex", None),
        format!(
            "Codex keeps its own history, so the conversation stays and the files go back to how they stood at this reply. {shared} {undo}."
        )
    );
    assert_eq!(
        rewind_note(1, false, true, "Claude Code", None),
        "The turn after this reply leaves the conversation; the files stay as they are."
    );
    assert_eq!(
        rewind_note(2, false, true, "Claude Code", None),
        "The 2 turns after this reply leave the conversation; the files stay as they are."
    );
    assert_eq!(
        rewind_note(1, true, true, "Claude Code", None),
        format!(
            "The turn after this reply leaves the conversation, and the files go back to how they stood at this reply. {shared} {undo}; the conversation does not come back."
        )
    );
}

#[test]
fn a_refusal_reads_as_a_sentence() {
    assert_eq!(as_sentence("the thread is working"), "The thread is working.");
    assert_eq!(as_sentence("nothing to undo!"), "Nothing to undo!");
    assert_eq!(as_sentence("Gone."), "Gone.");
}

#[gpui_kit::test]
fn the_dialog_holds_through_a_backdrop_press_and_closes_on_escape(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), CUTS, cx);
    let (reply, other) = (key_of(&view, "Answer 0.", cx), key_of(&view, "Answer 1.", cx));
    let under = bounds(cx, format!("copy-to-clipboard-{other}")).expect("no Copy to press under the dialog");
    click(cx, format!("rewind-to-here-{reply}"));
    assert!(shown(cx).is_some(), "the dialog did not open");
    cx.simulate_click(under.center(), Modifiers::none());
    frames(cx, 3);
    assert!(shown(cx).is_some(), "a press on the backdrop closed the dialog");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()), None, "the press went through the backdrop to the row under it");
    cx.simulate_keystrokes("escape");
    frames(cx, 3);
    assert!(shown(cx).is_none(), "Escape did not close the dialog");
}

#[gpui_kit::test]
fn a_capped_thread_offers_no_rewind(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), CUTS, cx);
    let reply = key_of(&view, "Answer 0.", cx);
    assert!(bounds(cx, format!("rewind-to-here-{reply}")).is_some());
    view.update(cx, |v, cx| v.set_listing(Listing { thread: "t".into(), capped: true, ..Listing::default() }, cx));
    frames(cx, 3);
    assert!(bounds(cx, format!("rewind-to-here-{reply}")).is_none(), "Rewind stands on a turn its computer holds back");
}

#[gpui_kit::test]
fn the_choice_stands_straight_under_the_title(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, three(json!({"ref": "c", "anchor": "a"})), CUTS, cx);
    let reply = key_of(&view, "Answer 0.", cx);
    click(cx, format!("rewind-to-here-{reply}"));
    let title = bounds(cx, "dialog-title".into()).expect("no title");
    let first = bounds(cx, "rewind-conversation".into()).expect("no choice");
    let second = bounds(cx, "rewind-files".into()).unwrap();
    let slot = bounds(cx, "refusal".into()).expect("no slot");
    // The header's `pb-1`, the group's 44 px lines with no gap, then the slot's `pt-2`.
    assert_eq!((first.top() - title.bottom(), second.top() - first.bottom(), slot.top() - second.bottom()), (px(4.), px(0.), px(8.)));
    assert_eq!(first.left() - title.left(), px(0.), "the group keeps the title's `px-5`");
}
