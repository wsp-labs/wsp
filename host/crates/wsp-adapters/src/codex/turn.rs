// SPDX-License-Identifier: AGPL-3.0-only
//! One Codex turn read off `codex app-server`, and the session it answers. TS adapter-codex/src/adapter.ts:519-1227
//! (`follow`). Spec: spec/codex_turn.md. The reader is spawned at `follow`; every method's synchronous half (the TS
//! before its first await) runs when it is called, before its future is returned (rule 23).
//!
//! Locks: the turn's state sits behind one RefCell. It is never borrowed across an `on_event` call or a stream call,
//! so a callback may call any method of the session from inside `on_event`.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::StreamExt;
use futures::task::noop_waker_ref;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use super::background::Background;
use super::children::Children;
use super::command::image_path;
use super::conversations::{LETS_GO, WRITER_HELD};
use super::fields::{rec, str};
use super::items::{changes_of, failure_words, is_turn_start, rollout_in, FailureWords, UsageSeen, APPROVALS};
use super::limits::{credits_of, limit_of, snapshot_of, stopped_until, LimitAccount};
use super::rpc::{
    decision_line, read_message, refuse_request_line, request, thread_resume_line, thread_unsubscribe_line, turn_interrupt_line,
    turn_start_line, turn_steer_line, Decision, RequestId, RpcMessage, ThreadOptions,
};
use super::shell_script::shell_script_of;
use crate::pending::{aside_wall_line, codex_reconnect_line, conversation_written_elsewhere_line};
use crate::port::DeltaKind;
use crate::port::{
    end_after_result, end_run, settles_within, AdapterEvent, AnswerOutcome, ExecFailure, ExecStream, KeptRun, KeptTurn, Now, OnEvent,
    PermissionAnswer, PermissionAsk, PermissionOutcome, SessionFile, Settle, SharedFuture, SteerOutcome, SubagentState, TaskStop,
    TurnDelta, TurnImage, TurnResult, TurnStatus, WriteOutcome, PERMISSION_ALLOW, PERMISSION_DENY,
};
use crate::port::{fire, started};
use futures::future::Shared;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};
use wsp_words::js::js_trim;

/// The compaction the server said it ran, from its item's start until it is said. TS adapter.ts:570.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Compaction {
    pub(super) before: Option<f64>,
    pub(super) after: Option<f64>,
    pub(super) ended: bool,
}

/// What a side run does with each answer: the next line to send, or how the run ends. TS turn-pages.ts:10.
#[allow(clippy::large_enum_variant)]
pub enum SideStep {
    Send(String),
    End(TurnResult),
}

/// A side run's handler, handed the thread's answer and each later answer of its own. TS turn-pages.ts:10.
pub type SideRun = Box<dyn FnMut(&RequestId, Option<&Map<String, Value>>) -> SideStep>;

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
    pub turn_line: Option<Box<dyn Fn(&str) -> String>>,
    /// What turn/start waits on once the thread answers.
    pub prompt_after: Option<Settle>,
    /// A re-opened run's turn, written where the run's channel holds no turn.
    pub owed_turn: Option<Box<dyn Fn(&str) -> String>>,
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
pub trait SessionCore {
    fn local_id(&self) -> String;
    fn thread_id(&self) -> String;
    fn command(&self) -> Option<String>;
    fn run(&self) -> Option<String>;
    fn pid(&self) -> Option<u32>;
    fn from(&self) -> Option<u64>;
    fn finished(&self) -> SharedFuture<TurnResult>;
    fn interrupt(&self) -> LocalBoxFuture<'static, ()>;
    fn kept(&self) -> Option<CodexKept>;
    fn steer(&self, prompt: String) -> LocalBoxFuture<'static, SteerOutcome>;
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>;
    fn stop_task(&self, task: String) -> LocalBoxFuture<'static, TaskStop>;
}

/// One Codex turn as the runtime holds it. TS adapter.ts:143 (CodexSession).
#[derive(Clone)]
pub struct CodexSession {
    pub core: Arc<dyn SessionCore>,
}

impl CodexSession {
    /// The registry key: the resume id, or one minted at start.
    pub fn local_id(&self) -> String {
        self.core.local_id()
    }
    /// The id the server gave the thread, equal to the local id until it does.
    pub fn thread_id(&self) -> String {
        self.core.thread_id()
    }
    pub fn command(&self) -> Option<String> {
        self.core.command()
    }
    pub fn run(&self) -> Option<String> {
        self.core.run()
    }
    pub fn pid(&self) -> Option<u32> {
        self.core.pid()
    }
    pub fn from(&self) -> Option<u64> {
        self.core.from()
    }
    pub fn finished(&self) -> SharedFuture<TurnResult> {
        self.core.finished()
    }
    /// TS adapter.ts:1193.
    pub fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.core.interrupt()
    }
    /// TS adapter.ts:1210.
    pub fn kept(&self) -> Option<CodexKept> {
        self.core.kept()
    }
    /// TS adapter.ts:1154.
    pub fn steer(&self, prompt: String) -> LocalBoxFuture<'static, SteerOutcome> {
        self.core.steer(prompt)
    }
    /// TS adapter.ts:1169. `Err` is the write's own failure, the ask left open.
    pub fn answer(&self, ask_id: String, answer: PermissionAnswer) -> LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>> {
        self.core.answer(ask_id, answer)
    }
    /// TS adapter.ts:1186.
    pub fn stop_task(&self, task: String) -> LocalBoxFuture<'static, TaskStop> {
        self.core.stop_task(task)
    }
}

/// The server a finished turn left up for the thread's next turn. TS adapter.ts:1213 (KeptAgent<CodexSession>).
pub struct CodexKept {
    pub next: Arc<dyn Fn(KeptTurn) -> Result<CodexSession, String>>,
    /// `now` closes with no wait.
    pub close: Arc<dyn Fn(bool) -> LocalBoxFuture<'static, ()>>,
    pub exited: SharedFuture<Option<i32>>,
    pub working: Arc<dyn Fn() -> bool>,
    pub session_file: SessionFile,
}

/// The turns a registry holds by local id; side questions and side runs never join it. TS adapter.ts:514.
/// Held weakly: a turn holds this map, so a strong one would keep every finished turn for as long as the adapter.
pub type Sessions = Arc<Mutex<indexmap::IndexMap<String, std::sync::Weak<dyn SessionCore>>>>;

/// Registers a session under its id, in place of an earlier one, and forgets the ones nothing holds any more.
pub(crate) fn register(sessions: &Sessions, id: String, session: &CodexSession) {
    let mut held = sessions.lock().unwrap();
    held.retain(|_, weak| weak.strong_count() > 0);
    held.insert(id, Arc::downgrade(&session.core));
}

/// The sessions something still holds, in the order they were registered.
pub(crate) fn live(sessions: &Sessions) -> indexmap::IndexMap<String, CodexSession> {
    sessions.lock().unwrap().iter().filter_map(|(id, weak)| Some((id.clone(), CodexSession { core: weak.upgrade()? }))).collect()
}

/// Starts reading one turn off its stream and answers its session. TS adapter.ts:522.
pub fn follow(ctx: &TurnContext, sessions: &Sessions, o: FollowOptions) -> CodexSession {
    let (finish_tx, finish_rx) = oneshot::channel::<TurnResult>();
    let finished: SharedFuture<TurnResult> =
        async move { finish_rx.await.unwrap_or_else(|_| bare(TurnStatus::Failed)) }.boxed_local().shared();
    let (open_tx, open_rx) = oneshot::channel::<()>();
    let steer_open: SharedFuture<()> = open_rx.map(|_| ()).boxed_local().shared();
    let prompts_later = o.turn_line.is_some() || o.owed_turn.is_some();
    let opened = o.opened.clone();
    let turn = Arc::new(Turn {
        ctx: ctx.clone(),
        sessions: sessions.clone(),
        stream: o.stream,
        local_id: o.local_id.clone(),
        started_at: o.started_at,
        command: o.command,
        model: o.model,
        cwd: o.cwd,
        prompts_later,
        turn_line: o.turn_line,
        owed_turn: o.owed_turn,
        prompt_after: o.prompt_after.map(|settle| settle.shared()),
        aside: o.aside,
        side: o.side_run.is_some(),
        side_run: RefCell::new(o.side_run),
        keeper: o.keeper,
        background: o.background,
        from: o.from,
        opened: opened.is_some(),
        thread_options: o.thread_options,
        on_event: o.on_event,
        finished: finished.clone(),
        steer_open,
        st: RefCell::new(State { thread_id: o.local_id.clone(), open_steer: Some(open_tx), ..State::default() }),
    });
    if let Some(opened) = opened {
        let mut out = Vec::new();
        {
            let mut st = turn.st.borrow_mut();
            st.legacy = opened.legacy;
            turn.announce(&mut st, Some(opened.thread_id), None, None, &mut out);
        }
        turn.emit_all(out);
        if let Some(line) = turn.turn_line.as_ref().map(|line| line(&turn.thread_id())) {
            turn.hand_over(line);
        }
    }
    if turn.aside {
        let me = turn.clone();
        let wall = ctx.aside_wall_ms;
        let timer = tokio::task::spawn_local(async move {
            tokio::time::sleep(Duration::from_millis(wall)).await;
            me.st.borrow_mut().words = Some(FailureWords { line: aside_wall_line(wall), cause: None });
            end_run(me.stream.as_ref(), me.ctx.grace_ms).await;
        });
        turn.st.borrow_mut().aside_wall = Some(timer);
    }
    let reader = turn.clone();
    tokio::task::spawn_local(async move {
        let result = reader.read_all().await;
        let _ = finish_tx.send(result);
    });
    let session = CodexSession { core: Arc::new(turn.clone()) };
    if !turn.aside && !turn.side {
        register(sessions, o.local_id, &session);
    }
    session
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
#[allow(clippy::too_many_arguments)]
pub fn next_on(
    ctx: &TurnContext,
    sessions: &Sessions,
    keeper: Arc<dyn KeptRun>,
    local_id: &str,
    thread: KeptThread,
    turn: KeptTurn,
    thread_options: Option<ThreadOptions>,
    background: Option<Arc<Mutex<Background>>>,
) -> Result<CodexSession, String> {
    let images = turn.images.iter().map(image_path_of).collect::<Result<Vec<String>, String>>()?;
    let (stream, from) = keeper.turn();
    // A thread let go is opened again before its turn/start, which 0.162.1 answers only in that order.
    if let Some(options) = &thread_options {
        fire(stream.write(thread_resume_line(options, &thread.thread_id)));
    }
    let prompt = turn.prompt;
    let turn_line: Box<dyn Fn(&str) -> String> = Box::new(move |thread_id: &str| turn_start_line(thread_id, &prompt, &images, None));
    Ok(follow(
        ctx,
        sessions,
        FollowOptions {
            stream,
            local_id: local_id.to_string(),
            started_at: (ctx.now)(),
            command: None,
            model: thread.model,
            cwd: thread.cwd,
            turn_line: Some(turn_line),
            prompt_after: turn.after,
            owed_turn: None,
            aside: false,
            side_run: None,
            keeper: Some(keeper),
            background,
            from,
            opened: Some(OpenedThread { thread_id: thread.thread_id, legacy: thread.legacy }),
            thread_options,
            on_event: turn.on_event,
        },
    ))
}

/// An image reaches the server as a file, so one that arrived with no path never travelled the runtime's file road.
pub(super) fn image_path_of(image: &TurnImage) -> Result<String, String> {
    match &image.path {
        None => Err("codex reads images off the machine's disk; this one has no path on it".to_string()),
        Some(path) => image_path(path),
    }
}

/// The turn's state. TS adapter.ts:522-660 (`follow`'s closed-over lets).
#[derive(Default)]
pub(super) struct State {
    pub(super) thread_id: String,
    pub(super) announced: bool,
    pub(super) legacy: bool,
    pub(super) turn_id: Option<String>,
    pub(super) open_steer: Option<oneshot::Sender<()>>,
    pub(super) exited: bool,
    pub(super) interrupt_requested: bool,
    pub(super) turn_result: Option<TurnResult>,
    pub(super) last_text: Option<String>,
    pub(super) usage: Option<UsageSeen>,
    pub(super) compaction: Option<Compaction>,
    pub(super) rate_limits: Option<Map<String, Value>>,
    pub(super) account: LimitAccount,
    pub(super) model_used: Option<String>,
    pub(super) cwd_used: Option<String>,
    pub(super) words: Option<FailureWords>,
    pub(super) last_error: Option<String>,
    pub(super) usage_limited: bool,
    pub(super) stall: Option<tokio::task::JoinHandle<()>>,
    pub(super) aside_wall: Option<tokio::task::JoinHandle<()>>,
    pub(super) stderr_tail: Vec<String>,
    pub(super) early: Vec<String>,
    pub(super) commands: IndexSet<String>,
    pub(super) changes_by_id: IndexMap<String, Value>,
    pub(super) pending: IndexMap<String, Pending>,
    pub(super) awaiting: IndexMap<String, oneshot::Sender<Reply>>,
    pub(super) request_seq: u64,
    pub(super) children: Children,
    pub(super) lead_end: Option<LeadEnd>,
}

/// An approval the server waits on: the id it asked under and the ask the runtime holds.
#[derive(Clone)]
pub(super) struct Pending {
    pub(super) id: RequestId,
}

/// How the lead's own turn ended, held while a subagent still runs. TS adapter.ts:649.
#[derive(Clone)]
pub(super) struct LeadEnd {
    pub(super) status: String,
    pub(super) error: Option<String>,
}

/// What a request a caller waits on was answered with. TS adapter.ts:626's `settle`.
pub(super) enum Reply {
    Answered { error: Option<String> },
    Gone,
}

/// The stream reader and everything the turn holds. adapter.ts:522-1227.
pub(super) struct Turn {
    pub(super) ctx: TurnContext,
    pub(super) sessions: Sessions,
    pub(super) stream: Arc<dyn ExecStream>,
    pub(super) local_id: String,
    pub(super) started_at: i64,
    pub(super) command: Option<String>,
    pub(super) model: Option<String>,
    pub(super) cwd: Option<String>,
    pub(super) prompts_later: bool,
    pub(super) turn_line: Option<Box<dyn Fn(&str) -> String>>,
    pub(super) owed_turn: Option<Box<dyn Fn(&str) -> String>>,
    pub(super) prompt_after: Option<Shared<LocalBoxFuture<'static, Result<(), String>>>>,
    pub(super) aside: bool,
    pub(super) side: bool,
    pub(super) side_run: RefCell<Option<SideRun>>,
    pub(super) keeper: Option<Arc<dyn KeptRun>>,
    pub(super) background: Option<Arc<Mutex<Background>>>,
    pub(super) from: Option<u64>,
    pub(super) opened: bool,
    pub(super) thread_options: Option<ThreadOptions>,
    pub(super) on_event: OnEvent,
    pub(super) finished: SharedFuture<TurnResult>,
    pub(super) steer_open: SharedFuture<()>,
    pub(super) st: RefCell<State>,
}

/// A result with only its status. TS adapter.ts:162.
pub(super) fn bare(status: TurnStatus) -> TurnResult {
    TurnResult {
        status,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: None,
        unreached: None,
        refusal: None,
        limit: None,
    }
}

/// A failed result with its words and no duration. TS adapter.ts:1088 (the request errors).
pub(super) fn plain_failure(error: String) -> TurnResult {
    TurnResult { error: Some(error), ..bare(TurnStatus::Failed) }
}

/// Polls `future` once, as the TS runs an async call up to its first await: its value where it is already settled,
/// else the future still to be awaited.
pub(super) fn settle_now<T>(mut future: LocalBoxFuture<'static, T>) -> Result<T, LocalBoxFuture<'static, T>> {
    match future.as_mut().poll(&mut Context::from_waker(noop_waker_ref())) {
        Poll::Ready(value) => Ok(value),
        Poll::Pending => Err(future),
    }
}

/// A future a caller may poll again after it settled, as a TS promise may be awaited again; each poll reads the one result.
pub(super) fn repeatable<T: Clone + 'static>(future: LocalBoxFuture<'static, T>) -> LocalBoxFuture<'static, T> {
    let mut pending = future.shared();
    let mut settled: Option<T> = None;
    futures::future::poll_fn(move |cx| {
        if let Some(value) = &settled {
            return Poll::Ready(value.clone());
        }
        match pending.poll_unpin(cx) {
            Poll::Ready(value) => {
                settled = Some(value.clone());
                Poll::Ready(value)
            }
            Poll::Pending => Poll::Pending,
        }
    })
    .boxed_local()
}

/// A future that is already settled with `value`.
pub(super) fn ready<T: 'static>(value: T) -> LocalBoxFuture<'static, T> {
    async move { value }.boxed_local()
}

/// The turn is open to a message: the server started it and it has neither completed nor gone. TS adapter.ts:1158.
pub(super) fn running(st: &State) -> bool {
    st.turn_id.is_some() && st.lead_end.is_none() && st.turn_result.is_none() && !st.exited && !st.interrupt_requested
}

/// Opens steering for the turn's messages once, as the first turn/started does. TS adapter.ts:968.
pub(super) fn open_steer(st: &mut State) {
    if let Some(open) = st.open_steer.take() {
        let _ = open.send(());
    }
}

/// The thread id when a message names another thread on this server, once the thread is announced. TS adapter.ts:632.
pub(super) fn foreign_thread(st: &State, params: &Map<String, Value>) -> Option<String> {
    let id = str(params.get("threadId"))?;
    (st.announced && id != st.thread_id).then(|| id.to_string())
}

/// A note the server printed: kept until the thread is announced. TS adapter.ts:614.
pub(super) fn note(st: &mut State, text: String, out: &mut Vec<AdapterEvent>) {
    if st.announced {
        out.push(AdapterEvent::TurnDelta(TurnDelta::new(st.thread_id.clone(), DeltaKind::Note, text)));
    } else {
        st.early.push(text);
    }
}

/// Removes the ask under `ask_id` and says it closed. TS adapter.ts:605.
pub(super) fn close_ask(st: &mut State, ask_id: &str, outcome: PermissionOutcome, option_id: Option<String>) -> AdapterEvent {
    st.pending.shift_remove(ask_id);
    AdapterEvent::PermissionClose { session_id: st.thread_id.clone(), ask_id: ask_id.to_string(), outcome, option_id }
}

/// Says the compaction, if one is open, and clears it. TS adapter.ts:570.
pub(super) fn say_compacted(st: &mut State, out: &mut Vec<AdapterEvent>) {
    if let Some(c) = st.compaction.take() {
        out.push(AdapterEvent::TurnCompacted { session_id: st.thread_id.clone(), before: c.before, after: c.after });
    }
}

/// The limit event for the latest reading, where one exists. TS adapter.ts:1000.
pub(super) fn limit_event(st: &State, credits: Option<wsp_wire::types::usage::BankedResets>) -> Option<AdapterEvent> {
    let rates = st.rate_limits.as_ref()?;
    let limit = limit_of(rates, &st.account, credits)?;
    Some(AdapterEvent::Limit { session_id: st.thread_id.clone(), limit })
}

/// Ends the process after `ms` of the lead's reply with it still up: a run ended by the wall or a stop. TS adapter.ts:1204.
pub(super) fn end_now(stream: Arc<dyn ExecStream>, grace_ms: u64) -> LocalBoxFuture<'static, ()> {
    async move { end_run(stream.as_ref(), grace_ms).await }.boxed_local()
}

impl SessionCore for Arc<Turn> {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn thread_id(&self) -> String {
        self.st.borrow().thread_id.clone()
    }
    fn command(&self) -> Option<String> {
        self.command.clone()
    }
    fn run(&self) -> Option<String> {
        self.stream.run()
    }
    fn pid(&self) -> Option<u32> {
        self.stream.pid()
    }
    fn from(&self) -> Option<u64> {
        self.from
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        repeatable(Turn::interrupt(self))
    }
    fn kept(&self) -> Option<CodexKept> {
        Turn::kept(self)
    }
    fn steer(&self, prompt: String) -> LocalBoxFuture<'static, SteerOutcome> {
        repeatable(Turn::steer(self, prompt))
    }
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>> {
        repeatable(Turn::answer(self, ask_id, answer))
    }
    fn stop_task(&self, task: String) -> LocalBoxFuture<'static, TaskStop> {
        repeatable(Turn::stop_task(self, task))
    }
}

impl Turn {
    pub(super) fn now(&self) -> i64 {
        (self.ctx.now)()
    }

    pub(super) fn emit_all(&self, events: Vec<AdapterEvent>) {
        for event in events {
            (self.on_event)(event);
        }
    }

    /// Whether the turn was stopped or its process has gone: no prompt is due any more. TS adapter.ts:886.
    fn halted(&self) -> bool {
        let st = self.st.borrow();
        st.interrupt_requested || st.exited
    }

    /// The thread is announced once: its session starts, and the notes said before it follow. TS adapter.ts:575.
    pub(super) fn announce(
        &self,
        st: &mut State,
        id: Option<String>,
        model: Option<String>,
        cwd: Option<String>,
        out: &mut Vec<AdapterEvent>,
    ) {
        if st.announced {
            return;
        }
        st.announced = true;
        if let Some(id) = id {
            st.thread_id = id;
        }
        let runs = self.model.clone().or(model);
        st.model_used = runs.clone();
        let folder = self.cwd.clone().or(cwd);
        st.cwd_used = folder.clone();
        let prompted = (self.prompts_later && st.turn_id.is_none()).then_some(false);
        out.push(AdapterEvent::SessionStart {
            session_id: st.thread_id.clone(),
            model: runs,
            cwd: folder,
            tools: None,
            harness: None,
            prompted,
        });
        for text in std::mem::take(&mut st.early) {
            out.push(AdapterEvent::TurnDelta(TurnDelta::new(st.thread_id.clone(), DeltaKind::Note, text)));
        }
    }

    /// Clears the reconnect stall, if one is armed. TS adapter.ts:620.
    pub(super) fn progress(st: &mut State) {
        if let Some(timer) = st.stall.take() {
            timer.abort();
        }
    }

    /// Arms the reconnect stall: a run of Reconnecting that outlives it ends the turn in words. TS adapter.ts:621.
    pub(super) fn reconnecting(self: &Arc<Self>, st: &mut State) {
        if st.stall.is_some() {
            return;
        }
        let stalled_at = self.now();
        let me = self.clone();
        st.stall = Some(tokio::task::spawn_local(async move {
            tokio::time::sleep(Duration::from_millis(me.ctx.reconnect_stall_ms)).await;
            let elapsed = me.now() - stalled_at;
            me.st.borrow_mut().words = Some(FailureWords { line: codex_reconnect_line(elapsed as f64), cause: None });
            end_run(me.stream.as_ref(), me.ctx.grace_ms).await;
        }));
    }

    /// The turn's end with its limit where the plan stopped it. TS adapter.ts:748.
    pub(super) fn with_limit(&self, st: &State, result: TurnResult) -> TurnResult {
        if result.status != TurnStatus::Failed || !st.usage_limited {
            return result;
        }
        let rates = st.rate_limits.clone().unwrap_or_default();
        let windows = limit_of(&rates, &st.account, None).map(|l| l.windows).unwrap_or_default();
        let resets = stopped_until(&windows, self.now() as f64);
        TurnResult { limit: Some(wsp_wire::types::usage::TurnLimit { resets_at: resets.map(JsNumber) }), ..result }
    }

    /// The failure a turn ends with, its words where wsp names them. TS adapter.ts:698.
    pub(super) fn failed(&self, st: &mut State, message: &str) -> TurnResult {
        if let Some(words) = failure_words(message, &self.ctx.login, self.ctx.key_env.as_deref()) {
            st.words = Some(words);
        }
        let words = st.words.clone();
        TurnResult {
            duration_ms: Some(JsNumber((self.now() - self.started_at) as f64)),
            error: Some(words.as_ref().map_or(message.to_string(), |w| w.line.clone())),
            refusal: words.and_then(|w| w.cause),
            ..bare(TurnStatus::Failed)
        }
    }

    /// The lead's turn as it ended, with every subagent call it waited on counted in its tokens. TS adapter.ts:705.
    pub(super) fn lead_result(&self, st: &mut State, end: &LeadEnd) -> TurnResult {
        if end.status == "interrupted" {
            return bare(TurnStatus::Interrupted);
        }
        if end.status != "completed" {
            return self.failed(st, end.error.as_deref().unwrap_or("codex reported a failed turn"));
        }
        let tokens = super::items::with_child_tokens(st.usage.as_ref().map(super::items::turn_tokens_of), &st.children.child_tokens);
        TurnResult {
            duration_ms: Some(JsNumber((self.now() - self.started_at) as f64)),
            text: st.last_text.clone(),
            tokens,
            model: st.model_used.clone(),
            ..bare(TurnStatus::Completed)
        }
    }

    /// The lead's reply goes once its own turn has ended and no subagent of it still runs. TS adapter.ts:730.
    pub(super) fn settle_lead(self: &Arc<Self>) {
        let ended = {
            let mut st = self.st.borrow_mut();
            let end = st.lead_end.clone();
            match end {
                Some(end) if st.children.running() == 0 && !st.exited => Some(self.lead_result(&mut st, &end)),
                _ => None,
            }
        };
        if let Some(result) = ended {
            self.finish(result);
        }
    }

    /// The turn's reply, once: its end is said, its asks close, and the process is let go. TS adapter.ts:749.
    pub(super) fn finish(self: &Arc<Self>, ended: TurnResult) {
        let (result, thread, release, unsubscribe) = {
            let mut st = self.st.borrow_mut();
            if st.turn_result.is_some() {
                return;
            }
            let result = self.with_limit(&st, ended);
            st.turn_result = Some(result.clone());
            let thread = st.thread_id.clone();
            let mut release = false;
            let mut unsubscribe = false;
            if result.status != TurnStatus::Completed && result.status != TurnStatus::Interrupted {
                release = true;
            } else if let (Some(keeper), true) = (self.keeper.as_ref(), self.thread_options.is_some()) {
                if keeper.up() {
                    let holding = self.background.as_ref().is_some_and(|bg| !bg.lock().unwrap().commands.is_empty());
                    if let Some(bg) = &self.background {
                        bg.lock().unwrap().held = holding.then(|| thread.clone());
                    }
                    unsubscribe = !holding;
                }
            }
            (result, thread, release, unsubscribe)
        };
        if release {
            if let Some(keeper) = &self.keeper {
                keeper.release();
            }
        }
        if unsubscribe {
            fire(self.stream.write(thread_unsubscribe_line(&thread)));
        }
        self.emit_all(vec![AdapterEvent::TurnDone { session_id: thread, result, held: None }]);
        let asks: Vec<String> = self.st.borrow().pending.keys().cloned().collect();
        for ask in asks {
            let event = close_ask(&mut self.st.borrow_mut(), &ask, PermissionOutcome::Cancelled, None);
            self.emit_all(vec![event]);
        }
        for (_, settle) in self.st.borrow_mut().awaiting.drain(..) {
            let _ = settle.send(Reply::Gone);
        }
        self.stream.close_input();
        let stream = self.stream.clone();
        let (wait, grace) = (self.ctx.result_exit_ms, self.ctx.grace_ms);
        fire(async move { end_after_result(stream.as_ref(), wait, grace).await }.boxed_local());
    }

    /// The reader: every line of the stream, then the exit. TS adapter.ts:1080-1128.
    async fn read_all(self: Arc<Self>) -> TurnResult {
        let mut stream_error = None;
        let mut stream_failure = None;
        let mut lines = self.stream.lines();
        while let Some(item) = lines.next().await {
            match item {
                Ok(raw) => self.read_line(&raw),
                Err(failure) => {
                    stream_error = Some(failure.message.clone());
                    stream_failure = Some(failure);
                    break;
                }
            }
        }
        drop(lines);
        let exit_code = self.stream.exited().await;
        self.exit_road(exit_code, stream_error, stream_failure)
    }

    fn read_line(self: &Arc<Self>, raw: &str) {
        match read_message(raw) {
            None => {
                let text = js_trim(raw);
                if !text.is_empty() {
                    let mut st = self.st.borrow_mut();
                    st.stderr_tail.push(text.to_string());
                    if st.stderr_tail.len() > super::items::STDERR_TAIL_LINES {
                        st.stderr_tail.remove(0);
                    }
                }
            }
            Some(RpcMessage::Notification { method, params, emitted_at_ms }) => self.on_notification(&method, &params, emitted_at_ms),
            Some(RpcMessage::Request { id, method, params }) => self.on_request(id, &method, &params),
            Some(RpcMessage::Response { id, result }) => self.on_response(id, &result),
            Some(RpcMessage::Error { id, message }) => self.on_error(id, &message),
        }
    }

    /// The server asks for something: an approval, or a method wsp does not answer. TS adapter.ts:849.
    fn on_request(self: &Arc<Self>, id: RequestId, method: &str, params: &Map<String, Value>) {
        let Some((_, tool_name)) = APPROVALS.iter().find(|(m, _)| *m == method) else {
            fire(self.stream.write(refuse_request_line(&id, method)));
            return;
        };
        if self.aside || self.side {
            fire(self.stream.write(decision_line(&id, Decision::Decline)));
            return;
        }
        let item_id = str(params.get("itemId")).map(String::from);
        let ask = {
            let mut st = self.st.borrow_mut();
            let input = if *tool_name == "file_change" {
                let changes = item_id.as_ref().and_then(|i| st.changes_by_id.get(i)).cloned();
                let rows: Vec<Value> = changes_of(changes.as_ref()).into_iter().map(|c| json!({"path": c.path, "kind": c.kind})).collect();
                json!({"changes": rows})
            } else {
                let mut input = Map::new();
                input.insert("command".into(), Value::String(shell_script_of(str(params.get("command")).unwrap_or(""))));
                if let Some(cwd) = str(params.get("cwd")) {
                    input.insert("cwd".into(), Value::String(cwd.to_string()));
                }
                Value::Object(input)
            };
            let asker = foreign_thread(&st, params);
            let parent = asker.map(|a| st.children.children.get(&a).map_or_else(|| a.clone(), |c| c.parent.clone()));
            let reason = str(params.get("reason")).filter(|r| !r.is_empty()).map(String::from);
            let ask = PermissionAsk {
                ask_id: id.js_string(),
                tool_name: tool_name.to_string(),
                tool_use_id: item_id,
                parent_tool_use_id: parent,
                input: wsp_wire::text::json_stringify(&input),
                detail: reason,
                options: vec![
                    PermissionOption { id: PERMISSION_ALLOW.into(), label: "Allow".into(), effect: PermissionEffect::Allow, mode: None },
                    PermissionOption { id: PERMISSION_DENY.into(), label: "Deny".into(), effect: PermissionEffect::Deny, mode: None },
                ],
            };
            st.pending.insert(ask.ask_id.clone(), Pending { id });
            ask
        };
        let session_id = self.thread_id();
        self.emit_all(vec![AdapterEvent::PermissionAsk { session_id, ask }]);
    }

    /// A reply to a request this adapter sent. TS adapter.ts:898.
    fn on_response(self: &Arc<Self>, id: RequestId, result: &Value) {
        let answer = rec(Some(result));
        if is(&id, request::THREAD) {
            let mut out = Vec::new();
            let hand = {
                let mut st = self.st.borrow_mut();
                let thread = rec(answer.and_then(|a| a.get("thread")));
                st.legacy |= !st.announced && thread.and_then(|t| str(t.get("historyMode"))) == Some("legacy");
                let model = str(answer.and_then(|a| a.get("model"))).or(str(thread.and_then(|t| t.get("model")))).map(String::from);
                let cwd = str(answer.and_then(|a| a.get("cwd"))).or(str(thread.and_then(|t| t.get("cwd")))).map(String::from);
                let id_text = str(thread.and_then(|t| t.get("id"))).map(String::from);
                self.announce(&mut st, id_text, model, cwd, &mut out);
                !self.side && !self.opened && self.turn_line.is_some()
            };
            self.emit_all(out);
            if self.side {
                self.side_step(&id, answer);
            } else if hand {
                if let Some(line) = self.turn_line.as_ref().map(|line| line(&self.thread_id())) {
                    self.hand_over(line);
                }
            } else if let Some(owed) = self.owed_turn.as_ref() {
                let taken = self.stream.taken();
                if taken.as_ref().is_some_and(|lines| !lines.iter().any(|l| is_turn_start(l))) {
                    fire(self.stream.write(owed(&self.thread_id())));
                }
            }
            return;
        }
        if self.side && (is(&id, request::TURNS) || is(&id, request::REVERT)) {
            self.side_step(&id, rec(Some(result)));
            return;
        }
        if is(&id, request::ACCOUNT) {
            let sign_in = rec(answer.and_then(|a| a.get("account")));
            let mut out = Vec::new();
            {
                let mut st = self.st.borrow_mut();
                let session_id = st.thread_id.clone();
                if str(sign_in.and_then(|s| s.get("type"))) == Some("apiKey") {
                    out.push(AdapterEvent::Limit {
                        session_id,
                        limit: wsp_wire::types::usage::HarnessLimit {
                            windows: Vec::new(),
                            plan: None,
                            status: None,
                            account: None,
                            keyed: Some(true),
                            credits: None,
                        },
                    });
                }
                let email = str(sign_in.and_then(|s| s.get("email"))).map(String::from);
                let plan = str(sign_in.and_then(|s| s.get("planType"))).map(String::from);
                if let Some(label) = email {
                    st.account.label = Some(label);
                }
                if let Some(plan) = plan {
                    st.account.plan = Some(plan);
                }
                out.extend(limit_event(&st, None));
            }
            self.emit_all(out);
            return;
        }
        if is(&id, request::RATE_LIMITS) {
            let mut out = Vec::new();
            {
                let mut st = self.st.borrow_mut();
                if let Some(account_id) = str(answer.and_then(|a| a.get("accountId"))).map(String::from) {
                    st.account.id = Some(account_id);
                }
                st.rate_limits = snapshot_of(answer);
                out.extend(limit_event(&st, credits_of(answer)));
            }
            self.emit_all(out);
            return;
        }
        if let RequestId::Str(key) = &id {
            let settle = self.st.borrow_mut().awaiting.shift_remove(key);
            if let Some(settle) = settle {
                let _ = settle.send(Reply::Answered { error: None });
            }
        }
    }

    /// A side run's next step: a line to send, or the run's end. TS adapter.ts:920.
    fn side_step(self: &Arc<Self>, id: &RequestId, answer: Option<&Map<String, Value>>) {
        let step = {
            let mut slot = self.side_run.borrow_mut();
            match slot.as_mut() {
                Some(run) => run(id, answer),
                None => return,
            }
        };
        match step {
            SideStep::Send(line) => fire(self.stream.write(line)),
            SideStep::End(result) => self.finish(result),
        }
    }

    /// An error the server answers a request with. TS adapter.ts:938.
    fn on_error(self: &Arc<Self>, id: RequestId, message: &str) {
        if let RequestId::Str(key) = &id {
            let settle = self.st.borrow_mut().awaiting.shift_remove(key);
            if let Some(settle) = settle {
                let _ = settle.send(Reply::Answered { error: Some(message.to_string()) });
                return;
            }
        }
        let ended = if is(&id, request::REVERT) {
            plain_failure(format!("codex would not cut the thread: {message}"))
        } else if is(&id, request::TURNS) {
            plain_failure(format!("codex could not list the thread's turns: {message}"))
        } else if is(&id, request::THREAD) {
            let error = if message.contains(WRITER_HELD) {
                conversation_written_elsewhere_line("Codex", Some(LETS_GO))
            } else {
                format!("codex could not open the thread: {message}")
            };
            plain_failure(error)
        } else if is(&id, request::TURN) || is(&id, request::INITIALIZE) {
            plain_failure(format!("codex could not start the turn: {message}"))
        } else {
            return;
        };
        self.finish(ended);
    }

    /// The server's end of the turn: its process closed. TS adapter.ts:1107-1160.
    fn exit_road(
        self: &Arc<Self>,
        exit_code: Option<i32>,
        stream_error: Option<String>,
        stream_failure: Option<ExecFailure>,
    ) -> TurnResult {
        let mut out = Vec::new();
        let (result, saw_result, thread) = {
            let mut st = self.st.borrow_mut();
            if let Some(timer) = st.aside_wall.take() {
                timer.abort();
            }
            st.exited = true;
            Turn::progress(&mut st);
            let thread = st.thread_id.clone();
            let asks: Vec<String> = st.pending.keys().cloned().collect();
            for ask in asks {
                out.push(close_ask(&mut st, &ask, PermissionOutcome::Cancelled, None));
            }
            for (_, settle) in st.awaiting.drain(..) {
                let _ = settle.send(Reply::Gone);
            }
            let ended = if st.interrupt_requested { SubagentState::Stopped } else { SubagentState::Failed };
            let running: Vec<String> =
                st.children.children.iter().filter(|(_, c)| c.state == SubagentState::Running).map(|(k, _)| k.clone()).collect();
            for task in running {
                st.children.end_child(&thread, &task, ended, &mut out);
            }
            if st.turn_result.is_none() {
                if let Some(end) = st.lead_end.clone() {
                    let result = self.lead_result(&mut st, &end);
                    let result = self.with_limit(&st, result);
                    st.turn_result = Some(result.clone());
                    out.push(AdapterEvent::TurnDone { session_id: thread.clone(), result, held: None });
                }
            }
            let saw_result = st.turn_result.is_some();
            if st.turn_result.is_none() {
                let tail = if st.stderr_tail.is_empty() { None } else { Some(st.stderr_tail.join("\n")) };
                let reason = st.last_error.clone().or(tail);
                let code = exit_code.map_or_else(|| "null".to_string(), |c| c.to_string());
                let died =
                    format!("codex exited with code {code} before its turn ended{}", reason.map_or_else(String::new, |r| format!(": {r}")));
                let result = if st.interrupt_requested {
                    bare(TurnStatus::Interrupted)
                } else {
                    let raw = match st.words.clone() {
                        Some(words) => TurnResult { error: Some(words.line), refusal: words.cause, ..bare(TurnStatus::Failed) },
                        None => plain_failure(stream_error.clone().unwrap_or(died)),
                    };
                    self.with_limit(&st, raw)
                };
                st.turn_result = Some(result.clone());
                out.push(AdapterEvent::TurnDone { session_id: thread.clone(), result, held: None });
            }
            let result = st.turn_result.clone().expect("the turn has a result");
            (result, saw_result, thread)
        };
        out.push(AdapterEvent::SessionEnd { session_id: thread, exit_code, saw_result, failure: stream_failure });
        self.emit_all(out);
        result
    }

    /// Writes the turn line once its prompt is due, unless the turn was stopped or its process went first. TS adapter.ts:884.
    pub(super) fn hand_over(self: &Arc<Self>, line: String) {
        let me = self.clone();
        let gate = self.prompt_after.clone();
        let due = async move {
            if let Some(gate) = gate {
                gate.await?;
            }
            if me.halted() {
                return Err("the turn was stopped before its prompt was due".to_string());
            }
            Ok(())
        }
        .boxed_local()
        .shared();
        if self.prompt_after.is_some() && self.stream.write_after(line.clone(), due.clone().boxed_local()).is_some() {
            return;
        }
        let stream = self.stream.clone();
        tokio::task::spawn_local(async move {
            if due.await.is_ok() {
                let _ = stream.write(line).await;
            }
        });
    }

    /// The steer's request: sent once a turn runs, its answer read off the awaited request. TS adapter.ts:1154.
    fn steer_now(self: &Arc<Self>, prompt: String) -> LocalBoxFuture<'static, SteerOutcome> {
        let (id, line, answered) = {
            let mut st = self.st.borrow_mut();
            let Some(turn) = st.turn_id.clone().filter(|_| running(&st)) else { return ready(SteerOutcome::NotRunning) };
            st.request_seq += 1;
            let id = format!("wsp-steer-{}", st.request_seq);
            let (tx, rx) = oneshot::channel();
            st.awaiting.insert(id.clone(), tx);
            let line = turn_steer_line(&id, &st.thread_id, &turn, &prompt);
            (id, line, rx)
        };
        let wrote = started(self.stream.write(line));
        let me = self.clone();
        async move {
            let outcome = match wrote.await {
                Ok(WriteOutcome::Written) => None,
                Ok(WriteOutcome::Gone) => Some(SteerOutcome::NotRunning),
                Err(_) if me.running() => None,
                Err(_) => Some(SteerOutcome::NotRunning),
            };
            if let Some(outcome) = outcome {
                me.st.borrow_mut().awaiting.shift_remove(&id);
                return outcome;
            }
            match answered.await {
                Ok(Reply::Answered { error: None }) => SteerOutcome::Accepted,
                _ => SteerOutcome::NotRunning,
            }
        }
        .boxed_local()
    }

    fn running(&self) -> bool {
        running(&self.st.borrow())
    }

    /// turn/steer on the running turn. TS adapter.ts:1154.
    fn steer(self: &Arc<Self>, prompt: String) -> LocalBoxFuture<'static, SteerOutcome> {
        if self.st.borrow().turn_id.is_some() {
            return self.steer_now(prompt);
        }
        let me = self.clone();
        let open = self.steer_open.clone();
        let finished = self.finished.clone();
        let join = self.ctx.steer_join_ms;
        async move {
            let either = async move {
                futures::future::select(open, finished).await;
            }
            .boxed_local();
            settles_within(either, join).await;
            me.steer_now(prompt).await
        }
        .boxed_local()
    }

    /// Answers an approval the server asked for. TS adapter.ts:1169.
    fn answer(self: &Arc<Self>, ask_id: String, answer: PermissionAnswer) -> LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>> {
        let open = {
            let mut st = self.st.borrow_mut();
            let Some(open) = st.pending.get(&ask_id).cloned() else { return ready(Ok(AnswerOutcome::Gone)) };
            if st.exited {
                return ready(Ok(AnswerOutcome::Gone));
            }
            st.pending.shift_remove(&ask_id);
            open
        };
        let decision = if answer.option_id == PERMISSION_ALLOW { Decision::Accept } else { Decision::Decline };
        let write = self.stream.write(decision_line(&open.id, decision));
        let me = self.clone();
        let land = move |wrote: Result<WriteOutcome, ExecFailure>| me.land_answer(ask_id, open, answer, wrote);
        match settle_now(write) {
            Ok(wrote) => ready(land(wrote)),
            Err(pending) => started(async move { land(pending.await) }.boxed_local()),
        }
    }

    /// The answer's write has landed or failed: its close is said here, so a callback that answers inside `on_event`
    /// sees it before the reader's next line, as the TS does. TS adapter.ts:1177-1184.
    fn land_answer(
        self: &Arc<Self>,
        ask_id: String,
        open: Pending,
        answer: PermissionAnswer,
        wrote: Result<WriteOutcome, ExecFailure>,
    ) -> Result<AnswerOutcome, ExecFailure> {
        match wrote {
            Ok(WriteOutcome::Written) => {}
            Ok(WriteOutcome::Gone) => return Ok(AnswerOutcome::Gone),
            Err(e) => {
                if self.st.borrow().exited {
                    return Ok(AnswerOutcome::Gone);
                }
                self.st.borrow_mut().pending.insert(ask_id, open);
                return Err(e);
            }
        }
        let session_id = self.st.borrow().thread_id.clone();
        let denied = answer.outcome == PermissionOutcome::Denied;
        self.emit_all(vec![AdapterEvent::PermissionClose {
            session_id,
            ask_id,
            outcome: answer.outcome,
            option_id: Some(answer.option_id.clone()),
        }]);
        if let (true, Some(reason)) = (denied, answer.reason) {
            fire(self.steer(reason));
        }
        Ok(AnswerOutcome::Answered)
    }

    /// Asks the server to stop one turn and settles on its answer. TS adapter.ts:1103 (`interruptTurn`).
    fn interrupt_turn(self: &Arc<Self>, thread: String, turn: String) -> LocalBoxFuture<'static, Reply> {
        let (id, answered) = {
            let mut st = self.st.borrow_mut();
            st.request_seq += 1;
            let id = format!("wsp-interrupt-{}", st.request_seq);
            let (tx, rx) = oneshot::channel();
            st.awaiting.insert(id.clone(), tx);
            (id, rx)
        };
        let wrote = started(self.stream.write(turn_interrupt_line(&id, &thread, &turn)));
        let me = self.clone();
        async move {
            match wrote.await {
                Ok(WriteOutcome::Written) => answered.await.unwrap_or(Reply::Gone),
                _ => {
                    me.st.borrow_mut().awaiting.shift_remove(&id);
                    Reply::Gone
                }
            }
        }
        .boxed_local()
    }

    /// turn/interrupt on the running turn and its subagents. TS adapter.ts:1193.
    fn interrupt(self: &Arc<Self>) -> LocalBoxFuture<'static, ()> {
        let (live, kids, commands) = {
            let mut st = self.st.borrow_mut();
            if st.exited {
                return ready(());
            }
            let live = if running(&st) { st.turn_id.clone() } else { None };
            st.interrupt_requested = true;
            open_steer(&mut st);
            (live, st.children.running_turns(), st.commands.len())
        };
        if live.is_none() && kids.is_empty() {
            return started(end_now(self.stream.clone(), self.ctx.grace_ms));
        }
        if commands > 0 {
            if let Some(keeper) = &self.keeper {
                keeper.release();
            }
        }
        for (task, turn) in kids {
            fire(self.interrupt_turn(task, turn));
        }
        if let Some(turn) = live {
            let thread = self.thread_id();
            fire(self.interrupt_turn(thread, turn));
        }
        let stream = self.stream.clone();
        let grace = self.ctx.grace_ms;
        started(async move { end_after_result(stream.as_ref(), grace, grace).await }.boxed_local())
    }

    /// Interrupts one subagent's turn. TS adapter.ts:1186.
    fn stop_task(self: &Arc<Self>, task: String) -> LocalBoxFuture<'static, TaskStop> {
        let target = {
            let st = self.st.borrow();
            let turn = st.children.children.get(&task).filter(|c| c.state == SubagentState::Running).and_then(|c| c.turn_id.clone());
            match turn {
                Some(turn) if !st.exited && st.turn_result.is_none() => Some(turn),
                _ => None,
            }
        };
        let Some(turn) = target else { return ready(TaskStop::NotRunning) };
        let answered = self.interrupt_turn(task, turn);
        async move {
            match answered.await {
                Reply::Gone => TaskStop::NotRunning,
                Reply::Answered { error: None } => TaskStop::Accepted,
                Reply::Answered { error: Some(error) } => TaskStop::Refused { error },
            }
        }
        .boxed_local()
    }

    /// The server still up for the thread's next turn, where this turn was kept. TS adapter.ts:1210.
    fn kept(self: &Arc<Self>) -> Option<CodexKept> {
        let keeper = self.keeper.clone()?;
        let st = self.st.borrow();
        if !keeper.up() || st.turn_result.is_none() {
            return None;
        }
        let thread =
            KeptThread { thread_id: st.thread_id.clone(), legacy: st.legacy, model: st.model_used.clone(), cwd: st.cwd_used.clone() };
        let ctx = self.ctx.clone();
        let sessions = self.sessions.clone();
        let thread_options = self.thread_options.clone();
        let background = self.background.clone();
        let next_keeper = keeper.clone();
        let next_thread = thread.clone();
        let next_background = background.clone();
        let next = Arc::new(move |turn: KeptTurn| {
            let local = next_thread.thread_id.clone();
            next_on(
                &ctx,
                &sessions,
                next_keeper.clone(),
                &local,
                next_thread.clone(),
                turn,
                thread_options.clone(),
                next_background.clone(),
            )
        });
        let close_keeper = keeper.clone();
        let (wait, grace) = (self.ctx.result_exit_ms, self.ctx.grace_ms);
        let close = Arc::new(move |now: bool| close_keeper.close(if now { 0 } else { wait }, grace));
        let working_background = background;
        Some(CodexKept {
            next,
            close,
            exited: keeper.exited(),
            working: Arc::new(move || working_background.as_ref().is_some_and(|bg| !bg.lock().unwrap().commands.is_empty())),
            session_file: rollout_in(&self.ctx.home, &thread.thread_id),
        })
    }
}

/// Whether a request id is the named one. TS adapter.ts:903 (`id === REQUEST.x`).
fn is(id: &RequestId, name: &str) -> bool {
    matches!(id, RequestId::Str(s) if s == name)
}
