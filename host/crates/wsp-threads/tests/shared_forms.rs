// SPDX-License-Identifier: AGPL-3.0-only
//! The forms wsp-threads shares with the runtime's context (create_runtime packet 0b): the maps its areas keep and
//! hand out as `Rc<RefCell<..>>`, in the one type `RuntimeCore` types them (context.ts:213, threads.ts:367 and :962,
//! transcripts.ts:615), and one signature for the context calls more than one of its traits asks.
#[path = "support_b/mod.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};
use support::threads_ctx::FakeThreadsCtx;
use support::transcripts_ctx::FakeTranscriptsCtx;
use support::{local, row, view};
use wsp_account::account::core_types::{AccountError, Entry};
use wsp_adapters::port::PermissionAsk;
use wsp_core::context::Joined;
use wsp_threads::activity::ActivityCtx;
use wsp_threads::threads::agents::AgentsCtx;
use wsp_threads::threads::conversations::ConversationsCtx;
use wsp_threads::threads::forks::ForksCtx;
use wsp_threads::threads::sessions::SessionsCtx;
use wsp_threads::threads::threads::ThreadsCtx;
use wsp_threads::threads::turns::TurnsCtx;
use wsp_words::exit::HostError;
use wsp_wire::views::session_events_more::SessionEvent;

fn ask(id: &str) -> PermissionAsk {
    serde_json::from_value(json!({ "askId": id, "toolName": "Bash", "input": "{}", "options": [] })).expect("a permission ask")
}

#[test]
fn the_lead_asks_an_area_keeps_are_the_map_it_hands_out_and_a_thread_that_ends_leaves_it() {
    let ctx = FakeThreadsCtx::new();
    let area = ctx.area();
    let held: Rc<RefCell<IndexMap<String, PermissionAsk>>> = area.lead_asks.clone();
    held.borrow_mut().insert("t1".into(), ask("a1"));
    assert!(Rc::ptr_eq(&held, &area.lead_asks));
    assert_eq!(area.lead_asks.borrow().get("t1").map(|a| a.ask_id.as_str()), Some("a1"));
    ctx.sessions.with(|s| {
        s.insert("r1".into(), row(view(json!({ "id": "r1", "workspaceId": "ws1", "threadId": "t1", "status": "running" })), "u1"));
    });
    area.settle_cut("r1", "cut", &|at| format!("cut at {at}"), false, false);
    assert!(held.borrow().get("t1").is_none(), "the handle the turns area holds sees the end");
}

#[test]
fn the_checkpoints_landing_an_area_keeps_are_the_map_it_hands_out() {
    let ctx = FakeThreadsCtx::new();
    let area = ctx.area();
    let held: Rc<RefCell<IndexMap<String, Joined>>> = area.checkpoints_landing.clone();
    let landing: Joined = futures::FutureExt::shared(futures::FutureExt::boxed_local(async {}));
    held.borrow_mut().insert("t1".into(), landing);
    assert!(Rc::ptr_eq(&held, &area.checkpoints_landing));
    assert!(area.checkpoints_landing.borrow().contains_key("t1"));
}

#[test]
fn viewed_marks_are_the_ts_record_per_thread_and_the_session_document_writes_them_as_node_does() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.add_live(json!({ "id": "ws1" }));
        let area = ctx.area();
        let held: Rc<RefCell<IndexMap<String, IndexMap<String, String>>>> = area.viewed_marks.clone();
        let mut marks = IndexMap::new();
        for (path, blob) in [("b.rs", "x"), ("10", "y"), ("9", "z")] {
            marks.insert(path.to_string(), blob.to_string());
        }
        held.borrow_mut().insert("ws1".into(), marks);
        assert!(Rc::ptr_eq(&held, &area.viewed_marks));
        area.persist_sessions("ws1").await;
        let doc = ctx.store.get_now("sessions", "ws1").expect("the document is put");
        assert_eq!(serde_json::to_string(&doc["viewed"]).expect("serializes"), r#"{"9":"z","10":"y","b.rs":"x"}"#, "a JS object lists integer keys first");
    });
}

#[allow(dead_code)]
fn the_threads_traits_share_one_signature_each_for_the_calls_more_than_one_asks(
    sessions: &dyn SessionsCtx,
    turns: &dyn TurnsCtx,
    agents: &dyn AgentsCtx,
    threads: &dyn ThreadsCtx,
    forks: &dyn ForksCtx,
    conversations: &dyn ConversationsCtx,
    activity: &dyn ActivityCtx,
    entry: &Entry,
) {
    let _: [LocalBoxFuture<'static, ()>; 5] = [
        sessions.persist_sessions("w"),
        turns.persist_sessions("w"),
        agents.persist_sessions("w"),
        threads.persist_sessions("w"),
        forks.persist_sessions("w"),
    ];
    let _: [Option<Joined>; 3] = [sessions.kept_writes("w"), threads.kept_writes("w"), forks.kept_writes("w")];
    let _: [LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>; 4] =
        [sessions.open_thread("w", "t"), turns.open_thread("w", "t"), threads.open_thread("w", "t"), forks.open_thread("w", "t")];
    let _: [LocalBoxFuture<'static, Result<Value, AccountError>>; 3] = [
        threads.with_daemon(entry, Value::Null),
        conversations.with_daemon(entry, Value::Null),
        activity.with_daemon(entry, Value::Null),
    ];
}
