// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `sweepLeftovers` and `SWEEP_STAGED_CMD` (packages/engine/src/golden-tools.ts:181).
use crate::errors::Result;
use crate::machine::Machine;

pub const SWEEP_STAGED_CMD: &str = "";

pub async fn sweep_leftovers(_machine: &dyn Machine) -> Result<()> {
    todo!()
}
