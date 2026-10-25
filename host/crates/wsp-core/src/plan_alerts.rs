// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/plan-alerts.ts at 65f6b89c7: each limit reading read against the one it replaced,
//! and a blocked account told it is back at its reset by a timer. Spec: spec/plan_alerts.md.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use wsp_wire::types::plan_alerts::UsageAlertEvent;
use wsp_wire::types::usage::AccountLimit;
use wsp_words::exit::HostError;

use crate::clock::{Cancel, Clock};

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
        todo!()
    }

    /// Arms the comeback of every account blocked when the host starts, whose block ends after now. plan-alerts.ts:66.
    pub async fn resume(self: &Rc<Self>) -> Result<(), HostError> {
        todo!()
    }

    /// Cancels every timer. plan-alerts.ts:72.
    pub fn close(&self) {
        todo!()
    }

    /// How many comeback timers stand, for tests and the status page.
    pub fn armed(&self) -> usize {
        todo!()
    }
}

/// plan-alerts.ts:28.
pub fn plan_alerts(deps: PlanAlertsDeps) -> Rc<PlanAlerts> {
    todo!()
}
