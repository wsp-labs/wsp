// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/git.ts at 65f6b89c7. Spec: spec/slate_sources.md.

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_wire::types::js_number::JsInt;

use super::context::{HostSlateSource, SlateSourceContext};

/// None without a checkout, else its facts with defaults (spec rule 13).
pub struct GitSource;

impl HostSlateSource for GitSource {
    fn name(&self) -> &'static str {
        "git"
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        futures::future::ready(view_of(ctx)).boxed_local()
    }
}

fn view_of(ctx: &dyn SlateSourceContext) -> Option<Value> {
    let c = ctx.workspace()?.checkout?;
    Some(json!({
        "branch": c.branch,
        "head": c.head,
        "ahead": c.ahead,
        "behind": c.behind,
        "changed": c.changed,
        "stashes": c.stashes.unwrap_or(JsInt::from(0i64)),
        "editsUnread": c.edits_unread.unwrap_or(false),
        "countsUnknown": c.counts_unknown.unwrap_or(false),
        "readAt": c.read_at,
    }))
}
