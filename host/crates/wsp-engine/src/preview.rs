// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/preview.ts. Spec: spec/machine_reads.md.
use wsp_wire::wire::machine_link::PreviewReach;

use crate::errors::Result;
use crate::machine::Machine;

/// preview.ts:7. Reuse a minted reach while more than this remains before its expiry.
pub const PREVIEW_REFRESH_MARGIN_MS: i64 = 10 * 60_000;

/// preview.ts:11. The in-guest daemon's port.
pub const DAEMON_PORT: u16 = 7070;

/// preview.ts:16. Whether the daemon is serving, asked from inside the guest through bash's own network road.
pub const DAEMON_LISTENING_CHECK: &str = "(exec 3<>/dev/tcp/127.0.0.1/7070) 2>/dev/null";

/// preview.ts:18. Fresh while strictly more than the margin remains: `expires_at - now > PREVIEW_REFRESH_MARGIN_MS`.
pub fn preview_is_fresh(reach: &PreviewReach, now: i64) -> bool {
    todo!()
}

/// preview.ts:25. `current` back while it is fresh; else a minted one from `machine.preview_url(port)`. A machine whose
/// offers have no `preview_url` is an `Other`: `machine <id> is on a backend without preview URLs`, nothing called.
pub async fn refresh_preview_token(machine: &dyn Machine, port: u16, current: Option<&PreviewReach>, now: i64) -> Result<PreviewReach> {
    todo!()
}
