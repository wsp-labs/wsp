// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-writes.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 8.
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::Clock;

// slate-writes.ts:2
// // The agent's budget of slate writes per thread (V751): 5 a second sustained, a burst of 20, 600 an hour.
// const WRITES_BURST = 20;
// const WRITES_PER_SECOND = 5;
// const WRITES_PER_HOUR = 600;
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

// slate-writes.ts:7
// /** Each thread's bucket. `spend` takes one write, or answers how many seconds to wait and takes nothing. */
// export function writeBudget(now: () => number) {
//   const writes = new Map<string, { tokens: number; at: number; hour: number[] }>();
pub fn write_budget(clock: Rc<dyn Clock>) -> WriteBudget {
    todo!()
}

impl WriteBudget {
    // slate-writes.ts:10
    //   return {
    //     spend(threadId: string): number | undefined {
    //       const at = now();
    //       const b = writes.get(threadId) ?? { tokens: WRITES_BURST, at, hour: [] };
    //       b.tokens = Math.min(WRITES_BURST, b.tokens + ((at - b.at) / 1000) * WRITES_PER_SECOND);
    //       b.at = at;
    //       b.hour = b.hour.filter(t => at - t < 3_600_000);
    //       writes.set(threadId, b);
    //       if (b.tokens < 1 || b.hour.length >= WRITES_PER_HOUR) return b.tokens < 1 ? Math.ceil((1 - b.tokens) / WRITES_PER_SECOND) : Math.ceil((3_600_000 - (at - b.hour[0]!)) / 1000);
    //       b.tokens -= 1;
    //       b.hour.push(at);
    //       return undefined;
    /// Takes one write, or answers how many seconds to wait and takes nothing.
    pub fn spend(&self, thread_id: &str) -> Option<i64> {
        todo!()
    }
    // slate-writes.ts:22
    //     },
    pub fn delete(&self, thread_id: &str) -> bool {
        todo!()
    }
}
