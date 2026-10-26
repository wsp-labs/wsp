// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/pull-requests.ts at 65f6b89c7: the git host read for a workspace and the pull
//! request kept on it. Holds runtime invariant 4. Spec: spec/pull_requests.md.
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use wsp_core::types::events::{CHECKOUT_TTL_MS, PR_PAGE_HOLD_MS, RATE_LIMIT_HOLD_MS, SLATE_PR_POLL_MS};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::check::parse;
use wsp_wire::types::pull_request::{
    GitPrReadReply, GitPrViewReply, GitRepoReadReply, GitUpdateReply, PullRequest, PullRequestSeen, PR_POLL_IDLE_MS, PR_POLL_MS,
};
use wsp_wire::types::tree::TreeRecord;
use wsp_wire::views::session::{thread_key_of, SessionStatus, SessionView};
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_words::exit::HostError;
use wsp_words::projects::runs_in_folder;
use wsp_words::pull_request::{
    is_pull_request_fact, no_pull_request_refusal, pull_request_post_line, pull_request_stopped_line, pull_request_unread_line,
};
use wsp_words::workspace_state::is_local_workspace;

use crate::account::core_types::{AccountError, AccountResult, Ask, Entry, Frame, Work};
use wsp_core::clock::Cancel;

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
    async fn on_this_computer(
        &self,
        work: Box<dyn FnOnce(Ask, String) -> futures::future::LocalBoxFuture<'static, AccountResult<Value>>>,
    ) -> AccountResult<Value>;
    /// `ctx.withDaemon(entry, ask => ...)` (channels_daemon D14).
    async fn with_daemon(&self, entry: &Entry, work: Work<Value>) -> AccountResult<Value>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError>;
    async fn entry_of(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<Entry, HostError>;
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError>;
    /// `ctx.status.watched()`: a window is open.
    fn status_watched(&self) -> bool;
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()>;
    async fn status_now(&self, entry: &Entry) -> AccountResult<()>;
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
    async fn queued(
        &self,
        workspace_id: &str,
        work: futures::future::LocalBoxFuture<'static, AccountResult<Value>>,
    ) -> AccountResult<Value>;
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

/// The record of an entry, copied out so no borrow is held across an await.
fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

/// `PullRequestSeen` of the variant's read time.
fn read_at_of(seen: &PullRequestSeen) -> i64 {
    let value = serde_json::to_value(seen).unwrap_or(Value::Null);
    value.get("readAt").and_then(Value::as_f64).map(|v| v as i64).unwrap_or(0)
}

/// `{ ...pr, readAt }` as a `PullRequestSeen`.
fn seen_from(pr: &PullRequest, read_at: i64) -> AccountResult<PullRequestSeen> {
    let mut value = serde_json::to_value(pr).map_err(|e| AccountError::Host(HostError::new(e.to_string())))?;
    if let Value::Object(map) = &mut value {
        map.insert("readAt".to_string(), json!(read_at));
    }
    serde_json::from_value(value).map_err(|e| AccountError::Host(HostError::new(e.to_string())))
}

/// `{ why, readAt }` as a `PullRequestSeen`.
fn unread(why: String, read_at: i64) -> AccountResult<PullRequestSeen> {
    serde_json::from_value(json!({ "why": why, "readAt": read_at })).map_err(|e| AccountError::Host(HostError::new(e.to_string())))
}

fn pr_key(remote: &str, number: i64) -> String {
    format!("pr:{remote}#{number}")
}

fn page_key(remote: &str, number: i64) -> String {
    format!("page:{remote}#{number}")
}

fn is_pending(check: &wsp_wire::types::pull_request::PullRequestCheck) -> bool {
    serde_json::to_value(check.state).ok() == Some(json!("pending"))
}

fn ask_frame(ask: Ask, frame: Frame) -> LocalBoxFuture<'static, AccountResult<Value>> {
    Box::pin(async move { ask(frame).await.map(Value::Object) })
}

impl PullRequestsArea {
    pub fn new(ctx: Rc<dyn PullRequestsCtx>) -> Rc<PullRequestsArea> {
        Rc::new(PullRequestsArea { ctx, rate_limited: RefCell::default(), repo_settings: RefCell::default() })
    }

    /// Q1, pull-requests.ts:20: `parse` reads the reply into the caller's type.
    pub async fn read_host<T: 'static>(
        self: &Rc<Self>,
        entry: &Entry,
        frame: FrameFor,
        parse: Rc<dyn Fn(Frame) -> AccountResult<T>>,
    ) -> AccountResult<T> {
        Ok(self.read_host_on(entry, frame, parse).await?.read)
    }

    /// Q1, pull-requests.ts:25.
    pub async fn read_host_on<T: 'static>(
        self: &Rc<Self>,
        entry: &Entry,
        frame: FrameFor,
        parse: Rc<dyn Fn(Frame) -> AccountResult<T>>,
    ) -> AccountResult<ReadOn<T>> {
        let here_ctx = self.ctx.clone();
        let here_frame = frame.clone();
        let here = self
            .held_off(
                "here",
                Box::pin(async move {
                    here_ctx
                        .on_this_computer(Box::new(move |ask: Ask, home: String| -> LocalBoxFuture<'static, AccountResult<Value>> {
                            ask_frame(ask, here_frame(&home))
                        }))
                        .await
                }),
            )
            .await;
        match here {
            Ok(reply) => return Ok(ReadOn { read: parse(into_frame(reply))?, here: true }),
            Err(e) => {
                if !e.is_no_host_cli() || !copy_reads_host(&record_of(entry)) {
                    return Err(e);
                }
            }
        }
        let road = record_of(entry).id;
        let copy_ctx = self.ctx.clone();
        let copy_entry = entry.clone();
        let copy_frame = frame.clone();
        let copy = self
            .held_off(
                &road,
                Box::pin(async move {
                    let record = record_of(&copy_entry);
                    let work_ctx = copy_ctx.clone();
                    copy_ctx
                        .with_daemon(
                            &copy_entry,
                            Box::new(move |ask: Ask| -> LocalBoxFuture<'static, AccountResult<Value>> {
                                Box::pin(async move {
                                    let cwd = work_ctx.checkout_of(&record)?;
                                    ask_frame(ask, copy_frame(&cwd)).await
                                })
                            }),
                        )
                        .await
                }),
            )
            .await?;
        Ok(ReadOn { read: parse(into_frame(copy))?, here: false })
    }

    /// Q2, pull-requests.ts:38. `road` is `"here"` or the workspace id.
    pub async fn held_off<T: 'static>(self: &Rc<Self>, road: &str, read: LocalBoxFuture<'static, AccountResult<T>>) -> AccountResult<T> {
        let held = self.rate_limited.borrow().get(road).cloned();
        if let Some((until, refusal)) = held {
            if self.ctx.now() < until {
                return Err(refusal);
            }
        }
        match read.await {
            Ok(value) => Ok(value),
            Err(e) => {
                if matches!(&e, AccountError::Refusal(r) if r.code.as_deref() == Some("rate-limited")) {
                    let until = self.ctx.now() + RATE_LIMIT_HOLD_MS as i64;
                    self.rate_limited.borrow_mut().insert(road.to_string(), (until, e.clone()));
                }
                Err(e)
            }
        }
    }

    /// Q3, pull-requests.ts:50.
    pub async fn pull_request_on(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(Entry, String, i64), HostError> {
        let entry = self.ctx.entry_of(workspace_id, origin).await?;
        let record = record_of(&entry);
        let Some(number) = record.pr.as_ref().map(|pr| pr.number.get()) else {
            return Err(HostError::new(no_pull_request_refusal(&record.name)));
        };
        let remote = self.ctx.project_held(&record.project)?.remote;
        Ok((entry, remote, number))
    }

    /// Q3, pull-requests.ts:57.
    pub fn page_key(remote: &str, number: i64) -> String {
        page_key(remote, number)
    }

    /// Q3, pull-requests.ts:90.
    pub fn pr_key(remote: &str, number: i64) -> String {
        pr_key(remote, number)
    }

    /// Q4, pull-requests.ts:61.
    pub async fn read_page(
        self: &Rc<Self>,
        entry: &Entry,
        remote: &str,
        number: i64,
        fresh: bool,
    ) -> AccountResult<ReadOn<GitPrViewReply>> {
        let key = page_key(remote, number);
        if !fresh {
            if let Some(held) = self.ctx.cache_get(&key) {
                if self.ctx.now() - held.fetched_at < PR_PAGE_HOLD_MS as i64 {
                    let page =
                        parse::<GitPrViewReply>(&held.body["page"]).map_err(|e| AccountError::Host(HostError::new(e.issue_text())))?;
                    let here = held.body["here"].as_bool().unwrap_or(false);
                    return Ok(ReadOn { read: page, here });
                }
            }
        }
        let fetched_at = self.ctx.now();
        let remote_owned = remote.to_string();
        let read = self
            .read_host_on(
                entry,
                Rc::new(move |cwd: &str| {
                    let mut frame = Frame::new();
                    frame.insert("op".to_string(), json!("git.prView"));
                    frame.insert("cwd".to_string(), json!(cwd));
                    frame.insert("remote".to_string(), json!(remote_owned));
                    frame.insert("number".to_string(), json!(number));
                    frame
                }),
                Rc::new(|reply: Frame| {
                    parse::<GitPrViewReply>(&Value::Object(reply)).map_err(|e| AccountError::Host(HostError::new(e.issue_text())))
                }),
            )
            .await?;
        self.ctx.cache_set(&key, CacheEntry { body: json!({ "page": read.read, "here": read.here }), tag: None, fetched_at });
        let ctx = self.ctx.clone();
        let timer_key = key.clone();
        let _ = self.ctx.schedule(
            PR_PAGE_HOLD_MS as i64,
            Box::new(move || {
                if ctx.cache_get(&timer_key).is_some_and(|held| held.fetched_at == fetched_at) {
                    ctx.cache_delete(&timer_key);
                }
            }),
        );
        Ok(read)
    }

    /// Q5, pull-requests.ts:74. Invariant 4: this computer's command line alone.
    pub async fn post_as_person<T: 'static>(
        self: &Rc<Self>,
        remote: &str,
        number: i64,
        frame: FrameFor,
        parse: Rc<dyn Fn(Frame) -> AccountResult<T>>,
    ) -> AccountResult<T> {
        self.ctx.cache_delete(&page_key(remote, number));
        let reply = self
            .ctx
            .on_this_computer(Box::new(move |ask: Ask, home: String| -> LocalBoxFuture<'static, AccountResult<Value>> {
                ask_frame(ask, frame(&home))
            }))
            .await;
        match reply {
            Ok(reply) => parse(into_frame(reply)),
            Err(e) => {
                if e.is_no_host_cli() {
                    return Err(HostError::new(pull_request_post_line(&Self::host_of_remote(remote))).into());
                }
                Err(e)
            }
        }
    }

    /// Q5, pull-requests.ts:85.
    pub fn host_of_remote(remote: &str) -> String {
        wsp_catalog::git_hosts::git_host_of(remote)
            .and_then(|host| host.ssh_hosts.first().map(|ssh| ssh.to_string()))
            .or_else(|| wsp_catalog::git_hosts::remote_host(remote))
            .unwrap_or_else(|| remote.to_string())
    }

    /// Q6, pull-requests.ts:88.
    pub fn pr_poll_ms(&self) -> i64 {
        if self.ctx.status_watched() {
            PR_POLL_MS as i64
        } else {
            PR_POLL_IDLE_MS as i64
        }
    }

    /// Q7, pull-requests.ts:99.
    pub async fn read_pull_request(self: &Rc<Self>, entry: &Entry, force: bool, whole: bool) -> Option<PullRequestSeen> {
        match self.read_pr(entry, force, whole).await {
            Ok(seen) => seen,
            Err(e) => {
                self.ctx.warn(&e.message());
                None
            }
        }
    }

    async fn read_pr(self: &Rc<Self>, entry: &Entry, force: bool, whole: bool) -> AccountResult<Option<PullRequestSeen>> {
        if self.left_branch(entry) {
            self.drop_pull_request(entry).await?;
            return Box::pin(self.read_pr(entry, force, whole)).await;
        }
        let kept = record_of(entry).pr;
        if kept.as_ref().is_some_and(|kept| kept.state != wsp_wire::types::pull_request::PullRequestState::Open) {
            return Ok(entry.borrow().pr.clone());
        }
        let held = entry.borrow().pr.clone();
        let is_fact = is_pull_request_fact(held.as_ref());
        let hold = if is_fact { self.pr_poll_ms() } else { CHECKOUT_TTL_MS as i64 };
        if !force {
            if let Some(held_seen) = &held {
                let read_at = read_at_of(held_seen);
                if self.ctx.now() - read_at < hold {
                    let (armed, armed_ms) = {
                        let live = entry.borrow();
                        (live.pr_poll.is_some(), live.pr_poll_ms)
                    };
                    if is_fact && armed && armed_ms != Some(hold as u64) {
                        self.poll_pull_request(entry, Some(read_at));
                    }
                    return Ok(held);
                }
            }
        }
        if !force && kept.is_none() {
            let none_at = entry.borrow().pr_none_at.clone();
            let head = entry.borrow().checkout.as_ref().and_then(|checkout| checkout.head.clone());
            if none_at.is_some() && none_at == head {
                return Ok(held);
            }
        }
        let reading = entry.borrow().pr_reading.clone();
        if let (Some(reading), false) = (reading, whole) {
            return Ok(reading.await?);
        }
        let this = self.clone();
        let run_entry = entry.clone();
        let pending: LocalBoxFuture<'static, Result<Option<PullRequestSeen>, HostError>> = Box::pin(async move {
            let outcome = this.run_read(&run_entry, whole).await;
            run_entry.borrow_mut().pr_reading = None;
            outcome.map_err(|e| HostError::new(e.message()))
        });
        let shared = pending.shared();
        entry.borrow_mut().pr_reading = Some(shared.clone());
        Ok(shared.await?)
    }

    /// The reading itself, the body of pull-requests.ts:113-150.
    async fn run_read(self: &Rc<Self>, entry: &Entry, whole: bool) -> AccountResult<Option<PullRequestSeen>> {
        let record = record_of(entry);
        let kept = record.pr.clone();
        let project = self.ctx.project_held(&record.project)?;
        if project.remote.is_empty() {
            return Ok(entry.borrow().pr.clone());
        }
        let checkout: Option<Checkout> = if kept.is_none() {
            let held = entry.borrow().checkout.clone();
            match held {
                Some(checkout) => Some(checkout),
                None => self.ctx.read_checkout(entry, false).await?,
            }
        } else {
            None
        };
        let branch = checkout.as_ref().map(|checkout| checkout.branch.clone());
        let base = record.base.clone().or_else(|| project.base.clone());
        if kept.is_none() {
            let unnamed = match &branch {
                None => true,
                Some(name) => name.starts_with('(') || base.as_deref() == Some(name.as_str()),
            };
            if unnamed {
                return Ok(entry.borrow().pr.clone());
            }
        }
        let remote = project.remote.clone();
        let kept_number = kept.as_ref().map(|kept| kept.number.get());
        let cached = kept_number.and_then(|number| self.ctx.cache_get(&pr_key(&remote, number)));
        let body: Option<PullRequest> = cached.as_ref().and_then(|cached| parse::<PullRequest>(&cached.body).ok());
        let seen: Option<String> = match (&body, &cached) {
            (Some(body), Some(cached)) if !whole && cached.tag.is_some() && !body.checks.iter().any(is_pending) => cached.tag.clone(),
            _ => None,
        };
        let at = self.ctx.now();
        let frame_remote = remote.clone();
        let frame_branch = branch.clone();
        let frame_seen = seen.clone();
        let frame: FrameFor = Rc::new(move |cwd: &str| {
            let mut out = Frame::new();
            out.insert("op".to_string(), json!("git.prRead"));
            out.insert("cwd".to_string(), json!(cwd));
            out.insert("remote".to_string(), json!(frame_remote));
            match kept_number {
                Some(number) => {
                    out.insert("number".to_string(), json!(number));
                }
                None => {
                    out.insert("branch".to_string(), json!(frame_branch));
                }
            }
            if let Some(seen) = &frame_seen {
                out.insert("seen".to_string(), json!(seen));
            }
            out
        });
        let outcome: AccountResult<()> = self.try_read(entry, frame, kept.is_none(), body, checkout, &remote, at).await;
        match outcome {
            Ok(()) => {}
            Err(e) if e.is_no_host_cli() => {
                let host = Self::host_of_remote(&remote);
                let stopped =
                    !is_local_workspace(Some(&record.kind)) && record.phase != wsp_wire::views::workspace::WorkspacePhase::Running;
                let why = if stopped { pull_request_stopped_line(&host, &record.name) } else { pull_request_unread_line(&host) };
                self.take_pull_request(entry, Some(unread(why, at)?)).await?;
            }
            Err(_) => self.poll_pull_request(entry, None),
        }
        Ok(entry.borrow().pr.clone())
    }

    /// The read itself and what it takes (pull-requests.ts:123-143): `unnamed` is a read by branch, with no kept number.
    #[allow(clippy::too_many_arguments)]
    async fn try_read(
        self: &Rc<Self>,
        entry: &Entry,
        frame: FrameFor,
        unnamed: bool,
        body: Option<PullRequest>,
        checkout: Option<Checkout>,
        remote: &str,
        at: i64,
    ) -> AccountResult<()> {
        let read: GitPrReadReply = self
            .read_host(
                entry,
                frame,
                Rc::new(|reply: Frame| {
                    parse::<GitPrReadReply>(&Value::Object(reply)).map_err(|e| AccountError::Host(HostError::new(e.issue_text())))
                }),
            )
            .await?;
        if read.unchanged == Some(true) {
            if let Some(body) = &body {
                self.take_pull_request(entry, Some(seen_from(body, at)?)).await?;
                return Ok(());
            }
        }
        if let Some(pr) = &read.pr {
            if let Some(tag) = &read.seen {
                let body_value = serde_json::to_value(pr).map_err(|e| AccountError::Host(HostError::new(e.to_string())))?;
                self.ctx
                    .cache_set(&pr_key(remote, pr.number.get()), CacheEntry { body: body_value, tag: Some(tag.clone()), fetched_at: at });
            }
        }
        if read.pr.is_none() && unnamed {
            entry.borrow_mut().pr_none_at = checkout.as_ref().and_then(|checkout| checkout.head.clone());
        } else {
            entry.borrow_mut().pr_none_at = None;
        }
        let seen = match &read.pr {
            Some(pr) => Some(seen_from(pr, at)?),
            None => None,
        };
        self.take_pull_request(entry, seen).await
    }

    /// Q9, pull-requests.ts:158.
    pub fn left_branch(&self, entry: &Entry) -> bool {
        let record = record_of(entry);
        let kept = record.pr.clone();
        let now = entry.borrow().checkout.as_ref().map(|checkout| checkout.branch.clone());
        if !shared_folder(&record) {
            return false;
        }
        let (Some(kept), Some(now)) = (kept, now) else {
            return false;
        };
        if now.starts_with('(') {
            return false;
        }
        match &kept.branch {
            Some(branch) => now != *branch,
            None => match self.base_of(&record) {
                Ok(base) => now == base,
                Err(_) => false,
            },
        }
    }

    /// `record.base ?? project.base ?? project.defaultBranch` (pull-requests.ts:166).
    fn base_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        let project = self.ctx.project_held(&record.project)?;
        Ok(record.base.clone().or(project.base).unwrap_or(project.default_branch))
    }

    /// Q10, pull-requests.ts:171.
    pub async fn drop_pull_request(self: &Rc<Self>, entry: &Entry) -> AccountResult<()> {
        let cancel = entry.borrow_mut().pr_poll.take();
        if let Some(cancel) = cancel {
            cancel();
        }
        let record_cell = {
            let mut live = entry.borrow_mut();
            live.pr = None;
            live.pr_none_at = None;
            live.record.clone()
        };
        let record = {
            let mut written = record_cell.borrow_mut();
            written.pr = None;
            written.pr_sent = None;
            written.clone()
        };
        self.ctx.persist(&record).await?;
        self.ctx.status_now(entry).await?;
        Ok(())
    }

    /// Q11, pull-requests.ts:185.
    pub async fn ran_on_branch(&self, entry: &Entry, thread_id: &str) -> AccountResult<()> {
        let record = record_of(entry);
        let branch = entry.borrow().checkout.as_ref().map(|checkout| checkout.branch.clone());
        if !shared_folder(&record) {
            return Ok(());
        }
        let Some(branch) = branch else {
            return Ok(());
        };
        if branch.starts_with('(') {
            return Ok(());
        }
        let on = record.branch_threads.clone();
        let base = self.base_of(&record)?;
        if branch == base {
            if on.is_none() {
                return Ok(());
            }
            entry.borrow().record.borrow_mut().branch_threads = None;
            let saved = record_of(entry);
            return self.ctx.persist(&saved).await;
        }
        if on.as_ref().is_some_and(|on| on.branch == branch && on.threads.iter().any(|t| t == thread_id)) {
            return Ok(());
        }
        let threads = match &on {
            Some(on) if on.branch == branch => {
                let mut threads = on.threads.clone();
                threads.push(thread_id.to_string());
                threads
            }
            _ => vec![thread_id.to_string()],
        };
        entry.borrow().record.borrow_mut().branch_threads = Some(wsp_core::types::wiring::BranchThreads { branch, threads });
        let saved = record_of(entry);
        self.ctx.persist(&saved).await
    }

    /// Q8, pull-requests.ts:203.
    pub async fn take_pull_request(self: &Rc<Self>, entry: &Entry, seen: Option<PullRequestSeen>) -> AccountResult<()> {
        entry.borrow_mut().pr = seen.clone();
        if let Some(PullRequestSeen::PullRequestFact(fact)) = &seen {
            let was = record_of(entry).pr;
            let moved = match &was {
                None => true,
                Some(was) => {
                    was.state != fact.state || was.number.get() != fact.number.get() || was.branch.as_deref() != Some(fact.branch.as_str())
                }
            };
            if moved {
                let read_at = fact.read_at.0;
                let open = fact.state == wsp_wire::types::pull_request::PullRequestState::Open;
                let (merged_at, closed_at) = match fact.state {
                    wsp_wire::types::pull_request::PullRequestState::Merged => (Some(wsp_wire::types::js_number::JsNumber(read_at)), None),
                    wsp_wire::types::pull_request::PullRequestState::Closed => (None, Some(wsp_wire::types::js_number::JsNumber(read_at))),
                    _ => (None, None),
                };
                let checks = if open { None } else { Some(fact.checks.clone()) };
                let record_pr = wsp_wire::types::pull_request::PullRequestRecord {
                    number: fact.number,
                    url: fact.url.clone(),
                    state: fact.state,
                    base: fact.base.clone(),
                    branch: Some(fact.branch.clone()),
                    merged_at,
                    closed_at,
                    checks,
                };
                let record_cell = entry.borrow().record.clone();
                let saved = {
                    let mut record = record_cell.borrow_mut();
                    if was.as_ref().is_some_and(|was| was.number.get() != fact.number.get()) {
                        record.pr_sent = None;
                    }
                    record.pr = Some(record_pr);
                    record.clone()
                };
                self.ctx.persist(&saved).await?;
            }
        }
        self.ctx.status_now(entry).await?;
        self.poll_pull_request(entry, None);
        let settled = record_of(entry).pr.is_some_and(|pr| pr.state != wsp_wire::types::pull_request::PullRequestState::Open);
        if settled {
            self.settle_tree(entry).await;
        }
        Ok(())
    }

    /// Q12, pull-requests.ts:224. `from` defaults to now at the call sites that name none.
    pub fn poll_pull_request(self: &Rc<Self>, entry: &Entry, from: Option<i64>) {
        let cancel = entry.borrow_mut().pr_poll.take();
        if let Some(cancel) = cancel {
            cancel();
        }
        let fact = entry.borrow().pr.clone();
        let Some(PullRequestSeen::PullRequestFact(fact)) = fact else {
            return;
        };
        let id = record_of(entry).id;
        let is_live = self.ctx.live(&id).is_some_and(|live| Rc::ptr_eq(&live, entry));
        if fact.state != wsp_wire::types::pull_request::PullRequestState::Open || !is_live {
            return;
        }
        let pending = fact.checks.iter().any(is_pending) && self.ctx.slates_watch_pr(&id);
        let ms = if pending { SLATE_PR_POLL_MS as i64 } else { self.pr_poll_ms() };
        let from = from.unwrap_or_else(|| self.ctx.now());
        entry.borrow_mut().pr_poll_ms = Some(ms as u64);
        let delay = (from + ms - self.ctx.now()).max(0);
        let weak = Rc::downgrade(self);
        let weak_entry = Rc::downgrade(entry);
        let cancel = self.ctx.schedule(
            delay,
            Box::new(move || {
                let (Some(area), Some(entry)) = (weak.upgrade(), weak_entry.upgrade()) else {
                    return;
                };
                entry.borrow_mut().pr_poll = None;
                let ctx = area.ctx.clone();
                ctx.spawn(Box::pin(async move {
                    let _ = area.read_pull_request(&entry, true, false).await;
                }));
            }),
        );
        entry.borrow_mut().pr_poll = Some(Box::new(move || cancel()));
    }

    /// Q16, pull-requests.ts:244.
    pub async fn keep_tree(&self, child: &Entry, kept: TreeRecord) -> AccountResult<()> {
        let empty = serde_json::to_value(&kept).map(|v| v.as_object().is_some_and(|o| o.is_empty())).unwrap_or(false);
        let record_cell = child.borrow().record.clone();
        let saved = {
            let mut record = record_cell.borrow_mut();
            record.tree = if empty { None } else { Some(kept) };
            record.clone()
        };
        self.ctx.persist(&saved).await?;
        self.ctx.read_lead_of(child);
        Ok(())
    }

    /// Q13, pull-requests.ts:256.
    pub async fn settle_tree(self: &Rc<Self>, entry: &Entry) {
        let record = record_of(entry);
        if record.parent_thread_id.is_some() {
            return;
        }
        let rows = self.ctx.session_views();
        let pr_branch = record.pr.as_ref().and_then(|pr| pr.branch.clone());
        let ran: BTreeSet<String> = match &record.branch_threads {
            Some(on) if Some(&on.branch) == pr_branch.as_ref() => on.threads.iter().cloned().collect(),
            _ => BTreeSet::new(),
        };
        let folder = shared_folder(&record);
        let ran_here = |view: &SessionView| !folder || ran.contains(&thread_key_of(view));
        let roots: BTreeSet<String> = rows
            .iter()
            .filter(|view| view.workspace_id == record.id && view.root_thread_id.is_none() && ran_here(view))
            .map(thread_key_of)
            .collect();
        if roots.is_empty() {
            return;
        }
        let tree: Vec<&SessionView> = rows
            .iter()
            .filter(|view| roots.contains(&thread_key_of(view)) || view.root_thread_id.as_ref().is_some_and(|root| roots.contains(root)))
            .collect();
        if tree.iter().any(|view| view.status == SessionStatus::Running) {
            return;
        }
        let at = self.ctx.now();
        let keys: Vec<String> = tree.iter().map(|view| thread_key_of(view)).collect::<IndexSet<String>>().into_iter().collect();
        if let Err(e) = self.ctx.mark_settled(keys, at).await {
            self.ctx.warn(&format!("the tree of {} was not settled: {}", record.name, e.message()));
        }
        if self.ctx.pauses(&record) && record.phase == wsp_wire::views::workspace::WorkspacePhase::Running {
            let ctx = self.ctx.clone();
            let id = record.id.clone();
            let name = record.name.clone();
            self.ctx.spawn(Box::pin(async move {
                if let Err(e) = ctx.nap_with(&id).await {
                    ctx.warn(&format!("{name} was not napped once its pull request settled: {}", e.message()));
                }
            }));
        }
    }

    /// Q14, pull-requests.ts:279.
    pub async fn merge_settings(self: &Rc<Self>, remote: &str) -> AccountResult<GitRepoReadReply> {
        let cached = self.repo_settings.borrow().get(remote).cloned();
        let now = self.ctx.now();
        if let Some((at, read)) = cached {
            if now - at < REPO_SETTINGS_MS {
                return Ok(read);
            }
        }
        let ctx = self.ctx.clone();
        let remote_owned = remote.to_string();
        let reply = self
            .held_off(
                "here",
                Box::pin(async move {
                    ctx.on_this_computer(Box::new(move |ask: Ask, home: String| -> LocalBoxFuture<'static, AccountResult<Value>> {
                        let mut frame = Frame::new();
                        frame.insert("op".to_string(), json!("git.repoRead"));
                        frame.insert("cwd".to_string(), json!(home));
                        frame.insert("remote".to_string(), json!(remote_owned));
                        ask_frame(ask, frame)
                    }))
                    .await
                }),
            )
            .await?;
        let read = parse::<GitRepoReadReply>(&reply).map_err(|e| AccountError::Host(HostError::new(e.issue_text())))?;
        self.repo_settings.borrow_mut().insert(remote.to_string(), (self.ctx.now(), read.clone()));
        Ok(read)
    }

    /// Q15, pull-requests.ts:289.
    pub async fn update_copy(self: &Rc<Self>, entry: &Entry) -> AccountResult<GitUpdateReply> {
        let record = record_of(entry);
        let base = match record.base.clone() {
            Some(base) => Some(base),
            None => self.ctx.project_held(&record.project)?.base,
        };
        let ctx = self.ctx.clone();
        let work_entry = entry.clone();
        let work: LocalBoxFuture<'static, AccountResult<Value>> = Box::pin(async move {
            let checkout_ctx = ctx.clone();
            let road_entry = work_entry.clone();
            ctx.with_daemon(
                &work_entry,
                Box::new(move |ask: Ask| -> LocalBoxFuture<'static, AccountResult<Value>> {
                    Box::pin(async move {
                        let record = record_of(&road_entry);
                        let mut frame = Frame::new();
                        frame.insert("op".to_string(), json!("git.update"));
                        frame.insert("cwd".to_string(), json!(checkout_ctx.checkout_of(&record)?));
                        if let Some(base) = &base {
                            frame.insert("base".to_string(), json!(base));
                        }
                        ask_frame(ask, frame).await
                    })
                }),
            )
            .await
        });
        let reply = self.ctx.queued(&record.id, work).await?;
        let done = parse::<GitUpdateReply>(&reply).map_err(|e| AccountError::Host(HostError::new(e.issue_text())))?;
        self.ctx.read_checkout(entry, true).await?;
        let area = self.clone();
        let read_entry = entry.clone();
        self.ctx.spawn(Box::pin(async move {
            let _ = area.read_pull_request(&read_entry, true, false).await;
        }));
        Ok(done)
    }
}

/// Whether a copy reads the git host itself where this computer cannot: a copy on a machine of its own, running.
fn copy_reads_host(record: &WorkspaceRecord) -> bool {
    !is_local_workspace(Some(&record.kind)) && record.phase == wsp_wire::views::workspace::WorkspacePhase::Running
}

/// A folder project's shared checkout: runs in a folder, no worktree (pull-requests.ts:157).
fn shared_folder(record: &WorkspaceRecord) -> bool {
    runs_in_folder(&record.kind) && record.worktree.is_none()
}

/// A reply the context hands back as a JSON object, as a frame.
fn into_frame(reply: Value) -> Frame {
    match reply {
        Value::Object(map) => map,
        _ => Frame::new(),
    }
}
