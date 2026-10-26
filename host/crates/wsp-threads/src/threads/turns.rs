// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/turns.ts at 65f6b89c7, in three units: `run` (the turn's state, row, handle and
//! ends), `forward` (the adapter's events) and `reopen` (the sweep and the re-open after a restart).
//! Specs: spec/turns_run.md, spec/turns_forward.md, spec/turns_reopen.md.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_adapters::port::{HarnessExec, HarnessSession, OnEvent, PermissionAsk};
use wsp_core::clock::Clock;
use wsp_core::context::Joined;
use wsp_core::types::internal::{KeptLaunch, TurnWritten};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::attachments::AttachmentRecord;
use wsp_wire::types::usage::{AccountsAnswer, HarnessLimit};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::{TurnResult, TurnTokens};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::HostError;

use crate::threads::agents::launch::{AdapterAsk, BuiltAdapter};
use crate::threads::threads::{CheckpointAsk, Entry, KeptHand, Row, Sessions, ThreadRecords, TurnChangesAsk};

pub mod forward;
pub mod reopen;
pub mod run;

/// Launches the harness session, handing it the turn's forward; may call it before it returns (TS `t.open`).
pub type Open = Box<dyn FnOnce(OnEvent) -> Result<Arc<dyn HarnessSession>, HostError>>;

/// What the turn's own `session.start` row carries (turns.ts:42).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Opening {
    pub prompt: String,
    pub request_id: Option<String>,
    /// TS `via: "slate"`.
    pub via_slate: bool,
    pub after_cut: bool,
    pub after_limit: Option<f64>,
    pub opens_thread: bool,
    pub title: Option<String>,
    pub attachments: Option<Vec<AttachmentRecord>>,
}

/// The snapshot the turn's changes are read from: a commit known, or one still being taken (turns.ts:56).
#[derive(Clone)]
pub enum SnapshotFrom {
    Known(Option<String>),
    Taking(futures::future::Shared<LocalBoxFuture<'static, Option<String>>>),
}

#[derive(Clone)]
pub struct TurnSnapshot {
    pub from: SnapshotFrom,
    pub cwd: String,
}

/// Everything `run_turn` is handed (turns.ts:19-67).
pub struct TurnStart {
    pub entry: Entry,
    /// The turn's row, built by the caller around the view it keeps writing; `run` inserts it into `sessions`.
    pub row: Row,
    pub written: Option<TurnWritten>,
    pub thread_id: String,
    pub turn_id: String,
    pub notify: Option<Vec<String>>,
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub turn_token: Option<String>,
    pub scope_device_id: Option<String>,
    /// Never `held`.
    pub outcome: SessionStartOutcome,
    pub opening: Opening,
    pub resume: Option<String>,
    pub images_dir: Option<String>,
    pub snapshot: Option<TurnSnapshot>,
    pub reports_edits: bool,
    pub waiting: Option<crate::threads::threads::Waiting>,
    pub keep: Option<KeptLaunch>,
    pub open: Open,
}

/// The account a turn's use is filed under (TS `ctx.usageAccountOf(...)`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsageAccount {
    pub key: String,
    pub label: Option<String>,
    pub road: Option<String>,
}

/// One ledger row a turn files (turns.ts:418-431), in the TS literal's key order.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerUse {
    pub at: i64,
    pub agent: String,
    pub account: String,
    pub account_label: Option<String>,
    pub computer: String,
    pub project: String,
    pub turns: u32,
    pub model: Option<String>,
    /// As the turn's result carried them: a split the agent left out stays out (`Partial<TurnTokens>`).
    pub tokens: Option<TurnTokens>,
    pub cost_usd: Option<f64>,
    pub session: Option<String>,
    /// Always `"wsp"`.
    pub source: String,
}

/// `ctx.ledger.limit(...)`'s ask (turns.ts:475).
#[derive(Clone, Debug, PartialEq)]
pub struct LimitAsk {
    pub key: String,
    pub agent: String,
    pub label: Option<String>,
    pub road: Option<String>,
    pub computer: String,
    pub limit: HarnessLimit,
}

/// `ctx.burn.add(...)`'s ask (turns.ts:487).
#[derive(Clone, Debug, PartialEq)]
pub struct BurnAsk {
    pub account: String,
    pub thread_id: String,
    pub tokens: f64,
    /// `{ run: turnId, at }` on a re-opened turn whose event carries `at`.
    pub replayed: Option<(String, f64)>,
}

/// What the runtime's close runs for each turn still open (TS `ctx.heldAtClose`).
pub type HeldAtClose = Rc<RefCell<Vec<Rc<dyn Fn()>>>>;

/// The sweep of an exec factory (TS `ctx.execFactoryFor(entry).sweep`): ends every run but those held, answers those
/// it ended.
pub type Sweep = Box<dyn FnOnce(Vec<String>) -> LocalBoxFuture<'static, Result<Vec<String>, HostError>>>;

/// What the turns area reads of the runtime.
#[async_trait(?Send)]
pub trait TurnsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn sessions(&self) -> Sessions;
    fn records(&self) -> ThreadRecords;
    fn live_entry(&self, workspace_id: &str) -> Option<Entry>;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn closing(&self) -> bool;
    /// `ctx.record(event)`: the transcript row, and the bus.
    fn record(&self, event: SessionEvent);
    /// `bus.emit(event)` for the events no transcript holds (`thread.marked`, `usage.account`).
    fn emit(&self, event: EventUnion);
    /// `bus.pass(event)`: delivered, never ringed.
    fn pass(&self, event: EventUnion);
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()>;
    async fn persist_record(&self, record: WorkspaceRecord) -> Result<(), HostError>;
    fn held_at_close(&self) -> HeldAtClose;
    async fn slates_turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError>;
    fn keep_checkpoint(&self, entry: &Entry, ask: CheckpointAsk) -> Joined;
    fn checkpoints_landing(&self) -> Rc<RefCell<IndexMap<String, Joined>>>;
    async fn read_turn_changes(&self, entry: &Entry, ask: TurnChangesAsk) -> Result<bool, HostError>;
    fn lead_asks(&self) -> Rc<RefCell<IndexMap<String, PermissionAsk>>>;
    /// `harnessCatalog(harness)?.permissionModes` label of a mode.
    fn mode_label(&self, harness: &str, mode: &str) -> Option<String>;
    /// `ctx.settleCut(row, reason, () => reason, byStop, unreached)`.
    fn settle_cut(&self, row: &Row, reason: &str, by_stop: bool, unreached: bool);
    /// `ctx.lineTry({ view, turnId, asked? })`.
    fn line_try(&self, row: &Row) -> bool;
    fn prompt_held(&self, request_id: &str);
    async fn name_in_harness(&self, view: SessionView, title: &str) -> Result<(), HostError>;
    /// `ctx.sourceOf(view)`: where the row's title came from.
    fn source_of(&self, view: &SessionView) -> Option<String>;
    fn hold_ask(&self, ask: LocalBoxFuture<'static, ()>);
    async fn refresh_title(&self, row: &Row, ended: bool) -> Result<(), HostError>;
    async fn make_title(&self, row: &Row) -> Result<(), HostError>;
    fn usage_account_of(&self, entry: &Entry, harness: &str, account: Option<(String, Option<String>)>) -> UsageAccount;
    fn usage_computer_of(&self, record: &WorkspaceRecord) -> String;
    async fn ledger_add(&self, row: LedgerUse) -> Result<(), HostError>;
    /// Answers the account's readings before and after.
    async fn ledger_limit(&self, ask: LimitAsk) -> Result<(serde_json::Value, serde_json::Value), HostError>;
    async fn alerts_read(&self, before: serde_json::Value, after: serde_json::Value) -> Result<(), HostError>;
    /// `(await ctx.usageAccounts()).accounts`, each with its key.
    async fn usage_accounts(&self) -> Result<AccountsAnswer, HostError>;
    fn burn_add(&self, ask: BurnAsk);
    /// `ctx.notifyEnd(row, notify, ctx.tellAs(row), result, { open })`.
    fn notify_end(&self, row: &Row, notify: &[String], result: TurnResult, open: bool);
    /// waits' `agent_retried`.
    fn agent_retried(&self, turn_id: &str, why: Option<&str>);
    fn end_snooze_for(&self, view: &SessionView);
    fn idle_hold(&self, workspace_id: &str);
    fn idle_release(&self, workspace_id: &str);
    fn arm_disk_watch(&self);
    async fn read_disk(&self, entry: &Entry) -> Result<(), HostError>;
    /// `harnessExec(entry.machine, ms)`: the machine's command road for a harness read bounded by `ms`.
    fn harness_exec(&self, entry: &Entry, timeout_ms: u64) -> Arc<dyn HarnessExec>;
    /// `ctx.moduleOf(entry.record.kind).sharedDaemon`.
    fn shared_daemon(&self, entry: &Entry) -> bool;
    fn turn_groups(&self) -> Rc<RefCell<IndexMap<String, IndexMap<i64, String>>>>;
    fn arm_roots_recheck(&self);
    fn port_roots_moved(&self, workspace_id: &str);
    /// Keeps the agent for the thread's next turn; a hold the runtime refuses is handed back, so the turn closes its
    /// agent (TS `ctx.holdKept(...)` answering false).
    fn hold_kept(&self, thread_id: &str, hold: KeptHand) -> Option<KeptHand>;
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError>;
    async fn read_checkout(&self, entry: &Entry, fresh: bool) -> Result<(), HostError>;
    async fn ran_on_branch(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError>;
    async fn read_pull_request(&self, entry: &Entry, fresh: bool) -> Result<(), HostError>;
    async fn read_lead_of(&self, entry: &Entry) -> Result<(), HostError>;
    /// The workspace's pull request state where it has one (`entry.record.pr?.state`).
    fn pull_request_state(&self, entry: &Entry) -> Option<String>;
    async fn settle_tree(&self, entry: &Entry) -> Result<(), HostError>;
    fn drop_images(&self, entry: &Entry, dir: &str);
    fn send_back(&self, row: &Row, unread: Vec<String>, stopped: bool);
    async fn take_review(&self, entry: &Entry, thread_id: &str, text: &str) -> Result<(), HostError>;
    // reopen
    fn is_held_away(&self, workspace_id: &str) -> bool;
    /// `ctx.execFactoryFor(entry).sweep`, None where the factory has none.
    fn exec_sweep(&self, entry: &Entry) -> Option<Sweep>;
    fn adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<BuiltAdapter, HostError>;
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>;
    /// TS `void promise`: work that outlives its caller. The host runs it on its `LocalSet`, so where it runs is its to say.
    fn spawn(&self, work: LocalBoxFuture<'static, ()>);
}

/// turns.ts:17. Clones share one context.
#[derive(Clone)]
pub struct TurnsArea {
    pub(crate) ctx: Rc<dyn TurnsCtx>,
}

impl TurnsArea {
    pub fn new(ctx: Rc<dyn TurnsCtx>) -> Self {
        TurnsArea { ctx }
    }
}

pub use run::{TurnHandle, TurnState};
