// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `GoneWatch` (packages/engine/src/golden.ts:200-260).
use std::rc::Rc;
use std::sync::Arc;

use crate::box_backend::Warn;
use crate::errors::{Result, RetryClock};
use crate::gone::KillConfirm;
use crate::machine::{Machine, MachineBackend};

/// golden.ts:193.
pub const GONE_RETRY_MS: u64 = 60_000;

/// golden.ts:195 `GoneWatchOptions`.
#[derive(Default, Clone)]
pub struct GoneWatchOptions {
    pub confirm: KillConfirm,
    pub retry_ms: Option<u64>,
    pub warn: Option<Warn>,
    pub clock: Option<Rc<dyn RetryClock>>,
}

pub struct GoneWatch;

impl GoneWatch {
    pub fn new(_opts: GoneWatchOptions) -> Self {
        todo!()
    }
    pub async fn stop(&self, _backend: Arc<dyn MachineBackend>, _machine: Arc<dyn Machine>) -> Result<()> {
        todo!()
    }
    pub fn ids(&self) -> Vec<String> {
        todo!()
    }
    pub async fn settled(&self) {
        todo!()
    }
    pub fn close(&self) {
        todo!()
    }
}
