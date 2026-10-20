// SPDX-License-Identifier: AGPL-3.0-only
use gpui_kit::component::Root;
use gpui_kit::{size, Entity, Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::protocol::SessionEvent;

use super::*;

fn event(kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
    let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u1", "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    cx.simulate_resize(size(px(900.), px(800.)));
    (view, cx)
}

fn load(view: &Entity<ThreadView>, events: Vec<SessionEvent>, cx: &mut VisualTestContext) {
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
        v.scroll_to_top(cx);
    });
    cx.run_until_parked();
}

/// A reply holding a shell block and a run of it, settled or still streaming.
fn ran(settled: bool) -> Vec<SessionEvent> {
    let mut events = vec![
        event("session.start", 1, json!({"prompt": "go"})),
        event("session.delta", 2, json!({"kind": "text", "text": "Try:\n\n```sh\nls\n```\n"})),
        event(
            "session.run",
            3,
            json!({"runId": "r1", "block": "u1:m@2@6", "command": "ls", "state": "exited", "exitCode": 0, "output": "a"}),
        ),
    ];
    if settled {
        events.push(event("session.done", 4, json!({"result": {"status": "completed"}})));
        events.push(event("session.end", 5, json!({"exitCode": 0, "sawResult": true})));
    }
    events
}

#[gpui_kit::test]
fn a_message_sent_while_the_agent_worked_says_so_on_hover(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let events = vec![
        event("session.start", 1, json!({"prompt": "go"})),
        event("session.delta", 2, json!({"kind": "text", "text": "Working."})),
        event("session.steer", 3, json!({"prompt": "also this"})),
    ];
    load(&view, events, cx);
    let plain = cx.debug_bounds("bubble").expect("the first message's bubble");
    cx.simulate_mouse_move(plain.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip").is_none(), "a message sent between turns says it was sent while the agent worked");
    let steered = cx.debug_bounds("bubble-steered").expect("the steered message's bubble");
    cx.simulate_mouse_move(steered.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip").is_some(), "no Sent while the agent was working on the steered bubble");
}

#[gpui_kit::test]
fn only_a_settled_reply_draws_its_shell_blocks_run(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, ran(true), cx);
    assert!(cx.debug_bounds("code-run-output").is_some(), "the settled reply's run");
    load(&view, ran(false), cx);
    let streaming = view
        .read_with(cx, |v, _| v.rows().iter().any(|r| matches!(&r.kind, RowKind::Agent { streaming: true, runs, .. } if !runs.is_empty())));
    assert!(streaming, "the streaming reply holds the run");
    assert!(cx.debug_bounds("code-run-output").is_none(), "a streaming reply drew a run");
}

#[gpui_kit::test]
fn copy_output_copies_the_whole_output_and_holds_its_check_for_a_second_and_a_half(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let output: String = (1..=20).map(|n| format!("line {n}\n")).collect();
    let events = vec![
        event("session.start", 1, json!({"prompt": "go"})),
        event("session.delta", 2, json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"seq 20\"}"})),
        event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": output, "exitCode": 0})),
        event("session.done", 4, json!({"result": {"status": "completed"}})),
        event("session.end", 5, json!({"exitCode": 0, "sawResult": true})),
    ];
    load(&view, events, cx);
    let shut = |v: &ThreadView| -> Vec<String> {
        let rows = v.rows().iter();
        let groups = rows.clone().filter_map(|r| match &r.kind {
            RowKind::Fold { open: false } => r.turn.clone(),
            RowKind::Group { group, open: false, .. } => Some(group.clone()),
            _ => None,
        });
        let calls = rows.flat_map(|r| match &r.kind {
            RowKind::Calls { calls, .. } => calls.iter().filter(|c| !c.open).map(|c| c.key.clone()).collect(),
            _ => Vec::new(),
        });
        groups.chain(calls).collect()
    };
    for _ in 0..4 {
        let Some(key) = view.read_with(cx, |v, _| shut(v).first().cloned()) else { break };
        view.update(cx, |v, cx| v.toggle(&key, cx));
        cx.run_until_parked();
    }
    let copy = cx.debug_bounds("copy-output").expect("Copy output under the open command");
    cx.simulate_click(copy.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some(output.trim_end()), "all 20 lines, not the 10 shown");
    assert!(cx.debug_bounds("output-copied").is_some(), "no check after the copy");
    cx.executor().advance_clock(Duration::from_millis(1_400));
    cx.run_until_parked();
    assert!(cx.debug_bounds("output-copied").is_some(), "the check did not hold");
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    assert!(cx.debug_bounds("copy-output").is_some(), "the check did not go back to Copy output");
}

#[gpui_kit::test]
fn a_turn_stopped_on_a_prompt_reads_waiting_for_you_before_its_clock(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let start = event("session.start", 1, json!({"prompt": "go", "at": 1_000.0}));
    load(&view, vec![start.clone()], cx);
    let working = cx.debug_bounds("clock-slot").expect("the Working line's clock");
    let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
    let ask =
        event("session.permission", 2, json!({"askId": "k", "toolName": "Bash", "input": "{\"command\":\"ls\"}", "options": options}));
    load(&view, vec![start, ask], cx);
    let waits = view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Working { waiting: true, .. })));
    assert!(waits, "the open prompt left the turn working");
    let waiting = cx.debug_bounds("clock-slot").expect("the waiting line's clock");
    assert!(waiting.left() > working.left(), "the clock stands where it does after \"Working for\", at {:?}", waiting.left());
}

#[gpui_kit::test]
fn plus_n_lines_shows_the_whole_output_and_fewer_lines_folds_it_again(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let output: String = (1..=40).map(|n| format!("line {n}\n")).collect();
    let events = vec![
        event("session.start", 1, json!({"prompt": "go"})),
        event("session.delta", 2, json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"seq 40\"}"})),
        event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": output, "exitCode": 0})),
        event("session.done", 4, json!({"result": {"status": "completed"}})),
        event("session.end", 5, json!({"exitCode": 0, "sawResult": true})),
    ];
    load(&view, events, cx);
    for key in ["u1", "group:tool:u1:a", "u1:call:a"] {
        view.update(cx, |v, cx| v.toggle(key, cx));
        cx.run_until_parked();
    }
    let folded = cx.debug_bounds("output").expect("the open command's output");
    let more = cx.debug_bounds("output-more").expect("no +N lines act under 40 lines of output");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    let whole = cx.debug_bounds("output").expect("the output after +N lines");
    assert!(whole.size.height > folded.size.height, "+N lines left the output at {:?}", whole.size.height);
    let fewer = cx.debug_bounds("output-more").expect("no Fewer lines act");
    cx.simulate_click(fewer.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("output").map(|b| b.size.height), Some(folded.size.height), "Fewer lines did not fold it again");
}

#[test]
fn timers_and_durations_read_as_the_web_app_writes_them() {
    assert_eq!(timer(12_900.), "12s");
    assert_eq!(timer(185_000.), "3m 5s");
    assert_eq!(timer(180_000.), "3m");
    assert_eq!(timer(3_840_000.), "1h 4m");
    assert_eq!(command_duration(850.), "850ms");
    assert_eq!(command_duration(2_390.), "2.3s");
    assert_eq!(command_duration(45_000.), "45s");
    assert_eq!(command_duration(65_000.), "1m 5s");
    assert_eq!(command_duration(0.2), "1ms");
    assert_eq!(command_duration(10_600.), "10s");
    assert_eq!(command_duration(59_700.), "59s");
    assert_eq!(thousands(1_234_567), "1,234,567");
}

#[test]
fn a_patch_numbers_each_line_on_its_own_side() {
    let lines = diff_lines("--- a/x\n+++ b/x\n@@ -10,3 +10,3 @@\n keep\n-old\n+new\n tail");
    let read: Vec<(usize, Option<char>, &str)> = lines.iter().map(|l| (l.number, l.mark, l.text)).collect();
    assert_eq!(read, [(10, None, "keep"), (11, Some('-'), "old"), (11, Some('+'), "new"), (12, None, "tail")]);
}

/// A key pressed and let go, as the keyboard's click on a focused piece needs both.
fn press(cx: &mut VisualTestContext, key: &str) {
    let keystroke = gpui_kit::Keystroke::parse(key).unwrap();
    cx.simulate_event(gpui_kit::KeyDownEvent { keystroke: keystroke.clone(), is_held: false, prefer_character_input: false });
    cx.simulate_event(gpui_kit::KeyUpEvent { keystroke });
    cx.run_until_parked();
}

fn settled(mut events: Vec<SessionEvent>, pos: u64) -> Vec<SessionEvent> {
    events.push(event("session.done", pos, json!({"result": {"status": "completed"}})));
    events.push(event("session.end", pos + 1, json!({"exitCode": 0, "sawResult": true})));
    events
}

fn open_all(view: &Entity<ThreadView>, keys: &[&str], cx: &mut VisualTestContext) {
    for key in keys {
        view.update(cx, |v, cx| v.toggle(key, cx));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn a_long_message_clamps_under_its_fade_and_show_full_message_opens_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let long = "word ".repeat(400);
    load(&view, vec![event("session.start", 1, json!({"prompt": long}))], cx);
    let clamped = cx.debug_bounds("row-0").expect("the person's row");
    // The bubble's 12 px padding round 176 px of text, the 24 px meta line under it, 4 px apart, and the first row's 16 px top.
    assert!((clamped.size.height - px(16. + 24. + 176. + 4. + 24. + 16.)).abs() < px(1.), "clamped to {:?}", clamped.size.height);
    let more = cx.debug_bounds("show-full").expect("Show full message on a 2,000 character message");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    let whole = cx.debug_bounds("row-0").expect("the person's row");
    assert!(whole.size.height > clamped.size.height + px(100.), "Show full message left it at {:?}", whole.size.height);
    let less = cx.debug_bounds("show-less").expect("Show less under the whole message");
    cx.simulate_click(less.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("row-0").map(|b| b.size.height), Some(clamped.size.height), "Show less did not clamp it again");
    load(&view, vec![event("session.start", 1, json!({"prompt": "short"}))], cx);
    assert!(cx.debug_bounds("show-full").is_none(), "a short message offers Show full message");
}

#[gpui_kit::test]
fn the_clamp_fade_ends_at_the_clamped_bodys_bottom(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    load(&view, vec![event("session.start", 1, json!({"prompt": format!("[a](https://example.com/{})", "x".repeat(600))}))], cx);
    let body = cx.debug_bounds("message-body").expect("the clamped message's body");
    assert!(body.size.height < CLAMP_HEIGHT - px(24.), "the body fills the clamp, so a top-anchored fade would pass: {:?}", body.size);
    let fade = cx.debug_bounds("clamp-fade").expect("the fade over a one word link past 600 characters");
    assert_eq!(fade.bottom(), body.bottom(), "the fade is measured from the body's bottom, as the web's mask is");
    assert_eq!(fade.size.height, px(80.));
}

/// Catches: the solid fade laying the theme's ground over the Mac's glass, which shows through a dark theme's bubble.
#[gpui_kit::test]
fn on_glass_a_long_message_is_cut_at_the_fades_foot_under_no_fade(cx: &mut TestAppContext) {
    use crate::screens::app_shell::material::Look;
    let (view, cx) = window(cx);
    cx.update(|_, cx| crate::ui::theme::set_theme_in("graphite", Look { mac: true, glass: true }, cx));
    load(&view, vec![event("session.start", 1, json!({"prompt": "word ".repeat(400)}))], cx);
    let body = cx.debug_bounds("message-body").expect("the clamped message's body");
    assert_eq!(body.size.height, CLAMP_HEIGHT - CLAMP_FADE_FOOT, "on glass the body is cut where the web's mask is transparent");
    let fades = cx.update(|window, cx| {
        let fill = clamp_fade_fill(cx.palette().background, cx.palette().accent);
        window.painted_quads().iter().filter(|q| q.background.as_solid() == Some(fill)).count()
    });
    assert_eq!(fades, 0, "on glass the collapsed message paints the solid fade");
    assert!(cx.debug_bounds("clamp-fade").is_none(), "on glass the collapsed message draws a fade");
}

#[test]
fn every_themes_clamp_fade_is_solid_in_the_bubbles_on_screen_color() {
    use crate::screens::app_shell::material::{fills, Look};
    for t in &crate::ui::theme::THEMES {
        let p = t.palette();
        for look in [Look { mac: false, glass: false }, Look { mac: true, glass: false }] {
            let centre = fills(look, t.id, &p).centre;
            let fill = clamp_fade_fill(centre, p.accent);
            assert_eq!(fill.a, 1., "{} {look:?}: the fade lets the text show through", t.id);
            let (ground, ink, got) = (Rgba::from(centre), Rgba::from(p.accent), Rgba::from(fill));
            let shown = |g: f32, i: f32| g * (1. - ink.a) + i * ink.a;
            for (channel, got, want) in
                [("r", got.r, shown(ground.r, ink.r)), ("g", got.g, shown(ground.g, ink.g)), ("b", got.b, shown(ground.b, ink.b))]
            {
                assert!((got - want).abs() < 1e-3, "{} {look:?}: {channel} is {got}, the bubble shows {want}", t.id);
            }
        }
    }
}

/// Catches: the fade laid in the bubble over the theme's background where the shell's centre is a dark material's ground.
#[gpui_kit::test]
fn the_clamp_fade_follows_the_centres_ground_on_a_solid_mac_dark_theme(cx: &mut TestAppContext) {
    use crate::screens::app_shell::material::{fills, Look};
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Dark, cx);
        crate::ui::theme::set_theme_in("graphite", Look { mac: true, glass: false }, cx);
    });
    let (p, centre) = cx.update(|cx| {
        let p = cx.palette().clone();
        let centre = fills(Look { mac: true, glass: false }, "graphite", &p).centre;
        (p, centre)
    });
    assert_ne!(centre, p.background, "graphite's material ground on a solid Mac is not its page background");
    let got = cx.update(|cx| centre_ground(&p, cx));
    assert_eq!(got, centre, "the fade is laid over the theme's background, not the centre the shell lays");
}

#[test]
fn a_message_clamps_past_600_characters_or_8_lines() {
    assert!(!clamps(&"a".repeat(600)));
    assert!(clamps(&"a".repeat(601)));
    assert!(!clamps(&"é".repeat(400)), "600 is counted in UTF-16 units, as JS counts, not bytes");
    assert!(!clamps("1\n2\n3\n4\n5\n6\n7\n8"));
    assert!(clamps("1\n2\n3\n4\n5\n6\n7\n8\n9"));
    assert!(!clamps(&" ".repeat(900)));
}

#[gpui_kit::test]
fn a_retried_model_call_reads_its_retry_line_in_the_working_lines_place(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let start = event("session.start", 1, json!({"prompt": "go", "at": 1_000.0}));
    let retry = event("session.retry", 2, json!({"attempt": 2, "of": 10, "why": "overloaded"}));
    load(&view, vec![start.clone(), retry.clone()], cx);
    assert!(cx.debug_bounds("retry").is_some(), "no retry line");
    assert!(cx.debug_bounds("clock-slot").is_some(), "the retry line's clock");
    assert!(cx.debug_bounds("shimmer").is_none(), "Thinking shimmers while the call is made again");
    let words = view.read_with(cx, |v, _| {
        v.rows().iter().find_map(|r| match &r.kind {
            RowKind::Working { retry, .. } => retry.clone(),
            _ => None,
        })
    });
    assert_eq!(words.as_deref(), Some("Overloaded, trying again (2 of 10)"));
    let wrote = event("session.delta", 3, json!({"kind": "text", "text": "Here"}));
    load(&view, vec![start, retry, wrote], cx);
    assert!(cx.debug_bounds("retry").is_none(), "the retry line outlived the agent's reply");
}

#[test]
fn an_insight_block_is_cut_out_of_its_reply() {
    let text = "Before.\n\n`★ Insight ─────────────────────────────────────`\n- one\n- two\n`─────────────────────────────────────────────────`\n\nAfter.";
    let read = insights(text, false).expect("an insight block");
    assert_eq!(
        read,
        [
            Segment { insight: false, text: "Before." },
            Segment { insight: true, text: "- one\n- two" },
            Segment { insight: false, text: "After." },
        ]
    );
    assert!(insights("no block here", false).is_none());
    let open = "Before.\n\n✶ Insights ───\nhalf";
    assert!(insights(open, false).is_none(), "an unclosed block stays text once the reply is done");
    assert_eq!(insights(open, true).map(|s| s.len()), Some(2), "and is drawn open while it streams");
    let fenced = "```\n★ Insight ───\nx\n───\n```";
    assert!(insights(fenced, false).is_none(), "a marker inside a fence is code");
}

#[test]
fn a_segment_numbers_its_shell_blocks_from_its_own_start() {
    let text = "```sh\nls\n```\n\n`★ Insight ─────`\n```sh\npwd\n```\n`─────`";
    let segments = insights(text, false).expect("an insight block");
    let start = segments[1].text.as_ptr() as usize - text.as_ptr() as usize;
    assert_eq!(fences_before(text, start), 1, "the block above the note comes first");
    assert_eq!(fences_before(text, 0), 0);
}

#[gpui_kit::test]
fn an_insight_block_draws_as_a_note_without_its_marker_lines(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let reply = "Before.\n\n`★ Insight ─────`\nThe point.\n`─────`\n\nAfter.";
    let events = settled(
        vec![event("session.start", 1, json!({"prompt": "go"})), event("session.delta", 2, json!({"kind": "text", "text": reply}))],
        3,
    );
    load(&view, events, cx);
    let note = cx.debug_bounds("insight").expect("the Insight note");
    assert!(note.size.height > px(16. + 4. + 22.), "the note holds its label and its line: {:?}", note.size.height);
}

#[test]
fn only_an_empty_reply_reads_empty_response() {
    let row = |text: &str| Row {
        key: "k".into(),
        kind: RowKind::Agent { at: None, meta: false, streaming: false, rewind: None, fork: None, runs: Vec::new() },
        text: text.into(),
        turn: None,
    };
    assert_eq!(body_text(&row("")), "(empty response)");
    assert_eq!(body_text(&row("  \n")), "  \n");
}

#[gpui_kit::test]
fn a_running_command_turns_its_spinner_counts_its_time_and_opens_to_no_output(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let events = vec![
        event("session.start", 1, json!({"prompt": "go"})),
        event(
            "session.delta",
            2,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"make\\nmake test\"}", "at": 5_000.0}),
        ),
    ];
    load(&view, events, cx);
    open_all(&view, &["group:tool:u1:a", "u1:call:a"], cx);
    assert!(cx.debug_bounds("call-line").is_some(), "the open group's command row");
    assert!(cx.debug_bounds("clock-slot").is_some(), "the running command's timer");
    assert!(cx.debug_bounds("no-output").is_none(), "a running command opened to No output");
}

fn edited(patch: serde_json::Value, extra: serde_json::Value) -> Vec<SessionEvent> {
    let mut result = json!({"kind": "tool_result", "toolUseId": "a", "text": "ok", "patch": patch});
    result.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    settled(
        vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": "a", "text": "{\"file_path\":\"a.rs\"}"}),
            ),
            event("session.delta", 3, result),
        ],
        4,
    )
}

fn lines(n: usize, sign: char) -> serde_json::Value {
    json!((0..n).map(|i| format!("{sign}line {i}")).collect::<Vec<_>>())
}

#[gpui_kit::test]
fn an_edit_row_says_a_new_files_lines_and_opens_a_head_per_file(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let one = json!([{"path": "a.rs", "hunks": [{"oldStart": 0, "oldLines": 0, "newStart": 1, "newLines": 3, "lines": lines(3, '+')}]}]);
    load(&view, edited(one, json!({})), cx);
    open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
    let facts = view.read_with(cx, |v, _| {
        v.rows().iter().find_map(|r| match &r.kind {
            RowKind::Calls { calls, .. } => calls.first().map(|c| c.files.clone()),
            _ => None,
        })
    });
    assert_eq!(facts.map(|f| (f.len(), f[0].created, f[0].added)), Some((1, true, 3)), "one new file of 3 lines");
    let open_diff = cx.debug_bounds("edit-open-diff").expect("Open diff on the open one-file edit");
    cx.simulate_mouse_move(open_diff.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tooltip").is_some(), "the edit's Open diff tooltip is not the app's");
    cx.simulate_mouse_move(gpui_kit::point(px(1.), px(1.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("file-head").is_none(), "a one-file edit has no head of its own");
    let two = json!([
        {"path": "a.rs", "hunks": [{"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-a", "+b"]}]},
        {"path": "b.rs", "hunks": [{"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-c", "+d"]}]},
    ]);
    load(&view, edited(two, json!({"patchCut": true})), cx);
    open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
    assert!(cx.debug_bounds("file-head").is_some(), "no head per file in a two-file edit");
    assert!(cx.debug_bounds("edit-cut").is_some(), "no word that the transcript kept part of the change");
}

#[gpui_kit::test]
fn a_long_hunk_folds_to_its_first_16_lines_and_fewer_lines_folds_it_again(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let long = json!([{"path": "a.rs", "hunks": [{"oldStart": 1, "oldLines": 0, "newStart": 1, "newLines": 30, "lines": lines(30, '+')}]}]);
    load(&view, edited(long, json!({})), cx);
    open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
    let folded = cx.debug_bounds("diff").expect("the open edit's hunks");
    let more = cx.debug_bounds("diff-more").expect("+N lines under 30 lines of hunks");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    let whole = cx.debug_bounds("diff").expect("the hunks after +N lines");
    assert!(whole.size.height > folded.size.height + px(200.), "+N lines left the hunks at {:?}", whole.size.height);
    view.update(cx, |v, cx| v.scroll_to_end(cx));
    cx.run_until_parked();
    let whole = cx.debug_bounds("diff").expect("the hunks at the end");
    let fewer = cx.debug_bounds("diff-more").expect("Fewer lines under the whole hunks");
    cx.simulate_click(fewer.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("diff").is_some_and(|b| b.size.height < whole.size.height), "Fewer lines did not fold the hunks");
}

#[gpui_kit::test]
fn a_cut_output_says_how_much_of_it_the_host_kept(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let output = "x".repeat(12 * 1024);
    let events = settled(
        vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"cat big\"}"}),
            ),
            event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": output, "exitCode": 0, "bytes": 40 * 1024})),
        ],
        4,
    );
    load(&view, events, cx);
    open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
    assert!(cx.debug_bounds("output-kept").is_some(), "no First 12 KB of 40 KB");
    assert_eq!(bytes_of(12 * 1024, 40 * 1024), "12 of 40 KB");
    assert_eq!(bytes_of(900, 40 * 1024), "900 B of 40 KB");
}

#[gpui_kit::test]
fn an_open_groups_list_keeps_its_cap_and_grows_by_what_a_call_opened(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let mut events = vec![event("session.start", 1, json!({"prompt": "go"}))];
    for n in 0..20u64 {
        let id = format!("r{n}");
        events.push(event(
            "session.delta",
            2 + 2 * n,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": id, "text": format!("{{\"command\":\"echo {n}\"}}")}),
        ));
        events.push(event(
            "session.delta",
            3 + 2 * n,
            json!({"kind": "tool_result", "toolUseId": id, "text": "line one\nline two", "exitCode": 0}),
        ));
    }
    load(&view, settled(events, 60), cx);
    open_all(&view, &["u1", "group:tool:u1:r0"], cx);
    let capped = cx.debug_bounds("calls").expect("the open group's list");
    // min(18rem, 50dvh) in an 800 px window.
    assert!((capped.size.height - px(288.)).abs() < px(1.), "the cap is {:?}", capped.size.height);
    assert!(cx.debug_bounds("calls-fade-bottom").is_some(), "no fade over the rows below the cap");
    assert!(cx.debug_bounds("calls-fade-top").is_none(), "a fade over the top of a list scrolled to its top");
    open_all(&view, &["u1:call:r0"], cx);
    cx.run_until_parked();
    let grown = cx.debug_bounds("calls").expect("the list with a call open");
    assert!(
        grown.size.height > capped.size.height && grown.size.height < px(400.),
        "opening a call left the list at {:?}",
        grown.size.height
    );
}

#[gpui_kit::test]
fn a_lone_work_row_opens_to_its_body(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let events = settled(
        vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event("session.notify", 2, json!({"notify": "me", "text": "Built it.\nAll 40 tests pass.\n"})),
            event("session.delta", 3, json!({"kind": "text", "text": "Done."})),
        ],
        4,
    );
    load(&view, events, cx);
    view.update(cx, |v, cx| v.toggle("u1", cx));
    cx.run_until_parked();
    let key = view.read_with(cx, |v, _| v.rows().iter().find(|r| matches!(r.kind, RowKind::Work { .. })).map(|r| r.key.clone()));
    let kinds = view.read_with(cx, |v, _| v.rows().iter().map(|r| format!("{:?}", r.kind)).collect::<Vec<_>>());
    let key = key.unwrap_or_else(|| panic!("the thinking row stands alone: {kinds:?}"));
    let shut = cx.debug_bounds("work").expect("the work row");
    cx.simulate_click(shut.center(), Modifiers::none());
    cx.run_until_parked();
    let open = view.read_with(cx, |v, _| v.rows().iter().any(|r| r.key == key && matches!(r.kind, RowKind::Work { open: true, .. })));
    assert!(open, "a press left the lone row shut: {kinds:?}");
    assert!(cx.debug_bounds("work").is_some_and(|b| b.size.height > shut.size.height), "the open row shows no body");
}

#[gpui_kit::test]
fn tab_reaches_a_fold_and_enter_opens_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let events = settled(
        vec![
            event("session.start", 1, json!({})),
            event(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a.rs\"}"}),
            ),
            event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
            event("session.delta", 4, json!({"kind": "text", "text": "Done."})),
        ],
        5,
    );
    load(&view, events, cx);
    // With nothing focused a test window's keys reach no key context; Root's Tab runs `focus_next`.
    cx.update(|window, cx| window.focus_next(cx));
    press(cx, "enter");
    let open = view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: true })));
    assert!(open, "Tab then Enter left the fold shut");
    press(cx, "space");
    let shut = view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: false })));
    assert!(shut, "Space left the fold open");
}

#[gpui_kit::test]
fn a_replys_meta_line_shows_under_the_pointer_and_with_focus_inside_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let events =
        settled(vec![event("session.start", 1, json!({})), event("session.delta", 2, json!({"kind": "text", "text": "Done."}))], 3);
    load(&view, events, cx);
    assert!(cx.debug_bounds("meta").is_some() && cx.debug_bounds("meta-shown").is_none(), "the meta line shows at rest");
    let reply = cx.debug_bounds("meta").expect("the reply's meta line");
    cx.simulate_mouse_move(reply.center(), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("meta-shown").is_some(), "the pointer over the reply left its meta line hidden");
    cx.simulate_mouse_move(reply.center() + gpui_kit::point(px(0.), px(600.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("meta-shown").is_none(), "the meta line stayed after the pointer left");
    cx.update(|window, cx| window.focus_next(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("meta-shown").is_some(), "focus on the reply's copy button left its meta line hidden");
}

#[gpui_kit::test]
fn the_files_card_and_the_slate_line_stand_inside_the_reply_over_its_meta_line(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let mut events = vec![
        event("session.start", 1, json!({})),
        event("session.delta", 2, json!({"kind": "text", "text": "Done."})),
        event("session.slate", 3, json!({"cause": "write", "version": 1, "by": "agent", "pieces": []})),
    ];
    events = settled(events, 4);
    let files = json!([{"path": "src/a.rs", "kind": "modified", "additions": 1, "deletions": 0}]);
    events.push(event("session.changes", 6, json!({"from": "a", "to": "b", "moved": [], "files": files})));
    load(&view, events, cx);
    let found = |cx: &mut VisualTestContext, s: &'static str| cx.debug_bounds(s).unwrap_or_else(|| panic!("no {s}"));
    let (card, slate, meta) = (found(cx, "changed-files"), found(cx, "slate-updated"), found(cx, "meta"));
    assert_eq!(slate.top() - card.bottom(), px(4.), "the slate line's `mt-1` under the files card");
    assert_eq!(meta.top() - slate.bottom(), px(6.), "the meta line's `mt-1.5` under the slate line");
    let last = view.read_with(cx, |v, _| v.rows().len() - 1);
    let row = found(cx, Box::leak(format!("row-{last}").into_boxed_str()));
    assert!(row.bottom() - meta.bottom() >= px(2. + 16.), "the reply's `py-0.5` and `pb-4` under its meta line");
    cx.simulate_mouse_move(card.center(), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("meta-shown").is_some(), "the pointer over the files card left the reply's meta line hidden");
    cx.simulate_mouse_move(meta.center() + gpui_kit::point(px(0.), px(600.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("meta-shown").is_none(), "the meta line stayed after the pointer left");
    let reply = view.read_with(cx, |v, _| v.rows().iter().position(|r| matches!(r.kind, RowKind::Agent { .. })).unwrap());
    let reply = found(cx, Box::leak(format!("row-{reply}").into_boxed_str()));
    assert_eq!(card.left() - reply.left(), px(4.), "the files card stands inside the reply's `px-1`");
    cx.simulate_mouse_move(reply.center(), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("meta-shown").is_some(), "the pointer over the reply left its meta line, under the slate line, hidden");
}

#[gpui_kit::test]
fn a_tree_rows_figures_stand_in_columns_of_their_own(cx: &mut TestAppContext) {
    // `DiffStatLabel`'s aligned layout: two `4ch` columns, so a row's stat is as wide whatever its figures.
    let width = |added: u64, removed: u64, cx: &mut TestAppContext| {
        let (view, cx) = window(cx);
        let mut events =
            settled(vec![event("session.start", 1, json!({})), event("session.delta", 2, json!({"kind": "text", "text": "Done."}))], 3);
        let files = json!([{"path": "a.rs", "kind": "modified", "additions": added, "deletions": removed}]);
        events.push(event("session.changes", 5, json!({"from": "a", "to": "b", "moved": [], "files": files})));
        load(&view, events, cx);
        cx.debug_bounds("stat-aligned").expect("the file row's stat").size.width
    };
    assert_eq!(width(2, 0, cx), width(1200, 999, cx), "a tree row's figures run together as the head's do");
}

/// The shell's root as far as the thread's keys read it.
struct Frame(Entity<ThreadView>);

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use crate::screens::app_shell::keys;
        div().size_full().key_context(format!("{} {}", keys::SHELL, keys::THREAD_OPEN).as_str()).child(self.0.clone())
    }
}

#[gpui_kit::test]
fn find_paints_its_match_in_a_command_row(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
        cx.bind_keys(crate::screens::app_shell::keys::bindings(cfg!(target_os = "macos"), &Default::default()));
    });
    let view = cx.new(ThreadView::new);
    let frame = cx.new(|_| Frame(view.clone()));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(frame, window, cx));
    cx.simulate_resize(size(px(900.), px(800.)));
    let events = settled(
        vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"grep kiwi notes\"}"}),
            ),
            event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "found", "exitCode": 0})),
        ],
        4,
    );
    load(&view, events, cx);
    open_all(&view, &["u1", "group:tool:u1:a"], cx);
    assert!(cx.debug_bounds("find-marked").is_none(), "a mark while the bar is shut");
    cx.update(|window, cx| window.focus(&gpui_kit::Focusable::focus_handle(view.read(cx), cx), cx));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-f" } else { "ctrl-f" });
    cx.simulate_input("kiwi");
    let tools = cx.debug_bounds("find-toggle-3").expect("Include tool calls");
    cx.simulate_click(tools.center(), Modifiers::none());
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    let status = view.read_with(cx, |v, _| v.find_state().status());
    assert!(cx.debug_bounds("find-marked").is_some(), "the command row's match is not painted: {status:?}");
}
