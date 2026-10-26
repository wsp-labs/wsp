// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/plan-alerts.ts at 65f6b89c7: each limit reading read against the one it replaced,
//! and a blocked account told it is back at its reset by a timer. Spec: spec/plan_alerts.md.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use wsp_wire::types::plan_alerts::{PlanAlert, PlanAlertBack, UsageAlertEvent};
use wsp_wire::types::usage::AccountLimit;
use wsp_words::exit::HostError;
use wsp_words::plan_alerts::{block_ends_at, plan_alerts_of, plan_blocked};

use crate::clock::{Cancel, Clock, Timer};

/// How long one timer waits at most, 24 hours: a reset further off wakes, reads again and waits on. plan-alerts.ts:11.
pub const LONGEST_WAIT_MS: i64 = 86_400_000;

/// plan-alerts.ts:13.
pub struct PlanAlertsDeps {
    pub clock: Rc<dyn Clock>,
    pub emit: Rc<dyn Fn(UsageAlertEvent)>,
    /// Every account's last reading, as the ledger keeps it.
    pub limits: Rc<dyn Fn() -> LocalBoxFuture<'static, Result<Vec<AccountLimit>, HostError>>>,
}

/// The host's plan alerts. Nothing survives a restart; `resume` re-arms from the ledger. plan-alerts.ts:20.
pub struct PlanAlerts {
    deps: PlanAlertsDeps,
    /// Each blocked account's comeback timer, by account key.
    timers: RefCell<IndexMap<String, Cancel>>,
    /// The comebacks a timer said, as (account key, the block's end as `block_ends_at` read it, its f64 bits).
    back_said: RefCell<IndexSet<(String, Option<u64>)>>,
}

impl PlanAlerts {
    /// One reading kept, against the one it replaced. plan-alerts.ts:56.
    pub fn read(self: &Rc<Self>, before: Option<&AccountLimit>, after: &AccountLimit) {
        for alert in plan_alerts_of(before, after, self.deps.clock.now() as f64) {
            if matches!(alert, PlanAlert::Back(_)) {
                self.cancel(&after.key);
                if let Some(before) = before {
                    if self.back_said.borrow_mut().shift_remove(&said(before)) {
                        continue;
                    }
                }
            }
            let blocked = matches!(alert, PlanAlert::Blocked(_));
            self.say(after, alert);
            if blocked {
                self.arm(after);
            }
        }
    }

    /// Arms the comeback of every account blocked when the host starts, whose block ends after now. plan-alerts.ts:66.
    pub async fn resume(self: &Rc<Self>) -> Result<(), HostError> {
        for l in (self.deps.limits)().await? {
            if block_ends_at(&l).is_some_and(|end| end > self.deps.clock.now() as f64) {
                self.arm(&l);
            }
        }
        Ok(())
    }

    /// Cancels every timer. plan-alerts.ts:72.
    pub fn close(&self) {
        let stops: Vec<Cancel> = self.timers.borrow_mut().drain(..).map(|(_, stop)| stop).collect();
        for stop in stops {
            stop();
        }
    }

    /// How many comeback timers stand, for tests and the status page.
    pub fn armed(&self) -> usize {
        self.timers.borrow().len()
    }

    fn say(&self, l: &AccountLimit, alert: PlanAlert) {
        (self.deps.emit)(UsageAlertEvent { key: l.key.clone(), agent: l.agent.clone(), label: l.label.clone(), alert });
    }

    fn cancel(&self, key: &str) {
        let stop = self.timers.borrow_mut().shift_remove(key);
        if let Some(stop) = stop {
            stop();
        }
    }

    /// plan-alerts.ts:38.
    fn arm(self: &Rc<Self>, blocked: &AccountLimit) {
        self.cancel(&blocked.key);
        let Some(end) = block_ends_at(blocked) else {
            return;
        };
        let now = self.deps.clock.now() as f64;
        let wait = (end - now).max(0.0).min(LONGEST_WAIT_MS as f64).trunc() as i64;
        let key = blocked.key.clone();
        let weak = Rc::downgrade(self);
        let timer: Timer = Box::new(move || {
            if let Some(this) = weak.upgrade() {
                tokio::task::spawn_local(async move {
                    this.fire(key).await;
                });
            }
        });
        let stop = self.deps.clock.schedule(timer, wait);
        self.timers.borrow_mut().insert(blocked.key.clone(), stop);
    }

    /// plan-alerts.ts:42: the comeback a timer reads off the ledger, said or armed again.
    async fn fire(self: Rc<Self>, key: String) {
        self.timers.borrow_mut().shift_remove(&key);
        let Ok(limits) = (self.deps.limits)().await else {
            return;
        };
        let Some(now) = limits.iter().find(|l| l.key == key) else {
            return;
        };
        if !plan_blocked(now) {
            return;
        }
        let next = block_ends_at(now);
        if next.is_some_and(|next| next > self.deps.clock.now() as f64) {
            return self.arm(now);
        }
        self.back_said.borrow_mut().insert(said(now));
        self.say(now, PlanAlert::Back(PlanAlertBack {}));
    }
}

/// The mark a comeback is said under: the account, and the block's end as `block_ends_at` reads it.
fn said(l: &AccountLimit) -> (String, Option<u64>) {
    (l.key.clone(), block_ends_at(l).map(f64::to_bits))
}

/// plan-alerts.ts:28.
pub fn plan_alerts(deps: PlanAlertsDeps) -> Rc<PlanAlerts> {
    Rc::new(PlanAlerts { deps, timers: RefCell::new(IndexMap::new()), back_said: RefCell::new(IndexSet::new()) })
}
