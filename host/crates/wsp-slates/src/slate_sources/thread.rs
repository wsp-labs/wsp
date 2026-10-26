// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/thread.ts at 65f6b89c7. Spec: spec/slate_sources.md.

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::{SessionStatus, SessionView};
use wsp_wire::views::session_events::TurnTokens;
use wsp_words::js::utf16;

use super::context::{HostSlateSource, SlateSourceContext};

/// The thread's latest row, its turns, cost, tokens and context (spec rule 9).
pub struct ThreadSource;

impl HostSlateSource for ThreadSource {
    fn name(&self) -> &'static str {
        "thread"
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        futures::future::ready(view_of(ctx)).boxed_local()
    }
}

fn status_of(latest: Option<&SessionView>) -> &'static str {
    let Some(latest) = latest else {
        return "starting";
    };
    match latest.status {
        SessionStatus::Running if latest.asking.is_some() || latest.waiting_on.is_some() => "needs-you",
        SessionStatus::Running => "working",
        SessionStatus::Failed => "failed",
        SessionStatus::Interrupted => "resting",
        SessionStatus::Completed => "done",
    }
}

fn started(row: &SessionView) -> f64 {
    row.started_at.map_or(0.0, |n| n.0)
}

fn view_of(ctx: &dyn SlateSourceContext) -> Option<Value> {
    let rows = ctx.rows();
    let latest = rows.iter().fold(None::<&SessionView>, |a, r| match a {
        Some(a) if started(r) < started(a) => Some(a),
        _ => Some(r),
    });
    let results = ctx.results();
    let last = results.last();
    let total = |pick: fn(&TurnTokens) -> f64| results.iter().filter_map(|r| r.tokens.as_ref()).fold(0.0, |n, t| n + pick(t));
    let with_context = results.iter().rev().filter_map(|r| r.tokens.as_ref()).find(|t| t.context.is_some());
    let used = with_context.and_then(|t| t.context).map(|n| n.0);
    let window = with_context.and_then(|t| t.window).map(|n| n.0);
    let free = match (used, window) {
        (Some(u), Some(w)) => Some(w - u),
        _ => None,
    };
    let percent = match (used, window) {
        (Some(u), Some(w)) if w > 0.0 => Some((u / w) * 100.0),
        _ => None,
    };
    let cost = rows.iter().fold(0.0, |n, r| n + r.cost_usd.map_or(0.0, |c| c.0));
    let facts = ctx.workspace();
    let facts_folder = facts.as_ref().and_then(|f| f.folder.as_deref());

    Some(json!({
        "id": ctx.thread_id(),
        "title": latest.and_then(|l| {
            l.harness_title.clone().or_else(|| {
                l.prompt.as_deref().map(|p| utf16::cut(p.split('\n').next().unwrap_or(""), 80))
            })
        }),
        "agent": latest.map(|l| &l.harness),
        "model": latest.and_then(|l| l.model.as_ref()),
        "effort": latest.and_then(|l| l.effort.as_ref()),
        "status": status_of(latest),
        "access": latest.and_then(|l| l.permission_mode.as_ref()),
        "computer": facts.as_ref().and_then(|f| f.computer.as_ref()),
        "project": facts.as_ref().and_then(|f| f.project.as_ref()),
        "folder": latest.and_then(|l| l.cwd.as_deref()).or(facts_folder),
        "turns": rows.len(),
        "lastTurn": {
            "status": last.map(|r| r.status),
            "durationMs": last.and_then(|r| r.duration_ms),
            "waitedMs": last.and_then(|r| r.waited_ms),
            "startedAt": latest.and_then(|l| l.started_at),
            "endedAt": latest.and_then(|l| l.ended_at),
            "model": latest.and_then(|l| l.model.as_ref()),
        },
        "cost": { "usd": JsNumber(cost) },
        "tokens": {
            "input": JsNumber(total(|t| t.input.0)),
            "output": JsNumber(total(|t| t.output.0)),
            "cached": JsNumber(total(|t| t.cached.map_or(0.0, |n| n.0))),
            "cacheWrite": JsNumber(total(|t| t.cache_write.map_or(0.0, |n| n.0))),
            "reasoning": JsNumber(total(|t| t.reasoning.map_or(0.0, |n| n.0))),
        },
        "context": {
            "used": used.map(JsNumber),
            "window": window.map(JsNumber),
            "free": free.map(JsNumber),
            "percent": percent.map(JsNumber),
        },
        "waitingOn": latest.and_then(|l| l.asking.as_ref()),
    }))
}
