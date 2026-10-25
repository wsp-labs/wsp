// SPDX-License-Identifier: AGPL-3.0-only
//! Whether the cloud is on in this process. Port of `packages/host/src/cloud.ts` at 65f6b89c7; the spec is
//! `spec/account.md`.
use std::sync::OnceLock;

use wsp_wire::types::env::CLOUD_ENV;

use crate::hosts::Env;

/// cloud.ts:15: false only in a public build.
pub const CLOUD_BUILT: bool = true;

/// cloud.ts:17 for an environment: `CLOUD_BUILT` and `WSP_CLOUD` exactly `1`.
pub fn cloud_on_in(env: &Env) -> bool {
    CLOUD_BUILT && env.get(CLOUD_ENV).is_some_and(|v| v == "1")
}

/// cloud.ts:17 as this process reads it: the process environment, read once and kept for the process's life.
pub fn cloud_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| CLOUD_BUILT && std::env::var(CLOUD_ENV).is_ok_and(|v| v == "1"))
}
