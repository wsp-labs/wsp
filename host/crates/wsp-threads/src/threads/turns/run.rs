// SPDX-License-Identifier: AGPL-3.0-only
//! turns.ts:17-290, 569-741: a turn's state, its row, its handle and its ends. Spec: spec/turns_run.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use wsp_adapters::port::{HarnessSession, PermissionAsk, TaskStop, TurnImage};
use wsp_core::types::internal::KeptLaunch;
use wsp_core::types::wiring::{AccessMove, Local, SessionHandle, SteerTaken};
use wsp_wire::views::session::{PermissionOutcome, SessionView};
use wsp_wire::views::session_events::{TurnResult, TurnStatus};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::runtime::{SessionAnswerResult, SessionStartOutcome};
use wsp_words::exit::HostError;

use super::{Opening, TurnSnapshot, TurnStart, TurnsArea, TurnsCtx};
use crate::threads::threads::{Entry, Row, Waiting};

/// The counts a turn has written of each numbered row kind, and how many of them a re-read still has to skip.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counts {
    pub lines: u64,
    pub replaying_lines: u64,
    pub subagents: u64,
    pub replaying_subagents: u64,
    pub compactions: u64,
    pub replaying_compactions: u64,
    pub retries: u64,
    pub replaying_retries: u64,
    pub replaying_told: u64,
    pub start_recorded: bool,
    /// The reply the transcript already holds, skipped once (`replyRecorded`).
    pub reply_recorded: Option<TurnStatus>,
}

/// What a turn holds while it runs (the closures' lets of turns.ts:69-290). One per turn, shared by its forward, its
/// handle, its `end` and its `settled`.
pub struct TurnState {
    pub(crate) ctx: Rc<dyn TurnsCtx>,
    pub(crate) entry: Entry,
    pub(crate) workspace_id: String,
    /// The turn's row: its view, live state, asked and calls are the row's own, shared with whoever holds the row.
    pub(crate) row: Row,
    pub(crate) thread_id: String,
    pub(crate) turn_id: String,
    pub(crate) row_id: RefCell<String>,
    pub(crate) notify: Option<Vec<String>>,
    pub(crate) notify_by: Option<ThreadScope>,
    pub(crate) notify_road: Option<WorkspaceOrigin>,
    pub(crate) turn_token: Option<String>,
    pub(crate) scope_device_id: Option<String>,
    pub(crate) outcome: SessionStartOutcome,
    pub(crate) opening: Opening,
    pub(crate) images_dir: Option<String>,
    pub(crate) snapshot: Option<TurnSnapshot>,
    pub(crate) waiting: Option<Waiting>,
    pub(crate) keep: Option<KeptLaunch>,
    /// Whether this turn is a re-open (TS `t.written !== undefined`).
    pub(crate) reopened: bool,
    pub(crate) counts: RefCell<Counts>,
    /// The paths the agent's tool calls wrote, where the harness reports edits.
    pub(crate) wrote: Option<RefCell<IndexSet<String>>>,
    pub(crate) changes_read: Cell<bool>,
    pub(crate) changes_out: Cell<bool>,
    pub(crate) end_written: Cell<bool>,
    pub(crate) turn_account: RefCell<Option<(String, Option<String>)>>,
    /// What the agent held after its latest call.
    pub(crate) held: RefCell<Option<(f64, Option<f64>)>>,
    pub(crate) result_held: Cell<bool>,
    pub(crate) ended: Cell<bool>,
    pub(crate) over: Cell<bool>,
    pub(crate) stopped: Cell<bool>,
    pub(crate) anchor: RefCell<Option<String>>,
    pub(crate) kept_why: RefCell<Option<String>>,
    pub(crate) open: RefCell<IndexMap<String, PermissionAsk>>,
    pub(crate) tasks_running: Cell<bool>,
    pub(crate) waited: Cell<f64>,
    pub(crate) waiting_since: Cell<Option<f64>>,
    pub(crate) unread: RefCell<Vec<String>>,
    pub(crate) session: RefCell<Option<Arc<dyn HarnessSession>>>,
    /// The closure `write_held` registered with `held_at_close`, kept to take it off.
    pub(crate) write_held: RefCell<Option<Rc<dyn Fn()>>>,
}

impl TurnState {
    /// The state a `TurnStart` opens with (turns.ts:68-170). Rules N1-N3.
    pub fn new(ctx: Rc<dyn TurnsCtx>, t: &TurnStart) -> Rc<Self> {
        todo!()
    }

    /// The row the notify and cut roads read.
    pub fn row(&self) -> Row {
        todo!()
    }

    /// turns.ts:118. Rule N3.
    pub fn write_held(&self) {
        todo!()
    }

    /// turns.ts:132. Rule N18.
    pub fn turn_over(self: &Rc<Self>) {
        todo!()
    }

    /// turns.ts:80. Rule N19.
    pub fn read_changes(self: &Rc<Self>, session_id: &str) {
        todo!()
    }

    /// turns.ts:181. Rule N8.
    pub fn waited_so_far(&self) -> f64 {
        todo!()
    }

    /// turns.ts:185. Rule N8.
    pub fn with_waited(&self, reply: TurnResult) -> TurnResult {
        todo!()
    }

    /// turns.ts:194. Rule N9.
    pub fn after_told(&self, result: &TurnResult, held: bool) -> Option<TurnResult> {
        todo!()
    }

    /// turns.ts:207. Rule N4.
    pub fn reads_waiting(&self) {
        todo!()
    }

    /// turns.ts:215. Rule N4.
    pub fn reads_open(&self) {
        todo!()
    }

    /// turns.ts:238. Rule N5.
    pub fn named(&self, ask: &PermissionAsk) -> PermissionAsk {
        todo!()
    }

    /// turns.ts:248. Rule N6.
    pub fn close_ask(&self, ask_id: &str, outcome: PermissionOutcome, option_id: Option<&str>) {
        todo!()
    }

    /// turns.ts:265. Rule N6.
    pub fn close_open_asks(&self) {
        todo!()
    }

    /// turns.ts:271. Rule N7.
    pub async fn answer(&self, ask_id: &str, option_id: &str, reason: Option<&str>) -> SessionAnswerResult {
        todo!()
    }

    /// turns.ts:290: the turn's line's try (`ctx.lineTry`).
    pub fn line_try(&self) -> bool {
        todo!()
    }

    /// turns.ts:634. Rule N17.
    pub fn end(self: &Rc<Self>, reason: &str, by_stop: bool, unreached: bool) {
        todo!()
    }

    /// turns.ts:671. Rules N20-N25.
    pub fn settled(self: &Rc<Self>, status: TurnStatus) {
        todo!()
    }
}

/// The handle a turn answers (turns.ts:617-633).
pub struct TurnHandle {
    pub(crate) state: Rc<TurnState>,
    pub(crate) finished: Shared<LocalBoxFuture<'static, TurnResult>>,
}

impl SessionHandle for TurnHandle {
    fn id(&self) -> String {
        todo!()
    }
    fn workspace_id(&self) -> String {
        todo!()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        todo!()
    }
    fn turn_id(&self) -> String {
        todo!()
    }
    fn outcome(&self) -> SessionStartOutcome {
        todo!()
    }
    fn notice(&self) -> Option<String> {
        todo!()
    }
    fn view(&self) -> SessionView {
        todo!()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn steer(&self, prompt: String, id: Option<String>, images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        todo!()
    }
    fn tells_unread(&self) -> Option<bool> {
        todo!()
    }
    fn answer(&self, ask_id: String, option_id: String, reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        todo!()
    }
    fn set_access(&self, mode: String) -> Option<Local<'static, AccessMove>> {
        todo!()
    }
    fn stop_task(&self, task: String) -> Option<Local<'static, TaskStop>> {
        todo!()
    }
}

impl TurnsArea {
    /// turns.ts:19. Rules N10-N16, N26; the switch over events is `TurnState::forward`.
    pub fn run_turn(&self, t: TurnStart) -> Result<Rc<dyn SessionHandle>, HostError> {
        todo!()
    }
}

