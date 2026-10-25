// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/waits.ts at 65f6b89c7. Spec: spec/waits.md.
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_wire::views::session_events_more::SessionEvent;

use crate::threads::threads::Sessions;

/// waits.ts:94.
pub const WAIT_SETTLE_MS: i64 = 2_000;
/// waits.ts:98.
pub const RETRY_SETTLE_MS: i64 = 2_000;

/// What the waits area reads of the runtime.
pub trait WaitsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn sessions(&self) -> Sessions;
    /// `ctx.state.closing`, read at the moment of each write.
    fn closing(&self) -> bool;
    /// `ctx.record(event)`: the transcript row, and the bus.
    fn record(&self, event: SessionEvent);
}

/// A retry the host saw (TS `{ attempt, why }`).
#[derive(Clone, Debug, PartialEq)]
pub struct RetryNote {
    pub attempt: u32,
    pub why: String,
}

#[derive(Default)]
struct WaitsState {
    settling: IndexMap<String, f64>,
    said: IndexMap<String, String>,
    unsaid: IndexMap<String, Vec<Rc<RetryNote>>>,
}

/// waits.ts:102. Clones share one state.
#[derive(Clone)]
pub struct WaitsArea {
    ctx: Rc<dyn WaitsCtx>,
    state: Rc<RefCell<WaitsState>>,
}

impl WaitsArea {
    pub fn new(ctx: Rc<dyn WaitsCtx>) -> Self {
        todo!()
    }

    /// waits.ts:154. `text` is the host's sentence where it answered with one. Rules W1-W8.
    pub fn note_waited(&self, thread_id: &str, ms: f64, text: Option<&str>) {
        todo!()
    }

    /// waits.ts:175. Rules W9, W10.
    pub fn note_retry(&self, thread_id: &str, retry: RetryNote) {
        todo!()
    }

    /// waits.ts:118. Rule W11.
    pub fn agent_retried(&self, turn_id: &str, why: Option<&str>) {
        todo!()
    }
}
