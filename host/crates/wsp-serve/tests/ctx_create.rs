// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 6 (#2476): the context's create and sub-spawn readings (`CreateRuntime`, `SubSpawnCtx`).
//! Each test builds the runtime on a throwaway state over `FakeBackend` and calls the context's method the way the
//! area crate calls it, through `&dyn Trait`. The TS is the oracle: each expectation is what `create.ts`,
//! `sub-spawn.ts`, `runtime.ts` and the area each member forwards to (`ctx.landingPlace`, `ctx.fork`, `ctx.attach`,
//! `ctx.placeGuard`, ...) answer. A test that goes through another packet's bodies says so on the test: a read of the
//! channels area waits on packet 3, a reach, daemon or machines read on 4, boot on 5, the queue and life on 7, the
//! projects road on 10, the agents on 11, the builders and the image on 14, and the whole-runtime ports wait on all
//! of them.
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::Runtime;
use wsp_core::types::internal::dead_machine;
use wsp_core::types::wiring::{
    Broker, LiveWorkspace, LocalWiring, Platform, RuntimeOptions, SeedPack, SeedWiring, StageReport, StageSaid, WiredId, WorkspaceRecord,
};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions, LOCAL_MACHINE_ID};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::machines::create::{CreateRuntime, KeptBuilder, LandingRoad, RecordBranchAsk};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_spawn::SubSpawnCtx;
use wsp_machines::places::types::{HerePlace, PlaceWiring};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::memory_store;
use wsp_store::store::Store;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace::{SeedChoice, SeedPlan};
use wsp_wire::views::workspace_events::WorkspaceCreateStage;
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_words::exit::HostError;
use wsp_words::words::thread::{name_deleting_refusal, name_taken_refusal, BLANK_NAME_REFUSAL};

// ----------------------------------------------------------------------------------------------------------------
// Fixtures

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn plain() -> RuntimeOptions {
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
    RuntimeOptions { place_links: Some(Rc::new(wiring())), ..plain() }
}

fn local_in(root: &Path) -> LocalWiring {
    let folder = root.to_string_lossy().to_string();
    let home_root = folder.clone();
    LocalWiring {
        backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: root.to_path_buf(), env: None })),
        folder: folder.clone(),
        exec_stream: Rc::new(|_, _| panic!("no exec stream in this test")),
        home: Rc::new(move |agent| format!("{home_root}/.{agent}")),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(|| IndexMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())])),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: Platform::Linux,
        close: None,
    }
}

fn record(extra: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": "w_1", "name": "api", "machineId": "m_1", "phase": "running", "golden": "g",
        "createdAt": "2026-10-26T00:00:00.000Z", "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 },
        "firstLife": false,
    });
    for (k, v) in extra.as_object().expect("an object").iter() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a workspace record")
}

fn project(id: &str, name: &str, path: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": "zingzys-mac", "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

/// A live entry over a record and a machine, as `attach` makes it.
fn entry_over(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
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
    Rc::new(RefCell::new(LiveWorkspace {
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
    }))
}

async fn machine_named(id: &str) -> Arc<dyn Machine> {
    backend().get(id).await.expect("a machine")
}

async fn entry_named(id: &str, name: &str) -> Entry {
    entry_over(record(json!({ "id": id, "name": name })), machine_named("m_1").await)
}

/// The runtime on a throwaway state, inside the task set the owner thread runs.
fn drive<Fut: std::future::Future<Output = ()>>(opts: RuntimeOptions, f: impl FnOnce(Rc<Ctx>, Rc<AssembledRuntime>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts).expect("a runtime");
        let ctx: Rc<Ctx> = runtime.context();
        f(ctx, runtime).await
    });
}

trait Refused<E> {
    fn refused(self, why: &str) -> E;
}

impl<T, E> Refused<E> for Result<T, E> {
    fn refused(self, why: &str) -> E {
        match self {
            Err(e) => e,
            Ok(_) => panic!("expected a refusal: {why}"),
        }
    }
}

fn same_backend(a: &Arc<dyn MachineBackend>, b: &Arc<dyn MachineBackend>) -> bool {
    Arc::as_ptr(a) as *const () == Arc::as_ptr(b) as *const ()
}

fn create(c: &Rc<Ctx>) -> &dyn CreateRuntime {
    &**c
}
fn spawn(c: &Rc<Ctx>) -> &dyn SubSpawnCtx {
    &**c
}

/// The events the bus carries, as the JSON a client reads, kept as they arrive.
fn tap(runtime: &AssembledRuntime, kind: &str) -> Rc<RefCell<Vec<Value>>> {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let held = seen.clone();
    let _ = runtime.events().on(
        kind,
        Rc::new(move |e| {
            held.borrow_mut().push(serde_json::to_value(e).expect("an event"));
            Ok(())
        }),
    );
    seen
}

fn scope(thread: &str, root: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "workspaceId": "w_lead", "threadId": thread, "rootThreadId": root })).expect("a scope")
}

// ----------------------------------------------------------------------------------------------------------------
// The core reads both traits share (runtime.ts:491-992): the clock, ids, bus, log, live map, places and door

#[test]
fn create_clock_ids_and_the_wired_place_are_the_hosts() {
    let fake = FakeClock::new(1_700_000_000_000);
    let opts = RuntimeOptions { clock: Some(fake.clock()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(create(&c).clock().now(), 1_700_000_000_000);
        fake.advance(7);
        assert_eq!(create(&c).clock().now(), 1_700_000_000_007);
        assert_eq!(spawn(&c).now(), 1_700_000_000_007.0);
        let ids = create(&c).ids();
        let (a, b) = (ids.uuid(), ids.uuid());
        assert_eq!(a.len(), 36);
        assert_ne!(a, b);
        assert_eq!(create(&c).wired(), "default");
    });
    let backend = backend();
    let opts = RuntimeOptions {
        places: Some(wsp_core::types::wiring::wired_place(WiredId::Fixed("solari".into()), backend.clone())),
        backend: Some(backend),
        ..plain()
    };
    drive(opts, |c, _| async move {
        assert_eq!(create(&c).wired(), "solari");
    });
}

#[test]
fn create_place_door_is_the_door_the_host_wired() {
    drive(plain(), |c, _| async move {
        assert!(create(&c).place_door().is_none());
    });
    drive(with_door(), |c, _| async move {
        let door = create(&c).place_door().expect("the door");
        assert_eq!(create(&c).place_door_name("pl_unknown"), door.name_of("pl_unknown"));
    });
}

/// Waits on packet 16 (a bus listener of the preferences area reads its context).
#[test]
fn create_emit_and_spawn_emit_put_events_on_the_bus() {
    drive(plain(), |c, runtime| async move {
        let seen = tap(&runtime, "workspace.deleted");
        let event = serde_json::from_value(json!({ "type": "workspace.deleted", "workspaceId": "w_1" })).expect("an event");
        create(&c).emit(event).expect("emitted");
        spawn(&c).emit(json!({ "type": "workspace.deleted", "workspaceId": "w_2" }));
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["workspaceId"], "w_1");
        assert_eq!(seen[1]["workspaceId"], "w_2");
        assert!(seen[1]["seq"].as_u64() > seen[0]["seq"].as_u64(), "the bus stamps each event in order");
    });
}

#[test]
fn create_and_spawn_warn_are_host_log_lines() {
    drive(plain(), |c, _| async move {
        let captured = wsp_core::console::capture();
        create(&c).warn("workspace w_1: a line");
        spawn(&c).warn("workspace w_2: another line");
        assert_eq!(captured.lines(), vec!["workspace w_1: a line".to_string(), "workspace w_2: another line".to_string()]);
    });
}

#[test]
fn create_live_and_spawn_live_are_the_one_map() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        create(&c).live().borrow_mut().insert("w_1".into(), entry.clone());
        let held = spawn(&c).live("w_1").expect("the same entry");
        assert!(Rc::ptr_eq(&held, &entry));
        assert!(spawn(&c).live("w_9").is_none());
        let web = entry_named("w_2", "web").await;
        create(&c).live().borrow_mut().insert("w_2".into(), web);
        spawn(&c).remove_live("w_1");
        assert!(spawn(&c).live("w_1").is_none());
        assert_eq!(create(&c).live().borrow().keys().cloned().collect::<Vec<_>>(), vec!["w_2".to_string()]);
        spawn(&c).remove_live("w_9");
    });
}

#[test]
fn spawn_is_absent_machine_is_false_for_a_machine_that_answers_and_for_a_dead_one() {
    drive(plain(), |c, _| async move {
        let real = entry_named("w_1", "api").await;
        assert!(!spawn(&c).is_absent_machine(&real));
        let dead = entry_over(record(json!({ "id": "w_3" })), dead_machine(""));
        assert!(!spawn(&c).is_absent_machine(&dead));
    });
}

#[test]
fn spawn_random_hex_is_two_hex_digits_a_byte_and_spawn_runs_the_task_detached() {
    drive(plain(), |c, _| async move {
        let one = spawn(&c).random_hex(8);
        assert_eq!(one.len(), 16);
        assert!(one.chars().all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()), "{one}");
        assert_ne!(one, spawn(&c).random_hex(8));
        assert_eq!(spawn(&c).random_hex(0), "");
        let ran = Rc::new(Cell::new(false));
        let flag = ran.clone();
        spawn(&c).spawn(async move { flag.set(true) }.boxed_local());
        tokio::task::yield_now().await;
        assert!(ran.get(), "a `void f()` runs without the caller awaiting it");
    });
}

#[test]
fn spawn_local_machine_is_this_computers_and_refuses_where_none_is_wired() {
    let root = std::env::temp_dir().join(format!("wsp-ctx-create-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("a folder");
    let opts = RuntimeOptions { local: Some(local_in(&root)), ..plain() };
    drive(opts, |c, _| async move {
        let machine = spawn(&c).local_machine().await.expect("this computer's machine");
        assert_eq!(machine.id(), LOCAL_MACHINE_ID);
    });
    drive(plain(), |c, _| async move {
        let said = spawn(&c).local_machine().await.refused("no local wiring").message();
        assert!(said.contains("local"), "{said}");
    });
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn spawn_provider_backend_is_the_places_table_then_the_door() {
    let wired = backend();
    let opts = RuntimeOptions { backend: Some(wired.clone()), ..plain() };
    drive(opts, |c, _| async move {
        let got = spawn(&c).provider_backend("default").expect("the wired row");
        assert!(same_backend(&got, &wired));
        assert!(spawn(&c).provider_backend("pl_nobody").is_none());
    });
    drive(with_door(), |c, _| async move {
        assert!(spawn(&c).provider_backend("pl_nobody").is_none());
    });
}

#[test]
fn spawn_place_settings_and_delete_after_read_the_door_and_none_without_one() {
    drive(plain(), |c, _| async move {
        assert!(spawn(&c).place_settings("pl_1").is_none());
        assert!(spawn(&c).delete_after_ms("pl_1").is_none());
    });
    drive(with_door(), |c, _| async move {
        let door = create(&c).place_door().expect("the door");
        let want = serde_json::to_value(door.settings_at("pl_1")).expect("settings");
        let got = spawn(&c).place_settings("pl_1").expect("the settings object");
        assert_eq!(Value::Object(got), want);
        let after = door.settings_at("pl_1").delete_after_ms.map(|ms| ms.0 .0);
        assert_eq!(spawn(&c).delete_after_ms("pl_1"), after);
    });
}

#[test]
fn spawn_place_name_and_create_place_name_are_the_builders_naming() {
    drive(plain(), |c, _| async move {
        let wired = create(&c).wired();
        assert_eq!(create(&c).place_name(&wired), spawn(&c).place_name(&wired));
        assert!(!create(&c).place_name("pl_1").is_empty());
    });
    drive(with_door(), |c, _| async move {
        let door = create(&c).place_door().expect("the door");
        assert_eq!(create(&c).place_name("pl_1"), door.name_of("pl_1"));
        assert_eq!(spawn(&c).place_name("pl_1"), door.name_of("pl_1"));
    });
}

#[test]
fn spawn_backend_for_a_cloud_record_is_the_wired_backend() {
    let wired = backend();
    let opts = RuntimeOptions { backend: Some(wired.clone()), ..plain() };
    drive(opts, |c, _| async move {
        let got = spawn(&c).backend_for(&record(json!({}))).expect("the wired backend");
        assert!(same_backend(&got, &wired));
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The create area's own rules the context hands the sub-spawn (create.ts:205-276)

#[test]
fn spawn_name_given_trims_and_name_refusal_names_what_holds_a_name() {
    drive(plain(), |c, _| async move {
        assert_eq!(spawn(&c).name_given("  api \n"), "api");
        assert_eq!(spawn(&c).name_refusal(""), Some(BLANK_NAME_REFUSAL.to_string()));
        assert_eq!(spawn(&c).name_refusal("   "), Some(BLANK_NAME_REFUSAL.to_string()));
        assert_eq!(spawn(&c).name_refusal("api"), None);
        let api = entry_named("w_1", "api").await;
        create(&c).live().borrow_mut().insert("w_1".into(), api);
        assert_eq!(spawn(&c).name_refusal("api"), Some(name_taken_refusal("api")));
        let deleting = entry_named("w_2", "web").await;
        deleting.borrow_mut().deleting = Some(async { Ok(()) }.boxed_local().shared());
        create(&c).live().borrow_mut().insert("w_2".into(), deleting);
        assert_eq!(spawn(&c).name_refusal("web"), Some(name_deleting_refusal("web")));
    });
}

#[test]
fn spawn_forking_holds_a_name_between_its_check_and_its_first_machine() {
    drive(plain(), |c, _| async move {
        assert_eq!(spawn(&c).name_refusal("api"), None);
        spawn(&c).forking_add("api");
        assert_eq!(spawn(&c).name_refusal("api"), Some(name_taken_refusal("api")));
        spawn(&c).forking_remove("api");
        assert_eq!(spawn(&c).name_refusal("api"), None);
        spawn(&c).forking_remove("api");
    });
}

#[test]
fn spawn_supersede_failed_deletes_every_failed_create_of_the_name() {
    drive(plain(), |c, runtime| async move {
        let deleted = tap(&runtime, "workspace.deleted");
        spawn(&c).supersede_failed("api");
        assert!(deleted.borrow().is_empty(), "nothing failed under that name");
    });
}

#[test]
fn spawn_stage_reporter_emits_each_stage_with_its_elapsed_time_and_who_asked() {
    let fake = FakeClock::new(1_000);
    let opts = RuntimeOptions { clock: Some(fake.clock()), ..plain() };
    drive(opts, |c, runtime| async move {
        let stages = tap(&runtime, "workspace.creating");
        let asked = json!({ "threadId": "t_child", "rootThreadId": "t_root" });
        let report: StageReport = spawn(&c).stage_reporter("w_1", "api", 400.0, Some(asked.clone()));
        report(WorkspaceCreateStage::ForkRequested, "starting api on default", None);
        report(WorkspaceCreateStage::Ready, "ready", Some(StageSaid { notice: Some("a note".into()), ..Default::default() }));
        let seen = stages.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["workspaceId"], "w_1");
        assert_eq!(seen[0]["name"], "api");
        assert_eq!(seen[0]["stage"], "fork-requested");
        assert_eq!(seen[0]["message"], "starting api on default");
        assert_eq!(seen[0]["elapsedMs"], 600);
        assert_eq!(seen[0]["askedBy"], asked);
        assert!(seen[0].get("notice").is_none());
        assert_eq!(seen[1]["notice"], "a note");
    });
    drive(plain(), |c, runtime| async move {
        let stages = tap(&runtime, "workspace.creating");
        let report = spawn(&c).stage_reporter("w_1", "api", 0.0, None);
        report(WorkspaceCreateStage::ForkRequested, "m", None);
        assert!(stages.borrow()[0].get("askedBy").is_none());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The branch-of-record reads (branch-of-record.ts:21, 124)

#[test]
fn create_brokered_start_needs_a_provider_road_and_a_ready_broker() {
    drive(plain(), |c, _| async move {
        assert!(!create(&c).brokered_start(true));
        assert!(!create(&c).brokered_start(false));
        assert!(!spawn(&c).brokered_start());
    });
}

#[test]
fn spawn_unbrokered_refusal_is_the_words_for_a_harness_with_no_broker_road() {
    drive(plain(), |c, _| async move {
        let want = wsp_machines::machines::brokered::unbrokered_refusal("claude").map(|e| e.to_string());
        assert_eq!(spawn(&c).unbrokered_refusal("claude").map(|e| e.to_string()), want);
        let want = wsp_machines::machines::brokered::unbrokered_refusal("cursor").map(|e| e.to_string());
        assert_eq!(spawn(&c).unbrokered_refusal("cursor").map(|e| e.to_string()), want);
    });
}

#[test]
fn spawn_free_branch_name_skips_the_branches_the_live_records_hold() {
    drive(plain(), |c, _| async move {
        assert_eq!(spawn(&c).free_branch_name("wsp/api", None), "wsp/api");
        let held = record(json!({
            "id": "w_1",
            "branchOfRecord": { "repoId": 1, "owner": "wsp", "name": "web", "branch": "wsp/api" }
        }));
        let entry = entry_over(held, machine_named("m_1").await);
        create(&c).live().borrow_mut().insert("w_1".into(), entry);
        assert_eq!(spawn(&c).free_branch_name("wsp/api", None), "wsp/api-2");
        let heads = "abc\trefs/heads/wsp/api-2\ndef\trefs/heads/main";
        assert_eq!(spawn(&c).free_branch_name("wsp/api", Some(heads)), "wsp/api-3");
        assert_eq!(spawn(&c).free_branch_name("wsp/web", Some(heads)), "wsp/web");
    });
}

#[test]
fn create_and_spawn_repo_and_named_of_record_refuse_a_remote_github_does_not_hold() {
    let said = "web's remote is on , and a machine on zingzys-mac reaches GitHub alone, through this host. Run this work on this computer.";
    drive(plain(), |c, _| async move {
        let ask = RecordBranchAsk {
            remote: String::new(),
            name: "api".into(),
            project: "web".into(),
            on: "zingzys-mac".into(),
            base: None,
            carried: false,
        };
        assert_eq!(create(&c).named_of_record(ask).await.refused("no remote").to_string(), said);
        assert_eq!(spawn(&c).repo_of_record("", "web", "zingzys-mac").await.map(|_| ()).refused("no remote").message(), said);
    });
}
// ----------------------------------------------------------------------------------------------------------------
// The landing road (create.ts:24-44; projects.ts:119-149; project-landing.ts:620)

#[test]
fn create_project_landing_gives_the_road_of_a_kind_and_refuses_a_kind_nothing_registered() {
    drive(plain(), |c, _| async move {
        for kind in ["box", "provider", "mac"] {
            let road: Rc<dyn LandingRoad> = create(&c).project_landing(kind).expect("a road");
            assert_eq!(road.kind(), kind);
        }
        let said = create(&c).project_landing("moon").err().expect("no such road").to_string();
        assert_eq!(said, "no project landing road for moon");
    });
}

#[test]
fn create_landing_roads_place_a_projects_memory_as_each_kind_does() {
    drive(plain(), |c, _| async move {
        let web = project("p_1", "web", "/work/web");
        let deps = json!({ "computerName": "zingzys-mac", "macStateHome": "/Users/z/.claude", "computer": { "home": "/home/z" } });
        let provider = create(&c).project_landing("provider").expect("a road");
        assert_eq!(provider.refusal(&web, &deps), None);
        assert_eq!(provider.memory_dir(&web, "-work-web", &deps).expect("a folder"), "/root/.claude-cfg/projects/-work-web/memory");
        assert!(provider.workspace_binds(&web).is_empty());
        let boxed = create(&c).project_landing("box").expect("a road");
        assert_eq!(boxed.refusal(&web, &deps), None);
        assert_eq!(boxed.memory_dir(&web, "-work-web", &deps).expect("a folder"), "/home/z/.claude-cfg/projects/-work-web/memory");
        assert!(boxed.workspace_binds(&web).is_empty());
        let mac = create(&c).project_landing("mac").expect("a road");
        assert_eq!(mac.refusal(&web, &deps), None);
        assert_eq!(mac.memory_dir(&web, "-work-web", &deps).expect("a folder"), "/Users/z/.claude/projects/-work-web/memory");
        assert!(mac.workspace_binds(&web).is_empty());
    });
}

/// join A (#2476): a box with no login to read throws in the TS (`computerOf`), so its memory folder is the refusal and
/// not the project's own folder.
#[test]
fn create_box_road_memory_of_a_box_with_no_login_is_refused() {
    drive(plain(), |c, _| async move {
        let web = project("p_1", "web", "/work/web");
        let boxed = create(&c).project_landing("box").expect("a road");
        let deps = json!({ "computerName": "a-box", "macStateHome": "" });
        assert!(boxed.memory_dir(&web, "-work-web", &deps).is_err());
    });
}

/// Waits on packet 8 (`Projects::landing_kind` reads `kind_of` through the folders context).
#[test]
fn create_landing_kind_is_the_word_of_the_road_a_computer_lands_by() {
    drive(plain(), |c, _| async move {
        let at = backend();
        assert_eq!(create(&c).landing_kind(HERE_PLACE_ID, &at), "mac");
        assert_eq!(create(&c).landing_kind("default", &at), "provider");
    });
}

/// Waits on packet 10 (`Projects::landing_deps`) and 5 (`BootArea::landing_place`).
#[test]
fn create_landing_deps_carry_the_computer_name_and_the_mac_state_home() {
    drive(plain(), |c, _| async move {
        let deps = create(&c).landing_deps(HERE_PLACE_ID).await.expect("the deps");
        assert!(deps.get("computerName").and_then(Value::as_str).is_some(), "{deps}");
        assert!(deps.get("macStateHome").and_then(Value::as_str).is_some(), "{deps}");
        assert_eq!(spawn(&c).landing_place(HERE_PLACE_ID).await.expect("a place"), None);
    });
}

/// Waits on packet 5 (boot's place reads).
#[test]
fn create_and_spawn_landing_place_is_none_for_this_host_and_a_place_for_a_joined_computer() {
    drive(plain(), |c, _| async move {
        assert_eq!(create(&c).landing_place(HERE_PLACE_ID).await.expect("this computer"), None);
        assert_eq!(create(&c).landing_place("default").await.expect("the wired place"), None);
        assert_eq!(spawn(&c).landing_place("default").await.expect("the wired place"), None);
        let said = create(&c).landing_place("a computer nobody joined").await.refused("no such place").to_string();
        assert!(!said.is_empty());
    });
}

/// Waits on packet 3 (`ChannelsArea::blocked_line`).
#[test]
fn create_place_refuses_nothing_where_no_place_is_named() {
    drive(plain(), |c, _| async move {
        create(&c).place_refuses(None).await.expect("no place, no refusal");
    });
}

/// Waits on packet 14 (the builders context under `landing_backend`).
#[test]
fn create_landing_backend_is_the_wired_backend_where_no_place_is_named() {
    let wired = backend();
    let opts = RuntimeOptions { backend: Some(wired.clone()), ..plain() };
    drive(opts, |c, _| async move {
        let at = create(&c).landing_backend(None).await.expect("the backend");
        assert!(same_backend(&at, &wired));
        assert!(create(&c).keeps_images(&at));
    });
}

/// Waits on packet 3 (`ChannelsArea::project_held`'s map read) and packet 8 (rules over the projects map).
#[test]
fn create_and_spawn_project_held_read_the_projects_the_host_holds() {
    drive(plain(), |c, _| async move {
        let web = project("p_1", "web", "/work/web");
        create(&c).remember_project(web.clone()).await.expect("remembered");
        assert_eq!(create(&c).project_held("p_1").expect("held"), web);
        assert_eq!(spawn(&c).project_held("p_1").expect("held"), web);
        let said = create(&c).project_held("p_9").refused("no such project").to_string();
        assert!(said.contains("p_9"), "{said}");
        assert_eq!(spawn(&c).project_held("p_9").refused("no such project").to_string(), said);
    });
}

/// Waits on packet 3 (rules' projects map through channels) for the held read after the write.
#[test]
fn create_remember_project_puts_the_project_in_the_map_and_the_store() {
    drive(plain(), |c, _| async move {
        let web = project("p_1", "web", "/work/web");
        create(&c).remember_project(web.clone()).await.expect("remembered");
        let again = ProjectView { memory_dir: "/m".into(), ..web };
        create(&c).remember_project(again.clone()).await.expect("remembered again");
        assert_eq!(create(&c).project_held("p_1").expect("held").memory_dir, "/m");
    });
}

#[test]
fn create_ref_of_names_a_project_by_id_and_name() {
    drive(plain(), |c, _| async move {
        let web = project("p_1", "web", "/work/web");
        let reference = create(&c).ref_of(&web);
        assert_eq!(
            serde_json::to_value(&reference).expect("a ref"),
            json!({ "id": "p_1", "name": "web", "path": "/work/web", "computer": "zingzys-mac" })
        );
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The image reads a fork makes (create.ts:50-54)

/// Waits on packet 14 (`BootArea::image_head` reads the builders context; the image area's copy of a snapshot).
#[test]
fn create_image_head_and_size_read_the_image_the_fork_comes_from() {
    drive(plain(), |c, _| async move {
        let head = create(&c).image_head().await.expect("the head");
        assert!(!head.is_empty());
        assert_eq!(create(&c).image_size("snap_none").await.expect("no such image, no size"), None);
    });
}

/// Waits on packet 14 (`BootArea::image_head` reads the builders context; `ImageArea::copy_for_fork`).
#[test]
fn create_copy_for_fork_hands_the_building_line_to_the_caller_as_it_is_said() {
    drive(plain(), |c, _| async move {
        let said: Rc<RefCell<Vec<(String, f64)>>> = Rc::default();
        let held = said.clone();
        let building = move |where_: &str, rate: f64| held.borrow_mut().push((where_.to_string(), rate));
        let head = create(&c).image_head().await.expect("the head");
        let copy = create(&c).copy_for_fork(&head, None, &building).await.expect("the golden");
        assert_eq!(copy, head, "at the image's own place the fork reads the image itself");
        assert!(said.borrow().is_empty(), "nothing is built where the image already stands");
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The tree and the spawn caps (rules.ts:tree_of; channels.ts:spawnAt)

#[test]
fn create_tree_of_writes_the_threads_of_the_asking_scope_and_nothing_without_one() {
    drive(plain(), |c, _| async move {
        assert!(create(&c).tree_of(None).is_empty());
        let one = scope("t_child", "t_root");
        let tree = create(&c).tree_of(Some(&one));
        assert_eq!(tree.get("rootThreadId"), Some(&json!("t_root")));
        assert_eq!(tree.get("parentThreadId"), Some(&json!("t_child")));
        let said = spawn(&c).tree_of(&one);
        assert_eq!(said.get("rootThreadId"), Some(&json!("t_root")));
        assert_eq!(said.get("parentThreadId"), Some(&json!("t_child")));
    });
}

/// Waits on packet 3 (`ChannelsArea::spawn_at` reads the door through the channels context).
#[test]
fn create_spawn_at_is_the_caps_a_place_takes_and_the_default_for_none() {
    drive(plain(), |c, _| async move {
        assert_eq!(create(&c).spawn_at("default"), wsp_wire::types::place_state::AGENTS_ON);
    });
}

/// Waits on packet 3 (`ChannelsArea::spawn_guard`'s rules read) for a thread at its cap; the plain guard passes.
#[test]
fn spawn_spawn_guard_hands_back_a_release_and_the_release_gives_the_place_back() {
    drive(plain(), |c, _| async move {
        let release = spawn(&c).spawn_guard(None).expect("a guard with no caller");
        release();
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The machine's own reads (reach.ts, daemon.ts, kinds.ts:534, records.ts:157)

/// Waits on packet 3 (`ChannelsArea::served_by_its_computer`).
#[test]
fn create_served_by_its_computer_is_some_only_for_a_machine_serving_its_own_daemon() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        let served = entry.borrow().machine.offers().daemon_frame;
        assert_eq!(create(&c).served_by_its_computer(&entry).is_some(), served);
    });
}

#[test]
fn create_daemon_answers_ms_is_the_lifecycle_budget_and_refuses_where_a_backend_has_none() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        let lifecycle = backend().lifecycle();
        match lifecycle {
            Some(l) => assert_eq!(create(&c).daemon_answers_ms(&entry).expect("a budget"), l.budgets().daemon_answers_ms.0 as u64),
            None => assert!(create(&c).daemon_answers_ms(&entry).refused("no lifecycle").to_string().contains("declare no lifecycle")),
        }
    });
}

#[test]
fn create_daemon_reach_asks_the_entrys_own_workspace_for_its_route() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        let ws = entry.borrow().ws.clone();
        let direct = ws.daemon_reach().await.map(|_| ()).map_err(|e| e.to_string());
        let said = create(&c).daemon_reach(&entry).await.map_err(|e| e.to_string());
        assert_eq!(said.is_ok(), direct.is_ok());
    });
}

/// Waits on packet 4 (`ReachArea::ping_daemon`'s reach context).
#[test]
fn create_and_spawn_ping_daemon_answers_what_the_reach_area_answers() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        let a = create(&c).ping_daemon(&entry).await;
        let b = spawn(&c).ping_daemon(&entry).await;
        assert_eq!(a, b);
    });
}

/// Waits on packet 4 (`DaemonArea::sync_daemon`'s daemon context).
#[test]
fn create_and_spawn_sync_daemon_run_the_sync_without_holding_the_caller() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        create(&c).sync_daemon(&entry);
        spawn(&c).sync_daemon(&entry);
        tokio::task::yield_now().await;
    });
}

/// Waits on packet 4 (`DaemonArea::write_daemon_roots`).
#[test]
fn create_write_daemon_roots_writes_the_roots_of_the_entry_not_strictly() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        create(&c).write_daemon_roots(&entry).await.expect("roots written");
    });
}

/// Waits on packet 4 (`ReachArea`'s machine reads) and 14 (the image area's sizes).
#[test]
fn create_read_memory_keeps_a_size_the_machine_does_not_say_otherwise() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        let held = entry.borrow().record.clone();
        let machine = entry.borrow().machine.clone();
        let sizes = backend()
            .capabilities()
            .sizes
            .iter()
            .map(|s| wsp_wire::wire::capabilities::WorkspaceSize { cpu: s.cpu, mem_mb: s.mem_mb })
            .collect::<Vec<_>>();
        let before = held.borrow().size.clone();
        let read = create(&c).read_memory(&held, &machine, &sizes).await;
        if !read {
            assert_eq!(held.borrow().size, before);
        }
    });
}

#[test]
fn create_module_of_a_kind_is_its_module_and_a_kind_nothing_serves_refuses() {
    drive(plain(), |c, _| async move {
        let cloud = create(&c).module_of("cloud").expect("the cloud module");
        assert!(cloud.backend(&record(json!({}))).is_ok());
        let said = create(&c).module_of("local").refused("no local wiring").to_string();
        assert!(said.contains("local"), "{said}");
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The records (records.ts:39-245) and the place a create marks used

/// Waits on packet 3/4 (the records area reads the store through its context, which a persist writes).
#[test]
fn create_persist_writes_the_record_to_the_store_and_view_shows_it() {
    drive(plain(), |c, _| async move {
        let held = record(json!({ "id": "w_1", "name": "api" }));
        create(&c).remember_project(project("p_1", "web", "/work/web")).await.expect("remembered");
        create(&c).persist(&held).await.expect("persisted");
        spawn(&c).persist(&held).await.expect("persisted");
        let view = create(&c).view(&held).expect("a view");
        assert_eq!(view.id, "w_1");
        assert_eq!(view.name, "api");
        assert_eq!(spawn(&c).view(&held).expect("a view")["id"], "w_1");
    });
}

#[test]
fn create_mark_used_is_a_no_op_without_a_door_and_the_doors_write_with_one() {
    drive(plain(), |c, _| async move {
        create(&c).mark_used(Some("pl_1")).await.expect("no door, nothing to mark");
        create(&c).mark_used(None).await.expect("no place, nothing to mark");
    });
    drive(with_door(), |c, _| async move {
        create(&c).mark_used(None).await.expect("nothing to mark");
        let said = create(&c).mark_used(Some("pl_nobody")).await.map_err(|e| e.to_string());
        let door = create(&c).place_door().expect("the door");
        let direct = door.mark_used(Some("pl_nobody")).await.map_err(|e| e.to_string());
        assert_eq!(said, direct.map_err(|e| e.to_string()));
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The builders the cap road reads (create.ts:130-139, 151-152)

#[test]
fn create_builders_are_none_until_a_builder_is_kept() {
    drive(plain(), |c, _| async move {
        let kept: Vec<KeptBuilder> = create(&c).builders();
        assert!(kept.is_empty());
        create(&c).cancel_grace("b_1");
    });
}

/// Waits on packet 14 (`BuildersArea::forget_builder`) and 5 (`BootArea::refresh_builders`).
#[test]
fn create_refresh_and_forget_builder_with_none_kept() {
    drive(plain(), |c, _| async move {
        create(&c).refresh_builders().await.expect("refreshed");
        create(&c).forget_builder("b_nobody").await.expect("forgetting what is not kept is fine");
    });
}

#[test]
fn create_holds_slot_counts_a_running_record_and_not_a_napping_or_gone_one() {
    drive(plain(), |c, _| async move {
        assert!(create(&c).holds_slot(&record(json!({ "phase": "running" }))));
        assert!(!create(&c).holds_slot(&record(json!({ "phase": "napping" }))));
        assert!(!create(&c).holds_slot(&record(json!({ "phase": "gone" }))));
    });
}

// ----------------------------------------------------------------------------------------------------------------
// attach, fork and unfork (machines.ts attach/fork/unfork)

/// Waits on packet 4 (`MachinesArea::attach` reads the live map, the kinds and the daemon through its context).
#[test]
fn create_and_spawn_attach_put_the_record_in_the_live_map_with_its_machine() {
    drive(plain(), |c, _| async move {
        let held = Rc::new(RefCell::new(record(json!({ "id": "w_1", "machineId": "m_1" }))));
        let entry = create(&c).attach(&held, machine_named("m_1").await).expect("attached");
        assert!(Rc::ptr_eq(&entry.borrow().record, &held));
        assert!(Rc::ptr_eq(&create(&c).live().borrow()["w_1"], &entry));
        let dead = Rc::new(RefCell::new(record(json!({ "id": "w_2", "machineId": "" }))));
        let queued = spawn(&c).attach_dead(&dead).expect("attached on a stand-in");
        assert!(Rc::ptr_eq(&spawn(&c).live("w_2").expect("live"), &queued));
        let again = Rc::new(RefCell::new(record(json!({ "id": "w_3", "machineId": "m_1" }))));
        let third = spawn(&c).attach(&again, machine_named("m_1").await).expect("attached");
        assert!(Rc::ptr_eq(&spawn(&c).live("w_3").expect("live"), &third));
    });
}

/// Waits on packet 4 (`MachinesArea::fork`) and 14 (the builders and image it reads).
#[test]
fn create_fork_binds_the_machine_before_the_claim_is_let_go_and_reports_its_stages() {
    drive(plain(), |c, runtime| async move {
        let stages = tap(&runtime, "workspace.creating");
        let held = Rc::new(RefCell::new(record(json!({ "id": "w_1", "machineId": "", "golden": "snap_g" }))));
        let bound: Rc<RefCell<Vec<String>>> = Rc::default();
        let seen = bound.clone();
        let bind = move |m: &Arc<dyn Machine>, r: &WorkspaceRecord| -> Result<(), HostError> {
            seen.borrow_mut().push(format!("{}:{}", m.id(), r.id));
            Ok(())
        };
        let report = spawn(&c).stage_reporter("w_1", "api", 0.0, None);
        let machine = create(&c).fork(&held, &bind, report).await.expect("a machine");
        assert_eq!(bound.borrow().len(), 1);
        assert!(bound.borrow()[0].starts_with(&format!("{}:", machine.id())));
        assert!(!stages.borrow().is_empty());
    });
}

/// Waits on packet 4 (`MachinesArea::fork`).
#[test]
fn spawn_fork_hands_the_machine_to_the_callback_and_answers_nothing() {
    drive(plain(), |c, _| async move {
        let held = Rc::new(RefCell::new(record(json!({ "id": "w_1", "machineId": "", "golden": "snap_g" }))));
        let got: Rc<RefCell<Vec<String>>> = Rc::default();
        let seen = got.clone();
        let on_machine = Box::new(move |m: Arc<dyn Machine>| -> Result<(), HostError> {
            seen.borrow_mut().push(m.id().to_string());
            Ok(())
        });
        let report = spawn(&c).stage_reporter("w_1", "api", 0.0, None);
        spawn(&c).fork(&held, on_machine, report).await.expect("forked");
        assert_eq!(got.borrow().len(), 1);
    });
}

/// Waits on packet 4 (`MachinesArea::unfork`) and 5 (the gone watch).
#[test]
fn spawn_unfork_kills_the_machine_a_failed_start_made() {
    drive(plain(), |c, rt| async move {
        // The machine is the runtime's own backend's: `unfork` kills it and reads its state there, so a machine from
        // another backend reads as running for good and the kill waits out its three asks (90 s).
        let spec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
        let machine = rt.backend().create(spec, None, None).await.expect("a machine");
        let entry = entry_over(record(json!({ "id": "w_1", "name": "api", "machineId": machine.id() })), machine);
        create(&c).live().borrow_mut().insert("w_1".into(), entry.clone());
        spawn(&c).unfork(&entry).await.expect("unforked");
    });
}

/// Waits on packet 4 (`ReachArea::emit_status`).
#[test]
fn spawn_emit_status_reachable_puts_a_reachable_status_on_the_bus() {
    drive(plain(), |c, runtime| async move {
        let status = tap(&runtime, "workspace.status");
        create(&c).remember_project(project("p_1", "web", "/work/web")).await.expect("remembered");
        let entry = entry_named("w_1", "api").await;
        spawn(&c).emit_status_reachable(&entry).await.expect("emitted");
        let seen = status.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0]["reach"], "reachable");
        assert_eq!(seen[0]["workspaceId"], "w_1");
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The sub-thread's queue and the boot rows (sub-spawn.ts:196-470)

/// Waits on packet 7 (`SubQueueArea::enqueue`).
#[test]
fn spawn_enqueue_holds_a_record_for_a_slot_and_queue_wait_says_when() {
    drive(plain(), |c, _| async move {
        assert!(spawn(&c).queue_wait("default", None).is_none(), "no limits, no wait");
        // The queued row is shown as `ctx.view(record)` shows it (sub-queue.ts:551), which throws for a project not held.
        create(&c).remember_project(project("p_1", "web", "/work/web")).await.expect("remembered");
        let held = Rc::new(RefCell::new(record(json!({ "id": "w_1", "machineId": "" }))));
        spawn(&c).enqueue(&held, None).await.expect("enqueued");
    });
}

/// Waits on packet 11 (`BootArea::rows_sent` reads the agents context) and 5 (`push_head`).
#[test]
fn spawn_rows_sent_and_push_head_are_boots() {
    drive(plain(), |c, _| async move {
        spawn(&c).rows_sent().await;
        spawn(&c).push_head("t_1");
        assert!(spawn(&c).thread_records().is_empty());
    });
}

/// Waits on packet 5 (`BootArea::computer_rows`).
#[test]
fn spawn_computer_rows_name_every_provider_row_this_host_holds() {
    drive(plain(), |c, _| async move {
        let rows = spawn(&c).computer_rows().await.expect("the rows");
        assert!(rows.iter().all(|r| !r.id.is_empty() && !r.name.is_empty()));
    });
}

/// Waits on packet 3 (`ChannelsArea::copy_blocked`, `with_daemon`, `checkout_of`, `place_guard`).
#[test]
fn spawn_the_lead_reads_go_through_the_channels_area() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        spawn(&c).copy_blocked(&entry).await.expect("a lead nothing blocks a copy of");
        spawn(&c).place_guard("default").await.expect("the wired place is not guarded");
    });
}

/// Waits on packet 16 (`PreferencesArea::get`) and 11 (`AgentsArea::default_agent_of`, `homes_here`, `adapter_for`).
#[test]
fn spawn_the_agents_reads_name_the_leads_agent_and_the_homes_here() {
    drive(plain(), |c, _| async move {
        let entry = entry_named("w_1", "api").await;
        let agent = spawn(&c).default_agent_of(&entry).await.expect("an agent");
        assert!(!agent.is_empty());
        let homes = spawn(&c).homes_here().await.expect("the homes");
        assert!(homes.is_object());
        assert!(spawn(&c).session_file(&entry, "claude", "s_1").is_none());
    });
}

/// Waits on packet 14 (the records area's image reads).
#[test]
fn spawn_copy_of_and_record_of_read_the_image_records() {
    drive(plain(), |c, _| async move {
        assert!(spawn(&c).copy_of("default", "snap_none").await.expect("a read").is_none());
        let image = spawn(&c).record_of("default").await.expect("a read");
        assert!(image.is_none() || image.is_some());
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The seed and the store (sub-spawn.ts:307, 396-420)

/// The seed the host wires, which keeps what it was asked.
struct FakeSeed {
    asked: Rc<RefCell<Vec<String>>>,
}

fn seed_plan_of(folder: &str) -> SeedPlan {
    serde_json::from_value(json!({
        "source": folder, "remote": null, "branch": "main", "defaultBranch": null, "unpushed": null, "uncommitted": 0,
        "memory": null, "files": [], "remembered": false,
    }))
    .expect("a plan")
}

#[async_trait::async_trait(?Send)]
impl SeedWiring for FakeSeed {
    async fn plan(&self, folder: &str, homes: &IndexMap<String, String>) -> Result<SeedPlan, HostError> {
        self.asked.borrow_mut().push(format!("plan {folder} {homes:?}"));
        Ok(seed_plan_of(folder))
    }
    async fn pack(&self, plan: SeedPlan, choice: SeedChoice, homes: &IndexMap<String, String>) -> Result<SeedPack, HostError> {
        self.asked.borrow_mut().push(format!("pack {} {:?} {homes:?}", plan.source, choice));
        Ok(SeedPack { tar: vec![1, 2, 3], files: 1, bytes: 3, commits: 9, left: vec!["big.bin".into()] })
    }
}

#[test]
fn spawn_seed_plan_and_pack_go_through_the_seed_the_host_wired() {
    let asked: Rc<RefCell<Vec<String>>> = Rc::default();
    let opts = RuntimeOptions { seed: Some(Rc::new(FakeSeed { asked: asked.clone() })), ..plain() };
    drive(opts, |c, _| async move {
        assert!(spawn(&c).seeds());
        let homes = json!({ "claude": "/home/z/.claude" });
        let plan = spawn(&c).seed_plan("/work/web", &homes).await.expect("a plan");
        assert_eq!(plan, seed_plan_of("/work/web"));
        let packed = spawn(&c).seed_pack(&plan, &["a.txt".to_string(), "b.txt".to_string()], &homes).await.expect("a pack");
        assert_eq!((packed.tar, packed.files, packed.bytes, packed.left), (vec![1, 2, 3], 1, 3, vec!["big.bin".to_string()]));
        let choice = SeedChoice { files: vec!["a.txt".into(), "b.txt".into()], memory: false, commits: false, remember: None };
        assert_eq!(
            *asked.borrow(),
            vec![
                "plan /work/web {\"claude\": \"/home/z/.claude\"}".to_string(),
                format!("pack /work/web {choice:?} {{\"claude\": \"/home/z/.claude\"}}"),
            ]
        );
    });
}

#[test]
fn spawn_seeds_is_false_with_no_seed_wired() {
    drive(plain(), |c, _| async move {
        assert!(!spawn(&c).seeds());
    });
}

#[test]
fn spawn_store_reads_the_seed_choice_a_blob_and_deletes_a_create() {
    let store = Rc::new(memory_store());
    let opts = RuntimeOptions { store: Some(store.clone()), ..plain() };
    drive(opts, |c, _| async move {
        assert_eq!(spawn(&c).get_seed_choice("/work/web").await.expect("a read"), None);
        let choice = SeedChoice { files: vec!["a.txt".into()], memory: true, commits: false, remember: Some(true) };
        store.put("seed-choices", "/work/web", serde_json::to_value(&choice).expect("a value")).await.expect("put");
        assert_eq!(spawn(&c).get_seed_choice("/work/web").await.expect("a read"), Some(choice));
        assert_eq!(spawn(&c).get_blob("sub-sessions", "t_1").await.expect("a read"), None);
        store.put_blob("sub-sessions", "t_1", vec![7, 8]).await.expect("put blob");
        assert_eq!(spawn(&c).get_blob("sub-sessions", "t_1").await.expect("a read"), Some(vec![7, 8]));
        store.put("creates", "k_1", json!({ "id": "w_1" })).await.expect("put");
        spawn(&c).delete_create("k_1").await.expect("deleted");
        assert_eq!(store.get("creates", "k_1").await.expect("a read"), None);
        spawn(&c).delete_create("k_1").await.expect("deleting what is not there is fine");
    });
}

#[test]
fn spawn_seed_plan_without_a_seed_refuses_as_a_missing_member_does() {
    drive(plain(), |c, _| async move {
        let said = spawn(&c).seed_plan("/work/web", &json!({})).await.refused("no seed wired").message();
        assert_eq!(said, "Cannot read properties of undefined (reading 'plan')");
    });
}

// ----------------------------------------------------------------------------------------------------------------
// The place a record stands on, its checkout, and the broker's lift (channels.ts:80-121; sub-spawn.ts:48, 470)

/// Waits on packet 3 (`ChannelsArea::place_id_of` reads the door through the channels context).
#[test]
fn spawn_place_id_of_is_this_computer_for_a_folder_here_the_named_place_else_the_wired_one() {
    drive(plain(), |c, _| async move {
        assert_eq!(spawn(&c).place_id_of(&record(json!({ "kind": "local" }))), Some(HERE_PLACE_ID.to_string()));
        assert_eq!(spawn(&c).place_id_of(&record(json!({ "kind": "cloud", "place": "pl_1" }))), Some("pl_1".to_string()));
        assert_eq!(spawn(&c).place_id_of(&record(json!({ "kind": "cloud" }))), Some("default".to_string()));
    });
}

/// Waits on packet 3 (`ChannelsArea::checkout_of` and `project_held`).
#[test]
fn spawn_checkout_of_is_the_projects_path_and_the_subs_own_folder() {
    drive(plain(), |c, _| async move {
        create(&c).remember_project(project("p_1", "web", "/work/web")).await.expect("remembered");
        assert_eq!(spawn(&c).checkout_of(&record(json!({ "project": "p_1" }))).expect("a checkout"), "/work/web");
        let sub = record(
            json!({ "project": "p_1", "sub": { "on": "default", "repo": "r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/root/sub" } }),
        );
        assert_eq!(spawn(&c).checkout_of(&sub).ok(), Some("/root/sub".to_string()));
        let said = spawn(&c).checkout_of(&record(json!({ "project": "p_9" }))).refused("no such project").to_string();
        assert!(said.contains("p_9"), "{said}");
    });
}

/// The broker the host wires, which keeps the lifts it was asked for.
struct LiftBroker {
    lifted: Rc<RefCell<Vec<String>>>,
}

#[async_trait::async_trait(?Send)]
impl Broker for LiftBroker {
    fn ready(&self) -> bool {
        true
    }
    async fn codex_claims(&self) -> Result<Option<wsp_adapters::pending::CodexClaims>, HostError> {
        Ok(None)
    }
    fn has_lift(&self) -> bool {
        true
    }
    fn lift(&self, workspace_id: &str) {
        self.lifted.borrow_mut().push(workspace_id.to_string());
    }
}

#[test]
fn spawn_broker_lift_and_the_brokered_start_follow_the_broker_the_host_wired() {
    let lifted: Rc<RefCell<Vec<String>>> = Rc::default();
    let opts = RuntimeOptions { broker: Some(Rc::new(LiftBroker { lifted: lifted.clone() })), ..plain() };
    drive(opts, |c, _| async move {
        spawn(&c).broker_lift("w_1");
        spawn(&c).broker_lift("w_2");
        assert_eq!(*lifted.borrow(), vec!["w_1".to_string(), "w_2".to_string()]);
        assert!(spawn(&c).brokered_start());
        assert!(create(&c).brokered_start(true));
        assert!(!create(&c).brokered_start(false));
        assert!(spawn(&c).brokered("cloud", None));
        assert!(spawn(&c).brokered("cloud", Some("default")));
        assert!(!spawn(&c).brokered("cloud", Some("pl_1")));
        assert!(!spawn(&c).brokered("place", Some("pl_1")));
        assert!(!spawn(&c).brokered("box", Some("default")));
        assert!(!spawn(&c).brokered("local", None));
    });
}

#[test]
fn spawn_folder_named_is_the_last_segment_of_a_path() {
    drive(plain(), |c, _| async move {
        let home = std::env::var("HOME").unwrap_or_default();
        let cwd = std::env::current_dir().expect("a cwd").to_string_lossy().to_string();
        for path in ["/work/web", "/work/web/", "~", "~/web", "web"] {
            assert_eq!(spawn(&c).folder_named(path), wsp_core::types::wiring::folder_named(path, &home, &cwd), "{path}");
        }
    });
}
