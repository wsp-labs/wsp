// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `sweepLeftovers` and `SWEEP_STAGED_CMD` (packages/engine/src/golden-tools.ts:181).
use crate::errors::Result;
use crate::exec_detached::INLINE_EXEC_MS;
use crate::golden_seam::{sweep_caches, TOOLS_PATH};
use crate::machine::{ExecOptions, Machine};

/// golden-tools.ts:177. The staging wsp's own roads make under /tmp on a machine and take away as they end, which a road
/// cut short leaves behind; one untouched for an hour is no road's still in flight. Named one by one, since
/// /tmp/wsp-slates is live. `STAGED_PREFIXES` joined, as node prints it.
pub const SWEEP_STAGED_CMD: &str = "find /tmp -maxdepth 1 \\( -name 'wsp-put-*' -o -name 'wsp-get-*' -o -name 'wsp-seed-*' -o -name 'wsp-session-*' -o -name 'wsp-vault-*' -o -name 'wsp-land-*' -o -name 'wsp-road-*' -o -name 'wsp-cask-*' -o -name 'wsp-draft-*' \\) -mmin +60 -exec rm -rf -- {} +";

/// golden-tools.ts:181. What wsp made on a machine that it can take back when the disk runs short: its stale staging,
/// then the build caches, each rebuilt on use. Work in a folder is never touched. The caches' phrase is not read, as the
/// TS does not read it; a failure of either road is the sweep's.
pub async fn sweep_leftovers(machine: &dyn Machine) -> Result<()> {
    machine.exec(SWEEP_STAGED_CMD, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    sweep_caches(machine, TOOLS_PATH).await?;
    Ok(())
}
