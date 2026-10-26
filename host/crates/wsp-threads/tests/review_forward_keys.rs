// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), path 1: the key order of the rows a turn records. Every `ctx.record({...})` in
//! turns.ts:317-565 is a literal whose keys go in the order written, and `record` (transcripts.ts:680) only spreads it
//! and adds `at` and `pos` after. The port builds the wire structs, whose keys go in the struct's own order, so a row
//! reaches the transcript with its keys moved (`line` after `cwd`, `exitCode` before `durationMs`, and so on).
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

fn keys_of(host: &FakeHost, kind: &str) -> Vec<String> {
    let rows = host.recorded_of(kind);
    rows.first().and_then(|r| r.as_object()).map(|o| o.keys().cloned().collect()).unwrap_or_default()
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_delta_row_keys_go_in_the_order_turns_ts_writes_them() {
    run_local(async {
        let r = rig();
        r.send(json!({ "type": "turn.delta", "sessionId": SESS, "kind": "tool_result", "text": "ok", "messageId": "m1", "toolName": "Bash",
            "toolUseId": "toolu_1", "isError": false, "parentToolUseId": "toolu_0", "cwd": "/w", "exitCode": 0, "durationMs": 12, "bytes": 4,
            "patch": [] }));
        assert_eq!(
            keys_of(&r.host, "session.delta"),
            words(&["type", "workspaceId", "sessionId", "turnId", "threadId", "line", "kind", "text", "messageId", "toolName", "toolUseId",
                "isError", "parentToolUseId", "cwd", "exitCode", "durationMs", "bytes", "patch"])
        );
    });
}

#[test]
fn a_start_row_keys_go_in_the_order_turns_ts_writes_them() {
    run_local(async {
        let r = rig_with(FakeSession::new(SESS), |t| {
            t.opening.request_id = Some("req-1".into());
            t.opening.after_cut = true;
            t.opening.after_limit = Some(5.0);
            t.opening.opens_thread = true;
            t.opening.attachments = Some(vec![]);
        });
        r.send(json!({ "type": "session.start", "sessionId": SESS, "model": "opus", "cwd": "/w", "tools": ["Bash"], "harness": { "name": "claude" } }));
        let keys = keys_of(&r.host, "session.start");
        let expected = words(&["type", "workspaceId", "sessionId", "turnId", "threadId", "prompt", "requestId", "afterCut", "afterLimit",
            "opensThread", "startedBy", "attachments", "model", "cwd", "agent", "tools", "harness"]);
        let present: Vec<String> = expected.iter().filter(|k| keys.contains(k)).cloned().collect();
        assert_eq!(keys.iter().filter(|k| present.contains(k)).cloned().collect::<Vec<_>>(), present);
    });
}

#[test]
fn the_counted_rows_put_their_line_first() {
    run_local(async {
        let r = rig();
        r.send(json!({ "type": "subagent", "sessionId": SESS, "task": "a1", "state": "running", "parentToolUseId": "toolu_a1", "title": "T",
            "summary": "S", "depth": 1, "model": "haiku", "asked": "do it" }));
        r.send(json!({ "type": "turn.compacted", "sessionId": SESS, "before": 10, "after": 5 }));
        r.send(json!({ "type": "turn.retry", "sessionId": SESS, "attempt": 1, "of": 3, "why": "overloaded" }));
        assert_eq!(
            keys_of(&r.host, "session.subagent"),
            words(&["type", "workspaceId", "sessionId", "turnId", "threadId", "line", "task", "state", "parentToolUseId", "title", "summary",
                "depth", "model", "asked"])
        );
        assert_eq!(
            keys_of(&r.host, "session.compacted"),
            words(&["type", "workspaceId", "sessionId", "turnId", "threadId", "line", "before", "after"])
        );
        assert_eq!(
            keys_of(&r.host, "session.retry"),
            words(&["type", "workspaceId", "sessionId", "turnId", "threadId", "line", "attempt", "of", "why"])
        );
    });
}
