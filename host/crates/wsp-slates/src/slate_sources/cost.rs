// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/cost.ts at 65f6b89c7. Spec: spec/slate_sources.md.
#![allow(unused_variables)]

use futures::future::LocalBoxFuture;
use serde_json::Value;

use super::context::{HostSlateSource, SlateSourceContext};

/// `{rateUsdPerHour, accruedUsd}`, each 0 when absent (spec rule 11).
pub struct CostSource;

impl HostSlateSource for CostSource {
    fn name(&self) -> &'static str {
        todo!()
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        todo!()
    }
}
