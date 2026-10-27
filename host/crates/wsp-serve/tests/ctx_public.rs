// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 17 (#2476): what the context answers the core (`CoreCtx`, runtime.ts:491-992), the public
//! members (`PublicCtx`, the sweep and the close, runtime.ts:1105-1275), the harness lists (`HarnessesCtx`) and the
//! status reads (`StatusCtx`). Each test builds the runtime over the sqlite store in a temp folder and calls the method
//! as the core or the public members do. A test that reaches another packet's `todo!()` body fails on it until that
//! packet lands; its comment names the packet it waits for.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_core::clock::{Clock as _, FakeClock};
use wsp_core::types::api::Runtime;
use wsp_core::types::events::{EventListener, Unlisten};
use wsp_core::types::internal::{BuilderLife, LiveBuilder, SessionEntry, ThreadRecord};
use wsp_core::types::wiring::{RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::{Machine as _, MachineBackend, MachineState};
use wsp_machines::machines::boot::BootRuntime;
use wsp_machines::machines::machines::{Entry, Row};
use wsp_serve::runtime::core::CoreCtx;
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::runtime::public::{CloseStep, HarnessesCtx, MachineRead, PublicCtx, StatusCtx};
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::{StateWriter, Store};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::events_union::EventUnion;

struct World {
    rt: Rc<AssembledRuntime>,
    c: Rc<Ctx>,
    store: Rc<dyn Store>,
    clock: FakeClock,
    backend: Arc<dyn MachineBackend>,
    _home: tempfile::TempDir,
}

fn world() -> World {
    let home = tempfile::tempdir().expect("a temp folder");
    let writer = StateWriter { wsp: "0.0.0".into(), daemon: JsInt::from(1), bin: "/bin/wsp".into() };
    let store: Rc<dyn Store> = Rc::new(sqlite_store(&home.path().join("state.json"), writer));
    let clock = FakeClock::new(1_000_000);
    let backend: Arc<dyn MachineBackend> = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
    let opts =
        RuntimeOptions { backend: Some(backend.clone()), store: Some(store.clone()), clock: Some(clock.clock()), ..Default::default() };
    let rt = create_runtime(opts);
    let c = rt.context();
    World { rt, c, store, clock, backend, _home: home }
}

/// The runtime on a throwaway state, inside the task set the owner thread runs.
fn run<Fut: std::future::Future<Output = ()>>(f: impl FnOnce(World) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move { f(world()).await });
}

fn workspace_record(id: &str, place: Option<&str>, project: &str) -> Rc<RefCell<WorkspaceRecord>> {
    let mut raw = json!({
        "id": id, "name": format!("fork-{id}"), "machineId": format!("m_{id}"), "phase": "running", "golden": "g",
        "createdAt": "2026-10-26T00:00:00.000Z", "project": project, "spec": {}, "size": { "cpu": 2, "memMb": 4096 },
        "firstLife": false, "kind": "cloud",
    });
    if let Some(place) = place {
        raw["place"] = json!(place);
    }
    Rc::new(RefCell::new(serde_json::from_value(raw).expect("a workspace record")))
}

/// A live entry on a machine of the fake backend, kept in the context's live map.
async fn attached(w: &World, id: &str, place: Option<&str>, project: &str) -> Entry {
    let spec: wsp_engine::machine::MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
    let machine = w.backend.create(spec, None, None).await.expect("a machine");
    let entry = BootRuntime::attach(&*w.c, &workspace_record(id, None, project), machine).expect("an entry");
    // A record that names a place goes through the place door on attach; the name is put on after, as a joined computer's would be.
    entry.borrow().record.borrow_mut().place = place.map(str::to_string);
    BootRuntime::live(&*w.c).borrow_mut().insert(id.to_string(), entry.clone());
    entry
}

fn thread_record(raw: Value) -> ThreadRecord {
    serde_json::from_value(raw).expect("a thread record")
}

fn project_view(id: &str, computer: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": format!("proj-{id}"), "path": format!("/home/p/{id}"), "computer": computer,
        "source": { "kind": "folder", "path": format!("/home/p/{id}") }, "remote": "", "defaultBranch": "main",
        "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project view")
}

fn session_row(id: &str, workspace: &str, thread: &str, turn: &str) -> Row {
    let view: SessionView = serde_json::from_value(json!({
        "id": id, "workspaceId": workspace, "harness": "claude", "status": "running", "threadId": thread,
    }))
    .expect("a session view");
    Rc::new(RefCell::new(SessionEntry {
        view,
        turn_id: turn.to_string(),
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

fn notice(message: &str) -> EventUnion {
    serde_json::from_value(json!({ "type": "host.notice", "message": message })).expect("an event")
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

// ---- CoreCtx: the handles and the maps it reads ---------------------------------------------------------------

#[test]
fn core_hands_out_the_runtimes_own_handles() {
    run(|w| async move {
        assert_eq!(CoreCtx::clock(&*w.c).now(), 1_000_000);
        w.clock.advance(500);
        assert_eq!(CoreCtx::clock(&*w.c).now(), 1_000_500);
        let devices = CoreCtx::devices(&*w.c);
        let own = w.rt.devices();
        assert!(std::ptr::addr_eq(Rc::as_ptr(&devices), Rc::as_ptr(&own)));
        assert_eq!(CoreCtx::host_key(&*w.c), None, "no place door, so no host key");
        let bus = CoreCtx::bus(&*w.c);
        let events = w.rt.events();
        assert!(std::ptr::addr_eq(Rc::as_ptr(&bus), Rc::as_ptr(&events)));
    });
}

#[test]
fn core_emit_stamps_and_delivers_on_the_bus() {
    run(|w| async move {
        let (seen, _off) = listen(&w, "host.notice");
        CoreCtx::emit(&*w.c, notice("one")).expect("emit");
        CoreCtx::emit(&*w.c, notice("two")).expect("emit");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["message"], json!("one"));
        assert_eq!(seen[0]["seq"], json!(1));
        assert_eq!(seen[1]["seq"], json!(2));
    });
}

#[test]
fn core_ready_waits_on_the_one_boot() {
    run(|w| async move {
        CoreCtx::ready(&*w.c).await.expect("the boot on an empty state");
        CoreCtx::ready(&*w.c).await.expect("and again, the same boot");
    });
}

#[test]
fn core_reads_thread_records_in_the_maps_order() {
    run(|w| async move {
        let threads = BootRuntime::thread_records(&*w.c);
        threads.borrow_mut().insert("t_a".into(), thread_record(json!({ "workspaceId": "w_1", "harness": "claude" })));
        threads.borrow_mut().insert(
            "t_b".into(),
            thread_record(json!({ "workspaceId": "w_2", "harness": "claude", "snoozedUntil": 5_000, "limitResume": { "at": 9_000, "turnId": "turn-1" } })),
        );
        assert_eq!(CoreCtx::thread_records(&*w.c), vec![("t_a".to_string(), "w_1".to_string()), ("t_b".to_string(), "w_2".to_string())]);
        assert_eq!(CoreCtx::thread_workspace(&*w.c, "t_b"), Some("w_2".to_string()));
        assert_eq!(CoreCtx::thread_workspace(&*w.c, "t_none"), None);
        assert_eq!(CoreCtx::snoozed_until(&*w.c, "t_b"), Some(5_000.0));
        assert_eq!(CoreCtx::snoozed_until(&*w.c, "t_a"), None);
        assert_eq!(CoreCtx::limit_resume_at(&*w.c, "t_b"), Some(9_000.0));
        assert_eq!(CoreCtx::limit_resume_at(&*w.c, "t_a"), None);
        assert_eq!(CoreCtx::limit_resume_at(&*w.c, "t_none"), None);
    });
}

#[test]
fn core_reads_the_session_views_in_the_maps_order() {
    run(|w| async move {
        let sessions = BootRuntime::sessions(&*w.c);
        sessions.borrow_mut().insert("s_1".into(), session_row("s_1", "w_1", "t_a", "turn-1"));
        sessions.borrow_mut().insert("s_2".into(), session_row("s_2", "w_2", "t_b", "turn-2"));
        let views = CoreCtx::session_views(&*w.c);
        assert_eq!(views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), vec!["s_1", "s_2"]);
        assert_eq!(views[1].workspace_id, "w_2");
        assert_eq!(views[1].thread_id.as_deref(), Some("t_b"));
        assert!(!CoreCtx::cap_held(&*w.c, "turn-1"), "no turn is held back by the cap");
    });
}

#[test]
fn core_reads_the_live_records_as_standing_entries() {
    run(|w| async move {
        let a = attached(&w, "w_a", Some("default"), "proj_1").await;
        attached(&w, "w_b", None, "proj_2").await;
        let machine_id = a.borrow().record.borrow().machine_id.clone();
        let live = CoreCtx::live(&*w.c);
        assert_eq!(live.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["w_a", "w_b"]);
        let first = &live[0];
        assert_eq!(first.name, "fork-w_a");
        assert_eq!(first.kind, WorkspaceKind::Cloud);
        assert_eq!(first.place.as_deref(), Some("default"));
        assert_eq!(first.project, "proj_1");
        assert!(first.running);
        assert_eq!(first.machine_id, machine_id);
        assert!(!first.sub);
        assert_eq!(live[1].place, None);
        assert_eq!(CoreCtx::record_name(&*w.c, "w_b"), "fork-w_b");
    });
}

#[test]
fn core_reads_the_projects_it_holds() {
    run(|w| async move {
        BootRuntime::projects_held(&*w.c).borrow_mut().insert("proj_1".into(), project_view("proj_1", "default"));
        let held = CoreCtx::projects_held(&*w.c);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].id, "proj_1");
        assert_eq!(held[0].computer, "default");
    });
}

#[test]
fn core_answers_a_launch_with_no_road_and_names_a_kinds_turn_road() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        assert!(!CoreCtx::brokered(&*w.c, "w_a"), "no broker is wired");
        let reach = CoreCtx::agents_reach(&*w.c, "w_a").expect("a cloud machine reaches this host through its own wsp server");
        assert_eq!(reach.url, None, "no address is dialled where no relay is wired");
        assert!(reach.wsp.is_some());
        assert_eq!(CoreCtx::agents_reach(&*w.c, "w_none"), None);
        assert_eq!(CoreCtx::turn_road(&*w.c, WorkspaceKind::Cloud), WorkspaceOrigin::Relayed);
    });
}

#[test]
fn core_reads_the_brokers_status_and_the_daemon_wait_when_nothing_is_brokered() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        CoreCtx::read_broker(&*w.c).await.expect("a broker read with no broker");
        CoreCtx::daemon_settled(&*w.c, "w_a").await.expect("no daemon sync is running");
        CoreCtx::read_public_marketplaces(&*w.c).await.expect("a read of the marketplaces on an empty home");
        CoreCtx::reread_public(&*w.c, "w_a").await.expect("a record with no repo of record has none to read");
    });
}

// ---- CoreCtx: calls on the areas --------------------------------------------------------------------------

#[test]
fn core_drops_a_record_and_ends_its_sessions() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        CoreCtx::end_sessions(&*w.c, "w_a", "deleted");
        CoreCtx::drop_record(&*w.c, "w_a").await.expect("drop");
        assert!(!BootRuntime::live(&*w.c).borrow().contains_key("w_a"));
    });
}

// Waits on packet 9 (the workspaces' phases): delete, wake and exec go through the workspaces area.
#[test]
fn core_deletes_wakes_and_runs_in_a_workspace_by_the_workspaces_area() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        CoreCtx::wake(&*w.c, "w_a").await.expect("a running workspace wakes at once");
        let out = CoreCtx::exec(&*w.c, "w_a", "true", 5_000).await.expect("exec");
        assert_eq!(out.exit_code.as_f64(), 0.0);
        CoreCtx::delete_workspace(&*w.c, "w_a").await.expect("delete");
        assert!(!BootRuntime::live(&*w.c).borrow().contains_key("w_a"));
    });
}

// Waits on packet 9: the fork's unsaved read runs its command through the workspaces area.
#[test]
fn core_reads_a_forks_unsaved_work_through_its_own_checkout() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        BootRuntime::projects_held(&*w.c).borrow_mut().insert("proj_1".into(), project_view("proj_1", "default"));
        let line = CoreCtx::unsaved_fork(&*w.c, "w_a", "fork-w_a").await.expect("the read");
        assert_eq!(line, None, "a fresh machine holds nothing unsaved");
    });
}

#[test]
fn core_forgets_a_projects_record() {
    run(|w| async move {
        w.store.put("projects", "proj_1", json!({ "id": "proj_1" })).await.expect("put");
        CoreCtx::store_delete_project(&*w.c, "proj_1").await.expect("delete");
        assert_eq!(w.store.get("projects", "proj_1").await.expect("get"), None);
    });
}

// Waits on packet 16 (the preferences area): the look is `preferences.projectLook[id]` and the write is
// `preferences.set({ projectLook: { [id]: look or null } })`.
#[test]
fn core_writes_a_projects_look_and_reads_it_back() {
    run(|w| async move {
        assert_eq!(CoreCtx::project_look(&*w.c, "proj_1").await.expect("read"), None);
        let look: wsp_wire::views::preferences::ProjectLook =
            serde_json::from_value(json!({ "icon": "code", "hue": "red" })).expect("a look");
        CoreCtx::set_project_look(&*w.c, "proj_1", Some(look.clone())).await.expect("set");
        assert_eq!(CoreCtx::project_look(&*w.c, "proj_1").await.expect("read"), Some(look));
        CoreCtx::set_project_look(&*w.c, "proj_1", None).await.expect("clear");
        assert_eq!(CoreCtx::project_look(&*w.c, "proj_1").await.expect("read"), None);
    });
}

// Waits on packet 16: the icon's change goes through the preferences.
#[test]
fn core_wears_an_icon_and_forgets_it_with_the_project() {
    run(|w| async move {
        let look: wsp_wire::views::preferences::ProjectLook = serde_json::from_value(json!({ "icon": "code" })).expect("a look");
        CoreCtx::set_project_look(&*w.c, "proj_1", Some(look)).await.expect("set");
        CoreCtx::forget_icon(&*w.c, "proj_1").await.expect("forget");
        assert_eq!(CoreCtx::project_look(&*w.c, "proj_1").await.expect("read"), None);
        CoreCtx::set_icon(&*w.c, "proj_1", None).await.expect_err("a project this host does not hold has no icon to take off");
    });
}

#[test]
fn core_names_a_place_by_the_door_else_by_the_computer_or_the_provider() {
    run(|w| async move {
        assert_eq!(CoreCtx::place_name(&*w.c, "here"), wsp_core::host_id::hostname());
        assert_eq!(CoreCtx::place_name(&*w.c, "hetzner"), wsp_words::place_word::provider_key_name("hetzner"));
    });
}

#[test]
fn core_says_a_place_is_away_only_where_a_door_holds_no_link() {
    run(|w| async move {
        assert!(!CoreCtx::place_away(&*w.c, "default"), "a host with no place door has no place it lost the link to");
        assert!(!CoreCtx::place_away(&*w.c, "p_other"));
    });
}

#[test]
fn core_reads_the_held_rows_of_a_place() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        attached(&w, "w_b", None, "proj_1").await;
        assert_eq!(CoreCtx::held_on(&*w.c, "default"), vec!["w_a".to_string()]);
        assert_eq!(CoreCtx::held_on(&*w.c, "p_none"), Vec::<String>::new());
    });
}

#[test]
fn core_lists_no_stored_record_a_live_entry_already_holds() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        w.store
            .put(
                "workspaces",
                "w_a",
                serde_json::to_value(&*workspace_record("w_a", Some("default"), "proj_1").borrow()).expect("a record"),
            )
            .await
            .expect("put");
        assert_eq!(CoreCtx::held_away_on(&*w.c, "default").await.expect("read"), Vec::<String>::new());
        assert!(!CoreCtx::hydrate(&*w.c, "w_none").await.expect("a record the store lacks is no entry"));
    });
}

#[test]
fn core_syncs_and_rereads_the_daemon_of_a_workspace_without_waiting() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        CoreCtx::sync_daemon(&*w.c, "w_a");
        CoreCtx::sync_daemon(&*w.c, "w_none");
        tokio::task::yield_now().await;
        let _ = CoreCtx::reread_held(&*w.c, "w_a", "record load").await;
    });
}

#[test]
fn core_keeps_the_image_current_without_waiting() {
    run(|w| async move {
        CoreCtx::keep_current(&*w.c, "default");
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
    });
}

// Waits on packet 15 (the status tracker's fill in `build_areas_of`): a poll of no rows ends at once.
#[test]
fn core_polls_the_status_of_the_rows_it_names() {
    run(|w| async move {
        CoreCtx::poll(&*w.c, vec![]).await.expect("poll");
    });
}

// ---- PublicCtx ---------------------------------------------------------------------------------------------

#[test]
fn public_reads_the_wall_clock_apart_from_the_runtimes_clock() {
    run(|w| async move {
        let before = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("now").as_millis() as i64;
        let wall = PublicCtx::wall_now(&*w.c);
        let after = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("now").as_millis() as i64;
        assert!((before..=after).contains(&wall));
        assert_eq!(PublicCtx::clock_now(&*w.c), 1_000_000);
    });
}

#[test]
fn public_keeps_the_writer_the_sweep_says_its_stops_through() {
    run(|w| async move {
        let said: Rc<RefCell<Vec<String>>> = Rc::default();
        let sink = said.clone();
        let say: Rc<dyn Fn(&str)> = Rc::new(move |line: &str| sink.borrow_mut().push(line.to_string()));
        let held = Rc::strong_count(&say);
        PublicCtx::set_say_stops(&*w.c, say.clone());
        assert_eq!(Rc::strong_count(&say), held + 1, "the state keeps the writer");
    });
}

#[test]
fn public_ready_and_refresh_builders_run_on_an_empty_state() {
    run(|w| async move {
        PublicCtx::ready(&*w.c).await.expect("ready");
        PublicCtx::refresh_builders(&*w.c).await.expect("refresh");
        assert!(PublicCtx::builders(&*w.c).is_empty());
    });
}

fn live_builder(w: &World, id: &str, life: BuilderLife, building: bool, place: Option<&str>) -> Rc<RefCell<LiveBuilder>> {
    let record: wsp_core::types::internal::BuilderRecord = serde_json::from_value(json!({
        "id": id, "name": format!("b-{id}"), "kind": "sandbox", "baseTemplate": "t", "setupSha": "s",
        "createdAt": "2026-10-20T10:00:00.000Z", "size": { "cpu": 2, "memMb": 4096 }, "firstLife": true,
        "building": building,
    }))
    .expect("a builder record");
    let spec: wsp_engine::machine::MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
    let machine = futures::executor::block_on(w.backend.create(spec, None, None)).expect("a machine");
    let builder = wsp_core::types::pending::Builder {
        machine,
        kind: record.kind,
        base_template: "t".into(),
        setup_sha: "s".into(),
        created_at: record.created_at.clone(),
        first_life: true,
        size: record.size.clone(),
        import: None,
        base: None,
        parent_snapshot_id: None,
        retired: None,
    };
    let mut record = record;
    record.place = place.map(str::to_string);
    Rc::new(RefCell::new(LiveBuilder { record, builder, sealable: true, life, reach: None, recipe: None }))
}

#[test]
fn public_reads_the_builders_map_as_swept_builders() {
    run(|w| async move {
        let own = live_builder(&w, "bld_1", BuilderLife::Own, false, None);
        let stale = live_builder(&w, "bld_2", BuilderLife::Stale, true, Some("default"));
        {
            let builders = BootRuntime::builders(&*w.c);
            builders.borrow_mut().insert("bld_1".into(), own);
            builders.borrow_mut().insert("bld_2".into(), stale);
        }
        let swept = PublicCtx::builders(&*w.c);
        assert_eq!(swept.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), vec!["bld_1", "bld_2"]);
        assert_eq!(swept[0].life, BuilderLife::Own);
        assert!(!swept[0].building);
        assert_eq!(swept[0].place, None);
        assert_eq!(swept[0].created_at, "2026-10-20T10:00:00.000Z");
        assert_eq!(swept[1].life, BuilderLife::Stale);
        assert!(swept[1].building);
        assert_eq!(swept[1].place.as_deref(), Some("default"));
        assert!(!swept[1].heartbeat_held);
    });
}

// Waits on packet 14 (the builders' context): forgetting a builder drops it from the map and the store.
#[test]
fn public_forgets_a_builder_from_the_map_and_the_store() {
    run(|w| async move {
        let held = live_builder(&w, "bld_1", BuilderLife::Stale, false, None);
        BootRuntime::builders(&*w.c).borrow_mut().insert("bld_1".into(), held);
        w.store.put("builders", "bld_1", json!({ "id": "bld_1" })).await.expect("put");
        PublicCtx::forget_builder(&*w.c, "bld_1").await.expect("forget");
        assert!(PublicCtx::builders(&*w.c).is_empty());
        assert_eq!(w.store.get("builders", "bld_1").await.expect("get"), None);
    });
}

// Waits on packet 14: the hold writes the builder's heartbeat through the builders' context.
#[test]
fn public_holds_an_own_builder_and_stops_one_by_its_place() {
    run(|w| async move {
        let held = live_builder(&w, "bld_1", BuilderLife::Own, false, None);
        BootRuntime::builders(&*w.c).borrow_mut().insert("bld_1".into(), held);
        PublicCtx::hold(&*w.c, "bld_1").await.expect("hold");
        PublicCtx::stop_builder(&*w.c, "bld_1", "default").await.expect("stop");
    });
}

// Waits on packet 14: the grace sweep reads the builders' grace timers.
#[test]
fn public_expires_no_grace_on_an_empty_state() {
    run(|w| async move {
        let (reaped, failed) = PublicCtx::expire_grace(&*w.c).await.expect("expire");
        assert!(reaped.is_empty() && failed.is_empty());
    });
}

#[test]
fn public_names_the_wired_place_first_then_the_rest_with_whether_a_backend_answers() {
    run(|w| async move {
        assert_eq!(PublicCtx::wired_place(&*w.c), "default");
        assert_eq!(PublicCtx::sweep_places(&*w.c), vec![("default".to_string(), true)]);
        assert_eq!(PublicCtx::place_name(&*w.c, "default"), wsp_words::place_word::provider_key_name("default"));
    });
}

#[test]
fn public_lists_the_machines_a_place_holds() {
    run(|w| async move {
        let spec: wsp_engine::machine::MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
        let machine = w.backend.create(spec, None, None).await.expect("a machine");
        let rows = PublicCtx::list_at(&*w.c, "default").await.expect("list");
        assert_eq!(rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec![machine.id().to_string()]);
        let missing = PublicCtx::list_at(&*w.c, "p_none").await;
        assert!(missing.is_err(), "a place this host holds no backend for lists nothing");
    });
}

#[test]
fn public_reads_the_live_records_as_swept_entries() {
    run(|w| async move {
        let entry = attached(&w, "w_a", Some("default"), "proj_1").await;
        let machine_id = entry.borrow().machine.id().to_string();
        let rows = PublicCtx::entries(&*w.c);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.id, "w_a");
        assert_eq!(row.machine_id, "m_w_a");
        assert_eq!(row.entry_machine_id, machine_id);
        assert!(!row.creating && !row.napping && !row.waking && !row.deleting);
        assert!(!row.absent_machine);
        assert_eq!(row.phase, wsp_wire::views::workspace::WorkspacePhase::Running);
    });
}

#[test]
fn public_claims_every_machine_id_something_holds() {
    run(|w| async move {
        let entry = attached(&w, "w_a", Some("default"), "proj_1").await;
        let held = live_builder(&w, "bld_1", BuilderLife::Own, false, None);
        BootRuntime::builders(&*w.c).borrow_mut().insert("bld_1".into(), held);
        let machine_id = entry.borrow().machine.id().to_string();
        let known = PublicCtx::known_ids(&*w.c);
        assert!(known.contains(&"m_w_a".to_string()));
        assert!(known.contains(&machine_id));
        assert!(known.contains(&"bld_1".to_string()));
    });
}

#[test]
fn public_reads_a_machines_state_and_says_when_it_is_missing() {
    run(|w| async move {
        let entry = attached(&w, "w_a", Some("default"), "proj_1").await;
        assert_eq!(PublicCtx::read_machine(&*w.c, "w_a").await, MachineRead::State(MachineState::Running));
        let machine = entry.borrow().machine.clone();
        machine.kill().await.expect("kill");
        match PublicCtx::read_machine(&*w.c, "w_a").await {
            MachineRead::Missing { .. } => {}
            other => panic!("a killed machine reads missing, read {other:?}"),
        }
    });
}

#[test]
fn public_settles_a_record_gone_from_the_words_the_sweep_read() {
    run(|w| async move {
        let entry = attached(&w, "w_a", None, "proj_1").await;
        BootRuntime::projects_held(&*w.c).borrow_mut().insert("proj_1".into(), project_view("proj_1", "default"));
        let machine = entry.borrow().machine.clone();
        // The record names the machine the provider holds, so the confirming reads ask the provider about it.
        entry.borrow().record.borrow_mut().machine_id = machine.id().to_string();
        machine.kill().await.expect("kill");
        // The confirming reads wait on the runtime's clock, which moves only when the test says so.
        let clock = w.clock.clone();
        let ticking = tokio::task::spawn_local(async move {
            loop {
                clock.advance(1_000);
                tokio::task::yield_now().await;
            }
        });
        PublicCtx::adopt_gone(&*w.c, "w_a", "the machine m_w_a is gone".to_string()).await.expect("adopt gone");
        ticking.abort();
        assert_eq!(entry.borrow().record.borrow().phase, wsp_wire::views::workspace::WorkspacePhase::Gone);
    });
}

// Waits on packet 5's `reread_held` reaching the live entry only: an entry the store lacks reads nothing again.
#[test]
fn public_rereads_and_recovers_a_held_record() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        let _ = PublicCtx::reread_held(&*w.c, "w_a", "sweep").await;
        PublicCtx::recover_gone(&*w.c, "w_none").await.expect_err("an id no record holds");
    });
}

// Waits on packet 14 and 15 (the replay, the adoption and the strays read the builders' and status contexts).
#[test]
fn public_sweep_steps_find_nothing_on_an_empty_listing() {
    run(|w| async move {
        let mut failed = vec![];
        let known = || vec![];
        assert!(PublicCtx::replay_creates(&*w.c, &known, &mut failed).await.expect("replay").is_empty());
        let mut claimed = vec![];
        let (found, lost) = PublicCtx::adopt_lost(&*w.c, "default", &[], &mut claimed, &mut failed).await.expect("adopt");
        assert!(found.is_empty() && lost.is_empty());
        let swept = PublicCtx::engine_reap(&*w.c, "default", vec![], vec![], None).await.expect("reap");
        assert!(swept.reaped.is_empty() && swept.spared.is_empty());
        let strays = PublicCtx::sweep_strays(&*w.c, vec![], &mut failed, &["default".to_string()]).await.expect("strays");
        assert!(strays.is_empty());
        assert!(failed.is_empty());
    });
}

#[test]
fn public_engine_reap_stops_nothing_this_host_claims() {
    run(|w| async move {
        let spec: wsp_engine::machine::MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
        let kept = w.backend.create(spec.clone(), None, None).await.expect("a machine");
        BootRuntime::set_owner(&*w.c, "owner_1");
        let rows = PublicCtx::list_at(&*w.c, "default").await.expect("list");
        let swept = PublicCtx::engine_reap(&*w.c, "default", rows, vec![kept.id().to_string()], None).await.expect("reap");
        assert!(swept.reaped.is_empty(), "the one machine is claimed, so nothing is stopped");
        let after = PublicCtx::list_at(&*w.c, "default").await.expect("list");
        assert_eq!(after.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec![kept.id().to_string()]);
    });
}

#[test]
fn public_has_no_kept_agent_to_reap_on_an_empty_state() {
    run(|w| async move {
        assert!(PublicCtx::kept_agents(&*w.c).is_empty());
        PublicCtx::reap_kept(&*w.c, "t_none").await;
    });
}

// ---- close_step: one test for the state each step writes, one for the steps that wait or run a callback --------

#[test]
fn close_marks_closing_then_closed() {
    run(|w| async move {
        assert!(!BootRuntime::closing(&*w.c) && !BootRuntime::closed(&*w.c));
        PublicCtx::close_step(&*w.c, CloseStep::MarkClosing).await.expect("step");
        assert!(BootRuntime::closing(&*w.c));
        assert!(!BootRuntime::closed(&*w.c));
        PublicCtx::close_step(&*w.c, CloseStep::MarkClosed).await.expect("step");
        assert!(BootRuntime::closed(&*w.c));
    });
}

#[test]
fn close_runs_each_turn_writer_held_at_close_once_and_empties_the_list() {
    run(|w| async move {
        let held = wsp_threads::threads::turns::TurnsCtx::held_at_close(&*w.c);
        let ran = Rc::new(Cell::new(0));
        for _ in 0..2 {
            let ran = ran.clone();
            held.borrow_mut().push(Rc::new(move || ran.set(ran.get() + 1)));
        }
        PublicCtx::close_step(&*w.c, CloseStep::RunHeldAtClose).await.expect("step");
        assert_eq!(ran.get(), 2);
        assert!(held.borrow().is_empty());
        PublicCtx::close_step(&*w.c, CloseStep::RunHeldAtClose).await.expect("step again");
        assert_eq!(ran.get(), 2, "nothing runs twice");
    });
}

#[test]
fn close_steps_that_wait_on_nothing_end_on_an_idle_runtime() {
    run(|w| async move {
        for step in [
            CloseStep::ClearRootsRecheck,
            CloseStep::AwaitCopiesMoving,
            CloseStep::StopSweep,
            CloseStep::AwaitSweeping,
            CloseStep::StopDiskWatch,
            CloseStep::CloseIdle,
            CloseStep::CloseAlerts,
            CloseStep::CloseSlates,
            CloseStep::CloseAgentsReader,
            CloseStep::SettleDaemonSyncs,
            CloseStep::SettleKeptWrites,
            CloseStep::SettleAgentAsks,
            CloseStep::StopMachineReadings,
            CloseStep::ClosePipes,
            CloseStep::CloseLocal,
            CloseStep::ClosePlaceDoor,
            CloseStep::CancelBeat,
            CloseStep::CancelTimers,
            CloseStep::StopPrPolls,
            CloseStep::CloseGoneWatch,
            CloseStep::AwaitTicking,
            CloseStep::FreeOwnBuilders,
            CloseStep::FlushTranscripts,
            CloseStep::AwaitTranscriptWrites,
        ] {
            PublicCtx::close_step(&*w.c, step).await.unwrap_or_else(|e| panic!("{step:?}: {}", e.message));
        }
    });
}

// Waits on packet 14 (the builders' context): the store write goes through it.
#[test]
fn close_frees_the_builders_this_host_held_and_leaves_the_others_held() {
    run(|w| async move {
        let own = live_builder(&w, "bld_1", BuilderLife::Own, false, None);
        own.borrow_mut().record.held_by = Some(wsp_core::types::internal::HeldBy { host: "h".into(), pid: 1, heartbeat: "t".into() });
        let reusable = live_builder(&w, "bld_2", BuilderLife::Reusable, false, None);
        reusable.borrow_mut().record.held_by = Some(wsp_core::types::internal::HeldBy { host: "h".into(), pid: 1, heartbeat: "t".into() });
        {
            let builders = BootRuntime::builders(&*w.c);
            builders.borrow_mut().insert("bld_1".into(), own.clone());
            builders.borrow_mut().insert("bld_2".into(), reusable.clone());
        }
        PublicCtx::close_step(&*w.c, CloseStep::FreeOwnBuilders).await.expect("step");
        assert!(own.borrow().record.held_by.is_none());
        assert!(reusable.borrow().record.held_by.is_some());
        let stored = w.store.get("builders", "bld_1").await.expect("get").expect("the record is written");
        assert!(stored.get("heldBy").is_none());
    });
}

#[test]
fn close_ends_the_whole_close_on_an_idle_runtime_and_marks_it_closed() {
    run(|w| async move {
        let rt = w.rt.clone();
        rt.close().await.expect("close");
        assert!(BootRuntime::closed(&*w.c));
        assert!(BootRuntime::closing(&*w.c));
    });
}

#[test]
fn close_cancels_a_snooze_so_its_wake_never_says_the_thread_marked() {
    run(|w| async move {
        let (seen, _off) = listen(&w, "thread.marked");
        BootRuntime::thread_records(&*w.c)
            .borrow_mut()
            .insert("t_a".into(), thread_record(json!({ "workspaceId": "w_1", "harness": "claude", "snoozedUntil": 1_005_000 })));
        BootRuntime::wake_at(&*w.c, "t_a", 1_005_000.0);
        PublicCtx::close_step(&*w.c, CloseStep::CancelTimers).await.expect("step");
        w.clock.advance(6_000);
        tokio::task::yield_now().await;
        assert!(seen.borrow().is_empty());
    });
}

// A runtime that is closed and dropped is freed: the context and its areas hold each other, and the close is where
// that is cut. Waits on a `Late::clear` in wsp-core (outside this packet's two files): the slot is a `OnceCell` the
// context cannot empty through a shared `Rc`.
#[test]
#[ignore = "waits on Late::clear in wsp-core, which this packet's files cannot add"]
fn a_closed_runtime_is_freed_when_it_is_dropped() {
    run(|w| async move {
        let World { rt, c, .. } = w;
        let weak = Rc::downgrade(&c);
        rt.close().await.expect("close");
        drop(rt);
        drop(c);
        assert_eq!(weak.strong_count(), 0, "the context is freed");
    });
}

// ---- HarnessesCtx -------------------------------------------------------------------------------------------

fn catalog(models: Value) -> wsp_wire::views::harness::HarnessCatalog {
    serde_json::from_value(json!({
        "source": "harness", "version": null, "contextWindows": [], "steers": false, "renames": false, "images": false,
        "harness": "claude", "label": "Claude", "models": models, "efforts": [], "permissionModes": [],
    }))
    .expect("a catalog")
}

/// The preferences a fresh state holds, with the keys of `more` laid over them.
fn prefs(more: Value) -> wsp_wire::views::preferences::Preferences {
    let mut base = json!({
        "theme": "dark", "lightTheme": "paper", "darkTheme": "graphite", "sidebarMode": "list", "terminalSize": "app",
        "terminalZoom": {}, "access": {}, "projectLook": {}, "projectIcon": {}, "computerLook": {}, "serverIcons": true,
        "agentVersions": true, "usageLogs": true, "productUsage": true, "keepAwake": true, "transparency": true,
        "projectOrder": [], "keybindings": {}, "appFont": "", "codeFont": "", "agentDefaults": {}, "projectDefaults": {},
        "sendWith": "enter", "midTurn": "queue", "notifyNeeds": "notify-sound", "notifyDone": "notify", "planAlerts": true,
        "settleAfter": "2h", "askDelete": true, "onQuit": "ask", "newThreadIn": "current", "labs": false,
    });
    for (key, value) in more.as_object().expect("keys") {
        base[key] = value.clone();
    }
    serde_json::from_value(base).expect("preferences")
}

#[test]
fn shaped_hides_orders_and_moves_the_mark_in_the_catalogs_own_key_order() {
    run(|w| async move {
        let models = json!([
            { "value": "a", "label": "A", "isDefault": true },
            { "value": "b", "label": "B" },
            { "value": "c", "label": "C" },
        ]);
        let held = prefs(json!({ "agentDefaults": { "claude": { "models": { "hide": ["a"], "order": ["c"] } } } }));
        let shaped = HarnessesCtx::shaped(&*w.c, &catalog(models), &held, None);
        let keys: Vec<&str> = shaped.keys().map(String::as_str).collect();
        let at = |k: &str| keys.iter().position(|x| *x == k).unwrap_or_else(|| panic!("{k} in {keys:?}"));
        assert!(at("models") < at("efforts") && at("efforts") < at("permissionModes") && at("permissionModes") < at("hiddenModels"));
        assert_eq!(
            shaped["models"],
            json!([{ "value": "c", "label": "C", "isDefault": true }, { "value": "b", "label": "B" }]),
            "the order the person set, and the mark moved to the first one shown"
        );
        assert_eq!(shaped["hiddenModels"], json!([{ "value": "a", "label": "A" }]));
    });
}

#[test]
fn shaped_adds_the_persons_own_model_ids_after_the_binarys() {
    run(|w| async move {
        let models = json!([{ "value": "a", "label": "A" }]);
        let held = prefs(json!({ "agentDefaults": { "claude": { "models": { "custom": ["mine"] } } } }));
        let shaped = HarnessesCtx::shaped(&*w.c, &catalog(models), &held, None);
        assert_eq!(shaped["models"], json!([{ "value": "a", "label": "A" }, { "value": "mine", "label": "mine", "added": true }]));
        assert!(!shaped.contains_key("hiddenModels"));
    });
}

#[test]
fn shaped_leaves_a_catalog_with_no_shaping_as_the_agent_wrote_it() {
    run(|w| async move {
        let models = json!([{ "value": "a", "label": "A" }, { "value": "b", "label": "B" }]);
        let shaped = HarnessesCtx::shaped(&*w.c, &catalog(models.clone()), &prefs(json!({})), None);
        assert_eq!(shaped["models"], models);
        assert_eq!(shaped["harness"], json!("claude"));
        assert!(!shaped.contains_key("hiddenModels") && !shaped.contains_key("unshaped") && !shaped.contains_key("isDefault"));
    });
}

// Waits on packet 16 (the preferences area).
#[test]
fn harnesses_read_the_preferences_the_person_holds() {
    run(|w| async move {
        let held = HarnessesCtx::preferences(&*w.c).await.expect("preferences");
        assert!(held.project_defaults.is_empty());
        assert!(held.agent_defaults.is_empty());
    });
}

#[test]
fn harnesses_say_a_workspace_talks_to_its_own_tree_only_for_a_caller_in_it() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        assert!(!HarnessesCtx::talks_to_tree_on(&*w.c, "w_a", None));
        assert!(!HarnessesCtx::talks_to_tree_on(&*w.c, "w_none", None));
    });
}

#[test]
fn harnesses_name_the_entry_a_list_is_read_for_by_its_record_and_project() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        let found = HarnessesCtx::entry_of(&*w.c, "w_a", None).await.expect("entry");
        assert_eq!(found, Some(("w_a".to_string(), "proj_1".to_string())));
        let missing = HarnessesCtx::entry_of(&*w.c, "w_none", None).await;
        assert!(missing.is_err(), "an id this host does not hold has no catalog to list");
    });
}

// Waits on packet 11 (the agents' context): the project's and the computer's home read the agents area.
#[test]
fn harnesses_find_the_home_of_a_project_and_of_a_computer() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        BootRuntime::projects_held(&*w.c).borrow_mut().insert("proj_1".into(), project_view("proj_1", "default"));
        assert_eq!(HarnessesCtx::project_home(&*w.c, "proj_none", None).await.expect("home"), None);
        assert_eq!(HarnessesCtx::computer_home(&*w.c, "p_none", None).await.expect("home"), None);
        let at_computer = HarnessesCtx::computer_home(&*w.c, "default", None).await.expect("home");
        assert_eq!(at_computer.map(|e| e.1), Some("proj_1".to_string()));
    });
}

// Waits on packet 11 and 16: the lists and the default agent read the agents area and the preferences.
#[test]
fn harnesses_list_every_agent_and_name_the_default() {
    run(|w| async move {
        let lists = HarnessesCtx::agent_lists(&*w.c, None).await.expect("lists");
        assert!(!lists.is_empty(), "the catalog knows its agents");
        let held = HarnessesCtx::preferences(&*w.c).await.expect("preferences");
        let agent = HarnessesCtx::default_agent_of(&*w.c, &held, None);
        assert!(lists.iter().any(|c| c.harness == agent), "the default is one of the agents listed");
    });
}

// ---- StatusCtx (serve) --------------------------------------------------------------------------------------

#[test]
fn status_lists_for_no_caller_every_live_workspace() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        attached(&w, "w_b", None, "proj_1").await;
        let mut shown = StatusCtx::listed_for(&*w.c, None).expect("listed");
        shown.sort();
        assert_eq!(shown, vec!["w_a".to_string(), "w_b".to_string()]);
    });
}

#[test]
fn status_refuses_no_id_for_no_caller() {
    run(|w| async move {
        attached(&w, "w_a", Some("default"), "proj_1").await;
        StatusCtx::refuse_named(&*w.c, "w_a", None).expect("a caller of the person's own sees all");
        StatusCtx::refuse_named(&*w.c, "w_gone", None).expect("a cost read answers for a workspace this host no longer holds");
    });
}

// Waits on packet 15 (the status tracker's fill in `build_areas_of`): the cost history and the probed list.
#[test]
fn status_reads_the_cost_history_and_the_probed_rows() {
    run(|w| async move {
        assert!(StatusCtx::history(&*w.c, "w_none").await.expect("history").is_empty());
        let rows = StatusCtx::probe_list(&*w.c, None).await.expect("probe");
        assert!(rows.is_empty());
        attached(&w, "w_a", Some("default"), "proj_1").await;
        let rows = StatusCtx::probe_list(&*w.c, None).await.expect("probe");
        assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["w_a"]);
    });
}

// Waits on packet 15: the public status members read the tracker through `StatusCtx`.
#[test]
fn status_history_through_the_public_member_answers_for_a_workspace_this_host_no_longer_holds() {
    run(|w| async move {
        let history = wsp_serve::runtime::public::status_history(&*w.c, "w_none", None).await.expect("history");
        assert!(history.is_empty());
        let rows = wsp_serve::runtime::public::status_list(&*w.c, None, None).await.expect("list");
        assert!(rows.is_empty());
    });
}
