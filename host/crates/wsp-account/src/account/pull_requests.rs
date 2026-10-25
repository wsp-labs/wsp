// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/pull-requests.ts at 65f6b89c7: the git host read for a workspace and the pull
//! request kept on it. Holds runtime invariant 4. Spec: spec/pull_requests.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::pull_request::{GitPrViewReply, GitRepoReadReply, GitUpdateReply, PullRequestSeen};
use wsp_wire::types::tree::TreeRecord;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_words::exit::HostError;

use wsp_core::clock::Cancel;
use crate::account::core_types::{AccountError, AccountResult, Ask, Entry, Frame, Work};

/// pull-requests.ts:278: how long a remote's repository settings are held.
pub const REPO_SETTINGS_MS: i64 = 60 * 60_000;

/// One GitHub cache entry (`GitHubCache`, github-cache.ts): the body, the tag a read saw, when it was fetched.
#[derive(Clone, Debug, PartialEq)]
pub struct CacheEntry {
    pub body: Value,
    pub tag: Option<String>,
    pub fetched_at: i64,
}

/// A read that says whether this computer's own command line answered it (pull-requests.ts:25).
#[derive(Clone, Debug, PartialEq)]
pub struct ReadOn<T> {
    pub read: T,
    pub here: bool,
}

/// A frame built for the folder a road runs in: this computer's home, or the copy's checkout.
pub type FrameFor = Rc<dyn Fn(&str) -> Frame>;

/// What the area reads off the runtime's context.
#[async_trait(?Send)]
pub trait PullRequestsCtx {
    fn now(&self) -> i64;
    /// `clock.schedule(f, ms, { unref: true })`.
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel;
    fn cache_get(&self, key: &str) -> Option<CacheEntry>;
    fn cache_set(&self, key: &str, entry: CacheEntry);
    fn cache_delete(&self, key: &str);
    /// `live.get(id)`.
    fn live(&self, workspace_id: &str) -> Option<Entry>;
    /// `sessions.values()`' views, in map order.
    fn session_views(&self) -> Vec<SessionView>;
    /// `ctx.onThisComputer((ask, home) => ...)` (channels_daemon D18).
    async fn on_this_computer(&self, work: Box<dyn FnOnce(Ask, String) -> futures::future::LocalBoxFuture<'static, AccountResult<Value>>>) -> AccountResult<Value>;
    /// `ctx.withDaemon(entry, ask => ...)` (channels_daemon D14).
    async fn with_daemon(&self, entry: &Entry, work: Work<Value>) -> AccountResult<Value>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError>;
    async fn entry_of(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<Entry, HostError>;
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError>;
    /// `ctx.status.watched()`: a window is open.
    fn status_watched(&self) -> bool;
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()>;
    async fn status_now(&self, entry: &Entry);
    /// `ctx.readCheckout(entry, force)`: the branch line, read or held.
    async fn read_checkout(&self, entry: &Entry, force: bool) -> AccountResult<Option<wsp_wire::types::changes::Checkout>>;
    /// `ctx.slates.watchesPr(id)`.
    fn slates_watch_pr(&self, workspace_id: &str) -> bool;
    /// `ctx.mark(threads, { readAt, settledAt }, undefined)`.
    async fn mark_settled(&self, threads: Vec<String>, at: i64) -> AccountResult<()>;
    /// `ctx.pauses(record)` (rules R4).
    fn pauses(&self, record: &WorkspaceRecord) -> bool;
    async fn nap_with(&self, workspace_id: &str) -> AccountResult<()>;
    fn read_lead_of(&self, child: &Entry);
    /// `ctx.queued(id, work)`: the workspace's one-at-a-time queue.
    async fn queued(&self, workspace_id: &str, work: futures::future::LocalBoxFuture<'static, AccountResult<Value>>) -> AccountResult<Value>;
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
}

/// pull-requests.ts:9 `pullRequestsArea(ctx)`.
pub struct PullRequestsArea {
    pub(crate) ctx: Rc<dyn PullRequestsCtx>,
    /// pull-requests.ts:37: road → until and the refusal met.
    pub(crate) rate_limited: RefCell<IndexMap<String, (i64, AccountError)>>,
    /// pull-requests.ts:277.
    pub(crate) repo_settings: RefCell<IndexMap<String, (i64, GitRepoReadReply)>>,
}

impl PullRequestsArea {
    pub fn new(ctx: Rc<dyn PullRequestsCtx>) -> Rc<PullRequestsArea> {
        todo!()
    }

    /// Q1, pull-requests.ts:20: `parse` reads the reply into the caller's type.
    pub async fn read_host<T: 'static>(self: &Rc<Self>, entry: &Entry, frame: FrameFor, parse: Rc<dyn Fn(Frame) -> AccountResult<T>>) -> AccountResult<T> {
        todo!()
    }

    /// Q1, pull-requests.ts:25.
    pub async fn read_host_on<T: 'static>(self: &Rc<Self>, entry: &Entry, frame: FrameFor, parse: Rc<dyn Fn(Frame) -> AccountResult<T>>) -> AccountResult<ReadOn<T>> {
        todo!()
    }

    /// Q2, pull-requests.ts:38. `road` is `"here"` or the workspace id.
    pub async fn held_off<T: 'static>(self: &Rc<Self>, road: &str, read: futures::future::LocalBoxFuture<'static, AccountResult<T>>) -> AccountResult<T> {
        todo!()
    }

    /// Q3, pull-requests.ts:50.
    pub async fn pull_request_on(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(Entry, String, i64), HostError> {
        todo!()
    }

    /// Q3, pull-requests.ts:57.
    pub fn page_key(remote: &str, number: i64) -> String {
        todo!()
    }

    /// Q3, pull-requests.ts:90.
    pub fn pr_key(remote: &str, number: i64) -> String {
        todo!()
    }

    /// Q4, pull-requests.ts:61.
    pub async fn read_page(self: &Rc<Self>, entry: &Entry, remote: &str, number: i64, fresh: bool) -> AccountResult<ReadOn<GitPrViewReply>> {
        todo!()
    }

    /// Q5, pull-requests.ts:74. Invariant 4: this computer's command line alone.
    pub async fn post_as_person<T: 'static>(self: &Rc<Self>, remote: &str, number: i64, frame: FrameFor, parse: Rc<dyn Fn(Frame) -> AccountResult<T>>) -> AccountResult<T> {
        todo!()
    }

    /// Q5, pull-requests.ts:85.
    pub fn host_of_remote(remote: &str) -> String {
        todo!()
    }

    /// Q6, pull-requests.ts:88.
    pub fn pr_poll_ms(&self) -> i64 {
        todo!()
    }

    /// Q7, pull-requests.ts:99.
    pub async fn read_pull_request(self: &Rc<Self>, entry: &Entry, force: bool, whole: bool) -> Option<PullRequestSeen> {
        todo!()
    }

    /// Q9, pull-requests.ts:158.
    pub fn left_branch(&self, entry: &Entry) -> bool {
        todo!()
    }

    /// Q10, pull-requests.ts:171.
    pub async fn drop_pull_request(self: &Rc<Self>, entry: &Entry) -> AccountResult<()> {
        todo!()
    }

    /// Q11, pull-requests.ts:185.
    pub async fn ran_on_branch(&self, entry: &Entry, thread_id: &str) -> AccountResult<()> {
        todo!()
    }

    /// Q8, pull-requests.ts:203.
    pub async fn take_pull_request(self: &Rc<Self>, entry: &Entry, seen: Option<PullRequestSeen>) -> AccountResult<()> {
        todo!()
    }

    /// Q12, pull-requests.ts:224. `from` defaults to now at the call sites that name none.
    pub fn poll_pull_request(self: &Rc<Self>, entry: &Entry, from: Option<i64>) {
        todo!()
    }

    /// Q16, pull-requests.ts:244.
    pub async fn keep_tree(&self, child: &Entry, kept: TreeRecord) -> AccountResult<()> {
        todo!()
    }

    /// Q13, pull-requests.ts:256.
    pub async fn settle_tree(self: &Rc<Self>, entry: &Entry) {
        todo!()
    }

    /// Q14, pull-requests.ts:279.
    pub async fn merge_settings(self: &Rc<Self>, remote: &str) -> AccountResult<GitRepoReadReply> {
        todo!()
    }

    /// Q15, pull-requests.ts:289.
    pub async fn update_copy(self: &Rc<Self>, entry: &Entry) -> AccountResult<GitUpdateReply> {
        todo!()
    }
}
