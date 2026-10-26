// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

fn spawned() -> Vec<SessionEvent> {
    vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": "{\"description\":\"Survey\"}"})),
        base("session.subagent", 3, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey it"})),
        base(
            "session.delta",
            4,
            json!({"kind": "tool_use", "toolName": "mcp__wsp__run", "toolUseId": "r", "text": "{\"title\":\"Fix docs\"}"}),
        ),
        base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "r", "text": "{\"threadId\":\"th1\"}"})),
    ]
}

#[gpui_kit::test]
fn the_children_a_call_started_stand_in_the_rows_box(cx: &mut TestAppContext) {
    let events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": "{\"description\":\"Survey\"}"})),
        base("session.subagent", 3, json!({"task": "k1", "state": "running", "parentToolUseId": "t", "title": "Survey it"})),
        base("session.delta", 4, json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it."})),
        base("session.subagent", 5, json!({"task": "k1", "state": "failed", "parentToolUseId": "t", "summary": "out of room"})),
        base(
            "session.delta",
            6,
            json!({"kind": "tool_use", "toolName": "mcp__wsp__run", "toolUseId": "r", "text": "{\"title\":\"Fix docs\"}"}),
        ),
        base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "r", "text": "{\"threadId\":\"th1\"}"})),
    ];
    let listing = Listing::holding(&events);
    let (view, cx) = window_listed(events, Some(listing), cx);
    let row = frame(&view, cx, |k| matches!(k, RowKind::Spawn { .. }));
    let column = bounds(cx, "spawn");
    // A row with a note under its title is at least 48 px, one without is 36; no gap, no gutter.
    assert_eq!((column.left(), column.top(), column.size.height), (row.left(), row.top(), px(48. + 36.)));
}

#[gpui_kit::test]
fn a_spawn_row_takes_its_state_off_the_listing_and_opens_its_child(cx: &mut TestAppContext) {
    let (view, cx) = window(spawned(), cx);
    let child_row = |view: &Entity<ThreadView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |v, _| {
            v.rows().iter().find_map(|r| match &r.kind {
                RowKind::Spawn { children } => children.iter().find(|c| !c.subagent).cloned(),
                _ => None,
            })
        })
    };
    assert!(child_row(&view, cx).is_none(), "a child the listing does not hold stands as its call");
    let subagents = Listing::holding(&spawned()).subagents;
    listed(
        &view,
        Listing {
            workspace: Some("w".into()),
            children: vec![child("th1", Kind::NeedsYou, ChildPart::Live)],
            subagents,
            ..Listing::default()
        },
        cx,
    );
    assert_eq!(child_row(&view, cx).and_then(|c| c.listed).map(|l| l.kind), Some(Kind::NeedsYou));
    let column = bounds(cx, "spawn");
    ROUTED.with(|r| r.borrow_mut().clear());
    // The subagent's row first, then the thread's, 36 px each.
    cx.simulate_click(column.origin + gpui_kit::point(px(60.), px(18.)), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_click(column.origin + gpui_kit::point(px(60.), px(36. + 18.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(routed(), [Route::Screen("subagent:w/t/t".into()), Route::Thread { workspace_id: "w2".into(), thread_id: "th1".into() }]);
}

#[gpui_kit::test]
fn the_rows_follow_a_listing_that_changes_and_a_closed_ask_answers_nothing(cx: &mut TestAppContext) {
    SENT.with(|s| s.borrow_mut().clear());
    let (view, cx) = window(spawned(), cx);
    let waiting = json!({"threadId": "th9", "workspaceId": "w", "sessionId": "s9", "title": "Lint pass", "prompt": {
        "askId": "a9", "toolName": "Bash", "input": "{\"command\":\"ls\"}", "options": [{"id": "y", "label": "Allow", "effect": "allow"}]}});
    let kid = |status: &str| json!({"id": "c1", "workspaceId": "w2", "harness": "codex", "status": status, "threadId": "th1", "parentThreadId": "t", "startedAt": 5.0, "endedAt": 6.0});
    let own = |waiting: Option<&serde_json::Value>| {
        let mut own = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": 1.0,
            "subagents": [{"id": "k1", "title": "Survey it", "state": "running", "parentToolUseId": "t", "startedAt": 2.0}]});
        if let Some(w) = waiting {
            own["waitingOn"] = w.clone();
        }
        own
    };
    let kind_of = |view: &Entity<ThreadView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |v, _| {
            let spawn = v.rows().iter().find_map(|r| match &r.kind {
                RowKind::Spawn { children } => children.iter().find_map(|c| c.listed.as_ref().map(|l| l.kind)),
                _ => None,
            });
            let threads = v.rows().iter().find_map(|r| match &r.kind {
                RowKind::Threads { children, .. } => children.iter().find(|c| c.subagent.is_none()).map(|c| c.kind),
                _ => None,
            });
            let asked = v.rows().iter().any(|r| r.key == "waiting-on:th9:a9");
            (spawn, threads, asked)
        })
    };
    listed(&view, Listing::of(&[own(Some(&waiting)), kid("running")], "t", 10.0), cx);
    assert_eq!(kind_of(&view, cx), (Some(Kind::Working), Some(Kind::Working), true));
    listed(&view, Listing::of(&[own(None), kid("completed")], "t", 10.0), cx);
    assert_eq!(kind_of(&view, cx), (Some(Kind::Done), Some(Kind::Done), false), "the rows kept the listing read at open");
    view.update(cx, |v, cx| answer(v, "s9", "a9", "y", cx));
    assert!(SENT.with(|s| s.borrow().is_empty()), "a question answered elsewhere was answered again");
}
