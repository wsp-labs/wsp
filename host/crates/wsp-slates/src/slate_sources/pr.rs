// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/pr.ts at 65f6b89c7. Spec: spec/slate_sources.md.

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_wire::types::pull_request::{PullRequestCheck, PullRequestSeen};
use wsp_words::pull_request::pull_request_word;

use super::context::{HostSlateSource, SlateSourceContext};

/// The pull request seen, unread or not seen (spec rule 14).
pub struct PrSource;

impl HostSlateSource for PrSource {
    fn name(&self) -> &'static str {
        "pr"
    }
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>> {
        futures::future::ready(view_of(ctx)).boxed_local()
    }
}

fn view_of(ctx: &dyn SlateSourceContext) -> Option<Value> {
    let Some(seen) = ctx.workspace().and_then(|f| f.pr) else {
        return Some(json!({ "number": null, "word": null, "unread": null, "checks": [] }));
    };
    let word = pull_request_word(&seen);
    let (fact, checks, number, url, state, base, read_at) = match &seen {
        PullRequestSeen::PullRequestUnread(u) => {
            return Some(json!({ "number": null, "word": word, "unread": u.why, "checks": [], "readAt": u.read_at }));
        }
        PullRequestSeen::PullRequestFact(f) => (Some(f), f.checks.as_slice(), f.number, &f.url, &f.state, &f.base, f.read_at),
        PullRequestSeen::PullRequestKept(k) => (None, k.checks.as_deref().unwrap_or(&[]), k.number, &k.url, &k.state, &k.base, k.read_at),
    };
    Some(json!({
        "number": number,
        "url": url,
        "state": state,
        "base": base,
        "draft": fact.map(|f| f.draft),
        "branch": fact.map(|f| &f.branch),
        "headOid": fact.map(|f| &f.head_oid),
        "headSubject": fact.map(|f| &f.head_subject),
        "mergeable": fact.map(|f| &f.mergeable),
        "review": fact.map(|f| &f.review),
        "checks": checks.iter().map(check_view).collect::<Vec<Value>>(),
        "additions": fact.map(|f| f.additions),
        "deletions": fact.map(|f| f.deletions),
        "changedFiles": fact.map(|f| f.changed_files),
        "commits": fact.map(|f| f.commits),
        "behindBase": fact.and_then(|f| f.behind_base),
        "author": fact.and_then(|f| f.author.as_ref()),
        "readAt": read_at,
        "word": word,
        "unread": null,
    }))
}

fn check_view(c: &PullRequestCheck) -> Value {
    json!({
        "name": c.name,
        "workflow": c.workflow,
        "state": c.state,
        "link": c.link,
        "description": c.description,
        "startedAt": c.started_at,
        "completedAt": c.completed_at,
    })
}
