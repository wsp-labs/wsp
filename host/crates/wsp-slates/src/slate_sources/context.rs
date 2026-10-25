// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-sources/context.ts at 65f6b89c7. Spec: spec/slate_sources.md (rules 1-3).
#![allow(unused_variables)]

use futures::future::LocalBoxFuture;
use serde_json::Value;

pub use wsp_wire::types::changes::Checkout;
pub use wsp_wire::types::pull_request::PullRequestSeen;
pub use wsp_wire::types::usage::AccountRow;
pub use wsp_wire::views::session::SessionView;
pub use wsp_wire::views::session_events::TurnResult;

/// What a host source reads a slate's sources off; the runtime fills it per read (context.ts:7).
pub trait SlateSourceContext {
    fn thread_id(&self) -> &str;
    fn workspace_id(&self) -> &str;
    /// Epoch ms, read once when the context was made.
    fn now(&self) -> i64;
    /// The thread's rows, oldest first.
    fn rows(&self) -> Vec<SessionView>;
    /// The results of the thread's ended turns, oldest first.
    fn results(&self) -> Vec<TurnResult>;
    /// The account the thread runs on, as the Usage page lists it.
    fn account(&self) -> LocalBoxFuture<'_, Option<AccountRow>>;
    fn workspace(&self) -> Option<SlateWorkspaceFacts>;
}

/// context.ts:20 SlateWorkspaceFacts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlateWorkspaceFacts {
    pub computer: Option<String>,
    pub project: Option<String>,
    pub folder: Option<String>,
    pub checkout: Option<Checkout>,
    pub pr: Option<PullRequestSeen>,
    pub rate_usd_per_hour: Option<f64>,
    pub accrued_usd: Option<f64>,
}

/// One source on the host: its first path segment and the whole value its paths are read out of; `None` has not
/// arrived (context.ts:32).
pub trait HostSlateSource {
    fn name(&self) -> &'static str;
    fn view<'a>(&'a self, ctx: &'a dyn SlateSourceContext) -> LocalBoxFuture<'a, Option<Value>>;
}

// context.ts:38 json(value)
/// A value as JSON with absent fields dropped and none as `null`.
pub fn json<T: serde::Serialize>(value: Option<&T>) -> Value {
    todo!()
}
