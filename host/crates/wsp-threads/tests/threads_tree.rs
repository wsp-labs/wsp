// SPDX-License-Identifier: AGPL-3.0-only
//! threads_tree (spec/threads_tree.md): rows, trees, tokens and who is told. Expected values read off
//! threads.ts at 65f6b89c7.
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;

use serde_json::json;
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::Lend;
use support::{err, local, record, record_map, row, rows_of, silent, view};
use wsp_threads::threads::threads::tree::{CallsOf, Registered};
use wsp_threads::threads::threads::{computer_silent, ThreadsArea, SILENT_MARKS};
use wsp_wire::views::workspace::Caller;

fn here() -> wsp_wire::views::workspace_more::WorkspaceOrigin {
    serde_json::from_value(json!("here")).expect("an origin")
}

fn scoped(thread: &str, root: &str) -> Caller {
    let by = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "ws1", "rootThreadId": root }))
        .expect("a scope");
    Caller::Scoped { origin: here(), by }
}

fn person() -> Caller {
    Caller::Road(here())
}

#[test]
fn computer_silent_reads_the_class_not_the_words() {
    for mark in SILENT_MARKS {
        assert!(computer_silent(Some(&silent("anything at all", mark))), "{mark}");
    }
    assert!(!computer_silent(Some(&err("the machine did not answer"))), "a plain error with silent words is a refusal");
    assert!(!computer_silent(Some(&silent("x", "SomethingElse"))));
    assert!(!computer_silent(None));
}

#[test]
fn thread_runs_counts_a_running_row_with_no_handle_and_no_row_without_a_thread() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "a", "threadId": "t1", "status": "running" })), "u1"),
            row(view(json!({ "id": "b", "status": "running" })), "u2"),
            row(view(json!({ "id": "c", "threadId": "t2", "status": "completed" })), "u3"),
        ])
    });
    let area = ctx.area();
    assert!(area.thread_runs("t1"));
    assert!(!area.thread_runs("t2"));
    assert!(!area.thread_runs("t3"));
}

#[test]
fn launching_on_and_running_on_answer_the_first_row_in_map_order() {
    let ctx = FakeThreadsCtx::new();
    let launch = futures::FutureExt::shared(futures::FutureExt::boxed_local(async {}));
    ctx.sessions.with(|s| {
        let first = row(view(json!({ "id": "a", "threadId": "t1", "status": "running" })), "u1");
        first.borrow_mut().launch = Some(launch.clone());
        let second = row(view(json!({ "id": "b", "threadId": "t1", "status": "running" })), "u2");
        second.borrow_mut().launch = Some(launch.clone());
        *s = rows_of(vec![first, second]);
    });
    let area = ctx.area();
    assert_eq!(area.launching_on("t1").map(|(turn, _)| turn), Some("u1".to_string()));
    assert_eq!(area.launching_on("t2").map(|(turn, _)| turn), None);
    assert_eq!(area.running_on("t1"), None, "no row of t1 holds a handle");
}

#[test]
fn latest_on_takes_the_newest_start_and_gives_a_tie_to_the_later_row() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "a", "threadId": "t1", "startedAt": 20 })), "u1"),
            row(view(json!({ "id": "b", "threadId": "t1", "startedAt": 30 })), "u2"),
            row(view(json!({ "id": "c", "threadId": "t1", "startedAt": 30 })), "u3"),
            row(view(json!({ "id": "d", "threadId": "t1", "startedAt": 10 })), "u4"),
        ])
    });
    let area = ctx.area();
    assert_eq!(area.latest_on("t1").map(|v| v.id), Some("c".to_string()));
    assert_eq!(area.latest_on("t9"), None);
}

#[test]
fn latest_on_reads_an_absent_start_as_zero() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "a", "threadId": "t1", "startedAt": 0 })), "u1"),
            row(view(json!({ "id": "b", "threadId": "t1" })), "u2"),
        ])
    });
    assert_eq!(ctx.area().latest_on("t1").map(|v| v.id), Some("b".to_string()), "absent reads as 0 and the later row wins the tie");
}

#[test]
fn invariant_3_a_token_no_running_row_or_kept_process_holds_is_refused() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let ended = row(view(json!({ "id": "a", "threadId": "t1", "status": "completed" })), "u1");
        ended.borrow_mut().turn_token = Some("tok-ended".into());
        let threadless = row(view(json!({ "id": "b", "status": "running" })), "u2");
        threadless.borrow_mut().turn_token = Some("tok-none".into());
        let running = row(view(json!({ "id": "c", "threadId": "t3", "status": "running" })), "u3");
        running.borrow_mut().turn_token = Some("tok-live".into());
        *s = rows_of(vec![ended, threadless, running]);
    });
    let area = ctx.area();
    assert_eq!(area.thread_of_token("tok-live").expect("a running row answers"), "t3");
    let no_such = "no turn on this host carries that token; only wsp running inside a turn has one, and a turn that ended has none";
    for token in ["tok-ended", "tok-none", "tok-unknown"] {
        let refused = area.thread_of_token(token).expect_err("refused");
        assert_eq!(refused.message, no_such, "{token}");
        assert_eq!(refused.kind, None, "{token}: no kind, as `new Error`");
    }
}

#[test]
fn thread_of_token_reads_a_kept_process_after_the_rows() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        ctx.thread_records.with(|r| *r = record_map(vec![("t5", record("ws1", json!({})))]));
        let area = ctx.area();
        let (e, _) = entry(json!({}));
        ctx.add_entry(e);
        area.hold_kept("t5", support_hand("ws1", "tok-kept"));
        assert_eq!(area.thread_of_token("tok-kept").expect("a kept process answers"), "t5");
    });
}

fn support_hand(workspace: &str, token: &str) -> wsp_threads::threads::threads::KeptHand {
    wsp_threads::threads::threads::KeptHand {
        workspace_id: workspace.into(),
        agent: Box::new(support::FakeKeptAgent::new(None)),
        pid: None,
        launch: Default::default(),
        session: Some("s1".into()),
        warm: None,
        turn_token: token.into(),
        scope_device_id: None,
        waiting: Default::default(),
    }
}

#[test]
fn tree_under_walks_level_by_level_in_parents_order() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-c", "threadId": "c", "parentThreadId": "b" })), "u1"),
            row(view(json!({ "id": "r-b", "threadId": "b", "parentThreadId": "a" })), "u2"),
            row(view(json!({ "id": "r-d", "threadId": "d", "parentThreadId": "a" })), "u3"),
            row(view(json!({ "id": "r-e", "threadId": "e", "parentThreadId": "c" })), "u4"),
        ])
    });
    let area = ctx.area();
    assert_eq!(area.tree_under("a"), vec!["b", "d", "c", "e"], "level one in parents order (b before d), then c, then e");
    assert_eq!(area.tree_under("c"), vec!["e"]);
    assert_eq!(area.tree_under("e"), Vec::<String>::new());
}

#[test]
fn tree_under_ends_on_a_loop_and_never_holds_itself() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-a", "threadId": "a", "parentThreadId": "b" })), "u1"),
            row(view(json!({ "id": "r-b", "threadId": "b", "parentThreadId": "a" })), "u2"),
        ])
    });
    assert_eq!(ctx.area().tree_under("a"), vec!["b"]);
}

#[test]
fn tree_under_reads_a_record_past_a_row_that_fell_off() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| *s = rows_of(vec![row(view(json!({ "id": "r-b", "threadId": "b", "parentThreadId": "x" })), "u1")]));
    ctx.thread_records.with(|r| {
        *r =
            record_map(vec![("b", record("ws1", json!({ "parentThreadId": "a" }))), ("c", record("ws1", json!({ "parentThreadId": "b" })))])
    });
    let area = ctx.area();
    assert_eq!(area.tree_under("a"), vec!["b", "c"], "the record's parent overwrites the row's");
    assert_eq!(area.tree_under("x"), Vec::<String>::new());
}

#[test]
fn restarts_reads_rows_then_records() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let held = row(view(json!({ "id": "r1", "threadId": "t-new" })), "u1");
        held.borrow_mut().replaces = Some("t-old".into());
        let other = row(view(json!({ "id": "r2", "threadId": "t-row" })), "u2");
        other.borrow_mut().replaces = Some("t-both".into());
        *s = rows_of(vec![held, other]);
    });
    ctx.thread_records.with(|r| *r = record_map(vec![("t-rec", record("ws1", json!({ "replaces": "t-both" })))]));
    let found = ctx.area().restarts();
    assert_eq!(found.get("t-old").map(String::as_str), Some("t-new"));
    assert_eq!(found.get("t-both").map(String::as_str), Some("t-rec"), "a record overwrites a row");
    assert_eq!(found.keys().collect::<Vec<_>>(), vec!["t-old", "t-both"]);
}

#[test]
fn drives_thread_with_no_scope_is_every_thread_and_with_one_its_root() {
    let ctx = FakeThreadsCtx::new();
    ctx.roots.borrow_mut().insert("t1".into(), "root-a".into());
    ctx.roots.borrow_mut().insert("t2".into(), "root-b".into());
    let area = ctx.area();
    assert!(area.drives_thread(Some("t1"), None));
    assert!(area.drives_thread(None, Some(&person())));
    let caller = scoped("t9", "root-a");
    assert!(area.drives_thread(Some("t1"), Some(&caller)));
    assert!(!area.drives_thread(Some("t2"), Some(&caller)));
    assert!(!area.drives_thread(None, Some(&caller)), "a row with no thread is in nobody's tree");
}

#[test]
fn settles_thread_is_the_scope_and_its_tree_never_its_lead() {
    let ctx = FakeThreadsCtx::new();
    ctx.thread_records.with(|r| {
        *r = record_map(vec![
            ("me", record("ws1", json!({ "parentThreadId": "lead" }))),
            ("kid", record("ws1", json!({ "parentThreadId": "me" }))),
            ("sib", record("ws1", json!({ "parentThreadId": "lead" }))),
        ])
    });
    let area = ctx.area();
    let caller = scoped("me", "lead");
    assert!(area.settles_thread("me", Some(&caller)));
    assert!(area.settles_thread("kid", Some(&caller)));
    assert!(!area.settles_thread("lead", Some(&caller)));
    assert!(!area.settles_thread("sib", Some(&caller)));
    assert!(area.settles_thread("lead", None));
}

#[test]
fn thread_title_folds_the_rows_else_the_thread_word() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "b", "threadId": "t1abcdefgh", "startedAt": 20, "prompt": "second", "harnessTitle": "Named", "titleSource": "person" })), "u2"),
            row(view(json!({ "id": "a", "threadId": "t1abcdefgh", "startedAt": 10, "prompt": "first" })), "u1"),
        ])
    });
    let area = ctx.area();
    assert_eq!(area.thread_title("t1abcdefgh"), "Named");
    assert_eq!(area.thread_title("t9zzzzzzzzzz"), "t9zzzzzz");
}

fn ask(id: &str) -> wsp_adapters::port::PermissionAsk {
    serde_json::from_value(json!({ "askId": id, "toolName": "Bash", "input": "{}", "options": [] })).expect("a prompt")
}

#[test]
fn stopped_behind_names_the_question_at_the_bottom_of_the_tree() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        *s = rows_of(vec![
            row(view(json!({ "id": "r-lead", "threadId": "lead", "status": "running" })), "u1"),
            row(
                view(json!({ "id": "r-kid", "threadId": "kid0001", "parentThreadId": "lead", "status": "running", "workspaceId": "ws2" })),
                "u2",
            ),
        ])
    });
    let area = ctx.area();
    area.lead_asks.borrow_mut().insert("kid0001".into(), ask("ask-1"));
    let mut calls = indexmap::IndexMap::new();
    calls.insert(
        "c1".to_string(),
        wsp_core::types::internal::CallSeen {
            tool_name: "mcp__wsp__threads_wait".into(),
            input: json!({ "threads": ["kid0"] }).to_string(),
        },
    );
    let caller = CallsOf { view: view(json!({ "id": "r-lead", "threadId": "lead" })), calls: Some(calls) };
    let behind = area.stopped_behind(&caller).expect("waiting behind the kid's question");
    assert_eq!(behind.thread_id, "kid0001");
    assert_eq!(behind.workspace_id, "ws2");
    assert_eq!(behind.session_id, "r-kid");
    assert_eq!(behind.prompt.ask_id, "ask-1");
}

#[test]
fn stopped_behind_skips_the_caller_and_a_thread_with_no_running_row() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| *s = rows_of(vec![row(view(json!({ "id": "r-kid", "threadId": "kid0001", "status": "completed" })), "u2")]));
    let area = ctx.area();
    area.lead_asks.borrow_mut().insert("kid0001".into(), ask("ask-1"));
    let mut calls = indexmap::IndexMap::new();
    calls.insert(
        "c1".to_string(),
        wsp_core::types::internal::CallSeen {
            tool_name: "mcp__wsp__threads_wait".into(),
            input: json!({ "threads": ["kid0"] }).to_string(),
        },
    );
    assert!(area.stopped_behind(&CallsOf { view: view(json!({ "threadId": "lead" })), calls: Some(calls.clone()) }).is_none());
    assert!(area.stopped_behind(&CallsOf { view: view(json!({})), calls: Some(calls) }).is_none(), "a row with no thread");
    assert!(area.stopped_behind(&CallsOf { view: view(json!({ "threadId": "lead" })), calls: None }).is_none());
}

#[test]
fn stop_under_stops_deepest_first_and_counts_only_accepted() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        ctx.sessions.with(|s| {
            *s = rows_of(vec![
                row(view(json!({ "id": "r-b", "threadId": "b", "parentThreadId": "a", "status": "running" })), "u1"),
                row(view(json!({ "id": "r-c", "threadId": "c", "parentThreadId": "b", "status": "running" })), "u2"),
                row(view(json!({ "id": "r-d", "threadId": "d", "parentThreadId": "a", "status": "completed" })), "u3"),
            ])
        });
        ctx.interrupts.borrow_mut().insert("r-b".into(), Ok("not-running".into()));
        let area = ctx.area();
        let stopped = area.stop_under("a", None).await;
        assert_eq!(stopped, vec!["c"]);
        assert_eq!(ctx.called(), vec!["interrupt r-c", "interrupt r-b"], "deepest first, a settled row asked nothing");
    });
}

#[test]
fn stop_under_warns_and_goes_on_past_a_failed_stop() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        ctx.sessions.with(|s| {
            *s = rows_of(vec![
                row(view(json!({ "id": "r-b", "threadId": "bbbbbbbbbb", "parentThreadId": "a", "status": "running" })), "u1"),
                row(view(json!({ "id": "r-c", "threadId": "c", "parentThreadId": "a", "status": "running" })), "u2"),
            ])
        });
        ctx.interrupts.borrow_mut().insert("r-c".into(), Err(err("gone away")));
        let warned = wsp_core::console::capture();
        let stopped = ctx.area().stop_under("a", None).await;
        assert_eq!(stopped, vec!["bbbbbbbbbb"]);
        assert_eq!(warned.lines(), vec!["thread c was not stopped with its root: gone away"]);
    });
}

#[test]
fn notify_on_answers_the_first_registering_row_with_its_scope_and_road() {
    let ctx = FakeThreadsCtx::new();
    let by: wsp_wire::views::workspace_more::ThreadScope =
        serde_json::from_value(json!({ "kind": "thread", "threadId": "lead", "workspaceId": "ws1", "rootThreadId": "lead" }))
            .expect("a scope");
    ctx.sessions.with(|s| {
        let silent = row(view(json!({ "id": "a", "threadId": "t1" })), "u1");
        let first = row(view(json!({ "id": "b", "threadId": "t1" })), "u2");
        first.borrow_mut().notify = Some(vec!["lead".into(), "me".into()]);
        first.borrow_mut().notify_by = Some(by.clone());
        let later = row(view(json!({ "id": "c", "threadId": "t1" })), "u3");
        later.borrow_mut().notify = Some(vec!["other".into()]);
        *s = rows_of(vec![silent, first, later]);
    });
    let area = ctx.area();
    let on = area.notify_on("t1").expect("registered");
    assert_eq!(on.notify, vec!["lead", "me"]);
    assert_eq!(on.by, Some(by.clone()));
    assert_eq!(on.road, None);
    assert!(area.notify_on("t2").is_none());
    let told = ThreadsArea::tell_as(&Registered { notify_by: Some(by.clone()), notify_road: None, asked: None });
    assert_eq!(told.by, Some(by));
    assert_eq!(told.road, None);
}

#[test]
fn tellable_needs_an_agent_session() {
    assert!(!ThreadsArea::tellable(None));
    assert!(!ThreadsArea::tellable(Some(&view(json!({})))));
    assert!(ThreadsArea::tellable(Some(&view(json!({ "claudeSessionId": "s1" })))));
}

#[test]
fn notify_reach_walks_targets_once_and_leaves_me_out() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let a = row(view(json!({ "id": "ra", "threadId": "a" })), "u1");
        a.borrow_mut().notify = Some(vec!["b".into(), "me".into()]);
        let b = row(view(json!({ "id": "rb", "threadId": "b" })), "u2");
        b.borrow_mut().notify = Some(vec!["a".into(), "c".into()]);
        *s = rows_of(vec![a, b]);
    });
    let reach = ctx.area().notify_reach(&["me".to_string(), "a".to_string(), "c".to_string()]);
    assert_eq!(
        reach.into_iter().collect::<Vec<_>>(),
        vec!["a", "c", "b"],
        "a then c from the start, then b off a; the loop back to a ends"
    );
}

#[test]
fn failure_of_reads_only_the_turn_still_on_the_row() {
    let ctx = FakeThreadsCtx::new();
    ctx.sessions.with(|s| {
        let r = row(view(json!({ "id": "r1", "threadId": "t1" })), "u2");
        r.borrow_mut().turn_live =
            Some(wsp_core::types::internal::TurnLive { failure: Some(err("the link dropped")), ..Default::default() });
        *s = rows_of(vec![r]);
    });
    let area = ctx.area();
    assert_eq!(area.failure_of("r1", "u2").map(|e| e.message), Some("the link dropped".to_string()));
    assert_eq!(area.failure_of("r1", "u1").map(|e| e.message), None, "another turn holds the row now");
    assert_eq!(area.failure_of("r9", "u2").map(|e| e.message), None);
}

#[test]
fn usage_computer_of_is_place_then_here_then_provider_then_kind() {
    let (placed, _) = entry(json!({ "place": "box1", "kind": "local" }));
    assert_eq!(ThreadsArea::usage_computer_of(&placed.borrow().record.borrow()), "box1");
    let (here, _) = entry(json!({ "kind": "local" }));
    assert_eq!(ThreadsArea::usage_computer_of(&here.borrow().record.borrow()), "here");
    let (cloud, _) = entry(json!({ "kind": "cloud", "provider": "solari" }));
    assert_eq!(ThreadsArea::usage_computer_of(&cloud.borrow().record.borrow()), "solari");
    let (bare, _) = entry(json!({ "kind": "cloud" }));
    assert_eq!(ThreadsArea::usage_computer_of(&bare.borrow().record.borrow()), "cloud");
}

#[test]
fn vaulted_for_is_token_before_key_and_nothing_for_an_unknown_agent() {
    let ctx = FakeThreadsCtx::new();
    let area: Rc<ThreadsArea> = ctx.area();
    let both: indexmap::IndexMap<String, String> =
        [("CLAUDE_CODE_OAUTH_TOKEN".to_string(), "t".to_string()), ("ANTHROPIC_API_KEY".to_string(), "k".to_string())]
            .into_iter()
            .collect();
    let key: indexmap::IndexMap<String, String> = [("ANTHROPIC_API_KEY".to_string(), "k".to_string())].into_iter().collect();
    assert_eq!(area.vaulted_for("claude", None, Some(&both)), Some("token"));
    assert_eq!(area.vaulted_for("claude", None, Some(&key)), Some("key"));
    assert_eq!(area.vaulted_for("no-such-agent", None, Some(&both)), None);
    ctx.vault.borrow_mut().insert("ANTHROPIC_API_KEY".into(), "k".into());
    assert_eq!(area.vaulted_for("claude", None, None), Some("key"), "no vault given reads the host's");
}

#[test]
fn limit_details_due_is_false_without_plan_resets_or_a_ledger() {
    local(async {
        let ctx = FakeThreadsCtx::new();
        let (e, _) = entry(json!({}));
        let area = ctx.area();
        assert!(!area.limit_details_due(&e, "claude").await, "claude banks no resets");
        assert!(!area.limit_details_due(&e, "codex").await, "a ledger that fails reads as not due");
    });
}
