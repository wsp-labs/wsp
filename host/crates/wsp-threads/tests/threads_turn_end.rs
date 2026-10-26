// SPDX-License-Identifier: AGPL-3.0-only
//! threads_turn_end (spec/threads_turn_end.md): checkpoints, taken turns, steers, and what a turn changed. Expected
//! values read off threads.ts:956-1142 at 65f6b89c7 and turn-changes.test.ts, changes.test.ts, steer-unread.test.ts,
//! runtime-steer.test.ts.
#[path = "support_b/mod.rs"]
mod support;

use std::collections::BTreeSet;
use std::rc::Rc;

use futures::FutureExt;
use serde_json::{json, Value};
use support::Lend;
use support::threads_ctx::{ask_failure, entry, held_project, FakeThreadsCtx};
use support::{event, local, row, settle, view};
use wsp_core::types::internal::{Steered, Taken, TakenOutcome, TurnLive};
use wsp_threads::threads::threads::turn_end::{CheckpointAsk, SteerSent, TopRead, TurnChangesAsk};
use wsp_threads::threads::threads::{Entry, ThreadsArea};
use wsp_wire::types::attachments::Attachment;

fn host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>, Entry) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "kind": "local", "name": "the-folder" }));
    ctx.add_entry(e.clone());
    let area = ctx.area();
    (ctx, area, e)
}

fn top(answer: Option<&str>, reads: Rc<std::cell::Cell<u32>>) -> TopRead {
    let answer = answer.map(String::from);
    Rc::new(move || {
        reads.set(reads.get() + 1);
        let a = answer.clone();
        async move { a }.boxed_local()
    })
}

fn steering_row(ctx: &FakeThreadsCtx, asking: bool) {
    ctx.sessions.with(|s| {
        let mut v = json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "cs-1", "status": "running" });
        if asking {
            v["asking"] = json!("may I run it?");
        }
        let r = row(view(v), "u1");
        r.borrow_mut().turn_live = Some(TurnLive::default());
        s.insert("r1".into(), r);
    });
}

fn steer(prompt: &str, images: usize) -> SteerSent {
    SteerSent {
        prompt: prompt.into(),
        request_id: Some("req-s".into()),
        via_slate: false,
        started_by: None,
        attachments: (0..images).map(|i| serde_json::from_value::<Attachment>(json!({ "mediaType": "image/png", "bytes": "AQID", "name": format!("{i}.png") })).expect("an attachment")).collect(),
    }
}

fn steered_on(ctx: &FakeThreadsCtx) -> Vec<String> {
    ctx.sessions.with(|s| {
        let steered: Vec<String> = s.get("r1").map(|r| r.borrow().turn_live.as_ref().and_then(|l| l.steered.as_ref()).map(|m| m.keys().cloned().collect()).unwrap_or_default()).unwrap_or_default();
        steered
    })
}

// ---- E1, E2 -------------------------------------------------------------------------------------------------------

#[test]
fn keep_checkpoint_records_the_ref_based_and_anchor() {
    local(async {
        let (ctx, area, e) = host();
        ctx.daemon_answers("git.checkpoint", Ok(json!({ "ref": "refs/wsp/ws1/t1/u1", "based": true })));
        area.keep_checkpoint(&e, CheckpointAsk { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), anchor: Some("msg-9".into()), kept: None }).await;
        assert_eq!(ctx.frames.borrow()[0], json!({ "op": "git.checkpoint", "cwd": "/work/ws1", "thread": "t1", "turn": "u1", "scope": "ws1" }));
        let keys: Vec<String> = ctx.frames.borrow()[0].as_object().expect("a frame").keys().cloned().collect();
        assert_eq!(keys, vec!["op", "cwd", "thread", "turn", "scope"]);
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        for (k, v) in [("type", json!("session.checkpoint")), ("workspaceId", json!("ws1")), ("sessionId", json!("cs-1")), ("turnId", json!("u1")), ("threadId", json!("t1")), ("ref", json!("refs/wsp/ws1/t1/u1")), ("based", json!(true)), ("anchor", json!("msg-9"))] {
            assert_eq!(said[0][k], v, "{k}");
        }
        assert_eq!(said[0].get("kept"), None);
    });
}

#[test]
fn keep_checkpoint_of_a_folder_with_no_repo_asks_nothing() {
    local(async {
        let (ctx, area, e) = host();
        ctx.projects.borrow_mut().insert("p1".into(), held_project("p1", None));
        area.keep_checkpoint(&e, CheckpointAsk { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), anchor: None, kept: Some("k1".into()) }).await;
        assert!(ctx.frames.borrow().is_empty(), "a folder git holds no repo in keeps no checkpoint");
        let said = ctx.recorded_json();
        assert_eq!((said[0]["kept"].clone(), said[0].get("ref").cloned(), said[0].get("based").cloned()), (json!("k1"), None, None));
        assert!(area.not_a_repo(&e.borrow().record.borrow()));
    });
}

#[test]
fn keep_checkpoint_warns_and_records_the_anchor_alone_when_the_daemon_fails() {
    local(async {
        let (ctx, area, e) = host();
        ctx.daemon_answers("git.checkpoint", Err(ask_failure("daemon too old", None)));
        let warned = wsp_core::console::capture();
        area.keep_checkpoint(&e, CheckpointAsk { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), anchor: Some("msg-9".into()), kept: None }).await;
        assert_eq!(warned.lines(), vec![wsp_core::types::internal::no_checkpoint_log_line("t1", "ws1", "daemon too old")]);
        let said = ctx.recorded_json();
        assert_eq!((said[0]["anchor"].clone(), said[0].get("ref").cloned()), (json!("msg-9"), None));
    });
}

#[test]
fn keep_checkpoint_with_nothing_to_keep_records_nothing() {
    local(async {
        let (ctx, area, e) = host();
        ctx.daemon_answers("git.checkpoint", Ok(json!({ "based": true })));
        area.keep_checkpoint(&e, CheckpointAsk { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), anchor: None, kept: None }).await;
        assert!(ctx.recorded.borrow().is_empty(), "no ref, no anchor, nothing kept");
    });
}

// ---- E3 -----------------------------------------------------------------------------------------------------------

#[test]
fn taken_turn_of_an_ended_turn_reads_its_result_off_the_transcript() {
    local(async {
        let (ctx, area, _) = host();
        ctx.sessions.with(|s| {
            s.insert("cs-1".into(), row(view(json!({ "id": "cs-1", "threadId": "t1", "status": "completed" })), "u2"));
        });
        ctx.thread_events.borrow_mut().insert(
            ("ws1".into(), "t1".into()),
            vec![
                event(json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "cs-1", "turnId": "u1", "threadId": "t1", "exitCode": null, "sawResult": false, "reason": "first reason" })),
                event(json!({ "type": "session.done", "workspaceId": "ws1", "sessionId": "cs-1", "turnId": "u1", "threadId": "t1", "result": { "status": "completed", "text": "done" } })),
            ],
        );
        let taken = Taken { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), outcome: TakenOutcome::Started };
        let handle = area.taken_turn("ws1", &taken).await.expect("the row is held");
        assert_eq!((handle.id(), handle.turn_id(), handle.workspace_id()), ("cs-1".to_string(), "u1".to_string(), "ws1".to_string()));
        assert_eq!(support::json_of(&handle.finished().now_or_never().expect("settled")), json!({ "status": "completed", "text": "done" }));
        ctx.sessions.with(|s| s.get("cs-1").expect("row").borrow_mut().view.prompt = Some("read at each call".into()));
        assert_eq!(handle.view().prompt.as_deref(), Some("read at each call"));
        ctx.thread_events.borrow_mut().insert(
            ("ws1".into(), "t1".into()),
            vec![event(json!({ "type": "session.end", "workspaceId": "ws1", "sessionId": "cs-1", "turnId": "u1", "threadId": "t1", "exitCode": null, "sawResult": false, "reason": "the end said why" }))],
        );
        let ended = area.taken_turn("ws1", &taken).await.expect("held");
        assert_eq!(support::json_of(&ended.finished().now_or_never().expect("settled")), json!({ "status": "failed", "error": "the end said why" }));
        ctx.thread_events.borrow_mut().clear();
        let silent = area.taken_turn("ws1", &taken).await.expect("held");
        assert_eq!(support::json_of(&silent.finished().now_or_never().expect("settled")), json!({ "status": "failed", "error": "host restarted while the agent was working" }));
    });
}

#[test]
fn taken_turn_hands_back_the_running_handle_with_the_taken_outcome() {
    local(async {
        let (ctx, area, _) = host();
        let (inner, _) = {
            let ctx2 = FakeThreadsCtx::new();
            let a = ctx2.area();
            ctx2.sessions.with(|s| {
                s.insert("cs-1".into(), row(view(json!({ "id": "cs-1", "threadId": "t1", "status": "completed" })), "u1"));
            });
            ctx2.thread_events.borrow_mut().insert(("ws1".into(), "t1".into()), vec![]);
            let t = Taken { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), outcome: TakenOutcome::Started };
            (a.taken_turn("ws1", &t).await.expect("a handle to hold"), a)
        };
        ctx.sessions.with(|s| {
            let r = row(view(json!({ "id": "cs-1", "threadId": "t1", "status": "running" })), "u1");
            r.borrow_mut().handle = Some(inner);
            s.insert("cs-1".into(), r);
        });
        let taken = Taken { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), outcome: TakenOutcome::Steered };
        let handle = area.taken_turn("ws1", &taken).await.expect("held");
        assert_eq!(support::json_of(&handle.outcome()), json!("steered"));
        assert_eq!(handle.id(), "cs-1");
    });
}

#[test]
fn taken_turn_of_a_row_gone_is_none() {
    local(async {
        let (_ctx, area, _) = host();
        let taken = Taken { session_id: "cs-gone".into(), thread_id: "t1".into(), turn_id: "u1".into(), outcome: TakenOutcome::Started };
        assert!(area.taken_turn("ws1", &taken).await.is_none());
    });
}

// ---- E4 to E8 -----------------------------------------------------------------------------------------------------

fn caller() -> wsp_wire::views::workspace::Caller {
    let by = serde_json::from_value(json!({ "kind": "thread", "threadId": "lead", "workspaceId": "ws1", "rootThreadId": "lead" })).expect("a scope");
    wsp_wire::views::workspace::Caller::Scoped { origin: serde_json::from_value(json!("relayed")).expect("an origin"), by }
}

#[test]
fn keep_steer_keeps_the_message_on_the_turn_and_persists() {
    local(async {
        let (ctx, area, _) = host();
        steering_row(&ctx, false);
        area.keep_steer("r1", &steer("also this", 1), Some(&caller()), "st1").await;
        assert!(area.steers_out.borrow().contains_key("st1"), "the start road owns it until the write answers");
        let kept = ctx.sessions.with(|s| s.get("r1").and_then(|r| r.borrow().turn_live.clone()).and_then(|l| l.steered).and_then(|m| m.get("st1").cloned())).expect("kept on the turn");
        assert_eq!(
            support::json_of(&kept),
            json!({ "prompt": "also this", "by": { "kind": "thread", "threadId": "lead", "workspaceId": "ws1", "rootThreadId": "lead" }, "road": "relayed", "startedBy": "person", "requestId": "req-s", "attachments": [{ "mediaType": "image/png", "bytes": 3, "name": "0.png" }] })
        );
        assert_eq!(ctx.called(), vec!["persist ws1"]);
        ctx.sessions.with(|s| s.get("r1").expect("row").borrow_mut().turn_live = None);
        area.keep_on_turn("r1", &steer("x", 0), None, "st2").await;
        assert_eq!(ctx.called(), vec!["persist ws1"], "a turn with no live state keeps nothing and persists nothing");
    });
}

#[test]
fn drop_steer_removes_only_that_message() {
    local(async {
        let (ctx, area, _) = host();
        steering_row(&ctx, false);
        area.keep_on_turn("r1", &steer("one", 0), None, "st1").await;
        area.keep_on_turn("r1", &steer("two", 0), None, "st2").await;
        area.keep_on_turn("r1", &steer("three", 0), None, "st3").await;
        area.drop_steer("r1", "st2");
        area.drop_steer("r1", "never-kept");
        settle().await;
        assert_eq!(steered_on(&ctx), vec!["st1", "st3"]);
        assert_eq!(ctx.called().iter().filter(|c| *c == "persist ws1").count(), 4, "three keeps and one drop");
        area.steers_out.borrow_mut().insert("st3".into(), Default::default());
        area.steer_lost("r1", "st3");
        settle().await;
        assert_eq!(steered_on(&ctx), vec!["st1"]);
        assert!(!area.steers_out.borrow().contains_key("st3"));
    });
}

#[test]
fn steer_answered_records_a_landed_steer() {
    local(async {
        let (ctx, area, _) = host();
        steering_row(&ctx, false);
        area.keep_steer("r1", &steer("also this", 0), None, "st1").await;
        assert!(area.steer_answered("r1", "h1", &steer("also this", 0), None, "st1", true));
        settle().await;
        assert!(!area.steers_out.borrow().contains_key("st1"));
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        for (k, v) in [("type", json!("session.steer")), ("workspaceId", json!("ws1")), ("sessionId", json!("cs-1")), ("turnId", json!("u1")), ("threadId", json!("t1")), ("prompt", json!("also this")), ("requestId", json!("req-s"))] {
            assert_eq!(said[0][k], v, "{k}");
        }
        assert_eq!(said[0].get("waiting"), None);
        assert_eq!(steered_on(&ctx), vec!["st1"]);
    });
}

#[test]
fn steer_answered_tells_the_person_when_a_stop_ended_the_turn() {
    local(async {
        let (ctx, area, _) = host();
        steering_row(&ctx, false);
        area.keep_steer("r1", &steer("also this", 0), None, "st1").await;
        let ended_row = row(view(json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "cs-1" })), "u1");
        ended_row.borrow_mut().turn_live = Some(TurnLive {
            steered: Some([("st1".to_string(), serde_json::from_value::<Steered>(json!({ "prompt": "also this", "startedBy": "person" })).expect("a steer"))].into_iter().collect()),
            ..Default::default()
        });
        area.steers_out.borrow_mut().get_mut("st1").expect("out").ended = Some((ended_row, true));
        assert!(area.steer_answered("r1", "h1", &steer("also this", 0), None, "st1", true), "the person is told, so the caller hears it landed");
        settle().await;
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        assert_eq!((said[0]["type"].clone(), said[0]["notify"].clone(), said[0]["text"].clone()), (json!("session.notify"), json!("me"), json!("the agent never read this message: also this")));
    });
}

#[test]
fn steer_answered_drops_a_steer_the_turn_ended_on() {
    local(async {
        let (ctx, area, _) = host();
        steering_row(&ctx, false);
        area.keep_steer("r1", &steer("x", 0), None, "st1").await;
        let ended_row = row(view(json!({ "id": "r1", "threadId": "t1" })), "u1");
        area.steers_out.borrow_mut().get_mut("st1").expect("out").ended = Some((ended_row, false));
        assert!(!area.steer_answered("r1", "h1", &steer("x", 0), None, "st1", true), "it goes as the next turn");
        settle().await;
        assert!(steered_on(&ctx).is_empty());
        assert!(ctx.recorded.borrow().is_empty());
        area.keep_steer("r1", &steer("y", 0), None, "st2").await;
        assert!(!area.steer_answered("r1", "h1", &steer("y", 0), None, "st2", false), "a write that found no turn running");
        settle().await;
        assert!(steered_on(&ctx).is_empty());
    });
}

#[test]
fn record_steer_keeps_images_before_the_row_and_says_waiting() {
    local(async {
        let (ctx, area, _) = host();
        steering_row(&ctx, true);
        area.record_steer("r1", "h1", &steer("look", 2), None, "st9");
        settle().await;
        let calls = ctx.called();
        assert_eq!(calls[0], "persist ws1", "kept on the turn first, since it was not (threads.ts:1040 then :1001)");
        assert_eq!(calls[1], "keepSentImages ws1 t1 req-s 2", "the images are kept before the row is written");
        let said = ctx.recorded_json();
        assert_eq!(said[0]["waiting"], json!(true), "the turn waits on a person");
        assert_eq!(said[0]["attachments"], json!([{ "mediaType": "image/png", "bytes": 3, "name": "0.png" }, { "mediaType": "image/png", "bytes": 3, "name": "1.png" }]));
        assert_eq!(steered_on(&ctx), vec!["st9"]);
    });
}

// ---- E9, E10, E11 -------------------------------------------------------------------------------------------------

#[test]
fn snapshot_of_answers_the_commit_or_nothing_inside_its_deadline() {
    local(async {
        let (ctx, area, e) = host();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "abc123" })));
        assert_eq!(area.snapshot_of(&e, "/work/ws1/sub").await.as_deref(), Some("abc123"));
        assert_eq!(ctx.frames.borrow()[0], json!({ "op": "git.snapshot", "cwd": "/work/ws1/sub" }));
        assert_eq!(ctx.clock.pending(), 0, "the deadline is cancelled");
        ctx.daemon.borrow_mut().clear();
        ctx.daemon_answers("git.snapshot", Ok(json!({})));
        assert_eq!(area.snapshot_of(&e, "/w").await.as_deref(), Some("undefined"), "String(undefined), as the TS writes it");
        ctx.daemon_hangs.borrow_mut().push("git.snapshot".into());
        let warned = wsp_core::console::capture();
        let late = tokio::task::spawn_local({
            let area = area.clone();
            let e = e.clone();
            async move { area.snapshot_of(&e, "/w").await }
        });
        settle().await;
        ctx.clock.advance(3_000);
        assert_eq!(late.await.expect("ran"), None);
        assert_eq!(warned.lines(), vec!["no snapshot of /w on the-folder inside 3000ms; the turn runs without one and its reply lists no changes"]);
        *ctx.reach.borrow_mut() = "unreachable".into();
        assert_eq!(area.snapshot_of(&e, "/w").await, None, "a daemon not reached is not asked");
    });
}

#[test]
fn snapshot_of_is_silent_for_a_folder_git_holds_no_repo_in() {
    local(async {
        let (ctx, area, e) = host();
        ctx.daemon_answers("git.snapshot", Err(ask_failure("not a git repository", Some("not-a-git-repo"))));
        let warned = wsp_core::console::capture();
        assert_eq!(area.snapshot_of(&e, "/w").await, None);
        assert!(warned.lines().is_empty());
        ctx.daemon.borrow_mut().clear();
        ctx.daemon_answers("git.snapshot", Err(ask_failure("disk full", Some("io"))));
        assert_eq!(area.snapshot_of(&e, "/w").await, None);
        assert_eq!(warned.lines(), vec!["no snapshot of /w on the-folder: disk full"]);
    });
}

fn beside(ctx: &FakeThreadsCtx, rows: Vec<Value>) {
    ctx.sessions.with(|s| {
        for (i, v) in rows.into_iter().enumerate() {
            let id = format!("b{i}");
            let mut v = v;
            v["id"] = json!(id);
            s.insert(id.clone(), row(view(v), &format!("ub{i}")));
        }
    });
}

#[test]
fn shared_folder_compares_checkout_tops_not_cwds() {
    local(async {
        let (ctx, area, e) = host();
        beside(&ctx, vec![json!({ "threadId": "other", "cwd": "/repo/b", "startedAt": 50, "endedAt": 150 })]);
        ctx.daemon_answers("git.worktrees", Ok(json!({ "worktrees": [{ "path": "/repo" }, { "path": "/repo-old", "prunable": true }] })));
        let reads = Rc::new(std::cell::Cell::new(0));
        assert!(area.shared_folder(&e, "mine", "/repo/a", top(Some("/repo"), reads.clone()), 100, 200).await, "two subfolders of one checkout share it");
        assert_eq!(reads.get(), 1);
        let reads = Rc::new(std::cell::Cell::new(0));
        assert!(!area.shared_folder(&e, "mine", "/elsewhere", top(Some("/other-top"), reads), 100, 200).await);
    });
}

#[test]
fn shared_folder_ignores_rows_outside_the_turns_time() {
    local(async {
        let (ctx, area, e) = host();
        beside(&ctx, vec![
            json!({ "threadId": "other", "cwd": "/repo/a", "startedAt": 10, "endedAt": 90 }),
            json!({ "threadId": "mine", "cwd": "/repo/a", "startedAt": 100 }),
            json!({ "threadId": "other", "cwd": "/repo/a", "startedAt": 201 }),
            json!({ "threadId": "other", "cwd": "/repo/a", "workspaceId": "ws2", "startedAt": 150 }),
        ]);
        let reads = Rc::new(std::cell::Cell::new(0));
        assert!(!area.shared_folder(&e, "mine", "/repo/a", top(Some("/repo"), reads.clone()), 100, 200).await);
        assert_eq!(reads.get(), 0, "no row beside it, so the top is never read");
        beside(&ctx, vec![json!({ "cwd": "/repo/a", "startedAt": 150 })]);
        assert!(area.shared_folder(&e, "mine", "/repo/a", top(None, reads), 100, 200).await, "a row with no thread is another thread's; a still running row's end reads as the turn's end");
    });
}

// ---- E12, E13 -----------------------------------------------------------------------------------------------------

fn diff(files: Value, moved: Value) -> Value {
    json!({ "base": null, "files": files, "truncated": false, "moved": moved })
}

fn file(path: &str) -> Value {
    json!({ "path": path, "kind": "modified", "additions": 1, "deletions": 0, "patch": "@@" })
}

fn changes(wrote: Option<&[&str]>) -> TurnChangesAsk {
    TurnChangesAsk {
        session_id: "cs-1".into(),
        turn_id: "u1".into(),
        thread_id: "mine".into(),
        cwd: "/repo/a".into(),
        from: "base123".into(),
        started_at: 999_000,
        wrote: wrote.map(|w| w.iter().map(|p| p.to_string()).collect::<BTreeSet<_>>()),
    }
}

#[test]
fn read_turn_changes_records_files_moved_and_shared() {
    local(async {
        let (ctx, area, e) = host();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([file("a/x.rs")]), json!(["b/y.rs"]))));
        assert!(area.read_turn_changes(&e, changes(None)).await);
        let frames = ctx.frames.borrow().clone();
        assert_eq!(frames[1], json!({ "op": "git.turn", "cwd": "/repo/a", "from": "base123", "to": "snap1" }));
        let said = ctx.recorded_json();
        assert_eq!(said.len(), 1);
        for (k, v) in [("type", json!("session.changes")), ("workspaceId", json!("ws1")), ("sessionId", json!("cs-1")), ("turnId", json!("u1")), ("threadId", json!("mine")), ("from", json!("base123")), ("to", json!("snap1")), ("files", json!([{ "path": "a/x.rs", "kind": "modified", "additions": 1, "deletions": 0 }])), ("moved", json!(["b/y.rs"]))] {
            assert_eq!(said[0][k], v, "{k}");
        }
        assert_eq!(said[0].get("shared"), None);
        assert_eq!(said[0].get("others"), None);
    });
}

#[test]
fn read_turn_changes_splits_own_files_from_others() {
    local(async {
        let (ctx, area, e) = host();
        beside(&ctx, vec![json!({ "threadId": "other", "cwd": "/repo/a", "startedAt": 999_500 })]);
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([file("a/x.rs"), file("a/z.rs")]), json!([]))));
        ctx.daemon_answers("git.worktrees", Ok(json!({ "worktrees": [{ "path": "/repo" }] })));
        assert!(area.read_turn_changes(&e, changes(Some(&["/repo/a/x.rs", "../outside.rs"]))).await);
        let said = ctx.recorded_json();
        assert_eq!(said[0]["files"], json!([{ "path": "a/x.rs", "kind": "modified", "additions": 1, "deletions": 0 }]));
        assert_eq!(said[0]["others"], json!([{ "path": "a/z.rs", "kind": "modified", "additions": 1, "deletions": 0 }]));
        assert_eq!(said[0]["shared"], json!(true));
        let reads = Rc::new(std::cell::Cell::new(0));
        assert_eq!(area.own_in("/repo/a", top(Some("/repo"), reads.clone()), &BTreeSet::new()).await, Some(BTreeSet::new()));
        assert_eq!(reads.get(), 0, "nothing written reads no top");
        assert_eq!(area.own_in("/repo/a", top(None, reads), &["x".to_string()].into_iter().collect()).await, None);
    });
}

#[test]
fn read_turn_changes_of_nothing_changed_records_nothing() {
    local(async {
        let (ctx, area, e) = host();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap1" })));
        ctx.daemon_answers("git.turn", Ok(diff(json!([]), json!([]))));
        assert!(area.read_turn_changes(&e, changes(None)).await, "the range was read");
        assert!(ctx.recorded.borrow().is_empty());
        ctx.daemon.borrow_mut().clear();
        ctx.daemon_answers("git.snapshot", Ok(json!({ "commit": "snap2" })));
        ctx.daemon_answers("git.turn", Err(ask_failure("bad range", None)));
        let warned = wsp_core::console::capture();
        assert!(!area.read_turn_changes(&e, changes(None)).await, "a range that did not read");
        assert_eq!(warned.lines(), vec!["what u1 changed in /repo/a was not read: bad range"]);
        *ctx.reach.borrow_mut() = "no-daemon".into();
        assert!(!area.read_turn_changes(&e, changes(None)).await, "no snapshot");
    });
}
