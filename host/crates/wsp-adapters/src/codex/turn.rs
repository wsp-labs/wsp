// SPDX-License-Identifier: AGPL-3.0-only
//! One Codex turn read off `codex app-server`, and the session it answers. TS adapter-codex/src/adapter.ts:519-1227
//! (`follow`). Spec: spec/codex_turn.md. The reader is spawned at `follow`; every method's synchronous half (the TS
//! before its first await) runs when it is called, before its future is returned (rule 23).

use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use serde_json::{Map, Value};

use super::background::Background;
use super::rpc::{RequestId, ThreadOptions};
use crate::port::{AnswerOutcome, ExecFailure, ExecStream, KeptRun, KeptTurn, Now, OnEvent, PermissionAnswer, SessionFile, Settle, SharedFuture, SteerOutcome, TaskStop, TurnResult};

/// What a side run does with each answer: the next line to send, or how the run ends. TS turn-pages.ts:10.
pub enum SideStep {
    Send(String),
    End(TurnResult),
}

/// A side run's handler, handed the thread's answer and each later answer of its own. TS turn-pages.ts:10.
pub type SideRun = Box<dyn FnMut(&RequestId, Option<&Map<String, Value>>) -> SideStep + Send>;

/// The thread already open on the server. TS adapter.ts:548.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedThread {
    pub thread_id: String,
    pub legacy: bool,
}

/// The adapter-wide settings a turn reads. TS adapter.ts:513-517 and the `deps` it closes over.
#[derive(Clone)]
pub struct TurnContext {
    pub login: String,
    pub key_env: Option<String>,
    pub home: String,
    pub grace_ms: u64,
    pub result_exit_ms: u64,
    pub steer_join_ms: u64,
    pub reconnect_stall_ms: u64,
    pub aside_wall_ms: u64,
    pub now: Now,
}

/// Everything a turn is once its stream exists. TS adapter.ts:522.
pub struct FollowOptions {
    pub stream: Arc<dyn ExecStream>,
    pub local_id: String,
    /// ms epoch.
    pub started_at: i64,
    pub command: Option<String>,
    pub model: Option<String>,
    pub cwd: Option<String>,
    /// The line that starts the turn, handed the thread id once the thread answers.
    pub turn_line: Option<Box<dyn Fn(&str) -> String + Send + Sync>>,
    /// What turn/start waits on once the thread answers.
    pub prompt_after: Option<Settle>,
    /// A re-opened run's turn, written where the run's channel holds no turn.
    pub owed_turn: Option<Box<dyn Fn(&str) -> String + Send + Sync>>,
    /// A side question's own run: approvals declined, no registry, a wall.
    pub aside: bool,
    /// A run that asks the thread things and runs no turn.
    pub side_run: Option<SideRun>,
    /// The server this turn runs on, kept for the thread's next turn.
    pub keeper: Option<Arc<dyn KeptRun>>,
    /// The commands that server runs past its turns.
    pub background: Option<Arc<Mutex<Background>>>,
    /// Where this turn starts in the run's log.
    pub from: Option<u64>,
    pub opened: Option<OpenedThread>,
    pub thread_options: Option<ThreadOptions>,
    pub on_event: OnEvent,
}

/// What a session does, whether it is one turn or the lost-session wrapper over two. Not part of the port.
pub trait SessionCore: Send + Sync {
    fn local_id(&self) -> String;
    fn thread_id(&self) -> String;
    fn command(&self) -> Option<String>;
    fn run(&self) -> Option<String>;
    fn pid(&self) -> Option<u32>;
    fn from(&self) -> Option<u64>;
    fn finished(&self) -> SharedFuture<TurnResult>;
    fn interrupt(&self) -> BoxFuture<'static, ()>;
    fn kept(&self) -> Option<CodexKept>;
    fn steer(&self, prompt: String) -> BoxFuture<'static, SteerOutcome>;
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> BoxFuture<'static, Result<AnswerOutcome, ExecFailure>>;
    fn stop_task(&self, task: String) -> BoxFuture<'static, TaskStop>;
}

/// One Codex turn as the runtime holds it. TS adapter.ts:143 (CodexSession).
#[derive(Clone)]
pub struct CodexSession {
    pub core: Arc<dyn SessionCore>,
}

impl CodexSession {
    /// The registry key: the resume id, or one minted at start.
    pub fn local_id(&self) -> String {
        todo!()
    }
    /// The id the server gave the thread, equal to the local id until it does.
    pub fn thread_id(&self) -> String {
        todo!()
    }
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
    pub fn finished(&self) -> SharedFuture<TurnResult> {
        todo!()
    }
    /// TS adapter.ts:1193.
    pub fn interrupt(&self) -> BoxFuture<'static, ()> {
        todo!()
    }
    /// TS adapter.ts:1210.
    pub fn kept(&self) -> Option<CodexKept> {
        todo!()
    }
    /// TS adapter.ts:1154.
    pub fn steer(&self, prompt: String) -> BoxFuture<'static, SteerOutcome> {
        todo!()
    }
    /// TS adapter.ts:1169. `Err` is the write's own failure, the ask left open.
    pub fn answer(&self, ask_id: String, answer: PermissionAnswer) -> BoxFuture<'static, Result<AnswerOutcome, ExecFailure>> {
        todo!()
    }
    /// TS adapter.ts:1186.
    pub fn stop_task(&self, task: String) -> BoxFuture<'static, TaskStop> {
        todo!()
    }
}

/// The server a finished turn left up for the thread's next turn. TS adapter.ts:1213 (KeptAgent<CodexSession>).
pub struct CodexKept {
    pub next: Arc<dyn Fn(KeptTurn) -> Result<CodexSession, String> + Send + Sync>,
    /// `now` closes with no wait.
    pub close: Arc<dyn Fn(bool) -> BoxFuture<'static, ()> + Send + Sync>,
    pub exited: SharedFuture<Option<i32>>,
    pub working: Arc<dyn Fn() -> bool + Send + Sync>,
    pub session_file: SessionFile,
}

/// The turns a registry holds by local id; side questions and side runs never join it. TS adapter.ts:514.
pub type Sessions = Arc<Mutex<indexmap::IndexMap<String, CodexSession>>>;

/// Starts reading one turn off its stream and answers its session. TS adapter.ts:522.
pub fn follow(ctx: &TurnContext, sessions: &Sessions, o: FollowOptions) -> CodexSession {
    todo!()
}

/// The thread a kept server holds, as the last turn left it. TS adapter.ts:1213's `thread`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptThread {
    pub thread_id: String,
    pub legacy: bool,
    pub model: Option<String>,
    pub cwd: Option<String>,
}

/// The thread's next turn on a server its last turn left up: thread/resume then turn/start, written together, where
/// the thread was let go. An image with no path, or one not absolute, refuses before anything is written. TS
/// adapter.ts:1238.
pub fn next_on(ctx: &TurnContext, sessions: &Sessions, keeper: Arc<dyn KeptRun>, local_id: &str, thread: KeptThread, turn: KeptTurn, thread_options: Option<ThreadOptions>, background: Option<Arc<Mutex<Background>>>) -> Result<CodexSession, String> {
    todo!()
}
