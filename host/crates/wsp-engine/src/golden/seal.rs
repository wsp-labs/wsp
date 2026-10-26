// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:56-130 and 1067-1228: the second half of a golden. Snapshots the builder, proves the snapshot boots by
//! smoke-testing a fork, and appends a manifest version.
use wsp_words::words::image::ProviderAnswer;

use super::types::{Builder, GoldenError, SealGoldenOptions, SealResult};
use crate::errors::{EngineError, RetryClock};

/// golden.ts:59.
pub const SNAPSHOT_ATTEMPTS: u32 = 3;
/// golden.ts:60.
pub const SNAPSHOT_RETRY_MS: u64 = 60_000;

/// golden.ts:94. The provider's answer as the failure line reports it: a refusal carries its status; a failure that
/// came with none is reported as the provider's 500.
pub fn answer_of(_e: &EngineError, _now_ms: i64) -> ProviderAnswer {
    todo!("golden.ts:94")
}

/// golden.ts:1071. Sequenced for a two-machine cap: unless the builder is kept, it dies before the smoke fork boots.
pub async fn seal_golden(_builder: Builder, _opts: SealGoldenOptions, _clock: &dyn RetryClock) -> Result<SealResult, GoldenError> {
    todo!("golden.ts:1071")
}
