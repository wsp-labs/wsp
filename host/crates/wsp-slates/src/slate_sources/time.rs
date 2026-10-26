// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/time.ts at 65f6b89c7. Spec: spec/slate_sources.md.

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};

use super::context::{HostSlateSource, SlateSourceContext};

/// `{now, today, zone}` in the host's zone (spec rule 12).
pub struct TimeSource;

impl HostSlateSource for TimeSource {
    fn name(&self) -> &'static str {
        "time"
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        futures::future::ready(view_of(ctx)).boxed_local()
    }
}

fn view_of(ctx: &dyn SlateSourceContext) -> Option<Value> {
    let now = ctx.now();
    let zone = jiff::tz::TimeZone::system();
    let name = zone.iana_name().unwrap_or("UTC");
    let today = jiff::Timestamp::from_millisecond(now)
        .map(|at| {
            let date = at.to_zoned(zone.clone()).date();
            format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
        })
        .unwrap_or_default();
    Some(json!({ "now": now, "today": today, "zone": name }))
}
