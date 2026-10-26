// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/host/test/watch.test.ts` and the `watchOn` entries of `fixtures/layout_oracle.json`.
#![allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::regex_creation_in_loops,
    non_snake_case,
    dead_code
)]
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use serde_json::Value;
use wsp_cli::watch::{watch_block, watch_on, Lines, Redraw, StopListener, WatchDeps, WatchOn, WatchSignals};
use wsp_words::exit::{HostError, Kind};

const HIDE: &str = "\x1b[?25l";
const SHOW: &str = "\x1b[?25h";

/// A terminal that records every write.
struct Term {
    writes: Mutex<Vec<String>>,
    columns: AtomicU16,
}

impl Redraw for Term {
    fn write(&self, text: &str) {
        self.writes.lock().unwrap().push(text.to_string());
    }
    fn columns(&self) -> u16 {
        self.columns.load(Ordering::SeqCst)
    }
}

/// The signals a test fires by hand: a real one would take the runner. `fire` calls every listener held, as the TS
/// test's `fire` does; the calls made on it are kept in order.
struct Fake {
    held: Mutex<Vec<(&'static str, StopListener)>>,
    calls: Mutex<Vec<String>>,
}

impl WatchSignals for Fake {
    fn on(&self, signal: &'static str, listener: StopListener) {
        self.calls.lock().unwrap().push(format!("on {signal}"));
        self.held.lock().unwrap().push((signal, listener));
    }
    fn off(&self, signal: &'static str, listener: &StopListener) {
        self.calls.lock().unwrap().push(format!("off {signal}"));
        self.held.lock().unwrap().retain(|(s, l)| !(*s == signal && Arc::ptr_eq(l, listener)));
    }
}

impl Fake {
    fn fire(&self) {
        let listeners: Vec<StopListener> = self.held.lock().unwrap().iter().map(|(_, l)| l.clone()).collect();
        for l in listeners {
            l();
        }
    }
    fn held_signals(&self) -> Vec<&'static str> {
        self.held.lock().unwrap().iter().map(|(s, _)| *s).collect()
    }
}

fn rig(columns: u16) -> (Arc<Term>, Arc<Fake>) {
    (
        Arc::new(Term { writes: Mutex::new(Vec::new()), columns: AtomicU16::new(columns) }),
        Arc::new(Fake { held: Mutex::new(Vec::new()), calls: Mutex::new(Vec::new()) }),
    )
}

fn writes(term: &Term) -> Vec<String> {
    term.writes.lock().unwrap().clone()
}

type Until = Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>;

fn deps(term: &Arc<Term>, fake: &Arc<Fake>, every_ms: Option<u64>, until: Option<Until>) -> WatchDeps {
    let redraw: Arc<dyn Redraw> = term.clone();
    let signals: Arc<dyn WatchSignals> = fake.clone();
    WatchDeps { redraw, every_ms, signals: Some(signals), until }
}

/// A `Lines` that counts its calls from one and hands the count to `f`, which runs inside the returned future.
fn lines_of(f: impl Fn(usize) -> Result<Vec<String>, HostError> + Send + Sync + 'static) -> Lines {
    let tick = Arc::new(AtomicUsize::new(0));
    let f = Arc::new(f);
    Arc::new(move || -> BoxFuture<'static, Result<Vec<String>, HostError>> {
        let tick = tick.clone();
        let f = f.clone();
        Box::pin(async move { f(tick.fetch_add(1, Ordering::SeqCst) + 1) })
    })
}

fn rows(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

async fn run(lines: Lines, deps: WatchDeps) -> Result<(), HostError> {
    tokio::time::timeout(Duration::from_secs(20), watch_block(lines, deps)).await.expect("the watch never stopped")
}

fn refusal(on: WatchOn) -> HostError {
    match on {
        WatchOn::Refusal(e) => e,
        WatchOn::Redraw(_) => panic!("expected a refusal, got a redraw"),
    }
}

/// watch.test.ts:26
#[tokio::test]
async fn watch_draws_one_write_per_frame_rewinds_the_rows_the_last_frame_took_and_gives_the_cursor_back_on_the_stop() {
    let (term, fake) = rig(100);
    let f = fake.clone();
    let lines = lines_of(move |tick| {
        if tick == 3 {
            f.fire();
        }
        Ok(vec!["WORKSPACE  STATE".to_string(), format!("alpha      Running {tick}")])
    });
    run(lines, deps(&term, &fake, Some(1), None)).await.expect("the watch ends clean");
    assert_eq!(
        writes(&term),
        vec![
            HIDE.to_string(),
            "WORKSPACE  STATE\nalpha      Running 1\n".to_string(),
            "\x1b[2A\x1b[G\x1b[JWORKSPACE  STATE\nalpha      Running 2\n".to_string(),
            "\x1b[2A\x1b[G\x1b[JWORKSPACE  STATE\nalpha      Running 3\n".to_string(),
            SHOW.to_string(),
        ]
    );
}

/// watch.test.ts:46
#[tokio::test]
async fn watch_counts_the_rows_a_frame_wrapped_onto_at_the_width_the_terminal_has_now() {
    let (term, fake) = rig(10);
    let f = fake.clone();
    let lines = lines_of(move |tick| {
        if tick == 2 {
            f.fire();
        }
        Ok(vec!["x".repeat(25)])
    });
    run(lines, deps(&term, &fake, Some(1), None)).await.expect("the watch ends clean");
    let x = "x".repeat(25);
    assert_eq!(writes(&term), vec![HIDE.to_string(), format!("{x}\n"), format!("\x1b[3A\x1b[G\x1b[J{x}\n"), SHOW.to_string()]);
}

/// watch.ts:75 (watch.test.ts:46 narrowed): the width is read when the frame is drawn, not once at the start.
#[tokio::test]
async fn watch_reads_the_width_again_for_every_frame() {
    let (term, fake) = rig(100);
    let (f, t) = (fake.clone(), term.clone());
    let lines = lines_of(move |tick| {
        match tick {
            2 => t.columns.store(10, Ordering::SeqCst),
            3 => f.fire(),
            _ => {}
        }
        Ok(vec!["x".repeat(25)])
    });
    run(lines, deps(&term, &fake, Some(1), None)).await.expect("the watch ends clean");
    let x = "x".repeat(25);
    assert_eq!(
        writes(&term),
        vec![
            HIDE.to_string(),
            format!("{x}\n"),
            format!("\x1b[3A\x1b[G\x1b[J{x}\n"),
            format!("\x1b[3A\x1b[G\x1b[J{x}\n"),
            SHOW.to_string()
        ]
    );
}

/// watch.test.ts:58
#[tokio::test]
async fn watch_ends_with_the_cursor_back_and_the_refusal_carried_out_to_the_caller_when_a_draw_fails() {
    let (term, fake) = rig(100);
    let lines = lines_of(|_| Err(HostError::new("the host stopped answering")));
    let err = run(lines, deps(&term, &fake, Some(1), None)).await.expect_err("the failure rides out");
    assert_eq!((err.message.as_str(), err.kind.clone(), err.fix.clone()), ("the host stopped answering", None, None));
    assert_eq!(writes(&term), vec![HIDE.to_string(), SHOW.to_string()]);
    assert_eq!(fake.held_signals(), Vec::<&str>::new());
}

/// watch.ts:50-52 (no TS case): a failure after frames were drawn leaves them where they are, with the cursor back.
#[tokio::test]
async fn watch_keeps_the_frames_it_drew_when_a_later_draw_fails() {
    let (term, fake) = rig(100);
    let lines =
        lines_of(|tick| if tick == 1 { Ok(vec!["one row".to_string()]) } else { Err(HostError::new("the host stopped answering")) });
    let err = run(lines, deps(&term, &fake, Some(1), None)).await.expect_err("the failure rides out");
    assert_eq!(err.message, "the host stopped answering");
    assert_eq!(writes(&term), vec![HIDE.to_string(), "one row\n".to_string(), SHOW.to_string()]);
    assert_eq!(fake.held_signals(), Vec::<&str>::new());
}

/// watch.test.ts:64
#[tokio::test]
async fn watch_hangs_one_listener_per_signal_and_takes_them_all_back_off_when_it_ends() {
    let (term, fake) = rig(100);
    let seen: Arc<Mutex<Vec<(Vec<&'static str>, usize)>>> = Arc::new(Mutex::new(Vec::new()));
    let (f, s) = (fake.clone(), seen.clone());
    let lines = lines_of(move |_| {
        let held = f.held.lock().unwrap();
        let distinct = held.iter().filter(|(_, l)| Arc::ptr_eq(l, &held[0].1)).count();
        s.lock().unwrap().push((held.iter().map(|(sig, _)| *sig).collect(), distinct));
        drop(held);
        f.fire();
        Ok(vec!["one row".to_string()])
    });
    run(lines, deps(&term, &fake, Some(1), None)).await.expect("the watch ends clean");
    // One listener per signal the watch stops on, all of them the same handler.
    assert_eq!(*seen.lock().unwrap(), vec![(vec!["SIGINT", "SIGTERM", "SIGHUP"], 3)]);
    assert_eq!(fake.held_signals(), Vec::<&str>::new());
}

/// watch.ts:23, 64, 91 and WatchSignals' doc (no TS case): on and off once per signal, in order, on and off in the same order.
#[tokio::test]
async fn watch_hangs_the_signals_in_order_before_it_draws_and_takes_them_off_in_the_same_order() {
    let (term, fake) = rig(100);
    let f = fake.clone();
    let lines = lines_of(move |_| {
        f.fire();
        Ok(vec!["one row".to_string()])
    });
    run(lines, deps(&term, &fake, Some(1), None)).await.expect("the watch ends clean");
    let calls: Vec<String> =
        ["on SIGINT", "on SIGTERM", "on SIGHUP", "off SIGINT", "off SIGTERM", "off SIGHUP"].iter().map(|s| s.to_string()).collect();
    assert_eq!(*fake.calls.lock().unwrap(), calls);
}

/// watch.test.ts:83
#[tokio::test]
async fn watch_draws_nothing_for_empty_rows_and_leaves_the_block_where_it_is() {
    let (term, fake) = rig(100);
    let f = fake.clone();
    let lines = lines_of(move |tick| {
        if tick == 4 {
            f.fire();
        }
        // A source that goes quiet for two ticks: the rows it had must stay where they are, not scroll on a newline.
        Ok(if tick == 2 || tick == 3 { Vec::new() } else { rows(&["one row"]) })
    });
    run(lines, deps(&term, &fake, Some(1), None)).await.expect("the watch ends clean");
    assert_eq!(
        writes(&term),
        vec![HIDE.to_string(), "one row\n".to_string(), "\x1b[1A\x1b[G\x1b[Jone row\n".to_string(), SHOW.to_string()]
    );
}

/// watch.test.ts:96
#[tokio::test]
async fn watch_draws_as_soon_as_the_source_pushes_rather_than_waiting_out_the_tick() {
    let (term, fake) = rig(100);
    let f = fake.clone();
    let lines = lines_of(move |tick| {
        if tick == 3 {
            f.fire();
        }
        Ok(vec![format!("row {tick}")])
    });
    let pushes = Arc::new(AtomicUsize::new(0));
    let p = pushes.clone();
    // A tick far past the run: every frame after the first is the push alone, so this counts pushes and not ticks.
    let until: Until = Arc::new(move || -> BoxFuture<'static, ()> {
        p.fetch_add(1, Ordering::SeqCst);
        Box::pin(tokio::time::sleep(Duration::from_millis(1)))
    });
    run(lines, deps(&term, &fake, Some(60_000), Some(until))).await.expect("the watch ends clean");
    assert_eq!(
        writes(&term),
        vec![
            HIDE.to_string(),
            "row 1\n".to_string(),
            "\x1b[1A\x1b[G\x1b[Jrow 2\n".to_string(),
            "\x1b[1A\x1b[G\x1b[Jrow 3\n".to_string(),
            SHOW.to_string(),
        ]
    );
    assert_eq!(pushes.load(Ordering::SeqCst), 2);
}

/// watch.test.ts:121
#[tokio::test]
async fn watch_keeps_drawing_on_the_tick_when_the_source_stops_pushing() {
    let (term, fake) = rig(100);
    let f = fake.clone();
    let lines = lines_of(move |tick| {
        if tick == 2 {
            f.fire();
        }
        Ok(vec![format!("row {tick}")])
    });
    // An until that never settles: every frame after the first is the tick alone.
    let until: Until = Arc::new(|| -> BoxFuture<'static, ()> { Box::pin(futures::future::pending()) });
    run(lines, deps(&term, &fake, Some(1), Some(until))).await.expect("the watch ends clean");
    assert_eq!(writes(&term), vec![HIDE.to_string(), "row 1\n".to_string(), "\x1b[1A\x1b[G\x1b[Jrow 2\n".to_string(), SHOW.to_string()]);
}

/// watch.ts:57-63, 81-85 (no TS case): a stop arriving during the wait ends the wait, not the frame after it.
#[tokio::test]
async fn watch_ends_a_wait_the_moment_a_stop_arrives_instead_of_drawing_another_frame() {
    let (term, fake) = rig(100);
    let f = fake.clone();
    let lines = lines_of(move |_| {
        let f = f.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            f.fire();
        });
        Ok(vec!["one row".to_string()])
    });
    let t0 = Instant::now();
    run(lines, deps(&term, &fake, Some(60_000), None)).await.expect("the watch ends clean");
    assert!(t0.elapsed() < Duration::from_secs(5), "the stop waited out the tick: {:?}", t0.elapsed());
    assert_eq!(writes(&term), vec![HIDE.to_string(), "one row\n".to_string(), SHOW.to_string()]);
    assert_eq!(fake.held_signals(), Vec::<&str>::new());
}

/// watch.ts:54 and `WatchDeps::every_ms`'s doc (no TS case): no interval is a second.
#[tokio::test]
async fn watch_waits_a_second_between_frames_when_it_is_given_no_interval() {
    let (term, fake) = rig(100);
    let f = fake.clone();
    let lines = lines_of(move |tick| {
        if tick == 2 {
            f.fire();
        }
        Ok(vec![format!("row {tick}")])
    });
    let t0 = Instant::now();
    run(lines, deps(&term, &fake, None, None)).await.expect("the watch ends clean");
    assert!(t0.elapsed() >= Duration::from_millis(900), "the second frame came after {:?}", t0.elapsed());
    assert_eq!(writes(&term), vec![HIDE.to_string(), "row 1\n".to_string(), "\x1b[1A\x1b[G\x1b[Jrow 2\n".to_string(), SHOW.to_string()]);
}

/// watch.test.ts:136
#[test]
fn watch_on_refuses_where_there_is_no_terminal_to_redraw_on_in_two_halves() {
    let e = refusal(watch_on("wsp workspaces", false, None));
    assert_eq!(
        e.message,
        "wsp workspaces --watch redraws where it stands, and this run has no terminal to redraw on. Run wsp workspaces without --watch to print the list once."
    );
    assert_eq!(e.fix.as_deref(), Some("Run wsp workspaces without --watch to print the list once."));
}

/// watch.test.ts:136
#[test]
fn watch_on_refuses_beside_json_in_two_halves() {
    let (term, _) = rig(100);
    let redraw: Arc<dyn Redraw> = term;
    let e = refusal(watch_on("wsp threads", true, Some(redraw)));
    assert_eq!(
        e.message,
        "wsp threads --watch redraws a table and --json answers with objects. Take one of the two: wsp threads --watch at a terminal, or wsp threads --json for the objects."
    );
    assert_eq!(e.fix.as_deref(), Some("Take one of the two: wsp threads --watch at a terminal, or wsp threads --json for the objects."));
}

/// watch.ts:99-103 and `watch_on`'s doc: `--json` is refused first, then no terminal.
#[test]
fn watch_on_refuses_json_before_it_looks_for_a_terminal() {
    let e = refusal(watch_on("wsp threads", true, None));
    assert_eq!(e.message, "wsp threads --watch redraws a table and --json answers with objects. Take one of the two: wsp threads --watch at a terminal, or wsp threads --json for the objects.");
}

/// watch.test.ts:136
#[test]
fn watch_on_hands_back_the_same_terminal_to_draw_on_when_there_is_one_and_no_json() {
    let (term, _) = rig(100);
    let redraw: Arc<dyn Redraw> = term;
    match watch_on("wsp workspaces", false, Some(redraw.clone())) {
        WatchOn::Redraw(got) => assert!(Arc::ptr_eq(&got, &redraw), "a different stream came back"),
        WatchOn::Refusal(e) => panic!("refused a terminal with no --json: {}", e.message),
    }
}

/// oracle
#[test]
fn watch_on_refusals_carry_the_recorded_message_usage_kind_and_fix() {
    let (term, _) = rig(100);
    let redraw: Arc<dyn Redraw> = term;
    let recorded = oracle_watch_on();
    let got: Vec<(String, Option<Kind>, Option<String>)> =
        vec![refusal(watch_on("wsp workspaces", false, None)), refusal(watch_on("wsp threads", true, Some(redraw)))]
            .into_iter()
            .map(|e| (e.message, e.kind, e.fix))
            .collect();
    let want: Vec<(String, Option<Kind>, Option<String>)> = recorded
        .iter()
        .map(|c| {
            assert_eq!(c["kind"], "usage");
            (c["message"].as_str().unwrap().to_string(), Some(Kind::Usage), Some(c["fix"].as_str().unwrap().to_string()))
        })
        .collect();
    assert_eq!(got, want);
}

fn oracle_watch_on() -> Vec<Value> {
    let all: Value = serde_json::from_str(include_str!("fixtures/layout_oracle.json")).expect("the oracle parses");
    all["watchOn"].as_array().expect("watchOn is a list").clone()
}
