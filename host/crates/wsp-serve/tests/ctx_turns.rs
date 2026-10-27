// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 12 (#2476): the context's bodies for `TurnsCtx` (61 methods, turns.ts:17-866, runtime.ts:467),
//! `ForksCtx` (25, forks.ts:1-240) and `ConversationsCtx` (21, conversations.ts:68-273, runtime.ts:482), read against the TS
//! at 65f6b89c7: the three area files' reads of `ctx` and the twelve TS tests the plan names (runtime-steer, steer-images,
//! steer-lines, steer-unread, turn-costs, turn-plan, rewind, codex-subagents, conversations, owed-lines, permission-relay,
//! aside).
//!
//! Those twelve send, steer, rewind and fork through `rt.sessions` (packets 11 and 13), and a Rust test cannot replace an area, so
//! each method is read at its own seam on the runtime `create_runtime` builds over `FakeBackend` and the sqlite store in a temp
//! folder, seeded through the handles the traits hand out. A test that needs another packet's bodies says which ("waits on
//! packet N") and is red until that packet lands.
mod support;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, ConversationRoad, ConversationStore, EarlierAsk, EarlierRead, HarnessAdapter, HarnessSession, HarnessStartOptions,
    KeptAgent, PermissionAsk, StoreError, StoredConversation,
};
use wsp_core::clock::FakeClock;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions};
use wsp_core::reach::Frame;
use wsp_core::types::api::Runtime as _;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_core::types::internal::{SessionEntry, ThreadRecord, TurnAsked};
use wsp_core::types::wiring::{LiveWorkspace, Local, LocalWiring, Platform, RuntimeOptions, WorkspaceRecord};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions, FakeGuest};
use wsp_engine::machine::{MachineBackend, PreviewReach};
use wsp_machines::machines::boot::BootRuntime;
use wsp_machines::machines::machines::{Entry, Row};
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::{StateWriter, Store};
use wsp_threads::threads::conversations::ConversationsCtx as V;
use wsp_threads::threads::forks::ForksCtx as F;
use wsp_threads::threads::turns::TurnsCtx as T;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::{SessionStatus, SessionView};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::Kind;

// ---------------------------------------------------------------------------------------------------------------
// Fixtures.

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("a folder")
}

/// The sqlite store over the temp folder, the road production takes.
fn sqlite(dir: &tempfile::TempDir) -> Rc<dyn Store> {
    Rc::new(sqlite_store(
        &dir.path().join("state.json"),
        StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ))
}

fn options(dir: &tempfile::TempDir) -> RuntimeOptions {
    RuntimeOptions {
        backend: Some(Arc::new(FakeBackend::new(FakeBackendOptions::default()))),
        store: Some(sqlite(dir)),
        ..Default::default()
    }
}

/// A runtime that has joined computers: the place door is wired, so the `place` kind has its module.
fn with_door(dir: &tempfile::TempDir) -> RuntimeOptions {
    let wiring = PlaceWiring {
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
    };
    RuntimeOptions { place_links: Some(Rc::new(wiring)), ..options(dir) }
}

struct Guest(std::path::PathBuf);

#[async_trait(?Send)]
impl FakeGuest for Guest {
    fn folder(&self, machine_id: &str) -> std::path::PathBuf {
        self.0.join(machine_id)
    }
    fn token_path(&self, machine_id: &str) -> String {
        self.0.join(machine_id).join("token").display().to_string()
    }
    async fn reach(&self, machine_id: &str, _port: u16) -> EngineResult<PreviewReach> {
        Ok(PreviewReach { url: format!("http://127.0.0.1:1/{machine_id}"), token: String::new(), expires_at: JsNumber(1.0) })
    }
}

/// Fake machines that run their commands in a folder under `dir`.
fn guest_options(dir: &tempfile::TempDir) -> FakeBackendOptions {
    FakeBackendOptions { guest: Some(Arc::new(Guest(dir.path().join("machines")))), ..Default::default() }
}

/// A runtime with a local computer over such machines, so git runs for real.
fn with_local(dir: &tempfile::TempDir) -> RuntimeOptions {
    let backend: Arc<dyn MachineBackend> = Arc::new(FakeBackend::new(guest_options(dir)));
    let local = LocalWiring {
        backend: backend.clone(),
        folder: dir.path().join("local").display().to_string(),
        exec_stream: Rc::new(|_, _| unimplemented!("no turn runs in these tests")),
        home: Rc::new(|_| "/home/me/.claude".to_string()),
        home_dir: "/home/me".into(),
        roots_path: "/home/me/.wsp/roots".into(),
        env: Rc::new(IndexMap::new),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: Platform::Linux,
        close: None,
    };
    RuntimeOptions { backend: Some(backend), local: Some(local), ..options(dir) }
}

/// The runtime on a throwaway state, inside the task set the owner thread runs, and its context.
fn run<Fut: Future<Output = ()>>(opts: RuntimeOptions, f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    tokio::task::LocalSet::new().block_on(&rt, async move {
        let runtime = create_runtime(opts);
        let ctx = runtime.context();
        f(runtime, ctx).await
    });
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

/// A record with no kind named, which reads as a workspace on this computer.
fn record(id: &str) -> WorkspaceRecord {
    let mut record = cloud_record(id, json!({}));
    record.kind = serde_json::from_value(json!("local")).expect("a kind");
    record
}

fn project(id: &str, name: &str, path: &str) -> wsp_wire::views::workspace::ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": "zingzys-mac", "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
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

fn thread(workspace: &str, extra: Value) -> ThreadRecord {
    let mut base = json!({ "workspaceId": workspace, "harness": "claude" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a thread record")
}

fn put_thread(c: &Rc<Ctx>, id: &str, workspace: &str, extra: Value) {
    BootRuntime::thread_records(&**c).borrow_mut().insert(id.to_string(), thread(workspace, extra));
}

/// A live entry over `record` on a fake machine, put in the runtime's live map through the boot area's handle.
async fn put_live(c: &Rc<Ctx>, record: WorkspaceRecord) -> Entry {
    put_live_with(c, record, FakeBackendOptions::default()).await
}

/// `put_live` on a machine of the given fake backend, so a guest it carries runs the machine's commands.
async fn put_live_with(c: &Rc<Ctx>, record: WorkspaceRecord, options: FakeBackendOptions) -> Entry {
    use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
    let machine = FakeBackend::new(options).get_fake(&record.machine_id).await.expect("a fake machine");
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

/// What the bus carries of one kind, as JSON, and the listener that keeps it.
fn listen(runtime: &AssembledRuntime, kind: &str) -> (Rc<RefCell<Vec<Value>>>, wsp_core::types::events::Unlisten) {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let sink = seen.clone();
    let listener: wsp_core::types::events::EventListener = Rc::new(move |e: &EventUnion| {
        sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
        Ok(())
    });
    (seen, runtime.events().on(kind, listener))
}

fn event(value: Value) -> EventUnion {
    serde_json::from_value(value).expect("an event")
}

fn session_event(extra: Value) -> wsp_wire::views::session_events_more::SessionEvent {
    let mut base = json!({ "type": "session.notify", "workspaceId": "w_1", "sessionId": "s_1", "turnId": "turn-1", "threadId": "t_1", "notify": "me", "text": "done" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a session event")
}

/// Lets the host's task set run what `spawn` handed it.
async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

fn git(cwd: &std::path::Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .status()
        .expect("git runs")
        .success();
    assert!(ok, "git {args:?} failed");
}

// ---------------------------------------------------------------------------------------------------------------
// TurnsCtx: the core's members (runtime.ts:491-992) and the transcript (transcripts.ts:336, 677).

#[test]
fn turns_the_core_handles_are_the_one_shared_state_the_other_traits_hold() {
    let fake = FakeClock::new(4_000);
    let d = dir();
    run(RuntimeOptions { clock: Some(fake.clock()), ..options(&d) }, |_, c| async move {
        assert_eq!(T::clock(&*c).now(), 4_000);
        fake.advance(1_500);
        assert_eq!(T::clock(&*c).now(), 5_500);
        assert!(Rc::ptr_eq(&T::sessions(&*c), &F::sessions(&*c)) && Rc::ptr_eq(&T::sessions(&*c), &V::sessions(&*c)));
        assert!(Rc::ptr_eq(&T::records(&*c), &F::thread_records(&*c)));
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({}))));
        assert_eq!(T::sessions(&*c).borrow().keys().cloned().collect::<Vec<_>>(), ["s_1"]);
        let entry = put_live(&c, record("w_1")).await;
        assert!(Rc::ptr_eq(&T::live_entry(&*c, "w_1").expect("w_1 is live"), &entry), "the one entry, not a copy");
        assert!(T::live_entry(&*c, "w_2").is_none());
        let ran = Rc::new(Cell::new(0));
        let hook = ran.clone();
        T::held_at_close(&*c).borrow_mut().push(Rc::new(move || hook.set(hook.get() + 1)));
        T::held_at_close(&*c).borrow().iter().for_each(|each| each());
        assert_eq!(ran.get(), 1, "a second handle sees the first one's push");
        T::lead_asks(&*c).borrow_mut().insert("t_1".into(), ask("a_1"));
        assert!(T::lead_asks(&*c).borrow().contains_key("t_1"));
        let landing: wsp_core::context::Joined = async {}.boxed_local().shared();
        T::checkpoints_landing(&*c).borrow_mut().insert("t_1".into(), landing);
        assert_eq!(T::checkpoints_landing(&*c).borrow().keys().cloned().collect::<Vec<_>>(), ["t_1"]);
    });
}

fn ask(id: &str) -> PermissionAsk {
    serde_json::from_value(json!({ "askId": id, "toolName": "Bash", "input": "{}", "options": [] })).expect("an ask")
}

#[test]
fn turns_latest_on_is_the_threads_newest_row_and_a_tie_goes_to_the_later_one() {
    // waits on packet 11: ThreadsArea reads the rows through ThreadsCtx::sessions (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        assert!(T::latest_on(&*c, "t_1").is_none());
        for (key, thread, at) in [("a", "t_1", 100), ("b", "t_1", 300), ("c", "t_1", 200), ("d", "t_2", 900)] {
            put_row(&c, key, row(view(key, "w_1", json!({ "threadId": thread, "startedAt": at }))));
        }
        assert_eq!(T::latest_on(&*c, "t_1").expect("a row").id, "b");
        put_row(&c, "e", row(view("e", "w_1", json!({ "threadId": "t_1", "startedAt": 300 }))));
        assert_eq!(T::latest_on(&*c, "t_1").expect("a row").id, "e");
    });
}

#[test]
fn turns_closing_is_false_until_the_close_has_begun_and_true_after() {
    // waits on packet 17: `close` sets the state's closing first and never clears it (runtime.ts:1218).
    let d = dir();
    run(options(&d), |runtime, c| async move {
        assert!(!T::closing(&*c));
        runtime.close().await.expect("the close");
        assert!(T::closing(&*c));
    });
}

#[test]
fn turns_record_stamps_the_event_keeps_its_row_and_tells_the_bus_and_open_thread_reads_it_back() {
    let fake = FakeClock::new(7_000);
    let d = dir();
    run(RuntimeOptions { clock: Some(fake.clock()), ..options(&d) }, |runtime, c| async move {
        let (seen, _keep) = listen(&runtime, "session.notify");
        for text in ["one", "two"] {
            T::record(&*c, session_event(json!({ "text": text })));
        }
        T::record(&*c, session_event(json!({ "text": "other thread", "threadId": "t_2" })));
        T::record(&*c, session_event(json!({ "text": "other workspace", "workspaceId": "w_2" })));
        let heard = seen.borrow().clone();
        assert_eq!(
            heard.iter().map(|e| (e["at"].clone(), e["pos"].clone())).take(2).collect::<Vec<_>>(),
            [(json!(7_000), json!(1)), (json!(7_000), json!(2))]
        );
        let events = T::open_thread(&*c, "w_1", "t_1").await.expect("opened");
        let texts: Vec<Value> = events.iter().map(|e| serde_json::to_value(e).expect("an event")["text"].clone()).collect();
        assert_eq!(texts, [json!("one"), json!("two")], "this thread's rows of this workspace, in order");
        assert!(T::open_thread(&*c, "w_1", "t_9").await.expect("opened").is_empty());
    });
}

#[test]
fn turns_emit_rings_the_event_and_pass_only_delivers_it() {
    let d = dir();
    run(options(&d), |runtime, c| async move {
        let (seen, _keep) = listen(&runtime, "host.notice");
        let start = runtime.events().since(None, None);
        T::emit(&*c, event(json!({ "type": "host.notice", "message": "one" })));
        assert_eq!(runtime.events().since(Some(start.head), Some(&start.stream)).events.len(), 1, "emit puts the event on the ring");
        T::pass(&*c, event(json!({ "type": "host.notice", "message": "two" })));
        let after = runtime.events().since(Some(start.head), Some(&start.stream));
        assert_eq!((after.events.len(), after.head), (1, start.head + 1), "pass puts nothing on the ring");
        assert_eq!(seen.borrow().iter().map(|e| e["message"].clone()).collect::<Vec<_>>(), [json!("one"), json!("two")]);
    });
}

#[test]
fn turns_persist_sessions_writes_a_live_workspaces_rows_and_threads_and_nothing_for_one_that_is_not() {
    let d = dir();
    let store = sqlite(&d);
    run(RuntimeOptions { store: Some(store.clone()), ..options(&d) }, |_, c| async move {
        put_live(&c, record("w_1")).await;
        put_thread(&c, "t_1", "w_1", json!({}));
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1" }))));
        put_row(&c, "s_2", row(view("s_2", "w_2", json!({ "threadId": "t_2" }))));
        T::persist_sessions(&*c, "w_1").await;
        T::persist_sessions(&*c, "w_2").await;
        let doc = store.get(wsp_core::types::internal::SESSIONS, "w_1").await.expect("a read").expect("the document");
        let ids: Vec<&str> = doc["sessions"].as_array().expect("sessions").iter().map(|s| s["id"].as_str().expect("an id")).collect();
        assert_eq!(ids, ["s_1"], "only this workspace's rows");
        assert!(doc["threads"]["t_1"].is_object() && doc["threads"]["t_1"].get("workspaceId").is_none());
        assert!(store.get(wsp_core::types::internal::SESSIONS, "w_2").await.expect("a read").is_none(), "w_2 is not live");
    });
}

#[test]
fn turns_persist_record_writes_the_workspace_and_its_name() {
    let d = dir();
    let store = sqlite(&d);
    run(RuntimeOptions { store: Some(store.clone()), ..options(&d) }, |_, c| async move {
        T::persist_record(&*c, record("w_1")).await.expect("persisted");
        assert_eq!(store.get("workspaces", "w_1").await.expect("a read").expect("the record")["name"], json!("pricing page"));
        assert_eq!(store.get("workspace-names", "w_1").await.expect("a read").expect("the name row")["workspaceId"], json!("w_1"));
    });
}

#[test]
fn turns_slates_turn_ended_answers_ok_for_a_thread_no_slate_watches() {
    // waits on packet 16: `Slates::turn_ended` runs through the slates area's context (ctx/preferences.rs, `todo!()` until packet 16).
    let d = dir();
    run(options(&d), |_, c| async move {
        T::slates_turn_ended(&*c, "t_1", "turn-1").await.expect("no slate, no failure");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// TurnsCtx: checkpoints, changes and the rows a turn settles (threads.ts:629-641, 870-1130).

#[test]
fn turns_keep_checkpoint_records_the_anchor_the_turn_names_and_nothing_where_it_names_none() {
    // waits on packet 11: ThreadsArea reads its context through ThreadsCtx (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |runtime, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let (seen, _keep) = listen(&runtime, "session.checkpoint");
        let asked = |turn: &str, anchor: Option<&str>| wsp_threads::threads::threads::CheckpointAsk {
            session_id: "s_1".into(),
            thread_id: "t_1".into(),
            turn_id: turn.into(),
            anchor: anchor.map(str::to_string),
            kept: None,
        };
        T::keep_checkpoint(&*c, &entry, asked("turn-1", None)).await;
        assert!(seen.borrow().is_empty(), "no ref, no anchor, nothing kept: no row");
        T::keep_checkpoint(&*c, &entry, asked("turn-2", Some("a_1"))).await;
        let heard = seen.borrow().clone();
        assert_eq!(heard.len(), 1, "an anchor makes a row even where no ref was taken");
        let row = &heard[0];
        assert_eq!(
            (&row["workspaceId"], &row["threadId"], &row["turnId"], &row["sessionId"], &row["anchor"]),
            (&json!("w_1"), &json!("t_1"), &json!("turn-2"), &json!("s_1"), &json!("a_1"))
        );
    });
}

#[test]
fn turns_read_turn_changes_answers_false_where_no_snapshot_can_be_read() {
    // waits on packet 11: ThreadsArea reads its context through ThreadsCtx (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let ask = wsp_threads::threads::threads::TurnChangesAsk {
            session_id: "s_1".into(),
            turn_id: "turn-1".into(),
            thread_id: "t_1".into(),
            cwd: "/work/pricing".into(),
            from: "abc".into(),
            started_at: 0,
            wrote: None,
        };
        assert!(!T::read_turn_changes(&*c, &entry, ask).await.expect("an answer, not a failure"));
    });
}

#[test]
fn turns_settle_cut_fails_or_interrupts_the_row_it_is_handed_whether_or_not_sessions_keys_it_by_its_id() {
    // waits on packet 11: ThreadsArea reads the rows through ThreadsCtx::sessions (ctx/agents.rs, `todo!()` until packet 11).
    let fake = FakeClock::new(9_000);
    let d = dir();
    run(RuntimeOptions { clock: Some(fake.clock()), ..options(&d) }, |_, c| async move {
        let running = |id: &str| row(view(id, "w_1", json!({ "threadId": "t_1", "status": "running", "startedAt": 100 })));
        let (failed, stopped, keyed_by_turn, loose) = (running("s_1"), running("s_2"), running("s_3"), running("s_4"));
        put_row(&c, "s_1", failed.clone());
        put_row(&c, "s_2", stopped.clone());
        put_row(&c, "turn-s_3", keyed_by_turn.clone());
        T::lead_asks(&*c).borrow_mut().insert("t_1".into(), ask("a_1"));
        T::settle_cut(&*c, &failed, "the box went away", false, false);
        assert_eq!(failed.borrow().view.status, SessionStatus::Failed);
        assert_eq!(failed.borrow().view.ended_at.map(|at| at.0), Some(9_000.0));
        assert!(T::lead_asks(&*c).borrow().get("t_1").is_none(), "the lead's open ask went with the turn");
        T::settle_cut(&*c, &stopped, "stopped", true, false);
        assert_eq!(stopped.borrow().view.status, SessionStatus::Interrupted);
        T::settle_cut(&*c, &keyed_by_turn, "the box went away", false, false);
        T::settle_cut(&*c, &loose, "the box went away", false, false);
        assert_eq!(
            (keyed_by_turn.borrow().view.status, loose.borrow().view.status),
            (SessionStatus::Failed, SessionStatus::Failed),
            "the TS settles the object it is passed"
        );
        assert_eq!(
            T::sessions(&*c).borrow().keys().cloned().collect::<Vec<_>>(),
            ["s_1", "s_2", "turn-s_3"],
            "no row is left behind for the loose one"
        );
    });
}

#[test]
fn turns_line_try_reads_the_row_it_is_handed_where_sessions_lacks_it() {
    // waits on packet 11: an owed line's request is looked up in `ThreadsCtx::taken` (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        let asking = |owed: Option<bool>| {
            let mut held = session_entry(view("s_1", "w_1", json!({ "threadId": "t_1" })));
            held.asked = Some(TurnAsked {
                prompt: "go".into(),
                effort: None,
                typed: None,
                request_id: Some("r_1".into()),
                owed,
                awaits_prompt: None,
            });
            Rc::new(RefCell::new(held))
        };
        assert!(!T::line_try(&*c, &row(view("s_2", "w_1", json!({})))), "a row that asked nothing");
        assert!(!T::line_try(&*c, &asking(None)), "an ask that is not an owed line is not a try");
        let owed = asking(Some(true));
        assert!(T::line_try(&*c, &owed), "no turn has taken its request, and no map holds the row");
        put_row(&c, "s_1", owed.clone());
        assert!(T::line_try(&*c, &owed));
    });
}

#[test]
fn turns_notify_end_tells_the_person_where_no_thread_can_be_told_and_the_thread_where_one_can() {
    // waits on packet 11: delivering an owed line runs through the threads area's context (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |runtime, c| async move {
        let (seen, _keep) = listen(&runtime, "session.notify");
        let held = row(view("s_1", "w_1", json!({ "threadId": "t_1", "claudeSessionId": "cs_1" })));
        let result = || -> wsp_wire::views::session_events::TurnResult {
            serde_json::from_value(json!({ "status": "completed", "text": "the page is done" })).expect("a result")
        };
        T::notify_end(&*c, &held, &["t_lead".to_string()], result(), false);
        let heard = seen.borrow().clone();
        assert_eq!(heard.len(), 1, "t_lead has no session to tell, so the person is told in its place");
        assert_eq!(
            (&heard[0]["notify"], &heard[0]["threadId"], &heard[0]["sessionId"], &heard[0]["turnId"]),
            (&json!("me"), &json!("t_1"), &json!("cs_1"), &json!("turn-s_1"))
        );
        assert!(heard[0]["text"].as_str().expect("text").starts_with("thread t_1 finished (completed)"));
        put_row(&c, "lead", row(view("lead", "w_1", json!({ "threadId": "t_lead", "claudeSessionId": "cs_lead" }))));
        T::notify_end(&*c, &held, &["t_lead".to_string()], result(), false);
        assert_eq!(seen.borrow()[1]["notify"], json!("t_lead"), "a thread with a session is told, and the line is recorded for it");
    });
}

#[test]
fn turns_send_back_tells_the_person_of_a_steer_the_agent_never_read_when_the_turn_was_stopped() {
    // waits on packet 11: ThreadsArea reads its context through ThreadsCtx (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |runtime, c| async move {
        let (seen, _keep) = listen(&runtime, "session.notify");
        let mut held = session_entry(view("s_1", "w_1", json!({ "threadId": "t_1", "claudeSessionId": "cs_1" })));
        let steered = wsp_core::types::internal::Steered {
            prompt: "also add a footer".into(),
            started_by: wsp_wire::views::session::SessionOrigin::Person,
            by: None,
            road: None,
            request_id: None,
            attachments: None,
        };
        held.turn_live = Some(wsp_core::types::internal::TurnLive {
            steered: Some(IndexMap::from([("st_1".to_string(), steered)])),
            ..Default::default()
        });
        let held: Row = Rc::new(RefCell::new(held));
        T::send_back(&*c, &held, vec!["st_1".to_string(), "st_unknown".to_string()], true);
        let heard = seen.borrow().clone();
        assert_eq!(heard.len(), 1, "one row, for the one message the turn kept");
        assert_eq!(
            (&heard[0]["notify"], &heard[0]["text"], &heard[0]["threadId"]),
            (&json!("me"), &json!("the agent never read this message: also add a footer"), &json!("t_1"))
        );
        T::send_back(&*c, &row(view("s_2", "w_1", json!({}))), vec!["st_1".to_string()], true);
        assert_eq!(seen.borrow().len(), 1, "a row with no thread tells nobody");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// TurnsCtx: titles, words and the agent (turns.ts:239, 347, 708; agents.ts:228-300).

#[test]
fn turns_mode_label_and_source_of_are_the_tables_words() {
    let d = dir();
    run(options(&d), |_, c| async move {
        assert_eq!(T::mode_label(&*c, "claude", "acceptEdits").as_deref(), Some("Accept edits"));
        assert_eq!(T::mode_label(&*c, "claude", "no-such-mode"), None);
        assert_eq!(T::mode_label(&*c, "no-such-harness", "default"), None);
        let source = |extra: Value| T::source_of(&*c, &view("s_1", "w_1", extra));
        assert_eq!(source(json!({})).as_deref(), Some("seed"));
        assert_eq!(source(json!({ "titleSource": "auto" })).as_deref(), Some("auto"));
        assert_eq!(source(json!({ "titleSource": "person" })).as_deref(), Some("person"));
    });
}

#[test]
fn turns_name_refresh_and_make_title_answer_ok_where_there_is_nothing_to_rename_read_or_title() {
    // waits on packet 11: AgentsArea reads its context through AgentsCtx (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        put_live(&c, record("w_1")).await;
        T::name_in_harness(&*c, view("s_1", "w_1", json!({})), "pricing page").await.expect("no session to rename");
        T::name_in_harness(&*c, view("s_1", "w_9", json!({ "claudeSessionId": "cs_1" })), "pricing page").await.expect("no such workspace");
        let bare = row(view("s_1", "w_1", json!({ "threadId": "t_1" })));
        T::refresh_title(&*c, &bare, false).await.expect("no session id, nothing to read");
        T::refresh_title(&*c, &bare, true).await.expect("forced, still nothing to read");
        T::make_title(&*c, &bare).await.expect("no prompt, nothing to title");
        let gone = row(view("s_2", "w_9", json!({ "threadId": "t_2", "prompt": "write the page", "claudeSessionId": "cs_2" })));
        T::refresh_title(&*c, &gone, false).await.expect("no such workspace");
        T::make_title(&*c, &gone).await.expect("no such workspace");
    });
}

#[test]
fn turns_hold_ask_and_spawn_run_their_work_to_its_end_on_the_hosts_task_set() {
    let d = dir();
    run(options(&d), |_, c| async move {
        let ran = Rc::new(Cell::new(0));
        let (held, spawned) = (ran.clone(), ran.clone());
        T::hold_ask(&*c, Box::pin(async move { held.set(held.get() + 1) }));
        T::spawn(&*c, Box::pin(async move { spawned.set(spawned.get() + 10) }));
        let slow = ran.clone();
        T::hold_ask(
            &*c,
            Box::pin(async move {
                tokio::task::yield_now().await;
                tokio::task::yield_now().await;
                slow.set(slow.get() + 100);
            }),
        );
        settle().await;
        assert_eq!(ran.get(), 111, "each was started, the slow one carried to its end");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// TurnsCtx: usage (turns.ts:418-487; threads.ts:1149-1165).

#[test]
fn turns_usage_computer_of_is_the_place_this_computer_or_the_provider_a_workspace_forks_at() {
    let d = dir();
    run(options(&d), |_, c| async move {
        assert_eq!(T::usage_computer_of(&*c, &cloud_record("w_1", json!({ "place": "p_box" }))), "p_box");
        assert_eq!(T::usage_computer_of(&*c, &record("w_2")), "here");
        assert_eq!(T::usage_computer_of(&*c, &cloud_record("w_3", json!({ "provider": "hetzner" }))), "hetzner");
        assert_eq!(T::usage_computer_of(&*c, &cloud_record("w_4", json!({}))), "cloud");
    });
}

#[test]
fn turns_usage_account_of_keys_a_named_account_by_its_id_and_an_own_login_by_its_computer() {
    // waits on packet 11: the sign-in a machine runs a harness with is read through `ThreadsCtx::login_stands` and `vault_on`.
    let d = dir();
    run(options(&d), |_, c| async move {
        let entry = put_live(&c, cloud_record("w_1", json!({ "provider": "hetzner" }))).await;
        let named = T::usage_account_of(&*c, &entry, "claude", Some(("acct_1".into(), Some("Work".into()))));
        assert_eq!((named.key.as_str(), named.label.as_deref(), named.road.as_deref()), ("claude:acct_1", Some("Work"), Some("named")));
        let own = T::usage_account_of(&*c, &entry, "claude", None);
        assert_eq!((own.key.as_str(), own.road.as_deref()), ("claude@hetzner", Some("own")));
        assert!(own.label.as_deref().is_some_and(|label| !label.is_empty()), "the label names the agent and the computer");
    });
}

#[test]
fn turns_ledger_add_files_the_turns_use_as_a_row_of_its_day() {
    // waits on packet 15: the ledger is the one `UsageCtx` builds and shares (ctx/usage.rs, `todo!()` until packet 15).
    let d = dir();
    let store = sqlite(&d);
    run(RuntimeOptions { store: Some(store.clone()), ..options(&d) }, |_, c| async move {
        let used = wsp_threads::threads::turns::LedgerUse {
            at: 1_793_016_000_000,
            agent: "claude".into(),
            account: "claude@here".into(),
            account_label: Some("Claude Code".into()),
            computer: "here".into(),
            project: "p_1".into(),
            turns: 1,
            model: Some("claude-opus-5-5".into()),
            tokens: None,
            cost_usd: Some(0.5),
            session: Some("cs_1".into()),
            source: "wsp".into(),
        };
        T::ledger_add(&*c, used).await.expect("filed");
        let index = store.get(wsp_machines::usage::USAGE_INDEX, "days").await.expect("a read").expect("the index of days");
        let day = index["days"][0].as_str().expect("a day").to_string();
        let bytes = store.get_blob(wsp_machines::usage::USAGE_DAYS, &day).await.expect("a read").expect("the day");
        let rows: Value = serde_json::from_slice::<Value>(&bytes).expect("a day's json")["rows"].clone();
        assert_eq!(
            (rows.as_array().map(Vec::len), &rows[0]["account"], &rows[0]["project"], &rows[0]["costUsd"]),
            (Some(1), &json!("claude@here"), &json!("p_1"), &json!(0.5))
        );
    });
}

#[test]
fn turns_ledger_limit_alerts_read_usage_accounts_and_burn_add_go_through_the_usage_area() {
    // waits on packet 15: the ledger, the alerts and the burn are the ones `UsageCtx` builds (ctx/usage.rs, `todo!()` until packet 15).
    let d = dir();
    run(options(&d), |_, c| async move {
        let limit: wsp_wire::types::usage::HarnessLimit =
            serde_json::from_value(json!({ "status": "ok", "windows": [] })).expect("a limit");
        let asked = wsp_threads::threads::turns::LimitAsk {
            key: "claude@here".into(),
            agent: "claude".into(),
            label: Some("Claude Code".into()),
            road: Some("own".into()),
            computer: "here".into(),
            limit,
        };
        let (before, after) = T::ledger_limit(&*c, asked).await.expect("a reading");
        assert!(before.is_null(), "the account held nothing before");
        assert_eq!(after["key"], json!("claude@here"));
        T::alerts_read(&*c, before, after).await.expect("read");
        T::burn_add(
            &*c,
            wsp_threads::threads::turns::BurnAsk {
                account: "claude@here".into(),
                thread_id: "t_1".into(),
                tokens: 1_200.0,
                replayed: None,
            },
        );
        T::burn_add(
            &*c,
            wsp_threads::threads::turns::BurnAsk {
                account: "claude@here".into(),
                thread_id: "t_1".into(),
                tokens: 300.0,
                replayed: Some(("turn-1".into(), 5_000.0)),
            },
        );
        let listed = T::usage_accounts(&*c).await.expect("the accounts");
        assert!(listed.accounts.iter().any(|account| account.key == "claude@here"), "the account that was read is listed");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// TurnsCtx: the machine, the timers and the areas a turn's end reads (turns.ts:60-110, 330-360, 640-866).

#[test]
fn turns_harness_exec_runs_a_command_on_the_entrys_machine_and_answers_its_stdout() {
    let d = dir();
    run(options(&d), |_, c| async move {
        let entry = put_live_with(&c, record("w_1"), guest_options(&d)).await;
        let exec = T::harness_exec(&*c, &entry, 5_000);
        assert_eq!(exec.exec("printf hello", None).await.expect("ran"), "hello");
        assert_eq!(exec.exec("printf partial; exit 3", None).await.expect("a non-zero exit still answers its stdout"), "partial");
        let bare = T::harness_exec(&*c, &put_live(&c, record("w_2")).await, 5_000);
        let failed = bare.exec("true", None).await.expect_err("the fake machine has no guest");
        assert!(failed.message.contains(wsp_engine::fake_backend::FAKE_NO_GUEST), "{}", failed.message);
    });
}

#[test]
fn turns_exec_sweep_asks_the_machines_exec_factory_to_end_every_run_but_those_held() {
    let d = dir();
    run(options(&d), |_, c| async move {
        let entry = put_live_with(&c, cloud_record("w_1", json!({})), guest_options(&d)).await;
        let sweep = T::exec_sweep(&*c, &entry).expect("a machine's exec factory sweeps");
        assert!(sweep(vec!["run_held".to_string()]).await.expect("swept").is_empty(), "no run stands on a fresh machine");
    });
}

#[test]
fn turns_shared_daemon_is_the_kinds_word_for_it() {
    let d = dir();
    run(with_door(&d), |_, c| async move {
        let on_a_computer = put_live(&c, cloud_record("w_1", json!({ "kind": "place" }))).await;
        let cloud = put_live(&c, cloud_record("w_2", json!({}))).await;
        assert!(T::shared_daemon(&*c, &on_a_computer), "a folder on a joined computer shares that computer's daemon");
        assert!(!T::shared_daemon(&*c, &cloud), "a cloud workspace has its own");
    });
}

#[test]
fn turns_turn_groups_are_the_one_map_the_roots_recheck_reads_and_it_is_armed_once() {
    use std::os::unix::process::CommandExt;
    let fake = FakeClock::new(0);
    let d = dir();
    let mut group = std::process::Command::new("sleep").arg("30").process_group(0).spawn().expect("a process group to stand for a turn's");
    let pgid = i64::from(group.id());
    run(RuntimeOptions { clock: Some(fake.clock()), ..options(&d) }, |_, c| async move {
        T::port_roots_moved(&*c, "w_1");
        T::turn_groups(&*c).borrow_mut().insert("w_1".into(), IndexMap::from([(pgid, "turn-1".to_string())]));
        let base = fake.pending();
        T::arm_roots_recheck(&*c);
        T::arm_roots_recheck(&*c);
        assert_eq!(fake.pending(), base + 1, "armed once while a turn group stands");
        fake.advance(wsp_account::account::channels::PORT_ROOTS_RECHECK_MS);
        assert_eq!(fake.pending(), base + 1, "and again: the channels area reads the map the turns area wrote");
        T::turn_groups(&*c).borrow_mut().clear();
        fake.advance(wsp_account::account::channels::PORT_ROOTS_RECHECK_MS);
        assert_eq!(fake.pending(), base, "with no group left it stops");
    });
    group.kill().expect("the group this test started");
    group.wait().expect("reaped");
}

#[test]
fn turns_the_disk_watch_the_idle_policy_held_away_and_the_waits_follow_the_machines() {
    let fake = FakeClock::new(0);
    let d = dir();
    run(RuntimeOptions { clock: Some(fake.clock()), ..options(&d) }, |_, c| async move {
        let base = fake.pending();
        T::arm_disk_watch(&*c);
        assert_eq!(fake.pending(), base, "nothing live, nothing to read");
        let entry = put_live(&c, record("w_1")).await;
        T::read_disk(&*c, &entry).await.expect("nothing to read is not a failure");
        T::idle_hold(&*c, "w_1");
        T::idle_hold(&*c, "w_1");
        T::idle_release(&*c, "w_1");
        T::idle_release(&*c, "w_1");
        T::idle_release(&*c, "w_1");
        T::prompt_held(&*c, "r_1");
        T::agent_retried(&*c, "turn-1", Some("rate limit"));
        assert!(T::is_held_away(&*c, "w_9"), "no live entry: its machine is away");
        assert!(!T::is_held_away(&*c, "w_1"));
    });
}

#[test]
fn turns_end_snooze_for_stamps_the_snoozed_threads_now_and_tells_the_bus() {
    // waits on packets 8, 11, 16 and 17: `BootArea::mark` and the boot's start run through their contexts (ctx/folders.rs, agents.rs, preferences.rs, public.rs).
    let fake = FakeClock::new(50_000);
    let d = dir();
    run(RuntimeOptions { clock: Some(fake.clock()), ..options(&d) }, |runtime, c| async move {
        let (seen, _keep) = listen(&runtime, "thread.marked");
        put_live(&c, record("w_1")).await;
        put_thread(&c, "t_1", "w_1", json!({ "snoozedUntil": 90_000 }));
        put_thread(&c, "t_2", "w_1", json!({}));
        T::end_snooze_for(&*c, &view("s_2", "w_1", json!({ "threadId": "t_2" })));
        settle().await;
        assert!(seen.borrow().is_empty(), "a thread not snoozed marks nothing");
        T::end_snooze_for(&*c, &view("s_1", "w_1", json!({ "threadId": "t_1", "rootThreadId": "t_2" })));
        for _ in 0..200 {
            if !seen.borrow().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(
            seen.borrow().iter().map(|e| e["threadIds"].clone()).collect::<Vec<_>>(),
            [json!(["t_1"])],
            "one mark, for the one thread that was snoozed"
        );
        assert_eq!(
            BootRuntime::thread_records(&*c).borrow()["t_1"].snoozed_until.as_ref().map(|at| at.0),
            Some(50_000.0),
            "the snooze ends now"
        );
    });
}

struct Kept(Rc<Cell<bool>>);

impl KeptAgent for Kept {
    fn next(&self, _turn: wsp_adapters::port::KeptTurn) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("a test agent runs no turns".into()))
    }
    fn close(&self, _now: bool) -> LocalBoxFuture<'static, ()> {
        self.0.set(true);
        Box::pin(async {})
    }
    fn exited(&self) -> wsp_adapters::port::SharedFuture<()> {
        futures::future::pending::<()>().boxed_local().shared()
    }
}

fn kept_hand(closed: &Rc<Cell<bool>>) -> wsp_threads::threads::threads::KeptHand {
    wsp_threads::threads::threads::KeptHand {
        workspace_id: "w_1".into(),
        agent: Box::new(Kept(closed.clone())),
        pid: None,
        launch: Default::default(),
        session: Some("cs_1".into()),
        warm: None,
        turn_token: "tok".into(),
        scope_device_id: None,
        waiting: Rc::new(Cell::new(false)),
    }
}

#[test]
fn turns_hold_kept_hands_back_a_hold_for_an_unknown_thread_and_keeps_the_agent_of_a_known_one() {
    // waits on packet 11: keeping an agent reads the kept-box and session-file state through `ThreadsCtx` (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        let closed = Rc::new(Cell::new(false));
        assert!(T::hold_kept(&*c, "t_9", kept_hand(&closed)).is_some(), "no record for the thread: the turn closes its own agent");
        settle().await;
        assert!(closed.get(), "and the refused agent is closed");
        put_thread(&c, "t_1", "w_1", json!({}));
        let kept = Rc::new(Cell::new(false));
        assert!(T::hold_kept(&*c, "t_1", kept_hand(&kept)).is_none(), "kept");
        settle().await;
        assert!(!kept.get(), "a kept agent is left running for the thread's next turn");
    });
}

#[test]
fn turns_revoke_device_drops_the_device_and_does_not_say_whether_it_stood() {
    let d = dir();
    let store = sqlite(&d);
    run(RuntimeOptions { store: Some(store.clone()), ..options(&d) }, |_, c| async move {
        let door = wsp_core::devices::make_devices(store.clone(), Rc::new(wsp_core::ids::OsIds));
        let scope = serde_json::from_value(json!({ "kind": "thread", "threadId": "t_1", "workspaceId": "w_1", "rootThreadId": "t_1" }))
            .expect("a scope");
        let minted = door.mint("turn", scope, 1_000, wsp_core::devices::MintMore::default()).await.expect("minted");
        T::revoke_device(&*c, &minted.device_id).await.expect("revoked");
        assert!(door.list().await.expect("a list").is_empty());
        T::revoke_device(&*c, &minted.device_id).await.expect("a device already gone is not a failure");
    });
}

fn checkout(branch: &str) -> wsp_wire::types::changes::Checkout {
    serde_json::from_value(json!({ "branch": branch, "ahead": 0, "behind": 0, "changed": 0, "readAt": 0 })).expect("a checkout")
}

#[test]
fn turns_ran_on_branch_records_the_thread_against_the_branch_a_shared_folder_stands_on() {
    let d = dir();
    run(options(&d), |_, c| async move {
        let project = project("p_1", "pricing", "/work/pricing");
        wsp_machines::machines::boot::BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project);
        let entry = put_live(&c, record("w_1")).await;
        T::ran_on_branch(&*c, &entry, "t_1").await.expect("no checkout read yet: nothing to record");
        assert!(entry.borrow().record.borrow().branch_threads.is_none());
        entry.borrow_mut().checkout = Some(checkout("feature"));
        for thread in ["t_1", "t_2", "t_1"] {
            T::ran_on_branch(&*c, &entry, thread).await.expect("recorded");
        }
        let on = entry.borrow().record.borrow().branch_threads.clone().expect("branch threads");
        assert_eq!((on.branch.as_str(), on.threads.as_slice()), ("feature", ["t_1".to_string(), "t_2".to_string()].as_slice()));
        entry.borrow_mut().checkout = Some(checkout("main"));
        T::ran_on_branch(&*c, &entry, "t_3").await.expect("back on the project's branch");
        assert!(entry.borrow().record.borrow().branch_threads.is_none(), "a turn on the base branch clears the record");
        let stray = put_live(&c, cloud_record("w_2", json!({ "kind": "local", "project": "p_missing" }))).await;
        stray.borrow_mut().checkout = Some(checkout("feature"));
        T::ran_on_branch(&*c, &stray, "t_1").await.expect_err("no such project: the refusal, not a silent ok");
    });
}

#[test]
fn turns_the_reads_of_a_workspace_with_nothing_to_read_answer_ok_and_its_pull_request_state_is_its_records() {
    let d = dir();
    run(options(&d), |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        T::read_checkout(&*c, &entry, true).await.expect("no daemon to ask is not a failure");
        assert!(entry.borrow().checkout.is_none());
        T::read_pull_request(&*c, &entry, false).await.expect("nothing to read");
        T::read_lead_of(&*c, &entry).await.expect("a read the area starts itself");
        T::settle_tree(&*c, &entry).await.expect("no tree to settle");
        T::take_review(&*c, &entry, "t_1", "looks fine").await.expect("not a review: nothing to read");
        assert!(entry.borrow().record.borrow().review.is_none());
        assert_eq!(T::pull_request_state(&*c, &entry), None);
        let pr = json!({ "number": 7, "url": "https://github.com/o/r/pull/7", "state": "merged", "base": "main" });
        let merged = put_live(&c, cloud_record("w_2", json!({ "pr": pr }))).await;
        assert_eq!(T::pull_request_state(&*c, &merged).as_deref(), Some("merged"));
    });
}

#[test]
fn turns_drop_images_removes_the_sends_folder_from_the_machine() {
    // waits on packet 11: the area starts the removal through `AgentsCtx::spawn` (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    let images = d.path().join("images-of-one-send");
    run(options(&d), |_, c| async move {
        let entry = put_live_with(&c, record("w_1"), guest_options(&d)).await;
        std::fs::create_dir_all(&images).expect("a folder");
        std::fs::write(images.join("a.png"), b"x").expect("a file");
        T::drop_images(&*c, &entry, images.to_str().expect("a path"));
        for _ in 0..400 {
            if !images.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(!images.exists(), "the folder is removed on the machine once the turn is over");
    });
}

#[test]
fn turns_adapter_for_a_harness_with_no_adapter_is_the_no_adapter_refusal() {
    // waits on packet 11: `AgentsArea::adapter_for` reads the wired adapters through `AgentsCtx::adapters` (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let ask = wsp_threads::threads::agents::launch::AdapterAsk { named: Some("claude".into()), ..Default::default() };
        let refused = T::adapter_for(&*c, &entry, ask).err().expect("no adapter is wired on this runtime");
        assert!(refused.message.contains("claude"), "{}", refused.message);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ForksCtx (forks.ts:1-240).

#[test]
fn forks_the_core_handles_are_the_runtimes_own_live_ids_keep_the_live_maps_order_and_spawn_runs_its_work() {
    let d = dir();
    let store = sqlite(&d);
    run(RuntimeOptions { store: Some(store.clone()), ..options(&d) }, |_, c| async move {
        F::store(&*c).put("things", "a", json!({ "n": 1 })).await.expect("put");
        assert_eq!(store.get("things", "a").await.expect("get"), Some(json!({ "n": 1 })), "the same store, not a copy");
        assert!(Rc::ptr_eq(&F::sessions(&*c), &BootRuntime::sessions(&*c)));
        assert!(Rc::ptr_eq(&F::thread_records(&*c), &BootRuntime::thread_records(&*c)));
        put_thread(&c, "t_b", "w_2", json!({}));
        put_thread(&c, "t_a", "w_1", json!({}));
        assert_eq!(F::thread_records(&*c).borrow().keys().cloned().collect::<Vec<_>>(), ["t_b", "t_a"], "in insertion order");
        assert!(F::live_ids(&*c).is_empty());
        put_live(&c, record("w_2")).await;
        put_live(&c, record("w_1")).await;
        assert_eq!(F::live_ids(&*c), ["w_2", "w_1"], "the live map's own order");
        let done = Rc::new(Cell::new(0));
        let counted = done.clone();
        F::spawn(&*c, Box::pin(async move { counted.set(7) }));
        settle().await;
        assert_eq!(done.get(), 7, "spawned work runs without anyone awaiting it");
    });
}

#[test]
fn forks_the_threads_rows_are_read_across_workspaces() {
    // waits on packet 11: ThreadsArea and AgentsArea read the rows through ThreadsCtx and AgentsCtx (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        let mut launching = session_entry(view("s_3", "w_2", json!({ "threadId": "t_1", "startedAt": 2000 })));
        launching.launch = Some(async {}.boxed_local().shared());
        put_row(&c, "turn-s_3", Rc::new(RefCell::new(launching)));
        put_row(&c, "turn-1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 1000, "status": "running" }))));
        put_row(&c, "turn-2", row(view("s_2", "w_1", json!({ "threadId": "t_2", "startedAt": 3000 }))));
        assert_eq!(F::latest_on(&*c, "t_1").expect("a row").id, "s_3", "the newest of the thread's rows, across workspaces");
        assert!(F::latest_on(&*c, "t_9").is_none());
        let ids: Vec<String> = F::rows_on(&*c, "t_1").into_iter().map(|v| v.id).collect();
        assert_eq!(ids, ["s_3", "s_1"], "in the map's order");
        assert_eq!(F::launching_turn(&*c, "t_1").as_deref(), Some("turn-s_3"));
        assert_eq!(F::launching_turn(&*c, "t_2"), None, "no launch in flight");
        assert_eq!(F::running_turn(&*c, "t_1"), None, "a running status with no handle is no running turn");
    });
}

#[test]
fn forks_ready_and_entry_of_row_answer_the_live_entry_and_refuse_a_workspace_that_is_not_live() {
    // waits on packet 11: the boot's start runs through ThreadsCtx, AgentsCtx and LaunchCtx (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        F::ready(&*c).await.expect("the boot's one start answers ok on a fresh runtime");
        let live = put_live(&c, record("w_1")).await;
        let at = |workspace: &str| wsp_machines::machines::boot::entries::RowAt {
            thread_id: Some("t_1".into()),
            workspace_id: workspace.into(),
        };
        let got =
            F::entry_of_row(&*c, &at("w_1"), None).await.unwrap_or_else(|e| panic!("the live workspace answers its entry: {}", e.message));
        assert!(Rc::ptr_eq(&got, &live), "the entry the live map holds, not a copy");
        let refused = F::entry_of_row(&*c, &at("w_9"), None).await.err().expect("a refusal");
        assert_eq!(refused.kind, Some(Kind::NotFound), "a missing workspace is not found");
        assert!(refused.message.contains("w_9"));
    });
}

#[test]
fn forks_open_thread_started_as_and_folder_of_read_the_transcript_and_the_index() {
    let d = dir();
    run(options(&d), |_, c| async move {
        T::record(&*c, session_event(json!({ "text": "one" })));
        T::record(&*c, session_event(json!({ "text": "other", "threadId": "t_2" })));
        let events = F::open_thread(&*c, "w_1", "t_1").await.expect("the thread's events");
        assert_eq!(events.iter().map(|e| serde_json::to_value(e).expect("an event")["text"].clone()).collect::<Vec<_>>(), [json!("one")]);
        wsp_threads::threads::transcripts::TranscriptsCtx::transcript_state(&*c)
            .with(|s| drop(s.index.entry("w_1".to_string()).or_default().starts.insert("t_1".to_string(), "abc".to_string())));
        assert_eq!(F::started_as(&*c, "w_1", "t_1").as_deref(), Some("abc"));
        assert_eq!((F::started_as(&*c, "w_2", "t_1"), F::started_as(&*c, "w_1", "t_9")), (None, None));
        put_row(&c, "turn-1", row(view("s_1", "w_1", json!({ "claudeSessionId": "abc", "cwd": "/work/a" }))));
        put_row(&c, "turn-2", row(view("s_2", "w_1", json!({ "claudeSessionId": "abc", "cwd": "/work/b" }))));
        assert_eq!(F::folder_of(&*c, "w_1", "abc").as_deref(), Some("/work/b"), "the later row that resumed the session");
        assert_eq!((F::folder_of(&*c, "w_2", "abc"), F::folder_of(&*c, "w_1", "zzz")), (None, None));
        assert_eq!(F::agent_label(&*c, "claude"), "Claude Code");
        assert_eq!(F::agent_label(&*c, "mystery"), "mystery", "an agent the catalog does not know is named by its id");
    });
}

#[test]
fn forks_keep_sent_images_chains_the_kept_document_and_the_blob_under_the_thread() {
    let d = dir();
    run(options(&d), |_, c| async move {
        assert!(F::kept_writes(&*c, "w_1").is_none(), "nothing kept yet");
        let attach = |media: &str, bytes: &str| wsp_wire::types::attachments::Attachment {
            media_type: media.into(),
            bytes: bytes.into(),
            name: None,
        };
        F::keep_sent_images(&*c, "w_1", "t_1", None, vec![attach("image/png", "aGk=")]).await;
        F::keep_sent_images(&*c, "w_1", "t_1", Some("req_2".into()), vec![attach("text/plain", "eA==")]).await;
        assert!(F::kept_writes(&*c, "w_1").is_none(), "no request id or no image: no key, no chain");
        F::keep_sent_images(&*c, "w_1", "t_1", Some("req_1".into()), vec![attach("image/png", "aGk="), attach("text/plain", "eA==")]).await;
        F::kept_writes(&*c, "w_1").expect("the write chain of w_1").await;
        assert!(F::kept_writes(&*c, "w_2").is_none(), "one chain per workspace");
        let doc = F::store(&*c).get(wsp_core::types::internal::ATTACHMENT_KEYS, "w_1").await.expect("a read").expect("a document");
        assert_eq!(doc, json!({ "threads": { "t_1": [{ "key": "t_1.req_1.0", "mediaType": "image/png" }] } }));
        let blob = F::store(&*c).get_blob(wsp_core::types::internal::ATTACHMENTS, "t_1.req_1.0").await.expect("a read");
        assert_eq!(blob, Some(b"hi".to_vec()), "the image's bytes, decoded");
    });
}

#[test]
fn forks_persist_sessions_marks_a_thread_worked_once_one_of_its_turns_ran_and_only_for_a_live_workspace() {
    let d = dir();
    run(options(&d), |_, c| async move {
        put_live(&c, record("w_1")).await;
        for (id, thread) in [("t_1", "w_1"), ("t_2", "w_1"), ("t_3", "w_2")] {
            put_thread(&c, id, thread, json!({ "worked": false }));
        }
        put_row(&c, "turn-1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "claudeSessionId": "abc" }))));
        put_row(&c, "turn-2", row(view("s_2", "w_1", json!({ "threadId": "t_2" }))));
        put_row(&c, "turn-3", row(view("s_3", "w_2", json!({ "threadId": "t_3", "claudeSessionId": "abc" }))));
        F::persist_sessions(&*c, "w_1").await;
        F::persist_sessions(&*c, "w_2").await;
        let worked: Vec<Option<bool>> = ["t_1", "t_2", "t_3"].iter().map(|id| F::thread_records(&*c).borrow()[*id].worked).collect();
        assert_eq!(worked, [Some(true), Some(false), Some(false)], "a turn that announced no session is not work, and w_2 is not live");
    });
}

#[test]
fn forks_project_held_and_checkout_of_read_the_held_project_and_refuse_one_that_is_not() {
    let d = dir();
    run(options(&d), |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".into(), project("p_1", "web", "/work/web"));
        assert_eq!(F::project_held(&*c, "p_1").expect("a held project").name, "web");
        assert!(F::project_held(&*c, "p_9").expect_err("a refusal for a project not held").message.contains("p_9"));
        let mut in_tree = record("w_2");
        in_tree.worktree =
            Some(serde_json::from_value(json!({ "path": "/work/web-wt", "branch": "fork", "made": true })).expect("a worktree"));
        assert_eq!(F::checkout_of(&*c, &record("w_1")).expect("the folder"), "/work/web");
        assert_eq!(F::checkout_of(&*c, &in_tree).expect("the folder"), "/work/web-wt", "the checkout is the worktree's folder");
        let mut stray = record("w_3");
        stray.project = "p_9".into();
        assert!(F::checkout_of(&*c, &stray).expect_err("a refusal").message.contains("p_9"));
    });
}

#[test]
fn forks_place_name_names_the_here_place_by_its_computer() {
    // waits on packet 14: the area reads the place door and the host name through BuildersCtx (ctx/builders.rs, `todo!()` until packet 14).
    let d = dir();
    run(with_door(&d), |_, c| async move {
        assert_eq!(F::place_name(&*c, wsp_wire::types::place_word::HERE_PLACE_ID), "zingzys-mac");
    });
}

#[test]
fn forks_git_here_runs_git_in_the_folder_on_the_local_machine() {
    let d = dir();
    let (repo, plain) = (d.path().join("repo"), d.path().join("plain"));
    std::fs::create_dir_all(&repo).expect("a repo folder");
    std::fs::create_dir_all(&plain).expect("a plain folder");
    git(&repo, &["init", "-q"]);
    run(with_local(&d), |_, c| async move {
        let args = || vec!["rev-parse".to_string(), "--show-toplevel".to_string()];
        let inside = F::git_here(&*c, &repo.display().to_string(), args()).await.expect("git answers");
        assert_eq!(
            (inside.exit_code.get(), inside.stdout.trim()),
            (0, std::fs::canonicalize(&repo).expect("the repo").to_str().expect("a path"))
        );
        let outside = F::git_here(&*c, &plain.display().to_string(), args()).await.expect("git answers");
        assert_ne!(outside.exit_code.get(), 0, "a folder outside any repo is not a checkout");
    });
}

#[test]
fn forks_worktree_folder_take_setup_line_and_remove_worktree_go_through_the_folders_area() {
    // waits on packet 8: the folders area makes and removes the worktree through FoldersCtx and Base (ctx/folders.rs, `todo!()` until packet 8).
    let d = dir();
    run(options(&d), |_, c| async move {
        let web = project("p_1", "web", "/repo/web");
        let made =
            F::worktree_folder(&*c, &web, "/repo/web", "fork-branch", "w_1", Some("root_1".into()), "abc123").await.expect("the worktree");
        let tree = made.borrow().record.borrow().worktree.clone().expect("a worktree record");
        assert_eq!((tree.branch.as_deref(), tree.made, tree.made_for.as_deref()), (Some("fork-branch"), true, Some("root_1")));
        assert_eq!(F::take_setup_line(&*c, "w_9"), None, "a workspace no worktree left a line for");
        let plain = put_live(&c, record("w_2")).await;
        F::remove_worktree(&*c, &plain, true).await.expect("an entry with no made worktree has nothing to take away");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ConversationsCtx (conversations.ts:68-273).

/// An adapter with no conversation store, and one whose store lets go a minute later.
struct Stub(bool);

struct Listed;

impl ConversationStore for Listed {
    fn list(&self, _cwds: Vec<String>, _road: ConversationRoad) -> LocalBoxFuture<'static, Result<Vec<StoredConversation>, StoreError>> {
        async { Ok(Vec::new()) }.boxed_local()
    }
    fn earlier(&self, _id: String, _ask: EarlierAsk, _road: ConversationRoad) -> LocalBoxFuture<'static, Result<EarlierRead, StoreError>> {
        unimplemented!()
    }
    fn lets_go(&self) -> Option<String> {
        Some("It lets go a minute later.".to_string())
    }
}

impl HarnessAdapter for Stub {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        unimplemented!()
    }
    fn steers(&self) -> bool {
        false
    }
    fn conversations(&self) -> Option<Arc<dyn ConversationStore>> {
        self.0.then(|| Arc::new(Listed) as Arc<dyn ConversationStore>)
    }
}

/// Adapters for the named agents, each with a store or none.
fn adapters(named: &[(&str, bool)]) -> IndexMap<String, HarnessAdapterFactory> {
    named
        .iter()
        .map(|(id, store)| {
            let store = *store;
            (
                id.to_string(),
                Arc::new(move |_: HarnessAdapterContext| Ok(Arc::new(Stub(store)) as Arc<dyn HarnessAdapter>)) as HarnessAdapterFactory,
            )
        })
        .collect()
}

/// A channel that answers each frame with itself, as an accepted reply, and never ends.
struct Echo;

#[async_trait(?Send)]
impl DaemonChannel for Echo {
    async fn send(&self, mut frame: Frame) -> Result<Frame, ChannelError> {
        frame.insert("ok".to_string(), json!(true));
        Ok(frame)
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending::<ChannelEnd>().await
    }
}

#[test]
fn conversations_the_core_handles_are_the_runtimes_own_and_live_project_reads_the_record_as_it_is_now() {
    let fake = FakeClock::new(1_000);
    let d = dir();
    run(RuntimeOptions { clock: Some(fake.clock()), ..options(&d) }, |runtime, c| async move {
        assert_eq!(V::clock(&*c).now(), 1_000);
        fake.advance(250);
        assert_eq!(V::clock(&*c).now(), 1_250, "a handle reads the time the test moved, not a copy of it");
        assert!(Rc::ptr_eq(&V::bus(&*c), &runtime.events()), "the area's bus is the runtime's bus");
        assert!(Rc::ptr_eq(&V::sessions(&*c), &BootRuntime::sessions(&*c)));
        V::transcript_state(&*c).with(|s| {
            let index = wsp_core::types::internal::TranscriptIndex {
                starts: IndexMap::from([("cs".to_string(), "t_1".to_string())]),
                ..Default::default()
            };
            s.index.insert("w_1".to_string(), index);
        });
        assert_eq!(
            V::transcript_state(&*c).with(|s| s.index["w_1"].starts.get("cs").cloned()),
            Some("t_1".to_string()),
            "a second handle reads what the first wrote"
        );
        let entry = put_live(&c, record("w_1")).await;
        assert_eq!((V::live_project(&*c, "w_1"), V::live_project(&*c, "w_9")), (Some("p_1".to_string()), None));
        entry.borrow().record.borrow_mut().project = "p_2".to_string();
        assert_eq!(V::live_project(&*c, "w_1"), Some("p_2".to_string()), "the record at the moment the forget runs");
    });
}

#[test]
fn conversations_with_daemon_sends_the_frame_on_the_entrys_own_channel_and_harness_exec_runs_on_its_machine() {
    let d = dir();
    let dialled = Rc::new(RefCell::new(Vec::<(String, String)>::new()));
    let log = dialled.clone();
    let opener: Rc<dyn Fn(DaemonChannelOptions) -> Local<'static, Rc<dyn DaemonChannel>>> = Rc::new(move |o| {
        log.borrow_mut().push((o.url.clone(), o.token.clone()));
        Box::pin(async { Ok(Rc::new(Echo) as Rc<dyn DaemonChannel>) })
    });
    let road = wsp_wire::views::workspace::DaemonReachView {
        url: "ws://127.0.0.1:7".into(),
        expires_at: JsNumber(9.0e15),
        daemon_token: Some("tok-7".into()),
    };
    let mut local = with_local(&d);
    local.local.as_mut().expect("a local computer").daemon_road = Some(Rc::new(move || {
        let road = road.clone();
        Box::pin(async move { Ok(road) })
    }));
    run(RuntimeOptions { daemon_channel: Some(opener), ..local }, |_, c| async move {
        let entry = put_live_with(&c, record("w_1"), guest_options(&d)).await;
        let reply = V::with_daemon(&*c, &entry, json!({ "op": "transcripts.list" }))
            .await
            .unwrap_or_else(|e| panic!("the daemon's reply: {}", e.message()));
        assert_eq!(reply["op"], json!("transcripts.list"), "the frame reaches the daemon and its answer comes back");
        assert_eq!(*dialled.borrow(), [("ws://127.0.0.1:7".to_string(), "tok-7".to_string())]);
        assert_eq!(V::harness_exec(&*c, &entry, 30_000).exec("printf hi", None).await.expect("the stdout"), "hi");
    });
}

#[test]
fn conversations_project_held_worktrees_of_and_folders_of_read_the_project_and_its_folders() {
    let root = dir();
    let (folder, tree) = (root.path().join("lab"), root.path().join("lab-wt"));
    std::fs::create_dir(&folder).expect("a folder");
    git(&folder, &["init", "-q", "-b", "main"]);
    git(&folder, &["commit", "-q", "--allow-empty", "-m", "first"]);
    git(&folder, &["worktree", "add", "-q", "-b", "feat/x", tree.to_str().expect("a path")]);
    let (folder, tree) = (std::fs::canonicalize(&folder).expect("the folder"), std::fs::canonicalize(&tree).expect("the worktree"));
    let d = dir();
    run(with_local(&d), |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".to_string(), project("p_1", "lab", "/work/lab"));
        assert_eq!(V::project_held(&*c, "p_1").expect("the held project").path, "/work/lab");
        let missing = V::project_held(&*c, "p_9").expect_err("a refusal for a project not held");
        assert_eq!(missing.message, wsp_words::projects::no_such_project_line("p_9", &["lab"]));
        // waits on packet 8: `git_here` runs through Base on Ctx (ctx/folders.rs, `todo!()` until packet 8).
        let trees = V::worktrees_of(&*c, folder.to_str().expect("a path")).await.expect("the worktrees");
        let listed: Vec<(String, Option<String>)> = trees.iter().map(|t| (t.path.clone(), t.branch.clone())).collect();
        assert_eq!(
            listed,
            [(folder.display().to_string(), Some("main".to_string())), (tree.display().to_string(), Some("feat/x".to_string()))]
        );
        let plain = root.path().join("plain");
        std::fs::create_dir(&plain).expect("a folder");
        assert!(V::worktrees_of(&*c, plain.to_str().expect("a path")).await.expect("no error for a plain folder").is_empty());
        for (id, project, kind) in [("w_1", "p_1", "local"), ("w_3", "p_2", "local"), ("w_2", "p_1", "local"), ("w_4", "p_1", "cloud")] {
            put_live(&c, cloud_record(id, json!({ "kind": kind, "project": project }))).await;
        }
        let ids: Vec<String> = V::folders_of(&*c, "p_1").into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["w_1", "w_2"], "the project's records that run in a folder, in live order");
    });
}

#[test]
fn conversations_has_adapter_agent_off_and_conversation_store_read_the_adapters_the_runtime_was_given() {
    // waits on packet 11: `agent_off` and `adapter_for` read `setup_off` and `adapters()` (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(RuntimeOptions { adapters: adapters(&[("claude", false), ("codex", true)]), ..options(&d) }, |_, c| async move {
        assert!(V::has_adapter(&*c, "claude") && !V::has_adapter(&*c, "cursor"));
        let entry = put_live(&c, record("w_1")).await;
        assert!(!V::agent_off(&*c, &entry, "claude"), "an agent the person never turned off");
        assert!(V::conversation_store(&*c, &entry, "claude").is_none(), "an adapter with no store reads as none");
        assert!(V::conversation_store(&*c, &entry, "cursor").is_none(), "an agent with no adapter reads as none");
        assert_eq!(
            V::conversation_store(&*c, &entry, "codex").expect("the codex store").lets_go().as_deref(),
            Some("It lets go a minute later.")
        );
    });
}

#[test]
fn conversations_agent_label_computer_of_and_reaches_row_are_the_areas_words_and_reach() {
    let d = dir();
    run(options(&d), |_, c| async move {
        assert_eq!((V::agent_label(&*c, "claude"), V::agent_label(&*c, "codex")), ("Claude Code".to_string(), "Codex".to_string()));
        assert_eq!(V::agent_label(&*c, "zed"), "zed", "an agent the catalog does not name reads as its id");
        let entry = put_live(&c, record("w_1")).await;
        assert_eq!(V::computer_of(&*c, &entry), "this computer");
        assert!(
            V::reaches_row(&*c, &view("s_1", "w_9", json!({ "threadId": "t_1" })), None),
            "with no caller, the person's own window reaches the row"
        );
    });
}

#[test]
fn conversations_latest_on_is_the_newest_session_of_the_thread() {
    // waits on packet 11: ThreadsArea reads the rows through ThreadsCtx::sessions (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        for (key, workspace, thread, at) in [("turn-1", "w_1", "t_1", 100), ("turn-2", "w_1", "t_1", 200), ("turn-3", "w_2", "t_2", 300)] {
            put_row(&c, key, row(view(&key[5..], workspace, json!({ "threadId": thread, "startedAt": at }))));
        }
        assert_eq!(V::latest_on(&*c, "t_1").expect("a row").id, "2");
        assert!(V::latest_on(&*c, "t_9").is_none());
    });
}

#[test]
fn conversations_folder_for_places_a_project_on_its_folder_record_and_refuses_one_not_held() {
    // waits on packet 8: the folders area places the thread through FoldersCtx (ctx/folders.rs, `todo!()` until packet 8).
    let d = dir();
    run(options(&d), |_, c| async move {
        BootRuntime::projects_held(&*c).borrow_mut().insert("p_1".to_string(), project("p_1", "lab", "/work/lab"));
        put_live(&c, record("w_1")).await;
        let at = V::folder_for(&*c, Some("p_1".to_string()), None, None).await.expect("a folder for the project");
        assert_eq!(at.entry.borrow().record.borrow().id, "w_1");
        assert!(V::folder_for(&*c, Some("p_9".to_string()), None, None).await.is_err());
    });
}

#[test]
fn conversations_entry_of_is_the_live_entry_and_refuses_an_unknown_workspace_as_not_found() {
    // waits on packet 11: the boot's start runs through ThreadsCtx, AgentsCtx and LaunchCtx (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(options(&d), |_, c| async move {
        let live = put_live(&c, record("w_1")).await;
        assert!(Rc::ptr_eq(&V::entry_of(&*c, "w_1", None).await.expect("the entry"), &live), "the live entry itself, not a copy");
        let refused = V::entry_of(&*c, "w_9", None).await.err().expect("a refusal");
        assert_eq!((refused.message.as_str(), refused.kind), ("no such workspace: w_9", Some(Kind::NotFound)));
    });
}

#[test]
fn conversations_preferences_answer_the_held_record_before_the_store() {
    // waits on packet 16: the preferences read runs through PreferencesCtx (ctx/preferences.rs, `todo!()` until packet 16).
    let d = dir();
    run(options(&d), |_, c| async move {
        let mut held = V::preferences(&*c).await.expect("the stored preferences");
        assert!(!held.labs, "a fresh store is not in labs");
        held.default_agent = Some("codex".to_string());
        wsp_account::account::channels::ChannelsCtx::state(&*c).with(|s| s.preferences_held = Some(held));
        assert_eq!(V::preferences(&*c).await.expect("the held preferences").default_agent.as_deref(), Some("codex"));
    });
}

#[test]
fn conversations_default_agent_of_is_the_prefs_default_where_it_runs_here_else_the_first_that_does() {
    // waits on packet 11: `default_agent_of` reads `adapters()` (ctx/agents.rs, `todo!()` until packet 11).
    let d = dir();
    run(RuntimeOptions { adapters: adapters(&[("claude", false), ("codex", false)]), ..options(&d) }, |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let mut prefs = wsp_wire::views::preferences::preferences_from(None);
        prefs.default_agent = Some("codex".to_string());
        assert_eq!(V::default_agent_of(&*c, &prefs, &entry), "codex");
        prefs.default_agent = Some("cursor".to_string());
        assert_eq!(V::default_agent_of(&*c, &prefs, &entry), "claude", "the default names an agent with no adapter here");
    });
}
