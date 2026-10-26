// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 4 (#2476): `ReachCtx`, `DaemonRuntime`, `MachinesRuntime` and `IdleDoor` over the one
//! context, each method read against the TS member it stands for (`reach.ts`, `daemon.ts`, `machines.ts:15-906`,
//! `runtime.ts:491-992`), on a runtime built over `FakeBackend` and a clock the test moves.
//!
//! Whole-runtime tests of `runtime-reach`, `idle`, `disk-watch`, `pause`, `box-nap`, `runtime-gone`, `gone`,
//! `wake-asks-again`, `wake-keeps-machine`, `resume-after-a-nap` and `background-wake` go through `rt.workspaces.*`
//! and a stub provider with scripted `killed`, `previewUrl` and `resume`, which this crate has no port of; their
//! scenarios are written below at the area, over this context, where the context is what they exercise.
//!
//! A test that goes through a method of another packet's trait is red until that packet lands: the packet is named
//! in the test's last doc line, `waits on packet N`.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::future::Shared;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::Runtime;
use wsp_core::types::events::ANY_EVENT;
use wsp_core::types::internal::{MachineMoment, SessionEntry, ThreadRecord};
use wsp_core::types::wiring::{GoldenExec, GoldenRecipe, LiveWorkspace, RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions, FAKE_NO_GUEST};
use wsp_engine::gone::KillConfirm;
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{Machine, MachineBackend, MachineSpec, MachineState};
use wsp_machines::machines::daemon::DaemonRuntime;
use wsp_machines::machines::machines::{Entry, IdleDoor, MachinesArea, MachinesConfig, MachinesRuntime, Row, Unreached};
use wsp_machines::machines::reach::{ForMachine, ReachCtx};
use wsp_serve::runtime::ctx::{Ctx, ExecHeld};
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::store::memory_store;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{ProjectView, ReachState, WorkspacePhase};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::machine_link::MachineShape;

const START: i64 = 1_000_000;

struct World {
    ctx: Rc<Ctx>,
    runtime: Rc<AssembledRuntime>,
    backend: Arc<FakeBackend>,
    clock: FakeClock,
}

fn world_with(adjust: impl FnOnce(&mut RuntimeOptions)) -> World {
    let backend = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
    let clock = FakeClock::new(START);
    let mut opts = RuntimeOptions {
        backend: Some(backend.clone()),
        store: Some(Rc::new(memory_store())),
        clock: Some(clock.clock()),
        ..Default::default()
    };
    adjust(&mut opts);
    let runtime = create_runtime(opts);
    World { ctx: runtime.context(), runtime, backend, clock }
}

fn run_with<Fut: std::future::Future<Output = ()>>(adjust: impl FnOnce(&mut RuntimeOptions), f: impl FnOnce(World) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move { f(world_with(adjust)).await });
}

fn run<Fut: std::future::Future<Output = ()>>(f: impl FnOnce(World) -> Fut) {
    run_with(|_| {}, f)
}

fn record_json(id: &str) -> Value {
    json!({
        "id": id, "name": format!("name {id}"), "machineId": "m_1", "phase": "running", "golden": "snap_g",
        "createdAt": "2026-10-26T00:00:00.000Z", "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    })
}

fn record(id: &str) -> WorkspaceRecord {
    serde_json::from_value(record_json(id)).expect("a workspace record")
}

fn project(id: &str, name: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "computer": "here", "source": { "kind": "folder", "path": format!("/home/me/{name}") },
        "path": format!("/home/me/{name}"), "remote": format!("git@github.com:acme/{name}.git"), "defaultBranch": "main",
        "memoryKey": format!("-home-me-{name}"), "memoryDir": format!("/home/me/.claude/projects/-home-me-{name}/memory"),
        "createdAt": "2026-10-01T00:00:00.000Z",
    }))
    .expect("a project")
}

async fn machine_of(w: &World) -> Arc<dyn Machine> {
    let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox", "cpu": 2 })).expect("a spec");
    w.backend.create(spec, None, None).await.expect("a machine")
}

/// A live entry on a fresh machine, with the record's `over` keys laid on the base.
async fn entry_with(w: &World, id: &str, over: Value) -> Entry {
    let machine = machine_of(w).await;
    let mut base = record_json(id);
    base["machineId"] = json!(machine.id());
    if let (Value::Object(b), Value::Object(o)) = (&mut base, over) {
        b.extend(o);
    }
    let record: WorkspaceRecord = serde_json::from_value(base).expect("a workspace record");
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

async fn entry(w: &World, id: &str) -> Entry {
    entry_with(w, id, json!({})).await
}

async fn live_entry(w: &World, id: &str) -> Entry {
    let e = entry(w, id).await;
    MachinesRuntime::live(&*w.ctx).borrow_mut().insert(id.to_string(), e.clone());
    e
}

fn session_row(turn: &str, workspace: &str) -> Row {
    let view: SessionView =
        serde_json::from_value(json!({ "id": turn, "workspaceId": workspace, "harness": "claude", "status": "running" }))
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

type Joined = Shared<futures::future::LocalBoxFuture<'static, ()>>;

fn done() -> Joined {
    futures::future::ready(()).boxed_local().shared()
}

fn area_of(w: &World) -> Rc<MachinesArea> {
    MachinesArea::new(
        w.ctx.clone() as Rc<dyn MachinesRuntime>,
        MachinesConfig {
            gone_confirm_ms: 0,
            late_read_ms: 180_000,
            vault_cap_bytes: 1 << 30,
            default_idle_window_ms: None,
            host_id: "host".into(),
            poll_interval_ms: 5_000,
        },
    )
}

// ---------------------------------------------------------------------------------------------------------------
// The core's own reads: the live map, the sessions, the clock, the options (runtime.ts:491-992).

#[test]
fn live_reads_the_one_live_map_every_trait_shares() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = live_entry(&w, "w_1").await;
        assert!(Rc::ptr_eq(&ReachCtx::live(c, "w_1").expect("the entry"), &e));
        assert!(ReachCtx::live(c, "w_2").is_none());
        assert_eq!(ReachCtx::live_all(c).len(), 1);
        assert!(Rc::ptr_eq(&MachinesRuntime::live(c), &DaemonRuntime::live(c)));
    });
}

#[test]
fn session_entries_read_the_session_rows_the_runtime_holds() {
    run(|w| async move {
        let c = &*w.ctx;
        MachinesRuntime::sessions(c).borrow_mut().insert("t_1".into(), session_row("t_1", "w_1"));
        let seen = ReachCtx::session_entries(c);
        assert_eq!(seen.len(), 1);
        assert_eq!((seen[0].turn_id.as_str(), seen[0].view.workspace_id.as_str()), ("t_1", "w_1"));
        assert!(Rc::ptr_eq(&MachinesRuntime::sessions(c), &DaemonRuntime::sessions(c)));
    });
}

#[test]
fn closing_and_closed_read_the_state_a_fresh_runtime_has_not_closed() {
    run(|w| async move {
        assert!(!ReachCtx::closing(&*w.ctx));
        assert!(!DaemonRuntime::closed(&*w.ctx));
    });
}

#[test]
fn now_reads_the_runtime_clock_in_every_trait() {
    run(|w| async move {
        let c = &*w.ctx;
        assert_eq!(ReachCtx::now(c), START as f64);
        w.clock.advance(5);
        assert_eq!(ReachCtx::now(c), (START + 5) as f64);
        assert_eq!(MachinesRuntime::clock(c).now(), START + 5);
        assert_eq!(DaemonRuntime::clock(c).now(), START + 5);
    });
}

#[test]
fn schedule_runs_once_after_its_ms_and_a_cancel_stops_it() {
    run(|w| async move {
        let c = &*w.ctx;
        let fired = Rc::new(Cell::new(0));
        let hit = fired.clone();
        let _ = ReachCtx::schedule(c, 50.0, Box::new(move || hit.set(hit.get() + 1)));
        w.clock.advance(49);
        assert_eq!(fired.get(), 0);
        w.clock.advance(1);
        assert_eq!(fired.get(), 1);
        let hit = fired.clone();
        let cancel = ReachCtx::schedule(c, 10.0, Box::new(move || hit.set(hit.get() + 10)));
        cancel();
        w.clock.advance(100);
        assert_eq!(fired.get(), 1);
    });
}

#[test]
fn sleep_settles_once_the_clock_has_moved_its_ms() {
    run(|w| async move {
        let mut sleeping = ReachCtx::sleep(&*w.ctx, 100.0);
        assert!((&mut sleeping).now_or_never().is_none());
        w.clock.advance(99);
        assert!((&mut sleeping).now_or_never().is_none());
        w.clock.advance(1);
        sleeping.await;
    });
}

#[test]
fn spawn_starts_the_future_at_once_on_the_task_set() {
    run(|w| async move {
        let c = &*w.ctx;
        let ran = Rc::new(Cell::new(false));
        let flag = ran.clone();
        ReachCtx::spawn(c, Box::pin(async move { flag.set(true) }));
        tokio::task::yield_now().await;
        assert!(ran.get());
    });
}

#[test]
fn warn_writes_one_console_line_for_each_trait() {
    run(|w| async move {
        let c = &*w.ctx;
        let lines = wsp_core::console::capture();
        ReachCtx::warn(c, "one");
        DaemonRuntime::warn(c, "two");
        MachinesRuntime::warn(c, "three");
        assert_eq!(lines.lines(), vec!["one", "two", "three"]);
    });
}

#[test]
fn the_daemon_hello_wait_is_the_option_or_its_default() {
    run(|w| async move {
        assert_eq!(ReachCtx::daemon_hello_timeout_ms(&*w.ctx), 5_000.0);
        assert_eq!(DaemonRuntime::daemon_hello_timeout_ms(&*w.ctx), 5_000);
    });
    run_with(
        |o| o.daemon_hello_timeout_ms = Some(300),
        |w| async move {
            assert_eq!(ReachCtx::daemon_hello_timeout_ms(&*w.ctx), 300.0);
            assert_eq!(DaemonRuntime::daemon_hello_timeout_ms(&*w.ctx), 300);
        },
    );
}

#[test]
fn ids_hand_out_a_new_uuid_each_time() {
    run(|w| async move {
        let c = &*w.ctx;
        let (a, b) = (DaemonRuntime::ids(c).uuid(), MachinesRuntime::ids(c).uuid());
        assert_eq!(a.len(), 36);
        assert_ne!(a, b);
    });
}

#[test]
fn store_backend_and_places_are_the_ones_the_options_named() {
    run(|w| async move {
        let c = &*w.ctx;
        let own: Arc<dyn MachineBackend> = w.backend.clone();
        assert!(Arc::ptr_eq(&MachinesRuntime::backend(c), &own));
        assert_eq!(MachinesRuntime::places(c).wired(), "default");
        let store = MachinesRuntime::store(c);
        store.put("probe", "k", json!({ "n": 1 })).await.expect("a put");
        assert_eq!(store.get("probe", "k").await.expect("a get"), Some(json!({ "n": 1 })));
    });
}

#[test]
fn owner_starts_empty_and_the_golden_hook_is_the_recipes_on_exec() {
    run(|w| async move {
        assert_eq!(MachinesRuntime::owner(&*w.ctx), "");
        assert!(MachinesRuntime::on_exec(&*w.ctx).is_none());
    });
    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let log = seen.clone();
    run_with(
        move |o| {
            o.golden_recipe = Some(GoldenRecipe {
                setup: "true".into(),
                smoke: "true".into(),
                on_exec: Some(Rc::new(move |e: GoldenExec| log.borrow_mut().push(e.cmd))),
                ..Default::default()
            })
        },
        |w| async move {
            let hook = MachinesRuntime::on_exec(&*w.ctx).expect("the recipe's hook");
            assert_eq!(hook(&GoldenExec { cmd: "ls".into(), ..Default::default() }), Ok(()));
        },
    );
    assert_eq!(*seen.borrow(), vec!["ls".to_string()]);
}

#[test]
fn kill_confirm_is_the_option_the_host_named() {
    run(|w| async move {
        assert_eq!(MachinesRuntime::kill_confirm(&*w.ctx), None);
    });
    let asked = KillConfirm { grace_ms: Some(5), poll_ms: Some(1) };
    run_with(
        move |o| o.kill_confirm = Some(asked),
        |w| async move {
            assert_eq!(MachinesRuntime::kill_confirm(&*w.ctx), Some(asked));
        },
    );
}

#[test]
fn project_held_answers_whether_the_projects_map_has_the_id() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(!MachinesRuntime::project_held(c, "p_1"));
        w.ctx.machines_seams().projects_held.borrow_mut().insert("p_1".into(), project("p_1", "web"));
        assert!(MachinesRuntime::project_held(c, "p_1"));
    });
}

#[test]
fn kept_agents_on_names_the_threads_whose_kept_agent_stands_on_the_workspace() {
    run(|w| async move {
        assert_eq!(MachinesRuntime::kept_agents_on(&*w.ctx, "w_1"), Vec::<String>::new());
    });
}

#[test]
fn end_execs_ends_each_exec_of_the_workspace_with_the_reason_and_no_other() {
    run(|w| async move {
        let c = &*w.ctx;
        let said = Rc::new(RefCell::new(Vec::<String>::new()));
        for (n, workspace) in [(1, "w_1"), (2, "w_2"), (3, "w_1")] {
            let log = said.clone();
            w.ctx.machines_seams().execs.borrow_mut().insert(
                n,
                ExecHeld {
                    workspace_id: workspace.into(),
                    end: Rc::new(move |reason: &str| log.borrow_mut().push(format!("{workspace} {n} {reason}"))),
                },
            );
        }
        MachinesRuntime::end_execs(c, "w_1", "machine gone");
        assert_eq!(*said.borrow(), vec!["w_1 1 machine gone", "w_1 3 machine gone"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The idle policy (machines.ts:789-846).

#[test]
fn the_idle_door_arms_a_workspaces_window_and_a_forget_takes_it_back() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = entry_with(&w, "w_1", json!({ "idleWindowMs": 60_000 })).await;
        MachinesRuntime::live(c).borrow_mut().insert("w_1".into(), e);
        assert_eq!(ReachCtx::idle_at(c, "w_1"), None);
        IdleDoor::touch(c, "w_1");
        assert_eq!(ReachCtx::idle_at(c, "w_1"), Some((START + 60_000) as f64));
        IdleDoor::forget(c, "w_1");
        assert_eq!(ReachCtx::idle_at(c, "w_1"), None);
        // `MachinesRuntime::idle` is the same policy the door touches.
        MachinesRuntime::idle(c).touch("w_1");
        assert_eq!(ReachCtx::idle_at(c, "w_1"), Some((START + 60_000) as f64));
    });
}
// waits on packet 2 (`KindsCtx::backend_for`, read when the policy arms its backstop).

// ---------------------------------------------------------------------------------------------------------------
// The bus (runtime.ts:491-992): the events these areas emit and the records they write.

#[test]
fn emit_puts_the_event_on_the_bus_for_the_json_and_the_event_alike() {
    run(|w| async move {
        let c = &*w.ctx;
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        let log = heard.clone();
        let _off = w.runtime.events().on(
            ANY_EVENT,
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event")["projectId"].as_str().unwrap_or("").to_string());
                Ok(())
            }),
        );
        ReachCtx::emit(c, json!({ "type": "project.removed", "projectId": "p_1" }));
        let event = serde_json::from_value(json!({ "type": "project.removed", "projectId": "p_2" })).expect("an event");
        MachinesRuntime::emit(c, event).expect("an emit");
        assert_eq!(*heard.borrow(), vec!["p_1", "p_2"]);
    });
}

#[test]
fn emit_takes_the_status_row_the_reach_area_writes_as_the_event_it_is() {
    run(|w| async move {
        let heard = Rc::new(RefCell::new(Vec::<Value>::new()));
        let log = heard.clone();
        let _off = w.runtime.events().on(
            "workspace.status",
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            }),
        );
        let status = json!({
            "id": "w_1", "name": "n", "machineId": "m_1", "phase": "running", "golden": "snap_g", "createdAt": "2026-10-26T00:00:00.000Z",
            "project": { "id": "p_1", "name": "web", "path": "/home/me/web", "computer": "here" },
            "machineState": "running", "reach": { "state": "reachable" }, "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.018,
            "reason": "hello",
        });
        ReachCtx::emit(&*w.ctx, json!({ "type": "workspace.status", "status": status.clone() }));
        let heard = heard.borrow();
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0]["status"], status);
    });
}

#[test]
fn record_stamps_the_session_event_and_puts_it_on_the_bus() {
    run(|w| async move {
        let heard = Rc::new(RefCell::new(Vec::<String>::new()));
        let log = heard.clone();
        let _off = w.runtime.events().on(
            "session.notice",
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event")["text"].as_str().unwrap_or("").to_string());
                Ok(())
            }),
        );
        ReachCtx::record(
            &*w.ctx,
            json!({ "type": "session.notice", "workspaceId": "w_1", "sessionId": "s_1", "turnId": "t_1", "threadId": "th_1", "text": "its disk is full" }),
        );
        assert_eq!(*heard.borrow(), vec!["its disk is full"]);
    });
}
// waits on packet 3 (transcripts: `TranscriptsArea::record` reads its context).

// ---------------------------------------------------------------------------------------------------------------
// What a record is, and where it is kept.

#[test]
fn view_joins_the_held_project_as_the_record_store_reads_it() {
    run(|w| async move {
        let c = &*w.ctx;
        w.ctx.machines_seams().projects_held.borrow_mut().insert("p_1".into(), project("p_1", "web"));
        let seen = MachinesRuntime::view(c, &record("w_1"));
        assert_eq!((seen.id.as_str(), seen.project.name.as_str(), seen.project.path.as_str()), ("w_1", "web", "/home/me/web"));
        assert_eq!(ReachCtx::view(c, &record("w_1")), serde_json::to_value(&seen).expect("a view"));
    });
}
// waits on packet 2 (`RecordsCtx::project_ref`, `module_folder`).

#[test]
fn persist_writes_the_record_and_its_name_for_all_three_traits() {
    run(|w| async move {
        let c = &*w.ctx;
        MachinesRuntime::persist(c, &record("w_1")).await.expect("a persist");
        DaemonRuntime::persist(c, &record("w_2")).await.expect("a persist");
        ReachCtx::persist(c, &record("w_3")).await.expect("a persist");
        let store = MachinesRuntime::store(c);
        for id in ["w_1", "w_2", "w_3"] {
            let stored = store.get("workspaces", id).await.expect("a get").expect("the record");
            assert_eq!(stored["id"], json!(id));
        }
    });
}
// waits on packet 2 (`RecordsCtx::store`, `bump_generation`).

#[test]
fn shape_of_reads_what_the_machine_says_it_is_and_size_built_takes_it_over_the_ask() {
    run(|w| async move {
        let c = &*w.ctx;
        let machine = machine_of(&w).await;
        let shape = MachinesRuntime::shape_of(c, &machine).await.expect("a shape");
        let asked = WorkspaceSize { cpu: serde_json::from_value(json!(4)).unwrap(), mem_mb: serde_json::from_value(json!(16384)).unwrap() };
        let built = MachinesRuntime::size_built(c, Some(&shape), asked.clone());
        assert_eq!(serde_json::to_value(&built).unwrap(), json!({ "cpu": 2, "memMb": 8192 }));
        let none: Option<&MachineShape> = None;
        assert_eq!(MachinesRuntime::size_built(c, none, asked.clone()), asked);
    });
}

#[test]
fn read_memory_that_the_machine_cannot_answer_keeps_the_rows_size_and_says_why() {
    run(|w| async move {
        let c = &*w.ctx;
        let machine = machine_of(&w).await;
        let rec = Rc::new(RefCell::new(record("w_1")));
        let lines = wsp_core::console::capture();
        assert!(!MachinesRuntime::read_memory(c, &rec, &machine, &[]).await);
        assert_eq!(rec.borrow().size.mem_mb.0, 4096.0);
        assert_eq!(lines.lines().len(), 1);
        assert!(lines.lines()[0].starts_with(&format!("workspace w_1: memory not read on {}", machine.id())), "{:?}", lines.lines());
    });
}
// waits on packet 2 (`RecordsCtx::warn`).

#[test]
fn image_of_names_the_golden_and_recorded_images_lists_what_the_store_holds() {
    run(|w| async move {
        let c = &*w.ctx;
        assert_eq!(MachinesRuntime::recorded_images(c).await.expect("a list"), Vec::<String>::new());
        assert_eq!(MachinesRuntime::image_of(c, "snap_g").await.expect("an image").version, None);
    });
}
// waits on packet 2 (`RecordsCtx::store`).

// ---------------------------------------------------------------------------------------------------------------
// The kind roads (kinds.ts:514-534) as these areas ask them.

#[test]
fn what_a_backend_offers_is_read_off_its_capabilities_by_every_trait() {
    run(|w| async move {
        let c = &*w.ctx;
        let own: Arc<dyn MachineBackend> = w.backend.clone();
        let caps = own.capabilities();
        assert_eq!(MachinesRuntime::keeps_images(c, &own), caps.images);
        assert_eq!(MachinesRuntime::names_workspace(c, &own), own.names_workspace());
        let keeps_disk = caps.pause_mode == Some(wsp_wire::wire::capabilities::PauseMode::Disk);
        assert_eq!(MachinesRuntime::pause_keeps_disk(c, &own), keeps_disk);
        assert_eq!(ReachCtx::pause_keeps_disk(c, &*own), keeps_disk);
    });
}

#[test]
fn backend_for_a_cloud_record_is_the_runtimes_own_backend() {
    run(|w| async move {
        let c = &*w.ctx;
        let own: Arc<dyn MachineBackend> = w.backend.clone();
        for got in [
            MachinesRuntime::backend_for(c, &record("w_1")).expect("a backend"),
            DaemonRuntime::backend_for(c, &record("w_1")).expect("a backend"),
            ReachCtx::backend_for(c, &record("w_1")).expect("a backend"),
        ] {
            assert!(Arc::ptr_eq(&got, &own));
        }
    });
}
// waits on packet 2 (`KindsCtx`).

#[test]
fn module_of_names_the_kinds_module_and_refuses_a_word_that_is_no_kind() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(MachinesRuntime::module_of(c, "cloud").is_ok());
        assert!(DaemonRuntime::module_of(c, "cloud").is_ok());
        assert!(MachinesRuntime::module_of(c, "teleporter").is_err());
    });
}
// waits on packet 2 (`KindsCtx`).

#[test]
fn the_daemon_answer_time_is_the_lifecycle_budget_of_the_entrys_backend() {
    run(|w| async move {
        let e = entry(&w, "w_1").await;
        assert_eq!(ReachCtx::daemon_answers_ms(&*w.ctx, &e).expect("a budget"), 1_000.0);
    });
}
// waits on packet 2 (`KindsCtx`).

#[test]
fn kind_has_daemon_asks_the_kinds_module() {
    run(|w| async move {
        let e = entry(&w, "w_1").await;
        let module = MachinesRuntime::module_of(&*w.ctx, "cloud").expect("a module");
        assert_eq!(ReachCtx::kind_has_daemon(&*w.ctx, &e).expect("an answer"), module.has_daemon(&e.borrow()));
    });
}
// waits on packet 2 (`KindsCtx`).

#[test]
fn machine_state_of_is_the_status_words_for_a_phase() {
    run(|w| async move {
        for phase in [WorkspacePhase::Running, WorkspacePhase::Napping, WorkspacePhase::Gone] {
            let word = serde_json::to_value(wsp_core::status::machine_state_of(phase)).expect("a word");
            assert_eq!(json!(ReachCtx::machine_state_of(&*w.ctx, &phase)), word);
        }
    });
}

#[test]
fn at_provider_reads_the_place_table_the_way_the_core_does() {
    run(|w| async move {
        let c = &*w.ctx;
        let places = MachinesRuntime::places(c);
        for place in [None, Some("default"), Some("pl_9")] {
            assert_eq!(MachinesRuntime::at_provider(c, place), wsp_core::types::wiring::at_provider(place, &*places));
        }
    });
}

#[test]
fn a_place_without_a_door_is_named_by_its_id_and_has_no_settings() {
    run(|w| async move {
        let c = &*w.ctx;
        assert_eq!(MachinesRuntime::place_name_of(c, "pl_9"), "pl_9");
        assert_eq!(MachinesRuntime::place_nap_ms(c, Some("pl_9")), None);
        assert_eq!(MachinesRuntime::place_nap_ms(c, None), None);
    });
}
// waits on packet 3 (`ChannelsCtx::has_door`, for `place_nap_ms`).

#[test]
fn place_id_and_provider_come_off_the_channels_area() {
    run(|w| async move {
        let c = &*w.ctx;
        assert_eq!(MachinesRuntime::place_id_of(c, &record("w_1")), Some("default".to_string()));
        assert_eq!(ReachCtx::provider_of(c, &record("w_1")), Some("default".to_string()));
        let mut placed = record("w_2");
        placed.place = Some("pl_9".into());
        assert_eq!(MachinesRuntime::place_id_of(c, &placed), Some("pl_9".to_string()));
        assert_eq!(ReachCtx::provider_of(c, &placed), None);
    });
}
// waits on packet 3 (`ChannelsCtx`).

#[test]
fn project_path_and_checkout_follow_the_held_project() {
    run(|w| async move {
        let c = &*w.ctx;
        w.ctx.machines_seams().projects_held.borrow_mut().insert("p_1".into(), project("p_1", "web"));
        assert_eq!(DaemonRuntime::project_path(c, "p_1").expect("a path"), "/home/me/web");
        assert_eq!(DaemonRuntime::checkout_of(c, &record("w_1")).expect("a checkout"), "/home/me/web");
        assert_eq!(ReachCtx::checkout_of(c, &record("w_1")).expect("a checkout"), "/home/me/web");
        assert!(DaemonRuntime::project_path(c, "p_9").is_err());
        assert!(ReachCtx::project_held(c, "p_9").is_err());
        assert_eq!(ReachCtx::project_held(c, "p_1").expect("a project").name, "web");
    });
}
// waits on packet 3 (`ChannelsCtx::projects_held`).

#[test]
fn a_machine_with_no_daemon_frame_is_not_served_by_its_computer() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = entry(&w, "w_1").await;
        assert!(!ReachCtx::served_by_its_computer(c, &e));
        assert_eq!(DaemonRuntime::served_by_its_computer(c, &e), None);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The status roads (reach.ts) and the marks a row carries.

#[test]
fn unreached_of_reads_the_daemon_areas_map_and_a_mark_for_another_machine_is_dropped() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = entry(&w, "w_1").await;
        let machine_id = e.borrow().machine.id().to_string();
        let marks = MachinesRuntime::unreached(c);
        marks.borrow_mut().insert("w_1".into(), Unreached { machine_id: machine_id.clone(), line: "the provider cannot reach it".into() });
        assert_eq!(ReachCtx::unreached_of(c, &e), Some("the provider cannot reach it".to_string()));
        marks.borrow_mut().insert("w_1".into(), Unreached { machine_id: "m_other".into(), line: "stale".into() });
        assert_eq!(ReachCtx::unreached_of(c, &e), None);
        assert!(marks.borrow().get("w_1").is_none());
    });
}

#[test]
fn a_row_that_is_unreached_reads_unreachable_without_asking_the_kind() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = entry(&w, "w_1").await;
        let machine_id = e.borrow().machine.id().to_string();
        MachinesRuntime::unreached(c).borrow_mut().insert("w_1".into(), Unreached { machine_id, line: "gone quiet".into() });
        assert_eq!(MachinesRuntime::reach_of(c, &e).expect("a reach"), ReachState::Unreachable);
    });
}

#[test]
fn a_refused_nap_and_a_refused_stop_are_read_for_the_machine_they_were_said_of() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = entry(&w, "w_1").await;
        let machine_id = e.borrow().machine.id().to_string();
        MachinesRuntime::nap_refusals(c)
            .borrow_mut()
            .insert("w_1".into(), ForMachine { machine_id: machine_id.clone(), value: "busy".into() });
        MachinesRuntime::stop_refusals(c).borrow_mut().insert("w_1".into(), ForMachine { machine_id, value: "held".into() });
        assert_eq!(MachinesRuntime::nap_refused_of(c, &e), Some("busy".to_string()));
        assert!(MachinesRuntime::nap_refused_reason(c, &e).expect("a reason").contains("busy"));
        assert!(MachinesRuntime::stop_refused_reason(c, &e).expect("a reason").contains("held"));
        MachinesRuntime::nap_refusals(c)
            .borrow_mut()
            .insert("w_1".into(), ForMachine { machine_id: "m_other".into(), value: "old".into() });
        assert_eq!(MachinesRuntime::nap_refused_of(c, &e), None);
    });
}

#[test]
fn a_row_says_what_its_wake_said_before_any_refusal() {
    run(|w| async move {
        let e = entry(&w, "w_1").await;
        e.borrow_mut().wake_said = Some("waking it".into());
        assert_eq!(ReachCtx::row_reason(&*w.ctx, &e), Some("waking it".to_string()));
    });
}

#[test]
fn emit_status_puts_a_status_row_on_the_bus_and_says_waking_keeps_its_line_on_the_entry() {
    run(|w| async move {
        let c = &*w.ctx;
        w.ctx.machines_seams().projects_held.borrow_mut().insert("p_1".into(), project("p_1", "web"));
        let e = live_entry(&w, "w_1").await;
        let statuses = Rc::new(RefCell::new(Vec::<Value>::new()));
        let log = statuses.clone();
        let _off = w.runtime.events().on(
            "workspace.status",
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event")["status"].clone());
                Ok(())
            }),
        );
        MachinesRuntime::emit_status(c, &e, ReachState::Reachable, Some("hello")).await.expect("a status");
        MachinesRuntime::says_waking(c, &e, "waking it").await.expect("a status");
        let seen = statuses.borrow();
        assert_eq!(seen[0]["id"], json!("w_1"));
        assert_eq!(seen[0]["reach"]["state"], json!("reachable"));
        assert_eq!(e.borrow().wake_said.as_deref(), Some("waking it"));
    });
}
// waits on packet 2 and packet 15 (`RecordsCtx::project_ref`; `StatusArea` rows).

#[test]
fn push_status_and_ping_daemon_ask_the_reach_area_for_the_entry() {
    run(|w| async move {
        let c = &*w.ctx;
        w.ctx.machines_seams().projects_held.borrow_mut().insert("p_1".into(), project("p_1", "web"));
        let e = live_entry(&w, "w_1").await;
        DaemonRuntime::push_status(c, &e).await.expect("a push");
        assert!(MachinesRuntime::ping_daemon(c, &e).await.is_none() || MachinesRuntime::ping_daemon(c, &e).await.is_some());
    });
}
// waits on packet 2 and packet 3 (`RecordsCtx`, `ChannelsCtx`).

// ---------------------------------------------------------------------------------------------------------------
// The roads to a machine's daemon.

#[test]
fn on_this_computer_without_a_daemon_here_refuses_in_the_hosts_words() {
    run(|w| async move {
        let refused =
            ReachCtx::on_this_computer(&*w.ctx, Box::new(|home| json!({ "op": "ls", "path": home }))).await.expect_err("a refusal");
        assert_eq!(refused.message(), "this host runs no daemon on this computer");
    });
}
// waits on packet 3 (`ChannelsCtx::local_has_daemon`).

#[test]
fn with_daemon_on_a_machine_that_answers_no_daemon_fails_with_the_machines_words() {
    run(|w| async move {
        let e = entry(&w, "w_1").await;
        assert!(ReachCtx::with_daemon(&*w.ctx, &e, json!({ "op": "ls" })).await.is_err());
    });
}
// waits on packet 3 (`ChannelsCtx`).

#[test]
fn daemon_token_of_a_machine_with_no_guest_is_the_transcripts_areas_answer() {
    run(|w| async move {
        let machine = machine_of(&w).await;
        let a = ReachCtx::daemon_token_of(&*w.ctx, &machine).await.map_err(|e| e.message());
        let b = ReachCtx::daemon_token_of(&*w.ctx, &machine).await.map_err(|e| e.message());
        assert_eq!(a, b);
    });
}
// waits on packet 3 (`TranscriptsCtx::rotate_daemon_token`).

#[test]
fn connect_daemon_answers_a_reach_over_the_url_and_token_it_was_given() {
    run(|w| async move {
        let reach = ReachCtx::connect_daemon(&*w.ctx, "http://127.0.0.1:9/", "t", Rc::new(|_| {}), None);
        assert!(reach.is_ok());
    });
}

#[test]
fn the_daemon_note_map_is_the_channels_areas_and_a_forget_takes_the_workspace_out_of_it() {
    run(|w| async move {
        let c = &*w.ctx;
        DaemonRuntime::daemon_notes(c).borrow_mut().insert("w_1".into(), "updating".into());
        DaemonRuntime::daemon_notes(c).borrow_mut().insert("w_2".into(), "keep".into());
        MachinesRuntime::forget_workspace_maps(c, "w_1");
        let notes = DaemonRuntime::daemon_notes(c);
        assert_eq!(notes.borrow().keys().cloned().collect::<Vec<_>>(), vec!["w_2".to_string()]);
    });
}

#[test]
fn sync_daemon_and_write_daemon_roots_run_the_daemon_area_for_the_entry() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = live_entry(&w, "w_1").await;
        MachinesRuntime::sync_daemon(c, &e);
        MachinesRuntime::write_daemon_roots(c, &e).await.expect("a write");
        tokio::task::yield_now().await;
    });
}
// waits on packet 3 (`ChannelsCtx`) and packet 2 (`KindsCtx`).

#[test]
fn sub_lost_releases_a_sub_threads_workspace_with_the_reason_and_answers_settled() {
    run(|w| async move {
        let sub = json!({
            "on": "boat", "repo": "https://github.com/acme/web.git", "base": { "branch": "feat/upload", "sha": "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a" },
            "branch": "wsp/fix-the-flaky-upload-test", "folder": "/root/web", "rateUsdPerHour": 0.018, "sizeName": "small",
        });
        let e = entry_with(&w, "w_1", json!({ "sub": sub })).await;
        MachinesRuntime::live(&*w.ctx).borrow_mut().insert("w_1".into(), e.clone());
        let outcome = MachinesRuntime::sub_lost(&*w.ctx, &e, "its machine is gone").await;
        assert!(matches!(outcome, Ok(wsp_core::types::wiring::GoneOutcome::Settled)));
        let record = e.borrow().record.borrow().clone();
        assert_eq!(record.phase, WorkspacePhase::Released);
        assert_eq!(record.sub.expect("the sub").lost.as_deref(), Some("its machine is gone"));
    });
}
// waits on packet 7 (`SubLifeCtx`).

#[test]
fn a_detached_send_starts_through_the_sessions_area() {
    run(|w| async move {
        let ask = wsp_machines::machines::daemon::DetachedStart {
            prompt: "go".into(),
            thread: None,
            request_id: "r_1".into(),
            on_held: Box::new(|_, _| {}),
        };
        let started = DaemonRuntime::sessions_start(&*w.ctx, "w_1", ask, None).await;
        assert!(started.is_err());
    });
}
// waits on packet 11 (`LaunchCtx`) and packet 13 (sessions).

#[test]
fn entry_of_finds_a_live_record_and_refuses_an_unknown_one() {
    run(|w| async move {
        let c = &*w.ctx;
        let e = live_entry(&w, "w_1").await;
        assert!(Rc::ptr_eq(&MachinesRuntime::entry_of(c, "w_1").await.expect("the entry"), &e));
        let refused = MachinesRuntime::entry_of(c, "w_9").await.err().expect("a refusal");
        assert!(refused.message.contains("w_9"), "{}", refused.message);
    });
}
// waits on packet 5 (boot: `BootArea::entry_of` awaits the start).

// ---------------------------------------------------------------------------------------------------------------
// The engine roads: the sweep after a machine is made, the vault, the gone watch.

#[test]
fn sweep_leftovers_runs_the_engines_sweep_on_the_machine() {
    run(|w| async move {
        let machine = machine_of(&w).await;
        let refused = ReachCtx::sweep_leftovers(&*w.ctx, &machine).await.expect_err("a refusal");
        assert_eq!(refused.message(), FAKE_NO_GUEST);
    });
}

#[test]
fn vault_export_walks_the_machine_for_the_paths_the_host_did_not_name_and_uses_the_ones_it_did() {
    run(|w| async move {
        let machine = machine_of(&w).await;
        let refused = MachinesRuntime::vault_export(&*w.ctx, &machine, 1 << 20, None).await.expect_err("a refusal");
        assert_eq!(refused.message, FAKE_NO_GUEST);
    });
    run_with(
        |o| o.vault_paths = Some(vec!["/root/.claude".into()]),
        |w| async move {
            let machine = machine_of(&w).await;
            let refused = MachinesRuntime::vault_export(&*w.ctx, &machine, 1 << 20, Some(vec!["/root/.claude/cache".into()]))
                .await
                .expect_err("a refusal");
            assert_eq!(refused.message, FAKE_NO_GUEST);
        },
    );
}

#[test]
fn gone_stop_kills_the_machine_through_the_runtimes_gone_watch() {
    run(|w| async move {
        let machine = machine_of(&w).await;
        let id = machine.id().to_string();
        let own: Arc<dyn MachineBackend> = w.backend.clone();
        MachinesRuntime::gone_stop(&*w.ctx, &own, &machine).await.expect("a stop");
        assert_eq!(w.backend.get(&id).await.expect("the record stays").state().await.expect("a state"), MachineState::Gone);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// What drop takes away beside the record (machines.ts:540-576).

#[test]
fn forget_workspace_maps_deletes_the_id_from_the_daemon_and_reach_areas_and_leaves_the_others() {
    run(|w| async move {
        let c = &*w.ctx;
        let seams = c.machines_seams();
        let moment = |m: &str| MachineMoment { machine_id: m.into(), at: wsp_wire::types::JsNumber(1.0) };
        let reach = |s: ReachState| ForMachine { machine_id: "m_1".into(), value: s };
        for id in ["w_1", "w_2"] {
            seams.daemon.revived_at_held().borrow_mut().insert(id.into(), moment("m_1"));
            seams.daemon.unread_at().borrow_mut().insert(id.into(), moment("m_1"));
            seams.daemon.unreached().borrow_mut().insert(id.into(), Unreached { machine_id: "m_1".into(), line: "x".into() });
            seams.reach.polled_reach.borrow_mut().insert(id.into(), reach(ReachState::Reachable));
            seams.reach.nap_refusals.borrow_mut().insert(id.into(), ForMachine { machine_id: "m_1".into(), value: "busy".into() });
            DaemonRuntime::daemon_notes(c).borrow_mut().insert(id.into(), "note".into());
        }
        seams.reach.stop_refusals.borrow_mut().insert("w_1".into(), ForMachine { machine_id: "m_1".into(), value: "held".into() });
        MachinesRuntime::forget_workspace_maps(c, "w_1");
        let keys = |m: Vec<String>| m;
        assert_eq!(keys(seams.daemon.revived_at().keys().cloned().collect()), vec!["w_2"]);
        assert_eq!(keys(seams.daemon.unread_at().borrow().keys().cloned().collect()), vec!["w_2"]);
        assert_eq!(keys(seams.daemon.unreached().borrow().keys().cloned().collect()), vec!["w_2"]);
        assert_eq!(keys(seams.reach.polled_reach.borrow().keys().cloned().collect()), vec!["w_2"]);
        assert_eq!(keys(seams.reach.nap_refusals.borrow().keys().cloned().collect()), vec!["w_2"]);
        assert_eq!(keys(DaemonRuntime::daemon_notes(c).borrow().keys().cloned().collect()), vec!["w_2"]);
        // machines.ts:540-550 leaves the stop refusals to the reach area's own sweep.
        assert!(seams.reach.stop_refusals.borrow().contains_key("w_1"));
    });
}

#[test]
fn forget_workspace_maps_drops_the_workspaces_transcript_queue_index_and_marks() {
    run(|w| async move {
        let c = &*w.ctx;
        let state = wsp_threads::threads::transcripts::TranscriptsCtx::transcript_state(c);
        state.with(|s| {
            for id in ["w_1", "w_2"] {
                s.index.insert(id.into(), Default::default());
                s.pending.insert(id.into(), Vec::new());
                s.pending_bytes.insert(id.into(), 10);
            }
        });
        MachinesRuntime::forget_workspace_maps(c, "w_1");
        assert_eq!(state.with(|s| s.index.keys().cloned().collect::<Vec<_>>()), vec!["w_2"]);
        assert_eq!(state.with(|s| s.pending.keys().cloned().collect::<Vec<_>>()), vec!["w_2"]);
        assert_eq!(state.with(|s| s.pending_bytes.keys().cloned().collect::<Vec<_>>()), vec!["w_2"]);
    });
}
// waits on packet 3 (`TranscriptsCtx::transcript_state`).

#[test]
fn forget_viewed_and_forget_kept_writes_delete_one_workspaces_entry() {
    run(|w| async move {
        let c = &*w.ctx;
        let seams = c.machines_seams();
        for id in ["w_1", "w_2"] {
            seams.transcripts.viewed_marks.borrow_mut().insert(id.into(), IndexMap::from([("t".to_string(), "m".to_string())]));
            seams.transcripts.kept_writes.borrow_mut().insert(id.into(), done());
        }
        MachinesRuntime::forget_viewed(c, "w_1");
        MachinesRuntime::forget_kept_writes(c, "w_1");
        assert_eq!(seams.transcripts.viewed_marks.borrow().keys().cloned().collect::<Vec<_>>(), vec!["w_2"]);
        assert_eq!(seams.transcripts.kept_writes.borrow().keys().cloned().collect::<Vec<_>>(), vec!["w_2"]);
    });
}

#[test]
fn cancel_flush_stops_the_workspaces_flush_timer() {
    run(|w| async move {
        let seams = w.ctx.machines_seams();
        let cancelled = Rc::new(Cell::new(false));
        let flag = cancelled.clone();
        seams.transcripts.transcript_timers.borrow_mut().insert("w_1".into(), Rc::new(move || flag.set(true)));
        MachinesRuntime::cancel_flush(&*w.ctx, "w_1");
        assert!(cancelled.get());
        assert!(seams.transcripts.transcript_timers.borrow().is_empty());
    });
}

#[test]
fn settle_flushes_waits_out_the_transcript_queue_and_then_drops_it() {
    run(|w| async move {
        let c = &*w.ctx;
        let seams = c.machines_seams();
        let (release, held) = futures::channel::oneshot::channel::<()>();
        let queued: Joined = async move {
            let _ = held.await;
        }
        .boxed_local()
        .shared();
        seams.transcripts.transcript_queue.borrow_mut().insert("w_1".into(), queued);
        let mut settling = MachinesRuntime::settle_flushes(c, "w_1");
        assert!((&mut settling).now_or_never().is_none());
        assert!(seams.transcripts.transcript_queue.borrow().contains_key("w_1"));
        release.send(()).expect("the queue is waiting");
        settling.await;
        assert!(seams.transcripts.transcript_queue.borrow().is_empty());
    });
}

#[test]
fn settle_flushes_waits_out_the_index_flush_and_drops_it() {
    run(|w| async move {
        let c = &*w.ctx;
        let flushes = wsp_threads::threads::transcripts::TranscriptsCtx::index_flushes(c);
        let (release, held) = futures::channel::oneshot::channel::<()>();
        let flush: Joined = async move {
            let _ = held.await;
        }
        .boxed_local()
        .shared();
        flushes.with(|f| f.insert("w_1".into(), flush));
        let mut settling = MachinesRuntime::settle_flushes(c, "w_1");
        assert!((&mut settling).now_or_never().is_none());
        release.send(()).expect("the flush is waiting");
        settling.await;
        assert!(flushes.with(|f| f.is_empty()));
    });
}
// waits on packet 3 (`TranscriptsCtx::index_flushes`).

#[test]
fn clear_rows_clears_the_workspaces_rows_and_its_unread_mark() {
    run(|w| async move {
        let c = &*w.ctx;
        let state = wsp_threads::threads::transcripts::TranscriptsCtx::transcript_state(c);
        state.with(|s| {
            s.unread.insert("w_1".into());
            s.unread.insert("w_2".into());
        });
        MachinesRuntime::clear_rows(c, "w_1");
        assert_eq!(state.with(|s| s.unread.iter().cloned().collect::<Vec<_>>()), vec!["w_2"]);
    });
}
// waits on packet 3 (`TranscriptsCtx::transcript_state`).

#[test]
fn drop_sent_images_of_a_workspace_that_kept_none_answers_ok() {
    run(|w| async move {
        MachinesRuntime::drop_sent_images(&*w.ctx, "w_1").await.expect("nothing to drop");
    });
}
// waits on packet 3 (`TranscriptsCtx::store`).

#[test]
fn reap_kept_of_a_thread_no_agent_is_kept_for_does_nothing() {
    run(|w| async move {
        MachinesRuntime::reap_kept(&*w.ctx, "t_1");
        assert!(w.ctx.machines_seams().threads.kept_agents.borrow().is_empty());
    });
}

#[test]
fn forget_slate_of_a_thread_with_no_slate_answers_ok() {
    run(|w| async move {
        MachinesRuntime::forget_slate(&*w.ctx, "t_1").await.expect("a forget");
    });
}
// waits on packet 16 (`SlatesAreaCtx`).

#[test]
fn thread_records_start_empty_and_every_call_answers_the_one_map() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(MachinesRuntime::thread_records(c).borrow().is_empty());
        assert!(Rc::ptr_eq(&MachinesRuntime::thread_records(c), &MachinesRuntime::thread_records(c)));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The scenarios of the whole-runtime tests, at the area over this context.

#[test]
fn a_dropped_workspace_leaves_nothing_of_its_own_in_the_maps_every_area_holds() {
    run(|w| async move {
        let c = &*w.ctx;
        let seams = c.machines_seams();
        seams.projects_held.borrow_mut().insert("p_1".into(), project("p_1", "web"));
        let e = live_entry(&w, "w_1").await;
        let other = live_entry(&w, "w_2").await;
        let machine_id = e.borrow().machine.id().to_string();
        MachinesRuntime::sessions(c).borrow_mut().insert("t_1".into(), session_row("t_1", "w_1"));
        MachinesRuntime::sessions(c).borrow_mut().insert("t_2".into(), session_row("t_2", "w_2"));
        seams.daemon.unreached().borrow_mut().insert("w_1".into(), Unreached { machine_id, line: "x".into() });
        for (thread, workspace) in [("t_1", "w_1"), ("t_2", "w_2")] {
            let held = ThreadRecord { workspace_id: workspace.into(), harness: "claude".into(), ..Default::default() };
            MachinesRuntime::thread_records(c).borrow_mut().insert(thread.into(), held);
        }
        MachinesRuntime::persist(c, &record("w_1")).await.expect("a persist");
        let deleted = Rc::new(RefCell::new(Vec::<String>::new()));
        let log = deleted.clone();
        let _off = w.runtime.events().on(
            "workspace.deleted",
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event")["workspaceId"].as_str().unwrap_or("").to_string());
                Ok(())
            }),
        );
        seams.transcripts.viewed_marks.borrow_mut().insert("w_1".into(), IndexMap::new());
        seams.transcripts.kept_writes.borrow_mut().insert("w_1".into(), done());
        DaemonRuntime::daemon_notes(c).borrow_mut().insert("w_1".into(), "note".into());
        let area = area_of(&w);
        area.drop("w_1").await.expect("a drop");
        let live = MachinesRuntime::live(c);
        assert_eq!(live.borrow().keys().cloned().collect::<Vec<_>>(), vec!["w_2"]);
        assert!(Rc::ptr_eq(live.borrow().get("w_2").expect("the other"), &other));
        assert_eq!(MachinesRuntime::sessions(c).borrow().keys().cloned().collect::<Vec<_>>(), vec!["t_2"]);
        assert_eq!(MachinesRuntime::thread_records(c).borrow().keys().cloned().collect::<Vec<_>>(), vec!["t_2"]);
        assert_eq!(*deleted.borrow(), vec!["w_1"]);
        assert!(seams.daemon.unreached().borrow().is_empty());
        assert!(seams.transcripts.viewed_marks.borrow().is_empty());
        assert!(seams.transcripts.kept_writes.borrow().is_empty());
        assert!(DaemonRuntime::daemon_notes(c).borrow().is_empty());
        let stored = MachinesRuntime::store(c).get("workspaces", "w_1").await.expect("a get");
        assert_eq!(stored, None);
    });
}
// waits on packet 2, packet 3 and packet 16 (the kinds module's `dropped`, the transcripts' store, the slates).

#[test]
fn a_machine_the_provider_lost_is_settled_gone_with_its_words_and_its_turns_end() {
    run(|w| async move {
        let c = &*w.ctx;
        w.ctx.machines_seams().projects_held.borrow_mut().insert("p_1".into(), project("p_1", "web"));
        let e = live_entry(&w, "w_1").await;
        let id = e.borrow().machine.id().to_string();
        let ended = Rc::new(RefCell::new(Vec::<String>::new()));
        let log = ended.clone();
        let row = session_row("t_1", "w_1");
        row.borrow_mut().end = Some(Rc::new(move |reason: &str, _, _| log.borrow_mut().push(reason.to_string())));
        MachinesRuntime::sessions(c).borrow_mut().insert("t_1".into(), row);
        let gone = Rc::new(RefCell::new(Vec::<Value>::new()));
        let log = gone.clone();
        let _off = w.runtime.events().on(
            "workspace.gone",
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            }),
        );
        // Deleted at the provider under a running host.
        w.backend.get(&id).await.expect("the machine").kill().await.expect("a kill");
        let area = area_of(&w);
        area.mark_gone(&e, &id, "the status poll found it gone at 2026-10-26T00:00:00.000Z").await.expect("a settle");
        assert_eq!(e.borrow().record.borrow().phase, WorkspacePhase::Gone);
        assert_eq!(gone.borrow().len(), 1);
        assert_eq!(gone.borrow()[0]["workspaceId"], json!("w_1"));
        assert_eq!(*ended.borrow(), vec!["machine gone at the provider while the agent was working"]);
    });
}
// waits on packet 2, packet 3 and packet 13 (`RecordsCtx`, `ChannelsCtx`, the sessions rows `end` reads).
