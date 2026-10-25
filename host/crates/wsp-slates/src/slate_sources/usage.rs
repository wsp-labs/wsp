// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/usage.ts at 65f6b89c7. Spec: spec/slate_sources.md.
#![allow(unused_variables)]

use futures::future::LocalBoxFuture;
use serde_json::Value;

use super::context::{HostSlateSource, SlateSourceContext};

/// The account's windows, burn and credits; none without an account (spec rule 10).
pub struct UsageSource;

impl HostSlateSource for UsageSource {
    fn name(&self) -> &'static str {
        todo!()
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        todo!()
    }
}
