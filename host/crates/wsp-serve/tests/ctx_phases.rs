// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 9 (#2476): the context's `PhasesCtx` (workspaces.ts:22-65, 289-645) and `GitCtx`
//! (workspaces.ts:647-1053), each method called through `&dyn Trait` on a runtime over `FakeBackend`; the TS is the
//! oracle. A test that reaches another packet's `todo!()` body fails on it until that packet lands; its comment says
//! which (machines and reach 4, boot 5, create 6, sub queue, sub life, pull requests 7, folders 8, projects 10, agents
//! and threads 11, transcripts 13).
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt as _;
use serde_json::{json, Value};
use wsp_account::projects::workspaces::git::GitCtx;
use wsp_account::projects::workspaces::phases::{MachineRead, PhasesCtx};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::clock::FakeClock;
use wsp_core::types::api::Runtime;
use wsp_core::types::events::{EventListener, Unlisten};
use wsp_core::types::internal::{SessionEntry, DROPPED};
use wsp_core::types::wiring::{RuntimeOptions, WorkspaceRecord};
use wsp_engine::errors::EngineError;
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions, FakeGuest};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{Machine, MachineBackend, MachineSpec, MachineState, PreviewReach};
use wsp_machines::machines::boot::BootRuntime;
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::store::{memory_store, Store};
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::wire::events_union::EventUnion;

/// A stand-in guest whose machines run their commands in a folder of the test's own.
struct Folder(PathBuf);

#[async_trait(?Send)]
impl FakeGuest for Folder {
    fn folder(&self, machine_id: &str) -> PathBuf {
        self.0.join(machine_id)
    }
    fn token_path(&self, machine_id: &str) -> String {
        self.0.join(machine_id).join("token").to_string_lossy().into_owned()
    }
    async fn reach(&self, _machine_id: &str, _port: u16) -> wsp_engine::errors::Result<PreviewReach> {
        Err(EngineError::Other { message: "no preview in this test".into(), code: None })
    }
}

struct Fx {
    c: Rc<Ctx>,
    rt: Rc<AssembledRuntime>,
    backend: Arc<dyn MachineBackend>,
    store: Rc<dyn Store>,
    clock: FakeClock,
}

/// A runtime to build once the task set the owner thread runs is up.
struct Setup {
    backend: Arc<dyn MachineBackend>,
    extra: Box<dyn FnOnce(&mut RuntimeOptions)>,
}

fn fx_over(backend: Arc<dyn MachineBackend>, extra: impl FnOnce(&mut RuntimeOptions) + 'static) -> Setup {
    Setup { backend, extra: Box::new(extra) }
}

fn build(setup: Setup) -> Fx {
    let store: Rc<dyn Store> = Rc::new(memory_store());
    let clock = FakeClock::new(1_000_000);
    let mut opts = RuntimeOptions {
        backend: Some(setup.backend.clone()),
        store: Some(store.clone()),
        clock: Some(clock.clock()),
        ..Default::default()
    };
    (setup.extra)(&mut opts);
    let store = opts.store.clone().expect("a store");
    let rt = create_runtime(opts).expect("a runtime");
    // Their records are of `p_1`, which the host holds: the TS view of a record throws for a project it does not (records.ts:88).
    BootRuntime::projects_held(&*rt.context()).borrow_mut().insert("p_1".into(), project("p_1", "api", "/srv/api"));
    Fx { c: rt.context(), rt, backend: setup.backend, store, clock }
}

fn drive_over<Fut: std::future::Future<Output = ()>>(setup: Setup, f: impl FnOnce(Fx) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    tokio::task::LocalSet::new().block_on(&rt, async move { f(build(setup)).await });
}

/// The runtime on a throwaway state, inside the task set the owner thread runs.
fn drive<Fut: std::future::Future<Output = ()>>(f: impl FnOnce(Fx) -> Fut) {
    drive_over(fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |_| {}), f);
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

async fn machine_of(fx: &Fx) -> Arc<dyn Machine> {
    let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
    fx.backend.create(spec, None, None).await.expect("a machine")
}

/// A live entry over a record and a machine, as `attach` makes it, but held by no one.
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
    Rc::new(RefCell::new(wsp_core::types::wiring::LiveWorkspace {
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

async fn entry(fx: &Fx, extra: Value) -> Entry {
    let machine = machine_of(fx).await;
    let mut extra = extra;
    extra["machineId"] = json!(machine.id());
    entry_over(record(extra), machine)
}

async fn attached(fx: &Fx, extra: Value) -> Entry {
    let machine = machine_of(fx).await;
    let mut extra = extra;
    extra["machineId"] = json!(machine.id());
    BootRuntime::attach(&*fx.c, &Rc::new(RefCell::new(record(extra))), machine).expect("an entry")
}

fn held(e: &Entry) -> Arc<dyn Machine> {
    e.borrow().machine.clone()
}
fn phases(c: &Rc<Ctx>) -> &dyn PhasesCtx {
    &**c
}
fn git(c: &Rc<Ctx>) -> &dyn GitCtx {
    &**c
}

fn heard(fx: &Fx, kind: &str) -> (Rc<RefCell<Vec<Value>>>, Unlisten) {
    let seen: Rc<RefCell<Vec<Value>>> = Rc::default();
    let sink = seen.clone();
    let listener: EventListener = Rc::new(move |e: &EventUnion| {
        sink.borrow_mut().push(serde_json::to_value(e).expect("an event"));
        Ok(())
    });
    (seen, fx.rt.events().on(kind, listener))
}

fn session_row(id: &str, thread: &str) -> wsp_threads::threads::threads::Row {
    let view =
        serde_json::from_value(json!({ "id": id, "workspaceId": "w_1", "harness": "claude", "status": "completed", "threadId": thread }))
            .expect("a session view");
    Rc::new(RefCell::new(SessionEntry {
        view,
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

fn sub_record_json() -> Value {
    json!({ "on": "boat", "repo": "/r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/f" })
}

/// machines.ts:356: the machine a wake replaced is killed and its id marked dropped. Waits on packet 4.
#[test]
fn unfork_kills_the_machine_and_marks_it_dropped() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let id = e.borrow().machine.id().to_string();
        phases(&f.c).unfork(&e).await.expect("unforked");
        assert_eq!(held(&e).state().await.expect("a state"), MachineState::Gone);
        let mark = f.store.get(DROPPED, &id).await.expect("a read").expect("a mark");
        assert_eq!(mark["machineId"], json!(id));
    });
}

/// sub-life.ts:554: a lost sub-thread's record is released with the words it was lost with. Waits on packets 4 and 7.
#[test]
fn sub_lost_releases_the_record_with_the_reason() {
    drive(|f| async move {
        let e = attached(&f, json!({ "sub": sub_record_json() })).await;
        phases(&f.c).sub_lost(&e, "the cloud lost it").await.expect("released");
        let r = e.borrow().record.borrow().clone();
        assert_eq!(r.phase, WorkspacePhase::Released);
        assert_eq!(r.sub.expect("a sub record").lost.as_deref(), Some("the cloud lost it"));
    });
}

/// reach.ts:107: every thread of the workspace hears the line as a notice on its last turn. Waits on packet 4.
#[test]
fn tell_threads_notices_each_thread_of_the_workspace() {
    drive(|f| async move {
        BootRuntime::sessions(&*f.c).borrow_mut().insert("s_1".into(), session_row("s_1", "t_1"));
        BootRuntime::sessions(&*f.c).borrow_mut().insert("s_2".into(), session_row("s_2", "t_2"));
        let (seen, _off) = heard(&f, "session.notice");
        phases(&f.c).tell_threads("w_1", "the computer napped");
        let seen = seen.borrow();
        let told: Vec<(&str, &str)> = seen.iter().map(|e| (e["threadId"].as_str().unwrap(), e["text"].as_str().unwrap())).collect();
        assert_eq!(told, vec![("t_1", "the computer napped"), ("t_2", "the computer napped")]);
    });
}

/// reach.ts:172: the status event carries the reach the caller names and the words it says. Waits on packet 4.
#[test]
fn emit_status_says_the_reach_and_the_words() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let (seen, _off) = heard(&f, "workspace.status");
        phases(&f.c).emit_status(&e, "napping", Some("the nap is on")).await.expect("emitted");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0]["status"]["reach"]["state"], json!("napping"));
        assert_eq!(seen[0]["status"]["reason"], json!("the nap is on"));
        assert_eq!(seen[0]["status"]["id"], json!("w_1"));
    });
}

/// machines.ts:~: the record follows the engine once a wake put a machine under it; a gone record is gone no more. Waits on packet 4.
#[test]
fn follow_machine_puts_a_gone_record_back_to_running() {
    drive(|f| async move {
        let e = attached(&f, json!({ "phase": "gone", "gone": "machine m is gone" })).await;
        phases(&f.c).follow_machine(&e);
        let r = e.borrow().record.borrow().clone();
        assert_eq!(r.phase, WorkspacePhase::Running);
        assert_eq!(r.gone, None);
        assert_eq!(r.machine_id, e.borrow().ws.machine_id());
    });
}

/// reach.ts:165: the word of the state a workspace's row reads. Waits on packet 4.
#[test]
fn reach_of_names_the_state_in_words() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let word = phases(&f.c).reach_of(&e);
        assert!(["reachable", "no-daemon", "unsupported", "unreachable", "napping"].contains(&word.as_str()), "{word}");
    });
}

/// machines.ts:592: a record that is not running is answered as it stands, with no pause asked. Waits on packet 4.
#[test]
fn nap_with_answers_a_napping_record_as_its_view() {
    drive(|f| async move {
        let _e = attached(&f, json!({ "phase": "napping" })).await;
        let view = phases(&f.c).nap_with("w_1").await.expect("a view");
        assert_eq!(view.id, "w_1");
        assert_eq!(view.phase, WorkspacePhase::Napping);
    });
}

/// rules.ts:49: a computer whose provider declares a pause mode is let through.
#[test]
fn refuse_pauseless_lets_a_pausing_computer_through() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(phases(&f.c).refuse_pauseless(&e, "nap").is_ok());
    });
}

/// boot.ts:1029: a workspace the caller's tree does not hold is not talked to. Waits on packet 8 (`Base`).
#[test]
fn talks_to_tree_on_is_false_for_a_workspace_nobody_holds() {
    drive(|f| async move {
        let origin = Caller::Road(wsp_wire::views::workspace_more::WorkspaceOrigin::Here);
        assert!(!phases(&f.c).talks_to_tree_on("w_1", Some(&origin)));
    });
}

/// sub-life.ts:587: a record with no sub-thread wakes as any other. Waits on packet 7.
#[test]
fn refuse_sub_wake_lets_a_plain_record_through() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(phases(&f.c).refuse_sub_wake(&e).is_ok());
    });
}

/// sub-life.ts:587: a released sub-thread's wake is refused with the words that say it was released. Waits on packet 7.
#[test]
fn refuse_sub_wake_refuses_a_released_sub_thread() {
    drive(|f| async move {
        let e = entry(&f, json!({ "phase": "released", "sub": sub_record_json() })).await;
        let said = phases(&f.c).refuse_sub_wake(&e).expect_err("refused");
        assert!(said.to_string().contains("was deleted after it was done, so there is none to wake"), "{said}");
    });
}

/// machines.ts:745: a record that is not marked gone moves nothing. Waits on packet 4.
#[test]
fn recover_gone_leaves_a_running_record_alone() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        assert_eq!(phases(&f.c).recover_gone(&e).await.expect("answered"), None);
    });
}

/// workspaces.ts:312: `machine.state()` read as the three words the phases half knows and one for every other.
#[test]
fn machine_state_reads_the_machine() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        let m = e.borrow().machine.clone();
        assert_eq!(phases(&f.c).machine_state(&e).await.expect("a read"), MachineRead::Running);
        m.pause(None).await.expect("paused");
        assert_eq!(phases(&f.c).machine_state(&e).await.expect("a read"), MachineRead::Paused);
        m.kill().await.expect("killed");
        assert_eq!(phases(&f.c).machine_state(&e).await.expect("a read"), MachineRead::Gone);
    });
}

/// status.ts `providerSaid`: the status and the message when the call answered with both.
#[test]
fn provider_said_quotes_the_status_and_the_message() {
    drive(|f| async move {
        let plain = EngineError::Other { message: "boom".into(), code: None };
        assert_eq!(phases(&f.c).provider_said(&plain), "boom");
        let refused = EngineError::Refused { message: "no such machine".into(), kind: None, status: Some(404), code: None };
        assert_eq!(phases(&f.c).provider_said(&refused), "404 no such machine");
    });
}

/// workspaces.ts:317: a machine the provider still holds is not gone: nothing settles. Waits on packet 4.
#[test]
fn settle_gone_settles_nothing_over_a_machine_the_provider_holds() {
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |o| o.gone_confirm_ms = Some(0));
    drive_over(f, |f| async move {
        let e = attached(&f, json!({})).await;
        assert!(!phases(&f.c).settle_gone(&e, "wake", 1_000_000, Some("404 no such machine".into())).await.expect("answered"));
        assert_eq!(e.borrow().record.borrow().phase, WorkspacePhase::Running);
    });
}

/// workspaces.ts:317: a machine the provider no longer holds settles gone, with the record carrying goneWords. Waits on packet 4.
#[test]
fn settle_gone_settles_a_machine_the_provider_lost() {
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |o| o.gone_confirm_ms = Some(0));
    drive_over(f, |f| async move {
        let e = attached(&f, json!({})).await;
        held(&e).kill().await.expect("killed");
        assert!(phases(&f.c).settle_gone(&e, "wake", 0, Some("404 no such machine".into())).await.expect("answered"));
        let gone = e.borrow().record.borrow().gone.clone().expect("a gone line");
        let id = e.borrow().machine.id().to_string();
        assert_eq!(
            gone,
            format!("machine {id} is gone at the provider: the wake found it gone at 1970-01-01T00:00:00Z (404 no such machine)")
        );
    });
}

/// machines.ts:651: a record the engine says napping, whose machine runs, is adopted as running. Waits on packet 4.
#[test]
fn adopt_pause_follows_a_pause_the_provider_took() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        held(&e).pause(None).await.expect("paused");
        phases(&f.c).adopt_pause(&e).await.expect("adopted");
        assert_eq!(e.borrow().record.borrow().phase, WorkspacePhase::Napping);
    });
}

/// machines.ts:667: a napping record whose machine the provider runs is one that runs under a nap. Waits on packet 4.
#[test]
fn runs_under_napping_reads_the_provider() {
    drive(|f| async move {
        let e = attached(&f, json!({ "phase": "napping" })).await;
        assert!(phases(&f.c).runs_under_napping(&e).await.expect("a read"));
        held(&e).pause(None).await.expect("paused");
        assert!(!phases(&f.c).runs_under_napping(&e).await.expect("a read"));
    });
}

/// machines.ts:671: a napping record over a running machine is adopted as running. Waits on packet 4.
#[test]
fn adopt_running_follows_a_resume_the_provider_took() {
    drive(|f| async move {
        let e = attached(&f, json!({ "phase": "napping" })).await;
        phases(&f.c).adopt_running(&e).await.expect("adopted");
        assert_eq!(e.borrow().record.borrow().phase, WorkspacePhase::Running);
    });
}

/// machines.ts: `proven` answers the view of a record once its machine answers. Waits on packet 4.
#[test]
fn proven_answers_the_view_of_the_record() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let view = phases(&f.c).proven(&e).await.expect("a view");
        assert_eq!(view.id, "w_1");
    });
}

/// workspaces.ts:342: `lifecycleOf(entry).budgets.resumeAsks`: the stand-in's lifecycle declares none, so a wake asks once.
#[test]
fn resume_asks_is_none_where_the_backend_declares_no_budget() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert_eq!(phases(&f.c).resume_asks(&e), None);
    });
}

/// runtime.ts:511: `sleeps(ms)` waits on the runtime's clock, so a test that moves the fake clock moves it.
#[test]
fn sleeps_waits_on_the_clock() {
    drive(|f| async move {
        let c = f.c.clone();
        let slept = tokio::task::spawn_local(async move { phases(&c).sleeps(500).await });
        tokio::task::yield_now().await;
        assert!(!slept.is_finished());
        f.clock.advance(499);
        tokio::task::yield_now().await;
        assert!(!slept.is_finished());
        f.clock.advance(1);
        slept.await.expect("slept");
    });
}

/// runtime.ts `tookTheResume(await readsState(machine))`: a machine the provider runs took the resume, one it keeps paused did not.
#[test]
fn took_the_resume_reads_the_state() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(phases(&f.c).took_the_resume(&e).await);
        held(&e).pause(None).await.expect("paused");
        assert!(!phases(&f.c).took_the_resume(&e).await);
    });
}

/// lifecycle.ts:130: a wake of a napping engine resumes the machine and answers no reason.
#[test]
fn ws_wake_resumes_a_napping_machine() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        phases(&f.c).ws_note_paused(&e);
        assert_eq!(phases(&f.c).ws_current_phase(&e), WorkspacePhase::Napping);
        let woke = phases(&f.c).ws_wake(&e, false).await.expect("woke");
        assert_eq!(woke.reason, None);
        assert_eq!(phases(&f.c).ws_current_phase(&e), WorkspacePhase::Running);
    });
}

/// lifecycle.ts:167, 196: a rebuild and an upgrade need the resurrect hook the entry was made with.
#[test]
fn ws_rebuild_and_ws_upgrade_ask_the_engine() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        let rebuilt = phases(&f.c).ws_rebuild(&e).await.expect_err("no hook");
        assert_eq!(rebuilt.to_string(), "rebuild requires a resurrect hook");
        let upgraded = phases(&f.c).ws_upgrade(&e).await.expect_err("no hook");
        assert_eq!(upgraded.to_string(), "upgrade requires a resurrect hook");
    });
}

/// machines.ts:640: a gone record's late read is armed once; the record keeps its phase. Waits on packet 4.
#[test]
fn arm_late_read_arms_the_entry_once() {
    drive(|f| async move {
        let e = attached(&f, json!({ "phase": "napping" })).await;
        phases(&f.c).arm_late_read(&e);
        assert_eq!(e.borrow().record.borrow().phase, WorkspacePhase::Napping);
    });
}

/// workspaces.ts:420: whether `entry.machine.resumeAt` exists. The stand-in has none.
#[test]
fn can_resume_at_is_what_the_machine_offers() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(!phases(&f.c).can_resume_at(&e));
    });
}

/// channels.ts:230: a copy that names no place lives at the provider the host is wired to; the default table names it default.
#[test]
fn provider_of_is_the_wired_provider_for_a_driven_copy() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert_eq!(phases(&f.c).provider_of(&e).as_deref(), Some("default"));
    });
}

/// workspaces.ts: the capabilities of the backend the record names, read after its limits. The stand-in offers the pause mode it declares.
#[test]
fn backend_capabilities_reads_the_records_backend() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        let caps = phases(&f.c).backend_capabilities(&e).await.expect("capabilities");
        let want = f.backend.capabilities();
        assert_eq!(serde_json::to_value(&caps).expect("json"), serde_json::to_value(&want).expect("json"));
    });
}

/// threads.ts: whether a thread runs, read off the session rows. Waits on packet 11.
#[test]
fn thread_runs_is_false_for_a_thread_with_no_turn() {
    drive(|f| async move {
        assert!(!phases(&f.c).thread_runs("t_none"));
    });
}

/// records.ts:182: the shape the provider says its machine has; the stand-in has one only where it is told one.
#[test]
fn shape_of_reads_the_machine() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        let want = held(&e).describe().await.ok();
        assert_eq!(phases(&f.c).shape_of(&e).await, want);
    });
}

/// records.ts:150: the size the provider built where it says, the size asked where it does not.
#[test]
fn size_built_takes_the_shape_the_provider_built() {
    drive(|f| async move {
        let asked = wsp_core::types::wiring::SizeAsk { cpu: 2.0.into(), mem_mb: 4096.0.into() };
        let same = phases(&f.c).size_built(None, asked);
        assert_eq!((same.cpu.0, same.mem_mb.0), (2.0, 4096.0));
        let shape: wsp_wire::wire::machine_link::MachineShape =
            serde_json::from_value(json!({ "cpu": 4, "memMb": 8192 })).expect("a shape");
        let built = phases(&f.c).size_built(Some(&shape), asked);
        assert_eq!((built.cpu.0, built.mem_mb.0), (4.0, 8192.0));
    });
}

/// reach.ts:477: a running workspace's status is said once, with the reach it reads. Waits on packet 4.
#[test]
fn status_now_says_the_status_once() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let (seen, _off) = heard(&f, "workspace.status");
        phases(&f.c).status_now(&e).await.expect("said");
        assert_eq!(seen.borrow().len(), 1);
        assert_eq!(seen.borrow()[0]["status"]["id"], json!("w_1"));
    });
}

/// rules.ts:43: a capability the provider has lets the act through; one it lacks refuses it in the provider's words.
#[test]
fn refuse_cannot_reads_the_providers_capabilities() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(phases(&f.c).refuse_cannot(&e, "images", "seal").is_ok());
        let said = phases(&f.c).refuse_cannot(&e, "templates", "seal").expect_err("refused");
        assert_eq!(
            said.to_string(),
            wsp_words::words::computer::provider_cannot_refusal(
                "api",
                &wsp_words::workspace_state::machine_word(&wsp_wire::views::workspace_kind::WorkspaceKind::Cloud),
                "seal"
            )
        );
    });
}

/// kinds.ts keepsImages and pauseKeepsDisk: read off the capabilities of the backend the record names.
#[test]
fn keeps_images_and_pause_keeps_disk_read_the_backend() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(phases(&f.c).keeps_images(&e));
        assert!(!phases(&f.c).pause_keeps_disk(&e));
    });
}

/// create.ts:269: a name is given trimmed.
#[test]
fn name_given_trims_the_name() {
    drive(|f| async move {
        assert_eq!(phases(&f.c).name_given("  api \n"), "api");
    });
}

/// create.ts:271: a blank name is refused with the blank-name words.
#[test]
fn name_refusal_refuses_a_blank_name() {
    drive(|f| async move {
        assert_eq!(phases(&f.c).name_refusal("   ").as_deref(), Some(wsp_words::words::thread::BLANK_NAME_REFUSAL));
    });
}

/// create.ts:271: a name a live workspace holds is refused with the name-taken words. Waits on packet 6.
#[test]
fn name_refusal_refuses_a_name_a_workspace_holds() {
    drive(|f| async move {
        let _e = attached(&f, json!({})).await;
        assert_eq!(phases(&f.c).name_refusal("api"), Some(wsp_words::words::thread::name_taken_refusal("api")));
        assert_eq!(phases(&f.c).name_refusal("web"), None);
    });
}

/// records.ts:129: a look fact is set, cleared with null and left alone with none.
#[test]
fn put_look_sets_clears_and_leaves() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        let read = || serde_json::to_value(&*e.borrow().record.borrow()).expect("json")["glyph"].clone();
        phases(&f.c).put_look(&e, "glyph", Some(json!("rocket")));
        assert_eq!(read(), json!("rocket"));
        phases(&f.c).put_look(&e, "glyph", None);
        assert_eq!(read(), json!("rocket"));
        phases(&f.c).put_look(&e, "glyph", Some(Value::Null));
        assert_eq!(read(), Value::Null);
    });
}

/// channels.ts:130: a machine whose own computer does not serve the daemon is not served by it.
#[test]
fn served_by_its_computer_is_none_where_the_machine_offers_no_daemon_frame() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert_eq!(phases(&f.c).served_by_its_computer(&e), None);
    });
}

/// channels.ts:124: a record that names no place is on the computer its kind says.
#[test]
fn computer_of_names_the_kinds_computer() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert_eq!(
            phases(&f.c).computer_of(&e),
            wsp_words::workspace_state::machine_word(&wsp_wire::views::workspace_kind::WorkspaceKind::Cloud)
        );
    });
}

/// workspaces.ts:556: a kind with no deploy wired has none to call.
#[test]
fn has_deploy_daemon_is_false_where_the_host_wires_no_bundle() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(!phases(&f.c).has_deploy_daemon(&e));
    });
}

/// daemon.ts:147: a kind with nothing to deploy refuses. Waits on packet 4.
#[test]
fn deploy_daemon_on_refuses_where_the_kind_has_nothing_to_deploy() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let said = phases(&f.c).deploy_daemon_on(&e).await.expect_err("refused");
        assert_eq!(said.to_string(), "this kind has no daemon to deploy");
    });
}

/// sub-queue.ts:294: a record the queue holds no start of answers none. Waits on packet 7.
#[test]
fn delete_queued_answers_none_for_a_record_the_queue_does_not_hold() {
    drive(|f| async move {
        assert!(phases(&f.c).delete_queued("w_none").await.is_none());
    });
}

fn git_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ctx-phases-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    let init = std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).status().expect("git");
    assert!(init.success());
    dir
}

/// folders.ts: a folder with a file no commit holds is refused, naming how many; a clean one passes. Waits on packet 8.
#[test]
fn refuse_changed_counts_the_files_no_commit_holds() {
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |o| o.local = Some(local_wiring()));
    drive_over(f, |f| async move {
        let dir = git_dir("changed");
        let path = dir.to_string_lossy().into_owned();
        phases(&f.c).refuse_changed(&path).await.expect("clean");
        std::fs::write(dir.join("a.txt"), "x").expect("a file");
        std::fs::write(dir.join("b.txt"), "y").expect("a file");
        let said = phases(&f.c).refuse_changed(&path).await.expect_err("refused");
        assert_eq!(said.to_string(), wsp_words::projects::worktree_changed_line(2.0));
        let _ = std::fs::remove_dir_all(&dir);
    });
}

/// workspaces.ts:576: `existsSync`.
#[test]
fn path_exists_is_exists_sync() {
    drive(|f| async move {
        let dir = std::env::temp_dir();
        assert!(phases(&f.c).path_exists(&dir.to_string_lossy()));
        assert!(!phases(&f.c).path_exists(&dir.join("ctx-phases-no-such-folder").to_string_lossy()));
    });
}

struct Revoking(Rc<RefCell<Vec<String>>>);

#[async_trait(?Send)]
impl wsp_core::types::wiring::Broker for Revoking {
    fn ready(&self) -> bool {
        true
    }
    async fn codex_claims(&self) -> Result<Option<wsp_adapters::pending::CodexClaims>, wsp_words::exit::HostError> {
        Ok(None)
    }
    fn revoke(&self, workspace_id: &str) -> Option<wsp_core::types::wiring::Local<'static, ()>> {
        self.0.borrow_mut().push(workspace_id.to_string());
        Some(Box::pin(async { Ok(()) }))
    }
}

/// pipe.ts:396: a machine is cut off at the broker before its provider deletes it.
#[test]
fn cut_off_revokes_the_machine_at_the_broker() {
    let revoked: Rc<RefCell<Vec<String>>> = Rc::default();
    let held = revoked.clone();
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), move |o| o.broker = Some(Rc::new(Revoking(held))));
    drive_over(f, |f| async move {
        let e = entry(&f, json!({})).await;
        phases(&f.c).cut_off(&e).await.expect("cut off");
        assert_eq!(*revoked.borrow(), vec!["w_1".to_string()]);
    });
}

/// machines.ts:~372: every row of the workspace ends with the reason; another workspace's row does not. Waits on packet 4.
#[test]
fn end_sessions_ends_the_rows_of_the_workspace() {
    drive(|f| async move {
        let ended: Rc<RefCell<Vec<String>>> = Rc::default();
        let (mine, other) = (session_row("s_1", "t_1"), session_row("s_2", "t_2"));
        other.borrow_mut().view.workspace_id = "w_other".into();
        for (row, id) in [(&mine, "s_1"), (&other, "s_2")] {
            let seen = ended.clone();
            row.borrow_mut().end = Some(Rc::new(move |reason: &str, _, _| seen.borrow_mut().push(format!("{id}: {reason}"))));
        }
        BootRuntime::sessions(&*f.c).borrow_mut().insert("s_1".into(), mine);
        BootRuntime::sessions(&*f.c).borrow_mut().insert("s_2".into(), other);
        phases(&f.c).end_sessions("w_1", "paused");
        assert_eq!(*ended.borrow(), vec!["s_1: paused".to_string()]);
    });
}

/// folders.ts:240: the checkpoints a thread holds are dropped best effort: the call always answers. Waits on packet 8.
#[test]
fn drop_checkpoints_never_refuses() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        phases(&f.c).drop_checkpoints(&e, "t_1").await.expect("answered");
    });
}

/// folders.ts removeWorktree with `ending`: a record that is no worktree has none to remove. Waits on packet 8.
#[test]
fn remove_worktree_ending_leaves_a_plain_folder_alone() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        phases(&f.c).remove_worktree_ending(&e).await.expect("nothing to remove");
        assert_eq!(e.borrow().record.borrow().phase, WorkspacePhase::Running);
    });
}

/// agents.ts:408: no thread named, no file dropped. Waits on packet 11.
#[test]
fn drop_thread_files_with_no_thread_drops_nothing() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        phases(&f.c).drop_thread_files(&e, vec![]).await.expect("answered");
    });
}

/// kindWords(kind).driven: a cloud kind is driven by wsp.
#[test]
fn driven_reads_the_kinds_words() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(phases(&f.c).driven(&e));
        let local = entry(&f, json!({ "kind": "local" })).await;
        assert!(!phases(&f.c).driven(&local));
    });
}

/// workspaces.ts:594: the machine is killed at the provider.
#[test]
fn machine_kill_kills_the_machine() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        phases(&f.c).machine_kill(&e).await.expect("killed");
        assert_eq!(held(&e).state().await.expect("a state"), MachineState::Gone);
    });
}

/// machines.ts:526: the record leaves the live map. Waits on packet 4.
#[test]
fn drop_workspace_takes_the_entry_out_of_the_live_map() {
    drive(|f| async move {
        let _e = attached(&f, json!({})).await;
        assert!(BootRuntime::live(&*f.c).borrow().contains_key("w_1"));
        phases(&f.c).drop_workspace("w_1").await.expect("dropped");
        assert!(!BootRuntime::live(&*f.c).borrow().contains_key("w_1"));
    });
}

/// workspaces.ts:618: `readGone(backendFor(record), machine.id)`: the provider's word for the machine.
#[test]
fn read_gone_reads_the_state_by_id() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert_eq!(phases(&f.c).read_gone(&e).await.expect("a read"), "running");
        held(&e).kill().await.expect("killed");
        assert_eq!(phases(&f.c).read_gone(&e).await.expect("a read"), "gone");
    });
}

/// machines.ts: touching a workspace opens its idle window, which a running status then names. Waits on packet 4.
#[test]
fn idle_touch_opens_the_idle_window() {
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |o| o.idle_default_window_ms = Some(60_000));
    drive_over(f, |f| async move {
        let e = attached(&f, json!({})).await;
        phases(&f.c).idle_touch("w_1");
        let (seen, _off) = heard(&f, "workspace.status");
        phases(&f.c).push_status(&e).await.expect("said");
        assert_eq!(seen.borrow()[0]["status"]["idleAt"], json!(1_060_000));
    });
}

/// workspaces.ts:556, 633: a cloud kind has no restart and no shared daemon; this computer's kind has a shared one.
#[test]
fn daemon_hooks_read_the_kind_module() {
    drive(|f| async move {
        let cloud = entry(&f, json!({})).await;
        assert!(!phases(&f.c).has_restart_daemon(&cloud));
        assert!(!phases(&f.c).shared_daemon(&cloud));
    });
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |o| {
        let mut local = local_wiring();
        local.restart_daemon = Some(Rc::new(|| Box::pin(async { Ok(()) })));
        o.local = Some(local);
    });
    drive_over(f, |f| async move {
        let local = entry(&f, json!({ "kind": "local" })).await;
        assert!(phases(&f.c).has_restart_daemon(&local));
        assert!(phases(&f.c).shared_daemon(&local));
        let said = phases(&f.c).restart_daemon_of(&local).await.expect_err("refused");
        assert_eq!(said.to_string(), "this host cannot restart its own daemon from here yet");
    });
}

/// channels.ts: the replaced shared daemon closes what the old one held; with none held it is a no-op.
#[test]
fn shared_daemon_replaced_with_nothing_open_is_a_no_op() {
    drive(|f| async move {
        phases(&f.c).shared_daemon_replaced();
    });
}

/// reach.ts: forgetting a poll that was never made leaves the status as the kind reads it. Waits on packet 4.
#[test]
fn forget_polled_reach_forgets_the_poll() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let before = phases(&f.c).reach_of(&e);
        phases(&f.c).forget_polled_reach("w_1");
        assert_eq!(phases(&f.c).reach_of(&e), before);
    });
}

/// reach.ts:376: a running workspace's status is pushed; a napping one's is not. Waits on packet 4.
#[test]
fn push_status_pushes_a_running_workspaces_status() {
    drive(|f| async move {
        let running = attached(&f, json!({})).await;
        let (seen, _off) = heard(&f, "workspace.status");
        phases(&f.c).push_status(&running).await.expect("pushed");
        assert_eq!(seen.borrow().len(), 1);
        let napping = entry(&f, json!({ "id": "w_2", "phase": "napping" })).await;
        phases(&f.c).push_status(&napping).await.expect("answered");
        assert_eq!(seen.borrow().len(), 1);
    });
}

/// create.ts:176: a create that never failed has none to forget. Waits on packet 6.
#[test]
fn forget_failed_create_is_false_for_a_create_that_did_not_fail() {
    drive(|f| async move {
        assert!(!phases(&f.c).forget_failed_create("w_none"));
    });
}

fn local_wiring() -> wsp_core::types::wiring::LocalWiring {
    let folder = std::env::temp_dir().to_string_lossy().into_owned();
    let home = folder.clone();
    wsp_core::types::wiring::LocalWiring {
        backend: Arc::new(wsp_engine::local_backend::LocalBackend::new(wsp_engine::local_backend::LocalBackendOptions {
            root: PathBuf::from(&folder),
            env: None,
        })),
        folder: folder.clone(),
        exec_stream: Rc::new(|_, _| panic!("no exec stream in this test")),
        home: Rc::new(move |agent| format!("{home}/.{agent}")),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(|| indexmap::IndexMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())])),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: wsp_core::types::wiring::Platform::Linux,
        close: None,
    }
}

fn project(id: &str, name: &str, path: &str) -> wsp_wire::views::workspace::ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": name, "path": path, "computer": "zingzys-mac", "source": { "kind": "folder", "path": path }, "remote": "",
        "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-26T00:00:00.000Z",
    }))
    .expect("a project")
}

fn hold(f: &Fx, p: wsp_wire::views::workspace::ProjectView) {
    BootRuntime::projects_held(&*f.c).borrow_mut().insert(p.id.clone(), p);
}

/// folders.ts:35: a record that is no worktree has no overlays to mount again. Waits on packet 8.
#[test]
fn worktree_mounted_leaves_a_plain_folder_alone() {
    drive(|f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        let e = entry(&f, json!({})).await;
        git(&f.c).worktree_mounted(&e).await.expect("nothing to mount");
    });
}

/// workspaces.ts:651: `entry.machine.exec(cmd, o)` on a machine with a guest runs the command there.
#[test]
fn machine_exec_runs_the_command_on_the_machine() {
    let root = std::env::temp_dir().join(format!("ctx-phases-guest-{}", std::process::id()));
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions { guest: Some(Arc::new(Folder(root))), ..Default::default() })), |_| {});
    drive_over(f, |f| async move {
        let e = entry(&f, json!({})).await;
        let ran = git(&f.c).machine_exec(&e, "echo hi", Some(5_000)).await.expect("ran");
        assert_eq!((ran.exit_code.get(), ran.stdout.as_str()), (0, "hi\n"));
    });
}

/// workspaces.ts:651: a machine with no guest refuses the command in the machine's own words.
#[test]
fn machine_exec_says_what_the_machine_says() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        let said = git(&f.c).machine_exec(&e, "echo hi", None).await.expect_err("refused");
        assert_eq!(said.to_string(), wsp_engine::fake_backend::FAKE_NO_GUEST);
    });
}

/// What a launch was handed, and how the run ended.
#[derive(Default)]
struct Launched {
    command: RefCell<Option<String>>,
    env: RefCell<Option<wsp_adapters::port::Env>>,
    killed: std::cell::Cell<bool>,
}

struct Scripted {
    seen: Rc<Launched>,
    lines: Vec<String>,
    stays_open: bool,
}

impl wsp_adapters::port::ExecStreamFactory for Scripted {
    fn launch(&self, command: &str, options: wsp_adapters::port::ExecOptions) -> Arc<dyn wsp_adapters::port::ExecStream> {
        *self.seen.command.borrow_mut() = Some(command.to_string());
        *self.seen.env.borrow_mut() = Some(options.env);
        Arc::new(ScriptedRun { seen: self.seen.clone(), lines: self.lines.clone(), stays_open: self.stays_open })
    }
}

struct ScriptedRun {
    seen: Rc<Launched>,
    lines: Vec<String>,
    stays_open: bool,
}

impl wsp_adapters::port::ExecStream for ScriptedRun {
    fn lines(&self) -> futures::stream::LocalBoxStream<'static, Result<String, wsp_adapters::port::ExecFailure>> {
        let held = futures::stream::iter(self.lines.clone().into_iter().map(Ok));
        if self.stays_open {
            held.chain(futures::stream::pending()).boxed_local()
        } else {
            held.boxed_local()
        }
    }
    fn teardown(&self) {}
    fn kill(&self) {
        self.seen.killed.set(true);
    }
    fn write(
        &self,
        _line: String,
    ) -> futures::future::LocalBoxFuture<'static, Result<wsp_adapters::port::WriteOutcome, wsp_adapters::port::ExecFailure>> {
        Box::pin(async { Ok(wsp_adapters::port::WriteOutcome::Gone) })
    }
    fn close_input(&self) {}
    fn exited(&self) -> wsp_adapters::port::SharedFuture<Option<i32>> {
        let stays_open = self.stays_open;
        futures::FutureExt::shared(Box::pin(async move {
            if stays_open {
                futures::future::pending::<Option<i32>>().await
            } else {
                Some(0)
            }
        }) as futures::future::LocalBoxFuture<'static, Option<i32>>)
    }
}

struct Drafting;

impl wsp_adapters::port::HarnessAdapter for Drafting {
    fn start(
        &self,
        _options: wsp_adapters::port::HarnessStartOptions,
    ) -> Result<Arc<dyn wsp_adapters::port::HarnessSession>, wsp_adapters::port::AdapterError> {
        Err(wsp_adapters::port::AdapterError("no turn in this test".into()))
    }
    fn steers(&self) -> bool {
        false
    }
    fn env(&self) -> Option<wsp_adapters::port::Env> {
        Some(wsp_adapters::port::Env::from([("WSP_PROBE".to_string(), "1".to_string())]))
    }
}

fn claude_adapters() -> indexmap::IndexMap<String, wsp_core::types::harness::HarnessAdapterFactory> {
    let factory: wsp_core::types::harness::HarnessAdapterFactory =
        Arc::new(|_| Ok(Arc::new(Drafting) as Arc<dyn wsp_adapters::port::HarnessAdapter>));
    indexmap::IndexMap::from([("claude".to_string(), factory)])
}

/// A local copy over this computer's folder, whose exec stream is the scripted one.
fn streaming(seen: Rc<Launched>, lines: &[&str], stays_open: bool) -> Setup {
    let lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), move |o| {
        let mut local = local_wiring();
        local.exec_stream = Rc::new(move |_, _| Arc::new(Scripted { seen: seen.clone(), lines: lines.clone(), stays_open }));
        o.local = Some(local);
        o.adapters = claude_adapters();
    })
}

/// workspaces.ts:658: the command runs in the folder asked, quoted word by word, with the adapter's env, over the
/// kind's unbounded stream; its lines and its exit pass through. Waits on packets 4 and 11.
#[test]
fn exec_stream_of_runs_the_argv_in_the_folder_with_the_adapters_env() {
    let seen: Rc<Launched> = Rc::default();
    let f = streaming(seen.clone(), &["one", "two"], false);
    drive_over(f, |f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        let e = entry(&f, json!({ "kind": "local" })).await;
        let run = git(&f.c).exec_stream_of(&e, vec!["echo".into(), "a b".into()], Some("/work/api".into())).await.expect("a stream");
        assert_eq!(run.ran_in.as_deref(), Some("/work/api"));
        assert_eq!(seen.command.borrow().as_deref(), Some("cd '/work/api' && 'echo' 'a b'"));
        assert_eq!(seen.env.borrow().as_ref().and_then(|e| e.get("WSP_PROBE")).map(String::as_str), Some("1"));
        let read: Vec<String> = run.stream.lines().map(|l| l.expect("a line")).collect().await;
        assert_eq!(read, vec!["one".to_string(), "two".to_string()]);
        assert_eq!(run.stream.exited().await, Some(0));
    });
}

/// workspaces.ts:671-690: a machine going away ends the stream with its reason as the failure, ahead of the inner
/// poll, kills the run, and settles `exited` with no code. Waits on packets 4 and 11.
#[test]
fn exec_stream_of_ends_with_the_reason_when_the_machine_goes_away() {
    let seen: Rc<Launched> = Rc::default();
    let f = streaming(seen.clone(), &["one"], true);
    drive_over(f, |f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        let e = entry(&f, json!({ "kind": "local" })).await;
        let run = git(&f.c).exec_stream_of(&e, vec!["sleep".into(), "9".into()], None).await.expect("a stream");
        let mut lines = run.stream.lines();
        assert_eq!(lines.next().await.expect("a line").expect("ok"), "one");
        wsp_machines::machines::machines::MachinesRuntime::end_execs(&*f.c, "w_1", "the machine went away");
        let ended = lines.next().await.expect("the failure").expect_err("ended");
        assert_eq!(ended.message, "the machine went away");
        assert!(lines.next().await.is_none());
        assert!(seen.killed.get());
        assert_eq!(run.stream.exited().await, None);
    });
}

/// folders.ts:112: git writes on one folder run one at a time and each caller gets its own answer.
#[test]
fn queued_answers_each_caller_its_own_result() {
    drive(|f| async move {
        let first = git(&f.c).queued("w_1", Box::pin(async { Ok(json!(1)) }));
        let second = git(&f.c).queued("w_1", Box::pin(async { Ok(json!(2)) }));
        assert_eq!(second.await.expect("second"), json!(2));
        assert_eq!(first.await.expect("first"), json!(1));
    });
}

/// pull-requests.ts keepTree: a tree record is kept on the copy's record and persisted. Waits on packet 7.
#[test]
fn keep_tree_keeps_the_record_on_the_copy() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let tree: wsp_wire::types::tree::TreeRecord = serde_json::from_value(json!({ "conflicts": ["a.ts"] })).expect("a tree");
        git(&f.c).keep_tree(&e, tree.clone()).await.expect("kept");
        assert_eq!(e.borrow().record.borrow().tree, Some(tree));
    });
}

/// pull-requests.ts takePullRequest: none clears the fact the entry holds. Waits on packet 7.
#[test]
fn take_pull_request_with_none_clears_the_fact() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        git(&f.c).take_pull_request(&e, Value::Null).await.expect("taken");
        assert!(e.borrow().pr.is_none());
    });
}

/// reach.ts:471: a copy that names no lead has none to read. Waits on packet 4.
#[test]
fn read_lead_of_with_no_lead_does_nothing() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        let (seen, _off) = heard(&f, "workspace.status");
        git(&f.c).read_lead_of(&e);
        tokio::task::yield_now().await;
        assert!(seen.borrow().is_empty());
    });
}

/// folders.ts folderFor: a project this host does not hold is refused. Waits on packets 8 and 10.
#[test]
fn folder_for_refuses_a_project_nobody_holds() {
    drive(|f| async move {
        let ask = wsp_core::types::api::FolderForAsk { project: Some("nope".into()), ..Default::default() };
        assert!(git(&f.c).folder_for(ask, None).await.is_err());
    });
}

/// folders.ts:310: a setup line is answered once and forgotten; none was left.
#[test]
fn take_setup_line_answers_none_where_none_was_left() {
    drive(|f| async move {
        assert_eq!(git(&f.c).take_setup_line("w_1"), None);
    });
}

/// projects.ts resolve: a held project is found by its name. Waits on packet 10.
#[test]
fn resolve_project_finds_a_held_project_by_name() {
    drive(|f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        assert_eq!(git(&f.c).resolve_project("api", None).await.expect("found").id, "p_1");
    });
}

/// rules.ts:220: a relayed caller may not record; a local one may.
#[test]
fn refuse_recording_refuses_a_relayed_caller() {
    drive(|f| async move {
        let relayed = Caller::Road(wsp_wire::views::workspace_more::WorkspaceOrigin::Relayed);
        let said = git(&f.c).refuse_recording("api", Some(&relayed)).expect_err("refused");
        assert_eq!(said.to_string(), wsp_words::words::computer::relayed_record_refusal("api"));
        assert!(git(&f.c).refuse_recording("api", None).is_ok());
    });
}

/// rules.ts refuseRelayed: a caller that names no road is let through.
#[test]
fn refuse_relayed_lets_a_local_caller_through() {
    drive(|f| async move {
        assert!(git(&f.c).refuse_relayed(&record(json!({})), None).is_ok());
    });
}

/// folders.ts:63: the project folder answers the folder already made for the project. Waits on packet 6.
#[test]
fn project_folder_answers_the_folder_that_stands() {
    drive(|f| async move {
        let held = attached(&f, json!({ "kind": "local" })).await;
        let got = git(&f.c).project_folder(&project("p_1", "api", "/work/api")).await.expect("a folder");
        assert!(Rc::ptr_eq(&got, &held));
    });
}

/// folders.ts:69: a worktree needs this computer's copier; a host that wires none says so. Waits on packet 8.
#[test]
fn worktree_folder_refuses_where_no_copier_is_wired() {
    drive(|f| async move {
        let said =
            git(&f.c).worktree_folder(&project("p_1", "api", "/work/api"), "/work/api", "fix/cart", None).await.err().expect("refused");
        assert_eq!(said.to_string(), wsp_core::types::wiring::NO_COPIER_HERE);
    });
}

/// folders.ts:592: a folder git knows no worktrees of lists none. Waits on packet 8.
#[test]
fn worktrees_of_lists_the_worktrees_git_knows() {
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |o| o.local = Some(local_wiring()));
    drive_over(f, |f| async move {
        let dir = git_dir("trees");
        let listed = git(&f.c).worktrees_of(&dir.to_string_lossy()).await.expect("listed");
        assert_eq!(listed.len(), 1);
        assert_eq!(std::path::Path::new(&listed[0].path).file_name(), dir.file_name());
        let _ = std::fs::remove_dir_all(&dir);
    });
}

/// create.ts:783: the folders of a project are the live entries naming it. Waits on packet 6.
#[test]
fn folders_of_lists_the_live_entries_of_the_project() {
    drive(|f| async move {
        let held = attached(&f, json!({})).await;
        let of = git(&f.c).folders_of("p_1");
        assert_eq!(of.len(), 1);
        assert!(Rc::ptr_eq(&of[0], &held));
        assert!(git(&f.c).folders_of("p_other").is_empty());
    });
}

/// folders.ts removeWorktree: a record that is no worktree is refused as one. Waits on packet 8.
#[test]
fn remove_worktree_refuses_a_plain_folder() {
    drive(|f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        let e = entry(&f, json!({})).await;
        let ask = wsp_account::projects::folders::tree::RemoveAsk::default();
        assert!(git(&f.c).remove_worktree(&e, false, ask).await.is_err());
    });
}

/// reach.ts:491: a checkout read asked fresh goes to the copy's daemon; with none to answer it reads none. Waits on 4.
#[test]
fn read_checkout_reads_none_where_the_daemon_does_not_answer() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        assert_eq!(git(&f.c).read_checkout(&e, true).await.expect("answered"), None);
    });
}

/// pull-requests.ts:358: a copy whose project has no remote reads no pull request. Waits on packets 7 and 8.
#[test]
fn read_pull_request_reads_none_where_there_is_no_remote() {
    drive(|f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        let e = attached(&f, json!({})).await;
        assert_eq!(git(&f.c).read_pull_request(&e, true, false).await.expect("answered"), None);
    });
}

/// reach.ts:581: a tree read of a copy that is not a lead settles and says nothing. Waits on packet 4.
#[test]
fn read_tree_answers_for_a_copy_with_no_children() {
    drive(|f| async move {
        let e = attached(&f, json!({})).await;
        git(&f.c).read_tree(&e).await.expect("read");
    });
}

/// types/internal.ts labelOf: the workspace's name, or the thread's short word where one is named.
#[test]
fn label_of_names_the_workspace_or_the_thread() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert_eq!(git(&f.c).label_of(&e, None), "api");
        assert_eq!(
            git(&f.c).label_of(&e, Some("5c2d0e14-0000-4000-8000-000000000000")),
            wsp_core::types::internal::label_of("api", Some("5c2d0e14-0000-4000-8000-000000000000"))
        );
    });
}

/// transcripts.ts:570: a thread with no transcript opens no events, on a store that keeps rows.
#[test]
fn open_thread_reads_no_events_for_an_unknown_thread() {
    let dir = tempfile::tempdir().expect("a folder");
    let sqlite = Rc::new(wsp_store::sqlite_store::sqlite_store(
        &dir.path().join("state.json"),
        wsp_store::store::StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ));
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), move |o| o.store = Some(sqlite));
    drive_over(f, |f| async move {
        assert!(git(&f.c).open_thread("w_1", "t_none").await.expect("answered").is_empty());
    });
}

/// agents.ts:291, 160, runtime.ts: the host holds an adapter for the agents it was given; the first it runs is the default, and
/// none is off until the workspace switches it. Waits on packet 11.
#[test]
fn the_agent_reads_come_from_the_adapters_the_host_holds() {
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), |o| o.adapters = claude_adapters());
    drive_over(f, |f| async move {
        let e = entry(&f, json!({})).await;
        assert!(git(&f.c).has_adapter("claude") && !git(&f.c).has_adapter("codex"));
        assert_eq!(git(&f.c).default_agent_of(&e), "claude");
        assert!(!git(&f.c).agent_off(&e, "claude"));
    });
}

/// agents.ts:281: an agent whose adapter this host holds drafts where it declares a draft; a host with none holds no
/// drafter. Waits on packet 11.
#[test]
fn drafter_is_none_where_the_host_holds_no_adapter() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(git(&f.c).drafter(&e, "claude").await.is_err());
    });
}

/// engine put-files: the file lands on the machine through a command that carries it in base64. The stand-in guest runs it.
#[test]
fn put_files_lands_the_file_on_the_machine() {
    let root = std::env::temp_dir().join(format!("ctx-phases-put-{}", std::process::id()));
    let f = fx_over(
        Arc::new(FakeBackend::new(FakeBackendOptions { guest: Some(Arc::new(Folder(root.clone()))), ..Default::default() })),
        |_| {},
    );
    drive_over(f, |f| async move {
        let e = entry(&f, json!({})).await;
        let id = e.borrow().machine.id().to_string();
        let put = git(&f.c).put_files(&e, "draft.txt", "the question", &[]).await.expect("put");
        assert_eq!(put.exit_code.get(), 0);
        assert_eq!(std::fs::read_to_string(root.join(&id).join("draft.txt")).expect("the file"), "the question");
        let _ = std::fs::remove_dir_all(&root);
    });
}

/// agents.ts:281: a draft asked of an agent this host holds no adapter for is refused. Waits on packet 11.
#[test]
fn draft_for_refuses_an_agent_the_host_does_not_hold() {
    drive(|f| async move {
        let e = entry(&f, json!({})).await;
        assert!(git(&f.c).draft_for(&e, "claude", "draft.txt", None).await.is_err());
    });
}

/// transcripts.ts:106: the viewed marks are the transcripts area's one record per workspace, shared.
#[test]
fn viewed_marks_is_the_transcripts_record() {
    drive(|f| async move {
        git(&f.c).viewed_marks().borrow_mut().insert("w_1".into(), indexmap::IndexMap::from([("a.ts".to_string(), "b1".to_string())]));
        assert_eq!(git(&f.c).viewed_marks().borrow().get("w_1").and_then(|m| m.get("a.ts")).map(String::as_str), Some("b1"));
    });
}

/// transcripts.ts:617: a workspace with no session rows persists an empty document. Waits on packet 13.
#[test]
fn persist_sessions_answers_for_a_workspace_with_no_rows() {
    drive(|f| async move {
        git(&f.c).persist_sessions("w_1").await.expect("persisted");
    });
}

/// pull-requests.ts:243: a workspace this host does not hold is refused. Waits on packets 4 and 7.
#[test]
fn pull_request_on_refuses_a_workspace_nobody_holds() {
    drive(|f| async move {
        assert!(git(&f.c).pull_request_on("w_none", None).await.is_err());
    });
}

/// pull-requests.ts:264, 778, 157, 315: the page, the settings, the host read and the post all need a daemon of this
/// computer or the copy's to answer; with none each refuses. Waits on packet 7.
#[test]
fn the_pull_request_reads_refuse_where_there_is_no_remote() {
    drive(|f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        let e = attached(&f, json!({})).await;
        assert!(git(&f.c).read_page(&e, "", 1, true).await.is_err());
        assert!(git(&f.c).merge_settings("").await.is_err());
        let frame: wsp_account::account::pull_requests::FrameFor =
            Rc::new(|cwd: &str| serde_json::from_value(json!({ "op": "git.prView", "cwd": cwd })).expect("a frame"));
        assert!(git(&f.c).read_host(&e, frame).await.is_err());
        assert!(git(&f.c).post_as_person("", 1, json!({})).await.is_err());
    });
}

/// daemon.ts:319, 216: a send to a workspace this host does not hold is refused.
#[test]
fn the_detached_sends_refuse_a_workspace_nobody_holds() {
    drive(|f| async move {
        assert!(git(&f.c).to_first_thread("w_none", "hello", None).await.is_err());
        assert!(git(&f.c).send_detached("w_none", "hello", "t_1", None).await.is_err());
    });
}

/// boot.ts:94: a child named that nobody holds is refused. Waits on packet 5.
#[test]
fn child_of_refuses_a_child_nobody_holds() {
    drive(|f| async move {
        let lead = attached(&f, json!({})).await;
        assert!(git(&f.c).child_of(&lead, "w_none", None, None, None).await.is_err());
    });
}

/// pull-requests.ts:809: a copy with no pull request has nothing to update. Waits on packet 7.
#[test]
fn update_copy_refuses_a_copy_with_no_pull_request() {
    drive(|f| async move {
        hold(&f, project("p_1", "api", "/work/api"));
        let e = attached(&f, json!({})).await;
        assert!(git(&f.c).update_copy(&e).await.is_err());
    });
}

/// workspaces.ts:1003: the page the cache holds under the remote and number is dropped; the pull request's own stays.
#[test]
fn drop_page_cache_drops_the_page_the_cache_holds() {
    let cache = wsp_core::github_cache::memory_github_cache();
    let held = cache.clone();
    let f = fx_over(Arc::new(FakeBackend::new(FakeBackendOptions::default())), move |o| o.github_cache = Some(held));
    drive_over(f, |f| async move {
        let entry = || wsp_core::github_cache::GitHubCacheEntry { body: json!({}), tag: None, fetched_at: 1 };
        cache.set("page:owner/repo#7", entry());
        cache.set("pr:owner/repo#7", entry());
        git(&f.c).drop_page_cache("owner/repo", 7);
        assert!(cache.get("page:owner/repo#7").is_none());
        assert!(cache.get("pr:owner/repo#7").is_some());
    });
}

/// @wsp/protocol isLocalWorkspace: a local record is this computer's own.
#[test]
fn is_local_workspace_reads_the_kind() {
    drive(|f| async move {
        assert!(git(&f.c).is_local_workspace(&record(json!({ "kind": "local" }))));
        assert!(!git(&f.c).is_local_workspace(&record(json!({}))));
    });
}

/// A TS `void f()`: the future runs now, with nothing awaiting it.
#[test]
fn spawn_runs_the_future_without_a_waiter() {
    drive(|f| async move {
        let ran = Rc::new(std::cell::Cell::new(false));
        let flag = ran.clone();
        git(&f.c).spawn(Box::pin(async move { flag.set(true) }));
        tokio::task::yield_now().await;
        assert!(ran.get());
    });
}
