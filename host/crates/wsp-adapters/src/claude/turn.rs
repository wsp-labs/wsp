// SPDX-License-Identifier: AGPL-3.0-only
//! One turn of Claude Code: the reader of its stream (`follow`) and what a caller does to a running turn (steer,
//! answer, set_access, stop_task, interrupt, kept). TS: packages/adapter-claude/src/adapter.ts:643-1270 at a3b870a6e.
//!
//! Locks: the turn's state sits behind one lock. `on_event` is never called while it is held, and neither is any
//! ExecStream method that may wait (write, the end of a run): a caller may call any method of the session from
//! inside `on_event`, and a stream may answer `write` only after a line is read.

use std::sync::Arc;

use crate::claude::adapter::AdapterCore;
use crate::claude::normalize::SavedUse;
use crate::port::{AccessOutcome, AnswerOutcome, ExecFailure, ExecStream, KeptRun, KeptTurn, OnEvent, PermissionAnswer, SessionFile, SharedFuture, SteerOutcome, TaskStop, TurnImage, TurnResult};

/// adapter.ts:150.
pub const STOP_TASK_WAIT_MS: u64 = 10_000;

/// What a turn is once its stream exists. adapter.ts:646 (`follow`'s options).
pub struct FollowOptions {
    pub stream: Arc<dyn ExecStream>,
    pub local_id: String,
    /// The CLI announced itself to an earlier host process (an attach): a steer is taken from the first byte.
    pub announced: bool,
    /// A new session: running totals start at nothing.
    pub fresh: bool,
    pub command: Option<String>,
    pub keeper: Option<Arc<dyn KeptRun>>,
    pub from: Option<u64>,
    pub saved: Option<SavedUse>,
    pub steered: Option<Vec<String>>,
    pub on_event: OnEvent,
}

/// A turn of Claude Code, as the runtime and the tests hold it. Cloning shares the turn. Fields are the
/// implementer's: a session made by a start that resumed a lost session delegates to the launch running now
/// (adapter.ts:1312), every other is one `follow`.
#[derive(Clone)]
pub struct ClaudeSession {
    inner: Arc<dyn std::any::Any>,
}

/// Reads `o.stream` on a task of its own from the moment it is called, and registers the session in `core`'s map
/// under `local_id`. adapter.ts:646.
pub fn follow(core: &Arc<AdapterCore>, o: FollowOptions) -> ClaudeSession {
    todo!()
}

impl ClaudeSession {
    /// Whether both handles are the same turn (the TS `toBe`).
    pub fn same(&self, other: &ClaudeSession) -> bool {
        todo!()
    }
    /// The registry key, fixed before spawn. adapter.ts:78.
    pub fn local_id(&self) -> String {
        todo!()
    }
    /// The id the CLI's init named; local_id until then. adapter.ts:80.
    pub fn claude_session_id(&self) -> String {
        todo!()
    }
    /// The line the launch ran (after the sign-in guard and the saved-spend step it is not); `None` on an attach.
    pub fn command(&self) -> Option<String> {
        todo!()
    }
    pub fn run(&self) -> Option<String> {
        todo!()
    }
    pub fn pid(&self) -> Option<u32> {
        todo!()
    }
    pub fn from(&self) -> Option<u64> {
        todo!()
    }
    /// Settles once session.end has been emitted.
    pub fn finished(&self) -> SharedFuture<TurnResult> {
        todo!()
    }
    /// adapter.ts:1235.
    pub async fn interrupt(&self) {
        todo!()
    }
    /// adapter.ts:1254. `None` unless the turn was kept, its process is up and its result was seen.
    pub fn kept(&self) -> Option<ClaudeKept> {
        todo!()
    }
    /// adapter.ts:1169. `id` is a random uuid where `None`.
    pub async fn steer(&self, prompt: &str, id: Option<String>, images: Vec<TurnImage>) -> Result<SteerOutcome, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1166: the CLI's init declared msg_lifecycle_v1, or it reported a command_lifecycle line.
    pub fn tells_unread(&self) -> bool {
        todo!()
    }
    /// adapter.ts:1141.
    pub async fn answer(&self, ask_id: &str, answer: PermissionAnswer) -> Result<AnswerOutcome, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1181.
    pub async fn set_access(&self, mode: &str) -> Result<AccessOutcome, ExecFailure> {
        todo!()
    }
    /// adapter.ts:1209.
    pub async fn stop_task(&self, task: &str) -> Result<TaskStop, ExecFailure> {
        todo!()
    }
}

/// The process a finished turn left up for the thread's next message. adapter.ts:1254 and 1298.
#[derive(Clone)]
pub struct ClaudeKept {
    inner: Arc<dyn std::any::Any>,
}

impl ClaudeKept {
    /// The next turn on the process, its message written at once or once `turn.after` settles. adapter.ts:1274.
    pub fn next(&self, turn: KeptTurn) -> ClaudeSession {
        todo!()
    }
    /// `now`: wait 0 before the tree is ended; else result_exit_ms. adapter.ts:1261.
    pub async fn close(&self, now: bool) {
        todo!()
    }
    pub fn exited(&self) -> SharedFuture<Option<i32>> {
        todo!()
    }
    /// The session's file; `None` on a process warmed ahead of any turn.
    pub fn session_file(&self) -> Option<SessionFile> {
        todo!()
    }
}

impl crate::port::Finishes for ClaudeSession {
    fn finished(&self) -> SharedFuture<TurnResult> {
        todo!()
    }
}

/// The runtime's view of a Claude turn: every optional TS member present.
impl crate::port::HarnessSession for ClaudeSession {
    fn local_id(&self) -> String {
        todo!()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        todo!()
    }
    fn run(&self) -> Option<String> {
        todo!()
    }
    fn pid(&self) -> Option<u32> {
        todo!()
    }
    fn from(&self) -> Option<u64> {
        todo!()
    }
    fn interrupt(&self) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn kept(&self) -> Option<Option<Box<dyn crate::port::KeptAgent>>> {
        todo!()
    }
    fn steer(&self, prompt: String, id: Option<String>, images: Vec<TurnImage>) -> Option<futures::future::LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        todo!()
    }
    fn tells_unread(&self) -> Option<bool> {
        todo!()
    }
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> Option<futures::future::LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        todo!()
    }
    fn set_access(&self, mode: String) -> Option<futures::future::LocalBoxFuture<'static, Result<AccessOutcome, ExecFailure>>> {
        todo!()
    }
    fn stop_task(&self, task: String) -> Option<futures::future::LocalBoxFuture<'static, Result<TaskStop, ExecFailure>>> {
        todo!()
    }
}

impl crate::port::KeptAgent for ClaudeKept {
    fn next(&self, turn: KeptTurn) -> Result<Arc<dyn crate::port::HarnessSession>, crate::port::AdapterError> {
        todo!()
    }
    fn close(&self, now: bool) -> futures::future::LocalBoxFuture<'static, ()> {
        todo!()
    }
    fn exited(&self) -> SharedFuture<()> {
        todo!()
    }
    fn session_file(&self) -> Option<SessionFile> {
        todo!()
    }
}
