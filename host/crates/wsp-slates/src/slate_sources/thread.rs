// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/thread.ts at 65f6b89c7. Spec: spec/slate_sources.md.
#![allow(unused_variables)]

use futures::future::LocalBoxFuture;
use serde_json::Value;

use super::context::{HostSlateSource, SlateSourceContext};

/// The thread's latest row, its turns, cost, tokens and context (spec rule 9).
pub struct ThreadSource;

impl HostSlateSource for ThreadSource {
    fn name(&self) -> &'static str {
        todo!()
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        todo!()
    }
}
