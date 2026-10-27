// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 3 (#2476): the context's bodies for `TranscriptsCtx`, `ChannelsCtx`, `ProcScopesCtx`,
//! `BoxPanelsCtx`, `TrimmedCtx` and `WaitsCtx` (111 methods), read against the TS at 65f6b89c7: runtime.ts:433-438
//! and :469, the area files' own reads of `ctx` (transcripts.ts, channels.ts, proc-scopes.ts, box-panels.ts,
//! trimmed-row.ts, waits.ts) and the six TS tests the plan names (channels, terminal-config, editor-door, ssh-door,
//! serve-daemon, sqlite-transcripts).
//!
//! The TS tests spy on `rt.workspaces` and drive a whole runtime through `createOn`; a Rust test cannot replace an
//! area, so each method is read at its own seam on the runtime `create_runtime` builds over `FakeBackend`, seeded
//! through the handles the traits hand out. A test that needs another packet's bodies says which in its comment
//! ("waits on packet N") and is red until that packet lands.
mod support;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_account::account::box_panels::{BoxPanelsCtx, StartedRow};
use wsp_account::account::channels::{ChannelRow, ChannelsCtx};
use wsp_account::account::proc_scopes::{LatestRow, ProcScopesCtx, ScopeRow};
use wsp_adapters::pending::CodexClaims;
use wsp_core::clock::FakeClock;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions};
use wsp_core::reach::Frame;
use wsp_core::types::api::Runtime;
use wsp_core::types::internal::{SessionEntry, ThreadRecord, ThreadWords, TranscriptIndex};
use wsp_core::types::wiring::{Broker, Local, LocalWiring, Platform, RuntimeOptions, WorkspaceRecord};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::{
    ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions, SnapshotOptions,
};
use wsp_machines::machines::machines::Row;
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::{memory_store, Store};
use wsp_threads::threads::transcripts::TranscriptsCtx;
use wsp_threads::threads::trimmed_row::TrimmedCtx;
use wsp_threads::threads::waits::WaitsCtx;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::DaemonReachView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
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

fn view(id: &str, workspace: &str, extra: Value) -> SessionView {
    let mut base = json!({ "id": id, "workspaceId": workspace, "harness": "claude", "status": "completed" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a session view")
}

fn row(view: SessionView) -> Row {
    Rc::new(RefCell::new(SessionEntry {
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
    }))
}

fn put_row(c: &Rc<Ctx>, key: &str, row: Row) {
    TranscriptsCtx::sessions(&**c).borrow_mut().insert(key.to_string(), row);
}

fn thread(workspace: &str, extra: Value) -> ThreadRecord {
    let mut base = json!({ "workspaceId": workspace, "harness": "claude" });
    for (k, v) in extra.as_object().expect("an object") {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a thread record")
}

fn put_thread(c: &Rc<Ctx>, id: &str, record: ThreadRecord) {
    TranscriptsCtx::thread_records(&**c).borrow_mut().insert(id.to_string(), record);
}

fn record(id: &str) -> WorkspaceRecord {
    serde_json::from_value(json!({
        "id": id, "name": "pricing page", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    }))
    .expect("a workspace record")
}

fn words(last: f64, first: &str) -> ThreadWords {
    ThreadWords { lines: vec![first.to_string()], last: JsNumber(last), first: Some(first.to_string()), ..Default::default() }
}

fn index_of(c: &Rc<Ctx>, workspace: &str, f: impl FnOnce(&mut TranscriptIndex)) {
    TranscriptsCtx::transcript_state(&**c).with(|s| f(s.index.entry(workspace.to_string()).or_default()));
}

fn event(value: Value) -> wsp_wire::wire::events_union::EventUnion {
    wsp_wire::wire::events_union::parse_event(&value).expect("an event")
}

fn delta(workspace: &str, text: &str, pos: u32) -> SessionEvent {
    serde_json::from_value(
        json!({ "type": "session.delta", "workspaceId": workspace, "sessionId": "s", "kind": "text", "text": text, "pos": pos }),
    )
    .expect("a session event")
}

/// A channel that answers each frame with itself and never ends.
struct Echo {
    closed: Cell<bool>,
}

#[async_trait(?Send)]
impl DaemonChannel for Echo {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        Ok(frame)
    }
    fn close(&self) {
        self.closed.set(true);
    }
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending::<ChannelEnd>().await
    }
}

/// A machine whose shell answers `stdout` to every command and keeps the commands it was asked.
struct Shell {
    stdout: String,
    asked: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl Machine for Shell {
    fn id(&self) -> &str {
        "m_shell"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers::default()
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> EngineResult<ExecResult> {
        self.asked.borrow_mut().push(cmd.to_string());
        Ok(ExecResult { exit_code: 0.into(), stdout: self.stdout.clone(), stderr: String::new() })
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> EngineResult<ExecResult> {
        unimplemented!()
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> EngineResult<String> {
        unimplemented!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> EngineResult<()> {
        unimplemented!()
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> EngineResult<()> {
        unimplemented!()
    }
    async fn kill(&self) -> EngineResult<()> {
        unimplemented!()
    }
    async fn state(&self) -> EngineResult<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
    async fn upload_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
}

struct Ready(bool);

#[async_trait(?Send)]
impl Broker for Ready {
    fn ready(&self) -> bool {
        self.0
    }
    async fn codex_claims(&self) -> Result<Option<CodexClaims>, HostError> {
        Ok(None)
    }
}

/// A live entry over `record` on a fake machine, put in the runtime's live map through the machines' own handle.
async fn put_live(c: &Rc<Ctx>, record: WorkspaceRecord) -> wsp_machines::machines::machines::Entry {
    use wsp_core::types::wiring::LiveWorkspace;
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
    let entry: wsp_machines::machines::machines::Entry = Rc::new(RefCell::new(LiveWorkspace {
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
    wsp_machines::machines::machines::MachinesRuntime::live(&**c).borrow_mut().insert(id, entry.clone());
    entry
}

// ---------------------------------------------------------------------------------------------------------------
// TranscriptsCtx (transcripts.ts:17-741, runtime.ts:433).

#[test]
fn transcripts_clock_is_the_options_clock() {
    let fake = FakeClock::new(5_000);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        let clock = TranscriptsCtx::clock(&*c);
        assert_eq!(clock.now(), 5_000);
        fake.advance(250);
        assert_eq!(clock.now(), 5_250, "the one clock the core holds, not a copy");
    });
}

#[test]
fn transcripts_ids_mint_distinct_uuids() {
    run(options(), |_, c| async move {
        let ids = TranscriptsCtx::ids(&*c);
        let (a, b) = (ids.uuid(), ids.uuid());
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(ids.bytes(24).len(), 24);
    });
}

#[test]
fn transcripts_emit_reaches_the_bus_and_a_listener_refusal_comes_back() {
    run(options(), |_, c| async move {
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        let seen = heard.clone();
        let bus = TranscriptsCtx::bus(&*c);
        let _off = bus.on(
            "workspace.gone",
            Rc::new(move |e: &wsp_wire::wire::events_union::EventUnion| {
                seen.borrow_mut().push(serde_json::to_value(e).unwrap()["workspaceId"].as_str().unwrap().to_string());
                Ok(())
            }),
        );
        let gone = json!({ "type": "workspace.gone", "workspaceId": "w_1", "machineId": "m_1", "reason": "x" });
        TranscriptsCtx::emit(&*c, event(gone.clone())).expect("delivered");
        assert_eq!(*heard.borrow(), vec!["w_1".to_string()]);

        let _refuses =
            bus.on("workspace.gone", Rc::new(|_: &wsp_wire::wire::events_union::EventUnion| Err(HostError::new("a listener said no"))));
        let refused = TranscriptsCtx::emit(&*c, event(gone));
        assert_eq!(refused.expect_err("the listener's refusal comes back").message, "a listener said no");
    });
}

#[test]
fn transcripts_store_is_the_options_store() {
    let store = Rc::new(memory_store());
    let opts = RuntimeOptions { store: Some(store.clone()), ..options() };
    run(opts, |_, c| async move {
        store.put("things", "a", json!({ "n": 1 })).await.expect("put");
        assert_eq!(TranscriptsCtx::store(&*c).get("things", "a").await.expect("get"), Some(json!({ "n": 1 })));
    });
}

#[test]
fn transcripts_rows_are_the_stores_rows_and_a_store_without_them_fails_the_call() {
    let dir = tempfile::tempdir().expect("a folder");
    let sqlite = Rc::new(wsp_store::sqlite_store::sqlite_store(
        &dir.path().join("state.json"),
        wsp_store::store::StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ));
    run(RuntimeOptions { store: Some(sqlite.clone()), ..options() }, |_, c| async move {
        let rows = TranscriptsCtx::rows(&*c);
        rows.append("w", &[delta("w", "one", 1), delta("w", "two", 2)]).expect("append");
        let kept = sqlite.transcripts().expect("a sqlite store keeps rows").all("w").expect("all");
        assert_eq!(kept.events.len(), 2, "the context's rows are the store's, not a copy");
        assert_eq!(rows.total("w", None).expect("total").count, 2);
    });
    run(options(), |_, c| async move {
        let rows = TranscriptsCtx::rows(&*c);
        assert!(rows.all("w").is_err(), "the memory store keeps no rows: the call fails, the host does not");
    });
}

#[test]
fn transcripts_state_cells_are_the_one_shared_state() {
    run(options(), |_, c| async move {
        TranscriptsCtx::transcript_state(&*c).with(|s| {
            s.unread.insert("w".to_string());
        });
        assert!(TranscriptsCtx::transcript_state(&*c).with(|s| s.unread.contains("w")));
        let held: wsp_core::context::Joined = futures::FutureExt::shared(futures::FutureExt::boxed_local(async {}));
        TranscriptsCtx::index_flushes(&*c).with(|f| {
            f.insert("w".to_string(), held);
        });
        assert_eq!(TranscriptsCtx::index_flushes(&*c).with(|f| f.keys().cloned().collect::<Vec<_>>()), vec!["w".to_string()]);
    });
}

#[test]
fn transcripts_sessions_and_thread_records_are_the_shared_maps_the_other_traits_read() {
    run(options(), |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({})));
        put_thread(&c, "t_2", thread("w_2", json!({})));
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 10 }))));
        assert_eq!(
            ProcScopesCtx::thread_records(&*c),
            vec![("t_1".to_string(), "w_1".to_string()), ("t_2".to_string(), "w_2".to_string())],
            "a record put through one trait is read through another, in the map's order"
        );
        assert_eq!(ProcScopesCtx::sessions(&*c).len(), 1);
        assert_eq!(ChannelsCtx::thread_ids(&*c), vec!["t_1".to_string(), "t_2".to_string()]);
    });
}

#[test]
fn transcripts_no_workspace_is_live_on_a_fresh_runtime() {
    run(options(), |_, c| async move {
        assert!(!TranscriptsCtx::is_live(&*c, "w_1"));
        assert!(TranscriptsCtx::entry(&*c, "w_1").is_none());
        assert!(!ChannelsCtx::live_has(&*c, "w_1"));
    });
}

#[test]
fn transcripts_a_live_workspace_is_seen_by_entry_is_live_and_live_has() {
    // waits on packet 4: seeded through `MachinesRuntime::live`.
    run(options(), |_, c| async move {
        let put = put_live(&c, record("w_1")).await;
        assert!(TranscriptsCtx::is_live(&*c, "w_1"));
        assert!(ChannelsCtx::live_has(&*c, "w_1"));
        let got = TranscriptsCtx::entry(&*c, "w_1").expect("the entry");
        assert!(Rc::ptr_eq(&got, &put), "the entry the map holds, not a copy");
        assert!(TranscriptsCtx::entry(&*c, "w_2").is_none());
    });
}

#[test]
fn transcripts_closed_reads_the_runtime_state() {
    run(options(), |_, c| async move {
        assert!(!TranscriptsCtx::closed(&*c));
        ChannelsCtx::state(&*c).with(|s| s.closed = true);
        assert!(TranscriptsCtx::closed(&*c));
    });
}

#[test]
fn transcripts_push_head_says_thread_head_for_a_thread_the_host_holds() {
    // waits on packet 5: `BootArea::push_head` reads the thread's facts through the boot context.
    run(options(), |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({})));
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 10 }))));
        let heard = Rc::new(RefCell::new(Vec::<Value>::new()));
        let seen = heard.clone();
        let _off = TranscriptsCtx::bus(&*c).on(
            "thread.head",
            Rc::new(move |e: &wsp_wire::wire::events_union::EventUnion| {
                seen.borrow_mut().push(serde_json::to_value(e).unwrap());
                Ok(())
            }),
        );
        TranscriptsCtx::push_head(&*c, "t_1");
        let heard = heard.borrow();
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0]["threadId"], json!("t_1"));
        assert_eq!(heard[0]["workspaceId"], json!("w_1"));
        drop(heard);
        TranscriptsCtx::push_head(&*c, "t_nobody");
    });
}

#[test]
fn transcripts_rows_on_and_latest_on_read_the_sessions() {
    // waits on packet 11: `AgentsArea::rows_on` and `ThreadsArea::latest_on` read the sessions through their own contexts.
    run(options(), |_, c| async move {
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 10 }))));
        put_row(&c, "s_2", row(view("s_2", "w_1", json!({ "threadId": "t_2", "startedAt": 20 }))));
        put_row(&c, "s_3", row(view("s_3", "w_1", json!({ "threadId": "t_1", "startedAt": 30 }))));
        let on: Vec<String> = TranscriptsCtx::rows_on(&*c, "t_1").into_iter().map(|v| v.id).collect();
        assert_eq!(on, vec!["s_1".to_string(), "s_3".to_string()], "the thread's rows in the map's order");
        assert_eq!(TranscriptsCtx::latest_on(&*c, "t_1").expect("a latest row").id, "s_3");
        assert!(TranscriptsCtx::latest_on(&*c, "t_nobody").is_none());
        assert!(TranscriptsCtx::rows_on(&*c, "t_nobody").is_empty());
    });
}

#[test]
fn transcripts_resumed_fact_reads_the_newest_row_then_the_index() {
    run(options(), |_, c| async move {
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "claudeSessionId": "abc", "cwd": "/old", "model": "sonnet" }))));
        put_row(&c, "s_2", row(view("s_2", "w_1", json!({ "claudeSessionId": "abc", "cwd": "/new" }))));
        assert_eq!(TranscriptsCtx::resumed_fact(&*c, "w_1", "abc", "cwd"), Some("/new".to_string()), "the newest row that carries it");
        assert_eq!(
            TranscriptsCtx::resumed_fact(&*c, "w_1", "abc", "model"),
            Some("sonnet".to_string()),
            "an older row where the newer has none"
        );
        index_of(&c, "w_1", |i| {
            i.facts.insert(
                "zzz".to_string(),
                wsp_core::types::internal_index::SessionFacts { cwd: Some("/indexed".into()), ..Default::default() },
            );
        });
        assert_eq!(TranscriptsCtx::resumed_fact(&*c, "w_1", "zzz", "cwd"), Some("/indexed".to_string()), "no row holds it: the index does");
        assert_eq!(TranscriptsCtx::resumed_fact(&*c, "w_1", "zzz", "effort"), None);
        assert_eq!(TranscriptsCtx::resumed_fact(&*c, "w_2", "abc", "cwd"), None, "another workspace's rows are not read");
    });
}

#[test]
fn transcripts_daemon_token_seed_is_the_option() {
    let seed = "cafef00d".repeat(3);
    run(RuntimeOptions { daemon_token: Some(seed.clone()), ..options() }, |_, c| async move {
        assert_eq!(TranscriptsCtx::daemon_token_seed(&*c), Some(seed));
    });
    run(options(), |_, c| async move {
        assert_eq!(TranscriptsCtx::daemon_token_seed(&*c), None);
    });
}

#[test]
fn transcripts_rotate_daemon_token_writes_the_token_where_the_machine_names_and_says_whether_it_stood() {
    // daemon-token.ts:59: the script is the one-command rotation; whether the machine had a token file is its answer.
    let token = "cafef00d".repeat(3);
    run(options(), |_, c| async move {
        let set = Arc::new(Shell { stdout: "WSP_DAEMON_TOKEN_SET\n".into(), asked: RefCell::default() });
        let own = "/tmp/stand-in/fk_c0ffee/.wsp-daemon-token".to_string();
        assert!(TranscriptsCtx::rotate_daemon_token(&*c, set.clone(), token.clone(), Some(own.clone())).await.expect("answered"));
        let asked = set.asked.borrow().clone();
        assert_eq!(asked.len(), 1);
        assert!(asked[0].contains(&own) && asked[0].contains(&token), "the token goes to the path named: {}", asked[0]);
        let none = Arc::new(Shell { stdout: "WSP_DAEMON_TOKEN_NONE\n".into(), asked: RefCell::default() });
        assert!(!TranscriptsCtx::rotate_daemon_token(&*c, none, token.clone(), None).await.expect("answered"), "no token file stood");
        let bad = TranscriptsCtx::rotate_daemon_token(&*c, set, "not a token".into(), None).await;
        assert!(bad.is_err(), "a token of the wrong shape is refused before any machine is asked");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// WaitsCtx (waits.ts:19-108, runtime.ts:469).

#[test]
fn waits_clock_sessions_and_closing_are_the_cores() {
    let fake = FakeClock::new(7_000);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        assert_eq!(WaitsCtx::clock(&*c).now(), 7_000);
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({}))));
        assert!(WaitsCtx::sessions(&*c).borrow().contains_key("s_1"), "the sessions map every area shares");
        assert!(!WaitsCtx::closing(&*c));
        ChannelsCtx::state(&*c).with(|s| s.closing = true);
        assert!(WaitsCtx::closing(&*c), "read at the moment of each write");
    });
}

#[test]
fn waits_record_writes_the_transcript_row_and_says_it_on_the_bus() {
    // waits on packet 4: the bus delivers to the machines area, whose context reads the live map.
    let fake = FakeClock::new(9_000);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        let heard = Rc::new(RefCell::new(Vec::<Value>::new()));
        let seen = heard.clone();
        let _off = TranscriptsCtx::bus(&*c).on(
            "session.delta",
            Rc::new(move |e: &wsp_wire::wire::events_union::EventUnion| {
                seen.borrow_mut().push(serde_json::to_value(e).unwrap());
                Ok(())
            }),
        );
        WaitsCtx::record(&*c, delta("w_1", "hello", 1));
        let heard = heard.borrow();
        assert_eq!(heard.len(), 1, "the event reached the bus once");
        assert_eq!(heard[0]["workspaceId"], json!("w_1"));
        assert_eq!(heard[0]["at"], json!(9_000), "stamped with the clock's moment");
        let pending = TranscriptsCtx::transcript_state(&*c).with(|s| s.pending.get("w_1").map(|p| p.len()));
        assert_eq!(pending, Some(1), "and kept for the next flush");
    });
}

#[test]
fn every_spawn_starts_its_work_now_and_lets_it_run_to_its_end() {
    // A TS `void f()`: the host's `LocalSet` runs it, as `spawn_detached` does.
    run(options(), |_, c| async move {
        let done = Rc::new(Cell::new(0));
        let work = |n: i32| -> futures::future::LocalBoxFuture<'static, ()> {
            let done = done.clone();
            Box::pin(async move { done.set(done.get() + n) })
        };
        TranscriptsCtx::spawn(&*c, work(1));
        ChannelsCtx::spawn(&*c, work(10));
        ProcScopesCtx::spawn(&*c, work(100));
        BoxPanelsCtx::spawn(&*c, work(1_000));
        tokio::task::yield_now().await;
        assert_eq!(done.get(), 1_111, "each of the four traits' spawn ran its work");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// TrimmedCtx (trimmed-row.ts:9-29).

#[test]
fn trimmed_started_as_reads_the_index_start() {
    run(options(), |_, c| async move {
        index_of(&c, "w_1", |i| {
            i.starts.insert("t_1".to_string(), "sess-1".to_string());
        });
        assert_eq!(TrimmedCtx::started_as(&*c, "w_1", "t_1"), Some("sess-1".to_string()));
        assert_eq!(TrimmedCtx::started_as(&*c, "w_1", "t_2"), None);
        assert_eq!(TrimmedCtx::started_as(&*c, "w_2", "t_1"), None);
    });
}

#[test]
fn trimmed_folder_of_reads_the_cwd_of_the_session() {
    run(options(), |_, c| async move {
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "claudeSessionId": "sess-1", "cwd": "/work/a" }))));
        assert_eq!(TrimmedCtx::folder_of(&*c, "w_1", "sess-1"), Some("/work/a".to_string()));
        assert_eq!(TrimmedCtx::folder_of(&*c, "w_1", "sess-2"), None);
    });
}

#[test]
fn trimmed_thread_words_read_the_index() {
    run(options(), |_, c| async move {
        index_of(&c, "w_1", |i| {
            i.words.insert("t_1".to_string(), words(1_234.0, "build the page"));
        });
        let got = TrimmedCtx::thread_words(&*c, "w_1", "t_1").expect("the thread's words");
        assert_eq!(got.first.as_deref(), Some("build the page"));
        assert_eq!(got.last.0, 1_234.0);
        assert!(TrimmedCtx::thread_words(&*c, "w_1", "t_2").is_none());
        assert!(TrimmedCtx::thread_words(&*c, "w_9", "t_1").is_none());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx (channels.ts:29-934, runtime.ts:437): the index and rows.

#[test]
fn channels_index_reads_starts_cuts_and_facts() {
    run(options(), |_, c| async move {
        index_of(&c, "w_1", |i| {
            i.starts.insert("t_1".to_string(), "sess-1".to_string());
            i.cut.insert("t_1".to_string(), true);
            i.cut.insert("t_2".to_string(), false);
            i.facts.insert(
                "sess-1".to_string(),
                wsp_core::types::internal_index::SessionFacts {
                    cwd: Some("/work/a".into()),
                    permission_mode: Some("plan".into()),
                    model: Some("opus".into()),
                    effort: Some("high".into()),
                    ..Default::default()
                },
            );
        });
        assert_eq!(ChannelsCtx::index_start(&*c, "w_1", "t_1"), Some("sess-1".to_string()));
        assert_eq!(ChannelsCtx::index_start(&*c, "w_1", "t_9"), None);
        assert_eq!(ChannelsCtx::index_cut(&*c, "w_1", "t_1"), Some(true));
        assert_eq!(ChannelsCtx::index_cut(&*c, "w_1", "t_2"), Some(false));
        assert_eq!(ChannelsCtx::index_cut(&*c, "w_1", "t_9"), None);
        for (fact, want) in [("cwd", "/work/a"), ("permissionMode", "plan"), ("model", "opus"), ("effort", "high")] {
            assert_eq!(ChannelsCtx::index_fact(&*c, "w_1", "sess-1", fact), Some(want.to_string()), "{fact}");
        }
        assert_eq!(ChannelsCtx::index_fact(&*c, "w_1", "sess-9", "cwd"), None);
        assert_eq!(ChannelsCtx::index_fact(&*c, "w_1", "sess-1", "nonsense"), None);
        assert_eq!(ChannelsCtx::index_start(&*c, "w_2", "t_1"), None);
    });
}

#[test]
fn channels_session_rows_project_each_row_in_the_maps_order() {
    run(options(), |_, c| async move {
        let first = row(view(
            "s_1",
            "w_1",
            json!({ "claudeSessionId": "sess-1", "cwd": "/work/a", "permissionMode": "plan", "model": "opus", "effort": "high", "status": "running" }),
        ));
        first.borrow_mut().pid = Some(4242);
        first.borrow_mut().run = Some("/tmp/wsp-run/000000000001".to_string());
        put_row(&c, "s_1", first);
        put_row(&c, "s_2", row(view("s_2", "w_2", json!({}))));
        let rows = ChannelsCtx::session_rows(&*c);
        assert_eq!(rows.len(), 2);
        let mut facts = IndexMap::new();
        facts.insert("cwd".to_string(), "/work/a".to_string());
        facts.insert("permissionMode".to_string(), "plan".to_string());
        facts.insert("model".to_string(), "opus".to_string());
        facts.insert("effort".to_string(), "high".to_string());
        assert_eq!(
            rows[0],
            ChannelRow {
                workspace_id: "w_1".into(),
                claude_session_id: Some("sess-1".into()),
                facts,
                status: "running".into(),
                pid: Some(4242),
                run: Some("/tmp/wsp-run/000000000001".into()),
            }
        );
        assert_eq!(rows[1].workspace_id, "w_2");
        assert_eq!(rows[1].claude_session_id, None);
        assert!(rows[1].facts.is_empty(), "a fact the row never recorded is not a key");
        assert_eq!(rows[1].status, "completed");
        assert_eq!(rows[1].pid, None);
    });
}

#[test]
fn channels_thread_record_and_ids_read_the_records() {
    run(options(), |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({ "permissionMode": "plan" })));
        put_thread(&c, "t_2", thread("w_2", json!({})));
        assert_eq!(ChannelsCtx::thread_ids(&*c), vec!["t_1".to_string(), "t_2".to_string()]);
        let got = ChannelsCtx::thread_record(&*c, "t_1").expect("the record");
        assert_eq!(got.workspace_id, "w_1");
        assert_eq!(got.permission_mode.as_deref(), Some("plan"));
        assert!(ChannelsCtx::thread_record(&*c, "t_9").is_none());
    });
}

#[test]
fn channels_latest_harness_is_the_latest_rows() {
    // waits on packet 11: `ThreadsArea::latest_on` reads the sessions through its own context.
    run(options(), |_, c| async move {
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 10, "harness": "claude" }))));
        put_row(&c, "s_2", row(view("s_2", "w_1", json!({ "threadId": "t_1", "startedAt": 20, "harness": "codex" }))));
        assert_eq!(ChannelsCtx::latest_harness(&*c, "t_1"), Some("codex".to_string()));
        assert_eq!(ChannelsCtx::latest_harness(&*c, "t_9"), None);
    });
}

#[test]
fn channels_root_of_walks_to_the_thread_the_record_names() {
    // waits on packet 2: `RulesArea::root_of` reads its context from the kinds file.
    run(options(), |_, c| async move {
        put_thread(&c, "t_child", thread("w_1", json!({ "rootThreadId": "t_root" })));
        assert_eq!(ChannelsCtx::root_of(&*c, "t_child"), "t_root");
        assert_eq!(ChannelsCtx::root_of(&*c, "t_alone"), "t_alone", "a thread no record names is its own root");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx: the kind's module.

/// This computer's own wiring, its daemon road the one given (kinds.ts:383-409).
fn local_wiring(road: Option<DaemonReachView>) -> LocalWiring {
    LocalWiring {
        backend: backend(),
        folder: "/tmp/wsp-local".into(),
        exec_stream: Rc::new(|_, _| unimplemented!("no turn runs in these tests")),
        home: Rc::new(|_| "/home/me/.claude".to_string()),
        home_dir: "/home/me".into(),
        roots_path: "/home/me/.wsp/roots".into(),
        env: Rc::new(IndexMap::new),
        daemon_road: road.map(|road| {
            let held: Rc<dyn Fn() -> Local<'static, DaemonReachView>> = Rc::new(move || {
                let road = road.clone();
                Box::pin(async move { Ok(road) })
            });
            held
        }),
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: Platform::Linux,
        close: None,
    }
}

fn road(url: &str, token: Option<&str>) -> DaemonReachView {
    DaemonReachView { url: url.into(), expires_at: JsNumber(9.0e15), daemon_token: token.map(String::from) }
}

#[test]
fn channels_kind_roads_of_a_cloud_machine_are_the_guests() {
    // waits on packet 2: `KindsArea::module_of` (kinds.ts:335-354).
    run(options(), |_, c| async move {
        let cloud = record("w_1");
        assert_eq!(ChannelsCtx::home_dir(&*c, &cloud), Some("/root".to_string()), "a fork's home is the guest's");
        assert_eq!(ChannelsCtx::turn_road(&*c, WorkspaceKind::Cloud), Some(WorkspaceOrigin::Relayed));
        assert!(!ProcScopesCtx::shared_daemon(&*c, WorkspaceKind::Cloud), "each fork has a daemon of its own");
    });
}

#[test]
fn channels_kind_roads_of_a_local_folder_read_the_local_wiring() {
    // waits on packet 2: `KindsArea::module_of` (kinds.ts:383-409).
    run(RuntimeOptions { local: Some(local_wiring(Some(road("ws://127.0.0.1:9", Some("tok"))))), ..options() }, |_, c| async move {
        let mut local = record("w_1");
        local.kind = WorkspaceKind::Local;
        assert_eq!(ChannelsCtx::home_dir(&*c, &local), Some("/home/me".to_string()));
        assert_eq!(ChannelsCtx::turn_road(&*c, WorkspaceKind::Local), Some(WorkspaceOrigin::Here));
        assert!(ProcScopesCtx::shared_daemon(&*c, WorkspaceKind::Local), "every folder here shares this computer's daemon");
        assert!(ChannelsCtx::local_has_daemon(&*c));
        assert_eq!(ChannelsCtx::local_home_dir(&*c), "/home/me");
        let got = ChannelsCtx::local_road(&*c).await.expect("the road the host wired");
        assert_eq!((got.url.as_str(), got.daemon_token.as_deref()), ("ws://127.0.0.1:9", Some("tok")));
    });
}

#[test]
fn channels_a_local_folder_on_a_host_wired_with_no_local_has_no_module() {
    // waits on packet 2: `moduleOf` throws for a kind nobody wired; the context answers what its caller can use.
    run(options(), |_, c| async move {
        let mut local = record("w_1");
        local.kind = WorkspaceKind::Local;
        assert_eq!(ChannelsCtx::home_dir(&*c, &local), None);
        assert_eq!(ChannelsCtx::turn_road(&*c, WorkspaceKind::Local), None);
        assert!(!ChannelsCtx::local_has_daemon(&*c));
    });
}

#[test]
fn channels_a_host_wired_with_no_daemon_here_says_so() {
    // waits on packet 2: `KindsArea::local_road` (kinds.ts:202).
    let said = "this host wired no daemon for this computer, so nothing on it can be dialled";
    run(options(), |_, c| async move {
        assert_eq!(ChannelsCtx::local_road(&*c).await.expect_err("no daemon here").message(), said);
        assert_eq!(ProcScopesCtx::channel_here(&*c).await.err().expect("no channel").message(), said);
    });
    run(RuntimeOptions { local: Some(local_wiring(None)), ..options() }, |_, c| async move {
        assert!(!ChannelsCtx::local_has_daemon(&*c));
        assert_eq!(ChannelsCtx::local_road(&*c).await.expect_err("no daemon here").message(), said);
    });
}

#[test]
fn proc_scopes_channel_here_dials_the_local_road_with_its_token() {
    // waits on packet 2: `KindsArea::local_road`; channels.ts:146 asks for the road's token before it dials.
    let dialled = Rc::new(RefCell::new(Vec::<(String, String)>::new()));
    let log = dialled.clone();
    let opener: Rc<dyn Fn(DaemonChannelOptions) -> Local<'static, Rc<dyn DaemonChannel>>> = Rc::new(move |o| {
        log.borrow_mut().push((o.url.clone(), o.token.clone()));
        Box::pin(async { Ok(Rc::new(Echo { closed: Cell::new(false) }) as Rc<dyn DaemonChannel>) })
    });
    let opts = RuntimeOptions {
        local: Some(local_wiring(Some(road("ws://127.0.0.1:7", Some("tok-7"))))),
        daemon_channel: Some(opener),
        ..options()
    };
    run(opts, |_, c| async move {
        ProcScopesCtx::channel_here(&*c).await.expect("a channel");
        assert_eq!(*dialled.borrow(), vec![("ws://127.0.0.1:7".to_string(), "tok-7".to_string())]);
    });
    run(RuntimeOptions { local: Some(local_wiring(Some(road("ws://127.0.0.1:7", None)))), ..options() }, |_, c| async move {
        let refused = ProcScopesCtx::channel_here(&*c).await.err().expect("a road with no token dials nothing");
        assert!(refused.message().ends_with("is not answering yet"), "{}", refused.message());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx: the place door.

#[test]
fn channels_without_a_door_every_place_read_is_empty() {
    run(options(), |_, c| async move {
        assert!(!ChannelsCtx::has_door(&*c));
        assert_eq!(ChannelsCtx::offer_of(&*c, "pl_1"), None);
        assert!(ChannelsCtx::settings_at(&*c, "pl_1").is_none());
        assert_eq!(ChannelsCtx::turn_limit_at(&*c, "pl_1"), None);
        assert!(!ChannelsCtx::joined(&*c, "pl_1"));
        assert_eq!(ChannelsCtx::name_of(&*c, "pl_1"), "pl_1", "channels.ts:125: `placeDoor?.nameOf(place) ?? place`");
        assert!(ChannelsCtx::report_of(&*c, "pl_1").await.is_none());
        assert!(ChannelsCtx::rows(&*c).await.is_empty());
        assert!(ChannelsCtx::link_channel(&*c, "pl_1", Rc::new(|_| {})).is_none());
    });
}

#[test]
fn channels_with_a_door_every_place_read_is_the_doors() {
    // waits on packet 5: the place door answers its rows through the boot context.
    run(with_door(), |runtime, c| async move {
        assert!(ChannelsCtx::has_door(&*c));
        let door = runtime.places().expect("the door");
        for place in ["pl_1", wsp_wire::types::place_word::HERE_PLACE_ID] {
            assert_eq!(ChannelsCtx::name_of(&*c, place), door.name_of(place), "name_of {place}");
            assert_eq!(ChannelsCtx::joined(&*c, place), door.joined(place), "joined {place}");
            assert_eq!(ChannelsCtx::offer_of(&*c, place), door.offer_of(place), "offer_of {place}");
            assert_eq!(
                serde_json::to_value(ChannelsCtx::settings_at(&*c, place)).unwrap(),
                serde_json::to_value(Some(door.settings_at(place))).unwrap(),
                "settings_at {place}"
            );
            assert_eq!(
                ChannelsCtx::turn_limit_at(&*c, place),
                door.turn_limit_at(place).map(|limit| limit.map(|n| n as i64)),
                "turn_limit_at {place}"
            );
        }
        let rows = door.rows().await.expect("the rows");
        assert_eq!(serde_json::to_value(ChannelsCtx::rows(&*c).await).unwrap(), serde_json::to_value(rows).unwrap());
        assert!(ChannelsCtx::report_of(&*c, "pl_1").await.is_none(), "a place the door lists no report for");
        assert!(ChannelsCtx::link_channel(&*c, "pl_1", Rc::new(|_| {})).is_none(), "no link stands to pl_1");
    });
}

#[test]
fn channels_spend_today_asks_the_status_tracker_for_each_place() {
    // waits on packet 15: `ctx.status_tracker` is filled there.
    run(options(), |_, c| async move {
        assert!(ChannelsCtx::spend_today(&*c, &[], 1_000).await.expect("a spend").is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx: dials, this computer, this process.

#[test]
fn channels_open_channel_goes_through_the_options_opener_when_one_is_wired() {
    let asked = Rc::new(RefCell::new(Vec::<(String, String)>::new()));
    let log = asked.clone();
    let opener: Rc<dyn Fn(DaemonChannelOptions) -> wsp_core::types::wiring::Local<'static, Rc<dyn DaemonChannel>>> = Rc::new(move |o| {
        log.borrow_mut().push((o.url.clone(), o.token.clone()));
        (o.on_event)(json!({ "type": "daemon.hello", "version": 1 }));
        Box::pin(async { Ok(Rc::new(Echo { closed: Cell::new(false) }) as Rc<dyn DaemonChannel>) })
    });
    run(RuntimeOptions { daemon_channel: Some(opener), ..options() }, |_, c| async move {
        let events = Rc::new(RefCell::new(Vec::<Value>::new()));
        let heard = events.clone();
        let channel =
            ChannelsCtx::open_channel(&*c, "ws://127.0.0.1:9", "tok", Rc::new(move |e| heard.borrow_mut().push(e))).await.expect("opened");
        let mut frame = Frame::new();
        frame.insert("op".into(), json!("ping"));
        assert_eq!(channel.send(frame.clone()).await.expect("answered"), frame);
        assert_eq!(*asked.borrow(), vec![("ws://127.0.0.1:9".to_string(), "tok".to_string())]);
        assert_eq!(*events.borrow(), vec![json!({ "type": "daemon.hello", "version": 1 })], "the event callback is the caller's");
    });
}

#[test]
fn channels_open_channel_with_nothing_listening_is_refused_in_words() {
    run(options(), |_, c| async move {
        let refused = ChannelsCtx::open_channel(&*c, "ws://127.0.0.1:1", "tok", Rc::new(|_| {})).await;
        assert!(!refused.err().expect("nothing answers on port 1").message().is_empty());
    });
}

#[test]
fn channels_broker_ready_is_the_brokers_word_and_false_with_none() {
    run(options(), |_, c| async move {
        assert!(!ChannelsCtx::broker_ready(&*c));
    });
    run(RuntimeOptions { broker: Some(Rc::new(Ready(true))), ..options() }, |_, c| async move {
        assert!(ChannelsCtx::broker_ready(&*c));
    });
    run(RuntimeOptions { broker: Some(Rc::new(Ready(false))), ..options() }, |_, c| async move {
        assert!(!ChannelsCtx::broker_ready(&*c));
    });
}

#[test]
fn channels_device_door_is_the_one_the_core_made() {
    run(options(), |_, c| async move {
        let (a, b) = (ChannelsCtx::device_door(&*c), ChannelsCtx::device_door(&*c));
        assert!(Rc::ptr_eq(&a, &b));
        assert!(a.list().await.expect("the paired devices").is_empty());
    });
}

#[test]
fn channels_this_process_is_the_hosts_pid_and_its_parents() {
    run(options(), |_, c| async move {
        assert_eq!(ChannelsCtx::host_pid(&*c), std::process::id() as i64);
        assert_eq!(ChannelsCtx::host_parent_pid(&*c), std::os::unix::process::parent_id() as i64);
        assert!(ChannelsCtx::pid_alive(&*c, std::process::id() as i64));
        assert!(!ChannelsCtx::pid_alive(&*c, 4_194_000), "a pid no process holds");
        assert!(!ChannelsCtx::pid_alive(&*c, i64::MAX), "a number no pid is");
    });
}

#[test]
fn channels_state_path_is_what_the_options_say() {
    run(options(), |_, c| async move {
        assert!(!ChannelsCtx::has_state_path(&*c));
    });
    run(RuntimeOptions { state_path: Some("/var/wsp/state.json".into()), ..options() }, |_, c| async move {
        assert!(ChannelsCtx::has_state_path(&*c));
        // waits on packet 2: the folder is `KindsArea::state_folder`.
        assert_eq!(ChannelsCtx::state_folder(&*c), "/var/wsp");
    });
}

#[test]
fn channels_read_link_names_a_links_target_and_nothing_for_a_file() {
    run(options(), |_, c| async move {
        let dir = tempfile::tempdir().expect("a folder");
        let target = dir.path().join("target");
        std::fs::write(&target, "x").expect("a file");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).expect("a link");
        assert_eq!(ChannelsCtx::read_link(&*c, link.to_str().unwrap()), Some(target.to_str().unwrap().to_string()));
        assert_eq!(ChannelsCtx::read_link(&*c, target.to_str().unwrap()), None, "readlinkSync throws on a file");
        assert_eq!(ChannelsCtx::read_link(&*c, dir.path().join("absent").to_str().unwrap()), None);
    });
}

#[test]
fn channels_agent_bins_are_the_catalogs_in_its_order() {
    run(options(), |_, c| async move {
        let want: Vec<String> = wsp_catalog::catalog::catalog_agents().iter().map(|a| a.bin.clone()).collect();
        assert!(!want.is_empty());
        assert_eq!(ChannelsCtx::agent_bins(&*c), want);
    });
}

#[test]
fn channels_agent_pid_of_reads_the_pid_a_run_wrote_while_it_lives() {
    run(options(), |_, c| async move {
        let dir = tempfile::tempdir().expect("a folder");
        let run = dir.path().join("000000000001");
        let run = run.to_str().unwrap();
        let me = std::process::id();
        std::fs::write(format!("{run}.agent"), format!("{me}\n")).expect("the agent word");
        assert_eq!(ChannelsCtx::agent_pid_of(&*c, run), Some(me as i64));
        std::fs::write(format!("{run}.agent"), "4194000\n").expect("a pid nothing holds");
        assert_eq!(ChannelsCtx::agent_pid_of(&*c, run), None);
        assert_eq!(ChannelsCtx::agent_pid_of(&*c, dir.path().join("absent").to_str().unwrap()), None);
    });
}

#[test]
fn channels_group_exists_follows_a_process_group_while_it_stands() {
    use std::os::unix::process::CommandExt;
    run(options(), |_, c| async move {
        let mut child = std::process::Command::new("sleep").arg("30").process_group(0).spawn().expect("a child in its own group");
        let pgid = child.id() as i64;
        assert!(ChannelsCtx::group_exists(&*c, pgid));
        assert!(ProcScopesCtx::group_exists(&*c, pgid));
        child.kill().expect("kill");
        child.wait().expect("reaped");
        assert!(!ChannelsCtx::group_exists(&*c, pgid));
        assert!(!ProcScopesCtx::group_exists(&*c, pgid));
        assert!(!ChannelsCtx::group_exists(&*c, i64::MAX), "a number no group is");
    });
}

#[test]
fn channels_box_panels_ctx_is_this_context() {
    run(options(), |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({})));
        let panels = ChannelsCtx::box_panels_ctx(&*c);
        assert_eq!(panels.thread_records(), vec![("t_1".to_string(), "w_1".to_string())], "the panels read the maps the context holds");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx: the store.

#[test]
fn channels_store_calls_reach_the_options_store() {
    let store = Rc::new(memory_store());
    run(RuntimeOptions { store: Some(store.clone()), ..options() }, |_, c| async move {
        ChannelsCtx::store_put(&*c, "pty-tokens", "w_1", json!({ "p1": "dev_1" })).await.expect("put");
        assert_eq!(store.get("pty-tokens", "w_1").await.expect("get"), Some(json!({ "p1": "dev_1" })), "the options' store itself");
        assert_eq!(ChannelsCtx::store_get(&*c, "pty-tokens", "w_1").await.expect("get"), Some(json!({ "p1": "dev_1" })));
        assert_eq!(ChannelsCtx::store_get(&*c, "pty-tokens", "w_2").await.expect("get"), None);
        ChannelsCtx::store_put(&*c, "pty-tokens", "w_2", json!({})).await.expect("put");
        assert_eq!(ChannelsCtx::store_keys(&*c, "pty-tokens").await.expect("keys"), vec!["w_1".to_string(), "w_2".to_string()]);
        ChannelsCtx::store_delete(&*c, "pty-tokens", "w_1").await.expect("delete");
        assert_eq!(ChannelsCtx::store_keys(&*c, "pty-tokens").await.expect("keys"), vec!["w_2".to_string()]);
    });
}

#[test]
fn channels_workspace_deleted_listeners_hear_the_bus() {
    // waits on packet 16: the preferences area listens to `workspace.deleted` too.
    run(options(), |_, c| async move {
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        let seen = heard.clone();
        ChannelsCtx::on_workspace_deleted(&*c, Rc::new(move |id| seen.borrow_mut().push(id.to_string())));
        TranscriptsCtx::emit(&*c, event(json!({ "type": "workspace.deleted", "workspaceId": "w_7" }))).expect("emit");
        assert_eq!(*heard.borrow(), vec!["w_7".to_string()]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx: the copy's acts.

#[test]
fn channels_thread_launch_asks_the_core() {
    // waits on packet 17: `Core::thread_launch` reads the workspace through `CoreCtx`, whose bodies are that packet's.
    run(options(), |_, c| async move {
        let entry = put_live(&c, record("w_1")).await;
        let launched = ChannelsCtx::thread_launch(&*c, &entry, "t_1", "t_1").await.expect("a launch");
        assert!(launched.scoped.is_none(), "no broker, no scoped device");
        assert!(launched.env.is_empty());
    });
}

#[test]
fn channels_brokered_env_is_the_token_and_the_repos_git_variables() {
    // waits on packet 4: the entry is seeded through the machines' live map.
    run(options(), |_, c| async move {
        let mut with_repo = record("w_1");
        with_repo.branch_of_record = Some(
            serde_json::from_value(json!({ "repoId": 4242, "owner": "acme", "name": "lab", "branch": "wsp/pricing-page" }))
                .expect("a branch of record"),
        );
        let entry = put_live(&c, with_repo).await;
        let env = ChannelsCtx::brokered_env(&*c, "tok-1", &entry);
        assert_eq!(env.get(wsp_wire::types::env::HOST_TOKEN_ENV).map(String::as_str), Some("tok-1"));
        assert_eq!(env.get("GH_TOKEN").map(String::as_str), Some("tok-1"));
        assert_eq!(env.get("GH_TELEMETRY").map(String::as_str), Some("false"));
        assert_eq!(env.get("CARGO_NET_GIT_FETCH_WITH_CLI").map(String::as_str), Some("true"));
        let bare = put_live(&c, record("w_2")).await;
        assert_eq!(
            ChannelsCtx::brokered_env(&*c, "tok-2", &bare).get("GH_TOKEN").map(String::as_str),
            Some("tok-2"),
            "no branch of record: the same token variables"
        );
    });
}

#[test]
fn channels_terminal_line_writes_the_programs_for_a_shell_of_that_agent() {
    run(options(), |_, c| async move {
        let line = ChannelsCtx::terminal_line(&*c, "codex", Some("#!/bin/sh\nexec codex\n"));
        assert_eq!(line, wsp_machines::machines::brokered::terminal_line("codex", Some("#!/bin/sh\nexec codex\n")));
        assert!(line.contains("mkdir -p"));
        assert_ne!(line, ChannelsCtx::terminal_line(&*c, "claude", None), "each agent's shell has its own line");
    });
}

#[test]
fn channels_persist_writes_the_workspace_row_to_the_store() {
    // waits on packet 2: `RecordsArea::persist` reads its context from the kinds file.
    let store = Rc::new(memory_store());
    run(RuntimeOptions { store: Some(store.clone()), ..options() }, |_, c| async move {
        ChannelsCtx::persist(&*c, &record("w_1")).await.expect("persisted");
        let kept = store.get("workspaces", "w_1").await.expect("get").expect("a row");
        assert_eq!(kept["id"], json!("w_1"));
        assert_eq!(kept["name"], json!("pricing page"));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx::git_origin (channels.ts:914-915): `git -C path remote get-url origin`, synchronous, with a timeout.

fn git(args: &[&str]) {
    let ran = std::process::Command::new("git").args(args).output().expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&ran.stderr));
}

#[test]
fn channels_git_origin_answers_the_exit_status_and_stdout_of_git() {
    run(options(), |_, c| async move {
        let dir = tempfile::tempdir().expect("a folder");
        let repo = dir.path().to_str().unwrap();
        git(&["init", "-q", repo]);
        git(&["-C", repo, "remote", "add", "origin", "https://github.com/acme/lab.git"]);
        assert_eq!(
            ChannelsCtx::git_origin(&*c, repo, 2_000),
            (Some(0), "https://github.com/acme/lab.git\n".to_string()),
            "stdout as git wrote it"
        );
    });
}

#[test]
fn channels_git_origin_of_a_repo_with_no_origin_is_git_s_own_status() {
    run(options(), |_, c| async move {
        let dir = tempfile::tempdir().expect("a folder");
        let repo = dir.path().to_str().unwrap();
        git(&["init", "-q", repo]);
        let (status, out) = ChannelsCtx::git_origin(&*c, repo, 2_000);
        assert_eq!(status, Some(2), "git remote get-url exits 2 for a remote it does not have (node: status 2)");
        assert_eq!(out, "");
    });
}

#[test]
fn channels_git_origin_of_a_folder_that_is_no_repo_or_is_not_there_is_128() {
    run(options(), |_, c| async move {
        let dir = tempfile::tempdir().expect("a folder");
        assert_eq!(ChannelsCtx::git_origin(&*c, dir.path().to_str().unwrap(), 2_000), (Some(128), String::new()));
        assert_eq!(ChannelsCtx::git_origin(&*c, dir.path().join("absent").to_str().unwrap(), 2_000), (Some(128), String::new()));
    });
}

#[test]
fn channels_git_origin_that_outlives_its_timeout_is_stopped_with_no_status() {
    // node: spawnSync(..., { timeout: 300 }) on a repo whose config never reads answers status null, stdout "" after
    // about 300 ms (SIGTERM, ETIMEDOUT), checked under node 22.
    run(options(), |_, c| async move {
        let dir = tempfile::tempdir().expect("a folder");
        let repo = dir.path();
        std::fs::create_dir_all(repo.join(".git/objects")).unwrap();
        std::fs::create_dir_all(repo.join(".git/refs")).unwrap();
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        let made = std::process::Command::new("mkfifo").arg(repo.join(".git/config")).status().expect("mkfifo");
        assert!(made.success());
        let started = std::time::Instant::now();
        let got = ChannelsCtx::git_origin(&*c, repo.to_str().unwrap(), 300);
        let took = started.elapsed();
        assert_eq!(got, (None, String::new()));
        assert!(
            took >= std::time::Duration::from_millis(250) && took < std::time::Duration::from_millis(2_000),
            "stopped at its timeout, took {took:?}"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ChannelsCtx: the host's own.

#[test]
fn channels_now_schedule_and_warn_are_the_hosts() {
    let fake = FakeClock::new(1_000);
    run(RuntimeOptions { clock: Some(fake.clock()), ..options() }, |_, c| async move {
        assert_eq!(ChannelsCtx::now(&*c), 1_000);
        let fired = Rc::new(Cell::new(0));
        let (a, b) = (fired.clone(), fired.clone());
        let _kept = ChannelsCtx::schedule(&*c, 500, Box::new(move || a.set(a.get() + 1)));
        let cancel = ChannelsCtx::schedule(&*c, 800, Box::new(move || b.set(b.get() + 10)));
        cancel();
        fake.advance(499);
        assert_eq!(fired.get(), 0);
        fake.advance(1);
        assert_eq!(fired.get(), 1, "the timer ran at its moment; the cancelled one never does");
        fake.advance(1_000);
        assert_eq!(fired.get(), 1);
        let said = wsp_core::console::capture();
        ChannelsCtx::warn(&*c, "a line for the host's log");
        assert_eq!(said.lines(), vec!["a line for the host's log".to_string()]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// ProcScopesCtx (proc-scopes.ts:28-210, runtime.ts:438).

#[test]
fn proc_scopes_turn_groups_and_kept_agents_and_terminals_are_empty_on_a_fresh_runtime() {
    run(options(), |_, c| async move {
        assert!(ProcScopesCtx::turn_groups(&*c).is_empty());
        assert!(ProcScopesCtx::kept_agents(&*c).is_empty());
        assert!(ProcScopesCtx::thread_terminals(&*c).is_empty());
    });
}

#[test]
fn proc_scopes_sessions_project_the_rows() {
    run(options(), |_, c| async move {
        let running = row(view("s_1", "w_1", json!({ "threadId": "t_1", "status": "running", "startedAt": 1_500 })));
        running.borrow_mut().pid = Some(321);
        put_row(&c, "s_1", running);
        put_row(&c, "s_2", row(view("s_2", "w_2", json!({}))));
        assert_eq!(
            ProcScopesCtx::sessions(&*c),
            vec![
                ScopeRow {
                    workspace_id: "w_1".into(),
                    thread_id: Some("t_1".into()),
                    status: "running".into(),
                    started_at: Some(1_500.0),
                    pid: Some(321)
                },
                ScopeRow { workspace_id: "w_2".into(), thread_id: None, status: "completed".into(), started_at: None, pid: None },
            ]
        );
    });
}

#[test]
fn proc_scopes_latest_on_is_the_latest_rows_workspace_and_cwd() {
    // waits on packet 11: `ThreadsArea::latest_on` reads the sessions through its own context.
    run(options(), |_, c| async move {
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 10, "cwd": "/a" }))));
        put_row(&c, "s_2", row(view("s_2", "w_2", json!({ "threadId": "t_1", "startedAt": 20, "cwd": "/b" }))));
        assert_eq!(ProcScopesCtx::latest_on(&*c, "t_1"), Some(LatestRow { workspace_id: "w_2".into(), cwd: Some("/b".into()) }));
        assert_eq!(ProcScopesCtx::latest_on(&*c, "t_9"), None);
    });
}

#[test]
fn proc_scopes_a_live_record_names_its_kind_place_and_checkout() {
    // waits on packet 4 for the live map.
    run(options(), |_, c| async move {
        assert!(ProcScopesCtx::live_record(&*c, "w_1").is_none());
        let mut held = record("w_1");
        held.place = Some("pl_1".into());
        put_live(&c, held).await;
        let got = ProcScopesCtx::live_record(&*c, "w_1").expect("the record");
        assert_eq!(got.place.as_deref(), Some("pl_1"));
        assert_eq!(got.kind, wsp_wire::views::workspace_kind::WorkspaceKind::Cloud);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// BoxPanelsCtx (box-panels.ts:60-263, channels.ts:518).

#[test]
fn box_panels_session_rows_and_thread_records_project_the_maps() {
    run(options(), |_, c| async move {
        put_thread(&c, "t_1", thread("w_1", json!({})));
        put_row(&c, "s_1", row(view("s_1", "w_1", json!({ "threadId": "t_1", "startedAt": 40 }))));
        put_row(&c, "s_2", row(view("s_2", "w_2", json!({}))));
        assert_eq!(
            BoxPanelsCtx::session_rows(&*c),
            vec![
                StartedRow { workspace_id: "w_1".into(), thread_id: Some("t_1".into()), started_at: Some(40.0) },
                StartedRow { workspace_id: "w_2".into(), thread_id: None, started_at: None },
            ]
        );
        assert_eq!(BoxPanelsCtx::thread_records(&*c), vec![("t_1".to_string(), "w_1".to_string())]);
    });
}

#[test]
fn box_panels_workspace_deleted_listeners_hear_the_bus() {
    // waits on packet 16: the preferences area listens to `workspace.deleted` too.
    run(options(), |_, c| async move {
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        let seen = heard.clone();
        BoxPanelsCtx::on_workspace_deleted(&*c, Rc::new(move |id| seen.borrow_mut().push(id.to_string())));
        TranscriptsCtx::emit(&*c, event(json!({ "type": "workspace.deleted", "workspaceId": "w_3" }))).expect("emit");
        assert_eq!(*heard.borrow(), vec!["w_3".to_string()]);
    });
}

#[test]
fn box_panels_without_a_door_there_is_no_link_no_version_and_no_watch() {
    run(options(), |_, c| async move {
        assert!(BoxPanelsCtx::link(&*c, "pl_1").is_none());
        assert_eq!(BoxPanelsCtx::daemon_version(&*c, "pl_1").await, None);
        assert!(BoxPanelsCtx::watch_sys(&*c, "pl_1").await.is_err());
    });
}

#[test]
fn box_panels_with_a_door_a_place_it_holds_no_link_to_has_none() {
    run(with_door(), |runtime, c| async move {
        let door = runtime.places().expect("the door");
        assert!(BoxPanelsCtx::link(&*c, "pl_1").is_none());
        assert_eq!(BoxPanelsCtx::name_of(&*c, "pl_1"), door.name_of("pl_1"));
        assert_eq!(BoxPanelsCtx::daemon_version(&*c, "pl_1").await, None);
        let refused = BoxPanelsCtx::watch_sys(&*c, "pl_1").await;
        assert!(refused.is_err_and(|said| !said.is_empty()), "the door's own words for a place it cannot watch");
    });
}

#[test]
fn box_panels_scopes_on_a_place_are_empty_with_nothing_owned() {
    run(options(), |_, c| async move {
        assert!(BoxPanelsCtx::scopes_on(&*c, "pl_1").is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The ops over the runtime `create_runtime` builds, as the TS tests drive them over serveRuntime: terminal-config,
// editor-door, ssh-door and serve-daemon.test.ts. The TS replaces `rt.workspaces` with a spy; here the workspace is
// one the runtime holds, so a test that names a workspace waits on the packets that read or create one.

use support::{host_socket, open, request, RecordingSink, TOKEN};
use wsp_serve::serve::door::Served;
use wsp_serve::serve::served::DynRuntime;
use wsp_serve::serve::socket::Socket;
use wsp_serve::serve::{ServeOptions, NO_EDITOR, NO_SSH, NO_TERMINAL_CONFIG};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::preferences::{EDITOR_TICKET_REFUSAL, SSH_TICKET_REFUSAL};
use wsp_wire::wire::place_link::PLACES_TICKET_REFUSAL;

fn served_over(runtime: &Rc<AssembledRuntime>, tweak: impl FnOnce(&mut ServeOptions)) -> Rc<Served> {
    let mut opts = ServeOptions { auth_token: TOKEN.to_string(), devices: Some(runtime.devices()), ..ServeOptions::default() };
    tweak(&mut opts);
    Served::new(runtime.clone() as Rc<DynRuntime>, opts).expect("a host token serves")
}

async fn ticketed(served: &Rc<Served>) -> (Rc<Socket>, Rc<RecordingSink>) {
    let (host, sink) = host_socket(served).await;
    let minted = request(&host, &sink, 90, "ticket.issue", json!({ "purpose": "relay" })).await;
    open(served, &format!("?ticket={}", minted["ticket"].as_str().expect("a ticket")))
}

#[test]
fn terminal_config_a_server_with_no_reader_refuses_the_op_in_one_line() {
    // terminal-config.test.ts: "a server with no reader refuses the op in one line".
    run(options(), |runtime, _| async move {
        let served = served_over(&runtime, |_| {});
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "host.terminalConfig", json!({})).await;
        assert_eq!(reply["ok"], json!(false));
        assert_eq!(reply["error"], json!(NO_TERMINAL_CONFIG));
        assert!(NO_TERMINAL_CONFIG.contains("cannot read the terminal config"));
    });
}

#[test]
fn editor_door_a_server_with_no_editor_door_refuses_the_list_in_one_line() {
    // editor-door.test.ts: "a server with no editor door refuses both ops in one line". The TS spies `workspaces.get` so
    // `editor.open` reaches the door check; here the runtime holds no such workspace, so only the list is asked.
    run(options(), |runtime, _| async move {
        let served = served_over(&runtime, |_| {});
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "editor.list", json!({})).await;
        assert_eq!((reply["ok"].clone(), reply["error"].clone()), (json!(false), json!(NO_EDITOR)));
    });
}

#[test]
fn ssh_door_a_server_with_no_ssh_door_refuses_both_ops_in_one_line() {
    // ssh-door.test.ts: "a server with no ssh door refuses both ops in one line".
    run(options(), |runtime, _| async move {
        let served = served_over(&runtime, |_| {});
        let (socket, sink) = host_socket(&served).await;
        for (id, op, fields) in [(1, "ssh.port", json!({ "workspaceId": "ws_fork" })), (2, "ssh.include", json!({ "on": true }))] {
            let reply = request(&socket, &sink, id, op, fields).await;
            assert_eq!((reply["ok"].clone(), reply["error"].clone()), (json!(false), json!(NO_SSH)), "{op}");
        }
    });
}

#[test]
fn editor_door_and_ssh_door_refuse_a_socket_a_ticket_let_in() {
    // editor-door.test.ts and ssh-door.test.ts:
    // "refuses both ops on a socket a ticket let in", before any door is asked.
    run(options(), |runtime, _| async move {
        let served = served_over(&runtime, |_| {});
        let (relayed, sink) = ticketed(&served).await;
        let cases = [
            ("editor.open", json!({ "workspaceId": "ws_copy", "path": "/Users/dev/web-codex-reviews/src/a.ts" }), EDITOR_TICKET_REFUSAL),
            ("editor.list", json!({}), EDITOR_TICKET_REFUSAL),
            ("ssh.port", json!({ "workspaceId": "ws_fork" }), SSH_TICKET_REFUSAL),
            ("ssh.include", json!({ "on": true }), SSH_TICKET_REFUSAL),
        ];
        for (id, (op, fields, said)) in cases.into_iter().enumerate() {
            let reply = request(&relayed, &sink, id as u64 + 1, op, fields).await;
            assert_eq!(
                (reply["ok"].clone(), reply["error"].clone(), reply["kind"].clone()),
                (json!(false), json!(said), json!("ticket")),
                "{op}"
            );
        }
    });
}

#[test]
fn serve_daemon_a_host_wired_with_no_daemon_here_says_so_in_one_sentence() {
    // serve-daemon.test.ts: "a host wired with no daemon here says so in one sentence".
    run(RuntimeOptions { daemon_token: Some("cafef00d".repeat(3)), ..options() }, |runtime, _| async move {
        let served = served_over(&runtime, |_| {});
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "daemon.open", json!({ "placeId": HERE_PLACE_ID })).await;
        assert_eq!(reply["ok"], json!(false));
        assert!(reply["error"].as_str().is_some_and(|said| said.contains("no daemon")), "{reply}");
    });
}

#[test]
fn serve_daemon_an_unknown_workspace_is_refused_before_anything_is_dialled() {
    // waits on packet 8: `workspaces.get` reads through the folders context. serve-daemon.test.ts: "an unknown
    // workspace is refused before anything is dialled".
    run(options(), |runtime, _| async move {
        let served = served_over(&runtime, |_| {});
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "daemon.open", json!({ "workspaceId": "ws_nobody" })).await;
        assert_eq!(reply["ok"], json!(false));
        assert!(reply["error"].as_str().is_some_and(|said| said.contains("no such workspace")), "{reply}");
    });
}

#[test]
fn serve_daemon_the_hosts_own_road_refuses_a_socket_a_ticket_let_in() {
    // serve-daemon.test.ts: "is the host's own road: a socket let in on a ticket is refused".
    let opts = RuntimeOptions { local: Some(local_wiring(Some(road("ws://127.0.0.1:1", Some("tok"))))), ..options() };
    run(opts, |runtime, _| async move {
        let served = served_over(&runtime, |_| {});
        let (ticketed, sink) = ticketed(&served).await;
        let reply = request(&ticketed, &sink, 1, "daemon.open", json!({ "placeId": HERE_PLACE_ID })).await;
        assert_eq!((reply["ok"].clone(), reply["error"].clone()), (json!(false), json!(PLACES_TICKET_REFUSAL)));
    });
}
