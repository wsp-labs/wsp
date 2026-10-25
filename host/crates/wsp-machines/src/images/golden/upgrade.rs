// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:292-459 at 65f6b89c7: upgrade, builderReach, builders and kill. Spec: `spec/golden_upgrade.md`.
use std::rc::Rc;

use wsp_core::types::wiring::{GoldenRecipe, GoldenUpgradeResult};
use wsp_wire::views::golden_image::{GoldenBuilderView, GoldenLogin};
use wsp_wire::views::workspace::DaemonReachView;
use wsp_words::exit::HostError;

use super::GoldenArea;
use crate::images::engine::GoldenDelta;
use crate::images::ImageResult;

/// api.ts:616 `golden.upgrade`'s ask. wsp-core's `api::UpgradeAsk` carries the empty `pending::GoldenDelta`; this one
/// carries the engine's delta, whose `import` and `retiredOnImage` the update reads.
#[derive(Clone, Default)]
pub struct UpgradeRequest {
    pub name: Option<String>,
    pub delta: GoldenDelta,
    pub keep_previous: Option<bool>,
    pub logins: Option<Vec<GoldenLogin>>,
    pub recipe: Option<GoldenRecipe>,
}

impl GoldenArea {
    /// golden.ts:292-415 (U1 to U8).
    pub async fn upgrade(self: &Rc<Self>, o: UpgradeRequest) -> ImageResult<GoldenUpgradeResult> {
        todo!()
    }

    /// golden.ts:418-428 (U9).
    pub async fn builder_reach(&self, builder_id: &str) -> ImageResult<DaemonReachView> {
        todo!()
    }

    /// golden.ts:430-434 (U10).
    pub async fn builders(&self) -> Result<Vec<GoldenBuilderView>, HostError> {
        todo!()
    }

    /// golden.ts:436-459 (U11, U12).
    pub async fn kill(&self, builder_id: &str) -> ImageResult<()> {
        todo!()
    }
}
