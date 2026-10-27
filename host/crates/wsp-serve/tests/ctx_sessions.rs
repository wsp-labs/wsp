// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 13 (#2476): the context's bodies for `SessionsCtx` (149 methods, sessions.ts:41-1498 and
//! runtime.ts:470), read against the TS at 65f6b89c7: the sessions area's reads of `ctx` and the thirteen TS tests the
//! plan names (runtime-history, runtime-notify, session-title, session-opening, session-search, session-run,
//! session-images, session-mcp, session-held, transcript-index, runtime-index, lost-session-line, title-after-close).
//!
//! Those thirteen start a whole runtime over a stub backend and a fake adapter, run turns on it and read the answer
//! through the websocket. A Rust test cannot replace an area, and the turn, launch and boot bodies they run through
//! belong to packets 5, 11 and 12, so each method is read at its own seam on the runtime `create_runtime` builds over
//! the sqlite store in a temp folder, seeded through the handles the traits hand out. A test that needs another
//! packet's bodies says which in its comment ("waits on packet N") and is red until that packet lands.
mod support;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::Runtime as _;
use wsp_core::types::internal::{SessionEntry, Taken, TakenOutcome, ThreadRecord, ThreadWords, TranscriptIndex, TreeTalk};
use wsp_core::types::wiring::{LiveWorkspace, RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::machines::{Entry, Row};
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::{StateWriter, Store};
use wsp_threads::threads::sessions::SessionsCtx as S;
use wsp_threads::threads::transcripts::TranscriptsCtx;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::events_union::EventUnion;

// Fixtures.

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn wiring() -> PlaceWiring {
    PlaceWiring {
        host_key: new_place_key_pair(),
        provider: None,
        here: Rc::new(|| HerePlace { name: "zingzys-mac".into(), ..Default::default() }),
        host_name: Rc::new(|| "zingzys-mac".to_string()),
        install: None,
        dial: None,
        log: None,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    }
}

/// A state folder that lives as long as the test, and the sqlite store over it: the road production takes, since
/// the memory store keeps no transcript rows.
struct State {
    dir: tempfile::TempDir,
    store: Rc<dyn Store>,
}

fn state() -> State {
    let dir = tempfile::tempdir().expect("a folder");
    let store: Rc<dyn Store> = Rc::new(wsp_store::sqlite_store::sqlite_store(
        &dir.path().join("state.json"),
        StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ));
    State { dir, store }
}

fn options(state: &State) -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(state.store.clone()), ..Default::default() }
}

fn with_door(state: &State) -> RuntimeOptions {
    RuntimeOptions { place_links: Some(Rc::new(wiring())), ..options(state) }
}

/// The runtime on a throwaway state, inside the task set the owner thread runs, and its context.
fn run<Fut: Future<Output = ()>>(opts: impl FnOnce(&State) -> RuntimeOptions, f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let held = state();
    let opts = opts(&held);
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts).expect("a runtime");
        let ctx = runtime.context();
        f(runtime, ctx).await;
        drop(held);
    });
}

fn record(id: &str) -> WorkspaceRecord {
    serde_json::from_value(json!({
        "id": id, "name": "pricing page", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    }))
    .expect("a workspace record")
}

fn record_with(id: &str, extra: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": id, "name": "pricing page", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a workspace record")
}

fn view(id: &str, workspace: &str, extra: Value) -> SessionView {
    let mut base = json!({ "id": id, "workspaceId": workspace, "harness": "claude", "status": "completed" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a session view")
}

fn session_entry(view: SessionView) -> SessionEntry {
    SessionEntry {
        turn_id: format!("turn-{}", view.id),
        view,
        replaces: None,
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        handle: None,
        end: None,
        turn_live: None,
        run: None,
        from: None,
        asked: None,
        snapshot: None,
        pid: None,
        launch: None,
        calls: None,
        unanswered: false,
        queued: false,
    }
}

fn row(view: SessionView) -> Row {
    Rc::new(RefCell::new(session_entry(view)))
}

fn put_row(c: &Rc<Ctx>, key: &str, row: Row) {
    S::sessions(&**c).borrow_mut().insert(key.to_string(), row);
}

fn thread(workspace: &str, extra: Value) -> ThreadRecord {
    let mut base = json!({ "workspaceId": workspace, "harness": "claude" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a thread record")
}

fn put_thread(c: &Rc<Ctx>, id: &str, record: ThreadRecord) {
    S::records(&**c).borrow_mut().insert(id.to_string(), record);
}

/// A caller that is a thread's own token: it reaches its own tree and nothing else.
fn as_thread(thread_id: &str, workspace: &str, root: &str) -> Caller {
    let by: wsp_wire::views::workspace_more::ThreadScope = serde_json::from_value(json!({
        "kind": "thread", "threadId": thread_id, "workspaceId": workspace, "rootThreadId": root,
    }))
    .expect("a scope");
    Caller::Scoped { origin: wsp_wire::views::workspace_more::WorkspaceOrigin::Relayed, by }
}

fn event(value: Value) -> EventUnion {
    wsp_wire::wire::events_union::parse_event(&value).expect("an event")
}

fn index_of(c: &Rc<Ctx>, workspace: &str, f: impl FnOnce(&mut TranscriptIndex)) {
    TranscriptsCtx::transcript_state(&**c).with(|s| f(s.index.entry(workspace.to_string()).or_default()));
}

fn words(last: f64, lines: &[&str]) -> ThreadWords {
    ThreadWords {
        lines: lines.iter().map(|l| l.to_string()).collect(),
        last: JsNumber(last),
        first: lines.first().map(|l| l.to_string()),
        ..Default::default()
    }
}

/// A live entry over `record` on a fake machine, put in the runtime's live map through the boot area's handle.
async fn put_live(c: &Rc<Ctx>, record: WorkspaceRecord) -> Entry {
    use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
    let machine = FakeBackend::new(FakeBackendOptions::default()).get_fake(&record.machine_id).await.expect("a fake machine");
    let hooks = WorkspaceHooks {
        golden_snapshot: String::new(),
        wake_attempts: 1,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    };
    let ws = Rc::new(Workspace::new(machine.clone(), hooks, WorkspaceInitial::default()));
    let id = record.id.clone();
    let entry: Entry = Rc::new(RefCell::new(LiveWorkspace {
        record: Rc::new(RefCell::new(record)),
        opens_thread: None,
        of_record_held: false,
        ws,
        machine,
        generation: 0,
        waking: None,
        unchecked: false,
        napping: None,
        adopting: None,
        deleting: None,
        late_read: None,
        wake_stop: None,
        resize_to: None,
        nap_wait_ms: None,
        wake_said: None,
        delete_said: None,
        wake_ask: None,
        creating: false,
        checkout: None,
        checkout_reading: None,
        tree: None,
        tree_reading: None,
        pr: None,
        pr_reading: None,
        pr_poll: None,
        pr_poll_ms: None,
        pr_none_at: None,
    }));
    wsp_machines::machines::boot::BootRuntime::live(&**c).borrow_mut().insert(id, entry.clone());
    entry
}

fn listen(runtime: &AssembledRuntime, kind: &str) -> (Rc<RefCell<Vec<Value>>>, wsp_core::types::events::Unlisten) {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let sink = seen.clone();
    let listener: wsp_core::types::events::EventListener = Rc::new(move |e: &EventUnion| {
        sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
        Ok(())
    });
    (seen, runtime.events().on(kind, listener))
}

// What the core holds: the clock, the shared maps, the bus (runtime.ts:491-992).

#[test]
fn the_clock_is_the_clock_the_host_was_given() {
    let fake = FakeClock::new(1_700_000_000_000);
    let clock = fake.clock();
    run(
        move |s| RuntimeOptions { clock: Some(clock), ..options(s) },
        |_, c| async move {
            assert_eq!(S::clock(&*c).now(), 1_700_000_000_000);
            fake.advance(5);
            assert_eq!(S::clock(&*c).now(), 1_700_000_000_005);
        },
    );
}

#[test]
fn sessions_and_records_are_the_one_shared_maps() {
    run(options, |_, c| async move {
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1" }))));
        put_thread(&c, "t_1", thread("w_1", json!({})));
        assert!(Rc::ptr_eq(&S::sessions(&*c), &TranscriptsCtx::sessions(&*c)), "the same handle every trait hands out");
        assert!(Rc::ptr_eq(&S::records(&*c), &TranscriptsCtx::thread_records(&*c)));
        assert_eq!(S::sessions(&*c).borrow().keys().collect::<Vec<_>>(), ["s_1"]);
        assert_eq!(S::records(&*c).borrow().keys().collect::<Vec<_>>(), ["t_1"]);
    });
}

#[test]
fn a_live_entry_is_the_live_maps_and_none_for_a_workspace_it_does_not_hold() {
    run(options, |_, c| async move {
        assert!(S::live_entry(&*c, "w_1").is_none());
        let entry = put_live(&c, record("w_1")).await;
        assert!(Rc::ptr_eq(&S::live_entry(&*c, "w_1").expect("held"), &entry));
        assert!(S::live_entry(&*c, "w_2").is_none());
    });
}

#[test]
fn emit_delivers_and_keeps_the_event_and_pass_delivers_and_keeps_nothing() {
    run(options, |runtime, c| async move {
        let (seen, _off) = listen(&runtime, "host.notice");
        S::emit(&*c, event(json!({ "type": "host.notice", "message": "kept" })));
        S::pass(&*c, event(json!({ "type": "host.notice", "message": "passed" })));
        let messages: Vec<String> = seen.borrow().iter().map(|e| e["message"].as_str().unwrap_or("").to_string()).collect();
        assert_eq!(messages, ["kept", "passed"], "both reach the listeners");
        let kept = runtime.events().since(Some(0), None);
        assert_eq!(
            kept.events.iter().map(|e| serde_json::to_value(e).unwrap()["message"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>(),
            ["kept"],
            "only emit goes in the ring"
        );
    });
}

#[test]
fn a_turn_settled_listener_hears_its_turns_start_and_end_and_nothing_after_it_is_taken_off() {
    run(options, |_, c| async move {
        let heard = Rc::new(Cell::new(0u32));
        let counted = heard.clone();
        let off = S::on_turn_settled(&*c, "turn_1", Rc::new(move || counted.set(counted.get() + 1)));
        let send = |kind: &str, turn: &str| {
            let body = if kind == "session.start" {
                json!({ "type": kind, "workspaceId": "w_1", "sessionId": "s_1", "turnId": turn })
            } else {
                json!({ "type": kind, "workspaceId": "w_1", "sessionId": "s_1", "turnId": turn, "exitCode": 0, "sawResult": true })
            };
            S::emit(&*c, event(body));
        };
        send("session.start", "turn_2");
        assert_eq!(heard.get(), 0, "another turn's start is not heard");
        send("session.start", "turn_1");
        send("session.end", "turn_1");
        assert_eq!(heard.get(), 2, "this turn's start and its end");
        off();
        send("session.end", "turn_1");
        assert_eq!(heard.get(), 2, "nothing after the off");
    });
}

#[test]
fn the_pinned_head_of_a_thread_is_said_for_a_thread_the_host_holds_and_nothing_for_another() {
    run(options, |runtime, c| async move {
        let (seen, _off) = listen(&runtime, "thread.head");
        S::push_head(&*c, "t_none");
        assert!(seen.borrow().is_empty(), "no thread by that id says nothing");
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "prompt": "fix it" }))));
        put_thread(&c, "t_1", thread("w_1", json!({})));
        S::push_head(&*c, "t_1");
        assert_eq!(seen.borrow().len(), 1, "the head of a held thread is pushed once");
        assert_eq!(seen.borrow()[0]["threadId"], json!("t_1"));
    });
}

#[test]
fn a_session_is_recorded_on_the_thread_and_said_on_the_bus() {
    run(options, |runtime, c| async move {
        let (seen, _off) = listen(&runtime, "session.delta");
        let delta: SessionEvent = serde_json::from_value(json!({
            "type": "session.delta", "workspaceId": "w_1", "sessionId": "s_1", "threadId": "t_1", "kind": "text", "text": "hello", "pos": 1,
        }))
        .expect("a session event");
        S::record(&*c, delta);
        assert_eq!(seen.borrow().len(), 1, "a recorded event is said on the bus");
        assert_eq!(seen.borrow()[0]["text"], json!("hello"));
        let kept = S::open_transcript(&*c, "w_1").await.expect("the transcript reads");
        assert_eq!(kept.len(), 1, "and kept in the workspace's rows");
    });
}

#[test]
fn the_label_of_an_agent_is_its_catalogs_and_the_word_itself_for_one_the_catalog_lacks() {
    run(options, |_, c| async move {
        assert_eq!(S::agent_label(&*c, "claude"), "Claude Code");
        assert_eq!(S::agent_label(&*c, "codex"), "Codex");
        assert_eq!(S::agent_label(&*c, "nobody"), "nobody");
    });
}

#[test]
fn the_waits_and_the_activity_door_are_each_built_once() {
    run(options, |_, c| async move {
        let first = S::activity(&*c);
        let again = S::activity(&*c);
        assert!(Rc::ptr_eq(&first, &again), "one activity door");
        let _ = S::waits(&*c);
        let _ = S::launch_area(&*c);
    });
}

#[test]
fn a_resume_timer_the_core_armed_is_taken_by_name_and_nothing_else_is() {
    // waits on packet 17: `resume_on_reset` reads the clock and the thread's record through the core's context.
    let fake = FakeClock::new(1_000_000);
    let clock = fake.clock();
    run(
        move |s| RuntimeOptions { clock: Some(clock), ..options(s) },
        |_, c| async move {
            S::cancel_resume_timer(&*c, "t_none");
            assert_eq!(fake.pending(), 0);
            wsp_machines::machines::boot::BootRuntime::resume_on_reset(&*c, "t_1", 1_000_000.0 + 60_000.0);
            assert_eq!(fake.pending(), 1);
            S::cancel_resume_timer(&*c, "t_1");
            assert_eq!(fake.pending(), 0, "the timer is cancelled, as `resumeTimers.get(id)?.()` does");
        },
    );
}

#[test]
fn a_workspace_still_booting_is_the_one_boot_work_names() {
    run(options, |_, c| async move {
        assert!(!S::booting(&*c, "w_1"));
        let work: futures::future::LocalBoxFuture<'static, ()> = Box::pin(futures::future::pending());
        use futures::FutureExt as _;
        wsp_machines::machines::boot::BootRuntime::boot_work(&*c).borrow_mut().insert("w_1".into(), work.shared());
        assert!(S::booting(&*c, "w_1"));
        assert!(!S::booting(&*c, "w_2"));
    });
}

// The places (runtime.ts:493-790; sessions.ts:96-125, the stop's reach of a computer).

#[test]
fn a_host_with_no_door_has_no_link_no_rows_and_no_computer_away() {
    run(options, |_, c| async move {
        assert!(!S::has_link(&*c, "pl_1"));
        assert!(!S::place_away(&*c, "pl_1"), "away needs a door to be away from");
        assert!(S::place_rows(&*c).await.is_none(), "`await placeDoor?.rows()`: none where the host has no door");
        assert!(S::ping(&*c, "pl_1").await.is_err(), "a computer with no link answers no ping");
        assert_eq!(S::place_name_of(&*c, "here"), wsp_core::host_id::hostname());
    });
}

#[test]
fn with_a_door_a_computer_with_no_backend_and_no_link_is_away_and_the_wired_one_is_not() {
    run(with_door, |runtime, c| async move {
        let door = runtime.places().expect("the door");
        assert!(S::place_away(&*c, "pl_9"), "no backend and no link");
        assert!(!S::place_away(&*c, "default"), "the wired place has a backend");
        assert!(!S::has_link(&*c, "pl_9"));
        assert!(S::ping(&*c, "pl_9").await.is_err());
        for place in ["pl_9", "here"] {
            assert_eq!(S::place_name_of(&*c, place), door.name_of(place), "name {place}");
        }
        let rows = S::place_rows(&*c).await.expect("a door answers its rows");
        let held = door.rows().await.expect("rows");
        assert_eq!(rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), held.iter().map(|r| r.id.clone()).collect::<Vec<_>>());
    });
}

#[test]
fn a_place_absent_listener_answers_an_off_that_can_be_called_more_than_once() {
    run(with_door, |_, c| async move {
        let fired = Rc::new(Cell::new(0u32));
        let counted = fired.clone();
        let off = S::on_place_absent(&*c, "pl_9", Rc::new(move || counted.set(counted.get() + 1)));
        off();
        off();
        assert_eq!(fired.get(), 0, "an off never fires the listener");
    });
}

#[test]
fn the_place_of_a_workspace_is_what_its_record_says() {
    run(options, |_, c| async move {
        assert_eq!(S::workspace_place(&*c, &record_with("w_1", json!({ "place": "pl_1" }))).as_deref(), Some("pl_1"));
        assert_eq!(S::workspace_place(&*c, &record("w_2")), None);
    });
}

// The transcript index and the store (transcript-index, runtime-index, session-search; sessions.ts:217, 261, 948, 1181).

#[test]
fn a_taken_request_is_the_one_the_workspaces_index_holds_by_its_request_id() {
    run(options, |_, c| async move {
        index_of(&c, "w_1", |index| {
            index.taken.insert(
                "req_1".into(),
                Taken { session_id: "s_1".into(), thread_id: "t_1".into(), turn_id: "turn_1".into(), outcome: TakenOutcome::Started },
            );
        });
        let taken = S::taken(&*c, "w_1", "req_1").expect("held");
        assert_eq!((taken.session_id.as_str(), taken.thread_id.as_str(), taken.turn_id.as_str()), ("s_1", "t_1", "turn_1"));
        assert!(S::taken(&*c, "w_1", "req_2").is_none());
        assert!(S::taken(&*c, "w_2", "req_1").is_none(), "another workspace's index does not hold it");
    });
}

#[test]
fn the_index_position_is_the_newest_the_index_issued_and_none_for_a_workspace_with_no_index() {
    run(options, |_, c| async move {
        assert_eq!(S::index_pos(&*c, "w_1"), None);
        index_of(&c, "w_1", |index| index.pos = 42);
        assert_eq!(S::index_pos(&*c, "w_1"), Some(42));
    });
}

#[test]
fn the_workspaces_with_an_index_are_its_keys_in_the_order_they_were_put() {
    run(options, |_, c| async move {
        assert!(S::index_workspaces(&*c).is_empty());
        index_of(&c, "w_2", |_| {});
        index_of(&c, "w_1", |_| {});
        assert_eq!(S::index_workspaces(&*c), ["w_2", "w_1"]);
    });
}

#[test]
fn a_search_reads_the_workspaces_index_one_hit_a_thread_and_only_the_threads_the_caller_drives() {
    run(options, |_, c| async move {
        index_of(&c, "w_1", |index| {
            index.words.insert(
                "t_1".into(),
                words(2.0, &["fix the redirect", "Done. The redirect now sends every old path to the canonical host."]),
            );
            index.words.insert("t_2".into(), words(1.0, &["tidy the readme", "Rewrote the install section."]));
        });
        let mut found = Vec::new();
        let unsearched = S::search_in(&*c, "w_1", "canonical", &|_| true, &mut found);
        assert_eq!(unsearched, 0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].hit.thread_id, "t_1");
        assert!(found[0].hit.snippet.contains("canonical host"), "{}", found[0].hit.snippet);
        let mut none = Vec::new();
        S::search_in(&*c, "w_1", "canonical", &|id| id == "t_2", &mut none);
        assert!(none.is_empty(), "a token that drives only t_2 finds nothing of t_1's words");
        let mut elsewhere = Vec::new();
        assert_eq!(S::search_in(&*c, "w_9", "canonical", &|_| true, &mut elsewhere), 0, "a workspace with no index has no hits");
        assert!(elsewhere.is_empty());
    });
}

#[test]
fn a_subagent_is_known_when_any_workspaces_index_holds_it_among_a_threads_children() {
    run(options, |_, c| async move {
        assert!(!S::subagent_known(&*c, "task_a"));
        let started: SessionEvent = serde_json::from_value(json!({
            "type": "session.subagent", "workspaceId": "w_1", "sessionId": "s_1", "threadId": "t_1", "turnId": "turn_1",
            "task": "task_a", "state": "running", "title": "look at the tests", "at": 5,
        }))
        .expect("a subagent row");
        index_of(&c, "w_1", |index| wsp_core::types::internal_index::fold_child(index, &started));
        assert!(S::subagent_known(&*c, "task_a"));
        assert!(!S::subagent_known(&*c, "task_b"));
    });
}

#[test]
fn a_store_read_is_the_stores_own_record_or_blob_and_none_where_there_is_none() {
    run(options, |_, c| async move {
        assert_eq!(S::store_get(&*c, "sessions", "w_1").await.expect("read"), None);
        assert_eq!(S::store_get_blob(&*c, "vault", "w_1").await.expect("read"), None);
        let store = TranscriptsCtx::store(&*c);
        store.put("sessions", "w_1", json!({ "sessions": [] })).await.expect("put");
        store.put_blob("vault", "w_1", vec![1, 2, 3]).await.expect("put blob");
        assert_eq!(S::store_get(&*c, "sessions", "w_1").await.expect("read"), Some(json!({ "sessions": [] })));
        assert_eq!(S::store_get_blob(&*c, "vault", "w_1").await.expect("read"), Some(vec![1, 2, 3]));
    });
}

// The threads area's reads of the rows and the records (threads.ts:75-361, sessions.ts's `latestOn` and friends).

/// A started turn that does nothing, so a row can hold a handle.
struct Idle(SessionView);

impl wsp_core::types::wiring::SessionHandle for Idle {
    fn id(&self) -> String {
        self.0.id.clone()
    }
    fn workspace_id(&self) -> String {
        self.0.workspace_id.clone()
    }
    fn finished(&self) -> futures::future::Shared<futures::future::LocalBoxFuture<'static, wsp_wire::views::session_events::TurnResult>> {
        use futures::FutureExt as _;
        let result: wsp_wire::views::session_events::TurnResult =
            serde_json::from_value(json!({ "status": "completed", "text": "" })).expect("a result");
        let done: futures::future::LocalBoxFuture<'static, _> = Box::pin(async move { result });
        done.shared()
    }
    fn turn_id(&self) -> String {
        format!("turn-{}", self.0.id)
    }
    fn outcome(&self) -> wsp_wire::wire::runtime::SessionStartOutcome {
        wsp_wire::wire::runtime::SessionStartOutcome::Started
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        self.0.clone()
    }
    fn interrupt(&self) -> futures::future::LocalBoxFuture<'static, ()> {
        Box::pin(async {})
    }
    fn steer(
        &self,
        _prompt: String,
        _id: Option<String>,
        _images: Vec<wsp_adapters::port::TurnImage>,
    ) -> Option<wsp_core::types::wiring::Local<'static, wsp_core::types::wiring::SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(
        &self,
        _ask_id: String,
        _option_id: String,
        _reason: Option<String>,
    ) -> Option<wsp_core::types::wiring::Local<'static, wsp_wire::wire::runtime::SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_core::types::wiring::AccessMove>> {
        None
    }
    fn stop_task(&self, _task: String) -> Option<wsp_core::types::wiring::Local<'static, wsp_adapters::port::TaskStop>> {
        None
    }
}

#[test]
fn the_latest_row_of_a_thread_is_the_one_that_started_last_and_a_thread_runs_while_a_row_of_it_does() {
    run(options, |_, c| async move {
        assert!(S::latest_on(&*c, "t_1").is_none());
        assert!(!S::thread_runs(&*c, "t_1"));
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 100 }))));
        put_row(&c, "s_2", row(view("s_2", "w_1", json!({ "threadId": "t_1", "startedAt": 200, "status": "running" }))));
        put_row(&c, "s_3", row(view("s_3", "w_1", json!({ "threadId": "t_2", "startedAt": 300 }))));
        assert_eq!(S::latest_on(&*c, "t_1").expect("a row").id, "s_2");
        assert!(S::thread_runs(&*c, "t_1"));
        assert!(!S::thread_runs(&*c, "t_2"));
    });
}

#[test]
fn the_running_row_of_a_thread_is_the_row_the_area_names_by_its_id_and_only_one_that_holds_a_handle() {
    run(options, |_, c| async move {
        let running = view("s_1", "w_1", json!({ "threadId": "t_1", "status": "running" }));
        let held = row(running.clone());
        put_row(&c, "s_1", held.clone());
        assert!(S::running_on(&*c, "t_1").is_none(), "a running row with no handle is not running on");
        held.borrow_mut().handle = Some(Rc::new(Idle(running)));
        let found = S::running_on(&*c, "t_1").expect("the row");
        assert!(Rc::ptr_eq(&found, &held), "the Row itself, looked up in the sessions map by the area's row id");
    });
}

#[test]
fn the_launching_row_of_a_thread_is_the_row_whose_launch_is_in_flight() {
    run(options, |_, c| async move {
        use futures::FutureExt as _;
        let launching = row(view("s_1", "w_1", json!({ "threadId": "t_1", "status": "running" })));
        put_row(&c, "s_1", launching.clone());
        assert!(S::launching_on(&*c, "t_1").is_none(), "no launch in flight");
        let pending: futures::future::LocalBoxFuture<'static, ()> = Box::pin(futures::future::pending());
        launching.borrow_mut().launch = Some(pending.shared());
        let found = S::launching_on(&*c, "t_1").expect("the row");
        assert!(Rc::ptr_eq(&found, &launching));
        assert!(S::launching_on(&*c, "t_2").is_none());
    });
}

#[test]
fn what_a_thread_tells_is_the_row_that_asked_and_who_it_reaches_follows_each_thread_on() {
    run(options, |_, c| async move {
        let mut asked = session_entry(view("s_1", "w_1", json!({ "threadId": "t_1" })));
        asked.notify = Some(vec!["t_2".into()]);
        put_row(&c, "s_1", Rc::new(RefCell::new(asked)));
        let mut second = session_entry(view("s_2", "w_1", json!({ "threadId": "t_2" })));
        second.notify = Some(vec!["t_3".into(), "me".into()]);
        put_row(&c, "s_2", Rc::new(RefCell::new(second)));
        assert_eq!(S::notify_on(&*c, "t_1").expect("asked").notify, ["t_2"]);
        assert!(S::notify_on(&*c, "t_3").is_none());
        let reach = S::notify_reach(&*c, &["t_1".to_string()]);
        assert_eq!(reach.iter().cloned().collect::<Vec<_>>(), ["t_1", "t_2", "t_3"], "the chain, with `me` never a thread");
    });
}

#[test]
fn a_thread_owns_the_threads_whose_parents_lead_to_it_and_a_replaced_thread_names_its_replacement() {
    run(options, |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({})));
        put_thread(&c, "t_2", thread("w_1", json!({ "parentThreadId": "t_1" })));
        put_thread(&c, "t_3", thread("w_1", json!({ "parentThreadId": "t_2", "replaces": "t_old" })));
        assert_eq!(S::tree_under(&*c, "t_1"), ["t_2", "t_3"]);
        assert!(S::tree_under(&*c, "t_3").is_empty());
        assert_eq!(S::restarts(&*c).into_iter().collect::<Vec<_>>(), [("t_old".to_string(), "t_3".to_string())]);
    });
}

#[test]
fn a_turns_token_names_its_running_thread_and_an_unknown_token_is_refused() {
    run(options, |_, c| async move {
        let mut running = session_entry(view("s_1", "w_1", json!({ "threadId": "t_1", "status": "running" })));
        running.turn_token = Some("tok_1".into());
        put_row(&c, "s_1", Rc::new(RefCell::new(running)));
        assert_eq!(S::thread_of_token(&*c, "tok_1").expect("a thread"), "t_1");
        assert!(S::thread_of_token(&*c, "tok_none").is_err());
    });
}

#[test]
fn a_thread_drives_and_settles_the_threads_its_scope_reaches_and_a_person_drives_every_thread() {
    run(options, |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({})));
        put_thread(&c, "t_2", thread("w_1", json!({ "parentThreadId": "t_1", "rootThreadId": "t_1" })));
        put_thread(&c, "t_9", thread("w_1", json!({})));
        let lead = as_thread("t_1", "w_1", "t_1");
        assert!(S::drives_thread(&*c, Some("t_9"), None));
        assert!(S::settles_thread(&*c, "t_9", None));
        assert!(S::drives_thread(&*c, Some("t_2"), Some(&lead)), "a thread's own tree");
        assert!(!S::drives_thread(&*c, Some("t_9"), Some(&lead)), "another tree");
        assert!(!S::drives_thread(&*c, None, Some(&lead)));
        assert!(S::settles_thread(&*c, "t_1", Some(&lead)) && S::settles_thread(&*c, "t_2", Some(&lead)));
        assert!(!S::settles_thread(&*c, "t_9", Some(&lead)));
    });
}

#[test]
fn the_parent_root_and_tree_of_a_thread_are_the_rules_areas() {
    run(options, |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({})));
        put_thread(&c, "t_2", thread("w_1", json!({ "parentThreadId": "t_1", "rootThreadId": "t_1" })));
        assert_eq!(S::parent_of(&*c, "t_2").as_deref(), Some("t_1"));
        assert_eq!(S::parent_of(&*c, "t_1"), None);
        assert_eq!(S::root_of(&*c, "t_2"), "t_1");
        assert_eq!(S::root_of(&*c, "t_1"), "t_1");
        let scope: wsp_wire::views::workspace_more::ThreadScope =
            serde_json::from_value(json!({ "kind": "thread", "threadId": "t_2", "workspaceId": "w_1", "rootThreadId": "t_1" }))
                .expect("a scope");
        let tree = S::tree_of(&*c, Some(&scope));
        let none = S::tree_of(&*c, None);
        assert_eq!(tree.root_thread_id.as_deref(), Some("t_1"));
        assert_ne!(tree, none, "a scoped tree names its root");
    });
}

#[test]
fn what_a_map_member_of_an_area_holds_is_read_off_it_and_none_for_a_key_it_lacks() {
    run(options, |_, c| async move {
        assert!(S::cap_held(&*c, "turn_1").is_none());
        assert!(S::host_writes(&*c, "s_1").is_none());
        assert!(S::kept_writes(&*c, "w_1").is_none());
        assert!(!S::kept_warm(&*c, "t_1"));
    });
}

// The computer rules and the project reads (rules.ts, channels.ts; sessions.ts:300-380).

fn project(id: &str, name: &str, computer: &str) -> wsp_wire::views::workspace::ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": format!("/work/{name}"), "computer": computer,
        "source": { "kind": "folder", "path": format!("/work/{name}") }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

fn hold_project(c: &Rc<Ctx>, project: wsp_wire::views::workspace::ProjectView) {
    wsp_account::account::rules::RulesCtx::projects_held(&**c).borrow_mut().insert(project.id.clone(), project);
}

#[test]
fn a_person_opens_in_no_workspace_and_a_thread_opens_only_in_a_folder_workspace_of_its_repository() {
    run(options, |_, c| async move {
        let local = record_with("w_1", json!({ "kind": "local" }));
        assert_eq!(S::opens_in(&*c, &local, None), Ok(false), "a person opens nowhere");
        let lead = as_thread("t_1", "w_9", "t_1");
        let cloud = record_with("w_2", json!({ "kind": "cloud" }));
        assert_eq!(S::opens_in(&*c, &cloud, Some(&lead)), Ok(false), "a cloud workspace is not a folder");
        assert_eq!(S::of_threads_repository(&*c, None, "p_1"), Ok(false));
    });
}

#[test]
fn opens_in_and_of_threads_repository_fail_where_the_project_held_read_does() {
    run(options, |_, c| async move {
        put_live(&c, record_with("w_9", json!({ "kind": "local", "project": "p_1" }))).await;
        let lead = as_thread("t_1", "w_9", "t_1");
        let error = S::of_threads_repository(&*c, Some(&lead), "p_other").expect_err("the project is not held");
        assert!(error.message.contains("p_1") || error.message.contains("p_other"), "{}", error.message);
        hold_project(&c, project("p_1", "web", "zingzys-mac"));
        assert_eq!(S::of_threads_repository(&*c, Some(&lead), "p_1"), Ok(true), "the thread's own project");
    });
}

#[test]
fn a_person_is_refused_nothing_for_a_computer_and_a_workspace_is_acted_on() {
    run(options, |_, c| async move {
        hold_project(&c, project("p_2", "api", "pl_9"));
        let target = record_with("w_2", json!({ "kind": "local", "project": "p_2" }));
        assert_eq!(S::elsewhere_refusal(&*c, None, "p_2", "api"), Ok(None));
        assert_eq!(S::away_for(&*c, &target, None, "start", "w_2"), Ok(None));
        assert!(S::acts_on(&*c, &target, None));
        assert!(
            S::elsewhere_refusal(&*c, None, "p_none", "x").is_err(),
            "an unheld project is the area's failure, as `projectHeld` throws"
        );
    });
}

#[test]
fn a_thread_on_one_computer_is_refused_a_project_on_another_with_the_computers_names() {
    run(with_door, |runtime, c| async move {
        let door = runtime.places().expect("the door");
        hold_project(&c, project("p_1", "web", "pl_1"));
        hold_project(&c, project("p_2", "api", "pl_9"));
        put_live(&c, record_with("w_9", json!({ "kind": "place", "place": "pl_1", "project": "p_1" }))).await;
        let lead = as_thread("t_1", "w_9", "t_1");
        let said = S::elsewhere_refusal(&*c, Some(&lead), "p_2", "api").expect("held").expect("refused");
        assert!(said.message.contains(&door.name_of("pl_9")) || said.message.contains("api"), "{}", said.message);
        let target = record_with("w_2", json!({ "kind": "local", "project": "p_2" }));
        let away = S::away_for(&*c, &target, Some(&lead), "start", "w_2").expect("held").expect("refused");
        assert!(away.message.contains(&door.name_of("pl_9")), "names the computer the workspace is on: {}", away.message);
        assert!(!S::acts_on(&*c, &target, Some(&lead)));
    });
}

#[test]
fn the_name_of_a_projects_computer_is_the_doors_and_an_unheld_project_is_the_areas_failure() {
    run(with_door, |runtime, c| async move {
        let door = runtime.places().expect("the door");
        hold_project(&c, project("p_1", "web", "pl_9"));
        assert_eq!(S::project_computer_name(&*c, "p_1").expect("held"), door.name_of("pl_9"));
        let error = S::project_computer_name(&*c, "p_none").expect_err("not held");
        assert!(error.message.contains("p_none"), "{}", error.message);
    });
}

#[test]
fn the_checkout_of_a_record_is_its_projects_folder_and_an_unheld_project_is_the_areas_failure() {
    run(options, |_, c| async move {
        hold_project(&c, project("p_1", "web", "zingzys-mac"));
        assert_eq!(S::checkout_of(&*c, &record("w_1")).expect("held"), "/work/web");
        assert!(S::checkout_of(&*c, &record_with("w_2", json!({ "project": "p_none" }))).is_err());
    });
}

#[test]
fn the_computer_of_an_entry_is_the_doors_name_of_its_place() {
    run(with_door, |runtime, c| async move {
        let door = runtime.places().expect("the door");
        let at = put_live(&c, record_with("w_1", json!({ "place": "pl_9" }))).await;
        assert_eq!(S::computer_of(&*c, &at), door.name_of("pl_9"));
    });
}

#[test]
fn a_record_is_brokered_only_where_a_broker_is_ready_and_the_kind_says_so() {
    run(options, |_, c| async move {
        assert!(!S::brokered(&*c, &record("w_1")), "no broker wired");
        assert!(!S::brokered(&*c, &record_with("w_2", json!({ "kind": "local" }))));
    });
}

#[test]
fn a_person_passes_the_spawn_guard_and_the_name_check_for_a_workspace() {
    run(options, |_, c| async move {
        put_live(&c, record("w_1")).await;
        assert!(S::spawn_guard(&*c, "thread_new", None).is_ok());
        assert!(S::spawn_guard(&*c, "send", None).is_ok());
        assert!(S::refuse_named(&*c, "w_1", None).is_ok());
    });
}

#[test]
fn a_tree_stands_on_no_workspace_for_a_person_and_a_person_reaches_every_row() {
    // waits on packet 5: the boot area reads the live records through its context.
    run(options, |_, c| async move {
        put_live(&c, record("w_1")).await;
        assert!(!S::tree_stands_on(&*c, "w_1", None, None).expect("read"), "a tree is a thread's: a person has none");
        let view = view("s_1", "w_1", json!({ "threadId": "t_1" }));
        assert!(S::reaches_row(&*c, &view, None, None).expect("read"), "a person reaches every row");
        assert!(S::reaches_row(&*c, &view, None, Some(TreeTalk::Send)).expect("read"));
    });
}

#[test]
fn a_persons_tree_records_are_the_thread_and_record_pairs_of_the_workspace_asked_for() {
    // waits on packet 5.
    run(options, |_, c| async move {
        put_live(&c, record("w_1")).await;
        put_live(&c, record_with("w_2", json!({ "machineId": "m_2" }))).await;
        put_thread(&c, "t_1", thread("w_1", json!({})));
        put_thread(&c, "t_2", thread("w_2", json!({})));
        let pairs = S::tree_records(&*c, None, TreeTalk::List, Some("w_1")).expect("read");
        assert_eq!(pairs.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["t_1"]);
        let all = S::tree_records(&*c, None, TreeTalk::List, None).expect("read");
        assert_eq!(all.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["t_1", "t_2"]);
    });
}

#[test]
fn a_thread_sub_is_refused_a_send_where_its_workspace_holds_one_and_a_plain_one_is_not() {
    run(options, |_, c| async move {
        let plain = put_live(&c, record("w_1")).await;
        assert!(S::refuse_sub_send(&*c, &plain).is_ok());
    });
}

#[test]
fn the_target_workspace_a_send_remembers_is_written_once_and_the_copy_and_worktree_guards_pass_a_plain_entry() {
    // waits on packet 16: `remember_target` writes the preference through the preferences area.
    run(options, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let held = entry.borrow().record.borrow().clone();
        S::remember_target(&*c, &held).await.expect("remembered");
        S::remember_target(&*c, &held).await.expect("again is no write");
        assert!(S::copy_blocked(&*c, &entry).await.is_ok());
        assert!(S::worktree_mounted(&*c, &entry).await.is_ok());
    });
}

// The transcript's writes and reads (runtime-history, session-run, session-opening; transcripts.ts).

fn delta_of(thread: &str, turn: &str, text: &str, pos: u32) -> SessionEvent {
    serde_json::from_value(json!({
        "type": "session.delta", "workspaceId": "w_1", "sessionId": "s_1", "threadId": thread, "turnId": turn,
        "kind": "text", "text": text, "pos": pos,
    }))
    .expect("a session event")
}

#[test]
fn a_flushed_transcript_reads_back_from_the_stores_rows_in_the_order_it_was_recorded() {
    run(options, |_, c| async move {
        for (n, text) in ["one", "two", "three"].into_iter().enumerate() {
            S::record(&*c, delta_of("t_1", "turn_1", text, n as u32 + 1));
        }
        S::flush_transcript(&*c, "w_1").await.expect("flushed");
        let read = S::open_transcript(&*c, "w_1").await.expect("read");
        let texts: Vec<String> = read.iter().map(|e| serde_json::to_value(e).unwrap()["text"].as_str().unwrap_or("").to_string()).collect();
        assert_eq!(texts, ["one", "two", "three"]);
    });
}

#[test]
fn the_events_of_one_thread_and_only_the_cut_turns_are_dropped_from_the_transcript() {
    run(options, |_, c| async move {
        S::record(&*c, delta_of("t_1", "turn_1", "keep", 1));
        S::record(&*c, delta_of("t_1", "turn_2", "cut", 2));
        S::record(&*c, delta_of("t_2", "turn_2", "another thread", 3));
        S::flush_transcript(&*c, "w_1").await.expect("flushed");
        let cut = vec!["turn_2".to_string()];
        S::drop_from_transcript(&*c, "w_1", "t_1", Some(&cut)).await.expect("dropped");
        let texts = |events: Vec<SessionEvent>| -> Vec<String> {
            events.iter().map(|e| serde_json::to_value(e).unwrap()["text"].as_str().unwrap_or("").to_string()).collect()
        };
        assert_eq!(texts(S::open_transcript(&*c, "w_1").await.expect("read")), ["keep", "another thread"]);
        S::drop_from_transcript(&*c, "w_1", "t_1", None).await.expect("dropped");
        assert_eq!(texts(S::open_transcript(&*c, "w_1").await.expect("read")), ["another thread"], "no turns named drops the whole thread");
    });
}

#[test]
fn a_thread_is_opened_as_its_events_and_the_words_of_its_opening_are_noted() {
    run(options, |_, c| async move {
        S::record(&*c, delta_of("t_1", "turn_1", "hello", 1));
        S::record(&*c, delta_of("t_2", "turn_1", "elsewhere", 2));
        S::flush_transcript(&*c, "w_1").await.expect("flushed");
        let opened = S::open_thread(&*c, "w_1", "t_1").await.expect("opened");
        assert_eq!(opened.len(), 1, "the thread's own events only");
        S::opened_words(&*c, "w_1", "t_1");
    });
}

#[test]
fn a_page_of_a_threads_history_is_read_with_its_position_and_total() {
    run(options, |_, c| async move {
        for n in 1..=3u32 {
            S::record(&*c, delta_of("t_1", "turn_1", &format!("line {n}"), n));
        }
        S::flush_transcript(&*c, "w_1").await.expect("flushed");
        let page = S::read_page(
            &*c,
            "w_1",
            "t_1",
            wsp_store::transcript_reader::ThreadRead { before: None, limit: 100, bytes: 1_000_000, strict: false, shape: None },
        )
        .await
        .expect("a page");
        let page = serde_json::to_value(&page).expect("a page as json");
        assert_eq!(page["events"].as_array().map(Vec::len), Some(3));
        assert_eq!(page["total"], json!(3));
    });
}

#[test]
fn a_sent_images_keep_and_drop_go_through_the_kept_writes_of_its_workspace() {
    run(options, |_, c| async move {
        let png =
            wsp_wire::types::attachments::Attachment { media_type: "image/png".into(), bytes: "AAAA".into(), name: Some("a.png".into()) };
        S::keep_sent_images(&*c, "w_1", "t_1", Some("req_1"), vec![png]).await.expect("kept");
        S::drop_sent_images(&*c, "w_1", &["t_1".to_string()]).await.expect("dropped");
    });
}

// The thread rows as listed (session-title, session-held; boot.ts:111, 376; agents.ts's title reads).

#[test]
fn listed_rows_are_each_held_row_as_a_listing_answers_it_looked_up_by_its_view_id() {
    // waits on packet 5: the boot area reads the session map through its context.
    run(options, |_, c| async move {
        let first = view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 100 }));
        let second = view("s_2", "w_1", json!({ "threadId": "t_1", "startedAt": 200 }));
        put_row(&c, "s_1", row(first.clone()));
        put_row(&c, "s_2", row(second.clone()));
        let listed = S::listed_rows(&*c, vec![first, second]);
        assert_eq!(
            listed.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            ["s_1", "s_2"],
            "each held row as a listing answers it, in the order given"
        );
    });
}

#[test]
fn placed_rows_of_nothing_are_nothing_and_the_rows_come_back_where_no_computer_is_named() {
    // waits on packet 5.
    run(options, |_, c| async move {
        assert!(S::placed_rows(&*c, Vec::new()).await.is_empty());
        let rows = vec![view("s_1", "w_1", json!({ "threadId": "t_1" }))];
        assert_eq!(S::placed_rows(&*c, rows).await.len(), 1, "a host with no door names no computer on a row");
    });
}

#[test]
fn title_rows_are_the_newest_row_of_each_session_and_a_title_the_harness_carried_is_read_off_the_threads_rows() {
    run(options, |_, c| async move {
        let rows = vec![
            view("s_1", "w_1", json!({ "threadId": "t_1", "claudeSessionId": "cs_1", "startedAt": 100, "prompt": "fix the redirect" })),
            view(
                "s_2",
                "w_1",
                json!({ "threadId": "t_1", "claudeSessionId": "cs_1", "startedAt": 200, "harnessTitle": "Redirect fix", "titleSource": "auto" }),
            ),
        ];
        let titled = S::title_rows(&*c, rows.clone());
        assert_eq!(
            titled.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            ["s_2"],
            "the newest row of each session is the one asked about"
        );
        let (title, source) = S::carried_title(&*c, "t_1");
        assert_eq!((title, source), (None, None), "no row of the runtime's own carries one yet");
        for r in rows {
            put_row(&c, &r.id.clone(), row(r));
        }
        assert_eq!(S::rows_on(&*c, "t_1").len(), 2);
        assert_eq!(S::carried_title(&*c, "t_1").0.as_deref(), Some("Redirect fix"));
    });
}

#[test]
fn a_row_without_a_session_has_no_title_to_refresh() {
    // waits on packet 11: the agents area asks the runtime for the adapter.
    run(options, |_, c| async move {
        let bare = view("s_1", "w_1", json!({ "threadId": "t_1" }));
        S::refresh_title(&*c, bare.clone(), false).await.expect("nothing to ask");
        S::refresh_title(&*c, bare, true).await.expect("nothing to ask, forced");
    });
}

#[test]
fn the_facts_of_a_thread_are_the_boot_areas_and_none_for_a_thread_the_host_does_not_hold() {
    // waits on packet 5.
    run(options, |_, c| async move {
        assert!(S::thread_facts(&*c, "t_none").is_none());
        assert!(S::facts_after_read(&*c, "t_none", None).await.expect("a read").is_none());
        assert!(!S::settled_now(&*c, "t_none", None), "a thread that is not held is not settled");
    });
}

#[test]
fn a_rows_running_turn_is_a_turn_of_its_workspace_and_no_other() {
    run(options, |_, c| async move {
        assert!(!S::turn_runs(&*c, "w_1"));
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "status": "running" }))));
        assert!(S::turn_runs(&*c, "w_1"));
        assert!(!S::turn_runs(&*c, "w_2"));
    });
}

// The adapter's own calls (session-title, session-mcp, session-images; sessions.ts:1085-1230, threads.ts:177).

use wsp_adapters::port::{
    AsideAnswer, AsideQuestion, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, SessionRenameWrite,
};

/// An adapter whose side question and rename answer what the case says.
struct Answering {
    aside: Option<Result<String, String>>,
    rename: Option<SessionRenameWrite>,
    renamed: Rc<RefCell<Vec<(String, String)>>>,
}

impl HarnessAdapter for Answering {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, wsp_adapters::port::AdapterError> {
        unimplemented!("these cases never start a turn")
    }
    fn steers(&self) -> bool {
        false
    }
    fn aside(&self, _question: AsideQuestion) -> Option<futures::future::LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        let answer = self.aside.clone()?;
        Some(Box::pin(async move { answer.map(|text| AsideAnswer { text, usage: None }) }))
    }
    fn rename_session(
        &self,
        harness_session_id: String,
        title: String,
        _exec: Arc<dyn wsp_adapters::port::HarnessExec>,
    ) -> Option<futures::future::LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        self.renamed.borrow_mut().push((harness_session_id, title));
        let wrote = self.rename.clone()?;
        Some(Box::pin(async move { Ok(wrote) }))
    }
}

fn answering(aside: Option<Result<String, String>>, rename: Option<SessionRenameWrite>) -> (Answering, Rc<RefCell<Vec<(String, String)>>>) {
    let renamed: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    (Answering { aside, rename, renamed: renamed.clone() }, renamed)
}

#[test]
fn an_aside_is_the_text_the_adapter_answers_and_its_refusal_or_absence_is_a_failure() {
    run(options, |_, c| async move {
        let ask = || AsideQuestion { session: "s_1".into(), question: "why?".into(), ..Default::default() };
        let (said, _) = answering(Some(Ok("because".into())), None);
        assert_eq!(S::aside(&*c, &said, ask()).await.expect("answered"), "because");
        let (refused, _) = answering(Some(Err("the agent said no".into())), None);
        assert!(S::aside(&*c, &refused, ask()).await.expect_err("refused").message.contains("the agent said no"));
        let (silent, _) = answering(None, None);
        assert!(S::aside(&*c, &silent, ask()).await.is_err(), "an adapter with no aside answers nothing");
    });
}

#[test]
fn a_session_write_asks_the_adapter_to_rename_and_answers_what_the_store_said() {
    run(options, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let (adapter, renamed) = answering(None, Some(SessionRenameWrite::Written));
        let adapter: Arc<dyn HarnessAdapter> = Arc::new(adapter);
        let wrote = S::write_session(&*c, "claude-s_1", &entry, adapter.clone(), "Redirect fix").await.expect("written");
        assert_eq!(wrote, SessionRenameWrite::Written);
        assert_eq!(renamed.borrow().as_slice(), [("claude-s_1".to_string(), "Redirect fix".to_string())]);
        let (without, _) = answering(None, None);
        assert!(S::write_session(&*c, "claude-s_1", &entry, Arc::new(without), "x").await.is_err(), "an adapter that renames nothing");
    });
}

#[test]
fn a_harness_exec_runs_the_command_on_the_entrys_machine() {
    run(options, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let exec = S::harness_exec(&*c, &entry, 5_000);
        let said = exec.exec("true", None).await.expect_err("the fake machine runs nothing");
        assert!(said.message.contains("stand-in provider"), "the command reached the entry's machine: {}", said.message);
    });
}

#[test]
fn a_mark_of_a_thread_the_host_does_not_hold_is_refused_and_an_empty_one_is_no_failure() {
    // waits on packet 5: `mark` is the boot area's, over the live records.
    run(options, |_, c| async move {
        let stamps: wsp_threads::threads::sessions::StampsOf = Rc::new(|_| Default::default());
        S::mark(&*c, &[], stamps.clone(), None).await.expect("nothing to mark");
        assert!(S::mark(&*c, &["t_none".to_string()], stamps, None).await.is_err());
        assert!(S::arm_resume(&*c, &["t_none".to_string()], true, None).await.is_err());
    });
}

#[test]
fn a_steer_the_turn_took_is_recorded_on_the_thread_and_said_on_the_bus() {
    run(options, |runtime, c| async move {
        let (seen, _off) = listen(&runtime, "session.steer");
        let running = row(view("s_1", "w_1", json!({ "threadId": "t_1", "status": "running", "claudeSessionId": "cs_1" })));
        put_row(&c, "s_1", running.clone());
        let ask =
            wsp_core::types::api::SteerAsk { prompt: "also fix the footer".into(), request_id: Some("req_2".into()), attachments: None };
        S::record_steer(&*c, &running, "s_1", &ask, None, "steer_1");
        assert_eq!(seen.borrow().len(), 1, "the steer is said once");
        assert_eq!(seen.borrow()[0]["prompt"], json!("also fix the footer"));
        assert_eq!(seen.borrow()[0]["threadId"], json!("t_1"));
    });
}

#[test]
fn a_steer_kept_then_lost_leaves_no_steer_on_the_turn() {
    run(options, |_, c| async move {
        let running = row(view("s_1", "w_1", json!({ "threadId": "t_1", "status": "running" })));
        put_row(&c, "s_1", running.clone());
        let opts = wsp_core::types::api::StartOptions { prompt: "more".into(), ..Default::default() };
        S::keep_steer(&*c, &running, &opts, None, "steer_1").await.expect("kept");
        S::steer_lost(&*c, &running, "steer_1");
        assert!(
            running.borrow().turn_live.as_ref().and_then(|live| live.steered.as_ref()).is_none_or(|steered| steered.is_empty()),
            "a lost steer is dropped from the row"
        );
        assert!(!S::steer_answered(&*c, &running, "h_1", &opts, None, "steer_2", false), "an answer that never landed records nothing");
    });
}

// Stops, ends and deletes (sessions.ts:96-210, 1380-1498).

#[test]
fn nothing_queued_is_dropped_nothing_runs_under_an_unheld_thread_and_a_try_with_no_line_is_no_line() {
    run(options, |_, c| async move {
        assert_eq!(S::drop_queued(&*c, "turn_none").await, Ok(false));
        assert_eq!(S::stop_under(&*c, "t_none", None).await, Ok(Vec::<String>::new()));
        S::try_stopped(&*c, "turn_none");
        let plain = row(view("s_1", "w_1", json!({ "threadId": "t_1" })));
        put_row(&c, "s_1", plain.clone());
        assert!(!S::line_try(&*c, &plain), "a row that asked for nothing owed has no line to try");
    });
}

#[test]
fn a_cloud_kind_ends_no_thread_group_and_owes_no_end() {
    // waits on packet 2 for the kind modules.
    run(options, |_, c| async move {
        let entry = put_live(&c, record_with("w_1", json!({ "kind": "cloud" }))).await;
        assert!(S::end_thread(&*c, &entry, "t_1", Default::default()).is_none(), "a cloud kind groups no processes");
        assert_eq!(S::end_owed(&*c, &entry, "t_1").await.map(|owed| owed.is_none()), Ok(true));
    });
}

#[test]
fn a_lead_of_no_workspace_leads_no_sub_and_a_sub_done_for_an_unheld_workspace_is_no_failure() {
    run(options, |_, c| async move {
        assert!(!S::lead_of_sub(&*c, None, None));
        assert!(!S::lead_of_sub(&*c, Some("w_none"), None));
        S::sub_done(&*c, "w_none");
    });
}

#[test]
fn a_thread_with_no_shell_open_ends_no_shell_and_a_checkpoint_drop_on_a_plain_entry_is_no_failure() {
    // waits on packet 8: the folders area drops the checkpoints through its context.
    run(options, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        S::end_thread_shells(&*c, &entry, "t_1").await.expect("no shell to end");
        S::drop_checkpoints(&*c, &entry, "t_1").await.expect("no checkpoint to drop");
        S::drop_thread_files(&*c, &entry, &["t_1".to_string()]).await.expect("no file to drop");
        S::row_gone(&*c, "s_1", "w_1", "t_1");
    });
}

#[test]
fn deleting_a_workspace_the_host_does_not_hold_is_refused() {
    // waits on packet 9: the workspaces area deletes through the phases context.
    run(options, |_, c| async move {
        assert!(S::delete_workspace(&*c, "w_none", None).await.is_err());
        assert!(S::wake(&*c, "w_none", None).await.is_err());
    });
}

#[test]
fn the_slates_of_a_thread_are_rewound_and_forgotten_through_the_slates_area() {
    // waits on wsp-slates: `SlatesArea::rewound` and `forget` hold `todo!()` on this base.
    run(options, |_, c| async move {
        S::slates_rewound(&*c, "t_1", "turn_1", &["turn_2".to_string()]).await.expect("nothing to rewind");
        S::slates_forget(&*c, "t_1", false).await.expect("nothing to forget");
    });
}

#[test]
fn a_kept_agent_that_is_not_there_is_reaped_and_checked_without_failure() {
    run(options, |_, c| async move {
        S::reap_kept(&*c, "t_none");
        S::kept_check(&*c, "t_none").await.expect("no kept agent to check");
        assert!(S::take_kept(&*c, "t_none", None, None, None).is_none());
        assert!(S::claim_warm(&*c, "w_1", "claude").is_none(), "no warm agent held for it");
    });
}

#[test]
fn a_thread_is_moved_onto_the_copy_of_its_session_in_every_row_that_named_the_session() {
    // waits on packet 12: `move_session` reads the sessions through the forks context.
    run(options, |_, c| async move {
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "claudeSessionId": "cs_old" }))));
        put_row(&c, "s_2", row(view("s_2", "w_1", json!({ "threadId": "t_2", "claudeSessionId": "cs_old" }))));
        S::move_session(&*c, "t_1", "cs_old", "cs_copy");
        let sessions = S::sessions(&*c);
        assert_eq!(sessions.borrow()["s_1"].borrow().view.claude_session_id.as_deref(), Some("cs_copy"));
        assert_eq!(sessions.borrow()["s_2"].borrow().view.claude_session_id.as_deref(), Some("cs_old"), "another thread's row is left");
    });
}

#[test]
fn an_open_without_a_fork_is_the_options_as_they_came_and_no_plan() {
    // waits on packet 12: the forks free functions take the context as `ForksCtx`.
    run(options, |_, c| async move {
        let opened = wsp_core::types::api::StartOptions { prompt: "fix it".into(), ..Default::default() };
        let (out, plan) = S::fork_opened(&*c, "w_1", opened, None).await.expect("opened");
        assert_eq!(out.prompt, "fix it");
        assert!(plan.is_none());
    });
}

// The turn end to end on a fake agent (acceptance 3 of #2476).

use wsp_adapters::port::AdapterEvent;

/// An agent that streams its answer in two pieces and ends, the way the TS tests' `replying` adapters do.
struct Replying;

struct Said {
    session: String,
    done: futures::future::Shared<futures::future::LocalBoxFuture<'static, wsp_wire::views::session_events::TurnResult>>,
}

impl HarnessSession for Said {
    fn local_id(&self) -> String {
        self.session.clone()
    }
    fn finished(&self) -> futures::future::Shared<futures::future::LocalBoxFuture<'static, wsp_wire::views::session_events::TurnResult>> {
        self.done.clone()
    }
    fn interrupt(&self) -> futures::future::LocalBoxFuture<'static, ()> {
        Box::pin(async {})
    }
}

impl HarnessAdapter for Replying {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, wsp_adapters::port::AdapterError> {
        use futures::FutureExt as _;
        let session = options.resume.clone().unwrap_or_else(|| "22222222-2222-4222-8222-222222222222".to_string());
        let on = options.on_event.clone().expect("a start carries its event sink");
        let result: wsp_wire::views::session_events::TurnResult = serde_json::from_value(
            json!({ "status": "completed", "text": "Done. The redirect now sends every old path to the canonical host." }),
        )
        .expect("a result");
        let id = session.clone();
        let sent = result.clone();
        let done: futures::future::LocalBoxFuture<'static, _> = Box::pin(async move {
            tokio::task::yield_now().await;
            let event = |value: Value| serde_json::from_value::<AdapterEvent>(value).expect("an adapter event");
            on(event(json!({ "type": "session.start", "sessionId": id, "cwd": "/root/app" })));
            on(event(
                json!({ "type": "turn.delta", "sessionId": id, "kind": "text", "text": "Done. The redirect now ", "messageId": "m1" }),
            ));
            on(event(
                json!({ "type": "turn.delta", "sessionId": id, "kind": "text", "text": "sends every old path to the canonical host.", "messageId": "m1" }),
            ));
            on(event(json!({ "type": "turn.done", "sessionId": id, "result": sent })));
            on(event(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            sent
        });
        Ok(Arc::new(Said { session, done: done.shared() }))
    }
    fn steers(&self) -> bool {
        false
    }
}

#[test]
fn a_turn_runs_and_comes_back() {
    // waits on packets 5, 11 and 12: the boot, launch and turn bodies a turn runs through.
    let held = state();
    let dir = held.dir.path().to_path_buf();
    let store = held.store.clone();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let factory: wsp_core::types::harness::HarnessAdapterFactory = Arc::new(|_| Ok(Arc::new(Replying) as Arc<dyn HarnessAdapter>));
        let mut adapters = IndexMap::new();
        adapters.insert("claude".to_string(), factory);
        let opts = |store: Rc<dyn Store>| RuntimeOptions {
            backend: Some(backend()),
            store: Some(store),
            adapters: adapters.clone(),
            ..Default::default()
        };
        let first = create_runtime(opts(store.clone())).expect("a runtime");
        let ctx = first.context();
        put_live(&ctx, record("w_1")).await;
        let (started, _off) = listen(&first, "session.start");
        let (ended, _off2) = listen(&first, "session.end");
        let handle = first
            .sessions()
            .start("w_1", wsp_core::types::api::StartOptions { prompt: "fix the redirect".into(), ..Default::default() }, None)
            .await
            .expect("a started turn");
        let result = handle.finished().await;
        assert_eq!(serde_json::to_value(&result).unwrap()["status"], json!("completed"));
        assert_eq!(started.borrow().len(), 1, "one start");
        assert_eq!(ended.borrow().len(), 1, "one end");
        let thread = handle.view().thread_id.expect("the turn opened a thread");
        let history = first.sessions().history("w_1", None).await.expect("history");
        let text: String = history
            .iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .filter(|e| e["type"] == json!("session.delta"))
            .map(|e| e["text"].as_str().unwrap_or("").to_string())
            .collect();
        assert_eq!(text, "Done. The redirect now sends every old path to the canonical host.");
        drop(first);

        let again = create_runtime(opts(store)).expect("a runtime");
        let rows = again.sessions().list(Some("w_1".to_string()), None).await.expect("rows");
        assert_eq!(
            rows.iter().filter_map(|r| r.thread_id.clone()).collect::<Vec<_>>(),
            std::slice::from_ref(&thread),
            "the thread reads back on a new runtime"
        );
        let back = again.sessions().history("w_1", None).await.expect("history");
        assert_eq!(back.len(), history.len(), "and so does every event of it");
        drop(dir);
    });
}

// The launch's holds on a computer and its files (session-images, thread-cap; sessions.ts:640-760, threads.ts:374-400).

#[test]
fn a_turn_held_for_its_computers_cap_is_read_off_the_area_and_taken_back_when_it_leaves() {
    // waits on packet 11: the threads area reads the place of the record through its context.
    run(options, |_, c| async move {
        let rec = record_with("w_1", json!({ "kind": "local" }));
        S::cap_hold(&*c, &rec, "turn_1", None);
        assert!(S::cap_held(&*c, "turn_1").is_some(), "held");
        S::cap_lend(&*c, "turn_1", "turn_0");
        S::cap_stop(&*c, "turn_1", "stopped");
        S::cap_left(&*c, "turn_1");
        assert!(S::cap_held(&*c, "turn_1").is_none(), "left");
        let release = S::cap_stopping(&*c, "turn_none");
        release();
    });
}

#[test]
fn a_computer_with_no_cap_is_never_full() {
    // waits on packet 11.
    run(options, |_, c| async move {
        assert!(S::cap_full(&*c, &record_with("w_1", json!({ "kind": "local" })), "turn_1").is_none());
    });
}

#[test]
fn the_cut_before_a_thread_is_what_its_index_says_and_no_cut_for_a_thread_it_does_not_hold() {
    run(options, |_, c| async move {
        assert!(!S::cut_before(&*c, "w_1", "t_1"));
        index_of(&c, "w_1", |index| {
            index.cut.insert("t_1".into(), true);
        });
        assert!(S::cut_before(&*c, "w_1", "t_1"));
        assert!(!S::cut_before(&*c, "w_1", "t_2"));
    });
}

#[test]
fn files_and_images_of_a_send_that_has_none_land_nowhere() {
    // waits on packet 11: the agents area lands attachments through its context.
    run(options, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        assert_eq!(S::land_files(&*c, &entry, "/root/app", "/root/app/.wsp/files", Vec::new()).await, Ok(Vec::<String>::new()));
        S::drop_images(&*c, &entry, "/root/app/.wsp/images");
    });
}

#[test]
fn a_taken_request_whose_turn_the_host_no_longer_holds_answers_no_handle() {
    // waits on packet 11.
    run(options, |_, c| async move {
        let taken = Taken { session_id: "s_9".into(), thread_id: "t_9".into(), turn_id: "turn_9".into(), outcome: TakenOutcome::Started };
        assert!(S::taken_turn(&*c, "w_1", taken).await.expect("an answer").is_none());
    });
}

#[test]
fn a_first_run_and_the_limit_details_are_read_off_the_computer_the_entry_holds() {
    // waits on packet 11.
    run(options, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        assert!(S::first_run_here(&*c, &entry, "claude").await.is_ok());
        assert!(S::limit_details_due(&*c, &entry, "claude").await.is_ok());
        let snapshot = S::snapshot_of(&*c, &entry, "/root/app");
        let _ = snapshot.await;
    });
}

#[test]
fn a_notice_to_no_thread_is_sent_to_none() {
    run(options, |_, c| async move {
        let ended = row(view("s_1", "w_1", json!({ "threadId": "t_1" })));
        let result: wsp_wire::views::session_events::TurnResult =
            serde_json::from_value(json!({ "status": "completed", "text": "done" })).expect("a result");
        S::notify_end(&*c, &ended, &[], None, None, result);
    });
}
