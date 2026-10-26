// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[test]
fn a_settled_turn_keeps_the_message_one_fold_and_the_last_reply() {
    let rows = rows(&turn(true), &HashSet::new());
    assert_eq!(shown(&rows), [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Done here.")]);
    assert_eq!(rows[1].kind, RowKind::Fold { open: false });
    assert_eq!(rows[1].turn.as_deref(), Some("u1"));
    assert_eq!(rows[2].kind, agent(None, true, false));
}

#[test]
fn an_opened_turn_draws_its_hidden_rows_in_place_under_the_fold() {
    let rows = rows(&turn(true), &opened(&["u1"]));
    assert_eq!(
        shown(&rows),
        [("person", "fix it"), ("fold", "Worked for 12s"), ("agent", "Looking."), ("group", "Ran 1 command"), ("agent", "Done here."),]
    );
    assert_eq!(rows[1].kind, RowKind::Fold { open: true });
    assert_eq!(rows[2].kind, agent(None, false, false));
}

#[test]
fn a_turn_without_a_clock_reads_worked() {
    let events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a\"}"})),
        base("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "ok"})),
        base("session.delta", 4, json!({"kind": "text", "text": "Done."})),
        base("session.done", 5, json!({"result": {"status": "completed"}})),
        base("session.end", 6, json!({"exitCode": 0, "sawResult": true})),
    ];
    assert_eq!(shown(&rows(&events, &HashSet::new())), [("person", "go"), ("fold", "Worked"), ("agent", "Done.")]);
}

#[test]
fn the_person_row_carries_its_stamp_and_files() {
    let events = vec![
        base(
            "session.start",
            1,
            json!({"prompt": "look", "at": 5_000.0, "requestId": "q1", "attachments": [
                {"mediaType": "image/png", "bytes": 1200, "name": "shot.png"},
                {"mediaType": "text/plain", "bytes": 30}
            ]}),
        ),
        base("session.steer", 2, json!({"prompt": "and this", "at": 6_000.0, "attachments": [{"mediaType": "image/jpeg", "bytes": 9}]})),
    ];
    let rows = rows(&events, &HashSet::new());
    assert_eq!(
        rows[0].kind,
        RowKind::Person {
            fork: None,
            full: false,
            steered: false,
            at: Some(5_000.0),
            files: vec![
                FileTile { id: Some("q1".into()), ..tile("shot.png", 1200, true, 0, "image/png") },
                FileTile { id: Some("q1".into()), ..tile("file", 30, false, 1, "text/plain") },
            ],
        }
    );
    assert_eq!(
        rows[1].kind,
        RowKind::Person { at: Some(6_000.0), files: vec![tile("image", 9, true, 0, "image/jpeg")], fork: None, full: false, steered: true },
        "a message sent while the agent worked is not marked so"
    );
    assert_eq!(rows[1].text, "and this");
    assert_eq!(name(&rows[2].kind), "working");
}

#[test]
fn a_running_turn_reads_working_its_replies_and_one_live_line() {
    let mut events = turn(false);
    events[5] = base(
        "session.delta",
        6,
        json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "c", "text": "{\"command\":\"cargo test\"}", "at": 4_000.0}),
    );
    events.truncate(6);
    let rows = rows(&events, &HashSet::new());
    assert_eq!(shown(&rows), [("person", "fix it"), ("working", "Working"), ("agent", "Looking."), ("live", "cargo test")]);
    assert_eq!(rows[1].kind, RowKind::Working { since: Some(1_000.0), waiting: false, retry: None, wait: None });
    assert_eq!(rows[2].kind, agent(None, false, false));
    let group = "group:tool:u1:a".to_owned();
    assert_eq!(
        rows[3].kind,
        RowKind::Live {
            group,
            glyph: Glyph::Terminal,
            verb: Some("Running".into()),
            mono: true,
            since: Some(4_000.0),
            open: false,
            active: true
        }
    );
    assert_eq!(rows[3].key, "live");
}

#[test]
fn a_streaming_reply_is_marked_and_thinking_closes_the_running_turn() {
    let events = vec![base("session.start", 1, json!({"prompt": "go"})), base("session.delta", 2, json!({"kind": "text", "text": "Hel"}))];
    let rows = rows(&events, &HashSet::new());
    assert_eq!(shown(&rows), [("person", "go"), ("working", "Working"), ("agent", "Hel"), ("thinking", "Thinking")]);
    assert_eq!(rows[2].kind, agent(None, false, true));
}

#[test]
fn a_finished_trailing_call_reads_ran_and_a_failed_one_leaves_thinking() {
    let mut events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base(
            "session.delta",
            2,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"ls -la\\nmore\"}"}),
        ),
        base("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "out", "exitCode": 0})),
    ];
    let live = rows(&events, &HashSet::new());
    assert_eq!(shown(&live), [("person", "go"), ("working", "Working"), ("live", "ls -la")]);
    assert!(matches!(&live[2].kind, RowKind::Live { verb: Some(v), since: None, .. } if v == "Ran"));
    events.push(base(
        "session.delta",
        4,
        json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "b", "text": "{\"command\":\"false\"}"}),
    ));
    events.push(base(
        "session.delta",
        5,
        json!({"kind": "tool_result", "toolUseId": "b", "text": "Exit code 1\nboom", "isError": true, "exitCode": 1}),
    ));
    assert_eq!(shown(&rows(&events, &HashSet::new())), [("person", "go"), ("working", "Working"), ("thinking", "Thinking")]);
}

#[test]
fn an_open_group_lists_its_calls_with_their_results() {
    let events = vec![
        base("session.start", 1, json!({"prompt": "go", "cwd": "/w"})),
        base(
            "session.delta",
            2,
            json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"false\"}", "at": 1_000.0}),
        ),
        base(
            "session.delta",
            3,
            json!({"kind": "tool_result", "toolUseId": "a", "text": "Exit code 1\n\nboom\n", "isError": true, "exitCode": 1, "at": 1_850.0}),
        ),
        base(
            "session.delta",
            4,
            json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": "b", "text": "{\"file_path\":\"/w/src/a.rs\"}"}),
        ),
        base(
            "session.delta",
            5,
            json!({"kind": "tool_result", "toolUseId": "b", "text": "updated", "patch": [{"path": "/w/src/a.rs", "hunks": [
                {"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-a", "+b"]}
            ]}]}),
        ),
        base("session.delta", 6, json!({"kind": "tool_use", "toolName": "Grep", "toolUseId": "c", "text": "{\"pattern\":\"fn main\"}"})),
        base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "c", "text": "src/main.rs:1\nsrc/lib.rs:4"})),
        base("session.delta", 8, json!({"kind": "text", "text": "Done."})),
    ];
    let group = "group:tool:u1:a";
    let rows = rows(&events, &opened(&[group, "u1:call:c"]));
    assert_eq!(rows[2].kind, RowKind::Group { group: group.into(), glyph: Glyph::Hammer, open: true });
    assert_eq!(rows[2].text, "Ran 1 command, changed 1 file, and searched code 1 time");
    let RowKind::Calls { calls, .. } = &rows[3].kind else { panic!("no calls row: {rows:?}") };
    assert_eq!(rows[3].key, format!("{group}:calls"));
    let [bash, edit, grep] = calls.as_slice() else { panic!("{calls:?}") };
    assert_eq!((bash.label.as_str(), bash.command.as_deref(), bash.mono), ("false", Some("false"), true));
    assert_eq!((bash.failed, bash.exit_code, bash.duration_ms, bash.glyph), (true, Some(1), Some(850.0), Glyph::Terminal));
    assert_eq!(bash.body.as_deref(), Some("boom"));
    assert_eq!((edit.verb.as_deref(), edit.label.as_str(), edit.glyph, edit.mono), (Some("Edited"), "src/a.rs", Glyph::SquarePen, true));
    assert_eq!(
        edit.patch.as_deref(),
        Some("diff --git a/w/src/a.rs b/w/src/a.rs\n--- a/w/src/a.rs\n+++ b/w/src/a.rs\n@@ -1,1 +1,1 @@\n-a\n+b")
    );
    assert_eq!((grep.label.as_str(), grep.glyph, grep.open, grep.body.as_deref()), ("src/main.rs:1", Glyph::Search, true, None));
    assert!(!bash.open);
}

#[test]
fn a_group_reads_its_one_kind_and_thinking_alone_reads_thinking() {
    let events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "thinking", "text": "Let me see"})),
        base("session.delta", 3, json!({"kind": "text", "text": "Hm."})),
        base("session.delta", 4, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a\"}"})),
        base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
        base("session.delta", 6, json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "b", "text": "{\"file_path\":\"b\"}"})),
        base("session.delta", 7, json!({"kind": "tool_result", "toolUseId": "b", "text": "y"})),
        base("session.delta", 8, json!({"kind": "text", "text": "Done."})),
        base("session.done", 9, json!({"result": {"status": "completed", "durationMs": 4_250}})),
        base("session.end", 10, json!({"exitCode": 0, "sawResult": true})),
    ];
    let rows = rows(&events, &opened(&["u1"]));
    assert_eq!(
        shown(&rows),
        [
            ("person", "go"),
            ("fold", "Worked for 4.3s"),
            ("group", "Thinking"),
            ("agent", "Hm."),
            ("group", "Read 2 files"),
            ("agent", "Done.")
        ]
    );
    assert!(matches!(rows[2].kind, RowKind::Group { glyph: Glyph::Bot, .. }));
    assert!(matches!(rows[4].kind, RowKind::Group { glyph: Glyph::Eye, .. }));
    assert_eq!(rows[2].key, "group:u1:w@2");
}

#[test]
fn an_interrupted_turn_never_folds_and_says_you_stopped() {
    let mut events = turn(false);
    events.push(base("session.done", 9, json!({"result": {"status": "interrupted", "durationMs": 4_200}})));
    events.push(base("session.end", 10, json!({"exitCode": 0, "sawResult": true})));
    let rows = rows(&events, &HashSet::new());
    assert_eq!(
        shown(&rows),
        [
            ("person", "fix it"),
            ("agent", "Looking."),
            ("group", "Ran 1 command"),
            ("agent", "Done here."),
            ("stopped", "You stopped after 4.2s"),
            ("footer", "interrupted"),
        ]
    );
    assert_eq!(rows[5].kind, RowKind::Footer { word: FooterWord::Interrupted, why: None });
    assert_eq!(rows[4].key, "stop:u1");
}

#[test]
fn a_stop_that_never_reached_the_computer_says_why_under_its_word() {
    let said = "The stop did not reach box: its computer did not answer";
    let ended = |unreached: bool| {
        let mut result = json!({"status": "interrupted", "error": said});
        if unreached {
            result["unreached"] = json!(true);
        }
        let events = vec![
            base("session.start", 1, json!({"prompt": "fix it"})),
            base("session.done", 2, json!({"result": result})),
            base("session.end", 3, json!({"exitCode": 0, "sawResult": true})),
        ];
        rows(&events, &HashSet::new()).last().map(|r| r.kind.clone())
    };
    assert_eq!(ended(true), Some(RowKind::Footer { word: FooterWord::Interrupted, why: Some(said.to_owned()) }));
    assert_eq!(ended(false), Some(RowKind::Footer { word: FooterWord::Interrupted, why: None }), "a stop the agent heard says why");
}

#[test]
fn a_stop_with_no_clock_says_you_stopped_this_response() {
    let events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "text", "text": "Hi"})),
        base("session.done", 3, json!({"result": {"status": "interrupted"}})),
        base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
    ];
    assert_eq!(shown(&rows(&events, &HashSet::new()))[2], ("stopped", "You stopped this response"));
}

#[test]
fn a_failed_turn_keeps_its_error_through_the_fold() {
    let mut events = turn(false);
    events.push(base("session.done", 9, json!({"result": {"status": "failed", "error": "API down", "durationMs": 2_000}})));
    events.push(base("session.end", 10, json!({"exitCode": 1, "sawResult": true})));
    let rows = rows(&events, &HashSet::new());
    assert_eq!(shown(&rows), [("person", "fix it"), ("fold", "Worked for 2.0s"), ("agent", "Done here."), ("error", "API down")]);
}

#[test]
fn a_failure_without_words_says_failed_under_the_transcript() {
    let mut events = turn(false);
    events.push(base("session.done", 9, json!({"result": {"status": "failed"}})));
    events.push(base("session.end", 10, json!({"exitCode": 1, "sawResult": true})));
    let rows = rows(&events, &HashSet::new());
    assert_eq!(rows.last().unwrap().kind, RowKind::Footer { word: FooterWord::Failed, why: None });
    assert!(!rows.iter().any(|r| r.kind == RowKind::Error));
}

#[test]
fn a_session_that_exits_without_a_result_fails_its_turn() {
    let mut events = turn(false);
    events.push(base("session.end", 9, json!({"exitCode": 137, "sawResult": false})));
    let rows = rows(&events, &HashSet::new());
    assert_eq!(rows.last().map(|r| (name(&r.kind), r.text.as_str())), Some(("error", "Session exited without a result (exit code 137)")));
}

#[test]
fn a_usage_limit_says_limit_reached() {
    let events = vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.done", 2, json!({"result": {"status": "failed", "error": "limit", "limit": {}}})),
        base("session.end", 3, json!({"exitCode": 0, "sawResult": true})),
    ];
    assert_eq!(shown(&rows(&events, &HashSet::new())), [("person", "go"), ("footer", "limit reached")]);
}

#[test]
fn notices_and_compactions_stand_alone_through_the_fold() {
    let mut events = turn(false);
    events.insert(2, base("session.notice", 20, json!({"text": "machine woke up"})));
    events.insert(3, base("session.compacted", 21, json!({"before": 180_000, "after": 42_100})));
    events.push(base("session.done", 9, json!({"result": {"status": "completed", "durationMs": 1_000}})));
    events.push(base("session.end", 10, json!({"exitCode": 0, "sawResult": true})));
    let rows = rows(&events, &HashSet::new());
    assert_eq!(
        shown(&rows),
        [
            ("person", "fix it"),
            ("fold", "Worked for 1.0s"),
            ("work", "Machine woke up"),
            ("work", "Compacted the context, 180k to 42.1k tokens"),
            ("agent", "Done here."),
        ]
    );
    assert_eq!(rows[2].kind, RowKind::Work { tone: Tone::Notice, glyph: Glyph::Info, body: None, open: false });
    assert_eq!(rows[3].kind, RowKind::Work { tone: Tone::Compaction, glyph: Glyph::Minimize, body: None, open: false });
}

#[test]
fn keys_hold_across_a_new_event() {
    let before = rows(&turn(false), &HashSet::new());
    let mut events = turn(false);
    events.push(base("session.delta", 9, json!({"kind": "text", "text": " More.", "messageId": "m2"})));
    let after = rows(&events, &HashSet::new());
    let keys = |rows: &[Row]| rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>();
    assert_eq!(keys(&before), keys(&after));
    assert_eq!(after.iter().find(|r| r.key == "u1:m@7").map(|r| r.text.as_str()), Some("Done here. More."));
}

#[test]
fn a_turn_whose_harness_reports_a_negative_duration_still_folds() {
    let mut events = turn(false);
    events.push(base("session.done", 9, json!({"result": {"status": "completed", "durationMs": -5.0, "waitedMs": 2.0}})));
    events.push(base("session.end", 10, json!({"exitCode": 0, "sawResult": true})));
    assert_eq!(name(&rows(&events, &HashSet::new())[1].kind), "fold");
}

#[test]
fn durations_read_as_the_web_app_writes_them() {
    assert_eq!(fmt_duration(400.0), "400ms");
    assert_eq!(fmt_duration(4_250.0), "4.3s");
    assert_eq!(fmt_duration(12_400.0), "12s");
    assert_eq!(fmt_duration(125_000.0), "2m 5s");
    assert_eq!(fmt_tokens(42_100.0), "42.1k");
    assert_eq!(fmt_tokens(4_270.0), "4.27k");
    assert_eq!(fmt_tokens(180_000.0), "180k");
}
