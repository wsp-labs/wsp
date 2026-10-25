// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/time.ts at 65f6b89c7. Spec: spec/slate_sources.md.
#![allow(unused_variables)]

use futures::future::LocalBoxFuture;
use serde_json::Value;

use super::context::{HostSlateSource, SlateSourceContext};

/// `{now, today, zone}` in the host's zone (spec rule 12).
pub struct TimeSource;

impl HostSlateSource for TimeSource {
    fn name(&self) -> &'static str {
        todo!()
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        todo!()
    }
}
