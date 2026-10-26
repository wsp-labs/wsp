// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/cost.ts at 65f6b89c7. Spec: spec/slate_sources.md.

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_wire::types::js_number::JsNumber;

use super::context::{HostSlateSource, SlateSourceContext};

/// `{rateUsdPerHour, accruedUsd}`, each 0 when absent (spec rule 11).
pub struct CostSource;

impl HostSlateSource for CostSource {
    fn name(&self) -> &'static str {
        "cost"
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        futures::future::ready(view_of(ctx)).boxed_local()
    }
}

fn view_of(ctx: &dyn SlateSourceContext) -> Option<Value> {
    let facts = ctx.workspace();
    Some(json!({
        "rateUsdPerHour": JsNumber(facts.as_ref().and_then(|f| f.rate_usd_per_hour).unwrap_or(0.0)),
        "accruedUsd": JsNumber(facts.as_ref().and_then(|f| f.accrued_usd).unwrap_or(0.0)),
    }))
}
