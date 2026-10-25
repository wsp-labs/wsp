// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/sessions.ts at 65f6b89c7, in eight units: `stop`, `start` (the checks before the
//! row), `start_launch` (the row to the handover), `reads`, `acts`, `settle`, `rewind` and `delete`.
//! Specs: spec/sessions_*.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use wsp_adapters::port::{AsideQuestion, HarnessAdapter, KeptAgent, TurnImage};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::api::{
    AnswerAsk, DeleteThreadAsk, ForkOptions, HistoryAsk, PageWindow, RewindAsk, SessionsApi, SettleAsk, StartOptions, SteerAsk,
    ThreadDeleted, WarmAsk,
};
use wsp_core::types::internal::{EndThread, KeptLaunch, SearchedHit, Taken, ThreadStamps, TreeTalk};
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_wire::types::attachments::KeptAttachment;
use wsp_wire::views::activity::ProcessesAsk;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::{HistoryPage, Preferences, ThreadFacts, ThreadHead};
use wsp_wire::views::session::{ForkedFrom, SessionView, ThreadCapWait, ThreadMarks};
use wsp_wire::views::session_events_more::{RunStep, SessionEvent, SessionRunEvent};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::runtime::{
    SessionAccessResult, SessionAnswerResult, SessionAsideResult, SessionInterruptResult, SessionRenameResult, SessionRestoreResult,
    SessionRewindResult, SessionSearchResult, SessionSettleResult, SessionSteerResult, SessionWarmResult,
};
use wsp_words::exit::HostError;

use crate::activity::ActivityDoor;
use crate::pending::{Entry, Joined, Records, Rows, SessionEntry, SharedView};
use crate::threads::launch::{AdapterAsk, BuiltAdapter, LaunchArea, LaunchCtx, LaunchServers, ThreadLaunch};
use crate::threads::trimmed_row::TrimmedCtx;
use crate::threads::turns::TurnStart;
use crate::threads::waits::WaitsArea;

pub mod acts;
pub mod delete;
pub mod reads;
pub mod rewind;
pub mod settle;
pub mod start;
pub mod start_launch;
pub mod stop;

/// A start as forks.ts's `forkStart` calls it back.
pub type StartFn = Rc<dyn Fn(String, StartOptions) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>>;

/// A fork's plan, as forks.ts's `forkOpened` hands it (the other slice's file; sessions reads these fields).
#[derive(Clone, Debug, PartialEq)]
pub struct ForkPlan {
    /// Whether the fork names a turn to cut at (`plan.turn !== undefined`).
    pub has_turn: bool,
    pub source_thread_id: String,
    pub source_session: Option<String>,
    pub forked_from: ForkedFrom,
    /// The plan as forks.ts holds it, handed back to its functions whole.
    pub raw: Value,
}

/// What `readyFork` readied (forks.ts): a copy the turn resumes, or the fork the start makes itself.
pub enum Readied {
    Resume { resume: String, drop: Option<Box<dyn FnOnce() -> LocalBoxFuture<'static, Result<(), HostError>>>> },
    Fork { fork: Value },
}

/// `ctx.capHeld.get(turnId)`.
#[derive(Clone, Default)]
pub struct CapHeld {
    pub wait: Option<ThreadCapWait>,
    pub stops: Vec<Joined>,
    pub stopped: Option<String>,
}

/// `ctx.capWait(...)`'s ask (sessions.ts:674).
#[derive(Clone, Debug, PartialEq)]
pub struct CapWaitAsk {
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub session_id: String,
    pub prompt: String,
    pub request_id: Option<String>,
}

/// A send's registered notify (`ctx.notifyOn(threadId)`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registered {
    pub notify: Vec<String>,
    pub by: Option<ThreadScope>,
    pub road: Option<WorkspaceOrigin>,
}

/// The tree a thread sits in (`{ parentThreadId?, rootThreadId? }`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree {
    pub parent_thread_id: Option<String>,
    pub root_thread_id: Option<String>,
}

/// `ctx.resumeSub(...)`'s answer.
pub struct ResumedSub {
    pub entry: Entry,
    pub preamble: Option<String>,
    pub notices: Vec<String>,
}

/// A kept process a turn runs on (`ctx.takeKept(...)`'s answer).
#[derive(Clone)]
pub struct TakenKept {
    pub agent: Arc<dyn KeptAgent>,
    pub turn_token: String,
    pub scope_device_id: Option<String>,
    pub launch: KeptLaunch,
    pub waiting: crate::pending::Waiting,
}

/// What a session write answered (`ctx.writeSession(...)`).
#[derive(Clone, Debug, PartialEq)]
pub enum SessionWrite {
    Written,
    NoSession,
    Failed(String),
}

/// What `ctx.landImages` answers.
#[derive(Clone, Debug, Default)]
pub struct Landed {
    pub images: Vec<TurnImage>,
    pub dir: Option<String>,
}

/// What a page read asks (`ctx.transcriptReader.read(...)`'s options).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadWindow {
    pub before: Option<u64>,
    /// None reads unbounded.
    pub limit: Option<u64>,
    pub bytes: u64,
    pub strict: bool,
    pub head_shape: bool,
}

/// What the sessions area reads of the runtime, sectioned by the unit that reads it. The launch area's reads come
/// through `LaunchCtx`; a trimmed row's through `TrimmedCtx`.
#[async_trait(?Send)]
pub trait SessionsCtx: LaunchCtx + TrimmedCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn sessions(&self) -> Rows;
    fn records(&self) -> Records;
    fn live_entry(&self, workspace_id: &str) -> Option<Entry>;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView>;
    fn thread_runs(&self, thread_id: &str) -> bool;
    fn running_on(&self, thread_id: &str) -> Option<SessionEntry>;
    fn launching_on(&self, thread_id: &str) -> Option<SessionEntry>;
    /// `ctx.entryOfRow({ threadId, workspaceId }, origin, talk, { beingMade })`; None where the caller does not reach.
    async fn entry_of_row(&self, thread_id: Option<&str>, workspace_id: &str, origin: Option<&Caller>, talk: Option<TreeTalk>, being_made: bool) -> Result<Option<Entry>, HostError>;
    /// `ctx.entryOf(id, origin, { beingMade, now })`.
    async fn entry_of_with(&self, workspace_id: &str, origin: Option<&Caller>, being_made: bool, now: bool) -> Result<Entry, HostError>;
    fn record(&self, event: SessionEvent);
    fn emit(&self, event: Value);
    fn pass(&self, event: Value);
    /// `bus.on("*")` for this turn's `session.start` and `session.end`; answers the listener's off.
    fn on_turn_settled(&self, turn_id: &str, f: Rc<dyn Fn()>) -> Rc<dyn Fn()>;
    async fn persist_sessions(&self, workspace_id: &str) -> Result<(), HostError>;
    fn push_head(&self, thread_id: &str);
    fn agent_label(&self, harness: &str) -> String;
    fn waits(&self) -> WaitsArea;
    fn activity(&self) -> Rc<dyn ActivityDoor>;

    // sessions_stop
    fn place_away(&self, place: &str) -> bool;
    fn has_link(&self, place: &str) -> bool;
    async fn ping(&self, place: &str) -> Result<(), HostError>;
    /// The door's `place.absent` listener for one place; answers its off.
    fn on_place_absent(&self, place: &str, f: Rc<dyn Fn()>) -> Rc<dyn Fn()>;
    fn place_name_of(&self, place: &str) -> String;
    /// `workspacePlace(record)`.
    fn workspace_place(&self, record: &WorkspaceRecord) -> Option<String>;
    fn cap_held(&self, turn_id: &str) -> Option<CapHeld>;
    fn cap_stop(&self, turn_id: &str, line: &str);
    /// `ctx.capStopping(sessionId)`: answers its release.
    fn cap_stopping(&self, row_id: &str) -> Rc<dyn Fn()>;
    async fn drop_queued(&self, turn_id: &str) -> Result<bool, HostError>;
    async fn stop_under(&self, thread_id: &str, origin: Option<&Caller>) -> Result<Vec<String>, HostError>;
    fn try_stopped(&self, turn_id: &str);
    /// `ctx.lineTry(row)`.
    fn line_try(&self, row: &SessionEntry) -> bool;
    /// The kind's `endThread`, None where the kind ends no group.
    fn end_thread(&self, entry: &Entry, thread_id: &str, o: EndThread) -> Option<LocalBoxFuture<'static, Result<Option<String>, HostError>>>;
    /// The kind's `endOwed`: None where the kind has none; `Some(None)` where nothing is owed; else the end's `paid`.
    async fn end_owed(&self, entry: &Entry, thread_id: &str) -> Result<Option<Option<Joined>>, HostError>;

    // sessions_start
    fn taken(&self, workspace_id: &str, request_id: &str) -> Option<Taken>;
    async fn taken_turn(&self, workspace_id: &str, taken: Taken) -> Result<Option<Rc<dyn SessionHandle>>, HostError>;
    /// forks.ts `forkOpened(ctx, workspaceId, opened, origin)`.
    async fn fork_opened(&self, workspace_id: &str, opened: StartOptions, origin: Option<&Caller>) -> Result<(StartOptions, Option<ForkPlan>), HostError>;
    fn claim_warm(&self, workspace_id: &str, harness: &str) -> Option<String>;
    fn opens_in(&self, record: &WorkspaceRecord, origin: Option<&Caller>) -> bool;
    fn of_threads_repository(&self, origin: Option<&Caller>, project: &str) -> bool;
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &str, name: &str) -> Option<HostError>;
    fn away_for(&self, record: &WorkspaceRecord, origin: Option<&Caller>, act: &str, workspace_id: &str) -> Option<HostError>;
    async fn worktree_gone(&self, entry: &Entry, why: &str, starting: bool) -> Result<(), HostError>;
    fn computer_of(&self, entry: &Entry) -> String;
    fn refuse_sub_send(&self, entry: &Entry) -> Result<(), HostError>;
    async fn proven(&self, entry: &Entry) -> Result<(), HostError>;
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError>;
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError>;
    async fn kept_check(&self, thread_id: &str) -> Result<(), HostError>;
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    fn parent_of(&self, thread_id: &str) -> Option<String>;
    fn root_of(&self, thread_id: &str) -> String;
    fn tree_of(&self, scope: Option<&ThreadScope>) -> Tree;
    fn thread_of_token(&self, token: &str) -> String;
    fn reaches_row(&self, view: &SessionView, origin: Option<&Caller>, talk: Option<TreeTalk>) -> bool;
    fn notify_reach(&self, notify: &[String]) -> IndexSet<String>;
    fn notify_on(&self, thread_id: &str) -> Option<Registered>;
    async fn resume_sub(&self, entry: &Entry, thread_id: &str, harness: &str, prompt: &str) -> Result<ResumedSub, HostError>;
    async fn place_rows(&self) -> Option<Vec<wsp_wire::views::place_more::PlaceView>>;
    async fn hold_queued(&self, workspace_id: &str, thread_id: &str, tree: Tree, opts: StartOptions, notify: Option<Vec<String>>, origin: Option<&Caller>) -> Result<Rc<dyn SessionHandle>, HostError>;
    fn launch_area(&self) -> LaunchArea;
    /// forks.ts `forkTurnOf(plan, byCount)`.
    fn fork_turn_of(&self, plan: &ForkPlan, by_count: bool) -> Option<Value>;
    fn sub_done(&self, workspace_id: &str);
    /// forks.ts `forkStart(ctx, start, o, origin)`.
    async fn fork_start(&self, opts: ForkOptions, start: StartFn, origin: Option<&Caller>) -> Result<Rc<dyn SessionHandle>, HostError>;

    // sessions_launch
    fn carried_title(&self, thread_id: &str) -> (Option<String>, Option<wsp_wire::views::session::TitleSource>);
    fn cap_hold(&self, record: &WorkspaceRecord, turn_id: &str, lender: Option<&str>);
    fn cap_full(&self, record: &WorkspaceRecord, turn_id: &str) -> Option<ThreadCapWait>;
    fn cap_wait(&self, ask: CapWaitAsk, full: ThreadCapWait) -> Joined;
    fn cap_left(&self, turn_id: &str);
    fn cap_lend(&self, turn_id: &str, lender: &str);
    async fn first_run_here(&self, entry: &Entry, harness: &str) -> Result<bool, HostError>;
    async fn limit_details_due(&self, entry: &Entry, harness: &str) -> Result<bool, HostError>;
    fn host_writes(&self, session: &str) -> Option<Joined>;
    async fn land_images(&self, entry: &Entry, adapter: &dyn HarnessAdapter, landing: &str, dir: &str, attachments: Vec<wsp_wire::types::attachments::Attachment>) -> Result<Landed, HostError>;
    async fn land_files(&self, entry: &Entry, landing: &str, dir: &str, attachments: Vec<wsp_wire::types::attachments::Attachment>) -> Result<Vec<String>, HostError>;
    /// forks.ts `readyFork(adapter, session, turn, refused)`.
    async fn ready_fork(&self, adapter: &dyn HarnessAdapter, session: &str, turn: Value, harness: &str) -> Result<Readied, HostError>;
    /// forks.ts `copyImages(ctx, plan, at)`.
    async fn copy_images(&self, plan: &ForkPlan, workspace_id: &str, thread_id: &str) -> Result<(), HostError>;
    /// forks.ts `payCutOwed(ctx, adapter, at, resume)`.
    async fn pay_cut_owed(&self, adapter: &dyn HarnessAdapter, workspace_id: &str, thread_id: &str, resume: &str) -> Result<String, HostError>;
    /// `ctx.snapshotOf(entry, cwd)`: the commit the turn's changes start from, once taken.
    fn snapshot_of(&self, entry: &Entry, cwd: &str) -> futures::future::Shared<LocalBoxFuture<'static, Option<String>>>;
    async fn keep_steer(&self, running: &SessionEntry, opts: &StartOptions, origin: Option<&Caller>, steer_id: &str) -> Result<(), HostError>;
    fn steer_lost(&self, running: &SessionEntry, steer_id: &str);
    fn steer_answered(&self, running: &SessionEntry, handle_id: &str, opts: &StartOptions, origin: Option<&Caller>, steer_id: &str, accepted: bool) -> bool;
    fn cut_before(&self, workspace_id: &str, thread_id: &str) -> bool;
    /// `ctx.takeKept(threadId, ask, resume, launchKey)`.
    fn take_kept(&self, thread_id: &str, ask: Option<KeptLaunch>, resume: Option<&str>, launch_key: Option<&KeptLaunch>) -> Option<TakenKept>;
    /// forks.ts `copiedEvents(plan, at)`, recorded through `ctx.recordCopies`.
    fn record_copies(&self, plan: &ForkPlan, workspace_id: &str, thread_id: &str);
    fn run_turn(&self, t: TurnStart) -> Result<Rc<dyn SessionHandle>, HostError>;
    async fn keep_sent_images(&self, workspace_id: &str, thread_id: &str, request_id: Option<&str>, attachments: Vec<wsp_wire::types::attachments::Attachment>) -> Result<(), HostError>;
    fn cancel_resume_timer(&self, thread_id: &str);
    async fn remember_target(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    /// `ctx.notifyEnd({ view, turnId }, notify, ctx.tellAs({ notifyBy?, notifyRoad? }), result)`.
    fn notify_end(&self, view: SharedView, turn_id: &str, notify: &[String], by: Option<&ThreadScope>, road: Option<&WorkspaceOrigin>, result: wsp_wire::views::session_events::TurnResult);
    fn drop_images(&self, entry: &Entry, dir: &str);
    async fn drop_sent_images(&self, workspace_id: &str, thread_ids: &[String]) -> Result<(), HostError>;
    fn end_kept(&self, thread_id: &str, kept: &TakenKept);
    /// `ctx.seedFor(...)`: the thread's earlier turns as its seed, this turn left out.
    async fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError>;

    // sessions_reads
    fn tree_stands_on(&self, workspace_id: &str, origin: Option<&Caller>, talk: Option<TreeTalk>) -> bool;
    fn refuse_named(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    fn title_rows(&self, rows: Vec<SessionView>) -> Vec<SessionView>;
    fn booting(&self, workspace_id: &str) -> bool;
    async fn refresh_title(&self, view: SessionView, ended: bool) -> Result<(), HostError>;
    fn tree_records(&self, origin: Option<&Caller>, talk: TreeTalk, workspace_id: Option<&str>) -> Vec<(String, crate::pending::HeldThread)>;
    fn listed_rows(&self, rows: Vec<SessionView>) -> Vec<SessionView>;
    fn placed_rows(&self, rows: Vec<SessionView>) -> Vec<SessionView>;
    fn drives_thread(&self, thread_id: Option<&str>, origin: Option<&Caller>) -> bool;
    async fn open_transcript(&self, workspace_id: &str) -> Result<Vec<SessionEvent>, HostError>;
    fn opened_words(&self, workspace_id: &str, thread_id: &str);
    fn index_pos(&self, workspace_id: &str) -> Option<u64>;
    async fn read_page(&self, workspace_id: &str, thread_id: &str, window: ReadWindow) -> Result<HistoryPage, HostError>;
    fn thread_facts(&self, thread_id: &str) -> Option<ThreadFacts>;
    async fn facts_after_read(&self, thread_id: &str, origin: Option<&Caller>) -> Result<Option<ThreadFacts>, HostError>;
    async fn kept_writes(&self, workspace_id: &str);
    async fn store_get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError>;
    async fn store_get_blob(&self, collection: &str, key: &str) -> Result<Option<Vec<u8>>, HostError>;
    fn index_workspaces(&self) -> Vec<String>;
    /// `searchIndex(workspaceId, index, words, drives, found)`.
    fn search_in(&self, workspace_id: &str, words: &str, drives: &dyn Fn(&str) -> bool, found: &mut Vec<SearchedHit>) -> u32;

    // sessions_acts
    fn record_steer(&self, row: &SessionEntry, row_id: &str, ask: &SteerAsk, origin: Option<&Caller>, steer_id: &str);
    async fn launch_adapter_for(&self, entry: &Entry, harness: &str, server_values: Option<wsp_adapters::port::ServerValues>) -> Result<BuiltAdapter, HostError>;
    async fn write_session(&self, session: &str, entry: &Entry, adapter: Arc<dyn HarnessAdapter>, title: &str) -> Result<SessionWrite, HostError>;
    fn brokered(&self, record: &WorkspaceRecord) -> bool;
    async fn server_values_for(&self, entry: &Entry, harness: &str, cwd: &str) -> Result<Option<wsp_adapters::port::ServerValues>, HostError>;
    async fn mark(&self, thread_ids: &[String], stamps: &dyn Fn(&str) -> ThreadStamps, origin: Option<&Caller>) -> Result<(), HostError>;
    async fn arm_resume(&self, thread_ids: &[String], on: bool, origin: Option<&Caller>) -> Result<(), HostError>;
    async fn aside(&self, adapter: &dyn HarnessAdapter, ask: AsideQuestion) -> Result<String, HostError>;

    // sessions_settle
    fn subagent_known(&self, id: &str) -> bool;
    fn settles_thread(&self, thread_id: &str, origin: Option<&Caller>) -> bool;
    fn settled_now(&self, thread_id: &str, settle_ms: Option<f64>) -> bool;
    fn tree_under(&self, thread_id: &str) -> Vec<String>;
    fn restarts(&self) -> IndexMap<String, String>;

    // sessions_rewind
    fn reap_kept(&self, thread_id: &str);
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    async fn wake(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    /// `ctx.queued(id, () => ctx.withDaemon(entry, ask => ask({ op: "git.restore", cwd, checkpoint, scope })))`.
    async fn git_restore(&self, entry: &Entry, cwd: &str, checkpoint: &str) -> Result<Map<String, Value>, HostError>;
    async fn flush_transcript(&self, workspace_id: &str) -> Result<(), HostError>;
    /// forks.ts `moveSession(ctx, threadId, from, to)`.
    fn move_session(&self, thread_id: &str, from: &str, to: &str);
    async fn drop_from_transcript(&self, workspace_id: &str, thread_id: &str, turns: Option<&[String]>) -> Result<(), HostError>;
    async fn slates_rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError>;

    // sessions_delete
    fn lead_of_sub(&self, workspace_id: Option<&str>, origin: Option<&Caller>) -> bool;
    async fn sub_delete(&self, entry: &Entry, thread_id: &str, origin: Option<&Caller>, ask: DeleteThreadAsk) -> Result<ThreadDeleted, HostError>;
    fn turn_runs(&self, workspace_id: &str) -> bool;
    async fn end_thread_shells(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError>;
    async fn delete_workspace(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    async fn drop_checkpoints(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError>;
    async fn drop_thread_files(&self, entry: &Entry, thread_ids: &[String]) -> Result<(), HostError>;
    fn row_gone(&self, row_id: &str, workspace_id: &str, thread_id: &str);
    async fn slates_forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError>;
}

/// A send waiting on an end owed to a computer that is away (sessions.ts:71).
#[derive(Clone)]
pub struct HeldSend {
    pub thread_id: String,
    pub give_up: Rc<dyn Fn() -> String>,
}

/// What a stop owes and holds, by thread (sessions.ts:47-71). Memory only.
#[derive(Default)]
pub struct StopState {
    pub ending: IndexMap<String, Joined>,
    pub waits_for: IndexMap<String, String>,
    pub behind_turn: IndexMap<String, Vec<(u64, Rc<dyn Fn()>)>>,
    pub held_sends: IndexMap<String, HeldSend>,
}

/// sessions.ts:41. Clones share one state.
#[derive(Clone)]
pub struct SessionsArea {
    pub(crate) ctx: Rc<dyn SessionsCtx>,
    pub(crate) stops: Rc<RefCell<StopState>>,
    /// sessions.ts:212.
    pub(crate) last_settle_at: Rc<Cell<f64>>,
}

impl SessionsArea {
    pub fn new(ctx: Rc<dyn SessionsCtx>) -> Self {
        todo!()
    }

    /// sessions.ts:1497: whether a stop's end of the thread's group holds its next turn.
    pub fn stop_holds(&self, thread_id: &str) -> bool {
        todo!()
    }
}

/// The runtime's `sessions`, each verb answered by its unit's own method.
#[async_trait(?Send)]
impl SessionsApi for SessionsArea {
    async fn start(&self, workspace_id: &str, opts: StartOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        self.start_send(workspace_id, opts, origin).await
    }
    async fn list(&self, workspace_id: Option<String>, origin: Option<Caller>) -> Result<Vec<SessionView>, HostError> {
        self.list_rows(workspace_id, origin).await
    }
    async fn history(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError> {
        self.history_of(workspace_id, origin).await
    }
    async fn thread_history(&self, workspace_id: &str, asked: HistoryAsk, origin: Option<Caller>) -> Result<Vec<SessionEvent>, HostError> {
        self.thread_history_of(workspace_id, asked, origin).await
    }
    async fn page(&self, workspace_id: &str, window: PageWindow, origin: Option<Caller>) -> Result<HistoryPage, HostError> {
        self.page_of(workspace_id, window, origin).await
    }
    async fn head(&self, thread_id: &str, origin: Option<Caller>, after_read: Option<bool>) -> Result<ThreadHead, HostError> {
        self.head_of(thread_id, origin, after_read == Some(true)).await
    }
    async fn attachment(&self, workspace_id: &str, thread_id: &str, request_id: &str, index: u32, origin: Option<Caller>) -> Result<KeptAttachment, HostError> {
        self.attachment_of(workspace_id, thread_id, request_id, index, origin).await
    }
    async fn interrupt(&self, session_id: &str, origin: Option<Caller>, task: Option<String>) -> Result<SessionInterruptResult, HostError> {
        self.interrupt_row(session_id, origin, task).await
    }
    async fn steer(&self, session_id: &str, ask: SteerAsk, origin: Option<Caller>) -> Result<SessionSteerResult, HostError> {
        self.steer_row(session_id, ask, origin).await
    }
    async fn answer(&self, session_id: &str, ask: AnswerAsk, origin: Option<Caller>) -> Result<SessionAnswerResult, HostError> {
        self.answer_row(session_id, ask, origin).await
    }
    async fn access(&self, session_id: &str, permission_mode: &str, origin: Option<Caller>) -> Result<SessionAccessResult, HostError> {
        self.access_row(session_id, permission_mode, origin).await
    }
    async fn rename(&self, session_id: &str, title: &str, origin: Option<Caller>) -> Result<SessionRenameResult, HostError> {
        self.rename_row(session_id, title, origin).await
    }
    async fn read(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.read_thread(thread_id, origin).await
    }
    async fn note_waited(&self, thread_id: &str, ms: f64, said: Option<String>) -> Result<(), HostError> {
        self.ctx.waits().note_waited(thread_id, ms, said.as_deref());
        Ok(())
    }
    fn note_retry(&self, thread_id: &str, attempt: u32, why: &str) {
        self.ctx.waits().note_retry(thread_id, crate::threads::waits::RetryNote { attempt, why: why.to_string() })
    }
    async fn replaceable(&self, thread_id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.replaceable_check(thread_id, origin.as_ref()).await
    }
    async fn fork(&self, opts: ForkOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        self.fork_send(opts, origin).await
    }
    async fn warm(&self, workspace_id: &str, ask: WarmAsk, origin: Option<Caller>) -> Result<SessionWarmResult, HostError> {
        self.ctx.launch_area().warm(workspace_id, ask, origin).await
    }
    async fn settle(&self, thread_ids: Vec<String>, origin: Option<Caller>, ask: SettleAsk) -> Result<SessionSettleResult, HostError> {
        self.settle_threads(thread_ids, origin, ask).await
    }
    async fn mark(&self, thread_ids: Vec<String>, marks: ThreadMarks, origin: Option<Caller>) -> Result<(), HostError> {
        self.mark_threads(thread_ids, marks, origin).await
    }
    async fn restore(&self, thread_ids: Vec<String>, origin: Option<Caller>) -> Result<SessionRestoreResult, HostError> {
        self.restore_threads(thread_ids, origin).await
    }
    async fn search(&self, query: &str, origin: Option<Caller>) -> Result<SessionSearchResult, HostError> {
        self.search_for(query, origin).await
    }
    async fn aside(&self, session_id: &str, question: &str, origin: Option<Caller>, ask_id: Option<String>) -> Result<SessionAsideResult, HostError> {
        self.aside_row(session_id, question, origin, ask_id).await
    }
    async fn rewind(&self, thread_id: &str, ask: RewindAsk, origin: Option<Caller>) -> Result<SessionRewindResult, HostError> {
        self.rewind_thread(thread_id, ask, origin).await
    }
    async fn run(&self, step: RunStep, origin: Option<Caller>) -> Result<SessionRunEvent, HostError> {
        self.run_step(step, origin).await
    }
    async fn forget(&self, thread_id: &str, origin: Option<Caller>, check: Option<bool>) -> Result<(), HostError> {
        self.forget_thread(thread_id, origin, check == Some(true)).await
    }
    async fn delete(&self, thread_id: &str, origin: Option<Caller>, ask: DeleteThreadAsk) -> Result<ThreadDeleted, HostError> {
        self.delete_thread(thread_id, origin, ask).await
    }
}
