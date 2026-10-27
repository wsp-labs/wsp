// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/pause.test.ts at 65f6b89c7, case for case. A pause is a phase of its own: pausing is persisted
//! and pushed before the provider is asked, sends are refused from then on, and every live session of the workspace ends
//! with a reason the timeline shows as its last row.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use common::gone::{edge_holding, until_n};
use common::local::ws_request;
use common::stub_backend::conflict;
use common::*;
use futures::future::{FutureExt, LocalBoxFuture};
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent, SharedFuture, TurnResult};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::status::StatusWatchOptions;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_core::types::wiring::{RuntimeOptions, WakeOptions};
use wsp_engine::errors::EngineError;
use wsp_serve::serve::{serve_runtime, ServeOptions};
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::views::workspace::{ReachState, WorkspacePhase};
use wsp_wire::wire::capabilities::PauseMode;
use wsp_words::words::computer::{move_timed_out_line, provider_cannot_refusal, Move, RESUME_UNANSWERED};
use wsp_words::words::thread::ALREADY_RUNNING;
use wsp_words::workspace_state::{
    machine_word, send_refusal, workspace_state, workspace_state_line, workspace_word, SendRefusalKind, WorkspaceStateInput,
};

const PAUSED: &str = "machine paused while the agent was working";

/// A harness whose turn never ends on its own: interrupt ends it the way the CLI would, with a session.end of its own.
#[derive(Default)]
struct Hanging {
    emitters: RefCell<HashMap<String, OnEvent>>,
    interrupts: RefCell<Vec<String>>,
    n: Cell<u32>,
}

struct HangingAdapter {
    hanging: Rc<Hanging>,
    workspace_id: String,
}

struct HangingSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    interrupt: Box<dyn Fn()>,
}

impl HarnessSession for HangingSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        (self.interrupt)();
        async {}.boxed_local()
    }
}

impl HarnessAdapter for HangingAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let h = &self.hanging;
        h.n.set(h.n.get() + 1);
        let session_id = format!("sess-{}", h.n.get());
        let on_event = o.on_event.clone().expect("an event sink");
        h.emitters.borrow_mut().insert(self.workspace_id.clone(), on_event.clone());
        let (tx, rx) = tokio::sync::oneshot::channel::<TurnResult>();
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn was interrupted") }.boxed_local();
        on_event(adapter::event(json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5" })));
        on_event(adapter::event(json!({ "type": "turn.delta", "sessionId": session_id, "kind": "text", "text": "working" })));
        let tx = RefCell::new(Some(tx));
        let (hanging, workspace_id, ending, sink) = (h.clone(), self.workspace_id.clone(), session_id.clone(), on_event);
        Ok(Arc::new(HangingSession {
            local_id: session_id,
            finished: finished.shared(),
            interrupt: Box::new(move || {
                hanging.interrupts.borrow_mut().push(workspace_id.clone());
                sink(adapter::event(json!({ "type": "turn.done", "sessionId": ending, "result": { "status": "interrupted" } })));
                sink(adapter::event(json!({ "type": "session.end", "sessionId": ending, "exitCode": 143, "sawResult": false })));
                if let Some(tx) = tx.borrow_mut().take() {
                    let _ = tx.send(adapter::result(json!({ "status": "interrupted" })));
                }
            }),
        }))
    }
    fn steers(&self) -> bool {
        false
    }
}

fn hanging_adapter() -> (HarnessAdapterFactory, Rc<Hanging>) {
    let hanging = Rc::new(Hanging::default());
    let kept = hanging.clone();
    let factory: HarnessAdapterFactory = Arc::new(move |ctx: HarnessAdapterContext| {
        Ok(Arc::new(HangingAdapter { hanging: kept.clone(), workspace_id: ctx.workspace_id.clone() }) as Arc<dyn HarnessAdapter>)
    });
    (factory, hanging)
}

fn ends(events: &[Value]) -> Vec<Value> {
    events.iter().filter(|e| e["type"] == "session.end").cloned().collect()
}

async fn get(rt: &Rc<AssembledRuntime>, id: &str) -> Value {
    j(&rt.workspaces().get(id, None, None).await.expect("the workspace"))
}

async fn phase_is(rt: &Rc<AssembledRuntime>, id: &str, phase: &str) -> bool {
    get(rt, id).await["phase"] == phase
}

async fn stored(store: &Rc<dyn Store>, id: &str) -> Value {
    store.get("workspaces", id).await.expect("a read").expect("a record")
}

async fn start(
    rt: &Rc<AssembledRuntime>,
    id: &str,
    prompt: &str,
) -> Result<Rc<dyn wsp_core::types::wiring::SessionHandle>, wsp_words::exit::HostError> {
    rt.sessions().start(id, StartOptions { prompt: prompt.into(), ..Default::default() }, None).await
}

fn index_of(events: &[Value], kind: &str) -> usize {
    events.iter().position(|e| e["type"] == kind).unwrap_or_else(|| panic!("no {kind} event"))
}

// "the pause mode gates the two moves" > "nap and wake are refused with the provider's own sentence on a backend without a pause mode, and taken on one declaring disk"
#[test]
fn nap_and_wake_are_refused_with_the_providers_own_sentence_on_a_backend_without_a_pause_mode_and_taken_on_one_declaring_disk() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        backend.set_capabilities(|c| c.pause_mode = None);
        // A machine wsp forks whose provider does not pause is not a machine wsp does not run: the refusal names the
        // provider's limit, as every other verb this machine cannot take does.
        let refused = fx.rt.workspaces().nap(&ws.view.id, None).await.expect_err("a refusal");
        let want = provider_cannot_refusal("x", &machine_word(&wsp_frames::WorkspaceKind::Cloud), "be paused");
        assert!(refused.message.contains(&want), "{}", refused.message);
        assert!(!backend.machine(0).paused.get());
        // The runtime reads that a mode is there, never which: a disk pause is a nap and a wake like any other here.
        backend.set_capabilities(|c| c.pause_mode = Some(PauseMode::Disk));
        assert_eq!(j(&fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap"))["phase"], "napping");
        assert!(backend.machine(0).paused.get());
        backend.set_capabilities(|c| c.pause_mode = None);
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        let want = provider_cannot_refusal("x", &machine_word(&wsp_frames::WorkspaceKind::Cloud), "be woken");
        assert!(refused.message.contains(&want), "{}", refused.message);
        assert!(backend.machine(0).paused.get());
        backend.set_capabilities(|c| c.pause_mode = Some(PauseMode::Disk));
        assert_eq!(j(&fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
        assert_eq!(backend.machine(0).resumes.get(), 1);
        fx.rt.close().await.expect("a close");
    });
}

// "nap phase order" > "persists and pushes pausing before the provider pause, napping after, and a read mid-pause says pausing"
#[test]
fn persists_and_pushes_pausing_before_the_provider_pause_napping_after_and_a_read_mid_pause_says_pausing() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m = backend.machine(0);
        let pause = m.pause_fn();
        let mid: Rc<RefCell<Option<(String, String, Vec<String>)>>> = Rc::default();
        let (kept, rt, store, seen, id) = (mid.clone(), fx.rt.clone(), state.store.clone(), events.clone(), ws.view.id.clone());
        m.set_pause(Rc::new(move || {
            let (kept, rt, store, seen, id, pause) = (kept.clone(), rt.clone(), store.clone(), seen.clone(), id.clone(), pause.clone());
            Box::pin(async move {
                let phase_stored = stored(&store, &id).await["phase"].as_str().unwrap_or_default().to_string();
                let phase_read = get(&rt, &id).await["phase"].as_str().unwrap_or_default().to_string();
                let pushed = pushed_lines(&seen);
                *kept.borrow_mut() = Some((phase_stored, phase_read, pushed));
                pause().await
            })
        }));
        let napped = fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(mid.borrow().clone(), Some(("pausing".to_string(), "pausing".to_string(), vec!["pausing/running/napping".to_string()])));
        assert_eq!(j(&napped)["phase"], "napping");
        assert_eq!(pushed_lines(&events), ["pausing/running/napping", "napping/paused/napping"]);
        assert_eq!(stored(&state.store, &ws.view.id).await["phase"], "napping");
        fx.rt.close().await.expect("a close");
    });
}

/// `${status.phase}/${status.machineState}/${status.reach.state}` of every status pushed so far.
fn pushed_lines(events: &Events) -> Vec<String> {
    events
        .statuses()
        .iter()
        .map(|s| {
            format!(
                "{}/{}/{}",
                s["phase"].as_str().unwrap_or(""),
                s["machineState"].as_str().unwrap_or(""),
                s["reach"]["state"].as_str().unwrap_or("")
            )
        })
        .collect()
}

// "a pause the provider refuses goes back to running with the error on the pushed status, its sessions untouched"
#[test]
fn a_pause_the_provider_refuses_goes_back_to_running_with_the_error_on_the_pushed_status_its_sessions_untouched() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let (factory, _) = hanging_adapter();
        let fx = runtime(&backend).adapter("claude", factory).build();
        let pushed = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let handle = start(&fx.rt, &ws.view.id, "hi").await.expect("a start");
        backend
            .machine(0)
            .set_pause(Rc::new(|| Box::pin(async { Err(EngineError::Other { message: "provider down".into(), code: None }) })));
        let refused = fx.rt.workspaces().nap(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("provider down"), "{}", refused.message);
        assert_eq!(get(&fx.rt, &ws.view.id).await["phase"], "running");
        assert!(!pushed.has("session.end"));
        assert_eq!(j(&handle.view())["status"], "running");
        let rows: Vec<(Value, Value)> =
            pushed.statuses().iter().map(|s| (s["phase"].clone(), s.get("reason").cloned().unwrap_or(Value::Null))).collect();
        assert_eq!(rows, [(json!("pausing"), Value::Null), (json!("running"), json!("provider down"))]);
        fx.rt.close().await.expect("a close");
    });
}

// "a wake asked during the pause waits for it and then wakes; a second nap joins the first"
#[test]
fn a_wake_asked_during_the_pause_waits_for_it_and_then_wakes_a_second_nap_joins_the_first() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m = backend.machine(0);
        let gate = Gate::new();
        let pause = m.pause_fn();
        let held = gate.clone();
        m.set_pause(Rc::new(move || {
            let (held, pause) = (held.clone(), pause.clone());
            Box::pin(async move {
                held.wait().await;
                pause().await
            })
        }));
        let (rt, id) = (fx.rt.clone(), ws.view.id.clone());
        let first = tokio::task::spawn_local({
            let (rt, id) = (rt.clone(), id.clone());
            async move { rt.workspaces().nap(&id, None).await }
        });
        until(|| {
            let (rt, id) = (rt.clone(), id.clone());
            async move { phase_is(&rt, &id, "pausing").await }
        })
        .await;
        let second = tokio::task::spawn_local({
            let (rt, id) = (rt.clone(), id.clone());
            async move { rt.workspaces().nap(&id, None).await }
        });
        let waking = tokio::task::spawn_local({
            let (rt, id) = (rt.clone(), id.clone());
            async move { rt.workspaces().wake(&id, None).await }
        });
        assert_eq!(get(&rt, &id).await["phase"], "pausing");
        gate.open();
        assert_eq!(j(&first.await.expect("a task").expect("a nap"))["phase"], "napping");
        assert_eq!(j(&second.await.expect("a task").expect("a nap"))["phase"], "napping");
        assert_eq!(j(&waking.await.expect("a task").expect("a wake"))["phase"], "running");
        assert_eq!(m.resumes.get(), 1);
        fx.rt.close().await.expect("a close");
    });
}

// "the start guard" > "refuses sessions.start while pausing, paused and waking with the composer's sentence, in process and over the wire"
#[test]
fn refuses_sessions_start_while_pausing_paused_and_waking_with_the_composers_sentence_in_process_and_over_the_wire() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let (factory, _) = hanging_adapter();
        let fx = runtime(&backend).adapter("claude", factory).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m = backend.machine(0);
        let pause_gate = Gate::new();
        let pause = m.pause_fn();
        let held = pause_gate.clone();
        m.set_pause(Rc::new(move || {
            let (held, pause) = (held.clone(), pause.clone());
            Box::pin(async move {
                held.wait().await;
                pause().await
            })
        }));
        let (rt, id) = (fx.rt.clone(), ws.view.id.clone());
        let napping = tokio::task::spawn_local({
            let (rt, id) = (rt.clone(), id.clone());
            async move { rt.workspaces().nap(&id, None).await }
        });
        until(|| {
            let (rt, id) = (rt.clone(), id.clone());
            async move { phase_is(&rt, &id, "pausing").await }
        })
        .await;
        let refused = start(&rt, &id, "hi").await.err().expect("a refusal");
        assert!(refused.message.contains("Workspace is pausing; wake it to send"), "{}", refused.message);
        pause_gate.open();
        napping.await.expect("a task").expect("a nap");
        let refused = start(&rt, &id, "hi").await.err().expect("a refusal");
        assert!(refused.message.contains("Workspace is paused; wake it to send"), "{}", refused.message);

        let served = serve_runtime(rt.clone(), ServeOptions { port: Some(0), auth_token: "t".into(), ..Default::default() })
            .await
            .expect("the runtime serves");
        let res = ws_request(served.port(), "t", "sessions.start", json!({ "workspaceId": id, "prompt": "hi" })).await;
        served.close().await.expect("a close");
        assert_matches(&res, json!({ "ok": false, "error": "Workspace is paused; wake it to send" }));

        let resume_gate = Gate::new();
        let resume = m.resume_with_fn();
        let held = resume_gate.clone();
        m.set_resume_with(Rc::new(move |signal| {
            let (held, resume) = (held.clone(), resume.clone());
            Box::pin(async move {
                held.wait().await;
                resume(signal).await
            })
        }));
        let waking = tokio::task::spawn_local({
            let (rt, id) = (rt.clone(), id.clone());
            async move { rt.workspaces().wake(&id, None).await }
        });
        until(|| {
            let (rt, id) = (rt.clone(), id.clone());
            async move { phase_is(&rt, &id, "waking").await }
        })
        .await;
        let refused = start(&rt, &id, "hi").await.err().expect("a refusal");
        assert!(refused.message.contains("Workspace is waking; sends open when it is running"), "{}", refused.message);
        resume_gate.open();
        waking.await.expect("a task").expect("a wake");
        let handle = start(&rt, &id, "hi").await.expect("a start");
        assert_eq!(j(&handle.view())["status"], "running");
        assert_eq!(rt.sessions().list(Some(id.clone()), None).await.expect("a list").len(), 1);
        rt.close().await.expect("a close");
    });
}

// "sessions end with the machine" > "nap ends every live session of that workspace with the reason once the provider has paused, once, and drops what the harness says after"
#[test]
fn nap_ends_every_live_session_of_that_workspace_with_the_reason_once_the_provider_has_paused_once_and_drops_what_the_harness_says_after() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let (factory, hanging) = hanging_adapter();
        let fx = runtime(&backend).adapter("claude", factory).build();
        let events = fx.events();
        let a = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let b = create_on(&fx.rt, CreateOn::golden("b"), None).await.expect("a workspace");
        let one = start(&fx.rt, &a.view.id, "one").await.expect("a start");
        let two = start(&fx.rt, &a.view.id, "two").await.expect("a start");
        let other = start(&fx.rt, &b.view.id, "other").await.expect("a start");
        let at_pause: Rc<RefCell<Vec<String>>> = Rc::default();
        let m = backend.machine(0);
        let pause = m.pause_fn();
        let (kept, rt, id) = (at_pause.clone(), fx.rt.clone(), a.view.id.clone());
        m.set_pause(Rc::new(move || {
            let (kept, rt, id, pause) = (kept.clone(), rt.clone(), id.clone(), pause.clone());
            Box::pin(async move {
                let listed = rt.sessions().list(Some(id), None).await.expect("a list");
                *kept.borrow_mut() = listed.iter().map(|s| j(s)["status"].as_str().unwrap_or_default().to_string()).collect();
                pause().await
            })
        }));

        fx.rt.workspaces().nap(&a.view.id, None).await.expect("a nap");

        // The reason says the machine paused, so it is written once the provider has confirmed the pause, not before.
        assert_eq!(*at_pause.borrow(), ["running", "running"]);
        let all = events.all();
        let ended: Vec<Value> = ends(&all).into_iter().filter(|e| e["workspaceId"] == a.view.id.as_str()).collect();
        assert_eq!(ended.len(), 2);
        for e in &ended {
            assert_matches(e, json!({ "exitCode": null, "sawResult": false, "reason": PAUSED }));
        }
        let ended_ids: std::collections::BTreeSet<String> =
            ended.iter().map(|e| e["sessionId"].as_str().unwrap_or_default().to_string()).collect();
        let started_ids: std::collections::BTreeSet<String> =
            [&one, &two].iter().map(|h| j(&h.view())["claudeSessionId"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(ended_ids, started_ids);
        assert!(index_of(&all, "session.end") < index_of(&all, "workspace.napped"));
        for s in fx.rt.sessions().list(Some(a.view.id.clone()), None).await.expect("a list") {
            let s = j(&s);
            assert_eq!(s["status"], "failed");
            assert!(s["endedAt"].is_number(), "{s}");
        }
        assert_eq!(*hanging.interrupts.borrow(), [a.view.id.clone(), a.view.id.clone()]);
        assert_eq!(j(&other.view())["status"], "running");
        assert!(ends(&all).iter().all(|e| e["workspaceId"] != b.view.id.as_str()));

        // The harness's own end and anything after it are already told: the reason row stays the last one.
        let history = fx.rt.sessions().history(&a.view.id, None).await.expect("a history");
        let last = j(history.last().expect("a row"));
        assert_matches(&last, json!({ "type": "session.end", "reason": PAUSED }));
        assert_eq!(history.iter().filter(|e| j(e)["type"] == "session.end").count(), 2);
        let emit = hanging.emitters.borrow()[&a.view.id].clone();
        emit(adapter::event(json!({ "type": "turn.delta", "sessionId": "sess-2", "kind": "text", "text": "late" })));
        assert_eq!(fx.rt.sessions().history(&a.view.id, None).await.expect("a history").len(), history.len());
        let interrupted = fx.rt.sessions().interrupt(&two.id(), None, None).await.expect("an interrupt");
        assert_eq!(j(&interrupted), json!({ "outcome": "not-running" }));
        one.finished().await;
        assert_eq!(j(&one.view())["status"], "failed");
        fx.rt.close().await.expect("a close");
    });
}

// "delete ends the live sessions with its own reason before the machine dies"
#[test]
fn delete_ends_the_live_sessions_with_its_own_reason_before_the_machine_dies() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let (factory, _) = hanging_adapter();
        let fx = runtime(&backend).adapter("claude", factory).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let handle = start(&fx.rt, &ws.view.id, "one").await.expect("a start");
        fx.rt.workspaces().delete(&ws.view.id, None).await.expect("a delete");
        let all = events.all();
        let done = ends(&all);
        assert_eq!(done.len(), 1);
        assert_matches(&done[0], json!({ "workspaceId": ws.view.id, "reason": "machine deleted while the agent was working" }));
        assert!(index_of(&all, "session.end") < index_of(&all, "workspace.deleted"));
        let view = j(&handle.view());
        assert_eq!(view["status"], "failed");
        assert!(view["endedAt"].is_number(), "{view}");
        fx.rt.close().await.expect("a close");
    });
}

/// A record the first runtime wrote, put back at `phase` for a second runtime over the same state to hydrate.
async fn left_at(phase: &str) -> (Arc<StubBackend>, State, String, Fx) {
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let first = runtime(&backend).store(&state.store).build();
    let ws = create_on(&first.rt, CreateOn::golden("x"), None).await.expect("a workspace");
    let mut record = stored(&state.store, &ws.view.id).await;
    record["phase"] = json!(phase);
    state.store.put("workspaces", &ws.view.id, record).await.expect("a write");
    (backend, state, ws.view.id, first)
}

// "a record left at pausing" > "hydrates as napping when the provider holds the machine paused, so the next wake resumes it"
#[test]
fn hydrates_as_napping_when_the_provider_holds_the_machine_paused_so_the_next_wake_resumes_it() {
    drive(|| async {
        let (backend, state, id, _first) = left_at("pausing").await;
        backend.machine(0).paused.set(true);
        let second = runtime(&backend).store(&state.store).build();
        assert_eq!(get(&second.rt, &id).await["phase"], "napping");
        assert_matches(&stored(&state.store, &id).await, json!({ "phase": "napping" }));
        let woken = second.rt.workspaces().wake(&id, None).await.expect("a wake");
        assert_eq!(j(&woken)["phase"], "running");
        assert_eq!(backend.machine(0).resumes.get(), 1);
        second.rt.close().await.expect("a close");
    });
}

// The host died between writing pausing and the provider's pause taking, or the pause was refused; or the nap wrote
// napping over a pause that never took, and the host was restarted with the store saying so.
async fn left_over_a_machine_the_provider_runs(left: &str) {
    let (backend, state, id, _first) = left_at(left).await;
    let warned = capture();
    let second = runtime(&backend).store(&state.store).build();
    let hydrated = get(&second.rt, &id).await;
    assert_eq!(hydrated["phase"], "running");
    let input = WorkspaceStateInput { phase: WorkspacePhase::Running, machine_state: None, reach: None, wake_refused: None };
    assert_eq!(send_refusal(&SendRefusalKind::State(workspace_state(&input)), None, None), None);
    assert_matches(&stored(&state.store, &id).await, json!({ "phase": "running" }));
    assert_matches(&status_rows(&second.rt).await[0], json!({ "phase": "running", "machineState": "running" }));
    let line = format!("workspace {id} was left {left} and its machine is running at the provider; the record hydrates running");
    assert!(warned.lines().contains(&line), "{:?}", warned.lines());
    assert_eq!(backend.machine(0).resumes.get(), 0);
    assert!(!backend.machine(0).paused.get());
    second.rt.close().await.expect("a close");
}

// "left pausing over a machine the provider runs, it hydrates running, says so in the log, and the machine bills and takes sends again"
#[test]
fn left_pausing_over_a_machine_the_provider_runs_it_hydrates_running_says_so_in_the_log_and_the_machine_bills_and_takes_sends_again() {
    drive(|| left_over_a_machine_the_provider_runs("pausing"));
}

// "left napping over a machine the provider runs, it hydrates running, says so in the log, and the machine bills and takes sends again"
#[test]
fn left_napping_over_a_machine_the_provider_runs_it_hydrates_running_says_so_in_the_log_and_the_machine_bills_and_takes_sends_again() {
    drive(|| left_over_a_machine_the_provider_runs("napping"));
}

/// A nap whose pause never took at the provider: the record says napping, the machine runs, and a resume would be refused.
struct Never {
    backend: Arc<StubBackend>,
    fx: Fx,
    id: String,
    events: Events,
    base: usize,
}

impl Never {
    fn since(&self) -> Vec<Value> {
        self.events.all()[self.base..].to_vec()
    }
}

async fn nap_that_never_took() -> Never {
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).build();
    let events = fx.events();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
    let m = backend.machine(0);
    m.paused.set(false);
    m.set_resume(Rc::new(|| Box::pin(async { Err(conflict("Sandbox is not paused")) })));
    let base = events.len();
    Never { backend, fx, id: ws.view.id, events, base }
}

// "a record that says napping over a machine the provider runs" > "pause asks the provider first and pauses the machine for real"
#[test]
fn pause_asks_the_provider_first_and_pauses_the_machine_for_real() {
    drive(|| async {
        let n = nap_that_never_took().await;
        let napped = n.fx.rt.workspaces().nap(&n.id, None).await.expect("a nap");
        assert_eq!(j(&napped)["phase"], "napping");
        assert!(n.backend.machine(0).paused.get());
        assert_eq!(get(&n.fx.rt, &n.id).await["phase"], "napping");
        n.fx.rt.close().await.expect("a close");
    });
}

// "two pauses asked at once adopt running once and pause once: one provider pause, one woken, no running push inside the pause"
#[test]
fn two_pauses_asked_at_once_adopt_running_once_and_pause_once_one_provider_pause_one_woken_no_running_push_inside_the_pause() {
    drive(|| async {
        let n = nap_that_never_took().await;
        let m = n.backend.machine(0);
        let pauses = Rc::new(Cell::new(0u32));
        let pause = m.pause_fn();
        let counted = pauses.clone();
        m.set_pause(Rc::new(move || {
            counted.set(counted.get() + 1);
            pause()
        }));
        let (a, b) = futures::future::join(n.fx.rt.workspaces().nap(&n.id, None), n.fx.rt.workspaces().nap(&n.id, None)).await;
        assert_eq!(j(&a.expect("a nap"))["phase"], "napping");
        assert_eq!(j(&b.expect("a nap"))["phase"], "napping");
        assert_eq!(pauses.get(), 1);
        assert!(m.paused.get());
        let since = n.since();
        assert_eq!(since.iter().filter(|e| e["type"] == "workspace.woken").count(), 1);
        let phases: Vec<&str> =
            since.iter().filter(|e| e["type"] == "workspace.status").map(|e| e["status"]["phase"].as_str().unwrap_or("")).collect();
        assert_eq!(phases, ["running", "pausing", "napping"]);
        n.fx.rt.close().await.expect("a close");
    });
}

// "pause on a record that says napping over a machine the provider holds paused asks once and pauses nothing"
#[test]
fn pause_on_a_record_that_says_napping_over_a_machine_the_provider_holds_paused_asks_once_and_pauses_nothing() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let m = backend.machine(0);
        let asked = m.calls.state.get();
        let pauses = Rc::new(Cell::new(0u32));
        let counted = pauses.clone();
        m.set_pause(Rc::new(move || {
            counted.set(counted.get() + 1);
            Box::pin(async { Ok(()) })
        }));
        assert_eq!(j(&fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap"))["phase"], "napping");
        assert_eq!(m.calls.state.get() - asked, 1);
        assert_eq!(pauses.get(), 0);
        fx.rt.close().await.expect("a close");
    });
}

// "wake takes the provider's running as the fact: the record becomes running with the line in words, nothing is resumed, and the row never says waking"
#[test]
fn wake_takes_the_providers_running_as_the_fact_the_record_becomes_running_with_the_line_in_words_nothing_is_resumed_and_the_row_never_says_waking(
) {
    drive(|| async {
        let n = nap_that_never_took().await;
        let woken = n.fx.rt.workspaces().wake(&n.id, None).await.expect("a wake");
        assert_eq!(j(&woken)["phase"], "running");
        assert_eq!(get(&n.fx.rt, &n.id).await["phase"], "running");
        assert_eq!(n.backend.machine(0).resumes.get(), 0);
        let since = n.since();
        let pushed: Vec<Value> = since.iter().filter(|e| e["type"] == "workspace.status").map(|e| e["status"].clone()).collect();
        assert!(!pushed.iter().any(|s| s["phase"] == "waking"));
        assert_matches(
            pushed.last().expect("a status"),
            json!({ "phase": "running", "machineState": "running", "reason": ALREADY_RUNNING }),
        );
        assert!(since.iter().any(|e| e["type"] == "workspace.woken"));
        // The exec that asked for the wake goes on, as on any running machine.
        let ran = n.fx.rt.workspaces().exec(&n.id, "true", None, None).await.expect("an exec");
        assert_eq!(j(&ran)["exitCode"], 0);
        n.fx.rt.workspaces().nap(&n.id, None).await.expect("a nap");
        assert!(n.backend.machine(0).paused.get());
        n.fx.rt.close().await.expect("a close");
    });
}

fn polling() -> impl FnOnce(&mut RuntimeOptions) {
    |o| {
        o.status = Some(StatusWatchOptions {
            cost_interval_ms: Some(60_000),
            poll_interval_ms: Some(5),
            prompt_ms: Some(10),
            probe_timeout_ms: Some(50),
            reconcile_min_ms: Some(0),
            ..Default::default()
        });
    }
}

// "a pause the runtime did not start" > "the poll sees the provider's machine paused under a running record: the phase follows, the sessions end, the status is pushed, and wake resumes it"
#[test]
fn the_poll_sees_the_providers_machine_paused_under_a_running_record_the_phase_follows_the_sessions_end_the_status_is_pushed_and_wake_resumes_it(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let (factory, _) = hanging_adapter();
        let fx = runtime(&backend).adapter("claude", factory).with(polling()).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let handle = start(&fx.rt, &ws.view.id, "one").await.expect("a start");
        let m = backend.machine(0);
        let _dark = edge_holding(&m).await;
        m.paused.set(true);
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        until_for(5_000, &mut || {
            let seen = events.clone();
            async move { seen.has("workspace.napped") }
        })
        .await;
        stop();
        assert_eq!(get(&fx.rt, &ws.view.id).await["phase"], "napping");
        let all = events.all();
        let done = ends(&all);
        assert_eq!(done.len(), 1);
        assert_matches(&done[0], json!({ "workspaceId": ws.view.id, "reason": PAUSED }));
        assert_eq!(j(&handle.view())["status"], "failed");
        assert!(events
            .statuses()
            .iter()
            .any(|s| s["phase"] == "napping" && s["machineState"] == "paused" && s["reason"] == "paused outside wsp"));
        let refused = start(&fx.rt, &ws.view.id, "again").await.err().expect("a refusal");
        assert!(refused.message.contains("Workspace is paused; wake it to send"), "{}", refused.message);
        let woken = fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_eq!(j(&woken)["phase"], "running");
        assert_eq!(m.resumes.get(), 1);
        assert!(!m.paused.get());
        fx.rt.close().await.expect("a close");
    });
}

// "a record left waking by a wake that never landed hydrates paused when the provider holds the machine paused, and the row offers Wake"
#[test]
fn a_record_left_waking_by_a_wake_that_never_landed_hydrates_paused_when_the_provider_holds_the_machine_paused_and_the_row_offers_wake() {
    drive(|| async {
        let (backend, state, id, _first) = left_at("waking").await;
        // The host died inside a wake the provider never finished, and the machine is paused at the provider.
        backend.machine(0).paused.set(true);
        let second = runtime(&backend).store(&state.store).build();
        let hydrated = get(&second.rt, &id).await;
        assert_eq!(hydrated["phase"], "napping");
        let napping =
            workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Napping, machine_state: None, reach: None, wake_refused: None });
        assert_eq!(workspace_word(&napping, Some(&backend.capabilities().pause_mode.expect("a pause mode"))), "Paused");
        assert_eq!(send_refusal(&SendRefusalKind::State(napping), None, None).as_deref(), Some("Workspace is paused; wake it to send"));
        assert_matches(&stored(&state.store, &id).await, json!({ "phase": "napping" }));
        let woken = second.rt.workspaces().wake(&id, None).await.expect("a wake");
        assert_eq!(j(&woken)["phase"], "running");
        assert_eq!(backend.machine(0).resumes.get(), 1);
        second.rt.close().await.expect("a close");
    });
}

// "a record left waking whose machine the provider is running hydrates running, so it bills, naps and takes sends again"
#[test]
fn a_record_left_waking_whose_machine_the_provider_is_running_hydrates_running_so_it_bills_naps_and_takes_sends_again() {
    drive(|| async {
        let (backend, state, id, _first) = left_at("waking").await;
        // The resume landed and the host died before it could write the record.
        let second = runtime(&backend).store(&state.store).build();
        let hydrated = get(&second.rt, &id).await;
        assert_eq!(hydrated["phase"], "running");
        let running =
            workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Running, machine_state: None, reach: None, wake_refused: None });
        assert_eq!(workspace_word(&running, None), "Running");
        assert_eq!(send_refusal(&SendRefusalKind::State(running), None, None), None);
        assert_matches(&stored(&state.store, &id).await, json!({ "phase": "running" }));
        assert_matches(&status_rows(&second.rt).await[0], json!({ "phase": "running", "machineState": "running" }));
        second.rt.close().await.expect("a close");
    });
}

// "a wake the provider refuses leaves the record paused, not waking, and the next wake tries again"
#[test]
fn a_wake_the_provider_refuses_leaves_the_record_paused_not_waking_and_the_next_wake_tries_again() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let m = backend.machine(0);
        let resume = m.resume_fn();
        let refuse = Rc::new(Cell::new(true));
        let refusing = refuse.clone();
        // What Solari's resume answers when it gives up: a transient status, out of retries.
        m.set_resume(Rc::new(move || {
            if refusing.get() {
                return Box::pin(async {
                    Err(EngineError::Refused {
                        message: "upstream request timeout".into(),
                        kind: Some(wsp_engine::errors::ErrorKind::Transient),
                        status: Some(504),
                        code: None,
                    })
                });
            }
            resume()
        }));
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("upstream request timeout"), "{}", refused.message);
        assert_eq!(get(&fx.rt, &ws.view.id).await["phase"], "napping");
        assert_matches(&stored(&state.store, &ws.view.id).await, json!({ "phase": "napping" }));

        refuse.set(false);
        assert_eq!(j(&fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
        assert!(!m.paused.get());
        fx.rt.close().await.expect("a close");
    });
}

// "a wake asked while the record still says running asks the provider once and resumes a machine it finds paused"
#[test]
fn a_wake_asked_while_the_record_still_says_running_asks_the_provider_once_and_resumes_a_machine_it_finds_paused() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let m = backend.machine(0);
        let asked = m.calls.state.get();
        assert_eq!(j(&fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
        assert_eq!(m.calls.state.get() - asked, 1);
        assert_eq!(m.resumes.get(), 0);
        m.paused.set(true);
        let woken = fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_eq!(j(&woken)["phase"], "running");
        assert_eq!(m.resumes.get(), 1);
        assert!(events.has("workspace.napped"));
        assert!(events.has("workspace.woken"));
        fx.rt.close().await.expect("a close");
    });
}

// A Boat box restoring from a nap reads running while its daemon has not started, for minutes: a paused reading kept
// from before the wake must not answer for the woken machine.
struct Woken {
    fx: Fx,
    backend: Arc<StubBackend>,
    events: Events,
    id: String,
    m: Arc<StubMachine>,
    _dark: common::Edge,
}

async fn woken_after_a_listing(road: &str) -> Woken {
    let backend = StubBackend::new(None);
    // Boat's: a stop that keeps the disk and kills every process.
    backend.set_capabilities(|c| c.pause_mode = Some(PauseMode::Disk));
    let (factory, _) = hanging_adapter();
    let fx = runtime(&backend)
        .adapter("claude", factory)
        .with(|o| {
            o.status = Some(StatusWatchOptions {
                cost_interval_ms: Some(60_000),
                poll_interval_ms: Some(5),
                prompt_ms: Some(10),
                probe_timeout_ms: Some(50),
                ..Default::default()
            });
        })
        .build();
    let events = fx.events();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    let m = backend.machine(0);
    fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
    let dark = edge_holding(&m).await;
    // What the app's status subscription asks when it connects: the provider, for every machine, the napping one included.
    let listed = status_rows(&fx.rt).await.into_iter().find(|s| s["id"] == ws.view.id.as_str()).expect("a row");
    assert_matches(&listed, json!({ "machineState": "paused" }));
    if road == "found running by the wake" {
        m.paused.set(false);
    }
    assert_eq!(j(&fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
    Woken { fx, backend, events, id: ws.view.id, m, _dark: dark }
}

async fn a_turn_started_on_a_machine_whose_daemon_is_not_up(road: &str) {
    let w = woken_after_a_listing(road).await;
    let woken = w.events.len();
    let handle = start(&w.fx.rt, &w.id, "review").await.expect("a start");
    let stop = w.fx.rt.status().watch(StatusWatchOptions::default());
    // The poll that finds the daemon silent pushes the row; an adoption of a pause it read would follow within ms.
    until_for(5_000, &mut || {
        let (events, id) = (w.events.clone(), w.id.clone());
        async move {
            events.all()[woken..].iter().any(|e| {
                e["type"] == "workspace.status" && e["status"]["id"] == id.as_str() && e["status"]["reach"]["state"] != "reachable"
            })
        }
    })
    .await;
    sleep_ms(100).await;
    stop();
    let after = w.events.all()[woken..].to_vec();
    assert!(after.iter().all(|e| e["type"] != "workspace.napped"));
    assert!(ends(&after).is_empty());
    assert_eq!(j(&handle.view())["status"], "running");
    assert_eq!(get(&w.fx.rt, &w.id).await["phase"], "running");
    assert!(!w.m.paused.get());
    w.fx.rt.close().await.expect("a close");
}

// "a turn started on a machine resumed by the wake, whose daemon is not up yet, runs on through the polls: a paused reading from its nap does not stop it"
#[test]
fn a_turn_started_on_a_machine_resumed_by_the_wake_whose_daemon_is_not_up_yet_runs_on_through_the_polls_a_paused_reading_from_its_nap_does_not_stop_it(
) {
    drive(|| a_turn_started_on_a_machine_whose_daemon_is_not_up("resumed by the wake"));
}

// "a turn started on a machine found running by the wake, whose daemon is not up yet, runs on through the polls: a paused reading from its nap does not stop it"
#[test]
fn a_turn_started_on_a_machine_found_running_by_the_wake_whose_daemon_is_not_up_yet_runs_on_through_the_polls_a_paused_reading_from_its_nap_does_not_stop_it(
) {
    drive(|| a_turn_started_on_a_machine_whose_daemon_is_not_up("found running by the wake"));
}

async fn a_wake_lists_it_up_and_not_answering(road: &str) {
    let w = woken_after_a_listing(road).await;
    let served = serve_runtime(w.fx.rt.clone(), ServeOptions { port: Some(0), auth_token: "t".into(), ..Default::default() })
        .await
        .expect("the runtime serves");
    // The op wsp wake prints its last line from.
    let res = ws_request(served.port(), "t", "status.list", json!({})).await;
    served.close().await.expect("a close");
    let listed = res["statuses"].as_array().expect("statuses").iter().find(|s| s["id"] == w.id.as_str()).expect("a row").clone();
    let state = workspace_state(&WorkspaceStateInput {
        phase: serde_json::from_value::<WorkspacePhase>(listed["phase"].clone()).expect("a phase"),
        machine_state: serde_json::from_value::<wsp_wire::views::workspace::MachineState>(listed["machineState"].clone()).ok(),
        reach: serde_json::from_value::<ReachState>(listed["reach"]["state"].clone()).ok(),
        wake_refused: None,
    });
    let line = workspace_state_line("a", &state, w.backend.capabilities().pause_mode.as_ref());
    assert_eq!(line, "a is up and not answering yet");
    w.fx.rt.close().await.expect("a close");
}

// "a wake of a machine resumed by the wake, whose daemon is not up yet, lists it up and not answering, never stopped"
#[test]
fn a_wake_of_a_machine_resumed_by_the_wake_whose_daemon_is_not_up_yet_lists_it_up_and_not_answering_never_stopped() {
    drive(|| a_wake_lists_it_up_and_not_answering("resumed by the wake"));
}

// "a wake of a machine found running by the wake, whose daemon is not up yet, lists it up and not answering, never stopped"
#[test]
fn a_wake_of_a_machine_found_running_by_the_wake_whose_daemon_is_not_up_yet_lists_it_up_and_not_answering_never_stopped() {
    drive(|| a_wake_lists_it_up_and_not_answering("found running by the wake"));
}

// "a provider move the backend gave up on": the backend settles its own pause and resume, on its own budgets and with its
// own reads of the machine: the runtime sends each move once, puts a typed failure's words on the row, and never reads the
// machine inside a move.
fn pause_words() -> String {
    move_timed_out_line(Move::Pause, 20.0, Some("running"))
}

/// A move that ends the way a backend ends one it gave up on: with the typed error carrying the row's words. After `land`
/// the machine reaches the state instead, as a provider that took the call late would.
struct Unanswered {
    calls: Rc<Cell<u32>>,
    lands: Rc<Cell<bool>>,
}

fn unanswered(backend: &Arc<StubBackend>, pausing: bool) -> Unanswered {
    let m = backend.machine(0);
    let counter = Unanswered { calls: Rc::new(Cell::new(0)), lands: Rc::new(Cell::new(false)) };
    let (calls, lands, machine) = (counter.calls.clone(), counter.lands.clone(), m.clone());
    let f: common::stub_backend::UnitFn = Rc::new(move || {
        calls.set(calls.get() + 1);
        if lands.get() {
            machine.paused.set(pausing);
            return Box::pin(async { Ok(()) });
        }
        let failure = if pausing {
            EngineError::MoveUnanswered { message: pause_words(), cause: None }
        } else {
            EngineError::ResumeUnanswered { message: RESUME_UNANSWERED.into(), cause: None }
        };
        Box::pin(async move { Err(failure) })
    });
    if pausing {
        m.set_pause(f);
    } else {
        m.set_resume(f);
    }
    counter
}

struct Rig {
    backend: Arc<StubBackend>,
    state: State,
    fc: FakeClock,
    fx: Fx,
    events: Events,
}

/// A runtime on a clock the test moves, so the late read and the asking cadence fire only where the test says so.
fn rig(wake: Option<WakeOptions>) -> Rig {
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let fc = FakeClock::at_now();
    let fx = runtime(&backend)
        .store(&state.store)
        .clock(&fc)
        .with(move |o| {
            if let Some(wake) = wake {
                o.wake = Some(wake);
            }
        })
        .build();
    let events = fx.events();
    Rig { backend, state, fc, fx, events }
}

fn pushes(events: &Events) -> Vec<Value> {
    events.statuses()
}

fn gave_up_one(text: &str) -> bool {
    text.starts_with("the provider answered none of 1 resume request over ")
}

// "a pause that ends in MoveUnansweredError puts the record back to running with the backend's words on the row, and the next pause works"
#[test]
fn a_pause_that_ends_in_move_unanswered_error_puts_the_record_back_to_running_with_the_backends_words_on_the_row_and_the_next_pause_works()
{
    drive(|| async {
        let r = rig(None);
        let ws = create_on(&r.fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let hung = unanswered(&r.backend, true);
        let refused = r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains(&pause_words()), "{}", refused.message);
        assert_eq!(hung.calls.get(), 1);
        assert_eq!(get(&r.fx.rt, &ws.view.id).await["phase"], "running");
        assert_matches(&stored(&r.state.store, &ws.view.id).await, json!({ "phase": "running" }));
        assert_matches(
            pushes(&r.events).last().expect("a status"),
            json!({ "phase": "running", "machineState": "running", "reason": pause_words() }),
        );
        hung.lands.set(true);
        assert_eq!(j(&r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap"))["phase"], "napping");
        assert_eq!(hung.calls.get(), 2);
        r.fx.rt.close().await.expect("a close");
    });
}

// Asking again is the host's own road and has its own file; a backend that declares no asking ends the wake on the first one.
// "a wake that ends in ResumeUnansweredError says so on the row, leaves the wake control open, and the next wake works"
#[test]
fn a_wake_that_ends_in_resume_unanswered_error_says_so_on_the_row_leaves_the_wake_control_open_and_the_next_wake_works() {
    drive(|| async {
        let r = rig(None);
        r.backend.lifecycle().set_resume_asks(None);
        let ws = create_on(&r.fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let _warn = capture();
        let hung = unanswered(&r.backend, false);
        let refused = r.fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(gave_up_one(&refused.message), "{}", refused.message);
        assert_eq!(hung.calls.get(), 1);
        // The row said the machine was being read about while the wake was still in flight.
        assert!(pushes(&r.events).iter().any(|s| s["phase"] == "waking" && s["reason"] == RESUME_UNANSWERED));
        assert_eq!(get(&r.fx.rt, &ws.view.id).await["phase"], "napping");
        assert_matches(&stored(&r.state.store, &ws.view.id).await, json!({ "phase": "napping" }));
        let last = pushes(&r.events).last().cloned().expect("a status");
        assert_matches(&last, json!({ "phase": "napping", "machineState": "paused" }));
        assert!(gave_up_one(last["reason"].as_str().expect("a reason")), "{last}");
        let napping =
            workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Napping, machine_state: None, reach: None, wake_refused: None });
        assert_eq!(workspace_word(&napping, r.backend.capabilities().pause_mode.as_ref()), "Paused");
        hung.lands.set(true);
        assert_eq!(j(&r.fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
        r.fx.rt.close().await.expect("a close");
    });
}

// "a resume that lands after the wake gave up is found by one later read: the record follows to running instead of billing under a paused row"
#[test]
fn a_resume_that_lands_after_the_wake_gave_up_is_found_by_one_later_read_the_record_follows_to_running_instead_of_billing_under_a_paused_row(
) {
    drive(|| async {
        let r = rig(Some(WakeOptions { late_read_ms: Some(50), ..Default::default() }));
        r.backend.lifecycle().set_resume_asks(None);
        let ws = create_on(&r.fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let warned = capture();
        unanswered(&r.backend, false);
        let refused = r.fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(gave_up_one(&refused.message), "{}", refused.message);
        drop(warned);
        assert_eq!(get(&r.fx.rt, &ws.view.id).await["phase"], "napping");
        // The provider's resume lands once nobody is waiting on it; the one late read is due 50 ms after the wake gave up.
        r.backend.machine(0).paused.set(false);
        r.fc.advance(50);
        until_for(2_000, &mut || {
            let (rt, id) = (r.fx.rt.clone(), ws.view.id.clone());
            async move { phase_is(&rt, &id, "running").await }
        })
        .await;
        assert_matches(&stored(&r.state.store, &ws.view.id).await, json!({ "phase": "running" }));
        assert!(r.events.has("workspace.woken"));
        assert_matches(
            pushes(&r.events).last().expect("a status"),
            json!({ "phase": "running", "machineState": "running", "reason": ALREADY_RUNNING }),
        );
        r.fx.rt.close().await.expect("a close");
    });
}

// "the runtime sends a pause once and never reads the machine inside it; a wake reads the machine before its resume and before a second ask, never inside a move"
#[test]
fn the_runtime_sends_a_pause_once_and_never_reads_the_machine_inside_it_a_wake_reads_the_machine_before_its_resume_and_before_a_second_ask_never_inside_a_move(
) {
    drive(|| async {
        let r = rig(None);
        r.backend.lifecycle().set_resume_asks(Some((60.0, 120.0)));
        let ws = create_on(&r.fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m = r.backend.machine(0);
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::default();
        let (real_state, real_pause, real_resume) = (m.state_fn(), m.pause_fn(), m.resume_with_fn());
        let noted = log.clone();
        m.set_state(Rc::new(move || {
            noted.borrow_mut().push("state");
            real_state()
        }));
        let noted = log.clone();
        m.set_pause(Rc::new(move || {
            noted.borrow_mut().push("pause");
            real_pause()
        }));
        let noted = log.clone();
        m.set_resume_with(Rc::new(move |signal| {
            noted.borrow_mut().push("resume");
            real_resume(signal)
        }));
        r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(*log.borrow(), ["pause"]);
        log.borrow_mut().clear();
        r.fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        // One read before the resume, which is the adopt check on a napping record; nothing between the resume and its outcome.
        assert_eq!(*log.borrow(), ["state", "resume"]);
        r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        log.borrow_mut().clear();
        // A resume the backend gave up on: the host reads the machine once before it asks again, never inside the move.
        let deaf = Rc::new(Cell::new(1i32));
        let (noted, left, machine) = (log.clone(), deaf.clone(), m.clone());
        m.set_resume(Rc::new(move || {
            noted.borrow_mut().push("resume");
            let still = left.get();
            left.set(still - 1);
            if still > 0 {
                return Box::pin(async { Err(EngineError::ResumeUnanswered { message: RESUME_UNANSWERED.into(), cause: None }) });
            }
            machine.paused.set(false);
            Box::pin(async { Ok(()) })
        }));
        let _warn = capture();
        let (rt, id) = (r.fx.rt.clone(), ws.view.id.clone());
        let waking = tokio::task::spawn_local(async move { rt.workspaces().wake(&id, None).await });
        let resumes = |log: &Rc<RefCell<Vec<&'static str>>>| log.borrow().iter().filter(|c| **c == "resume").count();
        until_n(|| resumes(&log) == 1).await;
        // The cadence is on the runtime's clock; a step taken before the timer exists is simply taken again.
        while resumes(&log) < 2 {
            r.fc.advance(60);
            sleep_ms(5).await;
        }
        assert_eq!(j(&waking.await.expect("a task").expect("a wake"))["phase"], "running");
        assert_eq!(*log.borrow(), ["state", "resume", "state", "resume"]);
        r.fx.rt.close().await.expect("a close");
    });
}
