// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the roads that touch a person's accounts: pull requests, git host posts, pushes. Every expected value is
//! the TS's, read off the line named or printed by a node probe (named in each comment). Over the same fake context as
//! tests/workspaces_git.rs.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/git.rs"]
mod git;
include!("workspaces_git/world.rs");

const HOSTILE: [&str; 6] = ["'; touch /tmp/x; '", "$(id)", "-x --force", "line one\nline two", "nul\u{0}byte", "`id`; rm -rf ~"];

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-1: workspaces.ts:906 `body.trim() === ""` is JavaScript's trim. git.rs:922 uses Rust's.
// Node printed: "﻿".trim() === "" -> true; "\^E".trim() === "" -> false.

#[test]
fn a_reply_of_only_a_byte_order_mark_is_blank_to_the_ts_and_posts_nothing() {
    run(async {
        let w = world(json!({}));
        script(&w, "post_as_person", json!({ "comment": comment(9, "Zingzy", "owner", "\u{FEFF}") }));
        let ask = PullRequestReplyAsk { workspace_id: "ws1".into(), reply_to: None, thread_id: None, body: "\u{FEFF}".into() };
        let got = area::pull_request_reply(&w.ctx, ask, origin()).await;
        assert!(
            w.base.args_of("post_as_person").is_empty(),
            "the TS refuses a body that JavaScript trims to nothing; the Rust posted it as the person: {:?}",
            w.base.args_of("post_as_person")
        );
        refusal(got, "a reply needs words", Some(Kind::Invalid));
    })
}

#[test]
fn a_reply_of_only_a_next_line_character_is_not_blank_to_the_ts_and_posts() {
    run(async {
        let w = world(json!({}));
        script(&w, "post_as_person", json!({ "comment": comment(9, "Zingzy", "owner", "\u{85}") }));
        let ask = PullRequestReplyAsk { workspace_id: "ws1".into(), reply_to: None, thread_id: None, body: "\u{85}".into() };
        let got = area::pull_request_reply(&w.ctx, ask, origin()).await;
        assert!(got.is_ok(), "the TS does not trim U+0085, so the body is not blank and posts; the Rust refused it: {:?}", got.err().map(|e| e.message));
        assert_eq!(w.base.names(), vec!["pull_request_on", "post_as_person"]);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-2 (NIT): workspaces.ts:909 builds `{ op, cwd, remote, number, ...where, body }`: replyTo and threadId before body.
// git.rs:926-932 appends them after it. The frame the daemon is sent differs in key order only.

#[test]
fn the_reply_frame_keeps_the_ts_key_order() {
    run(async {
        let w = world(json!({}));
        script(&w, "post_as_person", json!({ "comment": comment(9, "Zingzy", "owner", "Fixed.") }));
        let ask = PullRequestReplyAsk { workspace_id: "ws1".into(), reply_to: Some(7), thread_id: Some("PRRT_kwDOx".into()), body: "Fixed.".into() };
        area::pull_request_reply(&w.ctx, ask, origin()).await.expect("the reply posts");
        let sent = w.base.args_of("post_as_person")[0]["ask"].clone();
        let want = format!(
            "{{\"op\":\"git.prReply\",\"cwd\":\"{HOME}\",\"remote\":\"{REMOTE}\",\"number\":12,\"replyTo\":7,\"threadId\":\"PRRT_kwDOx\",\"body\":\"Fixed.\"}}"
        );
        assert_eq!(serde_json::to_string(&sent).unwrap(), want);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-3: workspaces.ts:902 and :973 hand `readHost` a frame BUILDER, `cwd => frame`. pull-requests.ts:27 builds it for
// this computer's home on the first road, and pull-requests.ts:31 for the copy's checkout on the second. git.rs:898-906
// and :1143-1149 hand `GitCtx::read_host` one finished frame whose cwd is the copy's checkout, so the first road, this
// computer's daemon, is told to run the person's command line in a folder of the copy (for a machine, a path that does
// not exist here). The suite strips `cwd` before it compares (without_cwd), so it cannot see this.
// TS: pull-requests.ts:20-32 and the comment above it: "the person's own signed-in command line, in their home".

#[test]
fn the_read_this_computer_answers_first_runs_in_this_computers_home() {
    run(async {
        let w = world(json!({}));
        script(&w, "read_host", json!({ "diff": "d", "truncated": false, "left": [] }));
        area::pull_request_diff(&w.ctx, "ws1", origin()).await.expect("the diff reads");
        let asked = w.base.args_of("read_host");
        assert_eq!(
            asked[0]["ask"]["cwd"],
            json!(HOME),
            "the frame the context sends this computer's daemon first must name this computer's home, as pull-requests.ts:27 does"
        );
    })
}

#[test]
fn the_check_log_read_this_computer_answers_first_runs_in_this_computers_home() {
    run(async {
        let w = world(json!({}));
        script(&w, "read_pull_request", failing_fact());
        script(&w, "read_host", json!({ "lines": ["boom"], "truncated": false }));
        script(&w, "to_first_thread", json!({ "outcome": "started", "threadId": "t1", "harness": "claude" }));
        let _ = area::fix(&w.ctx, FixAsk { workspace_id: "ws1".into(), check: Some("ci".into()), ..Default::default() }, origin()).await;
        let asked = w.base.args_of("read_host");
        assert!(!asked.is_empty(), "the failed check's log is read through the host: {:?}", w.base.names());
        assert_eq!(asked[0]["ask"]["cwd"], json!(HOME), "the log read goes to this computer's daemon first (workspaces.ts:973)");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Hostile values ride the frames unchanged. Where the TS sends JSON (a daemon frame) the Rust must send the same
// JSON; nothing here reaches a shell on the host. Expected: the TS object literals at workspaces.ts:714, :730, :833,
// :817, :1042, :1132. These pass.

#[test]
fn hostile_values_ride_the_bring_back_and_commit_and_discard_frames_unchanged() {
    run(async {
        for bad in HOSTILE {
            let w = world(json!({}));
            script(&w, "ask:git.push", push_reply());
            script(&w, "ask:git.pr", json!({ "pr": pr_json(), "created": true }));
            area::bring_back(&w.ctx, "ws1", Some(bad.into()), Some(bad.into()), origin()).await.expect("bring back answers");
            let want = vec![
                json!({ "op": "git.push", "cwd": CWD, "base": "main" }),
                json!({ "op": "git.pr", "cwd": CWD, "base": "main", "title": bad, "body": bad }),
            ];
            assert_eq!(serde_json::to_string(&w.base.asks()).unwrap(), serde_json::to_string(&want).unwrap(), "bring back {bad:?}");

            let w = world(json!({}));
            script(&w, "ask:git.commit", json!({ "oid": "abc", "branch": "main", "subject": "s", "files": 1 }));
            let ask = CommitAsk { workspace_id: "ws1".into(), message: bad.into(), paths: Some(vec![bad.into(), "-p".into()]), ..Default::default() };
            let _ = area::commit(&w.ctx, ask, origin()).await;
            let want = vec![json!({ "op": "git.commit", "cwd": CWD, "message": bad, "paths": [bad, "-p"] })];
            assert_eq!(serde_json::to_string(&w.base.asks()).unwrap(), serde_json::to_string(&want).unwrap(), "commit {bad:?}");

            let w = world(json!({}));
            script(&w, "ask:git.discard", json!({ "path": bad }));
            let ask = DiscardAsk { workspace_id: "ws1".into(), path: bad.into(), ..Default::default() };
            let _ = area::discard(&w.ctx, ask, origin()).await;
            let want = vec![json!({ "op": "git.discard", "cwd": CWD, "path": bad })];
            assert_eq!(serde_json::to_string(&w.base.asks()).unwrap(), serde_json::to_string(&want).unwrap(), "discard {bad:?}");
        }
    })
}

#[test]
fn a_post_as_the_person_names_this_computers_home_never_the_copys_checkout() {
    run(async {
        for bad in HOSTILE {
            let w = world(json!({}));
            script(&w, "post_as_person", json!({ "comment": comment(9, "Zingzy", "owner", bad) }));
            let ask = PullRequestReplyAsk { workspace_id: "ws1".into(), reply_to: None, thread_id: None, body: bad.into() };
            let _ = area::pull_request_reply(&w.ctx, ask, origin()).await;
            let sent = w.base.args_of("post_as_person");
            assert_eq!(sent[0]["ask"]["cwd"], json!(HOME), "reply {bad:?}");
            assert_eq!(sent[0]["ask"]["body"], json!(bad), "reply {bad:?}");
            assert!(w.base.asks().is_empty(), "nothing posts through the copy's daemon");

            let w = world(json!({}));
            script(&w, "post_as_person", json!({ "resolved": true }));
            let _ = area::pull_request_resolve(&w.ctx, "ws1", bad, true, origin()).await;
            assert_eq!(w.base.args_of("post_as_person")[0]["ask"]["cwd"], json!(HOME), "resolve {bad:?}");
            assert_eq!(w.base.args_of("post_as_person")[0]["ask"]["threadId"], json!(bad));
        }
    })
}

// A head a caller names rides the merge frame unchanged, on this computer, in this computer's home: workspaces.ts:1001.
// This passes.
#[test]
fn a_hostile_head_rides_the_merge_frame_unchanged_and_runs_on_this_computer() {
    run(async {
        for bad in HOSTILE {
            let w = world(json!({}));
            hold_fact(&w, json!({}));
            script(&w, "merge_settings", json!({ "methods": ["merge", "squash"], "defaultMethod": "squash", "autoMerge": false }));
            script(&w, "ask:git.prMerge", json!({ "merged": true, "autoArmed": false }));
            let ask = MergeAsk { workspace_id: "ws1".into(), head: Some(bad.into()), ..Default::default() };
            area::merge(&w.ctx, ask, origin()).await.expect("the merge runs");
            let want = vec![json!({ "op": "git.prMerge", "cwd": HOME, "remote": REMOTE, "number": 12, "method": "squash", "auto": false, "headOid": bad })];
            assert_eq!(serde_json::to_string(&w.base.asks()).unwrap(), serde_json::to_string(&want).unwrap(), "merge {bad:?}");
            assert!(w.base.names().iter().any(|n| n == "on_this_computer"), "the merge ran on this computer");
        }
    })
}
