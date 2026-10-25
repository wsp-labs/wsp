// SPDX-License-Identifier: AGPL-3.0-only
//! spec/idle.md; ported from packages/runtime/test/idle.test.ts "idle policy mechanics" (:22-265) plus the hard cases
//! the spec pins. The clock is fake, so a long window costs nothing.
mod support;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use support::{run_local, settle};
use wsp_core::clock::FakeClock;
use wsp_core::console;
use wsp_core::idle::{backstop_ms, create_idle_policy, IdlePolicy, IdlePolicyOptions, IDLE_OFF_BACKSTOP_MS};

const WINDOW: i64 = 5 * 60_000;
const RETRY: i64 = 15_000;

struct Rig {
    fc: FakeClock,
    windows: Rc<RefCell<HashMap<String, Option<i64>>>>,
    fired: Rc<RefCell<Vec<(String, i64)>>>,
    failing: Rc<Cell<bool>>,
    busy: Rc<RefCell<Vec<String>>>,
    heard: Rc<RefCell<Vec<(String, i64)>>>,
    /// Set by a test to run inside on_idle, before its future: what a nap does to the policy.
    during_nap: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
}

fn rig() -> Rig {
    Rig {
        fc: FakeClock::new(1_700_000_000_000),
        windows: Rc::default(),
        fired: Rc::default(),
        failing: Rc::default(),
        busy: Rc::default(),
        heard: Rc::default(),
        during_nap: Rc::default(),
    }
}

fn make(r: &Rig) -> IdlePolicy {
    let (windows, fired, failing, busy, heard, during) =
        (r.windows.clone(), r.fired.clone(), r.failing.clone(), r.busy.clone(), r.heard.clone(), r.during_nap.clone());
    create_idle_policy(IdlePolicyOptions {
        window_of: Rc::new(move |id| windows.borrow().get(id).copied().unwrap_or(Some(WINDOW))),
        on_idle: Rc::new(move |id, window| {
            fired.borrow_mut().push((id.to_string(), window));
            if let Some(f) = during.borrow().as_ref() {
                f(id);
            }
            let fail = failing.get();
            Box::pin(async move {
                if fail {
                    Err("fetch failed".to_string())
                } else {
                    Ok(())
                }
            })
        }),
        busy: Some(Rc::new(move |id| busy.borrow().iter().any(|b| b == id))),
        retry_ms: RETRY,
        on_backstop: Some(Rc::new(move |id, until| heard.borrow_mut().push((id.to_string(), until)))),
        clock: Some(r.fc.clock()),
    })
}

fn ids(fired: &Rc<RefCell<Vec<(String, i64)>>>) -> Vec<String> {
    fired.borrow().iter().map(|(id, _)| id.clone()).collect()
}

#[test]
fn fires_once_after_the_window_with_the_window_it_used_then_waits_for_the_next_touch() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW));
        assert_eq!(r.fc.pending(), 1);
        r.fc.advance(WINDOW - 1);
        assert!(r.fired.borrow().is_empty());
        r.fc.advance(1);
        assert_eq!(*r.fired.borrow(), vec![("a".to_string(), WINDOW)]);
        // Armed while the nap runs: the deadline still reads until the nap lands.
        assert_eq!(p.idle_at("a"), Some(r.fc.now()));
        settle().await;
        assert_eq!(p.idle_at("a"), None);
        r.fc.advance(WINDOW * 2);
        assert_eq!(r.fired.borrow().len(), 1);
        p.close();
    });
}

#[test]
fn every_touch_starts_the_window_over() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        for _ in 0..8 {
            p.touch("a");
            r.fc.advance(WINDOW / 4);
        }
        assert!(r.fired.borrow().is_empty());
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW * 3 / 4));
        r.fc.advance(WINDOW * 3 / 4);
        assert_eq!(*r.fired.borrow(), vec![("a".to_string(), WINDOW)]);
    });
}

#[test]
fn a_hold_keeps_the_window_from_firing_the_last_release_starts_it_over() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        p.hold("a");
        p.hold("a");
        assert_eq!(p.idle_at("a"), None);
        r.fc.advance(WINDOW * 5 / 2);
        assert!(r.fired.borrow().is_empty());
        p.release("a");
        r.fc.advance(WINDOW * 3 / 2);
        assert!(r.fired.borrow().is_empty(), "one hold still stands");
        p.release("a");
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW));
        r.fc.advance(WINDOW - 1);
        assert!(r.fired.borrow().is_empty());
        r.fc.advance(1);
        assert_eq!(*r.fired.borrow(), vec![("a".to_string(), WINDOW)]);
    });
}

#[test]
fn the_last_release_starts_the_window_over_from_the_release_not_from_the_touch() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        p.hold("a");
        r.fc.advance(WINDOW / 2);
        p.release("a");
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW));
        r.fc.advance(WINDOW - 1);
        assert!(r.fired.borrow().is_empty());
        r.fc.advance(1);
        assert_eq!(*r.fired.borrow(), vec![("a".to_string(), WINDOW)]);
    });
}

#[test]
fn a_busy_workspace_is_held_without_a_hold() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        r.busy.borrow_mut().push("a".into());
        assert_eq!(p.idle_at("a"), None);
        r.fc.advance(WINDOW);
        assert!(r.fired.borrow().is_empty(), "a busy workspace re-arms at its deadline");
        // The re-arm counted a fresh window from the deadline and told the backstop again.
        assert_eq!(r.heard.borrow().len(), 2);
        r.busy.borrow_mut().clear();
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW));
        r.fc.advance(WINDOW);
        assert_eq!(ids(&r.fired), ["a"]);
    });
}

#[test]
fn a_release_with_no_hold_rearms_an_armed_window() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        r.fc.advance(WINDOW / 2);
        p.release("a");
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW));
        // Nothing armed: a stray release arms nothing.
        p.release("b");
        assert_eq!(p.idle_at("b"), None);
        assert_eq!(r.fc.pending(), 1);
    });
}

#[test]
fn off_means_never_no_timer_no_deadline() {
    run_local(async {
        let r = rig();
        r.windows.borrow_mut().insert("a".into(), None);
        let p = make(&r);
        p.touch("a");
        assert_eq!(p.idle_at("a"), None);
        assert_eq!(r.fc.pending(), 0);
        r.fc.advance(WINDOW * 3);
        assert!(r.fired.borrow().is_empty());
    });
}

#[test]
fn forget_cancels_the_window_a_nap_or_a_delete() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        p.forget("a");
        assert_eq!(p.idle_at("a"), None);
        assert_eq!(r.fc.pending(), 0);
        r.fc.advance(WINDOW * 2);
        assert!(r.fired.borrow().is_empty());
    });
}

#[test]
fn forget_clears_the_holds_so_the_next_touch_counts_down() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        p.hold("a");
        p.forget("a");
        p.touch("a");
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW), "the hold went with the forget");
        r.fc.advance(WINDOW);
        assert_eq!(ids(&r.fired), ["a"]);
    });
}

#[test]
fn a_per_workspace_window_overrides_the_default() {
    run_local(async {
        let r = rig();
        r.windows.borrow_mut().insert("fast".into(), Some(WINDOW / 4));
        let p = make(&r);
        p.touch("fast");
        p.touch("slow");
        r.fc.advance(WINDOW / 4);
        assert_eq!(*r.fired.borrow(), vec![("fast".to_string(), WINDOW / 4)]);
        r.fc.advance(WINDOW * 3 / 4);
        assert_eq!(ids(&r.fired), ["fast", "slow"]);
    });
}

#[test]
fn a_nap_that_fails_keeps_its_deadline_and_is_asked_again_every_retry_interval_until_one_lands() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        r.failing.set(true);
        let p = make(&r);
        p.touch("a");
        let deadline = p.idle_at("a").unwrap();
        r.fc.advance(WINDOW);
        settle().await;
        assert_eq!(r.fired.borrow().len(), 1);
        assert_eq!(p.idle_at("a"), Some(deadline));
        r.fc.advance(RETRY - 1);
        assert_eq!(r.fired.borrow().len(), 1);
        r.fc.advance(1);
        settle().await;
        assert_eq!(*r.fired.borrow(), vec![("a".to_string(), WINDOW), ("a".to_string(), WINDOW)]);
        assert_eq!(p.idle_at("a"), Some(deadline));
        assert_eq!(warned.lines(), vec!["idle nap of a failed: fetch failed".to_string(); 2]);
        r.failing.set(false);
        r.fc.advance(RETRY);
        settle().await;
        assert_eq!(r.fired.borrow().len(), 3);
        assert_eq!(p.idle_at("a"), None);
        r.fc.advance(RETRY * 4);
        assert_eq!(r.fired.borrow().len(), 3);
    });
}

#[test]
fn on_backstop_hears_now_plus_twice_the_window_on_every_arming_now_plus_six_hours_when_off_and_nothing_on_forget() {
    run_local(async {
        let r = rig();
        r.windows.borrow_mut().insert("off".into(), None);
        let p = make(&r);
        let t0 = r.fc.now();
        p.touch("a");
        p.touch("off");
        assert_eq!(*r.heard.borrow(), vec![("a".to_string(), t0 + 2 * WINDOW), ("off".to_string(), t0 + IDLE_OFF_BACKSTOP_MS)]);
        // Every arming pushes again, from now: a touch, and the last release of a hold.
        r.fc.advance(1_000);
        p.touch("a");
        p.hold("a");
        p.release("a");
        assert_eq!(r.heard.borrow()[2..], [("a".to_string(), t0 + 1_000 + 2 * WINDOW), ("a".to_string(), t0 + 1_000 + 2 * WINDOW)]);
        // A forget pushes nothing: a napped or deleted machine needs no stop timer.
        p.forget("a");
        p.forget("off");
        assert_eq!(r.heard.borrow().len(), 4);
        assert_eq!(p.idle_at("off"), None);
        assert_eq!(backstop_ms(None), 21_600_000);
        assert_eq!(backstop_ms(Some(WINDOW)), 2 * WINDOW);
    });
}

#[test]
fn a_touch_or_a_forget_during_the_retries_ends_them() {
    run_local(async {
        let _warned = console::capture();
        let r = rig();
        r.failing.set(true);
        let p = make(&r);
        p.touch("a");
        p.touch("b");
        r.fc.advance(WINDOW);
        settle().await;
        assert_eq!(ids(&r.fired), ["a", "b"]);
        p.touch("a");
        assert_eq!(p.idle_at("a"), Some(r.fc.now() + WINDOW));
        p.forget("b");
        assert_eq!(p.idle_at("b"), None);
        r.fc.advance(RETRY * 3);
        settle().await;
        assert_eq!(r.fired.borrow().len(), 2);
        r.fc.advance(WINDOW - RETRY * 3);
        settle().await;
        assert_eq!(ids(&r.fired), ["a", "b", "a"]);
    });
}

#[test]
fn a_release_during_a_nap_in_flight_keeps_the_new_window_when_the_nap_lands() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        r.fc.advance(WINDOW);
        assert_eq!(r.fired.borrow().len(), 1);
        // Before the nap's outcome lands, a release re-arms: the outcome then finds another entry and keeps it.
        p.release("a");
        let rearmed = p.idle_at("a");
        assert_eq!(rearmed, Some(r.fc.now() + WINDOW));
        settle().await;
        assert_eq!(p.idle_at("a"), rearmed);
        r.fc.advance(WINDOW);
        assert_eq!(r.fired.borrow().len(), 2);
    });
}

#[test]
fn close_forgets_every_window_and_arms_nothing_after() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        p.touch("a");
        p.touch("b");
        p.hold("c");
        p.close();
        assert_eq!(r.fc.pending(), 0);
        assert_eq!((p.idle_at("a"), p.idle_at("b")), (None, None));
        let heard = r.heard.borrow().len();
        p.touch("a");
        p.release("c");
        assert_eq!(r.fc.pending(), 0);
        assert_eq!(r.heard.borrow().len(), heard, "a closed policy tells the backstop nothing");
        r.fc.advance(WINDOW * 2);
        assert!(r.fired.borrow().is_empty());
    });
}

#[test]
fn on_idle_may_forget_its_own_workspace_without_a_double_borrow() {
    run_local(async {
        let r = rig();
        let p = make(&r);
        let inner = p.clone();
        *r.during_nap.borrow_mut() = Some(Box::new(move |id| inner.forget(id)));
        p.touch("a");
        r.fc.advance(WINDOW);
        assert_eq!(ids(&r.fired), ["a"]);
        assert_eq!(p.idle_at("a"), None);
        settle().await;
        assert_eq!(p.idle_at("a"), None);
        assert_eq!(r.fc.pending(), 0);
    });
}
