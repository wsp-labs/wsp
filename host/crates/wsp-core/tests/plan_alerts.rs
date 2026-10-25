// SPDX-License-Identifier: AGPL-3.0-only
//! spec/plan_alerts.md; ported from packages/runtime/test/plan-alerts.test.ts (5 cases) on the fake clock, plus the
//! cases the spec pins: low alerts, a block with no reset, a second blocked reading, a ledger that lost the account
//! or freed it, a ledger read that fails, the strict `>` at resume, and arming after a close.
mod rest;

use std::cell::RefCell;
use std::rc::Rc;

use futures::FutureExt;
use rest::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::plan_alerts::{plan_alerts, PlanAlerts, PlanAlertsDeps, LONGEST_WAIT_MS};
use wsp_wire::types::usage::AccountLimit;
use wsp_words::exit::HostError;

const HOUR: i64 = 3_600_000;
const START: i64 = 1_000_000;

/// plan-alerts.test.ts:170 `limit`.
fn limit(over: Value) -> AccountLimit {
    let mut base = json!({ "key": "claude:acct", "agent": "claude", "label": "Claude Max", "windows": [], "readAt": 0, "computers": ["here"] });
    for (k, v) in over.as_object().unwrap() {
        base[k] = v.clone();
    }
    wire(&base)
}

struct Keeper {
    fc: FakeClock,
    alerts: Rc<PlanAlerts>,
    said: Rc<RefCell<Vec<Value>>>,
    kept: Rc<RefCell<Result<Vec<AccountLimit>, String>>>,
    reads: Rc<RefCell<usize>>,
}

impl Keeper {
    fn keep(&self, l: Vec<AccountLimit>) {
        *self.kept.borrow_mut() = Ok(l);
    }
    fn alerts_said(&self) -> Vec<Value> {
        self.said.borrow().iter().map(|e| e["alert"].clone()).collect()
    }
    async fn advance(&self, ms: i64) {
        self.fc.advance(ms);
        settle().await;
    }
    fn now(&self) -> i64 {
        self.fc.now()
    }
}

/// plan-alerts.test.ts:172 `keeper`.
fn keeper() -> Keeper {
    let fc = FakeClock::new(START);
    let said: Rc<RefCell<Vec<Value>>> = Rc::default();
    let kept: Rc<RefCell<Result<Vec<AccountLimit>, String>>> = Rc::new(RefCell::new(Ok(vec![])));
    let reads: Rc<RefCell<usize>> = Rc::default();
    let (s, k, r) = (said.clone(), kept.clone(), reads.clone());
    let alerts = plan_alerts(PlanAlertsDeps {
        clock: fc.clock(),
        emit: Rc::new(move |e| s.borrow_mut().push(json(&e))),
        limits: Rc::new(move || {
            *r.borrow_mut() += 1;
            let answer = k.borrow().clone();
            async move { answer.map_err(HostError::new) }.boxed_local()
        }),
    });
    Keeper { fc, alerts, said, kept, reads }
}

fn blocked_until(at: i64, kind: &str) -> AccountLimit {
    limit(json!({ "status": "reached", "windows": [{ "kind": kind, "usedPercent": 100, "resetsAt": at }] }))
}

// TS: plan-alerts.test.ts:181
#[test]
fn says_a_blocked_account_is_back_at_its_reset_by_the_timer_and_the_next_reading_does_not_say_it_again() {
    run_local(async {
        let k = keeper();
        let blocked = blocked_until(k.now() + 2 * HOUR, "session");
        k.keep(vec![blocked.clone()]);
        k.alerts.read(Some(&limit(json!({ "windows": [{ "kind": "session", "usedPercent": 85 }] }))), &blocked);
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })]);
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })]);
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "back" })]);
        let free = limit(json!({ "status": "ok", "windows": [{ "kind": "session", "usedPercent": 3, "resetsAt": k.now() + 5 * HOUR }] }));
        k.alerts.read(Some(&blocked), &free);
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "back" })]);
    });
}

// TS: plan-alerts.test.ts:196
#[test]
fn a_reading_free_of_the_block_before_the_reset_says_it_is_back_and_the_timer_says_nothing_after() {
    run_local(async {
        let k = keeper();
        let blocked = blocked_until(k.now() + 2 * HOUR, "session");
        k.alerts.read(None, &blocked);
        let free = limit(json!({ "status": "ok", "windows": [{ "kind": "session", "usedPercent": 3 }] }));
        k.keep(vec![free.clone()]);
        k.alerts.read(Some(&blocked), &free);
        k.advance(3 * HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "back" })]);
        assert_eq!(k.fc.pending(), 0);
        assert_eq!(k.alerts.armed(), 0);
    });
}

// TS: plan-alerts.test.ts:208
#[test]
fn arms_the_comeback_of_an_account_blocked_when_the_host_starts_and_none_for_one_whose_reset_has_passed() {
    run_local(async {
        let k = keeper();
        let mut passed = blocked_until(k.now() - HOUR, "week");
        passed.key = "codex:a".into();
        k.keep(vec![blocked_until(k.now() + HOUR, "week"), passed]);
        k.alerts.resume().await.unwrap();
        assert_eq!(k.fc.pending(), 1);
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "back" })]);
        assert_eq!(k.said.borrow()[0]["key"], "claude:acct");
    });
}

// TS: plan-alerts.test.ts:217
#[test]
fn a_reset_further_off_than_one_wait_reads_again_on_waking_and_waits_on() {
    run_local(async {
        let k = keeper();
        let blocked = blocked_until(k.now() + 40 * 24 * HOUR, "month");
        k.keep(vec![blocked.clone()]);
        k.alerts.read(None, &blocked);
        k.advance(24 * HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })]);
        assert_eq!(k.fc.pending(), 1);
        assert_eq!(*k.reads.borrow(), 1, "the wake read the ledger once");
    });
}

// TS: plan-alerts.test.ts:227
#[test]
fn closing_lets_every_timer_go() {
    run_local(async {
        let k = keeper();
        k.alerts.read(None, &blocked_until(k.now() + HOUR, "session"));
        assert_eq!(k.fc.pending(), 1);
        k.alerts.close();
        assert_eq!(k.fc.pending(), 0);
        assert_eq!(k.alerts.armed(), 0);
    });
}

// spec: A1 (every alert of a reading is said with the reading after's key, agent and label, in planAlertsOf's order)
#[test]
fn low_alerts_are_said_window_by_window_with_the_accounts_own_names_and_arm_nothing() {
    run_local(async {
        let k = keeper();
        let before = limit(json!({ "windows": [{ "kind": "session", "usedPercent": 60 }, { "kind": "week", "usedPercent": 50 }] }));
        let after = limit(json!({ "key": "codex:b", "agent": "codex", "label": "Codex Pro", "windows": [{ "kind": "session", "usedPercent": 75 }, { "kind": "week", "usedPercent": 95 }] }));
        k.alerts.read(Some(&before), &after);
        assert_eq!(
            *k.said.borrow(),
            vec![
                json!({ "key": "codex:b", "agent": "codex", "label": "Codex Pro", "alert": { "kind": "low", "window": "session", "step": 70 } }),
                json!({ "key": "codex:b", "agent": "codex", "label": "Codex Pro", "alert": { "kind": "low", "window": "week", "step": 90 } }),
            ]
        );
        assert_eq!(k.fc.pending(), 0);
    });
}

// spec: A2 (a block that names no reset is said and arms nothing)
#[test]
fn a_block_with_no_reset_named_is_said_and_no_timer_stands() {
    run_local(async {
        let k = keeper();
        k.alerts.read(None, &limit(json!({ "status": "reached", "windows": [{ "kind": "session", "usedPercent": 100 }] })));
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })]);
        assert_eq!(k.fc.pending(), 0);
        assert_eq!(k.alerts.armed(), 0);
    });
}

// spec: A3, A5 (a blocked reading after a blocked one says nothing and leaves the timer; the timer re-arms on a later end)
#[test]
fn a_second_blocked_reading_says_nothing_and_the_timer_waits_on_for_the_later_reset_it_reads() {
    run_local(async {
        let k = keeper();
        let first = blocked_until(k.now() + HOUR, "session");
        k.alerts.read(None, &first);
        let later = blocked_until(k.now() + 3 * HOUR, "session");
        k.alerts.read(Some(&first), &later);
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })]);
        assert_eq!(k.alerts.armed(), 1);
        k.keep(vec![later.clone()]);
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })], "the ledger still reads blocked to a later reset");
        assert_eq!(k.alerts.armed(), 1);
        k.advance(2 * HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "back" })]);
        assert_eq!(k.alerts.armed(), 0);
    });
}

// spec: A5 (the timer says nothing where the ledger lost the account, or a reading since freed it)
#[test]
fn the_timer_says_nothing_where_the_ledger_no_longer_holds_the_account_or_reads_it_free() {
    run_local(async {
        let k = keeper();
        k.alerts.read(None, &blocked_until(k.now() + HOUR, "session"));
        k.keep(vec![]);
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })]);
        assert_eq!(k.alerts.armed(), 0);

        let mut other = blocked_until(k.now() + HOUR, "session");
        other.key = "codex:c".into();
        k.alerts.read(None, &other);
        let mut free = limit(json!({ "status": "ok", "windows": [{ "kind": "session", "usedPercent": 10 }] }));
        free.key = "codex:c".into();
        k.keep(vec![free]);
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "blocked" })]);
        assert_eq!(k.fc.pending(), 0);
    });
}

// spec: A5 (a ledger read that fails inside the timer is swallowed: nothing said, nothing re-armed)
#[test]
fn a_ledger_read_that_fails_on_waking_says_nothing_and_arms_nothing() {
    run_local(async {
        let k = keeper();
        k.alerts.read(None, &blocked_until(k.now() + HOUR, "session"));
        *k.kept.borrow_mut() = Err("the ledger is gone".into());
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" })]);
        assert_eq!(k.fc.pending(), 0);
        assert_eq!(k.alerts.armed(), 0);
    });
}

// spec: A4 (the wait is max(0, end - now), at most LONGEST_WAIT_MS)
#[test]
fn the_wait_is_the_time_to_the_reset_capped_at_one_day_and_never_negative() {
    run_local(async {
        assert_eq!(LONGEST_WAIT_MS, 86_400_000);
        let k = keeper();
        let blocked = blocked_until(k.now() + 2 * HOUR, "session");
        k.keep(vec![blocked.clone()]);
        k.alerts.read(None, &blocked);
        assert!(!k.fc.due_for(k.now() + 2 * HOUR - 1), "nothing is due a millisecond early");
        assert!(k.fc.due_for(k.now() + 2 * HOUR));

        let late = keeper();
        let passed = blocked_until(late.now() - HOUR, "session");
        late.keep(vec![passed.clone()]);
        late.alerts.read(None, &passed);
        assert!(late.fc.due_for(late.now()), "a reset already past arms a wait of 0");
        late.advance(0).await;
        assert_eq!(late.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "back" })]);
    });
}

// spec: A6 (resume arms only an account whose block ends strictly after now)
#[test]
fn resume_arms_nothing_for_a_block_ending_exactly_now_or_an_account_not_blocked() {
    run_local(async {
        let k = keeper();
        let mut free = limit(json!({ "windows": [{ "kind": "session", "usedPercent": 10, "resetsAt": k.now() + HOUR }] }));
        free.key = "codex:free".into();
        k.keep(vec![blocked_until(k.now(), "session"), free]);
        k.alerts.resume().await.unwrap();
        assert_eq!(k.fc.pending(), 0);
        assert_eq!(k.alerts.armed(), 0);
        assert_eq!(k.alerts_said(), Vec::<Value>::new());
    });
}

// spec: A7 (close stops every timer and leaves the alerts working: a later block arms again)
#[test]
fn a_block_read_after_close_arms_again() {
    run_local(async {
        let k = keeper();
        k.alerts.read(None, &blocked_until(k.now() + HOUR, "session"));
        k.alerts.close();
        let again = blocked_until(k.now() + HOUR, "week");
        k.keep(vec![again.clone()]);
        k.alerts.read(Some(&limit(json!({}))), &again);
        assert_eq!(k.alerts.armed(), 1);
        k.advance(HOUR).await;
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "blocked" }), json!({ "kind": "back" })]);
    });
}

// spec: A3 (a comeback the timer said is kept by the end it said it at; a new block to a new end says back again)
#[test]
fn a_new_block_after_a_timed_comeback_says_its_own_comeback_by_reading() {
    run_local(async {
        let k = keeper();
        let first = blocked_until(k.now() + HOUR, "session");
        k.keep(vec![first.clone()]);
        k.alerts.read(None, &first);
        k.advance(HOUR).await;
        let second = blocked_until(k.now() + HOUR, "session");
        k.alerts.read(Some(&limit(json!({}))), &second);
        let free = limit(json!({ "status": "ok", "windows": [] }));
        k.alerts.read(Some(&second), &free);
        assert_eq!(k.alerts_said(), vec![json!({ "kind": "blocked" }), json!({ "kind": "back" }), json!({ "kind": "blocked" }), json!({ "kind": "back" })]);
        assert_eq!(k.alerts.armed(), 0);
    });
}
