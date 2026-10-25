// SPDX-License-Identifier: AGPL-3.0-only
//! A Codex plan's banked resets, read and spent through `codex app-server` outside any turn. TS
//! adapter-codex/src/reset.ts. Spec: spec/codex_limits.md rules 7-12.

use crate::port::{PlanResets, ResetReading, ResetRoad, ResetSpend};

/// TS reset.ts:342.
pub fn reset_read_command(road: &ResetRoad) -> Result<String, String> {
    todo!()
}

/// TS reset.ts:346.
pub fn reset_credit_command(road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> Result<String, String> {
    todo!()
}

/// `None` where the server never answered initialize. TS reset.ts:375.
pub fn parse_reset_read(stdout: &str) -> Option<ResetReading> {
    todo!()
}

/// TS reset.ts:385.
pub fn parse_reset_credit(stdout: &str) -> ResetSpend {
    todo!()
}

/// Finishes "<agent> on <computer> ..." for a build that answers no resets. TS reset.ts:399.
pub const TOO_OLD: &str = "is older than 0.141 and has no resets";

/// The agent's one registered set of reset roads. TS reset.ts:394.
pub struct CodexPlanResets;

impl PlanResets for CodexPlanResets {
    /// The port's method answers no error where the TS throws (a relative home, a key that is no slug): this panics
    /// with the refusal's words there. Asked of the port's owner to answer `Result`.
    fn read_command(&self, road: &ResetRoad) -> String {
        todo!()
    }
    fn parse_read(&self, stdout: &str) -> Option<ResetReading> {
        todo!()
    }
    fn spend_command(&self, road: &ResetRoad, idempotency_key: &str, credit_id: Option<&str>) -> String {
        todo!()
    }
    fn parse_spend(&self, stdout: &str) -> ResetSpend {
        todo!()
    }
    fn too_old(&self) -> &str {
        todo!()
    }
}
