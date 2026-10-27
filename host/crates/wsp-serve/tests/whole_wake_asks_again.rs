// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/wake-asks-again.test.ts at 65f6b89c7, case for case. A wake the provider never takes: the
//! backend's resume ends unanswered every time, which is what Solari's did for every paused machine on 2026-09-10.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::gone::until_n;
use common::stub_backend::aborted_call;
use common::*;
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_engine::errors::EngineError;
use wsp_engine::solari_backend::{solari_lifecycle_budgets, RESUME_CAP_MS};
use wsp_store::store::Store;
use wsp_wire::views::workspace::{MachineState, ReachState, WorkspacePhase};
use wsp_words::words::computer::{wake_asking_again_line, wake_asks_in, LineStyle, RESUME_UNANSWERED, WAKE_STOPPED};
use wsp_words::workspace_state::{needs_rebuild, WorkspaceStateInput};

/// The cadence between asks, shaped like the shipped one and short enough to count in whole ticks. The cap and the
/// read after it are the backend's own, so the fake provider is the one that ends a call, when the test says so.
const EVERY: i64 = 60;

/// `^the provider answered none of \d+ resume requests over `: the head of the line a run that gave up ends with; the
/// test steps the clock in whole cadences, so the span it names is not exact.
fn gave_up(text: &str) -> bool {
    text.strip_prefix("the provider answered none of ")
        .and_then(|rest| rest.split_once(" resume requests over "))
        .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// What a test says of the backend's asking: absent keeps the stub's cloud-provider numbers, `None` declares no asking
/// at all (the host asks once and stops), `For` the cadence `EVERY` for as long as given, `Is` both.
enum Asks {
    Keep,
    None,
    For(i64),
    Is(f64, f64),
}

struct Rig {
    backend: Arc<StubBackend>,
    store: Rc<dyn Store>,
    fc: FakeClock,
    fx: Fx,
    events: Events,
    _state: State,
}

impl Rig {
    fn rows(&self) -> Vec<Value> {
        self.events.statuses()
    }
}

/// A runtime over a stub whose backend asks again at the cadence given, for as long as given.
fn rig(asks: Asks) -> Rig {
    let backend = StubBackend::new(None);
    match asks {
        Asks::Keep => {}
        Asks::None => backend.lifecycle().set_resume_asks(None),
        Asks::For(for_ms) => backend.lifecycle().set_resume_asks(Some((EVERY as f64, for_ms as f64))),
        Asks::Is(every, for_ms) => backend.lifecycle().set_resume_asks(Some((every, for_ms))),
    }
    let state = sqlite_state();
    let fc = FakeClock::at_now();
    let fx = runtime(&backend).store(&state.store).clock(&fc).build();
    let events = fx.events();
    Rig { backend, store: state.store.clone(), fc, fx, events, _state: state }
}

/// The provider as the probe found it: the resume is taken and never answered, until the backend's cap cuts it off and
/// its read still finds the machine paused, or the caller's own stop ends it. `cap()` ends whatever call is in flight
/// the way the backend ends one it gave up on, with the typed error; `takes` lets a later call land at once.
struct Deaf {
    calls: Cell<u32>,
    takes: Cell<bool>,
    cap_tx: RefCell<Option<oneshot::Sender<()>>>,
}

impl Deaf {
    fn cap(&self) {
        if let Some(tx) = self.cap_tx.borrow_mut().take() {
            let _ = tx.send(());
        }
    }
}

fn deaf_resume(machine: &Arc<StubMachine>) -> Rc<Deaf> {
    let deaf = Rc::new(Deaf { calls: Cell::new(0), takes: Cell::new(false), cap_tx: RefCell::new(None) });
    let state = deaf.clone();
    let m = machine.clone();
    machine.set_resume_with(Rc::new(move |signal| {
        state.calls.set(state.calls.get() + 1);
        if state.takes.get() {
            m.paused.set(false);
            return Box::pin(async { Ok(()) });
        }
        let (tx, rx) = oneshot::channel();
        *state.cap_tx.borrow_mut() = Some(tx);
        let id = m.id().to_string();
        Box::pin(async move {
            let capped = async {
                if rx.await.is_err() {
                    std::future::pending::<()>().await;
                }
            };
            let aborted = async {
                match signal {
                    Some(signal) => signal.cancelled().await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                _ = capped => Err(EngineError::ResumeUnanswered { message: RESUME_UNANSWERED.into(), cause: None }),
                _ = aborted => Err(aborted_call(&format!("resume of {id}"))),
            }
        })
    }));
    deaf
}

/// Fires the cap on the call in flight and moves the clock a cadence, over and over until cond holds, so the test never
/// advances past a timer the runtime has not scheduled yet; fake time costs nothing, so a step taken early is simply
/// taken again.
async fn tick(fc: &FakeClock, deaf: &Deaf, mut cond: impl FnMut() -> bool, step: i64) {
    let deadline = Instant::now() + Duration::from_millis(2_000);
    while !cond() {
        assert!(Instant::now() <= deadline, "the runtime did not get there in time");
        deaf.cap();
        sleep_ms(5).await;
        fc.advance(step);
        sleep_ms(5).await;
    }
}

/// When each resume was sent, on the runtime's own clock.
fn times_resumes(machine: &Arc<StubMachine>, fc: &FakeClock) -> Rc<RefCell<Vec<i64>>> {
    let at: Rc<RefCell<Vec<i64>>> = Rc::default();
    let resume = machine.resume_with_fn();
    let noted = at.clone();
    let clock = fc.clone();
    machine.set_resume_with(Rc::new(move |signal| {
        noted.borrow_mut().push(clock.now());
        resume(signal)
    }));
    at
}

/// The wake's outcome as a value, watched from the instant it starts so a rejection is never left unheld while the test
/// moves the clock; `ended` says when the clock has been moved far enough for the wake to be over.
struct Watched {
    handle: JoinHandle<Result<Value, String>>,
    over: Rc<Cell<bool>>,
}

impl Watched {
    fn ended(&self) -> bool {
        self.over.get()
    }
    /// `"woke"` or the refusal's message.
    async fn outcome(self) -> String {
        match self.result().await {
            Ok(_) => "woke".to_string(),
            Err(message) => message,
        }
    }
    async fn result(self) -> Result<Value, String> {
        self.handle.await.expect("the wake task")
    }
}

fn waking(fx: &Fx, id: &str) -> Watched {
    let rt = fx.rt.clone();
    let id = id.to_string();
    let over = Rc::new(Cell::new(false));
    let done = over.clone();
    let handle = tokio::task::spawn_local(async move {
        let outcome = rt.workspaces().wake(&id, None).await.map(|v| j(&v)).map_err(|e| e.message);
        done.set(true);
        outcome
    });
    Watched { handle, over }
}

async fn napping_on_a_deaf_provider(r: &Rig) -> (String, Arc<StubMachine>, Rc<Deaf>) {
    let ws = create_on(&r.fx.rt, CreateOn::golden("b1"), None).await.expect("a workspace");
    r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
    let m = r.backend.machine(0);
    let deaf = deaf_resume(&m);
    (ws.view.id, m, deaf)
}

fn ask_of(row: &Value) -> Option<u64> {
    row["wakeAsk"]["ask"].as_u64()
}

// "a wake the provider does not answer" > "says the provider has not answered and that the machine is being read, as soon as the backend gives the resume up"
#[test]
fn says_the_provider_has_not_answered_and_that_the_machine_is_being_read_as_soon_as_the_backend_gives_the_resume_up() {
    drive(|| async {
        let r = rig(Asks::For(2 * EVERY));
        let (id, _m, deaf) = napping_on_a_deaf_provider(&r).await;
        let _warn = capture();
        let began = r.fc.now();
        let watched = waking(&r.fx, &id);
        until_n(|| deaf.calls.get() == 1).await;
        // The runtime holds no timer on the call: the backend's own end of it is what fires here, and the runtime's
        // clock has not moved, so the row speaks the moment the backend gives up and on nothing of the runtime's.
        deaf.cap();
        until_n(|| r.rows().iter().any(|s| s["reason"] == RESUME_UNANSWERED)).await;
        assert_eq!(r.fc.now(), began);
        assert_eq!(r.rows().iter().find(|s| s["reason"] == RESUME_UNANSWERED).expect("a row")["phase"], "waking");
        tick(&r.fc, &deaf, || watched.ended(), EVERY).await;
        let outcome = watched.outcome().await;
        assert!(gave_up(&outcome), "{outcome}");
        assert_eq!(deaf.calls.get(), 2);
        r.fx.rt.close().await.expect("a close");
    });
}

// "asks again on its own once a cadence, counting the asks on the row, and stops when the asking is spent"
#[test]
fn asks_again_on_its_own_once_a_cadence_counting_the_asks_on_the_row_and_stops_when_the_asking_is_spent() {
    drive(|| async {
        let r = rig(Asks::For(4 * EVERY));
        let (id, _m, deaf) = napping_on_a_deaf_provider(&r).await;
        let _warn = capture();
        let watched = waking(&r.fx, &id);
        tick(&r.fc, &deaf, || watched.ended(), EVERY).await;
        let outcome = watched.outcome().await;
        assert!(gave_up(&outcome), "{outcome}");
        // Four cadences of asking hold four asks: the person's, then three of the host's own, each counted on the row.
        assert_eq!(deaf.calls.get(), 4);
        // The numbers ride, so the row and the Machine tab each read them at the length they have room for.
        let mut seen: Vec<String> = vec![];
        for row in r.rows() {
            if row.get("wakeAsk").is_some() {
                let line = format!("{}/{}", row["wakeAsk"]["ask"], row["wakeAsk"]["of"]);
                if !seen.contains(&line) {
                    seen.push(line);
                }
            }
        }
        assert_eq!(seen, ["1/4", "2/4", "3/4"]);
        assert_eq!(wake_asking_again_line(3.0, 4.0, None), "waking, asking again (3 of 4)");
        assert_eq!(wake_asking_again_line(3.0, 4.0, Some(LineStyle::Short)), "asking 3/4");
        // The row is a paused workspace again, with the provider's silence as its last word.
        assert_eq!(j(&r.fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"))["phase"], "napping");
        let last = r.rows().last().cloned().expect("a row");
        assert_eq!(last["phase"], "napping");
        assert!(gave_up(last["reason"].as_str().expect("a reason")), "{last}");
        r.fx.rt.close().await.expect("a close");
    });
}

// "counts the cadence from when the ask began, so a cap that eats half of it does not push the next ask out"
#[test]
fn counts_the_cadence_from_when_the_ask_began_so_a_cap_that_eats_half_of_it_does_not_push_the_next_ask_out() {
    drive(|| async {
        let r = rig(Asks::For(3 * EVERY));
        let (id, m, deaf) = napping_on_a_deaf_provider(&r).await;
        let at = times_resumes(&m, &r.fc);
        let _warn = capture();
        let began = r.fc.now();
        let watched = waking(&r.fx, &id);
        until_n(|| deaf.calls.get() == 1).await;
        // The provider sits on the call for half the cadence and the cap cuts it off there, which is the shipped half
        // minute of a whole minute.
        r.fc.advance(EVERY / 2);
        deaf.cap();
        until_n(|| r.rows().iter().any(|s| ask_of(s) == Some(1))).await;
        // What is left of the cadence is all the host waits: the minute runs from the ask, not from its cap.
        until_n(|| r.fc.due_for(began + EVERY)).await;
        r.fc.advance(EVERY / 2);
        until_n(|| deaf.calls.get() == 2).await;
        assert_eq!(at.borrow().iter().map(|t| t - began).collect::<Vec<_>>(), [0, EVERY]);
        tick(&r.fc, &deaf, || watched.ended(), EVERY).await;
        let outcome = watched.outcome().await;
        assert!(gave_up(&outcome), "{outcome}");
        // Three cadences of asking hold three asks, each a cadence after the one before it began.
        let first = at.borrow()[0];
        assert_eq!(at.borrow().iter().map(|t| t - first).collect::<Vec<_>>(), [0, EVERY, 2 * EVERY]);
        r.fx.rt.close().await.expect("a close");
    });
}

// "gives up half an hour after the first ask, having asked once a minute, with a cap that always fires"
#[test]
fn gives_up_half_an_hour_after_the_first_ask_having_asked_once_a_minute_with_a_cap_that_always_fires() {
    drive(|| async {
        // The cloud provider's own asking, read off its declaration and not off a number of the runtime's.
        let asks = solari_lifecycle_budgets().resume_asks.expect("the provider's asking");
        let (every, for_ms) = (asks.every_ms.0, asks.for_ms.0);
        let r = rig(Asks::Is(every, for_ms));
        let (id, m, deaf) = napping_on_a_deaf_provider(&r).await;
        let at = times_resumes(&m, &r.fc);
        let _warn = capture();
        let began = r.fc.now();
        let watched = waking(&r.fx, &id);
        // One minute of wall time a turn, spent as the live road spends it: the provider takes the resume, answers
        // nothing for the half minute the engine's cap gives it, and the host has the rest of the minute to itself.
        let mut ask = 1u32;
        while !watched.ended() && f64::from(ask) <= 2.0 * (for_ms / every) {
            until_n(|| deaf.calls.get() == ask).await;
            r.fc.advance(RESUME_CAP_MS as i64);
            deaf.cap();
            until_n(|| r.fc.due_for(began + i64::from(ask) * every as i64) || watched.ended()).await;
            r.fc.advance(every as i64 - RESUME_CAP_MS as i64);
            ask += 1;
        }
        let ended = watched.outcome().await;
        // Half an hour of asking at one ask a minute is thirty asks, the last of them starting a minute inside the half
        // hour; the old count of thirty asks each waiting out its own cap spanned three quarters of an hour.
        assert_eq!(f64::from(deaf.calls.get()), wake_asks_in(for_ms, every));
        assert_eq!(deaf.calls.get(), 30);
        let first = at.borrow()[0];
        assert_eq!(at.borrow().iter().map(|t| (t - first) / every as i64).collect::<Vec<_>>(), (0..30).collect::<Vec<i64>>());
        assert!((*at.borrow().last().expect("an ask") - first) < for_ms as i64);
        // The line counts the asks made and the wall time they took, which is the last cap firing inside the half hour.
        assert!(ended.contains("none of 30 resume requests over 29m 30s"), "{ended}");
        r.fx.rt.close().await.expect("a close");
    });
}

// "reads before it asks again, sends no second resume at one that came up, and still checks the guest and ends first life"
#[test]
fn reads_before_it_asks_again_sends_no_second_resume_at_one_that_came_up_and_still_checks_the_guest_and_ends_first_life() {
    drive(|| async {
        let r = rig(Asks::For(30 * EVERY));
        let ws = create_on(&r.fx.rt, CreateOn::golden("b1"), None).await.expect("a workspace");
        assert_matches(&r.store.get("workspaces", &ws.view.id).await.expect("a read").expect("a record"), json!({ "firstLife": true }));
        r.fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let m = r.backend.machine(0);
        let deaf = deaf_resume(&m);
        // The wake check reads the machine's shape; counting that read is how a wake that skipped the check is caught.
        let described = m.calls.describe.get();
        let _warn = capture();
        let watched = waking(&r.fx, &ws.view.id);
        until_n(|| deaf.calls.get() == 1).await;
        deaf.cap();
        until_n(|| r.rows().iter().any(|s| ask_of(s) == Some(1))).await;
        // The call that hung at the provider goes through while the host waits out its cadence, as the probe measured a
        // pause doing: no answer in 30 s, the machine moved 40 s later.
        m.paused.set(false);
        tick(&r.fc, &deaf, || watched.ended(), EVERY).await;
        let woken = watched.outcome().await;
        assert_eq!(woken, "woke");
        assert_eq!(deaf.calls.get(), 1);
        assert_eq!(j(&r.fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"))["phase"], "running");
        // The same road as a resume whose call answered: the guest was checked, and a machine that has been resumed is
        // out of first life, which is the fence the snapshot rule stands on.
        assert_eq!(m.calls.describe.get() - described, 1);
        assert_matches(&r.store.get("workspaces", &ws.view.id).await.expect("a read").expect("a record"), json!({ "firstLife": false }));
        r.fx.rt.close().await.expect("a close");
    });
}

// "leaves the rebuild road on the record when the asking runs out, and a wake that lands takes it away"
#[test]
fn leaves_the_rebuild_road_on_the_record_when_the_asking_runs_out_and_a_wake_that_lands_takes_it_away() {
    drive(|| async {
        let r = rig(Asks::For(3 * EVERY));
        let (id, m, deaf) = napping_on_a_deaf_provider(&r).await;
        let _warn = capture();
        let watched = waking(&r.fx, &id);
        tick(&r.fc, &deaf, || watched.ended(), EVERY).await;
        let outcome = watched.outcome().await;
        assert!(gave_up(&outcome), "{outcome}");
        let left = j(&r.fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"));
        let refused = left["wakeRefused"].as_str().expect("the road on the record").to_string();
        // One ask of the person's and two of the host's, and the record keeps the whole sentence: what the provider did,
        // where the work is, and the road to a machine now.
        assert!(refused.contains("none of 3 resume requests"), "{refused}");
        assert!(refused.contains("the work on this machine's disk stays with the provider"), "{refused}");
        assert!(refused.contains("a rebuild starts a new machine from the image"), "{refused}");
        let last = r.rows().last().cloned().expect("a row");
        assert_eq!(last["reason"], refused.as_str());
        assert!(last.get("wakeAsk").is_none());
        // The row offers the rebuild, which before this only a gone or zombie machine did, and the wake beside it.
        let input = |wake_refused: Option<String>| WorkspaceStateInput {
            phase: WorkspacePhase::Napping,
            machine_state: Some(MachineState::Paused),
            reach: Some(ReachState::Napping),
            wake_refused,
        };
        assert!(needs_rebuild(&input(Some(refused.clone()))));
        assert!(!needs_rebuild(&input(None)));
        assert_matches(&r.store.get("workspaces", &id).await.expect("a read").expect("a record"), json!({ "wakeRefused": refused }));
        // Every poll after it carries the road too: a row built from the record alone still offers the rebuild.
        assert_matches(&status_rows(&r.fx.rt).await[0], json!({ "phase": "napping", "reason": refused }));
        // The provider comes back: the wake lands and the road goes with the fault that made it.
        let up = m.clone();
        m.set_resume(Rc::new(move || {
            up.paused.set(false);
            Box::pin(async { Ok(()) })
        }));
        let woke = j(&r.fx.rt.workspaces().wake(&id, None).await.expect("a wake"));
        assert!(woke.get("wakeRefused").is_none());
        assert!(r.store.get("workspaces", &id).await.expect("a read").expect("a record").get("wakeRefused").is_none());
        assert!(status_rows(&r.fx.rt).await[0].get("reason").is_none());
        r.fx.rt.close().await.expect("a close");
    });
}

// "stops when the person stops it from the row, while the host waits to ask again"
#[test]
fn stops_when_the_person_stops_it_from_the_row_while_the_host_waits_to_ask_again() {
    drive(|| async {
        let r = rig(Asks::For(30 * EVERY));
        let (id, _m, deaf) = napping_on_a_deaf_provider(&r).await;
        let _warn = capture();
        let watched = waking(&r.fx, &id);
        until_n(|| deaf.calls.get() == 1).await;
        deaf.cap();
        until_n(|| r.rows().iter().any(|s| ask_of(s) == Some(1))).await;
        let left = r.fx.rt.workspaces().stop_wake(&id, None).await.expect("a stop");
        assert_eq!(j(&left)["phase"], "napping");
        let refused = watched.outcome().await;
        assert!(refused.contains(WAKE_STOPPED), "{refused}");
        assert_eq!(deaf.calls.get(), 1);
        assert_matches(&r.store.get("workspaces", &id).await.expect("a read").expect("a record"), json!({ "phase": "napping" }));
        assert_matches(r.rows().last().expect("a row"), json!({ "phase": "napping", "reason": WAKE_STOPPED }));
        // Nothing asks again after the stop: the cadence's timer is spent with no call behind it.
        r.fc.advance(EVERY * 5);
        sleep_ms(20).await;
        assert_eq!(deaf.calls.get(), 1);
        r.fx.rt.close().await.expect("a close");
    });
}

// "stops while the host is on a call by ending it, and a cap that fires after says nothing on the paused row"
#[test]
fn stops_while_the_host_is_on_a_call_by_ending_it_and_a_cap_that_fires_after_says_nothing_on_the_paused_row() {
    drive(|| async {
        let r = rig(Asks::For(30 * EVERY));
        let (id, _m, deaf) = napping_on_a_deaf_provider(&r).await;
        let watched = waking(&r.fx, &id);
        until_n(|| deaf.calls.get() == 1).await;
        // The call is nowhere near its cap; the stop is what ends it, so nothing goes on running behind the wake.
        r.fx.rt.workspaces().stop_wake(&id, None).await.expect("a stop");
        let refused = watched.outcome().await;
        assert!(refused.contains(WAKE_STOPPED), "{refused}");
        assert_eq!(deaf.calls.get(), 1);
        assert_matches(r.rows().last().expect("a row"), json!({ "phase": "napping", "reason": WAKE_STOPPED }));
        // The provider's cap fires on the call the stop already ended, and the whole cadence passes: a settled stop wins,
        // so no poll of a paused row ever says the machine is being read about.
        deaf.cap();
        r.fc.advance(EVERY * 3);
        sleep_ms(20).await;
        assert_matches(r.rows().last().expect("a row"), json!({ "phase": "napping", "reason": WAKE_STOPPED }));
        let polled = status_rows(&r.fx.rt).await[0].clone();
        assert_eq!(polled["phase"], "napping");
        assert!(polled.get("reason").is_none() && polled.get("wakeAsk").is_none(), "{polled}");
        assert_eq!(deaf.calls.get(), 1);
        r.fx.rt.close().await.expect("a close");
    });
}

// "a resume the provider finally takes ends the wake with the machine running, however many asks it took"
#[test]
fn a_resume_the_provider_finally_takes_ends_the_wake_with_the_machine_running_however_many_asks_it_took() {
    drive(|| async {
        let r = rig(Asks::For(30 * EVERY));
        let (id, _m, deaf) = napping_on_a_deaf_provider(&r).await;
        let _warn = capture();
        let watched = waking(&r.fx, &id);
        until_n(|| deaf.calls.get() == 1).await;
        deaf.takes.set(true);
        tick(&r.fc, &deaf, || deaf.calls.get() == 2, EVERY).await;
        let woken = watched.result().await.expect("a wake");
        assert_eq!(woken["phase"], "running");
        assert_eq!(deaf.calls.get(), 2);
        let ids: Vec<Value> = r.events.of("workspace.woken").iter().map(|e| e["workspaceId"].clone()).collect();
        assert_eq!(ids, [json!(id)]);
        r.fx.rt.close().await.expect("a close");
    });
}

// "keeps the line on every status the poll builds, so the row does not fall silent between two asks"
#[test]
fn keeps_the_line_on_every_status_the_poll_builds_so_the_row_does_not_fall_silent_between_two_asks() {
    drive(|| async {
        let r = rig(Asks::For(30 * EVERY));
        let (id, _m, deaf) = napping_on_a_deaf_provider(&r).await;
        let _warn = capture();
        let watched = waking(&r.fx, &id);
        until_n(|| deaf.calls.get() == 1).await;
        deaf.cap();
        let rt = r.fx.rt.clone();
        until(|| {
            let rt = rt.clone();
            async move { status_rows(&rt).await.iter().any(|s| ask_of(s) == Some(1)) }
        })
        .await;
        // A poll builds its rows from the record alone, so this is the row a client that missed the push would draw.
        assert_matches(&status_rows(&r.fx.rt).await[0], json!({ "phase": "waking", "wakeAsk": { "ask": 1, "of": 30 } }));
        r.fx.rt.workspaces().stop_wake(&id, None).await.expect("a stop");
        watched.outcome().await;
        // The wake is over, so the ask goes with it and the row keeps only what the poll can see for itself.
        assert!(status_rows(&r.fx.rt).await[0].get("wakeAsk").is_none());
        r.fx.rt.close().await.expect("a close");
    });
}

// "a stop asked of a workspace with no wake in flight answers with the record as it stands"
#[test]
fn a_stop_asked_of_a_workspace_with_no_wake_in_flight_answers_with_the_record_as_it_stands() {
    drive(|| async {
        let r = rig(Asks::Keep);
        let ws = create_on(&r.fx.rt, CreateOn::golden("b1"), None).await.expect("a workspace");
        assert_eq!(j(&r.fx.rt.workspaces().stop_wake(&ws.view.id, None).await.expect("a stop"))["phase"], "running");
        r.fx.rt.close().await.expect("a close");
    });
}

// "a backend that declares no asking is asked once: no second resume, and the gave-up line counts one"
#[test]
fn a_backend_that_declares_no_asking_is_asked_once_no_second_resume_and_the_gave_up_line_counts_one() {
    drive(|| async {
        let r = rig(Asks::None);
        let (id, _m, deaf) = napping_on_a_deaf_provider(&r).await;
        let _warn = capture();
        let watched = waking(&r.fx, &id);
        until_n(|| deaf.calls.get() == 1).await;
        deaf.cap();
        let outcome = watched.outcome().await;
        // ^the provider answered none of 1 resume request over
        assert!(outcome.starts_with("the provider answered none of 1 resume request over "), "{outcome}");
        // Every ask that reaches a provider that bills starts is a start: a backend that says so is never asked twice.
        r.fc.advance(EVERY * 5);
        sleep_ms(20).await;
        assert_eq!(deaf.calls.get(), 1);
        assert!(!r.rows().iter().any(|s| s.get("wakeAsk").is_some()));
        assert_eq!(j(&r.fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"))["phase"], "napping");
        r.fx.rt.close().await.expect("a close");
    });
}
