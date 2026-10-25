// SPDX-License-Identifier: AGPL-3.0-only
//! forks (spec/forks.md): a fork read off the thread it forks, every refusal said before anything is written. Expected
//! values read off forks.ts at 65f6b89c7 and fork.test.ts.
mod support;

use std::rc::Rc;

use futures::FutureExt;
use serde_json::{json, Value};
use support::conversations_ctx::ws_entry;
use support::forks_ctx::{handle, FakeForkAdapter, FakeForksCtx, ForkAnswer};
use support::{err, event, json_of, local, record, records_of, row, sessions_of, settle, view};
use wsp_adapters::port::{CountedTurn, ForkTurn};
use wsp_core::console;
use wsp_core::types::api::{ForkOptions, StartOptions};
use wsp_threads::threads::forks::{
    copied_events, copy_images, fork_opened, fork_start, fork_turn_of, move_session, pay_cut_owed, read_fork, ready_fork, ForkCut, ForkPicks, ForkPlan, ForkPlanSource,
    ForkPlanTurn, ForksCtx,
};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::sub_thread::SUB_TURN_GONE_LINE;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::session::{ForkSource, ForkedFrom};
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::{refusal, usage_refusal, not_found_refusal, HostError, Kind};
use wsp_words::projects::{no_branches_line, place_branch_line};
use wsp_words::words::computer::thread_word;
use wsp_words::words::thread::{
    fork_agent_fix, fork_agent_line, fork_no_anchor_line, fork_no_turn_line, fork_turns_fix, fork_turns_line, FORK_BESIDE_FIX, FORK_BESIDE_LINE, FORK_NO_ANCHOR_FIX,
    FORK_NO_CHECKPOINT_FIX, FORK_NO_CHECKPOINT_LINE, FORK_NO_TURN_FIX, FORK_RESUME_FIX, FORK_RESUME_LINE, FORK_RUNNING_FIX, FORK_RUNNING_LINE,
};

const THREAD: &str = "th01abcd-1111-4000-8000-000000000001";

// ---- builders ----------------------------------------------------------------------------------------------------

fn ok<T>(r: Result<T, HostError>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("refused: {} ({:?})", e.message, e.kind),
    }
}

fn refused<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("the call was not refused"),
    }
}

fn same_refusal(got: &HostError, want: HostError) {
    assert_eq!(got.message, want.message);
    assert_eq!(got.fix, want.fix);
    assert_eq!(got.kind, want.kind);
}

/// An event of the source thread on workspace ws1, session r1.
fn ev(ty: &str, turn: Option<&str>, at: Option<i64>, extra: Value) -> SessionEvent {
    let mut v = json!({ "type": ty, "workspaceId": "ws1", "sessionId": "r1", "threadId": THREAD });
    if let Some(t) = turn {
        v["turnId"] = json!(t);
    }
    if let Some(a) = at {
        v["at"] = json!(a);
    }
    for (k, x) in extra.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    event(v)
}

/// One finished turn: its start, a checkpoint at `ref/<turn>` with anchor `a<n>`, its done and its end.
fn turn_events(turn: &str, n: u32) -> Vec<SessionEvent> {
    let at = Some(i64::from(n) * 100);
    vec![
        ev("session.start", Some(turn), at, json!({ "prompt": format!("p{n}"), "requestId": format!("req{n}") })),
        ev("session.checkpoint", Some(turn), at, json!({ "ref": format!("refs/c/{turn}"), "anchor": format!("a{n}") })),
        ev("session.done", Some(turn), at, json!({ "result": { "status": "completed", "text": format!("reply {n}") } })),
        ev("session.end", Some(turn), at, json!({ "exitCode": 0, "sawResult": true })),
    ]
}

fn earlier_event(text: &str) -> SessionEvent {
    ev("session.earlier", None, None, json!({ "who": "person", "text": text }))
}

fn slate_event(turn: &str) -> SessionEvent {
    ev("session.slate", Some(turn), None, json!({ "cause": "write", "version": 1, "by": "agent", "pieces": [] }))
}

/// Two earlier rows, then three finished turns u1 to u3 with a slate event among them.
fn standard_events() -> Vec<SessionEvent> {
    let mut events = vec![earlier_event("Remember ALPHA."), earlier_event("ALPHA kept.")];
    events.extend(turn_events("u1", 1));
    events.push(slate_event("u2"));
    events.extend(turn_events("u2", 2));
    events.extend(turn_events("u3", 3));
    events
}

/// Each event as `<turn or ->:<type>`.
fn tags(events: &[SessionEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| {
            let v = json_of(e);
            format!("{}:{}", v["turnId"].as_str().unwrap_or("-"), v["type"].as_str().expect("a type"))
        })
        .collect()
}

fn earlier_tags() -> Vec<String> {
    vec!["-:session.earlier".to_string(), "-:session.earlier".to_string()]
}

fn turn_tags(turn: &str) -> Vec<String> {
    ["session.start", "session.checkpoint", "session.done", "session.end"].iter().map(|t| format!("{turn}:{t}")).collect()
}

fn joined(parts: &[Vec<String>]) -> Vec<String> {
    parts.iter().flatten().cloned().collect()
}

fn fork_of(turn: Option<&str>, at: Option<f64>) -> ForkSource {
    ForkSource { thread_id: THREAD.into(), turn_id: turn.map(String::from), at: at.map(|n| JsInt(JsNumber(n))) }
}

fn opened(fork: Option<ForkSource>) -> StartOptions {
    StartOptions { prompt: "list every code word you remember".into(), fork, ..Default::default() }
}

fn result_of(v: Value) -> TurnResult {
    serde_json::from_value(v).expect("a TurnResult")
}

fn scoped(thread: &str, root: &str) -> Caller {
    let origin = serde_json::from_value(json!("here")).expect("an origin");
    let by = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "ws1", "rootThreadId": root })).expect("a scope");
    Caller::Scoped { origin, by }
}

struct F {
    ctx: Rc<FakeForksCtx>,
    dyn_ctx: Rc<dyn ForksCtx>,
}

/// A thread of Claude Code on workspace ws1 (local, project p1) with one row, a record and the standard events. Its
/// latest row ran at claude-opus-5, high effort, a 1m window, fast, in /work/ws1/sub/dir on session cs1; its record
/// names acceptEdits.
fn fixture() -> F {
    let ctx = FakeForksCtx::new();
    ctx.add_entry(ws_entry(json!({})));
    ctx.sessions.with(|s| {
        *s = sessions_of(vec![row(
            view(json!({
                "id": "r1", "workspaceId": "ws1", "threadId": THREAD, "claudeSessionId": "cs1", "cwd": "/work/ws1/sub/dir", "startedAt": 30, "status": "completed",
                "model": "claude-opus-5", "effort": "high", "contextWindow": "1m", "fast": true, "permissionMode": "default", "harnessTitle": "pricing page",
            })),
            "u3",
        )])
    });
    ctx.thread_records.with(|r| *r = records_of(vec![(THREAD, record("ws1", json!({ "harness": "claude", "permissionMode": "acceptEdits" })))]));
    ctx.events_of("ws1", THREAD, standard_events());
    let dyn_ctx = ctx.ctx();
    F { ctx, dyn_ctx }
}

fn plan_for(harness: &str, turn: Option<ForkPlanTurn>) -> ForkPlan {
    ForkPlan {
        source: ForkPlanSource {
            thread_id: THREAD.into(),
            workspace_id: "ws1".into(),
            entry: ws_entry(json!({})),
            harness: harness.into(),
            title: "pricing page".into(),
            session: Some("cs1".into()),
            cwd: None,
        },
        turn,
        events: vec![],
        picks: ForkPicks::default(),
        forked_from: ForkedFrom { thread_id: THREAD.into(), turn_id: None, title: "pricing page".into() },
    }
}

// ---- fork_opened (F1) ----------------------------------------------------------------------------------------------

// TS: fork.test.ts:565, :572
#[test]
fn fork_opened_refuses_a_fork_beside_a_thread_or_a_conversation() {
    local(async {
        let f = fixture();
        let mut beside_thread = opened(Some(fork_of(None, None)));
        beside_thread.thread = Some(THREAD.into());
        let got = refused(fork_opened(&f.dyn_ctx, "ws1", beside_thread, None).await);
        same_refusal(&got, usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX));

        let mut beside_replaces = opened(Some(fork_of(None, None)));
        beside_replaces.replaces = Some("t0".into());
        let got = refused(fork_opened(&f.dyn_ctx, "ws1", beside_replaces, None).await);
        same_refusal(&got, usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX));

        let outside = wsp_core::types::api::OutsideOpening { harness: "claude".into(), id: "0d0d0d0d".into(), copy: true, project: "lab".into(), cwd: None, title: "lab".into(), earlier: vec![] };
        let mut beside_outside = opened(Some(fork_of(None, None)));
        beside_outside.outside = Some(outside.clone());
        let got = refused(fork_opened(&f.dyn_ctx, "ws1", beside_outside, None).await);
        same_refusal(&got, usage_refusal(FORK_RESUME_LINE, FORK_RESUME_FIX));

        let mut both = opened(Some(fork_of(None, None)));
        both.thread = Some(THREAD.into());
        both.outside = Some(outside);
        let got = refused(fork_opened(&f.dyn_ctx, "ws1", both, None).await);
        same_refusal(&got, usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX));

        assert_eq!(f.ctx.called(), Vec::<String>::new(), "every refusal comes before the source is read");
    });
}

#[test]
fn fork_opened_answers_a_start_with_no_fork_itself_and_no_plan() {
    local(async {
        let f = fixture();
        let mut start = opened(None);
        start.model = Some("m1".into());
        start.thread = Some("t9".into());
        let (out, plan) = ok(fork_opened(&f.dyn_ctx, "ws1", start, None).await);
        assert!(plan.is_none());
        assert_eq!((out.prompt.as_str(), out.model.as_deref(), out.thread.as_deref(), out.harness.clone()), ("list every code word you remember", Some("m1"), Some("t9"), None));
        assert_eq!(f.ctx.called(), Vec::<String>::new(), "nothing was read");
    });
}

// TS: fork.test.ts:202
#[test]
fn fork_opened_runs_on_the_sources_agent_and_picks() {
    local(async {
        let f = fixture();
        let (out, plan) = ok(fork_opened(&f.dyn_ctx, "ws1", opened(Some(fork_of(None, None))), None).await);
        let plan = plan.expect("a plan");
        assert_eq!(plan.turn.as_ref().map(|t| t.turn_id.clone()), Some("u3".to_string()));
        assert_eq!(out.harness.as_deref(), Some("claude"));
        assert_eq!(out.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(out.effort.as_deref(), Some("high"));
        assert_eq!(out.context_window.as_deref(), Some("1m"));
        assert_eq!(out.fast, Some(true));
        assert_eq!(out.permission_mode.as_deref(), Some("acceptEdits"), "the record's mode, not the row's");
        assert_eq!(out.cwd.as_deref(), Some("/work/ws1/sub/dir"), "a fork on its source's own workspace takes the source's cwd");
        assert_eq!(out.prompt, "list every code word you remember");
        assert!(out.fork.is_some(), "the fork stays on the start");

        let (elsewhere, _) = ok(fork_opened(&f.dyn_ctx, "ws2", opened(Some(fork_of(None, None))), None).await);
        assert_eq!(elsewhere.cwd, None, "a fork on another workspace takes no cwd");
        assert_eq!(elsewhere.model.as_deref(), Some("claude-opus-5"));

        f.ctx.sessions.with(|s| s.get_mut("r1").expect("a row").view.fast = Some(false));
        let (slow, _) = ok(fork_opened(&f.dyn_ctx, "ws1", opened(Some(fork_of(None, None))), None).await);
        assert_eq!(slow.fast, None, "fast only where the source ran fast");
    });
}

#[test]
fn fork_opened_keeps_what_the_start_named() {
    local(async {
        let f = fixture();
        let mut named = opened(Some(fork_of(None, None)));
        named.harness = Some("claude".into());
        named.model = Some("claude-sonnet-5".into());
        named.effort = Some("low".into());
        named.context_window = Some("200k".into());
        named.fast = Some(false);
        named.access = Some(AccessChoice::Full);
        named.cwd = Some("/elsewhere".into());
        let (out, _) = ok(fork_opened(&f.dyn_ctx, "ws1", named, None).await);
        assert_eq!(out.harness.as_deref(), Some("claude"));
        assert_eq!(out.model.as_deref(), Some("claude-sonnet-5"));
        assert_eq!(out.effort.as_deref(), Some("low"));
        assert_eq!(out.context_window.as_deref(), Some("200k"));
        assert_eq!(out.fast, Some(false), "a named speed stays, off included");
        assert_eq!(out.permission_mode, None, "an access named leaves the source's mode off");
        assert_eq!(out.cwd.as_deref(), Some("/elsewhere"));

        let mut mode = opened(Some(fork_of(None, None)));
        mode.permission_mode = Some("plan".into());
        let (out, _) = ok(fork_opened(&f.dyn_ctx, "ws1", mode, None).await);
        assert_eq!(out.permission_mode.as_deref(), Some("plan"));
    });
}

// TS: fork.test.ts:326
#[test]
fn fork_opened_refuses_another_agent() {
    local(async {
        let f = fixture();
        let mut other = opened(Some(fork_of(None, None)));
        other.harness = Some("codex".into());
        let got = refused(fork_opened(&f.dyn_ctx, "ws1", other, None).await);
        same_refusal(&got, refusal(&fork_agent_line("Claude Code", "Codex"), &fork_agent_fix("Codex"), Some(Kind::Usage)));
        assert!(f.ctx.called().contains(&format!("openThread ws1 {THREAD}")), "the source is read first, as the refusal names its agent");
    });
}

// ---- read_fork (F2 to F5) ------------------------------------------------------------------------------------------

// TS: fork.test.ts:225, :293
#[test]
fn read_fork_picks_the_latest_finished_turn_by_default() {
    local(async {
        let f = fixture();
        let plan = ok(read_fork(&f.dyn_ctx, &fork_of(None, None), None).await);
        let turn = plan.turn.clone().expect("a turn");
        assert_eq!(turn.turn_id, "u3");
        assert_eq!(tags(&plan.events), joined(&[earlier_tags(), turn_tags("u1"), turn_tags("u2"), turn_tags("u3")]), "the earlier rows, then every event through u3; the slate event is left out");
        assert_eq!((plan.forked_from.thread_id.as_str(), plan.forked_from.turn_id.as_deref(), plan.forked_from.title.as_str()), (THREAD, Some("u3"), "pricing page"));
        assert_eq!(f.ctx.called(), vec!["ready".to_string(), format!("entryOfRow {THREAD} ws1"), format!("openThread ws1 {THREAD}")]);

        let second = ok(read_fork(&f.dyn_ctx, &fork_of(None, Some(2.0)), None).await);
        assert_eq!(second.turn.as_ref().map(|t| t.turn_id.clone()), Some("u2".to_string()), "--at counts from 1");
        assert_eq!(tags(&second.events), joined(&[earlier_tags(), turn_tags("u1"), turn_tags("u2")]));
        assert_eq!(second.turn.expect("a turn").after.len(), 1);

        let named = ok(read_fork(&f.dyn_ctx, &fork_of(Some("u1"), None), None).await);
        assert_eq!(tags(&named.events), joined(&[earlier_tags(), turn_tags("u1")]));
        assert_eq!(named.forked_from.turn_id.as_deref(), Some("u1"));
    });
}

// TS: fork.test.ts:430
#[test]
fn read_fork_refuses_a_running_turn_and_a_turn_never_run() {
    local(async {
        let f = fixture();
        f.ctx.running.borrow_mut().insert(THREAD.into(), "u3".into());
        let got = refused(read_fork(&f.dyn_ctx, &fork_of(Some("u3"), None), None).await);
        same_refusal(&got, refusal(FORK_RUNNING_LINE, FORK_RUNNING_FIX, Some(Kind::Conflict)));
        let latest = ok(read_fork(&f.dyn_ctx, &fork_of(None, None), None).await);
        assert_eq!(latest.turn.map(|t| t.turn_id), Some("u2".to_string()), "the running turn is not finished: the latest finished is u2");
        let earlier = ok(read_fork(&f.dyn_ctx, &fork_of(Some("u1"), None), None).await);
        assert_eq!(earlier.turn.map(|t| t.turn_id), Some("u1".to_string()), "an earlier turn forks while the later one runs");

        f.ctx.running.borrow_mut().clear();
        f.ctx.launching.borrow_mut().insert(THREAD.into(), "u2".into());
        let got = refused(read_fork(&f.dyn_ctx, &fork_of(Some("u2"), None), None).await);
        same_refusal(&got, refusal(FORK_RUNNING_LINE, FORK_RUNNING_FIX, Some(Kind::Conflict)));

        f.ctx.launching.borrow_mut().clear();
        let mut open = turn_events("u4", 4);
        open.truncate(1);
        f.ctx.thread_events.borrow_mut().get_mut(&("ws1".to_string(), THREAD.to_string())).expect("events").extend(open);
        let got = refused(read_fork(&f.dyn_ctx, &fork_of(Some("u4"), None), None).await);
        same_refusal(&got, refusal(FORK_RUNNING_LINE, FORK_RUNNING_FIX, Some(Kind::Conflict)));

        let got = refused(read_fork(&f.dyn_ctx, &fork_of(Some("u9"), None), None).await);
        same_refusal(&got, refusal(&fork_no_turn_line("u9"), FORK_NO_TURN_FIX, Some(Kind::NotFound)));
    });
}

// TS: fork.test.ts:293
#[test]
fn read_fork_refuses_an_at_out_of_range() {
    local(async {
        let f = fixture();
        let got = refused(read_fork(&f.dyn_ctx, &fork_of(None, Some(4.0)), None).await);
        same_refusal(&got, refusal(&fork_turns_line(3.0), &fork_turns_fix(3.0), Some(Kind::Usage)));

        let nothing = fixture();
        let mut only_a_start = turn_events("u1", 1);
        only_a_start.truncate(1);
        nothing.ctx.events_of("ws1", THREAD, only_a_start);
        let got = refused(read_fork(&nothing.dyn_ctx, &fork_of(None, None), None).await);
        same_refusal(&got, refusal(&fork_turns_line(0.0), &fork_turns_fix(0.0), Some(Kind::Usage)));
    });
}

// TS: fork.test.ts:293, :248
#[test]
fn read_fork_at_zero_carries_no_turn_and_no_earlier_rows() {
    local(async {
        let f = fixture();
        let plan = ok(read_fork(&f.dyn_ctx, &fork_of(None, Some(0.0)), None).await);
        assert!(plan.turn.is_none());
        assert_eq!(tags(&plan.events), Vec::<String>::new(), "no turn, so none of the source's events nor the conversation it opened on");
        assert_eq!((plan.forked_from.turn_id.clone(), plan.forked_from.title.as_str()), (None, "pricing page"));
        assert_eq!(plan.picks.model.as_deref(), Some("claude-opus-5"), "the picks come off the latest row all the same");
    });
}

// TS: fork.test.ts:248
#[test]
fn read_fork_carries_the_earlier_rows_ahead_of_the_turns_it_carries() {
    local(async {
        let f = fixture();
        let plan = ok(read_fork(&f.dyn_ctx, &fork_of(Some("u1"), None), None).await);
        let first = json_of(&plan.events[0]);
        assert_eq!((first["type"].as_str(), first["text"].as_str()), (Some("session.earlier"), Some("Remember ALPHA.")));
        let second = json_of(&plan.events[1]);
        assert_eq!(second["text"].as_str(), Some("ALPHA kept."));
    });
}

#[test]
fn read_fork_refuses_a_thread_the_caller_does_not_reach() {
    local(async {
        let f = fixture();
        f.ctx.unreached.borrow_mut().insert(THREAD.into());
        let got = refused(read_fork(&f.dyn_ctx, &fork_of(None, None), None).await);
        same_refusal(&got, not_found_refusal(&format!("no thread {}", thread_word(THREAD))));
        assert!(got.message.ends_with("no thread th01abcd"));
        assert!(!f.ctx.called().iter().any(|c| c.starts_with("openThread")), "the thread is not read for a caller that cannot reach it");

        let unknown = FakeForksCtx::new();
        let ctx: Rc<dyn ForksCtx> = unknown.ctx();
        let got = refused(read_fork(&ctx, &fork_of(None, None), None).await);
        same_refusal(&got, not_found_refusal(&format!("no thread {}", thread_word(THREAD))));
        assert_eq!(unknown.called(), vec!["ready".to_string()], "a thread with no row and no record is not asked for");
    });
}

// TS: fork.test.ts:430
#[test]
fn read_fork_refuses_a_turn_of_a_deleted_sub_machine() {
    local(async {
        let sub = json!({ "on": "box", "repo": "/r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/f", "releasedAt": 100 });
        let f = fixture();
        f.ctx.add_entry(ws_entry(json!({ "sub": sub })));
        let mut events = vec![];
        events.push(ev("session.start", Some("u1"), None, json!({ "prompt": "p1" })));
        events.push(ev("session.checkpoint", Some("u1"), Some(50), json!({ "ref": "refs/c/u1", "anchor": "a1" })));
        events.push(ev("session.done", Some("u1"), Some(60), json!({ "result": { "status": "completed" } })));
        events.push(ev("session.end", Some("u1"), Some(70), json!({ "exitCode": 0, "sawResult": true })));
        events.push(ev("session.start", Some("u2"), None, json!({ "prompt": "p2" })));
        events.push(ev("session.checkpoint", Some("u2"), Some(150), json!({ "ref": "refs/c/u2", "anchor": "a2" })));
        events.push(ev("session.done", Some("u2"), Some(160), json!({ "result": { "status": "completed" } })));
        events.push(ev("session.end", Some("u2"), Some(170), json!({ "exitCode": 0, "sawResult": true })));
        events.push(ev("session.start", Some("u3"), None, json!({ "prompt": "p3" })));
        events.push(ev("session.done", Some("u3"), None, json!({ "result": { "status": "completed" } })));
        events.push(ev("session.end", Some("u3"), None, json!({ "exitCode": 0, "sawResult": true })));
        f.ctx.events_of("ws1", THREAD, events);

        let gone = HostError::with_kind(SUB_TURN_GONE_LINE, Kind::Conflict);
        let got = refused(read_fork(&f.dyn_ctx, &fork_of(Some("u1"), None), None).await);
        same_refusal(&got, gone.clone());
        let got = refused(read_fork(&f.dyn_ctx, &fork_of(Some("u3"), None), None).await);
        same_refusal(&got, gone);
        let after = ok(read_fork(&f.dyn_ctx, &fork_of(Some("u2"), None), None).await);
        assert_eq!(after.turn.map(|t| t.turn_id), Some("u2".to_string()), "u2's first event with a time is at 150, after the release at 100");

        let plain = fixture();
        assert!(read_fork(&plain.dyn_ctx, &fork_of(Some("u1"), None), None).await.is_ok(), "a workspace with no sub record refuses nothing");
    });
}

// TS: fork.test.ts:202, :409
#[test]
fn read_fork_plan_carries_the_turns_events_anchor_ref_and_the_turns_after() {
    local(async {
        let f = fixture();
        let mut events = vec![earlier_event("Remember ALPHA.")];
        events.push(ev("session.start", Some("u1"), Some(10), json!({ "prompt": "p1" })));
        events.push(ev("session.checkpoint", Some("u1"), Some(11), json!({ "ref": "refs/c/u1", "anchor": "a1" })));
        events.push(ev("session.checkpoint", Some("u1"), Some(12), json!({ "ref": "refs/c/u1b", "anchor": "a1b" })));
        events.push(ev("session.done", Some("u1"), Some(13), json!({ "result": { "status": "completed", "text": "one" } })));
        events.push(ev("session.end", Some("u1"), Some(14), json!({ "exitCode": 0, "sawResult": true })));
        events.push(ev("session.start", Some("u2"), Some(20), json!({ "prompt": "p2" })));
        events.push(ev("session.done", Some("u2"), Some(21), json!({ "result": { "status": "completed", "text": "two" } })));
        events.push(ev("session.end", Some("u2"), Some(22), json!({ "exitCode": 0, "sawResult": true })));
        events.push(ev("session.start", Some("u3"), Some(30), json!({ "prompt": "p3" })));
        events.push(ev("session.checkpoint", Some("u3"), Some(31), json!({ "anchor": "a3" })));
        events.push(ev("session.done", Some("u3"), Some(32), json!({ "result": { "status": "completed", "text": "three" } })));
        events.push(ev("session.end", Some("u3"), Some(33), json!({ "exitCode": 0, "sawResult": true })));
        f.ctx.events_of("ws1", THREAD, events);

        let plan = ok(read_fork(&f.dyn_ctx, &fork_of(Some("u1"), None), None).await);
        let source = &plan.source;
        assert_eq!((source.thread_id.as_str(), source.workspace_id.as_str(), source.harness.as_str(), source.title.as_str()), (THREAD, "ws1", "claude", "pricing page"));
        assert_eq!((source.session.as_deref(), source.cwd.as_deref()), (Some("cs1"), Some("/work/ws1/sub/dir")));
        assert_eq!(source.entry.record.id, "ws1");
        assert_eq!(
            plan.turn,
            Some(ForkPlanTurn {
                turn_id: "u1".into(),
                anchor: Some("a1b".into()),
                at_ref: Some("refs/c/u1b".into()),
                after: vec![
                    CountedTurn { anchor: None, result: Some(result_of(json!({ "status": "completed", "text": "two" }))) },
                    CountedTurn { anchor: Some("a3".into()), result: Some(result_of(json!({ "status": "completed", "text": "three" }))) },
                ],
            }),
            "the picked turn's last checkpoint, and each later turn's last checkpoint and last done"
        );
        assert_eq!(
            plan.picks,
            ForkPicks {
                model: Some("claude-opus-5".into()),
                effort: Some("high".into()),
                context_window: Some("1m".into()),
                fast: Some(true),
                permission_mode: Some("acceptEdits".into()),
            }
        );
        assert_eq!(tags(&plan.events), joined(&[vec!["-:session.earlier".to_string()], vec!["u1:session.start".into(), "u1:session.checkpoint".into(), "u1:session.checkpoint".into(), "u1:session.done".into(), "u1:session.end".into()]]));
        assert_eq!(json_of(&plan.forked_from), json!({ "threadId": THREAD, "turnId": "u1", "title": "pricing page" }));
    });
}

#[test]
fn read_fork_reads_a_thread_with_no_rows_off_its_record() {
    local(async {
        let f = fixture();
        f.ctx.sessions.with(|s| s.clear());
        f.ctx.thread_records.with(|r| {
            *r = records_of(vec![(THREAD, record("ws1", json!({ "harness": "codex", "forkedFrom": { "threadId": "t0", "title": "The parent" } })))]);
        });
        f.ctx.started_as.borrow_mut().insert(("ws1".into(), THREAD.into()), "cs-start".into());
        f.ctx.folders.borrow_mut().insert(("ws1".into(), "cs-start".into()), "/work/ws1".into());
        let plan = ok(read_fork(&f.dyn_ctx, &fork_of(None, None), None).await);
        let s = &plan.source;
        assert_eq!((s.workspace_id.as_str(), s.harness.as_str(), s.title.as_str()), ("ws1", "codex", "The parent"), "the record's workspace, harness and forkedFrom title");
        assert_eq!((s.session.as_deref(), s.cwd.as_deref()), (Some("cs-start"), Some("/work/ws1")), "the session the thread started as and the folder it ran in");
        assert_eq!(plan.picks, ForkPicks::default(), "no latest row, no picks");

        f.ctx.thread_records.with(|r| *r = records_of(vec![(THREAD, record("ws1", json!({ "harness": "codex" })))]));
        let plan = ok(read_fork(&f.dyn_ctx, &fork_of(None, None), None).await);
        assert_eq!(plan.source.title, "th01abcd", "no title anywhere reads as the thread word");
        assert_eq!(plan.forked_from.title, "th01abcd");
    });
}

#[test]
fn read_fork_takes_the_harness_off_the_record_and_the_workspace_off_the_latest_row() {
    local(async {
        let f = fixture();
        f.ctx.thread_records.with(|r| *r = records_of(vec![(THREAD, record("ws9", json!({ "harness": "codex" })))]));
        let plan = ok(read_fork(&f.dyn_ctx, &fork_of(None, None), None).await);
        assert_eq!(plan.source.harness, "codex", "the record's harness over the row's");
        assert_eq!(plan.source.workspace_id, "ws1", "the latest row's workspace over the record's");
        assert_eq!(plan.picks.permission_mode.as_deref(), Some("default"), "no permission mode on the record: the row's");
    });
}

// ---- fork_turn_of (F6) ---------------------------------------------------------------------------------------------

// TS: fork.test.ts:326, :391, :409
#[test]
fn fork_turn_of_refuses_a_turn_with_no_anchor_on_a_harness_that_does_not_count() {
    let after = vec![CountedTurn { anchor: Some("a2".into()), result: None }, CountedTurn::default()];
    let anchored = plan_for("claude", Some(ForkPlanTurn { turn_id: "u1".into(), anchor: Some("a1".into()), at_ref: None, after: after.clone() }));
    assert_eq!(fork_turn_of(&anchored, false), Ok(Some(ForkCut::Anchor("a1".into()))));
    assert_eq!(fork_turn_of(&anchored, true), Ok(Some(ForkCut::Anchor("a1".into()))), "an anchor wins on a harness that counts too");

    let bare = plan_for("claude", Some(ForkPlanTurn { turn_id: "u1".into(), anchor: None, at_ref: None, after: after.clone() }));
    let got = fork_turn_of(&bare, false).expect_err("refused");
    same_refusal(&got, refusal(&fork_no_anchor_line("Claude Code"), FORK_NO_ANCHOR_FIX, Some(Kind::Conflict)));
    assert_eq!(fork_turn_of(&bare, true), Ok(Some(ForkCut::After(after))), "a harness that counts finds the turn by the turns after it");

    let codex = plan_for("codex", Some(ForkPlanTurn { turn_id: "u1".into(), anchor: None, at_ref: None, after: vec![] }));
    let got = fork_turn_of(&codex, false).expect_err("refused");
    same_refusal(&got, refusal(&fork_no_anchor_line("Codex"), FORK_NO_ANCHOR_FIX, Some(Kind::Conflict)));
    let unknown = plan_for("mystery", Some(ForkPlanTurn { turn_id: "u1".into(), anchor: None, at_ref: None, after: vec![] }));
    let got = fork_turn_of(&unknown, false).expect_err("refused");
    same_refusal(&got, refusal(&fork_no_anchor_line("mystery"), FORK_NO_ANCHOR_FIX, Some(Kind::Conflict)));

    assert_eq!(fork_turn_of(&plan_for("claude", None), false), Ok(None), "no turn, no cut");
}

// ---- copied_events (F7) --------------------------------------------------------------------------------------------

// TS: fork.test.ts:225
#[test]
fn copied_events_drop_pos_and_mark_the_source() {
    let mut plan = plan_for("claude", None);
    let already = ev("session.done", Some("u1"), Some(5), json!({ "result": { "status": "completed" }, "pos": 2, "copiedFrom": "t0" }));
    plan.events = vec![
        ev("session.start", Some("u1"), Some(4), json!({ "prompt": "p1", "pos": 1 })),
        already,
        earlier_event("Remember ALPHA."),
    ];
    let copies = copied_events(&plan, "wsB", "tB");
    let json: Vec<Value> = copies.iter().map(json_of).collect();
    assert_eq!(json.len(), 3);
    for (i, v) in json.iter().enumerate() {
        assert!(v.get("pos").is_none(), "event {i} keeps no pos");
        assert_eq!(v["workspaceId"], "wsB", "event {i}");
        assert_eq!(v["threadId"], "tB", "event {i}");
        assert_eq!(v["sessionId"], "r1", "event {i} keeps the rest as it was");
    }
    assert_eq!(json[0]["copiedFrom"], THREAD, "an event of the source's own is marked with the source");
    assert_eq!((json[0]["type"].as_str(), json[0]["prompt"].as_str(), json[0]["turnId"].as_str(), json[0]["at"].as_i64()), (Some("session.start"), Some("p1"), Some("u1"), Some(4)));
    assert_eq!(json[1]["copiedFrom"], "t0", "an event that was already a copy keeps the thread it came from");
    assert_eq!(json[2]["copiedFrom"], THREAD);
    assert!(json[2].get("turnId").is_none(), "an earlier row stays under no turn");
    assert_eq!(json[2]["text"], "Remember ALPHA.");
}

// ---- copy_images (F8) ----------------------------------------------------------------------------------------------

fn image_event(ty: &str, turn: &str, request: Option<&str>, attachments: Value) -> SessionEvent {
    let mut extra = json!({ "prompt": "look", "attachments": attachments });
    if let Some(r) = request {
        extra["requestId"] = json!(r);
    }
    let mut v = json!({ "type": ty, "workspaceId": "ws1", "sessionId": "r1", "threadId": "t1", "turnId": turn });
    for (k, x) in extra.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    event(v)
}

fn kept(f: &F) -> Vec<(String, String, Option<String>, Vec<(String, String, String)>)> {
    f.ctx
        .kept_images
        .borrow()
        .iter()
        .map(|(ws, thread, request, images)| (ws.clone(), thread.clone(), request.clone(), images.iter().map(|a| (a.name.clone().unwrap_or_else(|| "<none>".into()), a.media_type.clone(), a.bytes.clone())).collect()))
        .collect()
}

// TS: fork.test.ts:547
#[test]
fn copy_images_keeps_each_image_at_its_index_under_the_fork() {
    local(async {
        let f = fixture();
        let mut plan = plan_for("claude", None);
        plan.source.thread_id = "t1".into();
        plan.events = vec![
            image_event(
                "session.start",
                "u1",
                Some("req1"),
                json!([
                    { "mediaType": "image/png", "bytes": 5, "name": "dot.png" },
                    { "mediaType": "text/plain", "bytes": 3, "name": "notes.txt" },
                    { "mediaType": "image/png", "bytes": 5 },
                    { "mediaType": "image/png", "bytes": 4, "name": "gone.png" },
                ]),
            ),
            image_event("session.steer", "u1", Some("req2"), json!([{ "mediaType": "image/png", "bytes": 3, "name": "two.png" }])),
            image_event("session.start", "u2", Some("req3"), json!([{ "mediaType": "text/plain", "bytes": 3, "name": "only.txt" }])),
            ev("session.start", Some("u3"), None, json!({ "prompt": "no files" })),
            image_event("session.start", "u4", None, json!([{ "mediaType": "image/png", "bytes": 5, "name": "orphan.png" }])),
        ];
        f.ctx.store.seed(
            "attachment-keys",
            "ws1",
            json!({ "threads": {
                "t1": [{ "key": "t1.req1.0", "mediaType": "image/png" }, { "key": "t1.req1.3", "mediaType": "image/png" }, { "key": "t1.req2.0", "mediaType": "image/png" }],
                "t9": [{ "key": "t9.req1.0", "mediaType": "image/png" }],
            } }),
        );
        f.ctx.store.seed_blob("attachments", "t1.req1.0", b"hello");
        f.ctx.store.seed_blob("attachments", "t1.req2.0", b"two");
        let settled = f.ctx.clone();
        f.ctx.kept_writes.borrow_mut().insert("ws1".into(), async move { settled.call("keptWrites settled".into()) }.boxed_local().shared());

        ok(copy_images(&f.dyn_ctx, &plan, "ws2", "t2").await);
        assert_eq!(
            kept(&f),
            vec![
                (
                    "ws2".to_string(),
                    "t2".to_string(),
                    Some("req1".to_string()),
                    vec![
                        ("dot.png".to_string(), "image/png".to_string(), "aGVsbG8=".to_string()),
                        ("notes.txt".to_string(), "text/plain".to_string(), String::new()),
                        ("".to_string(), "text/plain".to_string(), String::new()),
                        ("gone.png".to_string(), "text/plain".to_string(), String::new()),
                    ]
                ),
                ("ws2".to_string(), "t2".to_string(), Some("req2".to_string()), vec![("two.png".to_string(), "image/png".to_string(), "dHdv".to_string())]),
                ("ws2".to_string(), "t2".to_string(), None, vec![("orphan.png".to_string(), "text/plain".to_string(), String::new())]),
            ],
            "an image whose key the source holds and whose blob reads is kept with its bytes; any other entry keeps its place as an empty text/plain; an event with no image is skipped"
        );
        let calls = f.ctx.called();
        assert_eq!(&calls[..3], &["keptWrites ws1".to_string(), "keptWrites settled".to_string(), "store.get attachment-keys/ws1".to_string()], "the source's pending writes settle first");
        let blobs: Vec<&String> = calls.iter().filter(|c| c.starts_with("store.getBlob")).collect();
        assert_eq!(blobs, vec!["store.getBlob attachments/t1.req1.0", "store.getBlob attachments/t1.req1.3", "store.getBlob attachments/t1.req2.0"], "blobs are read for held keys only, event by event");
    });
}

#[test]
fn copy_images_copies_nothing_where_the_source_holds_no_images() {
    local(async {
        let f = fixture();
        let mut plan = plan_for("claude", None);
        plan.source.thread_id = "t1".into();
        plan.events = vec![image_event("session.start", "u1", Some("req1"), json!([{ "mediaType": "image/png", "bytes": 5, "name": "dot.png" }]))];
        f.ctx.store.seed("attachment-keys", "ws1", json!({ "threads": { "t9": [{ "key": "t9.req1.0", "mediaType": "image/png" }] } }));
        ok(copy_images(&f.dyn_ctx, &plan, "ws2", "t2").await);
        assert_eq!(kept(&f), vec![], "the source thread holds no entry");
        assert!(!f.ctx.called().iter().any(|c| c.starts_with("store.getBlob")));

        let none = fixture();
        ok(copy_images(&none.dyn_ctx, &plan, "ws2", "t2").await);
        assert_eq!(kept(&none), vec![], "no record of keys at all");
    });
}

// ---- ready_fork (F9) -----------------------------------------------------------------------------------------------

// TS: fork.test.ts:312, :418
#[test]
fn ready_fork_says_the_agents_refusal_in_the_callers_words() {
    local(async {
        let adapter = FakeForkAdapter::new();
        adapter.answers(vec![ForkAnswer::Refuse("the session holds no message a2".into()), ForkAnswer::Resume("cs-copy".into())]);
        let said = |why: &str| err(&format!("said: {why}"));
        let got = refused(ready_fork(adapter.clone(), "cs1", ForkTurn::Anchor("a2".into()), &said).await);
        assert_eq!(got.message, "said: the session holds no message a2");
        match ok(ready_fork(adapter.clone(), "cs1", ForkTurn::After(vec![CountedTurn { anchor: Some("a3".into()), result: None }]), &said).await) {
            wsp_adapters::port::ForkedSession::Resume { resume, .. } => assert_eq!(resume, "cs-copy"),
            wsp_adapters::port::ForkedSession::Fork { .. } => panic!("a copy was queued"),
        }
        assert_eq!(
            adapter.asked(),
            vec![("cs1".to_string(), ForkTurn::Anchor("a2".into())), ("cs1".to_string(), ForkTurn::After(vec![CountedTurn { anchor: Some("a3".into()), result: None }]))]
        );
    });
}

// ---- move_session (F10) --------------------------------------------------------------------------------------------

// TS: fork.test.ts:340
#[test]
fn move_session_renames_only_that_threads_rows_on_that_session() {
    let f = fixture();
    f.ctx.sessions.with(|s| {
        *s = sessions_of(vec![
            row(view(json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "cs1" })), "u1"),
            row(view(json!({ "id": "r2", "threadId": "t1", "claudeSessionId": "cs2" })), "u2"),
            row(view(json!({ "id": "r3", "threadId": "t2", "claudeSessionId": "cs1" })), "u3"),
            row(view(json!({ "id": "r4", "threadId": "t1" })), "u4"),
            row(view(json!({ "id": "r5", "threadId": "t1", "claudeSessionId": "cs1" })), "u5"),
        ])
    });
    move_session(&f.dyn_ctx, "t1", "cs1", "cs9");
    let now: Vec<(String, Option<String>)> = f.ctx.sessions.with(|s| s.values().map(|r| (r.view.id.clone(), r.view.claude_session_id.clone())).collect());
    assert_eq!(
        now,
        vec![
            ("r1".to_string(), Some("cs9".to_string())),
            ("r2".to_string(), Some("cs2".to_string())),
            ("r3".to_string(), Some("cs1".to_string())),
            ("r4".to_string(), None),
            ("r5".to_string(), Some("cs9".to_string())),
        ]
    );
}

// ---- fork_start (F11) ----------------------------------------------------------------------------------------------

fn options(branch: Option<&str>, fork: ForkSource) -> ForkOptions {
    ForkOptions { start: StartOptions { prompt: "x".into(), ..Default::default() }, fork, branch: branch.map(String::from) }
}

fn made_worktree(made: bool) -> wsp_threads::threads::threads::Entry {
    ws_entry(json!({ "id": "ws2", "worktree": { "path": "/wt/feat", "branch": "main-fork", "made": made } }))
}

fn after_worktree(f: &F) -> Vec<String> {
    let calls = f.ctx.called();
    let at = calls.iter().position(|c| c.starts_with("worktreeFolder")).expect("a worktree was asked for");
    calls[at..].to_vec()
}

// TS: fork.test.ts:202
#[test]
fn fork_start_without_a_branch_starts_on_the_source() {
    local(async {
        let f = fixture();
        f.ctx.start_answers(Ok(handle("h1", "ws1")));
        let started = ok(fork_start(&f.dyn_ctx, f.ctx.start_fn(), options(None, fork_of(Some("u1"), None)), None).await);
        assert_eq!((started.id().as_str(), started.workspace_id().as_str()), ("h1", "ws1"));
        let seen = f.ctx.started.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!((seen[0].0.as_str(), seen[0].1.prompt.as_str(), seen[0].1.cwd.clone()), ("ws1", "x", None));
        assert_eq!(f.ctx.called(), vec!["ready".to_string(), format!("entryOfRow {THREAD} ws1"), format!("openThread ws1 {THREAD}"), "start ws1".to_string()], "no git, no worktree");
    });
}

// TS: fork.test.ts:326
#[test]
fn fork_start_refuses_another_agent_before_it_starts() {
    local(async {
        let f = fixture();
        let mut o = options(None, fork_of(None, None));
        o.start.harness = Some("codex".into());
        let got = refused(fork_start(&f.dyn_ctx, f.ctx.start_fn(), o, None).await);
        same_refusal(&got, refusal(&fork_agent_line("Claude Code", "Codex"), &fork_agent_fix("Codex"), Some(Kind::Usage)));
        assert_eq!(f.ctx.started.borrow().len(), 0);
    });
}

// TS: fork.test.ts:512
#[test]
fn fork_start_refuses_a_branch_with_no_checkpoint() {
    local(async {
        let f = fixture();
        let got = refused(fork_start(&f.dyn_ctx, f.ctx.start_fn(), options(Some("main-fork"), fork_of(None, Some(0.0))), None).await);
        same_refusal(&got, refusal(FORK_NO_CHECKPOINT_LINE, FORK_NO_CHECKPOINT_FIX, Some(Kind::Conflict)));
        assert!(!f.ctx.called().iter().any(|c| c.starts_with("git")), "a fork that carries no turn has no ref to check");

        let gone = fixture();
        gone.ctx.git_answers("rev-parse", 1);
        let got = refused(fork_start(&gone.dyn_ctx, gone.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
        same_refusal(&got, refusal(FORK_NO_CHECKPOINT_LINE, FORK_NO_CHECKPOINT_FIX, Some(Kind::Conflict)));
        assert!(gone.ctx.called().contains(&"git /repo/p1 rev-parse --verify -q refs/c/u1^{commit}".to_string()), "the ref is checked as a commit at the repo's top");
        assert!(!gone.ctx.called().iter().any(|c| c.starts_with("worktreeFolder")), "no worktree is made");
        assert_eq!(gone.ctx.started.borrow().len(), 0);

        let cloud = fixture();
        cloud.ctx.add_entry(ws_entry(json!({ "kind": "cloud" })));
        let got = refused(fork_start(&cloud.dyn_ctx, cloud.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
        same_refusal(&got, usage_refusal(&place_branch_line(&cloud.ctx.place_name("this")), FORK_NO_CHECKPOINT_FIX));
        assert!(!cloud.ctx.called().iter().any(|c| c.starts_with("git")));

        let bare = fixture();
        bare.ctx.projects.borrow_mut().insert("p1".into(), wsp_threads::threads::threads::HeldProject { id: "p1".into(), name: "lab".into(), path: "/repo/p1".into(), computer: "this".into(), git_top: None });
        let got = refused(fork_start(&bare.dyn_ctx, bare.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
        same_refusal(&got, usage_refusal(&no_branches_line("lab"), FORK_NO_CHECKPOINT_FIX));
    });
}

// TS: fork.test.ts:494
#[test]
fn fork_start_runs_in_the_matching_subfolder_of_the_worktree() {
    local(async {
        let f = fixture();
        f.ctx.worktrees.borrow_mut().push_back(Ok(made_worktree(true)));
        f.ctx.setup_lines.borrow_mut().insert("ws2".into(), "npm install".into());
        f.ctx.start_answers(Ok(handle("h2", "ws2")));
        let started = ok(fork_start(&f.dyn_ctx, f.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), Some(scoped("t9", "root-t"))).await);
        assert_eq!(started.workspace_id(), "ws2");
        assert_eq!(
            f.ctx.called().into_iter().skip_while(|c| !c.starts_with("git")).collect::<Vec<_>>(),
            vec![
                "git /repo/p1 rev-parse --verify -q refs/c/u1^{commit}".to_string(),
                "worktreeFolder p1 /repo/p1 main-fork ws1 root-t refs/c/u1".to_string(),
                "takeSetupLine ws2".to_string(),
                "start ws2".to_string(),
            ],
            "the worktree is made from the source's record, the caller's root thread and the turn's ref"
        );
        let seen = f.ctx.started.borrow();
        assert_eq!(seen[0].0, "ws2");
        assert_eq!(seen[0].1.cwd.as_deref(), Some("/wt/feat/sub/dir"), "the source ran in sub/dir of /work/ws1: the fork runs there in its tree");
        assert_eq!(seen[0].1.setup.as_deref(), Some("npm install"));
        assert_eq!(seen[0].1.prompt, "x");
    });
}

#[test]
fn fork_start_runs_at_the_worktrees_top_where_the_source_was_not_under_its_checkout() {
    local(async {
        for (source_cwd, checkout) in [("/work/ws1x/sub", "/work/ws1"), ("/work/ws1", "/work/ws1"), ("/elsewhere", "/work/ws1")] {
            let f = fixture();
            f.ctx.sessions.with(|s| s.get_mut("r1").expect("a row").view.cwd = Some(source_cwd.into()));
            f.ctx.checkouts.borrow_mut().insert("ws1".into(), checkout.into());
            f.ctx.worktrees.borrow_mut().push_back(Ok(made_worktree(true)));
            f.ctx.start_answers(Ok(handle("h2", "ws2")));
            ok(fork_start(&f.dyn_ctx, f.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
            let seen = f.ctx.started.borrow();
            assert_eq!(seen[0].1.cwd.as_deref(), Some("/wt/feat"), "source cwd {source_cwd}");
            assert_eq!(seen[0].1.setup, None);
            assert!(f.ctx.called().contains(&"worktreeFolder p1 /repo/p1 main-fork ws1 - refs/c/u1".to_string()), "no caller, no root thread");
        }
    });
}

// TS: fork.test.ts:529
#[test]
fn fork_start_takes_its_worktree_and_branch_away_when_the_start_refuses() {
    local(async {
        let f = fixture();
        f.ctx.worktrees.borrow_mut().push_back(Ok(made_worktree(true)));
        f.ctx.start_answers(Err(err("the session holds no message a1")));
        let got = refused(fork_start(&f.dyn_ctx, f.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
        assert_eq!((got.message.as_str(), got.kind.clone()), ("the session holds no message a1", None), "the start's own error goes on");
        assert_eq!(
            after_worktree(&f),
            vec![
                "worktreeFolder p1 /repo/p1 main-fork ws1 - refs/c/u1".to_string(),
                "takeSetupLine ws2".to_string(),
                "start ws2".to_string(),
                "removeWorktree ws2 true".to_string(),
                "git /repo/p1 branch -D main-fork".to_string(),
            ]
        );
    });
}

#[test]
fn fork_start_leaves_a_worktree_it_did_not_make_when_the_start_refuses() {
    local(async {
        let old = fixture();
        old.ctx.worktrees.borrow_mut().push_back(Ok(made_worktree(true)));
        old.ctx.live.borrow_mut().push("ws2".into());
        old.ctx.start_answers(Err(err("refused")));
        let got = refused(fork_start(&old.dyn_ctx, old.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
        assert_eq!(got.message, "refused");
        assert_eq!(after_worktree(&old).last().map(String::as_str), Some("start ws2"), "a record that was live before the call is not this call's to remove");

        let kept = fixture();
        kept.ctx.worktrees.borrow_mut().push_back(Ok(made_worktree(false)));
        kept.ctx.start_answers(Err(err("refused")));
        refused(fork_start(&kept.dyn_ctx, kept.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
        assert_eq!(after_worktree(&kept).last().map(String::as_str), Some("start ws2"), "a worktree this call did not make is left");
    });
}

#[test]
fn fork_start_warns_when_the_worktree_will_not_go_and_still_deletes_the_branch() {
    local(async {
        let f = fixture();
        f.ctx.worktrees.borrow_mut().push_back(Ok(made_worktree(true)));
        *f.ctx.remove_fails.borrow_mut() = Some("the folder is busy".into());
        f.ctx.git_answers("branch -D", 1);
        f.ctx.start_answers(Err(err("refused")));
        let captured = console::capture();
        let got = refused(fork_start(&f.dyn_ctx, f.ctx.start_fn(), options(Some("main-fork"), fork_of(Some("u1"), None)), None).await);
        assert_eq!(got.message, "refused", "the start's error goes on, whatever the clean-up came to");
        assert_eq!(captured.lines(), vec!["the worktree a refused fork made at /wt/feat was not removed: the folder is busy".to_string()]);
        assert_eq!(after_worktree(&f).last().map(String::as_str), Some("git /repo/p1 branch -D main-fork"), "the branch goes even so");
    });
}

// ---- pay_cut_owed (F12) --------------------------------------------------------------------------------------------

fn owing(f: &F) {
    f.ctx.thread_records.with(|r| *r = records_of(vec![(THREAD, record("ws1", json!({ "harness": "claude", "cutOwed": "a1" })))]));
    f.ctx.sessions.with(|s| {
        *s = sessions_of(vec![
            row(view(json!({ "id": "r1", "threadId": THREAD, "claudeSessionId": "cs1" })), "u1"),
            row(view(json!({ "id": "r2", "threadId": THREAD, "claudeSessionId": "cs1" })), "u2"),
            row(view(json!({ "id": "r3", "threadId": THREAD, "claudeSessionId": "cs0" })), "u3"),
            row(view(json!({ "id": "r4", "threadId": "t2", "claudeSessionId": "cs1" })), "u4"),
        ])
    });
}

fn sessions_now(f: &F) -> Vec<Option<String>> {
    f.ctx.sessions.with(|s| s.values().map(|r| r.view.claude_session_id.clone()).collect())
}

fn cut_owed(f: &F) -> Option<String> {
    f.ctx.thread_records.with(|r| r.get(THREAD).and_then(|h| h.record.cut_owed.clone()))
}

fn persisted(f: &F) -> usize {
    f.ctx.called().iter().filter(|c| *c == "persist ws1").count()
}

// TS: fork.test.ts:340
#[test]
fn pay_cut_owed_moves_the_thread_onto_the_copy_and_clears_the_debt() {
    local(async {
        let f = fixture();
        owing(&f);
        let adapter = FakeForkAdapter::new();
        adapter.answers(vec![ForkAnswer::Resume("cs-copy".into())]);
        let resume = pay_cut_owed(&f.dyn_ctx, adapter.clone(), "ws1", THREAD, "cs1").await;
        assert_eq!(resume, "cs-copy", "the copy's session answers");
        assert_eq!(adapter.asked(), vec![("cs1".to_string(), ForkTurn::Anchor("a1".into()))], "a copy through the owed cut");
        assert_eq!(sessions_now(&f), vec![Some("cs-copy".into()), Some("cs-copy".into()), Some("cs0".into()), Some("cs1".into())], "only the thread's rows on the session move");
        assert_eq!(cut_owed(&f), None);
        settle().await;
        assert_eq!(persisted(&f), 1, "the sessions are persisted, off the caller's wait");
    });
}

// TS: fork.test.ts:367
#[test]
fn pay_cut_owed_warns_and_resumes_uncut_when_the_copy_fails() {
    local(async {
        let f = fixture();
        owing(&f);
        let adapter = FakeForkAdapter::new();
        adapter.answers(vec![ForkAnswer::Refuse("No conversation found with session ID: gone".into())]);
        let captured = console::capture();
        let resume = pay_cut_owed(&f.dyn_ctx, adapter.clone(), "ws1", THREAD, "cs1").await;
        assert_eq!(resume, "cs1", "the resume runs uncut");
        assert_eq!(captured.lines(), vec!["thread th01abcd resumes without its owed cut: No conversation found with session ID: gone".to_string()]);
        assert_eq!(sessions_now(&f), vec![Some("cs1".into()), Some("cs1".into()), Some("cs0".into()), Some("cs1".into())], "nothing moves");
        assert_eq!(cut_owed(&f), None, "a copy refused for any reason owes nothing more");
        settle().await;
        assert_eq!(persisted(&f), 1);
        assert_eq!(adapter.asked().len(), 1);
    });
}

#[test]
fn pay_cut_owed_clears_the_debt_without_a_move_where_the_agent_forks_itself() {
    local(async {
        let f = fixture();
        owing(&f);
        let adapter = FakeForkAdapter::new();
        adapter.answers(vec![ForkAnswer::Fork { session: "cs1".into(), turn: "a1".into() }]);
        let resume = pay_cut_owed(&f.dyn_ctx, adapter, "ws1", THREAD, "cs1").await;
        assert_eq!(resume, "cs1");
        assert_eq!(sessions_now(&f), vec![Some("cs1".into()), Some("cs1".into()), Some("cs0".into()), Some("cs1".into())]);
        assert_eq!(cut_owed(&f), None);
        settle().await;
        assert_eq!(persisted(&f), 1);
    });
}

#[test]
fn pay_cut_owed_changes_nothing_where_nothing_is_owed_or_the_agent_forks_no_session() {
    local(async {
        let f = fixture();
        owing(&f);
        f.ctx.thread_records.with(|r| r.get_mut(THREAD).expect("a record").record.cut_owed = None);
        let adapter = FakeForkAdapter::new();
        assert_eq!(pay_cut_owed(&f.dyn_ctx, adapter.clone(), "ws1", THREAD, "cs1").await, "cs1");
        assert_eq!(pay_cut_owed(&f.dyn_ctx, adapter.clone(), "ws1", "t-no-record", "cs1").await, "cs1", "a thread with no record owes nothing");
        assert_eq!(adapter.asked(), vec![]);

        owing(&f);
        let none = FakeForkAdapter::without_forks();
        assert_eq!(pay_cut_owed(&f.dyn_ctx, none, "ws1", THREAD, "cs1").await, "cs1");
        assert_eq!(cut_owed(&f), Some("a1".to_string()), "an agent that forks no session leaves the debt where it is");
        settle().await;
        assert_eq!(persisted(&f), 0);
        assert_eq!(sessions_now(&f), vec![Some("cs1".into()), Some("cs1".into()), Some("cs0".into()), Some("cs1".into())]);
    });
}
