// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-writes.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 8.
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::Clock;

pub const WRITES_BURST: f64 = 20.0;
pub const WRITES_PER_SECOND: f64 = 5.0;
pub const WRITES_PER_HOUR: usize = 600;

/// One thread's bucket.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Bucket {
    pub(crate) tokens: f64,
    pub(crate) at: i64,
    pub(crate) hour: Vec<i64>,
}

/// Each thread's write budget.
pub struct WriteBudget {
    pub(crate) clock: Rc<dyn Clock>,
    pub(crate) writes: RefCell<IndexMap<String, Bucket>>,
}

pub fn write_budget(clock: Rc<dyn Clock>) -> WriteBudget {
    WriteBudget { clock, writes: RefCell::new(IndexMap::new()) }
}

impl WriteBudget {
    /// Takes one write, or answers how many seconds to wait and takes nothing.
    pub fn spend(&self, thread_id: &str) -> Option<i64> {
        let at = self.clock.now();
        let mut writes = self.writes.borrow_mut();
        let mut b = writes.get(thread_id).cloned().unwrap_or(Bucket { tokens: WRITES_BURST, at, hour: Vec::new() });
        b.tokens = f64::min(WRITES_BURST, b.tokens + ((at - b.at) as f64 / 1000.0) * WRITES_PER_SECOND);
        b.at = at;
        b.hour.retain(|t| at - t < 3_600_000);
        let wait = if b.tokens < 1.0 {
            Some(((1.0 - b.tokens) / WRITES_PER_SECOND).ceil() as i64)
        } else if b.hour.len() >= WRITES_PER_HOUR {
            Some(((3_600_000 - (at - b.hour[0])) as f64 / 1000.0).ceil() as i64)
        } else {
            None
        };
        if wait.is_none() {
            b.tokens -= 1.0;
            b.hour.push(at);
        }
        writes.insert(thread_id.to_string(), b);
        wait
    }

    pub fn delete(&self, thread_id: &str) -> bool {
        self.writes.borrow_mut().shift_remove(thread_id).is_some()
    }
}
