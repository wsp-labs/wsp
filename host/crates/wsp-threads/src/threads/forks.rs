// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/forks.ts` at 65f6b89c7: a fork read off the thread it forks, every refusal
//! said before anything is written. Free functions over the context, as the TS's are. Spec: `spec/forks.md`.
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use wsp_adapters::port::{CountedTurn, ForkTurn, ForkedSession};
use wsp_core::context::{Joined, StateCell};
use wsp_core::types::api::{ForkOptions, StartOptions};
use wsp_core::types::harness::HarnessAdapter;
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::views::session::{ForkSource, ForkedFrom, SessionView};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use super::threads::{Entry, HeldProject, Sessions, ThreadRecords};

/// Every `ctx.<x>` the fork functions call (README R2).
pub trait ForksCtx {
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn store(&self) -> Rc<dyn Store>;
    fn sessions(&self) -> StateCell<Sessions>;
    fn thread_records(&self) -> StateCell<ThreadRecords>;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView>;
    /// The running row's turn (`ctx.runningOn(thread)?.turnId`).
    fn running_turn(&self, thread_id: &str) -> Option<String>;
    /// The launching row's turn (`ctx.launchingOn(thread)?.turnId`).
    fn launching_turn(&self, thread_id: &str) -> Option<String>;
    fn entry_of_row(&self, thread_id: &str, workspace_id: &str, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>;
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String>;
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String>;
    fn agent_label(&self, agent: &str) -> String;
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined>;
    fn keep_sent_images(&self, workspace_id: &str, thread_id: &str, request_id: Option<String>, attachments: Vec<Attachment>) -> LocalBoxFuture<'static, ()>;
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()>;
    fn project_held(&self, project: &str) -> HeldProject;
    fn place_name(&self, computer: &str) -> String;
    fn git_here(&self, top: &str, args: Vec<String>) -> LocalBoxFuture<'static, Result<ExecResult, HostError>>;
    /// `new Set(ctx.live.keys())`.
    fn live_ids(&self) -> Vec<String>;
    /// `ctx.worktreeFolder(project, top, branch, from, root, ref)`.
    fn worktree_folder(&self, project: &HeldProject, top: &str, branch: &str, from: &str, root: Option<String>, at_ref: &str) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    fn take_setup_line(&self, workspace_id: &str) -> Option<String>;
    fn remove_worktree(&self, entry: &Entry, force: bool) -> LocalBoxFuture<'static, Result<(), HostError>>;
}

/// forks.ts:27 `ForkPlan.source`.
#[derive(Clone)]
pub struct ForkPlanSource {
    pub thread_id: String,
    pub workspace_id: String,
    pub entry: Entry,
    pub harness: String,
    pub title: String,
    pub session: Option<String>,
    pub cwd: Option<String>,
}

/// forks.ts:30 `ForkPlan.turn`.
#[derive(Clone, Debug, PartialEq)]
pub struct ForkPlanTurn {
    pub turn_id: String,
    pub anchor: Option<String>,
    pub at_ref: Option<String>,
    pub after: Vec<CountedTurn>,
}

/// forks.ts:34 `ForkPlan.picks`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForkPicks {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
    pub permission_mode: Option<String>,
}

/// forks.ts:26 `ForkPlan`: F5.
#[derive(Clone)]
pub struct ForkPlan {
    pub source: ForkPlanSource,
    pub turn: Option<ForkPlanTurn>,
    pub events: Vec<SessionEvent>,
    pub picks: ForkPicks,
    pub forked_from: ForkedFrom,
}

/// `{ anchor } | { after }` (forks.ts:143).
#[derive(Clone, Debug, PartialEq)]
pub enum ForkCut {
    Anchor(String),
    After(Vec<CountedTurn>),
}

/// The start a fork runs (`(workspaceId, o) => Promise<SessionHandle>`, forks.ts:199).
pub type StartFn = Rc<dyn Fn(String, StartOptions) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>>;

/// forks.ts:42: F1.
pub async fn fork_opened(ctx: &Rc<dyn ForksCtx>, workspace_id: &str, opened: StartOptions, origin: Option<Caller>) -> Result<(StartOptions, Option<ForkPlan>), HostError> {
    todo!()
}

/// forks.ts:67: F2 to F5.
pub async fn read_fork(ctx: &Rc<dyn ForksCtx>, fork: &ForkSource, origin: Option<Caller>) -> Result<ForkPlan, HostError> {
    todo!()
}

/// forks.ts:143: F6.
pub fn fork_turn_of(plan: &ForkPlan, by_count: bool) -> Result<Option<ForkCut>, HostError> {
    todo!()
}

/// forks.ts:153: F7.
pub fn copied_events(plan: &ForkPlan, workspace_id: &str, thread_id: &str) -> Vec<SessionEvent> {
    todo!()
}

/// forks.ts:162: F8.
pub async fn copy_images(ctx: &Rc<dyn ForksCtx>, plan: &ForkPlan, workspace_id: &str, thread_id: &str) -> Result<(), HostError> {
    todo!()
}

/// forks.ts:181: F9.
pub async fn ready_fork(adapter: Arc<dyn HarnessAdapter>, session: &str, turn: ForkTurn, said: &dyn Fn(&str) -> HostError) -> Result<ForkedSession, HostError> {
    todo!()
}

/// forks.ts:191: F10.
pub fn move_session(ctx: &Rc<dyn ForksCtx>, thread_id: &str, from: &str, to: &str) {
    todo!()
}

/// forks.ts:199: F11.
pub async fn fork_start(ctx: &Rc<dyn ForksCtx>, start: StartFn, o: ForkOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
    todo!()
}

/// forks.ts:231: F12. The session the resume runs on.
pub async fn pay_cut_owed(ctx: &Rc<dyn ForksCtx>, adapter: Arc<dyn HarnessAdapter>, workspace_id: &str, thread_id: &str, resume: &str) -> String {
    todo!()
}
