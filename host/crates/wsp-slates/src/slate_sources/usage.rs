// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/usage.ts at 65f6b89c7. Spec: spec/slate_sources.md.

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_wire::types::usage::{LimitKind, LimitWindow};

use super::context::{HostSlateSource, SlateSourceContext};

/// The account's windows, burn and credits; none without an account (spec rule 10).
pub struct UsageSource;

impl HostSlateSource for UsageSource {
    fn name(&self) -> &'static str {
        "usage"
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        view_of(ctx).boxed_local()
    }
}

async fn view_of(ctx: &dyn SlateSourceContext) -> Option<Value> {
    let row = ctx.account().await?;
    let windows: &[LimitWindow] = row.windows.as_deref().unwrap_or(&[]);
    let window = |kind: LimitKind| {
        let w = windows.iter().find(|w| w.kind == kind);
        json!({ "percent": w.map(|w| w.used_percent), "resetsAt": w.and_then(|w| w.resets_at) })
    };
    Some(json!({
        "account": {
            "key": row.key,
            "label": row.label,
            "agent": row.agent,
            "plan": row.plan,
            "address": row.address,
            "computers": row.computers,
        },
        "windows": windows
            .iter()
            .map(|w| json!({ "kind": w.kind, "usedPercent": w.used_percent, "resetsAt": w.resets_at }))
            .collect::<Vec<Value>>(),
        "session": window(LimitKind::Session),
        "week": window(LimitKind::Week),
        "status": row.status,
        "note": row.note,
        "burn": {
            "tokensPerMinute": row.burn.as_ref().map(|b| b.tokens_per_minute),
            "threads": row.burn.as_ref().map(|b| b.threads),
        },
        "credits": {
            "count": row.credits.as_ref().map(|c| c.count),
            "nextExpiresAt": row.credits.as_ref().and_then(|c| c.next_expires_at),
        },
        "readAt": row.read_at,
    }))
}
