// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/thread-read.test.ts:43-320 (a thread's messages), :322-384 (the latest turn and
//! its final reply), :386-450 (threadReadText, threadMarkdown over threadMessages), :475-503 (openAsk, leadAsk) and
//! :505-518 (threadSeed). Skipped: :452-473 (threadRowLines, fmtClock, the two silences, turnEndLine: module words::turn, ported in
//! tests/turn.rs) and :520-536 (worktreeOf, repoPathOf: module project_path).

use serde_json::{json, Value};
use wsp_wire::types::thread_read::{ThreadMessage, THREAD_SEED_CHARS};
use wsp_wire::views::session_events::{SessionEvent, SessionPermissionEvent, TurnResult};
use wsp_words::thread_read::*;
use wsp_words::words::turn::{fmt_clock, notify_line, thread_read_text, turn_end_line, NotifyLength, NEWER_TURN_LINE};

const AT: i64 = 1_788_930_312_000;

fn scope() -> Value {
    json!({ "workspaceId": "w1", "sessionId": "s1", "threadId": "t1" })
}

/// `{ type, ...SCOPE, ...fields }` of the TS event literals.
fn ev(kind: &str, fields: Value) -> Value {
    let mut v = scope();
    let o = v.as_object_mut().unwrap();
    o.insert("type".to_string(), json!(kind));
    for (k, x) in fields.as_object().unwrap() {
        o.insert(k.clone(), x.clone());
    }
    v
}

fn events(v: Vec<Value>) -> Vec<SessionEvent> {
    v.into_iter().map(|e| serde_json::from_value(e).unwrap()).collect()
}

fn result(v: Value) -> TurnResult {
    serde_json::from_value(v).unwrap()
}

fn msg(who: &str, at: Option<i64>, text: &str) -> ThreadMessage {
    let mut v = json!({ "who": who, "text": text });
    if let Some(at) = at {
        v["at"] = json!(at);
    }
    serde_json::from_value(v).unwrap()
}

fn who_of(m: &ThreadMessage) -> String {
    serde_json::to_value(m.who).unwrap().as_str().unwrap().to_string()
}

fn pairs(rows: &[ThreadMessage]) -> Vec<(String, String)> {
    rows.iter().map(|m| (who_of(m), m.text.clone())).collect()
}

fn want(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter().map(|(w, t)| (w.to_string(), t.to_string())).collect()
}

fn texts(rows: &[ThreadMessage]) -> Vec<String> {
    rows.iter().map(|m| m.text.clone()).collect()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn messages(v: Vec<Value>) -> Vec<ThreadMessage> {
    thread_messages(&events(v), "t1")
}

/// The zone of the computer reading it, which is the zone fmtClock and threadMarkdown read the clock in.
fn local(at: f64) -> i64 {
    let stamp = jiff::Timestamp::from_millisecond(at as i64).unwrap();
    jiff::tz::TimeZone::system().to_offset(stamp).seconds() as i64
}

/// The events one turn of a scripted thread leaves: the person's message, two pieces of text with a tool call
/// between them, and the reply the harness reported.
fn turn_with(prompt: &str, text: &str, result: Value) -> Vec<Value> {
    let head: String = text.chars().take(4).collect();
    let tail: String = text.chars().skip(4).collect();
    vec![
        ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": prompt })),
        ev("session.delta", json!({ "turnId": "u1", "at": AT + 1_000, "kind": "text", "text": head })),
        ev(
            "session.delta",
            json!({ "turnId": "u1", "at": AT + 2_000, "kind": "tool_use", "toolName": "Bash", "toolUseId": "toolu_1", "text": json!({ "command": "pnpm test" }).to_string() }),
        ),
        ev("session.delta", json!({ "turnId": "u1", "at": AT + 3_000, "kind": "text", "text": tail })),
        ev("session.done", json!({ "turnId": "u1", "at": AT + 4_000, "result": result })),
        ev("session.end", json!({ "turnId": "u1", "at": AT + 5_000, "exitCode": 0, "sawResult": true })),
    ]
}

fn turn(prompt: &str, text: &str) -> Vec<Value> {
    turn_with(prompt, text, json!({ "status": "completed", "text": text }))
}

mod a_thread_s_messages {
    use super::*;

    #[test]
    fn folds_the_transcript_into_who_spoke_when_and_the_text_with_the_tool_call_one_row() {
        let rows = messages(turn("build it", "read the ticket, then built it"));
        assert_eq!(
            rows,
            vec![
                msg("person", Some(AT), "build it"),
                msg("agent", Some(AT + 1_000), "read"),
                msg("tool", Some(AT + 2_000), "$ pnpm test"),
                msg("agent", Some(AT + 3_000), " the ticket, then built it"),
                msg("turn", Some(AT + 4_000), "completed"),
            ]
        );
    }

    #[test]
    fn reads_a_turn_that_never_started_as_a_turn_of_its_own_with_the_words_it_was_sent() {
        let mut all = turn("build it", "built it");
        all.push(ev(
            "session.capped",
            json!({ "turnId": "u2", "at": AT + 6_000, "placeId": "here", "place": "hetzner", "running": 1, "atOnce": 1 }),
        ));
        all.push(ev(
            "session.end",
            json!({ "turnId": "u2", "at": AT + 9_000, "exitCode": null, "sawResult": false, "reason": "the host restarted", "unstarted": true, "prompt": "and the tests" }),
        ));
        let rows = messages(all);
        assert_eq!(
            rows[rows.len() - 2..].to_vec(),
            vec![msg("person", Some(AT + 9_000), "and the tests"), msg("turn", Some(AT + 9_000), "failed: the host restarted")]
        );
    }

    #[test]
    fn reads_the_harness_s_two_messages_as_two_rows_the_reply_given_while_a_background() {
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "hold for 90 seconds" })),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 1_000, "kind": "text", "text": "Waiting for the hold to complete.", "messageId": "msg_a" }),
            ),
            ev("session.delta", json!({ "turnId": "u1", "at": AT + 91_000, "kind": "text", "text": "Done. ", "messageId": "msg_b" })),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 91_500, "kind": "text", "text": "It ended with exit code 0.", "messageId": "msg_b" }),
            ),
        ]);
        assert_eq!(
            rows,
            vec![
                msg("person", Some(AT), "hold for 90 seconds"),
                msg("agent", Some(AT + 1_000), "Waiting for the hold to complete."),
                msg("agent", Some(AT + 91_000), "Done. It ended with exit code 0."),
            ]
        );
    }

    #[test]
    fn leaves_a_harness_s_note_out_a_read_is_the_words_of_a_thread_and_the_note_is_the() {
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "say ready" })),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 1, "kind": "note", "text": "loading hooks from both files; prefer a single representation for this layer" }),
            ),
            ev("session.delta", json!({ "turnId": "u1", "at": AT + 2, "kind": "text", "text": "ready" })),
        ]);
        assert_eq!(rows, vec![msg("person", Some(AT), "say ready"), msg("agent", Some(AT + 2), "ready")]);
    }

    #[test]
    fn keeps_the_pieces_of_one_message_in_one_row_and_text_a_harness_named_no_message_f() {
        let pieces = |first: Option<&str>, second: Option<&str>| -> Vec<String> {
            let mut one = json!({ "turnId": "u1", "at": AT + 1, "kind": "text", "text": "half a " });
            if let Some(id) = first {
                one["messageId"] = json!(id);
            }
            let mut two = json!({ "turnId": "u1", "at": AT + 2, "kind": "text", "text": "sentence" });
            if let Some(id) = second {
                two["messageId"] = json!(id);
            }
            messages(vec![
                ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "go" })),
                ev("session.delta", one),
                ev("session.delta", two),
            ])
            .iter()
            .filter(|row| who_of(row) == "agent")
            .map(|row| row.text.clone())
            .collect()
        };
        assert_eq!(pieces(Some("msg_a"), Some("msg_a")), strs(&["half a sentence"]));
        assert_eq!(pieces(None, None), strs(&["half a sentence"]));
        assert_eq!(pieces(None, Some("msg_a")), strs(&["half a sentence"]));
    }

    #[test]
    fn prints_the_line_a_thread_is_told_where_its_worktree_went_and_where_it_was_left_b() {
        let rows = messages(vec![
            ev("session.moved", json!({ "turnId": "u1", "from": "/w/tree", "to": "/w/proj", "branch": "feat/x", "fresh": true })),
            ev("session.behind", json!({ "turnId": "u1", "text": "this worktree is behind pull request #7" })),
            ev("session.start", json!({ "turnId": "u1", "prompt": "go", "at": AT })),
            ev("session.notice", json!({ "turnId": "u1", "text": "The disk of web is 92% full (67.7 of 73.6 GB).", "at": AT })),
        ]);
        assert_eq!(
            pairs(&rows),
            want(&[
                ("turn", "its worktree at /w/tree is gone, so it goes on in /w/proj in a fresh session of its agent"),
                ("turn", "this worktree is behind pull request #7"),
                ("person", "go"),
                ("turn", "The disk of web is 92% full (67.7 of 73.6 GB)."),
            ])
        );
        assert_eq!(
            wsp_words::projects::thread_moved_line("/w/tree", "/w/proj", false),
            "its worktree at /w/tree is gone, so it goes on in /w/proj"
        );
    }

    #[test]
    fn prints_how_long_a_turn_waited_for_this_host_and_the_sentence_the_host_answered_w() {
        let said = "Codex's sign-in for cloud threads on acme's laptop ended. Sign in again in Settings, Agents.";
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "prompt": "fix the upload test", "at": AT })),
            ev("thread.waited", json!({ "turnId": "u1", "ms": 14 * 60_000, "at": AT })),
            ev("thread.waited", json!({ "turnId": "u1", "ms": 0, "said": said, "at": AT })),
            ev("session.notice", json!({ "turnId": "u1", "text": "a host notice", "at": AT })),
        ]);
        assert_eq!(
            pairs(&rows),
            want(&[
                ("person", "fix the upload test"),
                ("turn", "Waited 14 min for your computer"),
                ("turn", said),
                ("turn", "a host notice")
            ])
        );
    }

    #[test]
    fn reads_only_the_named_thread_s_events_and_none_of_a_row_the_runtime_stamped_no_th() {
        let other = vec![
            json!({ "type": "session.start", "workspaceId": "w1", "sessionId": "s2", "threadId": "t2", "at": AT, "prompt": "someone else's task" }),
            json!({ "type": "session.start", "workspaceId": "w1", "sessionId": "s3", "at": AT, "prompt": "a turn from before the stamp" }),
        ];
        let mut all = other.clone();
        all.extend(turn("mine", "done"));
        assert_eq!(texts(&messages(all)), strs(&["mine", "done", "$ pnpm test", "completed"]));
        assert_eq!(messages(other), vec![]);
    }

    #[test]
    fn keeps_a_call_whose_input_arrives_in_pieces_as_one_row_and_draws_its_line_from_th() {
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "edit it" })),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 1, "kind": "tool_use", "toolName": "Edit", "toolUseId": "toolu_1", "text": "{\"file_path\":\"packages/host" }),
            ),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 2, "kind": "tool_use", "toolUseId": "toolu_1", "text": "/src/verbs.ts\"}" }),
            ),
            ev("session.delta", json!({ "turnId": "u1", "at": AT + 3, "kind": "tool_result", "toolUseId": "toolu_1", "text": "ok" })),
        ]);
        assert_eq!(rows, vec![msg("person", Some(AT), "edit it"), msg("tool", Some(AT + 1), "edited packages/host/src/verbs.ts")]);
    }

    #[test]
    fn holds_a_call_in_the_present_until_its_result_lands_so_a_read_of_a_write_nobody_h() {
        let all = vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "write it" })),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 1, "kind": "tool_use", "toolName": "Write", "toolUseId": "toolu_1", "text": json!({ "file_path": "kai.txt" }).to_string() }),
            ),
        ];
        assert_eq!(texts(&messages(all.clone())), strs(&["write it", "writing kai.txt"]));
        let mut done = all;
        done.push(ev("session.delta", json!({ "turnId": "u1", "at": AT + 2, "kind": "tool_result", "toolUseId": "toolu_1", "text": "File created successfully at: kai.txt" })));
        assert_eq!(texts(&messages(done)), strs(&["write it", "wrote kai.txt"]));
    }

    #[test]
    fn gives_a_call_the_harness_named_no_id_for_a_row_of_its_own_and_never_runs_a_new_t() {
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "look around" })),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 1, "kind": "tool_use", "toolName": "Bash", "text": json!({ "command": "ls" }).to_string() }),
            ),
            ev(
                "session.delta",
                json!({ "turnId": "u1", "at": AT + 2, "kind": "tool_use", "toolName": "Bash", "text": json!({ "command": "pwd" }).to_string() }),
            ),
            ev("session.delta", json!({ "turnId": "u1", "at": AT + 3, "kind": "text", "text": "here we are" })),
            // The turn was cut without a done or an end, as a capped transcript leaves one; the next turn opens anyway.
            ev("session.start", json!({ "turnId": "u2", "at": AT + 4 })),
            ev("session.delta", json!({ "turnId": "u2", "at": AT + 5, "kind": "text", "text": "starting over" })),
        ]);
        assert_eq!(
            pairs(&rows),
            want(&[("person", "look around"), ("tool", "$ ls"), ("tool", "$ pwd"), ("agent", "here we are"), ("agent", "starting over")])
        );
    }

    #[test]
    fn takes_the_reply_from_the_result_where_no_text_arrived_which_is_a_transcript_capp() {
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "build it" })),
            ev(
                "session.done",
                json!({ "turnId": "u1", "at": AT + 1_000, "result": { "status": "completed", "text": "built it", "durationMs": 724_000, "costUsd": 0.41 } }),
            ),
        ]);
        assert_eq!(
            rows,
            vec![
                msg("person", Some(AT), "build it"),
                msg("agent", Some(AT + 1_000), "built it"),
                msg("turn", Some(AT + 1_000), "completed  Worked for 12m 4s  $0.41"),
            ]
        );
    }

    #[test]
    fn says_why_a_turn_did_not_complete_on_its_turn_row_and_says_so_too_for_a_turn_the() {
        let failed = messages(turn_with("cut", "", json!({ "status": "failed", "error": "stopped after 15m 00s with no output for 10m" })));
        assert_eq!(failed.last().unwrap(), &msg("turn", Some(AT + 4_000), "failed: stopped after 15m 00s with no output for 10m"));
        let napped = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "loop forever" })),
            ev(
                "session.end",
                json!({ "turnId": "u1", "at": AT + 9_000, "exitCode": null, "sawResult": false, "reason": "machine paused while the agent was working" }),
            ),
        ]);
        assert_eq!(napped.last().unwrap(), &msg("turn", Some(AT + 9_000), "failed: machine paused while the agent was working"));
        let quiet = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "loop forever" })),
            ev("session.end", json!({ "turnId": "u1", "at": AT + 1, "exitCode": null, "sawResult": false })),
        ]);
        assert_eq!(quiet.last().unwrap().text, "failed: turn ended without a result");
    }

    #[test]
    fn puts_a_message_steered_into_a_running_turn_where_the_turn_saw_it_as_a_person_s() {
        let steered = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "build it" })),
            ev("session.delta", json!({ "turnId": "u1", "at": AT + 1, "kind": "text", "text": "on it" })),
            ev("session.steer", json!({ "turnId": "u1", "at": AT + 2, "prompt": "also the codex case" })),
            ev("session.delta", json!({ "turnId": "u1", "at": AT + 3, "kind": "text", "text": "that too" })),
        ]);
        assert_eq!(
            pairs(&steered),
            want(&[("person", "build it"), ("agent", "on it"), ("person", "also the codex case"), ("agent", "that too")])
        );
    }

    #[test]
    fn says_the_turn_s_step_list_as_one_tool_row_the_count_then_a_task_line_per_step_re() {
        let at = |n: i64| AT + n;
        let plan = |n: i64, steps: Value| ev("session.plan", json!({ "turnId": "u1", "at": at(n), "steps": steps }));
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": at(0), "prompt": "work the list" })),
            plan(
                1,
                json!([{ "text": "read", "state": "working" }, { "text": "write", "state": "pending" }, { "text": "ship", "state": "pending" }]),
            ),
            ev("session.delta", json!({ "turnId": "u1", "at": at(2), "kind": "text", "text": "reading" })),
            plan(
                3,
                json!([{ "text": "read", "state": "done" }, { "text": "write", "state": "working" }, { "text": "ship", "state": "pending" }]),
            ),
            ev("session.plan", json!({ "turnId": "u1", "at": at(4), "text": "# Ship it\n\n1. Tag" })),
        ]);
        assert_eq!(
            pairs(&rows),
            want(&[
                ("person", "work the list"),
                ("tool", "Plan: 1 of 3 steps done\n- [x] read\n- [ ] write (working)\n- [ ] ship"),
                ("agent", "reading"),
                ("agent", "# Ship it\n\n1. Tag"),
            ])
        );
    }

    #[test]
    fn says_a_compaction_as_a_tool_row_with_the_figures_the_agent_gave() {
        let rows = messages(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "/compact" })),
            ev("session.compacted", json!({ "turnId": "u1", "at": AT + 1, "before": 27_500, "after": 3_100 })),
            ev("session.compacted", json!({ "turnId": "u1", "at": AT + 2 })),
        ]);
        assert_eq!(
            pairs(&rows),
            want(&[("person", "/compact"), ("tool", "Compacted the context, 27.5k to 3.1k tokens"), ("tool", "Compacted the context")])
        );
    }

    #[test]
    fn says_what_a_turn_changed_as_one_tool_row_above_the_turn_s_end_however_late_the_c() {
        let a40 = "a".repeat(40);
        let b40 = "b".repeat(40);
        let files = json!([
            { "path": "a.ts", "kind": "modified", "additions": 12, "deletions": 3 },
            { "path": "b.ts", "kind": "added", "additions": 2, "deletions": 0 },
        ]);
        let mut all = turn("build it", "built it");
        all.push(ev("session.changes", json!({ "turnId": "u1", "at": AT + 6_000, "from": a40, "to": b40, "files": files, "moved": [] })));
        let rows = messages(all);
        assert_eq!(pairs(&rows[rows.len() - 2..]), want(&[("tool", "Changed 2 files, +14 -3"), ("turn", "completed")]));
        let mut fix = turn("fix", "fixed");
        fix.push(ev(
            "session.changes",
            json!({ "turnId": "u1", "at": AT + 6_000, "from": a40, "to": b40, "files": [files[1].clone()], "moved": [] }),
        ));
        let one = messages(fix);
        assert_eq!(one[one.len() - 2].text, "Changed 1 file, +2 -0");
    }

    #[test]
    fn says_which_changes_were_the_thread_s_own_in_a_folder_other_threads_worked_in_or() {
        let a = json!({ "path": "a.ts", "kind": "added", "additions": 2, "deletions": 0 });
        let b = json!({ "path": "b.ts", "kind": "added", "additions": 5, "deletions": 1 });
        let line = |changes: Value| -> String {
            let mut fields = json!({ "turnId": "u1", "at": AT + 6_000, "from": "a".repeat(40), "to": "b".repeat(40), "moved": [] });
            for (k, v) in changes.as_object().unwrap() {
                fields[k] = v.clone();
            }
            let mut all = turn("go", "went");
            all.push(ev("session.changes", fields));
            let rows = messages(all);
            rows[rows.len() - 2].text.clone()
        };
        assert_eq!(
            line(json!({ "files": [a], "others": [b], "shared": true })),
            "Changed 1 file, +2 -0; 1 file also changed in this folder"
        );
        assert_eq!(
            line(json!({ "files": [], "others": [a, b], "shared": true })),
            "No files from this thread's edits; 2 files also changed in this folder"
        );
        assert_eq!(line(json!({ "files": [a], "others": [], "shared": true })), "Changed 1 file, +2 -0");
        assert_eq!(
            line(json!({ "files": [a, b], "shared": true })),
            "2 files changed in this folder, +7 -1; other threads worked in it too"
        );
    }
}

mod a_thread_s_latest_turn_and_its_final_reply {
    use super::*;

    #[test]
    fn is_the_done_s_result_and_the_reply_is_the_whole_final_message_the_thread_s_finis() {
        let all = events(turn_with(
            "build it",
            "green\nbuilt it",
            json!({ "status": "completed", "text": "green\nbuilt it", "durationMs": 724_000, "costUsd": 0.41 }),
        ));
        let got = thread_result(&all, "t1").unwrap();
        assert_eq!(got, result(json!({ "status": "completed", "text": "green\nbuilt it", "durationMs": 724_000, "costUsd": 0.41 })));
        // The reply's clock is the row the result came off, the done, not the end that may follow it by minutes.
        assert_eq!(thread_reply_rows(&all, "t1"), vec![msg("agent", Some(AT + 4_000), "green\nbuilt it")]);
        // The one rule: the wait's line for the same turn ends on the same message, whole.
        let line = notify_line("t1", &got, Some(NotifyLength::Whole));
        assert!(line.contains("green\nbuilt it"), "{line}");
    }

    #[test]
    fn is_failed_with_the_runtime_s_reason_where_the_runtime_ended_the_turn_and_the_rep() {
        let all = events(vec![
            ev("session.start", json!({ "turnId": "u1", "at": AT, "prompt": "loop forever" })),
            ev(
                "session.end",
                json!({ "turnId": "u1", "at": AT + 1, "exitCode": null, "sawResult": false, "reason": "machine paused while the agent was working" }),
            ),
        ]);
        assert_eq!(
            thread_result(&all, "t1"),
            Some(result(json!({ "status": "failed", "error": "machine paused while the agent was working" })))
        );
        assert_eq!(thread_reply_rows(&all, "t1"), vec![msg("turn", Some(AT + 1), "machine paused while the agent was working")]);
    }

    #[test]
    fn is_nothing_when_the_transcript_holds_no_turn_of_the_thread_which_a_caller_answer() {
        let all = events(turn("build it", "done"));
        assert_eq!(thread_result(&all, "t2"), None);
        assert_eq!(thread_reply_rows(&all, "t2"), vec![]);
    }

    #[test]
    fn never_hands_a_footer_or_an_error_back_under_the_agent_s_name_a_completed_turn_th() {
        // The Claude CLI answers a completed turn with an empty message where it spent its tokens and said nothing.
        let quiet = events(turn_with("build it", "", json!({ "status": "completed", "text": "", "durationMs": 724_000, "costUsd": 0.41 })));
        assert_eq!(thread_reply_rows(&quiet, "t1"), vec![msg("turn", Some(AT + 4_000), "completed  Worked for 12m 4s  $0.41")]);
        // The words of a turn that did not complete are the agent's where the harness left any and the runtime's error
        // where it did not, and each row says which.
        let cut = events(turn_with("build it", "", json!({ "status": "interrupted", "text": "got half of it" })));
        assert_eq!(thread_reply_rows(&cut, "t1"), vec![msg("agent", Some(AT + 4_000), "got half of it")]);
        let failed =
            events(turn_with("build it", "", json!({ "status": "failed", "text": "got half of it", "error": "the harness died" })));
        assert_eq!(thread_reply_rows(&failed, "t1"), vec![msg("turn", Some(AT + 4_000), "the harness died")]);
    }

    #[test]
    fn says_under_the_reply_when_the_thread_has_started_another_turn_since_so_a_stale_r() {
        let mut raw = turn("first", "the old report");
        raw.push(ev("session.start", json!({ "turnId": "u2", "at": AT + 6_000, "prompt": "again" })));
        raw.push(ev("session.delta", json!({ "turnId": "u2", "at": AT + 7_000, "kind": "text", "text": "on it" })));
        let all = events(raw);
        assert_eq!(
            thread_reply_rows(&all, "t1"),
            vec![msg("agent", Some(AT + 4_000), "the old report"), msg("turn", None, NEWER_TURN_LINE)]
        );
        // The turn that gave the reply is the newest ended one either way; the note is the only thing the newer turn adds.
        assert_eq!(thread_result(&all, "t1").unwrap().text, Some("the old report".to_string()));
        // A thread whose turn replied and whose agent process has not exited yet has started nothing newer, so no note.
        assert_eq!(thread_reply_rows(&all[..5], "t1"), vec![msg("agent", Some(AT + 4_000), "the old report")]);
    }

    #[test]
    fn takes_the_newest_turn_not_an_older_turn_s_done() {
        let mut raw = turn("first", "first done");
        raw.push(ev("session.start", json!({ "turnId": "u2", "at": AT + 6_000, "prompt": "again" })));
        raw.push(ev(
            "session.done",
            json!({ "turnId": "u2", "at": AT + 7_000, "result": { "status": "completed", "text": "second done" } }),
        ));
        raw.push(ev("session.end", json!({ "turnId": "u2", "at": AT + 8_000, "exitCode": 0, "sawResult": true })));
        assert_eq!(thread_result(&events(raw), "t1").unwrap().text, Some("second done".to_string()));
    }
}

mod the_printout_a_reader_sees {
    use super::*;

    #[test]
    fn is_who_and_the_clock_on_one_line_and_the_text_under_it_a_blank_line_between_rows() {
        let rows = messages(turn("build it", "read the ticket, then built it"));
        let clock = |n: i64| fmt_clock(Some((AT + n) as f64), &local);
        let got = thread_read_text(&rows, &local);
        assert_eq!(
            got.split('\n').map(str::to_string).collect::<Vec<_>>(),
            vec![
                format!("person {}", clock(0)),
                "build it".to_string(),
                "".to_string(),
                format!("agent {}", clock(1_000)),
                "read".to_string(),
                "".to_string(),
                format!("tool {}", clock(2_000)),
                "$ pnpm test".to_string(),
                "".to_string(),
                format!("agent {}", clock(3_000)),
                " the ticket, then built it".to_string(),
                "".to_string(),
                format!("turn {}", clock(4_000)),
                "completed".to_string(),
            ]
        );
    }

    #[test]
    fn copies_as_markdown_who_spoke_as_a_bold_header_the_reply_as_written_tool_rows_as() {
        let mut raw = turn_with(
            "build it",
            "read the ticket, then built it",
            json!({ "status": "completed", "text": "read the ticket, then built it", "durationMs": 10_000 }),
        );
        raw.push(ev(
            "session.changes",
            json!({
                "turnId": "u1", "at": AT + 6_000, "from": "a".repeat(40), "to": "b".repeat(40), "moved": [],
                "files": [
                    { "path": "a.ts", "kind": "modified", "additions": 14, "deletions": 3 },
                    { "path": "b.ts", "kind": "added", "additions": 0, "deletions": 0 },
                ],
            }),
        ));
        raw.push(ev("session.start", json!({ "turnId": "u2", "at": AT + 7_000, "prompt": "and the math" })));
        raw.push(ev("session.plan", json!({ "turnId": "u2", "at": AT + 7_500, "steps": [{ "text": "derive", "state": "done" }, { "text": "check", "state": "working" }] })));
        raw.push(ev(
            "session.delta",
            json!({ "turnId": "u2", "at": AT + 8_000, "kind": "text", "text": "Here:\n\n$$E=mc^2$$\n\n```ts\nconst x = 1;\n```" }),
        ));
        let rows = thread_messages(&events(raw), "t1");
        let got = thread_markdown(&rows);
        let clock = |n: i64| fmt_clock(Some((AT + n) as f64), &local);
        let end = turn_end_line(&result(json!({ "status": "completed", "text": "read the ticket, then built it", "durationMs": 10_000 })));
        let lines: Vec<String> = vec![
            format!("**You** {}", clock(0)),
            "".into(),
            "build it".into(),
            "".into(),
            "**Agent**".into(),
            "".into(),
            "read".into(),
            "".into(),
            "- $ pnpm test".into(),
            "".into(),
            " the ticket, then built it".into(),
            "".into(),
            "- Changed 2 files, +14 -3".into(),
            "".into(),
            format!("*{end}*"),
            "".into(),
            format!("**You** {}", clock(7_000)),
            "".into(),
            "and the math".into(),
            "".into(),
            "**Agent**".into(),
            "".into(),
            "- Plan: 1 of 2 steps done".into(),
            "  - [x] derive".into(),
            "  - [ ] check (working)".into(),
            "".into(),
            "Here:".into(),
            "".into(),
            "$$E=mc^2$$".into(),
            "".into(),
            "```ts".into(),
            "const x = 1;".into(),
            "```".into(),
        ];
        assert_eq!(got.split('\n').map(str::to_string).collect::<Vec<_>>(), lines);
    }
}

mod the_prompt_a_thread_is_stopped_on {
    use super::*;

    fn ask(ask_id: &str, command: &str) -> Value {
        ev(
            "session.permission",
            json!({
                "turnId": "u1", "at": AT, "askId": ask_id, "toolName": "Bash", "input": json!({ "command": command }).to_string(),
                "options": [{ "id": "allow", "label": "Allow", "effect": "allow" }],
            }),
        )
    }

    fn closed(ask_id: &str) -> Value {
        ev("session.permission.closed", json!({ "turnId": "u1", "at": AT + 1, "askId": ask_id, "outcome": "allowed" }))
    }

    fn ask_id(e: Option<SessionPermissionEvent>) -> Option<String> {
        e.map(|e| e.ask_id)
    }

    #[test]
    fn is_the_oldest_ask_with_no_close_behind_it_the_one_the_harness_stopped_at_and_not() {
        assert_eq!(ask_id(open_ask(&[], "t1")), None);
        assert_eq!(ask_id(open_ask(&events(vec![ask("a1", "ls")]), "t1")), Some("a1".to_string()));
        assert_eq!(ask_id(open_ask(&events(vec![ask("a1", "ls"), closed("a1")]), "t1")), None);
        assert_eq!(ask_id(open_ask(&events(vec![ask("a1", "ls"), closed("a1"), ask("a2", "rm -rf x")]), "t1")), Some("a2".to_string()));
        // Two open at once: the older leads, which is what the runtime leads the thread's row with.
        assert_eq!(ask_id(open_ask(&events(vec![ask("a1", "ls"), ask("a2", "rm -rf x")]), "t1")), Some("a1".to_string()));
        assert_eq!(lead_ask(Vec::<String>::new()), None);
    }

    #[test]
    fn is_read_per_thread_an_ask_open_on_another_thread_is_not_this_one_s() {
        let mut elsewhere = ask("a1", "ls");
        elsewhere["threadId"] = json!("t2");
        let all = events(vec![elsewhere]);
        assert_eq!(ask_id(open_ask(&all, "t1")), None);
        assert_eq!(ask_id(open_ask(&all, "t2")), Some("a1".to_string()));
    }
}

mod the_thread_a_new_session_is_handed {
    use super::*;

    #[test]
    fn is_the_thread_s_markdown_its_oldest_rows_left_out_past_the_cap_and_counted_first() {
        let rows = vec![msg("person", None, "fix the login page"), msg("agent", None, "done, pushed to fix/login")];
        assert_eq!(thread_seed(&rows), thread_markdown(&rows));
        let long = msg("agent", None, &"x".repeat(THREAD_SEED_CHARS as usize - 10));
        let mut all = rows.clone();
        all.push(long.clone());
        let seed = thread_seed(&all);
        assert!(seed.starts_with("*2 earlier messages left out*\n\n"), "{seed}");
        assert!(!seed.contains("fix the login page"), "{seed}");
        assert!(seed.contains(&long.text), "the long row is kept");
    }
}
