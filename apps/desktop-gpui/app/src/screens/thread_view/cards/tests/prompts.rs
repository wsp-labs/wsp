// SPDX-License-Identifier: AGPL-3.0-only

use super::*;
use crate::screens::composer::dock::{answer_words, sentence};

/// A subagent's question, which keeps its card; the thread's own docks in the composer.
fn question(multiple: bool) -> Vec<SessionEvent> {
    let input = json!({"questions": [{"question": "Which colour?", "header": "Colour", "multiSelect": multiple, "options": [
        {"label": "Red", "description": "warm"}, {"label": "Blue"}
    ]}]});
    let options = json!([{"id": "q0:o0", "label": "Red", "effect": "answer"}, {"id": "q0:o1", "label": "Blue", "effect": "answer"}]);
    let asked =
        json!({"askId": "w", "toolName": "AskUserQuestion", "parentToolUseId": "task-1", "input": input.to_string(), "options": options});
    vec![base("session.start", 1, json!({"prompt": "go"})), base("session.permission", 2, asked)]
}

#[gpui_kit::test]
fn the_threads_own_open_permission_is_the_waiting_record_and_offers_nothing(cx: &mut TestAppContext) {
    let (view, cx) = window(permission(), cx);
    let row = frame(&view, cx, |k| matches!(k, RowKind::Permission { .. }));
    let record = bounds(cx, "record");
    assert_eq!((record.left() - row.left(), record.top() - row.top(), record.size.height), (px(4.), px(4.), px(28.)));
    assert!(cx.debug_bounds("options").is_none(), "the composer's dock answers it, not the row");
    assert!(cx.debug_bounds("lead").is_none());
}

#[gpui_kit::test]
fn an_open_permission_draws_as_the_web_and_a_pick_sends_its_ids(cx: &mut TestAppContext) {
    SENT.with(|s| s.borrow_mut().clear());
    // A subagent's prompt keeps its card in its fold; the thread's own docks.
    let (view, cx) = window(permission_of(Some("task-1")), cx);
    let row = frame(&view, cx, |k| matches!(k, RowKind::Permission { asker: Some(_), .. }));
    let lead = bounds(cx, "lead");
    // The fold's gutter and rail (`px-1`, `ml-1.5`, the 1 px rule, `pl-[11px]`), `pt-1` twice and the asker's 16 px
    // line 4 px over the lead: the command's `py-1` on a 16 px line stands taller than the 22.75 px words.
    assert_eq!((lead.left() - row.left(), lead.top() - row.top(), lead.size.height), (px(22.), px(28.), px(24.)));
    // `pt-0.5` over the xs buttons, 4 px under the fields' line.
    let options = bounds(cx, "options");
    assert_eq!(options.size.height, px(26.));
    assert_eq!(options.top() - lead.bottom(), px(4. + 16. + 4.));
    cx.simulate_click(options.origin + gpui_kit::point(px(12.), px(14.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(sent(), [("s".to_owned(), "k".to_owned(), "y".to_owned())]);
    // With no host the call refuses at once, and the refusal stands under the options in the error ink's line.
    let refused = view.read_with(cx, |v, _| v.refusals.get("k").cloned());
    assert_eq!(refused.map(|r| r.said).as_deref(), Some("The thread view follows no host."));
    let line = bounds(cx, "refusal");
    assert_eq!((line.top() - options.bottom(), line.size.height), (px(4.), px(20.)));
}

#[gpui_kit::test]
fn the_threads_own_question_is_the_waiting_record(cx: &mut TestAppContext) {
    let mut events = question(false);
    if let Some(SessionEvent::SessionPermission { parent_tool_use_id, .. }) = events.last_mut() {
        *parent_tool_use_id = None;
    }
    let (_, cx) = window(events, cx);
    assert!(cx.debug_bounds("question").is_none(), "the composer's dock asks it");
    assert!(cx.debug_bounds("record").is_some());
}

#[gpui_kit::test]
fn a_question_with_several_answers_ticks_boxes_and_sends_them_as_one(cx: &mut TestAppContext) {
    SENT.with(|s| s.borrow_mut().clear());
    let (_, cx) = window(question(true), cx);
    let tick = bounds(cx, "checkbox");
    assert_eq!(tick.size, size(px(16.), px(16.)));
    let asked = bounds(cx, "question");
    let answer = asked.bottom_left() + gpui_kit::point(px(12.), px(-12.));
    // Held until every question has a tick: a press sends nothing.
    cx.simulate_click(answer, Modifiers::none());
    cx.run_until_parked();
    assert!(sent().is_empty(), "{:?}", sent());
    // The first option's line, under the header (16), the question (22.75), the gaps (6, 6) and `pt-0.5`.
    cx.simulate_click(asked.origin + gpui_kit::point(px(8.), px(16. + 6. + 22.75 + 6. + 2. + 10.)), Modifiers::none());
    cx.run_until_parked();
    // An option's line: `py-0.5`, the box `mt-0.5`.
    let tick = bounds(cx, "checkbox");
    let line = cx.debug_bounds("tick").unwrap();
    assert_eq!(tick.top() - line.top(), px(4.));
    // `tone="neutral"`: the tick fills with the foreground, not the accent.
    let foreground = cx.update(|_, cx| cx.palette().foreground);
    assert_eq!(crate::ui::controls::TICKED_FILL.with(|f| f.get()), Some(foreground), "the ticked box took the accent fill");
    cx.simulate_click(answer, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(sent(), [("s".to_owned(), "w".to_owned(), "q0:o0".to_owned())]);
}

#[gpui_kit::test]
fn a_question_with_one_answer_answers_on_the_press(cx: &mut TestAppContext) {
    SENT.with(|s| s.borrow_mut().clear());
    let (_, cx) = window(question(false), cx);
    assert!(cx.debug_bounds("checkbox").is_none(), "one answer takes no boxes");
    let asked = bounds(cx, "question");
    // The header (16) and the question (22.75), 6 px apart, then `pt-0.5`: the first answer's button.
    cx.simulate_click(asked.origin + gpui_kit::point(px(12.), px(16. + 6. + 22.75 + 6. + 2. + 12.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(sent(), [("s".to_owned(), "w".to_owned(), "q0:o0".to_owned())]);
}

#[test]
fn a_refused_pick_reads_the_protocols_answer_words() {
    let words = ["gone", "unsupported", "not-found", "no-option"].map(answer_words);
    assert_eq!(words[0], Some("the prompt closed before the answer reached it"));
    assert!(words.iter().all(|w| w.is_some_and(|w| !w.contains('"'))), "{words:?}");
    assert_eq!(answer_words("answered"), None, "an answered pick is no refusal");
    assert_eq!(sentence(words[2].unwrap()), "This host holds no turn of that thread.");
}

#[gpui_kit::test]
fn a_thread_behind_another_s_question_draws_that_question(cx: &mut TestAppContext) {
    let (view, cx) = window(vec![base("session.start", 1, json!({"prompt": "go"}))], cx);
    let waiting = json!({"threadId": "th9", "workspaceId": "w", "sessionId": "s9", "title": "Lint pass", "prompt": {
        "askId": "a9", "toolName": "Bash", "input": "{\"command\":\"rm -rf out\"}",
        "options": [{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]}});
    let own = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": 1.0, "waitingOn": waiting});
    listed(&view, Listing::of(&[own], "t", 10.0), cx);
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    assert!(rows.iter().any(|r| r.text == "Waiting for you"), "{rows:?}");
    assert!(!rows.iter().any(|r| matches!(r.kind, RowKind::Thinking)));
    let asked = rows.last().unwrap();
    assert!(
        matches!(&asked.kind, RowKind::Permission { session, asker: Some(a), .. } if session == "s9" && a == "Lint pass asks; this thread waits on the answer")
    );
    assert!(cx.debug_bounds("options").is_some(), "the borrowed question offers no answer");
}
