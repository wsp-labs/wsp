// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/box-nap.test.ts at 65f6b89c7, second part: "a wake that finds a Boat box already running", case for
//! case (the first part is `whole_box_nap_1.rs`). The box is the real Boat machine over a gateway that refuses commands
//! while its clock runs (`common/box_fake.rs`).
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use common::box_fake::{restoring_box, RestoringBox};
use common::gone::until_n;
use common::*;
use futures::future::{FutureExt, LocalBoxFuture};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use wsp_adapters::port::{AdapterError, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, SteerOutcome, TurnImage, TurnResult};
use wsp_core::console::capture;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::api::SteerAsk;
use wsp_engine::box_backend::plan::BOX_WORK_DIR;
use wsp_engine::gone::KillConfirm;
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_words::words::computer::{no_commands_yet_line, WAKE_STOPPED_UP};
use wsp_words::workspace_state::{send_refusal, workspace_state, SendRefusalKind, WorkspaceStateInput};

/// A harness whose turn runs until the test ends; each launch notes whether the box had taken a command by then.
struct Launching {
    proven: Rc<RefCell<Vec<bool>>>,
    state: Rc<common::box_fake::BoxState>,
}

struct Running(String, SharedFuture<TurnResult>);

impl HarnessSession for Running {
    fn local_id(&self) -> String {
        self.0.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.1.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(
        &self,
        _prompt: String,
        _id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        Some(async { Ok(SteerOutcome::Accepted) }.boxed_local())
    }
}

impl HarnessAdapter for Launching {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.proven.borrow_mut().push(self.state.last_command_restoring() == Some(false));
        let on_event = o.on_event.clone().expect("an event sink");
        on_event(common::adapter::event(json!({ "type": "session.start", "sessionId": "c1" })));
        let never: LocalBoxFuture<'static, TurnResult> = futures::future::pending().boxed_local();
        Ok(Arc::new(Running("s1".into(), never.shared())))
    }
    fn steers(&self) -> bool {
        true
    }
}

/// A workspace whose host restarted with its record left `left`, and whose box the new host reaches through the provider's
/// API: archived until `ready` is called, or ready from the start.
struct Restarted {
    _fx: Fx,
    rt: Rc<AssembledRuntime>,
    ws_id: String,
    machine_id: String,
    b: RestoringBox,
    state: State,
    backend: Arc<StubBackend>,
    proven: Rc<RefCell<Vec<bool>>>,
    box_state: Rc<RefCell<String>>,
    events: Events,
}

async fn restarted(left: &str, restored_at: f64) -> Restarted {
    restarted_over(left, restored_at, None).await
}

async fn restarted_over(left: &str, restored_at: f64, wrap: Option<&dyn Fn(&Rc<dyn Store>) -> Rc<dyn Store>>) -> Restarted {
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let first = runtime(&backend).store(&state.store).build();
    let ws = create_on(&first.rt, CreateOn::golden("x"), None).await.expect("a workspace");
    let machine_id = j(&ws.view)["machineId"].as_str().expect("a machine").to_string();
    first.rt.close().await.expect("a close");
    let mut held = state.store.get("workspaces", &ws.view.id).await.expect("a read").expect("a record");
    held["phase"] = json!(left);
    state.store.put("workspaces", &ws.view.id, held).await.expect("a write");
    let box_state = Rc::new(RefCell::new(if left == "napping" { "archived" } else { "ready" }.to_string()));
    let (reads, resumed) = (box_state.clone(), box_state.clone());
    let b = restoring_box(&machine_id, move || reads.borrow().clone(), restored_at, move || *resumed.borrow_mut() = "ready".into());
    backend.answer_get_with(&machine_id, b.machine.clone());
    let proven: Rc<RefCell<Vec<bool>>> = Rc::default();
    let (record, seen) = (b.state.clone(), proven.clone());
    let launching: HarnessAdapterFactory =
        Arc::new(move |_| Ok(Arc::new(Launching { proven: seen.clone(), state: record.clone() }) as Arc<dyn HarnessAdapter>));
    let store = match wrap {
        Some(wrap) => wrap(&state.store),
        None => state.store.clone(),
    };
    let fx = host(&backend, &store, launching.clone());
    let events = fx.events();
    let rt = fx.rt.clone();
    let _ = launching;
    Restarted { rt, _fx: fx, ws_id: ws.view.id, machine_id, b, state, backend, proven, box_state, events }
}

fn host(backend: &Arc<StubBackend>, store: &Rc<dyn Store>, adapter: HarnessAdapterFactory) -> Fx {
    runtime(backend)
        .store(store)
        .adapter("claude", adapter)
        .with(|o| {
            o.gone_confirm_ms = Some(0);
            o.kill_confirm = Some(KillConfirm { grace_ms: Some(0), poll_ms: Some(0) });
        })
        .build()
}

impl Restarted {
    fn ready(&self) {
        *self.box_state.borrow_mut() = "ready".into();
    }
    /// The phases pushed for this workspace since the host started.
    fn phases(&self) -> Vec<String> {
        self.events
            .statuses()
            .iter()
            .filter(|s| s["id"] == self.ws_id.as_str())
            .map(|s| s["phase"].as_str().unwrap_or_default().to_string())
            .collect()
    }
    fn next_host(&self) -> Fx {
        let factory: HarnessAdapterFactory = {
            let (record, seen) = (self.b.state.clone(), self.proven.clone());
            Arc::new(move |_| Ok(Arc::new(Launching { proven: seen.clone(), state: record.clone() }) as Arc<dyn HarnessAdapter>))
        };
        host(&self.backend, &self.state.store, factory)
    }
    fn waking(&self) -> JoinHandle<Result<Value, String>> {
        let (rt, id) = (self.rt.clone(), self.ws_id.clone());
        tokio::task::spawn_local(async move { rt.workspaces().wake(&id, None).await.map(|v| j(&v)).map_err(|e| e.message) })
    }
    async fn phase(&self) -> String {
        j(&self.rt.workspaces().get(&self.ws_id, None, None).await.expect("the workspace"))["phase"].as_str().unwrap_or_default().to_string()
    }
    async fn stored(&self) -> Option<Value> {
        self.state.store.get("workspaces", &self.ws_id).await.expect("a read")
    }
    async fn listed_by_a_new_host(&self) -> Vec<String> {
        let next = self.next_host();
        let listed = next.rt.workspaces().list(None).await.expect("a list");
        let ids = listed.iter().map(|w| w.id.clone()).collect();
        next.rt.close().await.expect("a close");
        ids
    }
}

const ROADS: [(&str, &str); 2] = [
    ("a napping record whose box the wake finds running", "napping"),
    ("a record a restart found running over a box that was waking", "waking"),
];

async fn the_wake_waits_reading_waking(left: &str) {
    let t = restarted(left, 30.0).await;
    t.ready();
    let woken = t.rt.workspaces().wake(&t.ws_id, None).await.expect("a wake");
    assert_eq!(j(&woken)["phase"], "running");
    // The box answered sandbox_restoring while the wake waited, and took a command before the wake answered.
    assert_eq!(t.b.state.last_command_restoring(), Some(false));
    assert!(t.b.state.commands.borrow().contains(&true));
    assert!(t.phases().contains(&"waking".to_string()), "{:?}", t.phases());
    assert_eq!(t.phases().last().map(String::as_str), Some("running"));
    t.rt.close().await.expect("a close");
}

async fn a_box_still_restoring_at_the_deadline(left: &str) {
    let t = restarted(left, 1_000.0).await;
    t.ready();
    let refused = t.rt.workspaces().wake(&t.ws_id, None).await.expect_err("a refusal");
    assert!(refused.message.contains(&no_commands_yet_line("x", "Box is restoring.")), "{}", refused.message);
    assert_eq!(t.phase().await, "running");
    t.b.state.at.set(1_000);
    assert_eq!(j(&t.rt.workspaces().wake(&t.ws_id, None).await.expect("a wake"))["phase"], "running");
    assert_eq!(t.b.state.last_command_restoring(), Some(false));
    t.rt.close().await.expect("a close");
}

// "a wake that finds a Boat box already running" > "a napping record whose box the wake finds running: the wake waits, reading waking, until the box takes commands"
#[test]
fn a_napping_record_whose_box_the_wake_finds_running_the_wake_waits_reading_waking_until_the_box_takes_commands() {
    drive(|| the_wake_waits_reading_waking(ROADS[0].1));
}

// "a napping record whose box the wake finds running: a box still restoring at the deadline fails the wake with a line that says so, and the next wake waits again"
#[test]
fn a_napping_record_whose_box_the_wake_finds_running_a_box_still_restoring_at_the_deadline_fails_the_wake_with_a_line_that_says_so_and_the_next_wake_waits_again() {
    drive(|| a_box_still_restoring_at_the_deadline(ROADS[0].1));
}

// "a record a restart found running over a box that was waking: the wake waits, reading waking, until the box takes commands"
#[test]
fn a_record_a_restart_found_running_over_a_box_that_was_waking_the_wake_waits_reading_waking_until_the_box_takes_commands() {
    drive(|| the_wake_waits_reading_waking(ROADS[1].1));
}

// "a record a restart found running over a box that was waking: a box still restoring at the deadline fails the wake with a line that says so, and the next wake waits again"
#[test]
fn a_record_a_restart_found_running_over_a_box_that_was_waking_a_box_still_restoring_at_the_deadline_fails_the_wake_with_a_line_that_says_so_and_the_next_wake_waits_again() {
    drive(|| a_box_still_restoring_at_the_deadline(ROADS[1].1));
}

// "a delete between two of the proof's asks sticks: nothing is written back, and a host started on the state lists no such workspace"
#[test]
fn a_delete_between_two_of_the_proofs_asks_sticks_nothing_is_written_back_and_a_host_started_on_the_state_lists_no_such_workspace() {
    drive(|| async {
        let t = restarted("napping", f64::INFINITY).await;
        t.ready();
        let release = t.b.state.hold_next();
        let waking = t.waking();
        until_n(|| t.b.state.hold.borrow().is_none()).await;
        t.rt.workspaces().delete(&t.ws_id, None).await.expect("a delete");
        assert!(t.stored().await.is_none());
        release.open();
        assert!(waking.await.expect("a task").is_err());
        assert!(t.stored().await.is_none());
        assert!(!t.listed_by_a_new_host().await.contains(&t.ws_id));
        t.rt.close().await.expect("a close");
    });
}

// "the row's stop ends the proof within a second, leaves the record running, and the next wake proves again"
#[test]
fn the_rows_stop_ends_the_proof_within_a_second_leaves_the_record_running_and_the_next_wake_proves_again() {
    drive(|| async {
        let t = restarted("napping", 0.0).await;
        t.ready();
        let release = t.b.state.hold_next();
        let waking = t.waking();
        until_n(|| t.b.state.hold.borrow().is_none()).await;
        assert_eq!(t.phase().await, "waking");
        let (rt, id) = (t.rt.clone(), t.ws_id.clone());
        let stop = async move { rt.workspaces().stop_wake(&id, None).await.map(|_| "stopped") };
        let answer = tokio::time::timeout(Duration::from_millis(1_000), stop).await;
        release.open();
        assert!(matches!(answer, Ok(Ok("stopped"))), "still waiting after a second");
        let refused = waking.await.expect("a task").expect_err("a refusal");
        assert!(refused.contains(WAKE_STOPPED_UP), "{refused}");
        assert_eq!(t.phase().await, "running");
        let proofs = t.b.machine.proofs.get();
        assert_eq!(j(&t.rt.workspaces().wake(&t.ws_id, None).await.expect("a wake"))["phase"], "running");
        assert_eq!(t.b.machine.proofs.get(), proofs + 1);
        t.rt.close().await.expect("a close");
    });
}

// "a box whose agent fails during the proof fails the wake with that failure, not a line to wait"
#[test]
fn a_box_whose_agent_fails_during_the_proof_fails_the_wake_with_that_failure_not_a_line_to_wait() {
    drive(|| async {
        let t = restarted("napping", 0.0).await;
        t.ready();
        t.b.state.agent_fails.set(true);
        let failed = t.rt.workspaces().wake(&t.ws_id, None).await.expect_err("a failure");
        // The TS reads `instanceof GuestUnusableError`; the host's error carries the class's words alone, and those say the
        // box the agent failed on and what it said.
        assert!(failed.message.contains(&t.machine_id) && failed.message.contains("Cannot read properties of undefined"), "{}", failed.message);
        assert!(!failed.message.contains("wake it again"), "{}", failed.message);
        assert_eq!(t.phase().await, "running");
        t.rt.close().await.expect("a close");
    });
}

fn no_work_dir_line(machine: &str) -> String {
    format!(
        "Boat left {machine} running but nothing on it can run: {BOX_WORK_DIR} is missing, and Boat runs every command there (it said: cwd must be an existing directory.); a wake cannot make it again, so a rebuild is the way out, and work not pushed is lost with the old disk"
    )
}

// "a box whose work folder is gone fails the wake naming the folder and the box, and the rebuild it names replaces the box"
#[test]
fn a_box_whose_work_folder_is_gone_fails_the_wake_naming_the_folder_and_the_box_and_the_rebuild_it_names_replaces_the_box() {
    drive(|| async {
        let t = restarted("napping", 0.0).await;
        t.ready();
        t.b.state.no_work_dir.set(true);
        let _warn = capture();
        let failed = t.rt.workspaces().wake(&t.ws_id, None).await.expect_err("a failure");
        assert_eq!(failed.message, no_work_dir_line(&t.machine_id));
        let rebuilt = j(&t.rt.workspaces().rebuild(&t.ws_id, None).await.expect("a rebuild"));
        assert_eq!(rebuilt["phase"], "running");
        assert_ne!(rebuilt["machineId"], t.machine_id.as_str());
        t.rt.close().await.expect("a close");
    });
}

// "a send to a box whose work folder is gone fails with the same line the wake gives"
#[test]
fn a_send_to_a_box_whose_work_folder_is_gone_fails_with_the_same_line_the_wake_gives() {
    drive(|| async {
        let t = restarted("waking", 0.0).await;
        t.ready();
        t.b.state.no_work_dir.set(true);
        let _warn = capture();
        let failed = t.rt.sessions().start(&t.ws_id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.err().expect("a failure");
        assert_eq!(failed.message, no_work_dir_line(&t.machine_id));
        t.rt.close().await.expect("a close");
    });
}

// "a box the provider no longer has during the proof takes the gone road"
#[test]
fn a_box_the_provider_no_longer_has_during_the_proof_takes_the_gone_road() {
    drive(|| async {
        let t = restarted("napping", 0.0).await;
        t.ready();
        let release = t.b.state.hold_next();
        let waking = t.waking();
        until_n(|| t.b.state.hold.borrow().is_none()).await;
        t.b.state.deleted.set(true);
        release.open();
        let failed = waking.await.expect("a task").expect_err("a failure");
        assert!(!failed.contains("wake it again"), "{failed}");
        assert_eq!(t.phase().await, "gone");
        t.rt.close().await.expect("a close");
    });
}

// "a start from the app on a machine not yet proven proves it first, reading waking, and then launches"
#[test]
fn a_start_from_the_app_on_a_machine_not_yet_proven_proves_it_first_reading_waking_and_then_launches() {
    drive(|| async {
        let t = restarted("waking", 30.0).await;
        t.rt.sessions().start(&t.ws_id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.expect("a start");
        assert_eq!(*t.proven.borrow(), [true]);
        assert!(t.phases().contains(&"waking".to_string()), "{:?}", t.phases());
        assert_eq!(t.phase().await, "running");
        t.rt.close().await.expect("a close");
    });
}

// "two wakes at once share one proof"
#[test]
fn two_wakes_at_once_share_one_proof() {
    drive(|| async {
        let t = restarted("waking", 30.0).await;
        t.rt.workspaces().get(&t.ws_id, None, None).await.expect("the workspace");
        // Both wakes read the machine before either starts its proof, as two callers a moment apart do.
        let arrived = Rc::new(Cell::new(0u32));
        let barrier = Gate::new();
        let (count, gate) = (arrived.clone(), barrier.clone());
        t.b.machine.before_each_state_read(Rc::new(move || {
            count.set(count.get() + 1);
            let n = count.get();
            if n == 2 {
                gate.open();
            }
            let gate = gate.clone();
            Box::pin(async move {
                if n <= 2 {
                    gate.wait().await;
                }
            })
        }));
        let release = t.b.state.hold_next();
        let answered: Rc<RefCell<Vec<String>>> = Rc::default();
        let both: Vec<JoinHandle<Result<Value, String>>> = (0..2)
            .map(|_| {
                let (rt, id, noted) = (t.rt.clone(), t.ws_id.clone(), answered.clone());
                tokio::task::spawn_local(async move {
                    let woke = rt.workspaces().wake(&id, None).await.map(|v| j(&v)).map_err(|e| e.message);
                    if let Ok(view) = &woke {
                        noted.borrow_mut().push(view["phase"].as_str().unwrap_or_default().to_string());
                    }
                    woke
                })
            })
            .collect();
        until_n(|| t.b.state.hold.borrow().is_none()).await;
        sleep_ms(50).await;
        // Neither wake answers while the proof's ask is held.
        assert!(answered.borrow().is_empty());
        release.open();
        let mut phases = vec![];
        for wake in both {
            phases.push(wake.await.expect("a task").expect("a wake")["phase"].as_str().unwrap_or_default().to_string());
        }
        assert_eq!(phases, ["running", "running"]);
        assert_eq!(t.b.machine.proofs.get(), 1);
        t.rt.close().await.expect("a close");
    });
}

// "a wake after a proven launch proves nothing, and a steer is not refused"
#[test]
fn a_wake_after_a_proven_launch_proves_nothing_and_a_steer_is_not_refused() {
    drive(|| async {
        let t = restarted("waking", 0.0).await;
        let handle = t.rt.sessions().start(&t.ws_id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.expect("a start");
        let proofs = t.b.machine.proofs.get();
        let release = t.b.state.hold_next();
        let waking = t.waking();
        sleep_ms(20).await;
        assert_eq!(t.phase().await, "running");
        let steered = t
            .rt
            .sessions()
            .steer(&handle.id(), SteerAsk { prompt: "and this".into(), request_id: None, attachments: None }, None)
            .await
            .expect("a steer");
        assert!(j(&steered)["outcome"].is_string(), "{}", j(&steered));
        release.open();
        let _ = waking.await;
        assert_eq!(t.b.machine.proofs.get(), proofs);
        t.rt.close().await.expect("a close");
    });
}

// "a nap drops the mark: a send on the napping record is refused as paused, runs no proof and leaves it napping"
#[test]
fn a_nap_drops_the_mark_a_send_on_the_napping_record_is_refused_as_paused_runs_no_proof_and_leaves_it_napping() {
    drive(|| async {
        let t = restarted("waking", 0.0).await;
        t.rt.workspaces().nap(&t.ws_id, None).await.expect("a nap");
        assert!(t.b.state.archived.get());
        let proofs = t.b.machine.proofs.get();
        let want = send_refusal(
            &SendRefusalKind::State(workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Napping, machine_state: None, reach: None, wake_refused: None })),
            None,
            Some("x"),
        )
        .expect("a refusal");
        let refused = t.rt.sessions().start(&t.ws_id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.err().expect("a refusal");
        assert!(refused.message.contains(&want), "{}", refused.message);
        assert_eq!(t.b.machine.proofs.get(), proofs);
        assert_eq!(t.phase().await, "napping");
        assert_eq!(t.stored().await.expect("a record")["phase"], "napping");
        t.rt.close().await.expect("a close");
    });
}

// "a delete that ends while the proof's 404 is confirmed sticks: nothing is written back, and a host started on the state lists no such workspace"
#[test]
fn a_delete_that_ends_while_the_proofs_404_is_confirmed_sticks_nothing_is_written_back_and_a_host_started_on_the_state_lists_no_such_workspace() {
    drive(|| async {
        let t = restarted("napping", 0.0).await;
        t.ready();
        let release = t.b.state.hold_next();
        let waking = t.waking();
        until_n(|| t.b.state.hold.borrow().is_none()).await;
        t.b.state.deleted.set(true);
        let let_read = t.b.state.hold_read();
        release.open();
        until_n(|| t.b.state.read_held.get()).await;
        t.rt.workspaces().delete(&t.ws_id, None).await.expect("a delete");
        assert!(t.stored().await.is_none());
        let_read.open();
        assert!(waking.await.expect("a task").is_err());
        assert!(t.stored().await.is_none());
        assert!(!t.listed_by_a_new_host().await.contains(&t.ws_id));
        t.rt.close().await.expect("a close");
    });
}

// "a stop that lands before the proof's first ask ends the wake with the stop's own line"
#[test]
fn a_stop_that_lands_before_the_proofs_first_ask_ends_the_wake_with_the_stops_own_line() {
    drive(|| async {
        let held: Rc<RefCell<Option<Rc<common::WrappedStore>>>> = Rc::default();
        let kept = held.clone();
        let wrap = move |inner: &Rc<dyn Store>| -> Rc<dyn Store> {
            let wrapped = common::WrappedStore::over(inner);
            *kept.borrow_mut() = Some(wrapped.clone());
            wrapped.store()
        };
        let t = restarted_over("napping", 0.0, Some(&wrap)).await;
        t.ready();
        let wrapped = held.borrow().clone().expect("a wrapped store");
        let let_put = wrapped.hold_first_put_at_phase("workspaces", "waking");
        let said = tokio::task::spawn_local({
            let (rt, id) = (t.rt.clone(), t.ws_id.clone());
            async move { rt.workspaces().wake(&id, None).await.map(|_| "woke".to_string()).unwrap_or_else(|e| e.message) }
        });
        until_n(|| wrapped.held()).await;
        let stopping = tokio::task::spawn_local({
            let (rt, id) = (t.rt.clone(), t.ws_id.clone());
            async move { rt.workspaces().stop_wake(&id, None).await }
        });
        let_put.open();
        stopping.await.expect("a task").expect("a stop");
        assert_eq!(said.await.expect("a task"), WAKE_STOPPED_UP);
        assert_eq!(t.phase().await, "running");
        t.rt.close().await.expect("a close");
    });
}

// "a resumed box still restoring at the deadline fails the wake with the same line as one found running"
#[test]
fn a_resumed_box_still_restoring_at_the_deadline_fails_the_wake_with_the_same_line_as_one_found_running() {
    drive(|| async {
        let t = restarted("napping", f64::INFINITY).await;
        let refused = t.rt.workspaces().wake(&t.ws_id, None).await.expect_err("a refusal");
        assert!(refused.message.contains(&no_commands_yet_line("x", "Box is restoring.")), "{}", refused.message);
        t.rt.close().await.expect("a close");
    });
}
