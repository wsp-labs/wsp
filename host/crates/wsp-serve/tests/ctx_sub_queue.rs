// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 7 (#2476): the context's bodies for `SubQueueCtx` (36 methods, sub-queue.ts:49-498),
//! `SubLifeCtx` (31, sub-life.ts:156-597) and `PullRequestsCtx` (24, pull-requests.ts:9-300), read against the TS at
//! 65f6b89c7: runtime.ts:444 and :454-455, the three area files' reads of `ctx`, and the seven TS tests the plan names
//! (waiting-behind, thread-cap, box-stop, stop-past-grace, cloud-turn-end, pull-request, subagents).
//!
//! Those seven build a whole runtime, make workspaces with `createOn` and run turns on a fake adapter or the real
//! Claude adapter over a fake computer. A Rust test cannot replace an area, and the create, turn and session bodies
//! they run through belong to packets 4, 6, 8, 9, 11, 12 and 13, so each method is read at its own seam on the runtime
//! `create_runtime` builds over `FakeBackend`, seeded through the handles the traits hand out. A test that needs
//! another packet's bodies says which in its comment ("waits on packet N") and is red until that packet lands.
mod support;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use wsp_account::account::core_types::{AccountError, Ask};
use wsp_account::account::pull_requests::{CacheEntry, PullRequestsCtx as P};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::Runtime as _;
use wsp_core::types::internal::{SessionEntry, SubStartRecord, ThreadRecord, SUB_STARTS};
use wsp_core::types::wiring::{Broker, LiveWorkspace, RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::machines::boot::BootRuntime;
use wsp_machines::machines::machines::{Entry, Row};
use wsp_machines::machines::sub_life::SubLifeCtx as L;
use wsp_machines::machines::sub_queue::SubQueueCtx as Q;
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::memory_store;
use wsp_wire::views::session::SessionView;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures.

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn options() -> RuntimeOptions {
    RuntimeOptions { backend: Some(backend()), store: Some(Rc::new(memory_store())), ..Default::default() }
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

fn with_door() -> RuntimeOptions {
    RuntimeOptions { place_links: Some(Rc::new(wiring())), ..options() }
}

/// The runtime on a throwaway state, inside the task set the owner thread runs, and its context.
fn run<Fut: Future<Output = ()>>(opts: RuntimeOptions, f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts).expect("a runtime");
        let ctx = runtime.context();
        f(runtime, ctx).await
    });
}

fn record(id: &str) -> WorkspaceRecord {
    serde_json::from_value(json!({
        "id": id, "name": "pricing page", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    }))
    .expect("a workspace record")
}

fn cloud_record(id: &str, extra: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": id, "name": "pricing page", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false, "kind": "cloud",
    });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a workspace record")
}

fn project(id: &str, name: &str, path: &str) -> wsp_wire::views::workspace::ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": "zingzys-mac", "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

fn cell(record: WorkspaceRecord) -> Rc<RefCell<WorkspaceRecord>> {
    Rc::new(RefCell::new(record))
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
    BootRuntime::sessions(&**c).borrow_mut().insert(key.to_string(), row);
}

fn thread(workspace: &str) -> ThreadRecord {
    serde_json::from_value(json!({ "workspaceId": workspace, "harness": "claude" })).expect("a thread record")
}

fn put_thread(c: &Rc<Ctx>, id: &str, workspace: &str) {
    BootRuntime::thread_records(&**c).borrow_mut().insert(id.to_string(), thread(workspace));
}

fn notice(message: &str) -> Value {
    json!({ "type": "host.notice", "message": message })
}

fn start_record(workspace: &str, thread: &str, turn: &str) -> SubStartRecord {
    serde_json::from_value(json!({
        "workspaceId": workspace, "threadId": thread, "turnId": turn, "at": "2026-10-26T00:00:00.000Z",
        "opts": { "prompt": "write the page" }, "tree": { "rootThreadId": thread },
    }))
    .expect("a held start")
}

/// A live entry over `record` on a fake machine, put in the runtime's live map through the boot area's handle.
async fn put_live(c: &Rc<Ctx>, record: WorkspaceRecord) -> Entry {
    let machine: Arc<dyn Machine> =
        FakeBackend::new(FakeBackendOptions::default()).get_fake(&record.machine_id).await.expect("a fake machine");
    put_live_over(c, record, machine)
}

/// The same over a machine the caller made, as a test that reads the machine's state through the runtime's own backend does.
fn put_live_over(c: &Rc<Ctx>, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
    use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
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
    BootRuntime::live(&**c).borrow_mut().insert(id, entry.clone());
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

/// A broker that keeps every lift and revoke it was asked for, and whose revoke fails when `says` is set.
struct Keeping {
    lifted: RefCell<Vec<String>>,
    revoked: RefCell<Vec<String>>,
    says: Option<String>,
}

#[async_trait(?Send)]
impl Broker for Keeping {
    fn ready(&self) -> bool {
        true
    }
    async fn codex_claims(&self) -> Result<Option<wsp_adapters::pending::CodexClaims>, HostError> {
        Ok(None)
    }
    fn revoke(&self, workspace_id: &str) -> Option<wsp_core::types::wiring::Local<'static, ()>> {
        self.revoked.borrow_mut().push(workspace_id.to_string());
        let says = self.says.clone();
        Some(Box::pin(async move {
            match says {
                Some(says) => Err(HostError::new(says)),
                None => Ok(()),
            }
        }))
    }
    fn lift(&self, workspace_id: &str) {
        self.lifted.borrow_mut().push(workspace_id.to_string());
    }
}

fn keeping(says: Option<&str>) -> Rc<Keeping> {
    Rc::new(Keeping { lifted: RefCell::default(), revoked: RefCell::default(), says: says.map(str::to_string) })
}

// ---------------------------------------------------------------------------------------------------------------
// The reads and writes of the core's own maps (sub-queue.ts:50, sub-life.ts:157, pull-requests.ts:9).

#[test]
fn every_live_read_is_the_one_live_map_in_its_insertion_order() {
    run(options(), |_, c| async move {
        assert!(Q::live_all(&*c).is_empty());
        put_live(&c, record("w_1")).await;
        put_live(&c, record("w_2")).await;
        for (live, ids) in [(Q::live(&*c, "w_2"), Q::live_all(&*c)), (L::live(&*c, "w_2"), L::live_all(&*c)), (P::live(&*c, "w_2"), vec![])]
        {
            assert_eq!(live.expect("w_2 is live").borrow().record.borrow().id, "w_2");
            if !ids.is_empty() {
                let order: Vec<String> = ids.iter().map(|e| e.borrow().record.borrow().id.clone()).collect();
                assert_eq!(order, vec!["w_1".to_string(), "w_2".to_string()]);
            }
        }
        assert!(Q::live(&*c, "w_9").is_none());
        assert!(L::live(&*c, "w_9").is_none());
        assert!(P::live(&*c, "w_9").is_none());
    });
}

#[test]
fn the_record_cell_is_the_live_entrys_own_record() {
    run(options(), |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let held = Q::record_cell(&*c, "w_1").expect("a live record");
        assert!(Rc::ptr_eq(&held, &entry.borrow().record), "the shared cell, not a copy");
        held.borrow_mut().name = "renamed".into();
        assert_eq!(entry.borrow().record.borrow().name, "renamed");
        assert!(Q::record_cell(&*c, "w_9").is_none());
    });
}

#[test]
fn queue_session_reads_copy_the_row_and_writes_keep_the_maps_order() {
    run(options(), |_, c| async move {
        let mut first = session_entry(view("s_1", "w_1", json!({})));
        first.notify = Some(vec!["t_lead".into()]);
        first.end = Some(Rc::new(|_, _, _| {}));
        first.queued = true;
        put_row(&c, "turn-1", Rc::new(RefCell::new(first)));
        put_row(&c, "turn-2", row(view("s_2", "w_1", json!({}))));
        put_row(&c, "turn-3", row(view("s_3", "w_1", json!({}))));

        let read = Q::session(&*c, "turn-1").expect("a row");
        assert_eq!(read.view.id, "s_1");
        assert_eq!(read.notify, Some(vec!["t_lead".to_string()]));
        assert!(read.end.is_some() && read.queued, "what the row holds beyond its view comes with it");
        assert!(Q::session(&*c, "turn-9").is_none());

        Q::update_session(&*c, "turn-2", &mut |held| held.view.harness = "codex".into());
        let rows = BootRuntime::sessions(&*c);
        assert_eq!(rows.borrow()["turn-2"].borrow().view.harness.as_str(), "codex", "changed in place");
        assert_eq!(rows.borrow().keys().cloned().collect::<Vec<_>>(), ["turn-1", "turn-2", "turn-3"]);
        Q::update_session(&*c, "turn-9", &mut |_| panic!("no row stands at turn-9"));

        Q::set_session(&*c, "turn-2", session_entry(view("s_2b", "w_1", json!({}))));
        assert_eq!(rows.borrow()["turn-2"].borrow().view.id, "s_2b", "a set replaces the row");
        assert_eq!(rows.borrow().keys().cloned().collect::<Vec<_>>(), ["turn-1", "turn-2", "turn-3"], "at the place it held");
        Q::set_session(&*c, "turn-4", session_entry(view("s_4", "w_1", json!({}))));
        assert_eq!(rows.borrow().keys().last().map(String::as_str), Some("turn-4"));

        Q::remove_session(&*c, "turn-1");
        assert!(Q::session(&*c, "turn-1").is_none());
        assert_eq!(rows.borrow().len(), 3);
    });
}

#[test]
fn sub_life_and_pull_request_views_are_the_session_rows_in_map_order() {
    run(options(), |_, c| async move {
        put_row(&c, "turn-1", row(view("s_1", "w_1", json!({ "threadId": "t_1" }))));
        put_row(&c, "turn-2", row(view("s_2", "w_2", json!({}))));
        for views in [L::session_views(&*c), P::session_views(&*c)] {
            assert_eq!(views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["s_1", "s_2"]);
        }
    });
}

#[test]
fn thread_records_are_thread_and_workspace_pairs_in_insertion_order() {
    run(options(), |_, c| async move {
        assert!(L::thread_records(&*c).is_empty());
        put_thread(&c, "t_b", "w_2");
        put_thread(&c, "t_a", "w_1");
        assert_eq!(L::thread_records(&*c), vec![("t_b".to_string(), "w_2".to_string()), ("t_a".to_string(), "w_1".to_string())]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The clock, ids, tasks and the log.

#[test]
fn now_is_the_clock_in_each_traits_own_number() {
    let fake = FakeClock::new(4_000);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        assert_eq!((Q::now(&*c), L::now(&*c), P::now(&*c)), (4_000.0, 4_000.0, 4_000));
        fake.advance(1_500);
        assert_eq!((Q::now(&*c), L::now(&*c), P::now(&*c)), (5_500.0, 5_500.0, 5_500));
    });
}

#[test]
fn schedule_runs_on_the_clock_and_a_cancel_stops_it() {
    let fake = FakeClock::new(0);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        let fired = Rc::new(Cell::new(0));
        let bump = |n: i32| -> Box<dyn FnOnce()> {
            let fired = fired.clone();
            Box::new(move || fired.set(fired.get() + n))
        };
        let _kept = Q::schedule(&*c, 100.0, bump(1));
        let _also = L::schedule(&*c, 200.0, bump(10));
        let _third = P::schedule(&*c, 300, bump(100));
        let cancelled = Q::schedule(&*c, 150.0, bump(1_000));
        cancelled();
        fake.advance(99);
        assert_eq!(fired.get(), 0);
        fake.advance(1);
        assert_eq!(fired.get(), 1);
        fake.advance(200);
        assert_eq!(fired.get(), 111, "the cancelled one never ran");
    });
}

#[test]
fn uuid_draws_a_fresh_value_each_time() {
    run(options(), |_, c| async move {
        let (a, b) = (Q::uuid(&*c), Q::uuid(&*c));
        assert_eq!((a.len(), b.len()), (36, 36));
        assert_ne!(a, b);
    });
}

#[test]
fn every_spawn_starts_its_work_now_and_lets_it_run_to_its_end() {
    run(options(), |_, c| async move {
        let done = Rc::new(Cell::new(0));
        let work = |n: i32| -> futures::future::LocalBoxFuture<'static, ()> {
            let done = done.clone();
            Box::pin(async move { done.set(done.get() + n) })
        };
        Q::spawn(&*c, work(1));
        L::spawn(&*c, work(10));
        P::spawn(&*c, work(100));
        tokio::task::yield_now().await;
        assert_eq!(done.get(), 111, "each of the three traits' spawn ran its work");
    });
}

#[test]
fn every_warn_writes_one_line_to_the_hosts_log() {
    run(options(), |_, c| async move {
        let captured = wsp_core::console::capture();
        Q::warn(&*c, "one");
        L::warn(&*c, "two");
        P::warn(&*c, "three");
        assert_eq!(captured.lines(), ["one", "two", "three"]);
    });
}

#[test]
fn on_event_hears_the_bus_by_kind_and_by_star() {
    run(options(), |_, c| async move {
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        let (queue, life) = (heard.clone(), heard.clone());
        Q::on_event(
            &*c,
            "host.notice",
            Rc::new(move |e: &Value| queue.borrow_mut().push(format!("q:{}", e["message"].as_str().unwrap_or("")))),
        );
        L::on_event(
            &*c,
            "host.notice",
            Rc::new(move |e: &Value| life.borrow_mut().push(format!("l:{}", e["message"].as_str().unwrap_or("")))),
        );
        let star = Rc::new(Cell::new(0));
        let counted = star.clone();
        Q::on_event(&*c, "*", Rc::new(move |_: &Value| counted.set(counted.get() + 1)));
        BootRuntime::emit(&*c, serde_json::from_value(notice("one")).expect("an event")).expect("emit");
        assert_eq!(*heard.borrow(), ["q:one", "l:one"]);
        assert!(star.get() >= 1, "a `*` listener hears every event");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The bus and the transcript.

#[test]
fn emit_delivers_the_event_to_the_bus_for_both_traits() {
    run(options(), |runtime, c| async move {
        let (seen, _off) = listen(&runtime, "host.notice");
        Q::emit(&*c, notice("from the queue"));
        L::emit(&*c, notice("from the life"));
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["message"], json!("from the queue"));
        assert_eq!(seen[1]["message"], json!("from the life"));
        assert_eq!(seen[1]["seq"].as_i64(), Some(seen[0]["seq"].as_i64().unwrap() + 1), "each stamped in turn");
    });
}

#[test]
fn emit_of_a_shape_that_is_no_event_warns_and_goes_nowhere() {
    run(options(), |_, c| async move {
        let captured = wsp_core::console::capture();
        Q::emit(&*c, json!({ "type": "no.such.event" }));
        assert_eq!(captured.lines().len(), 1, "one warning, no panic");
    });
}

#[test]
fn record_writes_a_transcript_row_and_says_it_on_the_bus() {
    // waits on packet 4: the bus reaches the machines area, whose context reads the live map.
    let fake = FakeClock::new(9_000);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |runtime, c| async move {
        let (seen, _off) = listen(&runtime, "session.delta");
        Q::record(
            &*c,
            json!({ "type": "session.delta", "workspaceId": "w_1", "sessionId": "s", "kind": "text", "text": "waiting", "pos": 1 }),
        );
        assert_eq!(seen.borrow().len(), 1, "the event reached the bus once");
        assert_eq!(seen.borrow()[0]["workspaceId"], json!("w_1"));
        assert_eq!(seen.borrow()[0]["at"], json!(9_000), "stamped with the clock's moment");
    });
}

#[test]
fn push_head_says_thread_head_for_a_thread_the_host_holds_and_nothing_for_another() {
    run(options(), |runtime, c| async move {
        let (seen, _off) = listen(&runtime, "thread.head");
        put_thread(&c, "t_1", "w_1");
        put_row(&c, "turn-1", row(view("s_1", "w_1", json!({ "threadId": "t_1" }))));
        Q::push_head(&*c, "t_9");
        assert!(seen.borrow().is_empty(), "no thread t_9");
        Q::push_head(&*c, "t_1");
        L::push_head(&*c, "t_1");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2, "both traits push the head");
        assert_eq!(seen[0]["threadId"], json!("t_1"));
        assert_eq!(seen[0]["workspaceId"], json!("w_1"));
    });
}

#[test]
fn rows_sent_answers_at_once_when_no_push_is_queued() {
    run(options(), |_, c| async move {
        Q::rows_sent(&*c).await;
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The held starts (store.put(SUB_STARTS, ..)) and the blobs.

#[test]
fn held_starts_are_put_listed_and_deleted_in_the_stores_sub_starts() {
    run(options(), |_, c| async move {
        let store = BootRuntime::store(&*c);
        assert!(Q::list_starts(&*c).await.expect("list").is_empty());
        Q::put_start(&*c, &start_record("w_1", "t_1", "turn-1")).await.expect("put");
        Q::put_start(&*c, &start_record("w_2", "t_2", "turn-2")).await.expect("put");
        let written = store.get(SUB_STARTS, "w_1").await.expect("get").expect("a row at the workspace's id");
        assert_eq!(written["threadId"], json!("t_1"));
        assert_eq!(written["opts"], json!({ "prompt": "write the page" }));
        let listed = Q::list_starts(&*c).await.expect("list");
        assert_eq!(listed.iter().map(|r| r.workspace_id.as_str()).collect::<Vec<_>>(), ["w_1", "w_2"]);
        Q::delete_start(&*c, "w_1").await.expect("delete");
        assert!(store.get(SUB_STARTS, "w_1").await.expect("get").is_none());
        assert_eq!(Q::list_starts(&*c).await.expect("list").len(), 1);
        Q::delete_start(&*c, "w_9").await.expect("deleting what is not there is no failure");
    });
}

#[test]
fn blobs_are_the_stores_blobs() {
    run(options(), |_, c| async move {
        let store = BootRuntime::store(&*c);
        L::put_blob(&*c, "sub-reads", "w_1", b"bytes".to_vec()).await.expect("put");
        assert_eq!(store.get_blob("sub-reads", "w_1").await.expect("get"), Some(b"bytes".to_vec()));
        L::delete_blob(&*c, "sub-reads", "w_1").await.expect("delete");
        assert_eq!(store.get_blob("sub-reads", "w_1").await.expect("get"), None);
    });
}

#[test]
fn persist_writes_the_workspace_row_through_the_records_area() {
    // waits on packet 2: the records area's context (project_ref) is kinds.rs.
    run(options(), |_, c| async move {
        let store = BootRuntime::store(&*c);
        let r = record("w_1");
        Q::persist(&*c, &r).await.expect("queue persist");
        let kept = store.get("workspaces", "w_1").await.expect("get");
        assert_eq!(kept.expect("a row")["id"], json!("w_1"));
        store.delete("workspaces", "w_1").await.expect("delete");
        L::persist(&*c, &r).await.expect("life persist");
        assert!(store.get("workspaces", "w_1").await.expect("get").is_some());
        store.delete("workspaces", "w_1").await.expect("delete");
        P::persist(&*c, &r).await.expect("pull request persist");
        assert!(store.get("workspaces", "w_1").await.expect("get").is_some());
    });
}

#[test]
fn persist_sessions_writes_the_workspaces_session_rows() {
    // waits on packet 4: the kept write reads the live record of the workspace.
    run(options(), |_, c| async move {
        put_live(&c, record("w_1")).await;
        put_row(&c, "turn-1", row(view("s_1", "w_1", json!({ "threadId": "t_1" }))));
        Q::persist_sessions(&*c, "w_1").await.expect("persist sessions");
    });
}

#[test]
fn view_is_the_records_view_of_the_record() {
    // waits on packet 2: the rest of the view is read by the kinds context.
    run(options(), |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web"));
        let shown = Q::view(&*c, &record("w_1")).expect("a view");
        assert_eq!(shown["id"], json!("w_1"));
        assert_eq!(shown["name"], json!("pricing page"));
        assert_eq!(shown["phase"], json!("running"));
        assert_eq!(shown["project"]["name"], json!("web"));
        let lost = Q::view(&*c, &cloud_record("w_2", json!({ "project": "p_9" }))).expect_err("refused");
        assert_eq!(lost.message, "no project \"p_9\"; this host holds web", "a project the host does not hold: records.ts:88 throws");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Where a record's machines are counted (rules, channels, the place door, the backend's limits).

#[test]
fn a_workspace_of_this_computer_is_at_here_and_a_cloud_one_at_its_place() {
    run(options(), |_, c| async move {
        let local = cloud_record("w_1", json!({ "kind": "local" }));
        assert_eq!(Q::place_id_of(&*c, &local).as_deref(), Some("here"));
        assert_eq!(Q::place_id_of(&*c, &record("w_3")).as_deref(), Some("default"), "no kind and no place: the wired provider");
        let at = cloud_record("w_2", json!({ "place": "pl_9" }));
        assert_eq!(Q::place_id_of(&*c, &at).as_deref(), Some("pl_9"));
        assert_eq!(L::place_id_of(&*c, &at).as_deref(), Some("pl_9"));
    });
}

#[test]
fn a_slot_is_held_only_by_a_record_that_pauses_and_has_not_ended() {
    // waits on packet 2: `pause_mode_declared` reads the kind's module.
    run(options(), |_, c| async move {
        let running = cloud_record("w_1", json!({}));
        assert_eq!(Q::holds_slot(&*c, &running), P::pauses(&*c, &running), "a running workspace holds a slot where its kind pauses");
        let napping = cloud_record("w_2", json!({ "phase": "napping" }));
        assert!(!Q::holds_slot(&*c, &napping), "a napping workspace holds no slot");
    });
}

#[test]
fn a_host_with_no_door_names_no_place_and_counts_no_machines() {
    run(options(), |_, c| async move {
        assert_eq!(Q::place_name_of(&*c, "here"), None);
        assert_eq!(Q::machines_at(&*c, "here"), None);
        assert_eq!(L::delete_after_ms(&*c, Some("pl_1")), None);
        assert_eq!(L::delete_after_ms(&*c, None), None);
    });
}

#[test]
fn a_host_with_a_door_asks_the_door() {
    // waits on packet 5: the door answers its rows through the boot context.
    run(with_door(), |runtime, c| async move {
        let door = runtime.places().expect("the door");
        for place in ["pl_1", "here"] {
            assert_eq!(Q::place_name_of(&*c, place), Some(door.name_of(place)), "name {place}");
            assert_eq!(Q::machines_at(&*c, place), door.machines_at(place).map(|n| n as f64), "machines {place}");
        }
        assert_eq!(L::delete_after_ms(&*c, Some("pl_1")), None, "a place that set no delete-after");
    });
}

#[test]
fn a_backend_that_offers_no_limits_answers_none() {
    // waits on packet 2: `backend_for` goes through the kinds module of the record's kind.
    run(options(), |_, c| async move {
        let limits = Q::limits_of(&*c, &record("w_1")).await.expect("a backend with no limits is no failure");
        assert!(limits.is_none(), "the fake backend has no limits road");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The thread reads of the sub-life area (threads.ts, agents.ts, transcripts.ts).

#[test]
fn the_thread_reads_of_an_empty_runtime_say_nothing_runs() {
    // waits on packet 11: `latest_on`, `thread_runs` and `launching_on` are the threads area's.
    run(options(), |_, c| async move {
        assert!(L::latest_on(&*c, "t_1").is_none());
        assert!(!L::thread_runs(&*c, "t_1"));
        assert!(!L::launching_on(&*c, "t_1"));
    });
}

#[test]
fn the_latest_row_of_a_thread_is_the_last_one_put() {
    // waits on packet 11.
    run(options(), |_, c| async move {
        put_thread(&c, "t_1", "w_1");
        put_row(&c, "turn-1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "status": "completed" }))));
        put_row(&c, "turn-2", row(view("s_2", "w_1", json!({ "threadId": "t_1", "status": "running" }))));
        assert_eq!(L::latest_on(&*c, "t_1").expect("a row").id, "s_2");
        assert!(L::thread_runs(&*c, "t_1"));
    });
}

#[test]
fn a_session_file_is_what_the_named_adapter_says_and_none_where_it_names_none() {
    // waits on packet 11: `adapter_for` is the agents area's, over `opts.adapters`.
    use wsp_adapters::port::{HarnessAdapter, HarnessSession, HarnessStartOptions, SessionFile as AdapterFile};
    struct Files;
    impl HarnessAdapter for Files {
        fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, wsp_adapters::port::AdapterError> {
            unimplemented!()
        }
        fn steers(&self) -> bool {
            false
        }
        fn session_file(&self, session_id: &str) -> Option<AdapterFile> {
            Some(AdapterFile { folder: "/root/.files".into(), name: format!("{session_id}.jsonl"), depth: 2 })
        }
    }
    let mut adapters = indexmap::IndexMap::new();
    adapters.insert(
        "claude".to_string(),
        Arc::new(|_: wsp_core::types::harness::HarnessAdapterContext| Ok(Arc::new(Files) as Arc<dyn HarnessAdapter>)) as _,
    );
    run(RuntimeOptions { adapters, ..options() }, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let found = L::session_file(&*c, &entry, "claude", "s_1").expect("a session file");
        assert_eq!((found.folder.as_str(), found.name.as_str(), found.depth), ("/root/.files", "s_1.jsonl", 2));
        assert!(L::session_file(&*c, &entry, "codex", "s_1").is_none(), "no adapter named codex");
    });
}

#[test]
fn a_transcript_read_answers_the_events_of_the_thread_as_json() {
    let dir = tempfile::tempdir().expect("a folder");
    let sqlite = Rc::new(wsp_store::sqlite_store::sqlite_store(
        &dir.path().join("state.json"),
        wsp_store::store::StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ));
    run(RuntimeOptions { store: Some(sqlite.clone()), ..options() }, |_, c| async move {
        let event = |text: &str, pos: u32| -> wsp_wire::views::session_events_more::SessionEvent {
            serde_json::from_value(json!({
                "type": "session.delta", "workspaceId": "w_1", "sessionId": "s_1", "threadId": "t_1", "kind": "text", "text": text, "pos": pos,
            }))
            .expect("a session event")
        };
        wsp_threads::threads::transcripts::TranscriptsCtx::rows(&*c)
            .append("w_1", &[event("one", 1), event("two", 2), event("three", 3)])
            .expect("append");
        let all = L::read_transcript(&*c, "w_1", "t_1", 10, 1_000_000).await.expect("read");
        assert_eq!(all.iter().map(|e| e["text"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["one", "two", "three"]);
        assert_eq!(all[0]["type"], json!("session.delta"));
        let last = L::read_transcript(&*c, "w_1", "t_1", 1, 1_000_000).await.expect("read");
        assert_eq!(
            last.iter().map(|e| e["text"].as_str().unwrap_or("")).collect::<Vec<_>>(),
            ["three"],
            "the newest events within the limit"
        );
        assert!(L::read_transcript(&*c, "w_1", "t_9", 10, 1_000_000).await.expect("read").is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The machines area's roads (machines.ts, reach.ts), for the entry and the record.

#[test]
fn an_absent_or_dead_machine_is_attached_to_the_record_and_put_in_the_live_map() {
    // waits on packet 4: `attach` builds its engine workspace through the machines context.
    run(options(), |_, c| async move {
        let absent = Q::attach_absent(&*c, &cell(record("w_1")), "the box is away").expect("an entry");
        assert_eq!(absent.borrow().record.borrow().id, "w_1");
        assert!(Q::live(&*c, "w_1").is_some(), "attach sets it in live");
        let machine = absent.borrow().machine.clone();
        let said = machine.exec("true", Default::default()).await.expect_err("an absent machine answers nothing");
        assert!(said.to_string().contains("the box is away"), "it fails with the words it was given: {said}");
        let dead = Q::attach_dead(&*c, &cell(record("w_2"))).expect("an entry");
        assert_eq!(dead.borrow().machine.id(), "", "the queue's dead machine has no id");
        let dead_again = L::attach_dead(&*c, &dead, "m_gone").expect("an entry");
        assert_eq!(dead_again.borrow().machine.id(), "m_gone");
        assert!(Rc::ptr_eq(&dead_again.borrow().record, &dead.borrow().record), "on the same record");
    });
}

#[test]
fn drop_workspace_takes_the_record_out_of_the_live_map() {
    // waits on packet 4.
    run(options(), |_, c| async move {
        put_live(&c, record("w_1")).await;
        Q::drop_workspace(&*c, "w_1").await.expect("drop");
        assert!(Q::live(&*c, "w_1").is_none());
    });
}

#[test]
fn unfork_of_an_entry_with_no_fork_to_undo_is_no_failure() {
    // waits on packet 4.
    run(options(), |rt, c| async move {
        // The machine is the runtime's own backend's: `unfork` kills it and reads its state there, so a machine from
        // another backend reads as running for good and the kill waits out its three asks (90 s).
        let spec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
        let machine = rt.backend().create(spec, None, None).await.expect("a machine");
        let mut held = record("w_1");
        held.machine_id = machine.id().to_string();
        let entry = put_live_over(&c, held, machine);
        Q::unfork(&*c, &entry).await.expect("queue unfork");
        L::unfork(&*c, &entry).await.expect("life unfork");
    });
}

#[test]
fn nap_with_and_end_sessions_reach_the_machines_area() {
    // waits on packet 4.
    run(options(), |_, c| async move {
        put_live(&c, record("w_1")).await;
        let ended = Rc::new(RefCell::new(Vec::<String>::new()));
        let sink = ended.clone();
        let mut held = session_entry(view("s_1", "w_1", json!({})));
        held.end = Some(Rc::new(move |reason: &str, _, _| sink.borrow_mut().push(reason.to_string())));
        put_row(&c, "turn-1", Rc::new(RefCell::new(held)));
        put_row(&c, "turn-2", row(view("s_2", "w_2", json!({}))));
        L::end_sessions(&*c, "w_1", "the machine is going");
        assert_eq!(*ended.borrow(), ["the machine is going"], "only the workspace's own rows end");
        L::nap_with(&*c, "w_9", "idle").await.expect_err("no workspace w_9 to nap");
        P::nap_with(&*c, "w_9").await.expect_err("no workspace w_9 to nap");
    });
}

#[test]
fn a_status_read_over_an_entry_reaches_the_reach_area() {
    // waits on packet 4: the reach area reads the daemon and the record through the machines context.
    run(options(), |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web"));
        let entry = put_live(&c, record("w_1")).await;
        L::emit_status(&*c, &entry, wsp_wire::views::workspace::ReachState::Napping).await.expect("emit status");
        P::status_now(&*c, &entry).await.expect("status now");
        let checkout = P::read_checkout(&*c, &entry, false).await.expect("read checkout");
        assert!(checkout.is_none(), "a fake machine holds no checkout");
        P::read_lead_of(&*c, &entry);
    });
}

#[test]
fn forget_marks_drops_the_id_from_the_nap_and_stop_refusals_and_the_unreached() {
    // waits on packet 4: the three maps are read through the machines context.
    use wsp_machines::machines::machines::MachinesRuntime;
    use wsp_machines::machines::machines::Unreached;
    use wsp_machines::machines::reach::ForMachine;
    let gone = |machine: &str| Unreached { machine_id: machine.to_string(), line: "the provider cannot reach it".to_string() };
    run(options(), |_, c| async move {
        let held = |said: &str| ForMachine { machine_id: "m_1".to_string(), value: said.to_string() };
        MachinesRuntime::nap_refusals(&*c).borrow_mut().insert("w_1".into(), held("no nap"));
        MachinesRuntime::nap_refusals(&*c).borrow_mut().insert("w_2".into(), held("no nap"));
        MachinesRuntime::stop_refusals(&*c).borrow_mut().insert("w_1".into(), held("no stop"));
        MachinesRuntime::unreached(&*c).borrow_mut().insert("w_1".into(), gone("m_1"));
        MachinesRuntime::unreached(&*c).borrow_mut().insert("w_2".into(), gone("m_2"));
        L::forget_marks(&*c, "w_1");
        assert_eq!(MachinesRuntime::nap_refusals(&*c).borrow().keys().cloned().collect::<Vec<_>>(), ["w_2"]);
        assert!(MachinesRuntime::stop_refusals(&*c).borrow().is_empty());
        assert_eq!(MachinesRuntime::unreached(&*c).borrow().keys().cloned().collect::<Vec<_>>(), ["w_2"]);
        L::forget_marks(&*c, "w_1");
    });
}

#[test]
fn idle_forget_takes_the_workspace_out_of_the_idle_policy() {
    // waits on packet 4: the idle window fires through the machines area.
    let fake = FakeClock::new(0);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        BootRuntime::idle_touch(&*c, "w_1");
        L::idle_forget(&*c, "w_1");
        fake.advance(48 * 3_600_000);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The broker (pipe.ts cutOff, opts.broker).

#[test]
fn broker_lift_asks_the_wired_broker_and_no_broker_is_no_failure() {
    let broker = keeping(None);
    run(RuntimeOptions { broker: Some(broker.clone()), ..options() }, |_, c| async move {
        L::broker_lift(&*c, "w_1");
        L::broker_lift(&*c, "w_2");
    });
    assert_eq!(*broker.lifted.borrow(), ["w_1", "w_2"]);
    run(options(), |_, c| async move {
        L::broker_lift(&*c, "w_1");
    });
}

#[test]
fn cut_off_revokes_the_machine_at_the_broker_and_a_failure_is_warned_and_goes_on() {
    let broker = keeping(Some("the broker is down"));
    run(RuntimeOptions { broker: Some(broker.clone()), ..options() }, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let captured = wsp_core::console::capture();
        Q::cut_off(&*c, &entry).await;
        L::cut_off(&*c, &entry).await;
        assert_eq!(
            captured.lines(),
            vec!["pricing page was not cut off from your computer before its delete: the broker is down".to_string(); 2]
        );
    });
    assert_eq!(*broker.revoked.borrow(), ["w_1", "w_1"], "the workspace's id, asked once by each trait");
    run(options(), |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let captured = wsp_core::console::capture();
        Q::cut_off(&*c, &entry).await;
        assert!(captured.lines().is_empty(), "no broker, nothing to cut off");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// What a failed held start tells (sub-queue.ts:229), starts and deletes (sub-queue.ts:1164, sub-life.ts:delete).

#[test]
fn a_failed_start_with_no_one_to_tell_tells_no_one() {
    // waits on packet 11: `notify_end` is the threads area's.
    run(options(), |_, c| async move {
        let held = view("s_1", "w_1", json!({ "threadId": "t_1", "status": "failed" }));
        Q::notify_failed(&*c, &held, "turn-1", &[], None, None, "the start was dropped");
    });
}

#[test]
fn a_start_over_a_workspace_the_host_does_not_hold_is_refused() {
    // waits on packet 13: `sessions.start` is the sessions area's; and on packet 5 for the boot it waits on.
    run(options(), |_, c| async move {
        let mut asked = serde_json::Map::new();
        asked.insert("prompt".into(), json!("write the page"));
        asked.insert("as".into(), json!({ "threadId": "t_1", "turnId": "turn-1" }));
        let refused = Q::start_turn(&*c, "w_9", asked, None).await.expect_err("no workspace w_9");
        assert!(refused.message().contains("no workspace"), "the refusal's words: {}", refused.message());
    });
}

#[test]
fn standing_up_a_queued_record_of_a_project_the_host_does_not_hold_fails() {
    // waits on packet 6: `stand_up_queued` is the sub-spawn area's.
    run(options(), |_, c| async move {
        let queued = cell(cloud_record("w_1", json!({ "phase": "queued" })));
        let posted: Rc<dyn Fn()> = Rc::new(|| {});
        Q::stand_up_queued(&*c, &queued, posted, false).await.expect_err("no project p_1");
    });
}

#[test]
fn deleting_a_workspace_the_host_does_not_hold_is_refused() {
    // waits on packet 9: `workspaces.delete` is the phases road.
    run(options(), |_, c| async move {
        L::delete_workspace(&*c, "w_9").await.expect_err("no workspace w_9");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// PullRequestsCtx (pull-requests.ts:9-300, runtime.ts:444).

#[test]
fn the_github_cache_is_the_one_the_options_name_and_each_entry_keeps_its_three_fields() {
    run(options(), |_, c| async move {
        assert!(P::cache_get(&*c, "pr:o/r#1").is_none());
        P::cache_set(&*c, "pr:o/r#1", CacheEntry { body: json!({ "state": "open" }), tag: Some("W/\"a\"".into()), fetched_at: 77 });
        P::cache_set(&*c, "pr:o/r#2", CacheEntry { body: json!(null), tag: None, fetched_at: 78 });
        let first = P::cache_get(&*c, "pr:o/r#1").expect("an entry");
        assert_eq!((first.body, first.tag.as_deref(), first.fetched_at), (json!({ "state": "open" }), Some("W/\"a\""), 77));
        let second = P::cache_get(&*c, "pr:o/r#2").expect("an entry");
        assert_eq!((second.tag, second.fetched_at), (None, 78));
        P::cache_delete(&*c, "pr:o/r#1");
        assert!(P::cache_get(&*c, "pr:o/r#1").is_none());
        assert!(P::cache_get(&*c, "pr:o/r#2").is_some());
    });
}

#[test]
fn a_cache_the_options_carry_is_the_one_read() {
    let cache = Rc::new(wsp_core::github_cache::MemoryGitHubCache::default());
    run(RuntimeOptions { github_cache: Some(cache.clone()), ..options() }, |_, c| async move {
        P::cache_set(&*c, "page:o/r#1", CacheEntry { body: json!(1), tag: None, fetched_at: 5 });
    });
    use wsp_core::github_cache::GitHubCache as _;
    assert_eq!(cache.get("page:o/r#1").expect("the entry the context wrote").fetched_at, 5);
}

#[test]
fn a_host_with_no_daemon_on_this_computer_refuses_with_the_no_host_cli_code() {
    run(options(), |_, c| async move {
        let work: Box<dyn FnOnce(Ask, String) -> futures::future::LocalBoxFuture<'static, Result<Value, AccountError>>> =
            Box::new(|_, _| Box::pin(async { Ok(json!({ "ran": true })) }));
        let refused = P::on_this_computer(&*c, work).await.expect_err("no daemon here");
        assert!(refused.is_no_host_cli(), "the daemon's refusal keeps its code: {}", refused.message());
        assert_eq!(refused.message(), "this host runs no daemon on this computer");
    });
}

#[test]
fn a_project_the_host_does_not_hold_is_refused_and_a_record_names_its_checkout() {
    run(options(), |_, c| async move {
        let refused = P::project_held(&*c, "p_9").expect_err("no project p_9");
        assert!(!refused.message.is_empty());
        let local = record("w_1");
        assert_eq!(P::checkout_of(&*c, &local).ok(), P::checkout_of(&*c, &local).ok(), "a record's checkout reads the same twice");
    });
}

#[test]
fn an_entry_the_boot_area_does_not_hold_is_refused() {
    // waits on packet 5 and what its start waits on (2, 3, 4, 8, 9, 14): `entry_of` awaits the boot.
    run(options(), |_, c| async move {
        let refused = P::entry_of(&*c, "w_9", None).await.err().expect("no workspace w_9");
        assert!(refused.message.contains("no workspace"), "the refusal's words: {}", refused.message);
        let entry = put_live(&c, record("w_1")).await;
        let found = P::entry_of(&*c, "w_1", None).await.expect("a live workspace");
        assert!(Rc::ptr_eq(&found, &entry));
    });
}

#[test]
fn a_window_watching_the_status_is_what_status_watched_says() {
    // waits on packet 15: `ctx.status_tracker` is filled there.
    run(options(), |_, c| async move {
        assert!(!P::status_watched(&*c), "nobody has a window open on a fresh runtime");
    });
}

#[test]
fn a_settle_over_threads_the_host_does_not_hold_is_refused() {
    // waits on packet 5: `mark` is the boot area's.
    run(options(), |_, c| async move {
        P::mark_settled(&*c, vec!["t_9".into()], 5_000).await.expect_err("no thread t_9");
    });
}

#[test]
fn no_slate_watches_a_pull_request_on_a_fresh_runtime() {
    // waits on packet 16: the slates area is built there.
    run(options(), |_, c| async move {
        assert!(!P::slates_watch_pr(&*c, "w_1"));
    });
}

#[test]
fn a_with_daemon_over_a_machine_the_runtime_cannot_dial_fails_with_the_machines_words() {
    // waits on packet 4: the reach of the entry's workspace is read through the machines context.
    run(options(), |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let work: Box<dyn FnOnce(Ask) -> futures::future::LocalBoxFuture<'static, Result<Value, AccountError>>> =
            Box::new(|_| Box::pin(async { Ok(json!(1)) }));
        P::with_daemon(&*c, &entry, work).await.expect_err("nothing answers on a fake machine");
    });
}

#[test]
fn queued_runs_the_works_of_one_workspace_one_at_a_time_and_a_refusal_keeps_its_code() {
    run(options(), |_, c| async move {
        let order = Rc::new(RefCell::new(Vec::<&'static str>::new()));
        let (a, b) = (order.clone(), order.clone());
        let first = P::queued(
            &*c,
            "w_1",
            Box::pin(async move {
                a.borrow_mut().push("first start");
                tokio::task::yield_now().await;
                tokio::task::yield_now().await;
                a.borrow_mut().push("first end");
                Ok(json!("one"))
            }),
        );
        let second = P::queued(
            &*c,
            "w_1",
            Box::pin(async move {
                b.borrow_mut().push("second");
                Err(AccountError::Refusal(wsp_core::types::internal::DaemonRefusal {
                    code: Some("no-host-cli".into()),
                    message: "no gh".into(),
                }))
            }),
        );
        let (one, two) = futures::future::join(first, second).await;
        assert_eq!(one.expect("the first answers its own result"), json!("one"));
        let refused = two.expect_err("the second answers its own refusal");
        assert_eq!(*order.borrow(), ["first start", "first end", "second"]);
        assert!(refused.is_no_host_cli(), "the daemon's code is not lost on the way through the queue: {}", refused.message());
    });
}
