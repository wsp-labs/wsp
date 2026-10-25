// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/pr.ts at 65f6b89c7. Spec: spec/slate_sources.md.
#![allow(unused_variables)]

use futures::future::LocalBoxFuture;
use serde_json::Value;

use super::context::{HostSlateSource, SlateSourceContext};

/// The pull request seen, unread or not seen (spec rule 14).
pub struct PrSource;

impl HostSlateSource for PrSource {
    fn name(&self) -> &'static str {
        todo!()
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        todo!()
    }
}
