// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/idle.test.ts at 65f6b89c7, the whole-runtime cases: "idle policy in the runtime" and "provider
//! backstop on the fork spec". The nine cases of "idle policy mechanics" drive `createIdlePolicy` alone, with no runtime,
//! and are the unit tests of `wsp-core/tests/idle.rs`.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::gone::{immediate, until_n};
use common::local::ws_request;
use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::idle::IDLE_OFF_BACKSTOP_MS;
use wsp_core::status::StatusWatchOptions;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::{IdleWindow, RuntimeOptions};
use wsp_engine::errors::EngineError;
use wsp_serve::serve::{serve_runtime, ServeOptions};
use wsp_serve::AssembledRuntime;
use wsp_wire::types::JsNumber;
use wsp_words::words::computer::{move_timed_out_line, Move};

/// Minutes, not milliseconds: the clock is fake, so a long window costs nothing and scheduling jitter cannot reach it.
const WINDOW: i64 = 5 * 60_000;

struct T {
    fx: Fx,
    backend: Arc<StubBackend>,
    fc: FakeClock,
    events: Events,
    _state: State,
}

fn test_runtime(adapter: Option<HarnessAdapterFactory>, window_ms: i64) -> T {
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let fc = FakeClock::at_now();
    let mut wiring = runtime(&backend).store(&state.store).clock(&fc).with(move |o: &mut RuntimeOptions| {
        o.idle_default_window_ms = Some(window_ms as u64);
        o.status = Some(StatusWatchOptions { cost_interval_ms: Some(60_000), poll_interval_ms: Some(60_000), ..Default::default() });
    });
    if let Some(adapter) = adapter {
        wiring = wiring.adapter("claude", adapter);
    }
    let fx = wiring.build();
    let events = fx.events();
    T { fx, backend, fc, events, _state: state }
}

async fn phase_of(rt: &Rc<AssembledRuntime>, id: &str) -> String {
    j(&rt.workspaces().get(id, None, None).await.expect("the workspace"))["phase"].as_str().unwrap_or_default().to_string()
}

async fn napping(rt: &Rc<AssembledRuntime>, id: &str) {
    until(|| {
        let (rt, id) = (rt.clone(), id.to_string());
        async move { phase_of(&rt, &id).await == "napping" }
    })
    .await;
}

async fn first_row(rt: &Rc<AssembledRuntime>) -> Value {
    status_rows(rt).await[0].clone()
}

/// `until(async () => (await rt.status.list())[0].idleAt === at)` and its other forms.
async fn until_row(rt: &Rc<AssembledRuntime>, holds: impl Fn(&Value) -> bool + 'static) {
    let holds = Rc::new(holds);
    until(|| {
        let (rt, holds) = (rt.clone(), holds.clone());
        async move { holds(&first_row(&rt).await) }
    })
    .await;
}

fn running(statuses: &[Value]) -> Vec<Value> {
    statuses.iter().filter(|s| s["phase"] == "running").cloned().collect()
}

fn quiet(ms: Option<u64>) -> common::stub_backend::QuietFn {
    Rc::new(move || Box::pin(async move { Ok(ms) }))
}

const GOLDEN: fn(&str) -> CreateOn = CreateOn::golden;

// "idle policy in the runtime" > "naps an untouched workspace after the window and pushes a status that says why"
#[test]
fn naps_an_untouched_workspace_after_the_window_and_pushes_a_status_that_says_why() {
    drive(|| async {
        let t = test_runtime(None, WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        let before = first_row(&t.fx.rt).await;
        assert_eq!(before["idleAt"], t.fc.now() + WINDOW);
        t.fc.advance(WINDOW - 1);
        assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "running");
        t.fc.advance(1);
        until_n(|| t.events.statuses().iter().any(|s| s["phase"] == "napping")).await;
        assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "napping");
        assert_eq!(t.events.of("workspace.napped").len(), 1);
        assert!(t.backend.machine(0).paused.get());
        let pushed = t.events.statuses().into_iter().find(|s| s["phase"] == "napping").expect("a napping status");
        assert_matches(&pushed, json!({ "id": ws.view.id, "machineState": "paused", "reach": { "state": "napping" } }));
        assert_eq!(pushed["reason"], "idle 5 min");
        assert!(pushed.get("idleAt").is_none());
        t.fc.advance(WINDOW * 2);
        assert_eq!(t.events.of("workspace.napped").len(), 1); // napping is not idle
        t.fx.rt.close().await.expect("a close");
    });
}

// "starts the window over instead of stopping a workspace its own computer says is busy"
#[test]
fn starts_the_window_over_instead_of_stopping_a_workspace_its_own_computer_says_is_busy() {
    drive(|| async {
        let t = test_runtime(None, WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        // The computer running it saw a byte through a published port a minute ago, which is inside the window: the
        // window starts over and nothing is stopped under the person clicking through it.
        t.backend.lifecycle().set_quiet_for_ms(Some(quiet(Some(60_000))));
        let before = first_row(&t.fx.rt).await;
        assert_eq!(before["idleAt"], t.fc.now() + WINDOW);
        t.fc.advance(WINDOW);
        let again = t.fc.clone();
        until_row(&t.fx.rt, move |r| r["idleAt"] == again.now() + WINDOW).await;
        assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "running");
        assert!(!t.backend.machine(0).paused.get());
        // And once that computer says it has been quiet for the whole window, the stop goes ahead.
        t.backend.lifecycle().set_quiet_for_ms(Some(quiet(Some(WINDOW as u64))));
        t.fc.advance(WINDOW);
        napping(&t.fx.rt, &ws.view.id).await;
        assert!(t.backend.machine(0).paused.get());
        t.fx.rt.close().await.expect("a close");
    });
}

// "reads the figure off a lifecycle that writes it as a method, which keeps that lifecycle's own fields"
// The TS pulls `quietForMs` off the interface and calls it detached, which loses `this`; a Rust trait method always has its
// `self`, so the lifecycle's own field is the figure a closure holds and the case reads it the same way.
#[test]
fn reads_the_figure_off_a_lifecycle_that_writes_it_as_a_method_which_keeps_that_lifecycles_own_fields() {
    drive(|| async {
        let t = test_runtime(None, WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        // A lifecycle written as a method reading a field of its own: the workspace stops under somebody using it if the
        // read of that field answers nothing.
        let figure = Rc::new(Cell::new(60_000u64));
        let own = figure.clone();
        t.backend.lifecycle().set_quiet_for_ms(Some(Rc::new(move || {
            let said = own.get();
            Box::pin(async move { Ok(Some(said)) })
        })));
        t.fc.advance(WINDOW);
        let again = t.fc.clone();
        until_row(&t.fx.rt, move |r| r["idleAt"] == again.now() + WINDOW).await;
        assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "running");
        assert!(!t.backend.machine(0).paused.get());
        // And the same lifecycle once its own figure is past the window: the stop goes ahead.
        figure.set(WINDOW as u64);
        t.fc.advance(WINDOW);
        napping(&t.fx.rt, &ws.view.id).await;
        assert!(t.backend.machine(0).paused.get());
        t.fx.rt.close().await.expect("a close");
    });
}

// "stops a workspace whose computer cannot say how quiet it is, and one whose reading fails"
#[test]
fn stops_a_workspace_whose_computer_cannot_say_how_quiet_it_is_and_one_whose_reading_fails() {
    drive(|| async {
        let warned = capture();
        // Every provider: nothing above the daemon on a computer somebody owns counts a byte on a port, so the host's own
        // clock is the whole policy there, as it was before the figure existed.
        let t = test_runtime(None, WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        t.backend.lifecycle().set_quiet_for_ms(Some(quiet(None)));
        t.fc.advance(WINDOW);
        napping(&t.fx.rt, &ws.view.id).await;
        assert!(t.backend.machine(0).paused.get());

        // A reading that fails says nothing about the workspace, so the stop goes ahead rather than a workspace living for
        // ever behind a computer that will not answer.
        let second = test_runtime(None, WINDOW);
        let other = create_on(&second.fx.rt, GOLDEN("b"), None).await.expect("a workspace");
        second.backend.lifecycle().set_quiet_for_ms(Some(Rc::new(|| {
            Box::pin(async { Err(EngineError::Other { message: "the computer did not answer".into(), code: None }) })
        })));
        second.fc.advance(WINDOW);
        napping(&second.fx.rt, &other.view.id).await;
        assert!(second.backend.machine(0).paused.get());
        assert!(warned.lines().join(" ").contains("quiet figure of"), "{:?}", warned.lines());
        t.fx.rt.close().await.expect("a close");
        second.fx.rt.close().await.expect("a close");
    });
}

// "names the window in minutes the way the rail will show it"
#[test]
fn names_the_window_in_minutes_the_way_the_rail_will_show_it() {
    drive(|| async {
        let t = test_runtime(None, 20 * 60_000);
        create_on(
            &t.fx.rt,
            CreateOn { idle_window_ms: IdleWindow::Ms(JsNumber(3.0 * 60_000.0)), ..GOLDEN("a") },
            None,
        )
        .await
        .expect("a workspace");
        t.fc.advance(3 * 60_000);
        until_n(|| t.events.statuses().iter().any(|s| s["phase"] == "napping")).await;
        assert_eq!(t.events.statuses().last().expect("a status")["reason"], "idle 3 min");
        assert!(first_row(&t.fx.rt).await.get("idleAt").is_none());
        t.fx.rt.close().await.expect("a close");
    });
}

// "workspaces.touch over the wire starts the window over"
#[test]
fn workspaces_touch_over_the_wire_starts_the_window_over() {
    drive(|| async {
        let t = test_runtime(None, WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        let served = serve_runtime(t.fx.rt.clone(), ServeOptions { port: Some(0), auth_token: "secret".into(), ..Default::default() })
            .await
            .expect("the runtime serves");
        for _ in 0..8 {
            t.fc.advance(WINDOW / 4);
            let res = ws_request(served.port(), "secret", "workspaces.touch", json!({ "workspaceId": ws.view.id })).await;
            assert_eq!(res["ok"], true, "{res}");
        }
        assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "running");
        assert_eq!(first_row(&t.fx.rt).await["idleAt"], t.fc.now() + WINDOW);
        t.fc.advance(WINDOW);
        napping(&t.fx.rt, &ws.view.id).await;
        let missing = t.fx.rt.workspaces().touch("ws_nope", None).await.expect_err("a refusal").message;
        assert!(missing.contains("no such workspace"), "{missing}");
        served.close().await.expect("a close");
        t.fx.rt.close().await.expect("a close");
    });
}

// "a running session holds the workspace awake; the window starts when it ends"
#[test]
fn a_running_session_holds_the_workspace_awake_the_window_starts_when_it_ends() {
    drive(|| async {
        let held = Held::new(false);
        let t = test_runtime(Some(held.factory()), WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        t.fx.rt.sessions().start(&ws.view.id, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.expect("a start");
        t.fc.advance(WINDOW * 3);
        assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "running");
        assert!(first_row(&t.fx.rt).await.get("idleAt").is_none());
        held.end(0, "", json!({}));
        until_row(&t.fx.rt, |r| r.get("idleAt").is_some()).await;
        assert_eq!(first_row(&t.fx.rt).await["idleAt"], t.fc.now() + WINDOW);
        t.fc.advance(WINDOW);
        napping(&t.fx.rt, &ws.view.id).await;
        t.fx.rt.close().await.expect("a close");
    });
}

async fn a_turn_started_on_a_machine_woken_by(road: &str) {
    let held = Held::new(false);
    let t = test_runtime(Some(held.factory()), WINDOW);
    let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
    t.fc.advance(WINDOW);
    napping(&t.fx.rt, &ws.view.id).await;
    if road == "found running by the wake" {
        t.backend.machine(0).paused.set(false);
    }
    assert_eq!(j(&t.fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
    t.fx.rt.sessions().start(&ws.view.id, StartOptions { prompt: "review".into(), ..Default::default() }, None).await.expect("a start");
    t.fc.advance(WINDOW * 3);
    immediate().await;
    assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "running");
    assert!(!t.backend.machine(0).paused.get());
    held.end(0, "", json!({}));
    until_row(&t.fx.rt, |r| r.get("idleAt").is_some()).await;
    t.fc.advance(WINDOW);
    napping(&t.fx.rt, &ws.view.id).await;
    t.fx.rt.close().await.expect("a close");
}

// "a turn started on a machine resumed by the wake holds it awake for as long as it runs"
#[test]
fn a_turn_started_on_a_machine_resumed_by_the_wake_holds_it_awake_for_as_long_as_it_runs() {
    drive(|| a_turn_started_on_a_machine_woken_by("resumed by the wake"));
}

// "a turn started on a machine found running by the wake holds it awake for as long as it runs"
#[test]
fn a_turn_started_on_a_machine_found_running_by_the_wake_holds_it_awake_for_as_long_as_it_runs() {
    drive(|| a_turn_started_on_a_machine_woken_by("found running by the wake"));
}

// "off means never; a per-workspace window beats the default"
#[test]
fn off_means_never_a_per_workspace_window_beats_the_default() {
    drive(|| async {
        let t = test_runtime(None, 20 * 60_000);
        let off = create_on(&t.fx.rt, CreateOn { idle_window_ms: IdleWindow::Off, ..GOLDEN("off") }, None).await.expect("a workspace");
        let fast = create_on(&t.fx.rt, CreateOn { idle_window_ms: IdleWindow::Ms(JsNumber(WINDOW as f64)), ..GOLDEN("fast") }, None)
            .await
            .expect("a workspace");
        t.fc.advance(WINDOW);
        napping(&t.fx.rt, &fast.view.id).await;
        t.fc.advance(40 * 60_000);
        assert_eq!(phase_of(&t.fx.rt, &off.view.id).await, "running");
        let rows = status_rows(&t.fx.rt).await;
        let by_name = rows.iter().find(|s| s["name"] == "off").expect("off's row");
        assert!(by_name.get("idleAt").is_none());
        t.fx.rt.close().await.expect("a close");
    });
}

// "wake and upgrade start the window over; a rehydrated running workspace gets one too"
#[test]
fn wake_and_upgrade_start_the_window_over_a_rehydrated_running_workspace_gets_one_too() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fc = FakeClock::at_now();
        let with_window = |o: &mut RuntimeOptions| o.idle_default_window_ms = Some(WINDOW as u64);
        let fx = runtime(&backend).store(&state.store).clock(&fc).with(with_window).build();
        let rt = fx.rt.clone();
        let ws = create_on(&rt, GOLDEN("a"), None).await.expect("a workspace");
        fc.advance(WINDOW);
        napping(&rt, &ws.view.id).await;
        rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        fc.advance(WINDOW);
        napping(&rt, &ws.view.id).await;
        rt.workspaces().upgrade(&ws.view.id, None).await.expect("an upgrade");
        assert_eq!(phase_of(&rt, &ws.view.id).await, "running");
        fc.advance(WINDOW);
        napping(&rt, &ws.view.id).await;
        rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        rt.close().await.expect("a close");

        let fx2 = runtime(&backend).store(&state.store).clock(&fc).with(with_window).build();
        let rt2 = fx2.rt.clone();
        assert_eq!(phase_of(&rt2, &ws.view.id).await, "running");
        fc.advance(WINDOW);
        napping(&rt2, &ws.view.id).await;
        rt2.close().await.expect("a close");
    });
}

// "an idle nap the provider refuses is not asked again at the cadence: the row keeps the words and one full window starts from the answer"
#[test]
fn an_idle_nap_the_provider_refuses_is_not_asked_again_at_the_cadence_the_row_keeps_the_words_and_one_full_window_starts_from_the_answer() {
    drive(|| async {
        let t = test_runtime(None, WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        let m = t.backend.machine(0);
        let pause = m.pause_fn();
        let calls = Rc::new(Cell::new(0u32));
        let refusing = Rc::new(Cell::new(true));
        let (counted, refused) = (calls.clone(), refusing.clone());
        m.set_pause(Rc::new(move || {
            counted.set(counted.get() + 1);
            if refused.get() {
                return Box::pin(async { Err(EngineError::Other { message: "upstream request timeout".into(), code: None }) });
            }
            pause()
        }));
        let since = t.events.statuses().len();
        let warned = capture();
        t.fc.advance(WINDOW);
        until_n(|| running(&t.events.statuses()[since..]).iter().any(|s| s["idleAt"] == t.fc.now() + WINDOW)).await;
        drop(warned);
        let answered = t.fc.now();
        assert_eq!(calls.get(), 1);
        let last = running(&t.events.statuses()[since..]).pop().expect("a running status");
        assert_eq!(last["reason"], "upstream request timeout");
        assert_eq!(last["idleAt"], answered + WINDOW);
        assert_eq!(first_row(&t.fx.rt).await["idleAt"], answered + WINDOW);
        t.fc.advance(WINDOW - 1);
        immediate().await;
        assert_eq!(calls.get(), 1);
        refusing.set(false);
        t.fc.advance(1);
        napping(&t.fx.rt, &ws.view.id).await;
        assert_eq!(calls.get(), 2);
        assert_matches(t.events.statuses().last().expect("a status"), json!({ "phase": "napping", "reason": "idle 5 min" }));
        t.fx.rt.close().await.expect("a close");
    });
}

// "an idle nap whose pause the backend gave up on keeps the deadline on the row with the backend's words, and is asked again at the cadence until one lands"
#[test]
fn an_idle_nap_whose_pause_the_backend_gave_up_on_keeps_the_deadline_on_the_row_with_the_backends_words_and_is_asked_again_at_the_cadence_until_one_lands() {
    drive(|| async {
        let t = test_runtime(None, WINDOW);
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        let deadline = first_row(&t.fx.rt).await["idleAt"].clone();
        let m = t.backend.machine(0);
        let pause = m.pause_fn();
        let words = move_timed_out_line(Move::Pause, 20.0, Some("running"));
        let calls = Rc::new(Cell::new(0u32));
        let unanswered = Rc::new(Cell::new(2i32));
        let (counted, left, said) = (calls.clone(), unanswered.clone(), words.clone());
        m.set_pause(Rc::new(move || {
            counted.set(counted.get() + 1);
            let still = left.get();
            left.set(still - 1);
            if still > 0 {
                let cause = EngineError::MoveUnanswered { message: said.clone(), cause: None };
                return Box::pin(async move { Err(cause) });
            }
            pause()
        }));
        let since = t.events.statuses().len();
        let warned = capture();
        t.fc.advance(WINDOW);
        until_n(|| !running(&t.events.statuses()[since..]).is_empty()).await;
        let failed = running(&t.events.statuses()[since..]).pop().expect("a running status");
        // The row carries the backend's own words and the deadline it had, so it never reads active.
        assert_eq!(failed["reason"], words.as_str());
        assert_eq!(failed["idleAt"], deadline);
        assert_eq!(calls.get(), 1);
        assert_eq!(phase_of(&t.fx.rt, &ws.view.id).await, "running");
        // /^idle nap of ws_\w+ failed: pause did not complete/
        assert!(
            warned.lines().iter().any(|l| l
                .strip_prefix("idle nap of ws_")
                .and_then(|rest| rest.split_once(" failed: pause did not complete"))
                .is_some_and(|(id, _)| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))),
            "{:?}",
            warned.lines()
        );
        t.fc.advance(60_000 - 1);
        assert_eq!(calls.get(), 1);
        t.fc.advance(1);
        until_n(|| running(&t.events.statuses()[since..]).len() > 1).await;
        assert_eq!(running(&t.events.statuses()[since..]).last().expect("a status")["idleAt"], deadline);
        assert_eq!(calls.get(), 2);
        t.fc.advance(60_000);
        napping(&t.fx.rt, &ws.view.id).await;
        drop(warned);
        assert_eq!(calls.get(), 3);
        assert_matches(t.events.statuses().last().expect("a status"), json!({ "phase": "napping", "reason": "idle 5 min" }));
        assert!(running(&t.events.statuses()[since..]).iter().all(|s| s["idleAt"] == deadline));
        t.fc.advance(WINDOW * 2);
        assert_eq!(calls.get(), 3);
        t.fx.rt.close().await.expect("a close");
    });
}

// "provider backstop on the fork spec" > "pauses on idle at twice the policy window"
#[test]
fn pauses_on_idle_at_twice_the_policy_window() {
    drive(|| async {
        let t = test_runtime(None, 20 * 60_000);
        create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        assert_matches(&j(&t.backend.machine(0).spec_now()), json!({ "onIdle": "pause", "idleTimeoutMs": 40 * 60_000 }));
        t.fx.rt.close().await.expect("a close");
    });
}

// "follows the per-workspace window, and a rebuilt fork carries it too"
#[test]
fn follows_the_per_workspace_window_and_a_rebuilt_fork_carries_it_too() {
    drive(|| async {
        let t = test_runtime(None, 20 * 60_000);
        let ws = create_on(&t.fx.rt, CreateOn { idle_window_ms: IdleWindow::Ms(JsNumber(5.0 * 60_000.0)), ..GOLDEN("a") }, None)
            .await
            .expect("a workspace");
        assert_matches(&j(&t.backend.machine(0).spec_now()), json!({ "onIdle": "pause", "idleTimeoutMs": 10 * 60_000 }));
        t.fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_matches(&j(&t.backend.machine(1).spec_now()), json!({ "onIdle": "pause", "idleTimeoutMs": 10 * 60_000 }));
        t.fx.rt.close().await.expect("a close");
    });
}

// "a backend declaring a backstop is handed the machine and the instant on create and on activity, nothing on nap, the six-hour instant with auto-nap off, and a call that fails is logged"
#[test]
fn a_backend_declaring_a_backstop_is_handed_the_machine_and_the_instant_on_create_and_on_activity_nothing_on_nap_the_six_hour_instant_with_auto_nap_off_and_a_call_that_fails_is_logged() {
    drive(|| async {
        let t = test_runtime(None, 20 * 60_000);
        let heard: Rc<RefCell<Vec<(String, i64)>>> = Rc::default();
        let refuse = Rc::new(Cell::new(false));
        let (seen, refusing) = (heard.clone(), refuse.clone());
        t.backend.lifecycle().set_backstop(Some(Rc::new(move |machine, until| {
            if refusing.get() {
                return Box::pin(async { Err(EngineError::Other { message: "PATCH ttlSeconds refused".into(), code: None }) });
            }
            seen.borrow_mut().push((machine, until));
            Box::pin(async { Ok(()) })
        })));
        let t0 = t.fc.now();
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        assert_eq!(*heard.borrow(), [("m1".to_string(), t0 + 40 * 60_000)]);
        t.fc.advance(5_000);
        t.fx.rt.workspaces().touch(&ws.view.id, None).await.expect("a touch");
        assert_eq!(*heard.borrow(), [("m1".to_string(), t0 + 40 * 60_000), ("m1".to_string(), t0 + 5_000 + 40 * 60_000)]);
        t.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(heard.borrow().len(), 2);
        // A client touching a napped workspace arms nothing at the backend: its machine is stopped, and on a provider
        // whose backstop is a stop timer that call would land on an archived machine.
        t.fx.rt.workspaces().touch(&ws.view.id, None).await.expect("a touch");
        t.fc.advance(1_000);
        t.fx.rt.workspaces().touch(&ws.view.id, None).await.expect("a touch");
        assert_eq!(heard.borrow().len(), 2);
        // With auto-nap off the runtime still hands over its six-hour instant, so a dead host never leaves a machine billing.
        let off = create_on(&t.fx.rt, CreateOn { idle_window_ms: IdleWindow::Off, ..GOLDEN("b") }, None).await.expect("a workspace");
        assert_eq!(heard.borrow().last().cloned(), Some(("m2".to_string(), t.fc.now() + IDLE_OFF_BACKSTOP_MS)));
        // A call the provider refuses is logged, and the window is armed all the same.
        refuse.set(true);
        let warned = capture();
        t.fx.rt.workspaces().touch(&off.view.id, None).await.expect("a touch");
        immediate().await;
        // /^backstop of ws_\w+ on m2 not set: PATCH ttlSeconds refused$/
        assert!(
            warned.lines().iter().any(|l| l
                .strip_prefix("backstop of ws_")
                .and_then(|rest| rest.split_once(" on m2 not set: PATCH ttlSeconds refused"))
                .is_some_and(|(id, tail)| tail.is_empty() && !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))),
            "{:?}",
            warned.lines()
        );
        drop(warned);
        let rows = status_rows(&t.fx.rt).await;
        assert!(rows.iter().find(|s| s["id"] == off.view.id.as_str()).expect("a row").get("idleAt").is_none());
        assert_eq!(heard.borrow().len(), 3);
        t.fx.rt.close().await.expect("a close");
    });
}

// "a backend without a backstop is never asked: the window arms as before"
#[test]
fn a_backend_without_a_backstop_is_never_asked_the_window_arms_as_before() {
    use wsp_engine::machine::Lifecycle as _;
    drive(|| async {
        let t = test_runtime(None, WINDOW);
        assert!(!t.backend.lifecycle().has_backstop());
        let ws = create_on(&t.fx.rt, GOLDEN("a"), None).await.expect("a workspace");
        t.fx.rt.workspaces().touch(&ws.view.id, None).await.expect("a touch");
        assert_eq!(first_row(&t.fx.rt).await["idleAt"], t.fc.now() + WINDOW);
        t.fx.rt.close().await.expect("a close");
    });
}

// "off still leaves a long pause backstop so a crashed runtime stops billing"
#[test]
fn off_still_leaves_a_long_pause_backstop_so_a_crashed_runtime_stops_billing() {
    drive(|| async {
        let t = test_runtime(None, 20 * 60_000);
        create_on(&t.fx.rt, CreateOn { idle_window_ms: IdleWindow::Off, ..GOLDEN("a") }, None).await.expect("a workspace");
        assert_matches(&j(&t.backend.machine(0).spec_now()), json!({ "onIdle": "pause", "idleTimeoutMs": IDLE_OFF_BACKSTOP_MS }));
        assert!(IDLE_OFF_BACKSTOP_MS >= 60 * 60_000);
        t.fx.rt.close().await.expect("a close");
    });
}
