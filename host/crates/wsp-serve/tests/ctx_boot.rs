// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 5 (#2476): what the context answers the boot area (`BootRuntime`, boot.ts:29-1071) and the
//! machines' `LandingRoad`. Each test builds the runtime on a throwaway state over `FakeBackend` and calls the
//! method as the area does. A test that reaches another packet's `todo!()` body fails on it until that packet lands;
//! its comment names the packet it waits for.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_core::clock::{Clock, FakeClock};
use wsp_core::devices::MintMore;
use wsp_core::types::api::Runtime;
use wsp_core::types::events::{EventListener, Unlisten};
use wsp_core::types::internal::{SessionEntry, ThreadRecord};
use wsp_core::types::wiring::{RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::boot::BootRuntime;
use wsp_machines::machines::create::LandingRoad;
use wsp_machines::machines::machines::{Entry, Row};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::store::{memory_store, Store};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::events_union::EventUnion;

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

struct World {
    rt: Rc<AssembledRuntime>,
    c: Rc<Ctx>,
    store: Rc<dyn Store>,
    clock: FakeClock,
}

fn world() -> World {
    let store: Rc<dyn Store> = Rc::new(memory_store());
    let clock = FakeClock::new(1_000_000);
    let opts = RuntimeOptions { backend: Some(backend()), store: Some(store.clone()), clock: Some(clock.clock()), ..Default::default() };
    let rt = create_runtime(opts).expect("a runtime");
    let c = rt.context();
    World { rt, c, store, clock }
}

/// A body that builds its own runtimes, inside the task set the owner thread runs.
fn run_bare<Fut: std::future::Future<Output = ()>>(f: impl FnOnce() -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    tokio::task::LocalSet::new().block_on(&rt, async move { f().await });
}

/// The runtime on a throwaway state, inside the task set the owner thread runs.
fn run<Fut: std::future::Future<Output = ()>>(f: impl FnOnce(World) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move { f(world()).await });
}

fn workspace_record(id: &str) -> Rc<RefCell<WorkspaceRecord>> {
    let record = serde_json::from_value(json!({
        "id": id, "name": "n", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    }))
    .expect("a workspace record");
    Rc::new(RefCell::new(record))
}

fn view(id: &str, thread: Option<&str>, status: &str) -> SessionView {
    let mut v = json!({ "id": id, "workspaceId": "w_1", "harness": "claude", "status": status });
    if let Some(t) = thread {
        v["threadId"] = json!(t);
    }
    serde_json::from_value(v).expect("a session view")
}

fn row(id: &str, thread: &str, status: &str) -> Row {
    Rc::new(RefCell::new(SessionEntry {
        view: view(id, Some(thread), status),
        turn_id: format!("turn-{id}"),
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

/// An event no area of the runtime hears, so a test of the bus reaches no other area's body.
fn notice(message: &str) -> EventUnion {
    serde_json::from_value(json!({ "type": "host.notice", "message": message })).expect("an event")
}

/// A live entry on a machine of the fake backend, made by the boot road's own `attach` (waits on packet 4).
async fn attached(w: &World, id: &str) -> Entry {
    let spec: wsp_engine::machine::MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
    let machine = w.rt.backend().create(spec, None, None).await.expect("a machine");
    BootRuntime::attach(&*w.c, &workspace_record(id), machine).expect("an entry")
}

fn listen(w: &World, kind: &str) -> (Rc<RefCell<Vec<Value>>>, Unlisten) {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let sink = seen.clone();
    let listener: EventListener = Rc::new(move |e: &EventUnion| {
        sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
        Ok(())
    });
    (seen, w.rt.events().on(kind, listener))
}

// ---- the context's own handles --------------------------------------------------------------------------------

#[test]
fn store_is_the_one_the_runtime_was_given() {
    run(|w| async move {
        BootRuntime::store(&*w.c).put("probe", "a", json!({ "n": 1 })).await.expect("put");
        assert_eq!(w.store.get("probe", "a").await.expect("get"), Some(json!({ "n": 1 })));
    });
}

#[test]
fn clock_is_the_options_clock() {
    run(|w| async move {
        assert_eq!(BootRuntime::clock(&*w.c).now(), 1_000_000);
        w.clock.advance(2_500);
        assert_eq!(BootRuntime::clock(&*w.c).now(), 1_002_500);
    });
}

#[test]
fn ids_draw_fresh_values() {
    run(|w| async move {
        let ids = BootRuntime::ids(&*w.c);
        let (a, b) = (ids.uuid(), ids.uuid());
        assert_eq!((a.len(), b.len()), (36, 36));
        assert_ne!(a, b);
        assert_eq!(ids.bytes(12).len(), 12);
        assert!((0.0..1.0).contains(&ids.random()));
    });
}

#[test]
fn places_is_the_one_the_runtime_was_given() {
    run(|w| async move {
        let places = BootRuntime::places(&*w.c);
        assert_eq!(places.wired(), "default");
        assert_eq!(places.list(), vec!["default".to_string()]);
    });
}

#[test]
fn emit_stamps_and_delivers_where_pass_only_delivers() {
    run(|w| async move {
        let (seen, _off) = listen(&w, "host.notice");
        BootRuntime::emit(&*w.c, notice("t_1")).expect("emit");
        BootRuntime::pass(&*w.c, notice("t_2")).expect("pass");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["message"], json!("t_1"));
        assert_eq!(seen[0]["seq"], json!(1));
        assert_eq!(seen[1]["message"], json!("t_2"));
        assert_eq!(seen[1]["seq"], json!(1), "a passed event reads the head and moves nothing");
        let kept = w.rt.events().since(Some(0), None);
        assert_eq!(kept.events.len(), 1, "only the emitted event is retained");
    });
}

#[test]
fn a_listener_that_fails_fails_the_emit() {
    run(|w| async move {
        let failing: EventListener = Rc::new(|_| Err(wsp_words::exit::HostError::new("no")));
        let _off = w.rt.events().on("host.notice", failing);
        let err = BootRuntime::emit(&*w.c, notice("t_1")).expect_err("the listener's failure");
        assert_eq!(err.message, "no");
        let err = BootRuntime::pass(&*w.c, notice("t_1")).expect_err("the listener's failure");
        assert_eq!(err.message, "no");
    });
}

#[test]
fn warn_writes_one_line_to_the_hosts_log() {
    run(|w| async move {
        let captured = wsp_core::console::capture();
        BootRuntime::warn(&*w.c, "a machine of this setup was left");
        assert_eq!(captured.lines(), vec!["a machine of this setup was left".to_string()]);
    });
}

#[test]
fn owner_and_reads_since_are_the_states_and_start_empty() {
    run(|w| async move {
        assert_eq!(BootRuntime::owner(&*w.c), "");
        assert_eq!(BootRuntime::reads_since(&*w.c), 0.0);
        BootRuntime::set_owner(&*w.c, "h_abc");
        BootRuntime::set_reads_since(&*w.c, 1_234.0);
        assert_eq!(BootRuntime::owner(&*w.c), "h_abc");
        assert_eq!(BootRuntime::reads_since(&*w.c), 1_234.0);
    });
}

#[test]
fn a_runtime_that_has_not_closed_is_neither_closed_nor_closing() {
    run(|w| async move {
        assert!(!BootRuntime::closed(&*w.c));
        assert!(!BootRuntime::closing(&*w.c));
    });
}

#[test]
fn hostname_is_this_computers() {
    run(|w| async move {
        let name = BootRuntime::hostname(&*w.c);
        assert!(!name.is_empty());
        assert_eq!(name, wsp_core::host_id::hostname());
    });
}

#[test]
fn local_wired_is_false_with_no_local_wiring() {
    run(|w| async move { assert!(!BootRuntime::local_wired(&*w.c)) });
}

// ---- the shared maps ------------------------------------------------------------------------------------------

#[test]
fn the_maps_are_one_handle_each() {
    run(|w| async move {
        let c = &*w.c;
        assert!(Rc::ptr_eq(&BootRuntime::live(c), &BootRuntime::live(c)));
        assert!(Rc::ptr_eq(&BootRuntime::sessions(c), &BootRuntime::sessions(c)));
        assert!(Rc::ptr_eq(&BootRuntime::thread_records(c), &BootRuntime::thread_records(c)));
        assert!(Rc::ptr_eq(&BootRuntime::projects_held(c), &BootRuntime::projects_held(c)));
        assert!(Rc::ptr_eq(&BootRuntime::boot_work(c), &BootRuntime::boot_work(c)));
        assert!(Rc::ptr_eq(&BootRuntime::builders(c), &BootRuntime::builders(c)));
        assert!(Rc::ptr_eq(&BootRuntime::viewed_marks(c), &BootRuntime::viewed_marks(c)));
        assert!(Rc::ptr_eq(&BootRuntime::rereading(c), &BootRuntime::rereading(c)));
    });
}

#[test]
fn the_maps_start_empty_and_a_write_through_one_read_shows_in_the_next() {
    run(|w| async move {
        let c = &*w.c;
        assert!(BootRuntime::live(c).borrow().is_empty());
        assert!(BootRuntime::sessions(c).borrow().is_empty());
        assert!(BootRuntime::projects_held(c).borrow().is_empty());
        assert!(BootRuntime::boot_work(c).borrow().is_empty());
        assert!(BootRuntime::builders(c).borrow().is_empty());
        assert!(BootRuntime::viewed_marks(c).borrow().is_empty());
        let record: ThreadRecord = serde_json::from_value(json!({ "workspaceId": "w_1", "harness": "claude" })).expect("a thread record");
        BootRuntime::thread_records(c).borrow_mut().insert("t_1".into(), record);
        assert_eq!(BootRuntime::thread_records(c).borrow().get("t_1").map(|r| r.workspace_id.clone()), Some("w_1".into()));
        BootRuntime::sessions(c).borrow_mut().insert("s_1".into(), row("s_1", "t_1", "running"));
        assert_eq!(BootRuntime::sessions(c).borrow().len(), 1);
        BootRuntime::rereading(c).borrow_mut().insert("w_1".into());
        assert!(BootRuntime::rereading(c).borrow().contains("w_1"));
    });
}

// The thread map and the rows are the very ones other traits read: the boot area writes, the others see it.
// waits: packet 3 (BoxPanelsCtx::thread_records) and packet 4 (MachinesRuntime::live, thread_records).
#[test]
fn the_boot_maps_are_the_machines_areas_maps() {
    run(|w| async move {
        let record: ThreadRecord = serde_json::from_value(json!({ "workspaceId": "w_1", "harness": "claude" })).expect("a thread record");
        BootRuntime::thread_records(&*w.c).borrow_mut().insert("t_1".into(), record);
        assert!(Rc::ptr_eq(&BootRuntime::thread_records(&*w.c), &wsp_machines::machines::machines::MachinesRuntime::thread_records(&*w.c)));
        assert!(Rc::ptr_eq(&BootRuntime::live(&*w.c), &wsp_machines::machines::machines::MachinesRuntime::live(&*w.c)));
        let pairs = wsp_account::account::box_panels::BoxPanelsCtx::thread_records(&*w.c);
        assert_eq!(pairs, vec![("t_1".to_string(), "w_1".to_string())]);
    });
}

// ---- the transcript index -------------------------------------------------------------------------------------

#[test]
fn a_workspace_with_no_transcript_index_reads_nothing() {
    run(|w| async move {
        assert!(!BootRuntime::has_transcript_index(&*w.c, "w_1"));
        assert_eq!(BootRuntime::index_pos(&*w.c, "w_1"), 0);
        assert!(BootRuntime::index_children(&*w.c, "w_1", "t_1").is_empty());
    });
}

// A transcript row on the store, read by `load_index`, is what the three index reads then answer.
// waits: packet 3 (TranscriptsCtx, which the area's load reads its rows through).
#[test]
fn load_index_reads_the_transcript_rows_into_the_index() {
    run(|w| async move {
        let rows = wsp_threads::threads::transcripts::TranscriptsCtx::rows(&*w.c);
        let start = json!({ "type": "session.start", "workspaceId": "w_1", "sessionId": "s_1", "turnId": "turn-1", "threadId": "t_1", "harness": "claude", "startedAt": 10 });
        let event: wsp_wire::views::session_events::SessionEvent = serde_json::from_value(start).expect("a session event");
        rows.append("w_1", &[event]).expect("a row");
        BootRuntime::load_index(&*w.c, "w_1").await.expect("loaded");
        assert!(BootRuntime::has_transcript_index(&*w.c, "w_1"));
        assert!(BootRuntime::index_pos(&*w.c, "w_1") > 0);
        assert!(BootRuntime::index_children(&*w.c, "w_1", "t_1").is_empty());
    });
}

// ---- the place door --------------------------------------------------------------------------------------------

#[test]
fn a_runtime_with_no_place_door_loads_nothing_and_lists_no_computers() {
    run(|w| async move {
        BootRuntime::place_door_load(&*w.c).await.expect("no door: nothing to load");
        assert!(BootRuntime::place_rows(&*w.c).await.is_none());
        assert!(BootRuntime::door_backend_of(&*w.c, "p_1").is_none());
    });
}

// `ctx.placeDoorOf()` throws `noKindLine("place")` with no door wired (kinds.ts:221-224), and the area passes that on.
#[test]
fn place_for_with_no_door_is_the_refusal_of_a_missing_door() {
    run(|w| async move {
        let refused = BootRuntime::place_for(&*w.c, "elsewhere").await.expect_err("no door");
        assert_eq!(refused.message, wsp_words::words::computer::no_kind_line("place"));
    });
}

// ---- the start's steps that answer for the state as it is ------------------------------------------------------

#[test]
fn the_steps_proposal_3_dropped_answer_ok() {
    run(|w| async move {
        BootRuntime::migrate_copies(&*w.c).await.expect("migrate_copies");
        BootRuntime::move_transcript(&*w.c, json!({ "id": "w_1" })).await.expect("move_transcript");
        BootRuntime::move_blobs(&*w.c);
        BootRuntime::move_old_copies(&*w.c).await.expect("move_old_copies");
    });
}

#[test]
fn setups_load_reads_the_stored_setups() {
    run(|w| async move {
        BootRuntime::setups_load(&*w.c).await.expect("the setups load");
    });
}

// ---- devices ----------------------------------------------------------------------------------------------------

fn scope(thread: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "w_1", "rootThreadId": thread })).expect("a scope")
}

#[test]
fn devices_lists_every_device_with_the_thread_its_token_is_scoped_to() {
    run(|w| async move {
        let door = w.rt.devices();
        let paired = door.admit("laptop", 1_000).await.expect("a paired device");
        let scoped = door.mint("turn", scope("t_1"), 2_000, MintMore::default()).await.expect("a scoped device");
        let listed = BootRuntime::devices(&*w.c).await.expect("the devices");
        let by_id = |id: &str| listed.iter().find(|d| d.id == id).expect("a listed device").thread_id.clone();
        assert_eq!(listed.len(), 2);
        assert_eq!(by_id(&paired.device.id), None);
        assert_eq!(by_id(&scoped.device.id), Some("t_1".to_string()));
    });
}

#[test]
fn revoke_device_takes_one_away_and_revoke_asides_takes_the_asides() {
    run(|w| async move {
        let door = w.rt.devices();
        let keep = door.admit("laptop", 1_000).await.expect("a paired device");
        let gone = door.mint("turn", scope("t_1"), 2_000, MintMore::default()).await.expect("a scoped device");
        let aside = door.mint("aside", scope("t_2"), 3_000, MintMore { road: None, aside: true }).await.expect("an aside");
        BootRuntime::revoke_device(&*w.c, &gone.device.id).await.expect("revoked");
        let left: Vec<String> = door.list().await.expect("list").into_iter().map(|d| d.id).collect();
        assert!(left.contains(&keep.device.id) && left.contains(&aside.device.id) && !left.contains(&gone.device.id));
        BootRuntime::revoke_asides(&*w.c).await.expect("asides revoked");
        let left: Vec<String> = door.list().await.expect("list").into_iter().map(|d| d.id).collect();
        assert_eq!(left, vec![keep.device.id]);
    });
}

// ---- timers the core owns -----------------------------------------------------------------------------------------

// `resume_on_reset` arms a timer the clock owns; `cancel_resume` is `ctx.resumeTimers.get(thread)?.()` and takes it.
// waits: packet 17 (CoreCtx, which the core reads the clock and the thread's record through).
#[test]
fn cancel_resume_takes_a_timer_resume_on_reset_armed_and_nothing_else() {
    run(|w| async move {
        BootRuntime::cancel_resume(&*w.c, "t_none");
        assert_eq!(w.clock.pending(), 0);
        BootRuntime::resume_on_reset(&*w.c, "t_1", 1_000_000.0 + 60_000.0);
        assert_eq!(w.clock.pending(), 1);
        BootRuntime::cancel_resume(&*w.c, "t_1");
        assert_eq!(w.clock.pending(), 0);
    });
}

// waits: packet 17 (CoreCtx).
#[test]
fn wake_at_arms_a_snooze_timer_for_a_future_time() {
    run(|w| async move {
        BootRuntime::wake_at(&*w.c, "t_1", 1_000_000.0 + 60_000.0);
        assert_eq!(w.clock.pending(), 1);
    });
}

// ---- pure helpers ---------------------------------------------------------------------------------------------------

#[test]
fn agent_pid_of_a_run_that_left_no_pid_is_nothing() {
    run(|w| async move { assert_eq!(BootRuntime::agent_pid_of(&*w.c, "no-such-run-file"), None) });
}

#[test]
fn sub_row_of_a_record_that_is_no_sub_is_nothing() {
    run(|w| async move {
        let record = workspace_record("w_1");
        assert!(BootRuntime::sub_row_of(&*w.c, &record.borrow()).is_none());
    });
}

// waits: packet 2 (BackendFor, which the disk read asks for the record's backend).
#[test]
fn disk_kept_until_a_running_record_is_nothing() {
    run(|w| async move {
        let record = workspace_record("w_1");
        assert_eq!(BootRuntime::disk_kept_until(&*w.c, &record.borrow()), None);
    });
}

// ---- the doors that read the record: waits on packet 2 (RulesCtx) ----------------------------------------------------

// waits: packet 2 (RulesCtx), packet 11 (ThreadsCtx).
#[test]
fn a_caller_with_no_scope_drives_every_thread_and_acts_on_every_record() {
    run(|w| async move {
        let record = workspace_record("w_1");
        assert!(BootRuntime::drives_thread(&*w.c, Some("t_1"), None));
        assert!(BootRuntime::acts_on(&*w.c, &record.borrow(), None));
        assert!(!BootRuntime::talks_to_its_tree(&*w.c, None).expect("no caller reads no tree"));
        BootRuntime::refuse_relayed(&*w.c, &record.borrow(), None, Some("stop"), Some("t_1"))
            .expect("a caller that is no relay is let through");
    });
}

// ---- the landing road -------------------------------------------------------------------------------------------------

#[test]
fn the_context_answers_for_this_hosts_own_provider_road() {
    run(|w| async move {
        let project = serde_json::from_value(json!({
            "id": "p_1", "name": "site", "computer": "default", "source": { "kind": "git", "url": "https://example.test/site.git" },
            "path": "/home/wsp/site", "remote": "https://example.test/site.git", "defaultBranch": "main", "memoryKey": "-home-wsp-site",
            "memoryDir": "/home/wsp/.claude/projects/-home-wsp-site/memory", "createdAt": "2026-10-26T00:00:00.000Z",
        }))
        .expect("a project");
        assert_eq!(LandingRoad::kind(&*w.c), "provider");
        assert_eq!(LandingRoad::refusal(&*w.c, &project, &json!({})), None);
        assert_eq!(
            LandingRoad::memory_dir(&*w.c, &project, "-home-wsp-site", &json!({})).expect("a folder"),
            wsp_core::project_landing::guest_memory_dir("-home-wsp-site")
        );
        assert!(LandingRoad::workspace_binds(&*w.c, &project).is_empty());
    });
}

// ---- the forwards: each answers for an empty state as the area does. Every one waits on the packet owning the area's
// own context (the area reads the maps through it) -------------------------------------------------------------------

// waits: packet 11 (ThreadsCtx).
#[test]
fn the_thread_reads_of_an_empty_runtime_are_empty() {
    run(|w| async move {
        let c = &*w.c;
        assert!(BootRuntime::restarts(c).is_empty());
        assert!(BootRuntime::latest_on(c, "t_1").is_none());
        assert!(BootRuntime::rows_on(c, "t_1").is_empty());
        assert!(!BootRuntime::thread_runs(c, "t_1"));
        assert!(!BootRuntime::lead_asks(c, "t_1"));
        assert!(BootRuntime::cap_wait(c, "turn-1").is_none());
        BootRuntime::reap_kept(c, "t_1");
    });
}

// waits: packet 11 (ThreadsCtx) and packet 13 (SessionsCtx).
#[test]
fn a_held_row_is_found_by_its_thread_and_by_the_thread_it_stopped_behind() {
    run(|w| async move {
        let c = &*w.c;
        BootRuntime::sessions(c).borrow_mut().insert("s_1".into(), row("s_1", "t_1", "running"));
        assert_eq!(BootRuntime::latest_on(c, "t_1").map(|v| v.id), Some("s_1".to_string()));
        assert_eq!(BootRuntime::rows_on(c, "t_1").len(), 1);
        assert!(BootRuntime::thread_runs(c, "t_1"));
        assert!(BootRuntime::stopped_behind(c, &row("s_1", "t_1", "running")).is_none());
    });
}

// waits: packet 11 (AgentsCtx).
#[test]
fn title_rows_of_no_rows_is_no_rows() {
    run(|w| async move { assert!(BootRuntime::title_rows(&*w.c, Vec::new()).is_empty()) });
}

// waits: packet 12 (TurnsCtx).
#[test]
fn sweep_runs_and_the_deliveries_of_an_empty_runtime_answer_ok() {
    run(|w| async move {
        BootRuntime::deliver_owed(&*w.c).await.expect("nothing owed");
        BootRuntime::resume_queue(&*w.c).await.expect("nothing queued");
        BootRuntime::slates_ready(&*w.c).await.expect("the slates are ready");
        assert!(BootRuntime::shell_held_devices(&*w.c).await.expect("no shells").is_empty());
    });
}

// waits: packet 15 (the usage ledger).
#[test]
fn ledger_limits_of_a_ledger_that_read_nothing_is_empty() {
    run(|w| async move { assert!(BootRuntime::ledger_limits(&*w.c).await.is_empty()) });
}

// ---- forwards that need a live entry, made by attach: waits on packet 4 (MachinesRuntime) and the packets it reads ----

// waits: packet 4 (attach), packet 11 (AgentsCtx).
#[test]
fn an_attached_entry_names_its_setup_place_and_has_no_refusal() {
    run(|w| async move {
        let entry = attached(&w, "w_1").await;
        assert_eq!(BootRuntime::live(&*w.c).borrow().len(), 1);
        assert!(Rc::ptr_eq(&BootRuntime::live(&*w.c).borrow()["w_1"], &entry));
        assert_eq!(BootRuntime::setup_place(&*w.c, &entry), wsp_threads::threads::agents::AgentsArea::setup_place(&entry));
        assert_eq!(BootRuntime::setup_refusal(&*w.c, "w_1", "claude"), None);
        assert_eq!(BootRuntime::place_id_of(&*w.c, &entry.borrow().record.borrow()), None);
    });
}

// waits: packet 4 (attach), packet 2 (KindsCtx), packet 3 (ChannelsCtx), packet 11 (ThreadsCtx).
#[test]
fn account_key_on_a_provider_machine_is_the_agent_at_the_computer() {
    run(|w| async move {
        let entry = attached(&w, "w_1").await;
        let key = BootRuntime::account_key_on(&*w.c, &entry, "claude");
        assert!(key.starts_with("claude"), "{key}");
    });
}

// waits: packet 4 (attach, mark_gone), packet 2 (KindsCtx).
#[test]
fn mark_gone_settles_an_entrys_machine_and_says_what_happened() {
    run(|w| async move {
        let entry = attached(&w, "w_1").await;
        let outcome = BootRuntime::mark_gone(&*w.c, &entry, "m_1", "the machine went").await.expect("an outcome");
        assert!(matches!(outcome, wsp_core::types::wiring::GoneOutcome::Settled | wsp_core::types::wiring::GoneOutcome::NotGone));
    });
}

// ---- more forwards: each waits on the packets owning the area and the contexts it reads through ------------------------------

// waits: packet 2 (KindsCtx, RecordsCtx, RulesCtx).
#[test]
fn the_kind_and_record_forwards_read_and_write_what_the_areas_hold() {
    run(|w| async move {
        let record = workspace_record("w_1");
        assert!(BootRuntime::backend_for(&*w.c, &record.borrow()).is_ok());
        assert!(BootRuntime::module_of(&*w.c, "cloud").is_ok());
        let unknown = BootRuntime::module_of(&*w.c, "nonsense").err().expect("a kind no module serves");
        assert_eq!(unknown.message, wsp_words::words::computer::no_kind_line("nonsense"));
        let held = record.borrow().clone();
        BootRuntime::persist(&*w.c, &held).await.expect("persisted");
        assert_eq!(w.store.get("workspaces", "w_1").await.expect("get").expect("a record")["name"], json!("n"));
        let project = serde_json::from_value(json!({
            "id": "p_1", "name": "site", "computer": "default", "source": { "kind": "git", "url": "https://example.test/site.git" },
            "path": "/home/wsp/site", "remote": "https://example.test/site.git", "defaultBranch": "main", "memoryKey": "-home-wsp-site",
            "memoryDir": "/home/wsp/.claude/projects/-home-wsp-site/memory", "createdAt": "2026-10-26T00:00:00.000Z",
        }))
        .expect("a project");
        BootRuntime::remember_project(&*w.c, project).await.expect("remembered");
        assert!(BootRuntime::projects_held(&*w.c).borrow().contains_key("p_1"));
        assert!(w.store.get("projects", "p_1").await.expect("get").is_some());
    });
}

// waits: packet 2 (RecordsCtx), packet 4 (MachinesRuntime), packet 3 (TranscriptsCtx), packet 13 (SessionsCtx).
#[test]
fn the_reads_of_an_empty_state_answer_nothing() {
    run(|w| async move {
        let c = &*w.c;
        assert_eq!(BootRuntime::image_kind(c, "snap_none").await.expect("an image read"), None);
        assert_eq!(BootRuntime::default_image_head(c).await.expect("no image yet"), None);
        assert!(!BootRuntime::turn_changes_written(c, "w_1", "t_1", "turn-1").await.expect("no thread, no changes"));
        assert!(BootRuntime::git_top_of(c, "/no/such/folder").await.is_err());
        assert!(BootRuntime::resolve_workspace(c, "nope", None).await.is_err());
        assert!(BootRuntime::resume_turn(c, "w_none", "t_1", 5.0).await.is_err());
    });
}

// waits: packet 4 (attach, observed, idle, daemon), packet 8 (folders), packet 7 (sub life), packet 11 (agents).
#[test]
fn the_machines_the_fire_and_forget_forwards_answer_for_an_attached_entry() {
    run(|w| async move {
        let entry = attached(&w, "w_1").await;
        let machine = entry.borrow().machine.clone();
        assert_eq!(BootRuntime::observed(&*w.c, machine.clone()).id(), machine.id());
        BootRuntime::idle_touch(&*w.c, "w_1");
        BootRuntime::sync_daemon(&*w.c, &entry);
        BootRuntime::arm_subs(&*w.c);
        BootRuntime::arm_sweep(&*w.c);
        BootRuntime::refresh_title(&*w.c, view("s_1", Some("t_1"), "completed"));
        tokio::task::yield_now().await;
    });
}

// waits: packet 11 (ThreadsCtx), packet 3 (TranscriptsCtx), and every area that hears the event.
#[test]
fn a_held_end_with_no_one_to_tell_tells_no_one_and_record_stamps_and_emits() {
    run(|w| async move {
        let (all, _off) = listen(&w, "*");
        let end = wsp_machines::machines::boot::HeldEnd {
            view: view("s_1", Some("t_1"), "failed"),
            turn_id: "turn-s_1".into(),
            notify: Vec::new(),
            notify_by: None,
            notify_road: None,
            result: serde_json::from_value(json!({ "status": "failed", "error": "the host went down" })).expect("a result"),
        };
        BootRuntime::notify_end(&*w.c, end);
        assert!(all.borrow().is_empty(), "no target, so nothing is told");
        let ended: EventUnion = serde_json::from_value(json!({
            "type": "session.end", "workspaceId": "w_1", "sessionId": "s_1", "turnId": "turn-s_1", "threadId": "t_1",
            "exitCode": null, "sawResult": false, "reason": "the host went down", "unstarted": true,
        }))
        .expect("an event");
        BootRuntime::record(&*w.c, ended);
        let all = all.borrow();
        assert_eq!(all.iter().filter(|e| e["type"] == "session.end" && e["seq"].as_u64() > Some(0)).count(), 1);
    });
}

// ---- serve-replay.test.ts: the ring the context's emit feeds ----------------------------------------------------------

#[test]
fn a_cursor_in_the_ring_replays_exactly_the_events_after_it_and_one_older_than_the_ring_is_a_gap() {
    run(|w| async move {
        for n in 0..5 {
            BootRuntime::emit(&*w.c, notice(&format!("t_{n}"))).expect("emit");
        }
        let after_two = w.rt.events().since(Some(2), None);
        assert!(!after_two.gap);
        assert_eq!(after_two.head, 5);
        let seen: Vec<Value> = after_two.events.iter().map(|e| serde_json::to_value(e).expect("an event")["message"].clone()).collect();
        assert_eq!(seen, vec![json!("t_2"), json!("t_3"), json!("t_4")]);
        for n in 0..wsp_core::types::events::EVENT_RING_CAP {
            BootRuntime::emit(&*w.c, notice(&format!("late_{n}"))).expect("emit");
        }
        let old = w.rt.events().since(Some(2), None);
        assert!(old.gap && old.events.is_empty(), "a cursor older than the ring gets a gap and no replay");
        assert_eq!(old.head, 5 + wsp_core::types::events::EVENT_RING_CAP as u64);
    });
}

#[test]
fn a_cursor_past_the_head_is_a_gap_too() {
    run(|w| async move {
        BootRuntime::emit(&*w.c, notice("t_1")).expect("emit");
        let since = w.rt.events().since(Some(99), None);
        assert!(since.gap && since.events.is_empty());
    });
}

#[test]
fn a_fresh_subscribe_replays_nothing_and_each_event_is_stamped_once_per_runtime() {
    run(|w| async move {
        let (seen, _off) = listen(&w, "*");
        BootRuntime::emit(&*w.c, notice("t_1")).expect("emit");
        BootRuntime::emit(&*w.c, notice("t_2")).expect("emit");
        let seen = seen.borrow();
        assert_eq!(seen.iter().map(|e| e["seq"].clone()).collect::<Vec<_>>(), vec![json!(1), json!(2)]);
        assert_eq!(w.rt.events().since(None, None).head, 2);
    });
}

// ---- state-0-3-6.test.ts: a state file wsp 0.3.6 wrote ------------------------------------------------------------------

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/runtime/test/fixtures/state-0.3.6.json");

fn written() -> serde_json::Map<String, Value> {
    serde_json::from_str(&std::fs::read_to_string(FIXTURE).expect("the 0.3.6 fixture")).expect("a state document")
}

async fn seed(store: &dyn Store, state: &serde_json::Map<String, Value>) {
    for (collection, records) in state {
        if collection == "$shape" {
            continue;
        }
        for (id, record) in records.as_object().expect("a collection") {
            store.put(collection, id, record.clone()).await.expect("put");
        }
    }
}

fn runtime_over(store: Rc<dyn Store>, backend: Arc<dyn MachineBackend>) -> Rc<AssembledRuntime> {
    create_runtime(RuntimeOptions { backend: Some(backend), store: Some(store), ..Default::default() }).expect("a runtime")
}

/// The fake backend holding one machine, and the fixture with its workspace naming that machine (the fake mints its own
/// ids where the stub of the TS test names `m1`), so the start finds it running.
async fn backend_and_state() -> (Arc<dyn MachineBackend>, serde_json::Map<String, Value>) {
    let backend = backend();
    let spec: wsp_engine::machine::MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
    let machine = backend.create(spec, None, None).await.expect("a machine");
    let mut state = written();
    state.get_mut("workspaces").and_then(|w| w.get_mut("ws_be677449")).expect("the workspace")["machineId"] = json!(machine.id());
    (backend, state)
}

#[test]
fn the_fixture_is_in_the_shape_this_build_reads() {
    let shape = &written()["$shape"];
    assert_eq!(
        (shape["shape"].clone(), shape["wsp"].clone(), shape["daemon"].clone()),
        (json!(wsp_wire::wire::daemon_version::STATE_SHAPE), json!("0.3.6"), json!(157))
    );
}

async fn loads_unchanged(store: Rc<dyn Store>, backend: Arc<dyn MachineBackend>, state: serde_json::Map<String, Value>) {
    let rt = runtime_over(store.clone(), backend);
    let projects = rt.projects().list(None).await.expect("projects");
    let workspaces = rt.workspaces().list(None).await.expect("workspaces");
    let expected: Vec<Value> = state["projects"].as_object().expect("projects").values().cloned().collect();
    assert_eq!(projects.iter().map(|p| serde_json::to_value(p).expect("a project")).collect::<Vec<_>>(), expected);
    assert_eq!(workspaces.len(), 1);
    assert!(serde_json::to_value(&workspaces[0]).expect("a view").get("branchOfRecord").is_none());
    for (collection, records) in &state {
        if collection == "$shape" {
            continue;
        }
        for (id, record) in records.as_object().expect("a collection") {
            assert_eq!(store.get(collection, id).await.expect("get").as_ref(), Some(record), "{collection}/{id}");
        }
    }
    rt.close().await.expect("closed");
}

// waits: packets 2, 3, 4, 8 (the start and the lists), 11 (ThreadsCtx).
#[test]
fn a_state_file_0_3_6_wrote_loads_unchanged_into_the_memory_store() {
    run_bare(|| async move {
        let store: Rc<dyn Store> = Rc::new(memory_store());
        let (backend, state) = backend_and_state().await;
        seed(&*store, &state).await;
        loads_unchanged(store, backend, state).await;
    });
}

// waits: the same packets.
#[test]
fn a_state_file_0_3_6_wrote_loads_unchanged_into_the_sqlite_store_a_host_opens() {
    run_bare(|| async move {
        let dir = std::env::temp_dir().join(format!("wsp-state-036-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let (backend, state) = backend_and_state().await;
        std::fs::write(dir.join("state.json"), serde_json::to_string(&Value::Object(state.clone())).expect("a document"))
            .expect("the state written");
        let writer = wsp_store::store::StateWriter {
            wsp: "test".into(),
            daemon: wsp_wire::wire::daemon_version::DAEMON_VERSION.into(),
            bin: "/usr/local/bin/wsp".into(),
        };
        let store: Rc<dyn Store> = Rc::new(wsp_store::sqlite_store::sqlite_store(&dir.join("state.json"), writer));
        loads_unchanged(store, backend, state).await;
        std::fs::remove_dir_all(&dir).expect("removed");
    });
}

// ---- no-provider-hydrate.test.ts: a host started without its provider -----------------------------------------------------

async fn keyless() -> (Rc<dyn Store>, String) {
    let store: Rc<dyn Store> = Rc::new(memory_store());
    let state = written();
    seed(&*store, &state).await;
    (store, "ws_be677449".to_string())
}

// waits: packets 2, 3, 4, 8, 9 (the lists, resolve and nap), 11.
#[test]
fn a_host_without_its_provider_serves_keeps_every_cloud_record_and_answers_the_providers_sentence() {
    run_bare(|| async move {
        let (store, id) = keyless().await;
        let rt = runtime_over(store.clone(), Arc::new(wsp_engine::no_provider_backend::NoProviderBackend::new(None)));
        let listed = serde_json::to_value(rt.workspaces().list(None).await.expect("the list")).expect("views");
        assert_eq!(
            (listed[0]["id"].clone(), listed[0]["name"].clone(), listed[0]["phase"].clone(), listed[0]["kind"].clone()),
            (json!(id), json!("fix-login"), json!("running"), json!("cloud"))
        );
        assert_eq!(rt.workspaces().resolve("fix-login", None).await.expect("resolved").id, id);
        let refused = rt.workspaces().nap(&id, None).await.expect_err("no key");
        assert!(refused.message.contains(wsp_words::words::thread::NO_PROVIDER_LINE), "{}", refused.message);
        let kept = store.get("workspaces", &id).await.expect("get").expect("the record stays");
        assert_eq!((kept["id"].clone(), kept["phase"].clone()), (json!(id), json!("running")));
        rt.close().await.expect("closed");
    });
}

// waits: the same packets.
#[test]
fn a_host_without_its_provider_reads_a_builder_another_host_left_as_it_is() {
    run_bare(|| async move {
        let (store, _) = keyless().await;
        let builder = json!({ "id": "m_b1", "kind": "sandbox", "createdAt": "2026-10-26T00:00:00.000Z", "firstLife": true, "baseTemplate": "t", "setupSha": "s" });
        store.put("builders", "m_b1", builder).await.expect("put");
        let rt = runtime_over(store.clone(), Arc::new(wsp_engine::no_provider_backend::NoProviderBackend::new(None)));
        rt.workspaces().list(None).await.expect("the list");
        assert_eq!(store.get("builders", "m_b1").await.expect("get").expect("kept")["id"], json!("m_b1"));
        rt.close().await.expect("closed");
    });
}

// ---- reconcile.test.ts: the sweep records a machine of this setup that no record claims -----------------------------------

fn ago(ms: i64) -> String {
    wsp_words::js_date::iso_string((wsp_core::clock::RealClock.now() - ms) as f64).expect("a date")
}

/// A state whose owner, project and kept name are known and whose workspace record is lost, over a backend holding the
/// one machine the workspace was forked on, stamped as a fork stamps it.
async fn lost(paused: bool, stamped: bool, age_ms: i64) -> (Rc<dyn Store>, Arc<dyn MachineBackend>, Rc<AssembledRuntime>, String) {
    let store: Rc<dyn Store> = Rc::new(memory_store());
    let state = written();
    seed(&*store, &state).await;
    store.delete("workspaces", "ws_be677449").await.expect("the record is lost");
    store.put("owner", "id", json!({ "id": "h_owner" })).await.expect("owner");
    let backend = backend();
    let mut labels = json!({ "wsp": "1", "wsp-host": "1", "wsp-owner": "h_owner", "createdAt": ago(age_ms) });
    if stamped {
        labels["wsp-workspace"] = json!("ws_be677449");
        labels["wsp-name"] = json!("fix-login");
        labels["wsp-golden"] = json!("snap_g");
    }
    let spec: wsp_engine::machine::MachineSpec =
        serde_json::from_value(json!({ "kind": "sandbox", "labels": labels })).expect("a machine spec");
    let machine = backend.create(spec, None, None).await.expect("a machine");
    if paused {
        machine.pause(None).await.expect("paused");
    }
    let rt = runtime_over(store.clone(), backend.clone());
    (store, backend, rt, machine.id().to_string())
}

fn reap_json(r: &wsp_core::types::wiring::SweepResult) -> Value {
    json!({
        "reaped": r.reaped.reaped.iter().map(|m| json!({ "id": m.id })).collect::<Vec<_>>(),
        "spared": r.reaped.spared.iter().map(|m| json!({ "id": m.id })).collect::<Vec<_>>(),
        "failed": r.reaped.failed.as_ref().map(|f| f.iter().map(|m| json!({ "id": m.id, "message": m.message })).collect::<Vec<_>>()),
        "adopted": r.adopted.as_ref().map(|a| a.iter().map(|m| json!({ "id": m.id, "workspaceId": m.workspace_id, "name": m.name, "phase": serde_json::to_value(m.phase).expect("a phase") })).collect::<Vec<_>>()),
    })
}

// waits: packets 2, 3, 4, 9, 14, 15, 17 (the sweep and the status it emits).
#[test]
fn a_running_machine_with_the_owner_label_and_no_record_becomes_a_running_record_and_is_not_killed() {
    run_bare(|| async move {
        let (store, _backend, rt, machine) = lost(false, true, 2 * 60_000).await;
        let (events, _off) = {
            let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
            let sink = seen.clone();
            let listener: EventListener = Rc::new(move |e: &EventUnion| {
                sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            });
            (seen, rt.events().on("*", listener))
        };
        let result = rt.reap(None, None).await.expect("the sweep");
        let said = reap_json(&result);
        assert_eq!(said["adopted"], json!([{ "id": machine, "workspaceId": "ws_be677449", "name": "fix-login", "phase": "running" }]));
        assert_eq!((said["reaped"].clone(), said["spared"].clone()), (json!([]), json!([])));
        let listed = serde_json::to_value(rt.workspaces().list(None).await.expect("list")).expect("views");
        assert_eq!(
            (listed[0]["id"].clone(), listed[0]["machineId"].clone(), listed[0]["phase"].clone(), listed[0]["golden"].clone()),
            (json!("ws_be677449"), json!(machine), json!("running"), json!("snap_g"))
        );
        let kept = store.get("workspaces", "ws_be677449").await.expect("get").expect("a record");
        assert_eq!(
            (kept["machineId"].clone(), kept["phase"].clone(), kept["firstLife"].clone()),
            (json!(machine), json!("running"), json!(false))
        );
        let heard: Vec<Value> = events.borrow().clone();
        assert_eq!(heard.iter().filter(|e| e["type"] == "workspace.created").count(), 1);
        let status: Vec<&Value> = heard.iter().filter(|e| e["type"] == "workspace.status").collect();
        assert_eq!(
            (status.len(), status[0]["status"]["phase"].clone(), status[0]["status"]["reason"].clone()),
            (1, json!("running"), json!(wsp_words::words::thread::RECORD_RESTORED))
        );
        let second = reap_json(&rt.reap(None, None).await.expect("the second sweep"));
        assert_eq!(second["adopted"], Value::Null, "the second sweep finds it claimed");
    });
}

// waits: the same packets.
#[test]
fn a_paused_one_hydrates_napping_with_the_same_words() {
    run_bare(|| async move {
        let (_store, _backend, rt, machine) = lost(true, true, 2 * 60_000).await;
        let said = reap_json(&rt.reap(None, None).await.expect("the sweep"));
        assert_eq!(said["adopted"], json!([{ "id": machine, "workspaceId": "ws_be677449", "name": "fix-login", "phase": "napping" }]));
        let listed = serde_json::to_value(rt.workspaces().list(None).await.expect("list")).expect("views");
        assert_eq!(listed[0]["phase"], json!("napping"));
    });
}

// waits: the same packets.
#[test]
fn a_machine_forked_before_the_stamps_is_reported_not_recorded() {
    run_bare(|| async move {
        let (_store, _backend, rt, machine) = lost(false, false, 2 * 60_000).await;
        rt.workspaces().list(None).await.expect("started");
        let said = reap_json(&rt.reap(None, None).await.expect("the sweep"));
        assert_eq!(said["adopted"], Value::Null);
        let failed = said["failed"].as_array().expect("failed rows");
        assert_eq!(failed[0]["id"], json!(machine));
        assert!(failed[0]["message"].as_str().expect("words").contains("this host holds no project for it"));
        assert!(rt.workspaces().list(None).await.expect("list").is_empty());
    });
}

// waits: the same packets.
#[test]
fn a_machine_inside_its_create_minute_is_left_for_the_next_sweep() {
    run_bare(|| async move {
        let (_store, _backend, rt, machine) = lost(false, true, 30_000).await;
        let said = reap_json(&rt.reap(None, None).await.expect("the sweep"));
        assert_eq!(said["adopted"], Value::Null);
        assert_eq!(said["spared"], json!([{ "id": machine }]));
        assert!(rt.workspaces().list(None).await.expect("list").is_empty());
    });
}

// waits: the same packets.
#[test]
fn builders_smoke_forks_another_setups_machines_and_reserved_experiments_are_never_recorded() {
    run_bare(|| async move {
        let store: Rc<dyn Store> = Rc::new(memory_store());
        store.put("owner", "id", json!({ "id": "h_owner" })).await.expect("owner");
        let backend = backend();
        let at = ago(2 * 60_000);
        let mut made: Vec<String> = Vec::new();
        for labels in [
            json!({ "wsp": "1", "wsp-builder": "1", "wsp-owner": "h_owner", "createdAt": at }),
            json!({ "wsp": "1", "wsp-smoke": "1", "wsp-owner": "h_owner", "createdAt": at }),
            json!({ "wsp": "1", "wsp-owner": "h_other", "createdAt": at }),
            json!({ "wsp": "1", "wsp-owner": "h_owner", "poc": "ttl-test", "createdAt": at }),
        ] {
            let spec: wsp_engine::machine::MachineSpec =
                serde_json::from_value(json!({ "kind": "sandbox", "labels": labels })).expect("a spec");
            made.push(backend.create(spec, None, None).await.expect("a machine").id().to_string());
        }
        let rt = runtime_over(store, backend);
        rt.workspaces().list(None).await.expect("started");
        let said = reap_json(&rt.reap(None, None).await.expect("the sweep"));
        assert_eq!(said["adopted"], Value::Null);
        assert!(rt.workspaces().list(None).await.expect("list").is_empty());
        assert_eq!(said["reaped"], json!([{ "id": made[0] }, { "id": made[1] }]));
    });
}
