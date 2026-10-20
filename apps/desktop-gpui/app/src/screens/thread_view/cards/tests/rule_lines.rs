// SPDX-License-Identifier: AGPL-3.0-only

use super::*;
use crate::screens::thread_view::cards::rule_line::SLATE_INK;

#[gpui_kit::test]
fn a_fork_says_where_it_came_from_on_one_line(cx: &mut TestAppContext) {
    let mut events = vec![
        base("session.start", 1, json!({"prompt": "fix it", "copiedFrom": "th-source"})),
        base("session.delta", 2, json!({"kind": "text", "text": "Done.", "copiedFrom": "th-source"})),
        base("session.done", 3, json!({"result": {"status": "completed"}, "copiedFrom": "th-source"})),
        base("session.end", 4, json!({"exitCode": 0, "sawResult": true, "copiedFrom": "th-source"})),
    ];
    events.push(base("session.start", 5, json!({"prompt": "and now", "turnId": "u2"})));
    let (view, cx) = window(events, cx);
    assert!(cx.debug_bounds("fork").is_none(), "the rule stands before the listing names its thread");
    let at = ThreadAt { workspace: "w0".into(), thread: "th-source".into() };
    let source = transcript::ForkSource { title: "Fix the build".into(), at: Some(at) };
    listed(&view, Listing { source: Some(source), ..Listing::default() }, cx);
    let row = frame(&view, cx, |k| matches!(k, RowKind::Fork { .. }));
    let line = bounds(cx, "fork");
    assert_eq!((line.size.height, row.size.height), (px(24.), px(24. + 16.)));
    assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| r.text == "Forked from Fix the build")));
    ROUTED.with(|r| r.borrow_mut().clear());
    let at = bounds(cx, "fork-open").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(routed(), [Route::Thread { workspace_id: "w0".into(), thread_id: "th-source".into() }]);
}

fn sub_row(sub: serde_json::Value) -> serde_json::Value {
    json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": 1.0, "sub": sub})
}

#[gpui_kit::test]
fn a_failed_start_says_why_under_its_message_with_try_again(cx: &mut TestAppContext) {
    let (view, cx) = window(vec![base("session.start", 1, json!({"prompt": "go"}))], cx);
    assert!(cx.debug_bounds("start-failed").is_none());
    let spawn = json!({"startedAt": 1.0, "log": ["asked boat"], "asked": "go", "failed": {"step": "machine", "said": "the cloud refused the key", "fix": "Add it again."}});
    let listing = Listing::of(&[sub_row(json!({"state": "released", "on": "box", "spawn": spawn}))], "t", 10.0);
    let failed = listing.start_failed.clone().expect("the listing kept no failed start");
    assert_eq!((failed.on.as_str(), failed.step.as_str()), ("Boat", "Asking Boat for a machine"));
    assert_eq!(listing.strip, None, "a restart that failed is said by its line alone");
    listed(&view, listing, cx);
    let person = frame(&view, cx, |k| matches!(k, RowKind::Person { .. }));
    let line = bounds(cx, "start-failed");
    assert!(line.top() >= person.bottom(), "the line stands under the message it was for");
    assert!(cx.debug_bounds("spawn-steps").is_some(), "a failed start opens on its step");
    let at = bounds(cx, "spawn-line").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("spawn-steps").is_none(), "the chevron did not shut the steps");
}

#[gpui_kit::test]
fn the_machine_line_and_the_starting_notice_stand_at_the_foot(cx: &mut TestAppContext) {
    let (view, cx) = window(plan("Plan."), cx);
    assert!(cx.debug_bounds("machine-line").is_none());
    let starting = Some(transcript::agent_starting_line("Claude Code", false));
    listed(&view, Listing { machine: Some("Out of memory, 3.8 of 4 GB".into()), starting, ..Listing::default() }, cx);
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    let texts: Vec<&str> = rows.iter().rev().take(2).map(|r| r.text.as_str()).collect();
    assert_eq!(texts, ["Out of memory, 3.8 of 4 GB", "Starting Claude Code"]);
    assert_eq!(bounds(cx, "machine-line").size.height, px(24.));
}

#[gpui_kit::test]
fn the_slate_line_stands_once_under_the_turns_last_reply_and_eases_its_ink(cx: &mut TestAppContext) {
    let slate = |pos, turn: &str, by: &str| {
        base("session.slate", pos, json!({"turnId": turn, "cause": "write", "version": pos, "by": by, "pieces": []}))
    };
    let mut events = changes();
    events.insert(2, slate(10, "u1", "agent"));
    events.insert(3, slate(11, "u1", "agent"));
    events.extend([
        base("session.start", 20, json!({"prompt": "and more", "turnId": "u2"})),
        base("session.delta", 21, json!({"kind": "text", "text": "Also done.", "turnId": "u2"})),
        slate(22, "u2", "person"),
        base("session.done", 23, json!({"result": {"status": "completed"}, "turnId": "u2"})),
        base("session.end", 24, json!({"exitCode": 0, "sawResult": true, "turnId": "u2"})),
    ]);
    let (view, cx) = window(events, cx);
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    let lines: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r.kind, RowKind::Slate)).map(|(i, _)| i).collect();
    let changed = rows.iter().position(|r| matches!(r.kind, RowKind::ChangedFiles { .. })).unwrap();
    assert_eq!(lines, [changed + 1], "one line under the first turn's reply and its files, none for a person's write");
    let line = bounds(cx, "slate-updated");
    let muted = cx.update(|_, cx| cx.palette().muted_foreground);
    let ink = || SLATE_INK.with(|i| i.get());
    assert_eq!(ink(), Some(muted));
    cx.simulate_mouse_move(line.center(), None, Modifiers::none());
    cx.run_until_parked();
    cx.executor().advance_clock(std::time::Duration::from_millis(60));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    let mid = ink().unwrap();
    let foreground = cx.update(|_, cx| cx.palette().foreground);
    assert!(mid != muted && mid != foreground, "the ink did not ease: {mid:?}");
    cx.executor().advance_clock(std::time::Duration::from_millis(200));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    assert_eq!(ink(), Some(foreground));
}

fn running() -> Vec<SessionEvent> {
    vec![
        base("session.start", 1, json!({"prompt": "go", "at": 1_000.0})),
        base("session.delta", 2, json!({"kind": "text", "text": "On it.", "at": 2_000.0})),
    ]
}

#[gpui_kit::test]
fn a_paused_workspace_says_so_under_a_quiet_thread_with_wake(cx: &mut TestAppContext) {
    RAN.with(|r| r.borrow_mut().clear());
    let (view, cx) = window(plan("Plan."), cx);
    listed(&view, Listing { workspace: Some("w".into()), paused: Some("Paused".into()), ..Listing::default() }, cx);
    let last = view.read_with(cx, |v, _| v.rows().last().map(|r| (r.key.clone(), r.text.clone())));
    assert_eq!(last, Some((transcript::PAUSED_KEY.to_owned(), "Paused".to_owned())), "no paused line under the thread");
    assert_eq!(bounds(cx, "workspace-paused").size.height, px(24.));
    let wake = bounds(cx, "workspace-wake");
    cx.simulate_click(wake.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["wake w"], "Wake did not wake the workspace");
}

#[gpui_kit::test]
fn a_turn_on_a_paused_workspace_says_what_it_waits_for_in_place_of_working(cx: &mut TestAppContext) {
    RAN.with(|r| r.borrow_mut().clear());
    let (view, cx) = window(running(), cx);
    let wait = transcript::Wait { label: "waiting for spoo to wake".into(), wake: true, elapsed: false };
    listed(&view, Listing { workspace: Some("w".into()), wait: Some(wait), paused: Some("Paused".into()), ..Listing::default() }, cx);
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    assert!(rows.iter().all(|r| r.key != transcript::PAUSED_KEY), "a working thread said its workspace's paused word");
    assert!(rows.iter().all(|r| !matches!(r.kind, RowKind::Thinking) || r.text.is_empty()), "a thread on a sleeping machine thinks");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let line = bounds(cx, "machine-wait");
    assert_eq!(line.size.height, px(24.));
    assert!(cx.debug_bounds("clock").is_none(), "a wait for a wake counts its seconds");
    let wake = bounds(cx, "wait-wake");
    cx.simulate_click(wake.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["wake w"], "the wait's Wake did not wake the workspace");
    let waking = transcript::Wait { label: "waking spoo on Mac mini".into(), wake: false, elapsed: true };
    listed(&view, Listing { workspace: Some("w".into()), wait: Some(waking), ..Listing::default() }, cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("wait-wake").is_none(), "a workspace already waking offers Wake");
    assert!(cx.debug_bounds("clock").is_some(), "the waking line does not count its seconds");
}

fn spawn_listing(spawn: serde_json::Value) -> Listing {
    let row = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": 1.0,
        "sub": {"state": "starting", "on": "box", "machine": "m-1", "spawn": spawn}});
    Listing { thread: "t".into(), ..Listing::of(&[row], "t", 10.0) }
}

#[gpui_kit::test]
fn a_machine_start_under_way_says_so_under_its_message_and_opens_to_its_steps(cx: &mut TestAppContext) {
    let spawn = json!({"startedAt": 1_000.0, "machine": {"since": 1_000.0, "id": "m-1", "states": ["provisioning"]}, "log": ["asked box"]});
    let events = vec![base("session.start", 1, json!({"prompt": "go", "at": 2_000.0}))];
    let (view, cx) = window_listed(events, Some(spawn_listing(spawn)), cx);
    let line = view.read_with(cx, |v, _| v.rows().iter().find(|r| matches!(r.kind, RowKind::StartFailed { .. })).map(|r| r.text.clone()));
    assert_eq!(line.as_deref(), Some("Starting a machine on Boat"), "a start under way says nothing under its message");
    assert!(cx.debug_bounds("spawn-steps").is_none(), "a start under way opens on its steps");
    let head = bounds(cx, "spawn-line");
    cx.simulate_click(head.center(), Modifiers::none());
    cx.run_until_parked();
    for step in ["spawn-step-machine", "spawn-step-agent"] {
        assert!(cx.debug_bounds(step).is_some(), "{step} not drawn once the line opened");
    }
}

#[gpui_kit::test]
fn a_machine_start_the_agent_answered_stands_done_between_the_message_and_the_reply(cx: &mut TestAppContext) {
    let spawn = json!({"startedAt": 1_000.0, "machine": {"since": 1_000.0, "id": "m-1", "states": [], "ms": 3_000.0}, "readyAt": 5_000.0, "log": []});
    let events = vec![
        base("session.start", 1, json!({"prompt": "go", "at": 2_000.0})),
        base("session.delta", 2, json!({"kind": "text", "text": "On it.", "at": 9_000.0})),
    ];
    let (view, _cx) = window_listed(events, Some(spawn_listing(spawn)), cx);
    let rows = view.read_with(_cx, |v, _| v.rows().to_vec());
    let at = rows.iter().position(|r| matches!(r.kind, RowKind::StartFailed { .. })).expect("no start line");
    assert_eq!(rows[at].text, "Started on Boat in 8.0s");
    assert!(matches!(rows[at - 1].kind, RowKind::Person { .. }), "the line does not stand under the message");
    assert!(rows[at + 1..].iter().any(|r| matches!(r.kind, RowKind::Agent { .. })), "the reply does not stand under the line");
    let RowKind::StartFailed { failed } = &rows[at].kind else { unreachable!() };
    let agent = failed.steps.iter().find(|s| s.id == "agent").expect("no agent step");
    assert_eq!((agent.state, agent.ms), (transcript::StepState::Done, Some(4_000.0)));
}

/// Catches: the line standing under a message with no time, which `machineStartAt` never takes as the asking one, while
/// its time counts from the message that did ask.
#[gpui_kit::test]
fn a_machine_start_stands_under_the_message_its_time_counts_from(cx: &mut TestAppContext) {
    let spawn = json!({"startedAt": 1_000.0, "machine": {"since": 1_000.0, "id": "m-1", "states": [], "ms": 3_000.0}, "readyAt": 5_000.0, "log": []});
    let events = vec![
        base("session.start", 1, json!({"prompt": "before"})),
        base("session.start", 2, json!({"prompt": "go", "at": 2_000.0, "turnId": "u2"})),
        base("session.delta", 3, json!({"kind": "text", "text": "On it.", "at": 9_000.0, "turnId": "u2"})),
    ];
    let (view, _cx) = window_listed(events, Some(spawn_listing(spawn)), cx);
    let rows = view.read_with(_cx, |v, _| v.rows().to_vec());
    let at = rows.iter().position(|r| matches!(r.kind, RowKind::StartFailed { .. })).expect("no start line");
    assert_eq!(rows[at].text, "Started on Boat in 8.0s");
    assert_eq!(rows[at - 1].text, "go", "the line stands under another message than the one its time counts from");
}

/// The listing off a sub-thread's own row, running since `started` with its machine in `state`.
fn own_listing(state: &str, started: f64, spawn: serde_json::Value) -> Listing {
    let row = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": started,
        "prompt": "Fix the flaky test", "sub": {"state": state, "on": "box", "machine": "m-1", "spawn": spawn}});
    Listing { thread: "t".into(), ..Listing::of(&[row], "t", started) }
}

#[gpui_kit::test]
fn a_machine_start_counts_to_the_agents_first_entry_of_any_kind_on_a_settled_turn(cx: &mut TestAppContext) {
    let spawn = json!({"startedAt": 1_000.0, "machine": {"since": 1_000.0, "states": [], "ms": 3_000.0}, "readyAt": 5_000.0, "log": []});
    let events = vec![
        base("session.start", 1, json!({"prompt": "go", "at": 2_000.0})),
        base("session.delta", 2, json!({"kind": "thinking", "text": "Reading the test.", "at": 9_000.0})),
        base(
            "session.delta",
            3,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "b", "text": "{\"command\":\"ls\"}", "at": 10_000.0}),
        ),
        base("session.delta", 4, json!({"kind": "tool_result", "toolUseId": "b", "text": "a", "at": 11_000.0})),
        base("session.delta", 5, json!({"kind": "text", "text": "Fixed ", "at": 150_000.0})),
        base("session.delta", 6, json!({"kind": "text", "text": "it.", "at": 151_000.0})),
        base("session.done", 7, json!({"result": {"status": "completed", "durationMs": 149_000}, "at": 152_000.0})),
        base("session.end", 8, json!({"exitCode": 0, "sawResult": true, "at": 152_000.0})),
    ];
    let (view, _cx) = window_listed(events, Some(own_listing("running", 2_000.0, spawn)), cx);
    let rows = view.read_with(_cx, |v, _| v.rows().to_vec());
    let line = rows.iter().find(|r| matches!(r.kind, RowKind::StartFailed { .. })).expect("no start line");
    assert_eq!(line.text, "Started on Boat in 8.0s", "the start line does not count to the thinking that came first");
    let RowKind::StartFailed { failed } = &line.kind else { unreachable!() };
    let agent = failed.steps.iter().find(|s| s.id == "agent").expect("no agent step");
    assert_eq!(agent.ms, Some(4_000.0), "the agent's step does not count from the machine standing to its first entry");
}

#[gpui_kit::test]
fn a_running_turn_counts_from_its_rows_start_where_that_came_first(cx: &mut TestAppContext) {
    let spawn = json!({"startedAt": 1_000.0, "readyAt": 5_000.0, "log": []});
    let events = vec![
        base("session.start", 1, json!({"prompt": "go", "at": 20_000.0})),
        base("session.delta", 2, json!({"kind": "text", "text": "On it.", "at": 21_000.0})),
    ];
    let (view, _cx) = window_listed(events, Some(own_listing("running", 12_000.0, spawn)), cx);
    let since = view.read_with(_cx, |v, _| {
        v.rows().iter().find_map(|r| match r.kind {
            RowKind::Working { since, .. } => Some(since),
            _ => None,
        })
    });
    assert_eq!(since, Some(Some(12_000.0)), "the Working line does not count from the row's start");
}

#[gpui_kit::test]
fn a_sub_thread_whose_machine_is_starting_is_not_at_work(cx: &mut TestAppContext) {
    let spawn = json!({"startedAt": 1_000.0, "log": []});
    let (view, cx) = window_listed(running(), Some(own_listing("starting", 1_000.0, spawn.clone())), cx);
    let working = |view: &Entity<ThreadView>, cx: &mut VisualTestContext| {
        view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Working { .. } | RowKind::Thinking)))
    };
    assert!(!working(&view, cx), "a sub-thread whose machine is starting says it is working");
    listed(&view, own_listing("running", 1_000.0, json!({"startedAt": 1_000.0, "readyAt": 2_000.0, "log": []})), cx);
    assert!(working(&view, cx), "a sub-thread whose machine stands says nothing of its turn");
}

#[gpui_kit::test]
fn a_starting_sub_thread_with_no_event_shows_its_opening_message_with_its_time(cx: &mut TestAppContext) {
    let spawn = json!({"startedAt": 1_000.0, "machine": {"since": 1_000.0, "states": ["provisioning"]}, "log": []});
    let (view, cx) = window_listed(Vec::new(), Some(own_listing("starting", 1_000.0, spawn)), cx);
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    let drawn: Vec<(&str, &str)> = rows.iter().map(|r| (r.key.as_str(), r.text.as_str())).collect();
    assert_eq!(
        drawn,
        [("opening", "Fix the flaky test"), ("start-failed", "Starting a machine on Boat")],
        "the opening message is not drawn"
    );
    assert!(matches!(rows[0].kind, RowKind::Person { at: Some(at), .. } if at == 1_000.0), "the opening message has no time");
    let (bubble, meta) = (bounds(cx, "bubble"), bounds(cx, "meta"));
    assert!(meta.top() >= bubble.bottom(), "the opening message has no line for its time and Copy");
    assert!(
        view.read_with(cx, |v, _| v.rows().iter().all(|r| !matches!(r.kind, RowKind::Working { .. }))),
        "a starting machine is at work"
    );
}
