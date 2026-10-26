// SPDX-License-Identifier: AGPL-3.0-only
//! spec/clock.md. The fake clock is what 51 TS test files lean on, so its loop is pinned case by case.
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use support::run_local;
use wsp_core::clock::{real_clock, Clock, FakeClock, RealClock};

fn log() -> Rc<RefCell<Vec<(String, i64)>>> {
    Rc::new(RefCell::new(Vec::new()))
}

fn mark(clock: &Rc<dyn Clock>, seen: &Rc<RefCell<Vec<(String, i64)>>>, name: &str) -> Box<dyn FnOnce()> {
    let (clock, seen, name) = (clock.clone(), seen.clone(), name.to_string());
    Box::new(move || seen.borrow_mut().push((name, clock.now())))
}

#[test]
fn fake_clock_runs_due_timers_earliest_first_and_lands_on_the_target() {
    let fc = FakeClock::new(1_000);
    let clock = fc.clock();
    let seen = log();
    clock.schedule(mark(&clock, &seen, "late"), 300);
    clock.schedule(mark(&clock, &seen, "early"), 100);
    clock.schedule(mark(&clock, &seen, "beyond"), 900);
    fc.advance(500);
    assert_eq!(*seen.borrow(), vec![("early".to_string(), 1_100), ("late".to_string(), 1_300)]);
    assert_eq!(fc.now(), 1_500);
    assert_eq!(clock.now(), 1_500);
    assert_eq!(fc.pending(), 1);
    assert!(fc.due_for(1_900));
}

#[test]
fn clock_ties_run_in_scheduling_order() {
    let fc = FakeClock::new(0);
    let clock = fc.clock();
    let seen = log();
    for name in ["a", "b", "c"] {
        clock.schedule(mark(&clock, &seen, name), 50);
    }
    fc.advance(50);
    let names: Vec<String> = seen.borrow().iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(names, ["a", "b", "c"]);
}

#[test]
fn clock_a_timer_scheduled_inside_advance_runs_in_the_same_advance() {
    let fc = FakeClock::new(0);
    let clock = fc.clock();
    let seen = log();
    let (c2, s2) = (clock.clone(), seen.clone());
    clock.schedule(
        Box::new(move || {
            s2.borrow_mut().push(("outer".into(), c2.now()));
            let (c3, s3) = (c2.clone(), s2.clone());
            c2.schedule(Box::new(move || s3.borrow_mut().push(("inner".into(), c3.now()))), 0);
            let (c4, s4) = (c2.clone(), s2.clone());
            c2.schedule(Box::new(move || s4.borrow_mut().push(("next".into(), c4.now()))), 100);
        }),
        10,
    );
    fc.advance(50);
    assert_eq!(*seen.borrow(), vec![("outer".to_string(), 10), ("inner".to_string(), 10)]);
    assert_eq!(fc.pending(), 1);
    fc.advance(60);
    assert_eq!(seen.borrow().last().unwrap(), &("next".to_string(), 110));
}

#[test]
fn fake_clock_now_inside_a_timer_reads_its_own_at() {
    let fc = FakeClock::new(5_000);
    let clock = fc.clock();
    let seen = log();
    clock.schedule(mark(&clock, &seen, "t"), 250);
    fc.advance(10_000);
    assert_eq!(*seen.borrow(), vec![("t".to_string(), 5_250)]);
    assert_eq!(fc.now(), 15_000);
}

#[test]
fn fake_clock_advance_zero_runs_what_is_due_now() {
    let fc = FakeClock::new(100);
    let clock = fc.clock();
    let seen = log();
    clock.schedule(mark(&clock, &seen, "now"), 0);
    assert!(seen.borrow().is_empty());
    fc.advance(0);
    assert_eq!(*seen.borrow(), vec![("now".to_string(), 100)]);
    assert_eq!(fc.pending(), 0);
}

#[test]
fn fake_clock_a_negative_delay_runs_at_the_next_advance_with_its_own_at() {
    let fc = FakeClock::new(1_000);
    let clock = fc.clock();
    let seen = log();
    clock.schedule(mark(&clock, &seen, "past"), -400);
    assert!(fc.due_for(600));
    fc.advance(1);
    assert_eq!(*seen.borrow(), vec![("past".to_string(), 600)]);
    assert_eq!(fc.now(), 1_001);
}

#[test]
fn clock_cancel_stops_a_pending_timer_and_is_harmless_twice_or_after_firing() {
    let fc = FakeClock::new(0);
    let clock = fc.clock();
    let seen = log();
    let stopped = clock.schedule(mark(&clock, &seen, "stopped"), 10);
    let fired = clock.schedule(mark(&clock, &seen, "fired"), 20);
    stopped();
    stopped();
    assert_eq!(fc.pending(), 1);
    fc.advance(30);
    fired();
    fired();
    assert_eq!(*seen.borrow(), vec![("fired".to_string(), 20)]);
    assert_eq!(fc.pending(), 0);
}

#[test]
fn fake_clock_a_timer_may_schedule_and_cancel_on_its_own_clock() {
    let fc = FakeClock::new(0);
    let clock = fc.clock();
    let seen = log();
    let victim = clock.schedule(mark(&clock, &seen, "victim"), 20);
    let (c2, s2, fc2) = (clock.clone(), seen.clone(), fc.clone());
    clock.schedule(
        Box::new(move || {
            victim();
            assert_eq!(fc2.pending(), 0);
            s2.borrow_mut().push(("killer".into(), c2.now()));
        }),
        10,
    );
    fc.advance(100);
    assert_eq!(*seen.borrow(), vec![("killer".to_string(), 10)]);
}

#[test]
fn fake_clock_pending_and_due_for_count_what_is_stored() {
    let fc = FakeClock::new(0);
    let clock = fc.clock();
    assert_eq!(fc.pending(), 0);
    assert!(!fc.due_for(0));
    let a = clock.schedule(Box::new(|| {}), 5);
    clock.schedule(Box::new(|| {}), 7);
    assert_eq!(fc.pending(), 2);
    assert!(fc.due_for(5) && fc.due_for(7) && !fc.due_for(6));
    a();
    assert_eq!(fc.pending(), 1);
    assert!(!fc.due_for(5));
}

#[test]
fn fake_clock_handles_share_one_now() {
    let fc = FakeClock::new(42);
    let one = fc.clock();
    let two = fc.clone().clock();
    fc.advance(8);
    assert_eq!((one.now(), two.now(), fc.now()), (50, 50, 50));
    let fresh = FakeClock::at_now();
    let wall = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    assert!((fresh.now() - wall).abs() < 5_000);
}

#[test]
fn real_clock_fires_on_the_local_set() {
    run_local(async {
        let clock = real_clock();
        let wall = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        assert!((clock.now() - wall).abs() < 1_000);
        let hit = Rc::new(RefCell::new(false));
        let h = hit.clone();
        clock.schedule(Box::new(move || *h.borrow_mut() = true), 20);
        let stopped = Rc::new(RefCell::new(false));
        let s = stopped.clone();
        let cancel = clock.schedule(Box::new(move || *s.borrow_mut() = true), 20);
        cancel();
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(*hit.borrow());
        assert!(!*stopped.borrow());
        cancel();
    });
}

#[test]
fn real_clock_clamps_a_delay_past_the_timer_range_to_one_ms() {
    run_local(async {
        let hit = Rc::new(RefCell::new(None));
        let h = hit.clone();
        let started = Instant::now();
        RealClock.schedule(Box::new(move || *h.borrow_mut() = Some(Instant::now())), wsp_core::clock::TIMEOUT_MAX_MS + 1);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let at = hit.borrow().expect("a delay past the range fires at once, as setTimeout does");
        assert!(at - started < Duration::from_millis(100));
    });
}

#[test]
fn real_clock_clamps_a_negative_delay_to_one_ms() {
    run_local(async {
        let hit = Rc::new(RefCell::new(false));
        let h = hit.clone();
        RealClock.schedule(Box::new(move || *h.borrow_mut() = true), -5);
        assert!(!*hit.borrow(), "a timer never runs inside schedule");
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(*hit.borrow());
    });
}

use futures::FutureExt;
use wsp_core::clock::RetryClockOver;
use wsp_engine::errors::RetryClock;

#[test]
fn retry_clock_now_reads_the_core_clock() {
    let fc = FakeClock::new(7_000);
    let retry = RetryClockOver::new(fc.clock());
    assert_eq!(retry.now(), 7_000);
    fc.advance(250);
    assert_eq!(retry.now(), 7_250);
}

#[test]
fn retry_clock_sleep_ends_when_the_core_clock_reaches_it() {
    run_local(async {
        let fc = FakeClock::new(0);
        let retry = RetryClockOver::new(fc.clock());
        let done = Rc::new(RefCell::new(false));
        let flag = done.clone();
        let task = tokio::task::spawn_local(async move {
            retry.sleep(300).await;
            *flag.borrow_mut() = true;
        });
        support::settle().await;
        assert_eq!(fc.pending(), 1);
        assert!(fc.due_for(300));
        fc.advance(299);
        support::settle().await;
        assert!(!*done.borrow());
        fc.advance(1);
        task.await.unwrap();
        assert!(*done.borrow());
        assert_eq!(fc.pending(), 0);
    });
}

#[test]
fn retry_clock_sleep_of_nothing_ends_at_the_next_advance() {
    let fc = FakeClock::new(50);
    let retry = RetryClockOver::new(fc.clock());
    let mut sleeping = retry.sleep(0).boxed_local();
    assert!(sleeping.as_mut().now_or_never().is_none());
    fc.advance(0);
    assert!(sleeping.as_mut().now_or_never().is_some());
}

#[test]
fn retry_clock_a_dropped_sleep_leaves_no_timer_behind() {
    let fc = FakeClock::new(0);
    let retry = RetryClockOver::new(fc.clock());
    let mut sleeping = retry.sleep(1_000).boxed_local();
    assert!(sleeping.as_mut().now_or_never().is_none());
    assert_eq!(fc.pending(), 1);
    drop(sleeping);
    assert_eq!(fc.pending(), 0);
}

#[test]
fn retry_clock_random_draws_from_zero_up_to_one() {
    let retry = RetryClockOver::new(FakeClock::new(0).clock());
    let draws: Vec<f64> = (0..200).map(|_| retry.random()).collect();
    assert!(draws.iter().all(|r| (0.0..1.0).contains(r)));
    assert!(draws.windows(2).any(|w| w[0] != w[1]));
}

// fake-clock.ts:25 `at: now + ms` is a JS number: a delay past the end of time is an `at` that never comes due.
#[test]
fn fake_clock_a_delay_past_the_end_of_time_saturates_and_never_fires() {
    let fc = FakeClock::new(1_000);
    let clock = fc.clock();
    let seen = log();
    clock.schedule(mark(&clock, &seen, "never"), i64::MAX);
    assert_eq!(fc.pending(), 1);
    assert!(fc.due_for(i64::MAX));
    fc.advance(1_000_000);
    assert!(seen.borrow().is_empty());
    assert_eq!(fc.pending(), 1);
}
