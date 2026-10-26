// SPDX-License-Identifier: AGPL-3.0-only
//! One turn of Claude Code: the reader of its stream (`follow`) and what a caller does to a running turn (steer,
//! answer, set_access, stop_task, interrupt, kept). TS: packages/adapter-claude/src/adapter.ts:643-1270 at a3b870a6e.
//!
//! Locks: the turn's state sits behind one RefCell. It is never borrowed across an `on_event` call or a stream call,
//! so a callback may call any method of the session from inside `on_event`.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::channel::oneshot;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use tokio::task::JoinHandle;

use crate::claude::adapter::AdapterCore;
use crate::claude::fields::{num, parse_line, rec, string, Obj};
use crate::claude::handback::{end_answer, interim_end, later_end, note_line, task_ended, HandbackBook};
use crate::claude::landmines::{new_session_id, user_message_line};
use crate::claude::limits::{note_rejected, with_limit, Rejected};
use crate::claude::normalize::{
    answered_nothing, api_error_cause, background_tasks_of, compacted_to, drained_notice, models_of, no_output_error, normalize_event,
    own_use, parse_input, saved_use_of, session_file_in, subagent_launch, subagent_prompts, subagent_started, task_finished_of, with_calls,
    with_context, ApiErrorCause, ModelUse, SavedUse, STDERR_TAIL_LINES, STOP_TASK_UNSUPPORTED,
};
use crate::claude::permissions::{
    control_allow_line, control_error_line, control_line, interrupt_line, mode_option_on, set_mode_line, stop_task_line, ControlLine,
    SKIP_PROMPTS_MODE,
};
use crate::claude::plans::PlanBook;
use crate::claude::shell_cwd::shell_cwd_after;
use crate::claude::steers::Steers;
use crate::claude::subagent_spend::{note_child_call, note_progress, subagent_spent, subagent_spent_at_end, SpendBook};
use crate::claude::tokens::{held_tokens, input_tokens, plus_tokens, usage_of, usage_tokens};
use crate::pending::{
    background_tasks_line, fmt_duration, harness_exit_line, subagent_asked, task_finished_line, HarnessEnded, Refusal, QUESTION_TOOL,
};
pub(crate) use crate::port::fire;
use crate::port::{
    end_after_result, end_run, settles_within, system_now, AccessOutcome, AdapterEvent, AnswerOutcome, DeltaKind, ExecFailure,
    ExecFailureKind, ExecStream, KeptRun, KeptTurn, OnEvent, PermissionAnswer, PermissionAsk, PermissionOutcome, SessionFile, SharedFuture,
    SteerOutcome, SubagentState, TaskStop, TurnImage, TurnResult, TurnStatus, TurnTokens, WriteOutcome, PERMISSION_ALLOW,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::UsageTokens;
use wsp_wire::views::session::TurnRefusal;
use wsp_words::date_parse::date_parse;
use wsp_words::js::js_trim;

/// adapter.ts:150.
pub const STOP_TASK_WAIT_MS: u64 = 10_000;
/// protocol wire/limits.ts:77.
pub const RUN_EXIT_MS: u64 = 10_000;
/// protocol wire/limits.ts:85.
pub const INTERRUPT_GRACE_MS: u64 = 5_000;
/// protocol wire/limits.ts:89.
pub const STEER_JOIN_MS: u64 = 10_000;
/// protocol wire/limits.ts:80.
pub const ASIDE_WALL_MS: u64 = 3 * 60_000;

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
    inner: SessionInner,
}

#[derive(Clone)]
enum SessionInner {
    Turn(Rc<Turn>),
    Lost(Rc<Lost>),
}

/// A session the registry does not keep alive.
#[derive(Clone)]
pub struct WeakSession(WeakInner);

#[derive(Clone)]
enum WeakInner {
    Turn(Weak<Turn>),
    Lost(Weak<Lost>),
}

impl WeakSession {
    pub(crate) fn alive(&self) -> bool {
        match &self.0 {
            WeakInner::Turn(w) => w.strong_count() > 0,
            WeakInner::Lost(w) => w.strong_count() > 0,
        }
    }
    pub(crate) fn upgrade(&self) -> Option<ClaudeSession> {
        let inner = match &self.0 {
            WeakInner::Turn(w) => SessionInner::Turn(w.upgrade()?),
            WeakInner::Lost(w) => SessionInner::Lost(w.upgrade()?),
        };
        Some(ClaudeSession { inner })
    }
}

impl ClaudeSession {
    pub(crate) fn downgrade(&self) -> WeakSession {
        WeakSession(match &self.inner {
            SessionInner::Turn(t) => WeakInner::Turn(Rc::downgrade(t)),
            SessionInner::Lost(l) => WeakInner::Lost(Rc::downgrade(l)),
        })
    }
}

/// The session a start answers once it may have resumed a lost session: the launch running now answers for it.
struct Lost {
    local_id: String,
    now: Arc<Mutex<Arc<ClaudeSession>>>,
    finished: SharedFuture<TurnResult>,
}

/// The process a finished turn left up for the thread's next message. adapter.ts:1254 and 1298.
#[derive(Clone)]
pub struct ClaudeKept {
    inner: Rc<Kept>,
}

/// What a kept process needs for its next turn. adapter.ts:1254 (`kept()`) and 1298 (`warm`).
pub(crate) struct Kept {
    pub(crate) core: Arc<AdapterCore>,
    pub(crate) keeper: Arc<dyn KeptRun>,
    pub(crate) local_id: String,
    pub(crate) session_id: String,
    pub(crate) saved: Option<SavedUse>,
    /// The launch line a warmed process's first turn reports as its command.
    pub(crate) first: Option<String>,
    pub(crate) session_file: Option<SessionFile>,
}

/// Where a turn's reply to a control request came back: the CLI's answer, or the request is gone.
enum AskReply {
    Answered { error: Option<String> },
    Gone,
}

/// The turn's mutable state. adapter.ts:664-772.
#[derive(Default)]
struct State {
    claude_session_id: String,
    saw_init: bool,
    saw_result: bool,
    exited: bool,
    interrupt_requested: bool,
    turn_result: Option<TurnResult>,
    empty_result: Option<TurnResult>,
    stderr_tail: Vec<String>,
    harness_cwd: Option<String>,
    shell_cwd: Option<String>,
    background_tasks: u32,
    running_tasks: Vec<String>,
    held_reply: Option<TurnResult>,
    held_at: f64,
    held_told: bool,
    task_names: IndexMap<String, String>,
    steers: Steers,
    plans: PlanBook,
    book: HandbackBook,
    held_context: Option<f64>,
    rejected: Rejected,
    compacted: bool,
    init_model: Option<String>,
    saved: Option<SavedUse>,
    totals: Option<SavedUse>,
    drawn: IndexSet<String>,
    spent: Option<TurnTokens>,
    calls: IndexMap<String, ModelUse>,
    calls_counted: usize,
    finished_after: Vec<String>,
    settle_timer: Option<JoinHandle<()>>,
    woken: bool,
    refusal_cause: Option<Option<TurnRefusal>>,
    pending: IndexMap<String, PermissionAsk>,
    asked: IndexMap<String, oneshot::Sender<AskReply>>,
    asked_seq: u64,
    launched_by: IndexMap<String, String>,
    subagents: IndexMap<String, SubagentState>,
    asked_by: IndexMap<String, String>,
    model_said: IndexSet<String>,
    spend: SpendBook,
    skips_prompts: bool,
    anchor: Option<String>,
    anchored: bool,
    steer_open: Option<oneshot::Sender<()>>,
}

impl State {
    fn running(&self) -> bool {
        self.saw_init && !self.saw_result && !self.exited && !self.interrupt_requested
    }
    fn busy(&self) -> bool {
        self.woken || self.steers.working()
    }
    fn next_request(&mut self) -> u64 {
        self.asked_seq += 1;
        self.asked_seq
    }
}

/// adapter.ts:162 onward: a result with only what it has, and `status`.
fn bare(status: TurnStatus) -> TurnResult {
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

fn now_ms(core: &AdapterCore) -> f64 {
    match &core.deps.now {
        Some(now) => now() as f64,
        None => system_now()() as f64,
    }
}

fn grace_ms(core: &AdapterCore) -> u64 {
    core.deps.interrupt_grace_ms.unwrap_or(INTERRUPT_GRACE_MS)
}

fn exit_ms(core: &AdapterCore) -> u64 {
    core.deps.result_exit_ms.unwrap_or(RUN_EXIT_MS)
}

fn no_parent(event: &Obj) -> bool {
    !matches!(event.get("parent_tool_use_id"), Some(v) if !v.is_null())
}

fn api_error_flag(event: &Obj) -> bool {
    event.get("is_api_error_message") == Some(&Value::Bool(true))
}

fn turn_tokens_of(u: &UsageTokens) -> TurnTokens {
    TurnTokens {
        input: u.input,
        output: u.output,
        cached: Some(u.cached),
        cache_write: Some(u.cache_write),
        reasoning: Some(u.reasoning),
        context: None,
        window: None,
    }
}

/// adapter.ts:708 (`drewUse`): the calls printed from `from` on, as a turn's use with no cost.
struct Drew {
    tokens: Option<TurnTokens>,
    models: Option<Vec<ModelUse>>,
    model: Option<String>,
}

fn drew_use(st: &State, from: usize) -> Drew {
    let calls: Vec<ModelUse> = st.calls.values().skip(from).cloned().collect();
    let none: Vec<ModelUse> = Vec::new();
    let models = with_calls(&none, &calls);
    if models.is_empty() {
        return Drew { tokens: None, models: None, model: None };
    }
    let spent = |m: &ModelUse| m.tokens.input.0 + m.tokens.output.0;
    let model = models.iter().reduce(|best, m| if spent(m) > spent(best) { m } else { best }).map(|m| m.model.clone());
    let tokens = models.iter().fold(None, |sum: Option<TurnTokens>, m| Some(plus_tokens(sum.as_ref(), &turn_tokens_of(&m.tokens))));
    Drew { tokens, models: Some(models), model }
}

/// adapter.ts:713 (`withCallsAfter`). Changes the running totals, so it runs once per reply.
fn with_calls_after(st: &mut State, reply: &TurnResult) -> TurnResult {
    let after = drew_use(st, st.calls_counted);
    let Some(after_models) = after.models.clone() else {
        return reply.clone();
    };
    if let Some(totals) = st.totals.as_mut() {
        let models = with_calls(&totals.models, &after_models);
        totals.models = models;
        let mut unpriced = totals.unpriced.clone().unwrap_or_default();
        unpriced.extend(after_models.iter().map(|m| m.model.clone()));
        totals.unpriced = Some(unpriced);
    }
    let window = reply.tokens.as_ref().and_then(|t| t.window);
    let context = st.held_context.or(reply.tokens.as_ref().and_then(|t| t.context.map(|c| c.0)));
    let own = reply.models.clone().unwrap_or_else(|| match &reply.tokens {
        None => Vec::new(),
        Some(tokens) => vec![ModelUse {
            model: reply.model.clone().or(after.model.clone()).unwrap_or_default(),
            tokens: usage_of(tokens),
            cost_usd: reply.cost_usd,
        }],
    });
    let mut tokens = after.tokens.as_ref().map(|a| plus_tokens(reply.tokens.as_ref(), a)).or_else(|| reply.tokens.clone());
    if let Some(t) = tokens.as_mut() {
        if window.is_some() {
            t.window = window;
        }
        if let Some(c) = context {
            t.context = Some(JsNumber(c));
        }
    }
    TurnResult { tokens, models: Some(own.into_iter().chain(after_models).collect()), ..reply.clone() }
}

/// adapter.ts:742 (`heldWithFinished`).
fn held_with_finished(st: &mut State, reply: &TurnResult, running: u32) -> TurnResult {
    let result = with_calls_after(st, reply);
    let mut lines = st.finished_after.clone();
    if running > 0 && result.status == TurnStatus::Completed {
        lines.push(background_tasks_line(running));
    }
    if lines.is_empty() {
        return result;
    }
    let text = [result.text.clone().unwrap_or_default(), String::new()].into_iter().chain(lines).collect::<Vec<_>>().join("\n");
    TurnResult { text: Some(text), ..result }
}

/// adapter.ts:757 (`spanned`).
fn spanned(st: &State, result: TurnResult, now: f64) -> TurnResult {
    let Some(held) = st.held_reply.as_ref().and_then(|h| h.duration_ms) else {
        return result;
    };
    let own = result.duration_ms.map_or(0.0, |d| d.0);
    TurnResult { duration_ms: Some(JsNumber(held.0 + (now - st.held_at).max(own))), ..result }
}

/// adapter.ts:672 (`anchorOnce`): the event the first time an anchor was seen.
fn anchor_once(st: &mut State, session_id: &str) -> Option<AdapterEvent> {
    if st.anchored {
        return None;
    }
    let anchor = st.anchor.clone()?;
    st.anchored = true;
    Some(AdapterEvent::TurnAnchor { session_id: session_id.into(), anchor, kept: None })
}

/// steers.ts:89 (`tellUnread`) as the events it emits.
fn unread_events(st: &mut State, session_id: &str) -> Vec<AdapterEvent> {
    let out: RefCell<Vec<AdapterEvent>> = RefCell::new(Vec::new());
    st.steers.tell_unread(session_id, &|event| out.borrow_mut().push(event));
    out.into_inner()
}

/// adapter.ts:1292 (`refusalOf`): wsp's half of a refusal the CLI named a cause for.
fn refusal_of(core: &AdapterCore, cause: Option<Option<TurnRefusal>>) -> Option<Refusal> {
    let cause = cause?;
    let road = if cause == Some(TurnRefusal::SignIn) { core.deps.sign_in_refusal.clone() } else { None };
    Some(Refusal { road, cause })
}

/// Reads `o.stream` on a task of its own from the moment it is called, and registers the session in `core`'s map
/// under `local_id`. adapter.ts:646.
pub fn follow(core: &Arc<AdapterCore>, o: FollowOptions) -> ClaudeSession {
    let (finish_tx, finish_rx) = oneshot::channel::<TurnResult>();
    let finished: SharedFuture<TurnResult> =
        async move { finish_rx.await.unwrap_or_else(|_| bare(TurnStatus::Failed)) }.boxed_local().shared();
    let (open_tx, open_rx) = oneshot::channel::<()>();
    let steer_open: SharedFuture<()> = open_rx.map(|_| ()).boxed_local().shared();
    let steers = Steers::new(&o.stream.taken().unwrap_or_default(), &o.steered.unwrap_or_default());
    let turn = Rc::new(Turn {
        core: core.clone(),
        stream: o.stream,
        local_id: o.local_id.clone(),
        fresh: o.fresh,
        command: o.command,
        keeper: o.keeper,
        from: o.from,
        on_event: o.on_event,
        steer_open,
        finished: finished.clone(),
        st: RefCell::new(State {
            claude_session_id: o.local_id.clone(),
            saw_init: o.announced,
            saved: o.saved,
            steers,
            steer_open: Some(open_tx),
            ..State::default()
        }),
    });
    let session = ClaudeSession { inner: SessionInner::Turn(turn.clone()) };
    core.register(o.local_id, &session);
    tokio::task::spawn_local(async move {
        let result = turn.read_all().await;
        let _ = finish_tx.send(result);
    });
    session
}

/// The turn's stream reader and everything the turn holds. adapter.ts:643-1136.
pub(crate) struct Turn {
    core: Arc<AdapterCore>,
    stream: Arc<dyn ExecStream>,
    local_id: String,
    fresh: bool,
    command: Option<String>,
    keeper: Option<Arc<dyn KeptRun>>,
    from: Option<u64>,
    on_event: OnEvent,
    steer_open: SharedFuture<()>,
    finished: SharedFuture<TurnResult>,
    st: RefCell<State>,
}

impl Turn {
    fn emit(&self, event: AdapterEvent) {
        (self.on_event)(event)
    }

    fn claude_session_id(&self) -> String {
        self.st.borrow().claude_session_id.clone()
    }

    fn settle_asked(st: &mut State) -> Vec<oneshot::Sender<AskReply>> {
        std::mem::take(&mut st.asked).into_iter().map(|(_, tx)| tx).collect()
    }

    fn arm_settle(self: &Rc<Self>, st: &mut State) {
        if let Some(timer) = st.settle_timer.take() {
            timer.abort();
        }
        let me = self.clone();
        let wait = exit_ms(&self.core);
        st.settle_timer = Some(tokio::task::spawn_local(async move {
            tokio::time::sleep(Duration::from_millis(wait)).await;
            me.settle_fired();
        }));
    }

    fn settle_fired(self: &Rc<Self>) {
        let held = {
            let mut st = self.st.borrow_mut();
            st.settle_timer = None;
            match st.held_reply.clone() {
                Some(reply) if st.background_tasks == 0 => {
                    let result = held_with_finished(&mut st, &reply, 0);
                    Some((result, st.held_told, st.claude_session_id.clone()))
                }
                _ => None,
            }
        };
        if let Some((result, told, sid)) = held {
            self.deliver(result, sid, told);
        }
    }

    fn deliver(self: &Rc<Self>, reply: TurnResult, session_id: String, held: bool) {
        let (result, queued_left, asked) = {
            let mut st = self.st.borrow_mut();
            let result = with_limit(reply, &st.rejected);
            if let Some(timer) = st.settle_timer.take() {
                timer.abort();
            }
            st.held_reply = None;
            st.saw_result = true;
            let queued_left = st.steers.settle();
            (result, queued_left, Self::settle_asked(&mut st))
        };
        if (result.status != TurnStatus::Completed && result.status != TurnStatus::Interrupted) || answered_nothing(&result) {
            if let Some(keeper) = &self.keeper {
                keeper.release();
            }
        }
        if queued_left {
            if let Some(keeper) = &self.keeper {
                keeper.release();
            }
        }
        self.stream.close_input();
        for tx in asked {
            let _ = tx.send(AskReply::Gone);
        }
        let grace = grace_ms(&self.core);
        let exit = exit_ms(&self.core);
        let stream = self.stream.clone();
        fire(
            async move {
                if queued_left {
                    end_run(&*stream, grace).await;
                } else {
                    end_after_result(&*stream, exit, grace).await;
                }
            }
            .boxed_local(),
        );
        if answered_nothing(&result) && !self.st.borrow().compacted {
            self.st.borrow_mut().empty_result = Some(result);
            return;
        }
        self.publish(result, session_id, held.then_some(true));
    }

    /// adapter.ts:1116-1123 and 1128-1131: the turn's result is set, its anchor and unread messages told, then turn.done.
    fn publish(&self, result: TurnResult, session_id: String, held: Option<bool>) {
        let (anchor, unread) = {
            let mut st = self.st.borrow_mut();
            st.turn_result = Some(result.clone());
            (anchor_once(&mut st, &session_id), unread_events(&mut st, &session_id))
        };
        if let Some(event) = anchor {
            self.emit(event);
        }
        for event in unread {
            self.emit(event);
        }
        self.emit(AdapterEvent::TurnDone { session_id, result, held });
    }

    async fn read_all(self: Rc<Self>) -> TurnResult {
        let mut lines = self.stream.lines();
        let mut stream_error: Option<ExecFailure> = None;
        while let Some(item) = lines.next().await {
            match item {
                Ok(raw) => self.read_line(raw),
                Err(failure) => {
                    stream_error = Some(failure);
                    break;
                }
            }
        }
        if let Some(timer) = self.st.borrow_mut().settle_timer.take() {
            timer.abort();
        }
        let exit_code = self.stream.exited().await;
        let (pending_ids, asked) = {
            let mut st = self.st.borrow_mut();
            st.exited = true;
            (st.pending.keys().cloned().collect::<Vec<_>>(), Self::settle_asked(&mut st))
        };
        for ask_id in pending_ids {
            self.close_ask(&ask_id, PermissionOutcome::Cancelled, None);
        }
        for tx in asked {
            let _ = tx.send(AskReply::Gone);
        }
        let (reached, path) = (self.st.borrow().saw_init, self.core.env.get("PATH").cloned());
        let exit_line = match &stream_error {
            Some(failure) => failure.message.clone(),
            None => harness_exit_line(
                "claude",
                exit_code,
                path.as_deref(),
                &HarnessEnded { reached: Some(reached), signal: self.stream.signalled() },
            ),
        };
        let held_end = {
            let mut st = self.st.borrow_mut();
            match st.held_reply.clone() {
                Some(held) if st.turn_result.is_none() => {
                    st.saw_result = true;
                    let now = now_ms(&self.core);
                    let cut = |st: &mut State| {
                        let mut r = with_calls_after(st, &held);
                        if let Some(d) = held.duration_ms {
                            r.duration_ms = Some(JsNumber(d.0 + now - st.held_at));
                        }
                        r
                    };
                    let result = if st.interrupt_requested {
                        TurnResult { status: TurnStatus::Interrupted, ..cut(&mut st) }
                    } else if st.woken {
                        TurnResult { status: TurnStatus::Failed, error: Some(exit_line.clone()), ..cut(&mut st) }
                    } else {
                        let running = st.background_tasks;
                        held_with_finished(&mut st, &held, running)
                    };
                    Some((result, st.held_told && !st.woken, st.claude_session_id.clone()))
                }
                _ => None,
            }
        };
        if let Some((result, held, sid)) = held_end {
            self.publish(result, sid, held.then_some(true));
        }
        if self.st.borrow().turn_result.is_none() {
            let (result, sid) = {
                let st = self.st.borrow();
                let result = match st.empty_result.clone() {
                    Some(empty) => {
                        let error = no_output_error(&empty, &st.stderr_tail);
                        TurnResult { status: TurnStatus::Failed, error: Some(error), ..empty }
                    }
                    None => {
                        let drew = drew_use(&st, 0);
                        let status = if st.interrupt_requested { TurnStatus::Interrupted } else { TurnStatus::Failed };
                        let error = (!st.interrupt_requested).then(|| exit_line.clone());
                        TurnResult { status, error, tokens: drew.tokens, models: drew.models, model: drew.model, ..bare(status) }
                    }
                };
                (result, st.claude_session_id.clone())
            };
            self.publish(result, sid, None);
        }
        let (sid, saw_result, result) = {
            let st = self.st.borrow();
            (st.claude_session_id.clone(), st.saw_result, st.turn_result.clone())
        };
        self.emit(AdapterEvent::SessionEnd { session_id: sid, exit_code, saw_result, failure: stream_error });
        result.unwrap_or_else(|| bare(TurnStatus::Failed))
    }

    /// One stream line, in the order of adapter.ts:872-1090.
    fn read_line(self: &Rc<Self>, raw: String) {
        {
            let mut st = self.st.borrow_mut();
            if st.held_reply.is_some() && st.background_tasks == 0 && !st.busy() {
                self.arm_settle(&mut st);
            }
        }
        let Some(event) = parse_line(&raw) else {
            let text = js_trim(&raw).to_string();
            if !text.is_empty() {
                let mut st = self.st.borrow_mut();
                st.stderr_tail.push(text);
                if st.stderr_tail.len() > STDERR_TAIL_LINES {
                    st.stderr_tail.remove(0);
                }
            }
            return;
        };
        let kind = string(event.get("type"));
        if kind == Some("cost-state") {
            let mut st = self.st.borrow_mut();
            st.saved = saved_use_of(&event, &self.local_id);
            return;
        }
        if self.read_steer(&event) {
            return;
        }
        if kind == Some("result") && !drained_notice(&event) {
            let mut st = self.st.borrow_mut();
            let cost = num(event.get("total_cost_usd"));
            st.totals = cost.map(|c| SavedUse { cost_usd: c, models: models_of(event.get("modelUsage")), unpriced: None });
            if let Some(used) = rec(event.get("usage")) {
                let sum = plus_tokens(st.spent.as_ref(), &usage_tokens(used));
                st.spent = Some(sum);
            }
            st.calls_counted = st.calls.len();
        }
        {
            let mut guard = self.st.borrow_mut();
            let st = &mut *guard;
            if kind == Some("assistant") && no_parent(&event) {
                if let Some(uuid) = string(event.get("uuid")) {
                    st.anchor = Some(uuid.to_string());
                }
            }
            for (call, prompt) in subagent_prompts(&event) {
                st.asked_by.insert(call, prompt);
            }
            note_line(&mut st.book, &event);
        }
        let child_line = if kind == Some("assistant") && !api_error_flag(&event) {
            string(event.get("parent_tool_use_id")).map(String::from)
        } else {
            None
        };
        if let Some(child) = &child_line {
            let mut guard = self.st.borrow_mut();
            let st = &mut *guard;
            let child_task =
                st.launched_by.iter().find(|(id, call)| *call == child && st.subagents.contains_key(*id)).map(|(id, _)| id.clone());
            if let Some(task) = child_task {
                note_child_call(&mut st.spend, &task, &event);
            }
            let child_model = rec(event.get("message")).and_then(|m| string(m.get("model"))).map(String::from);
            if let Some(model) = child_model {
                let task = st
                    .launched_by
                    .iter()
                    .find(|(id, call)| {
                        *call == child && st.subagents.get(*id) == Some(&SubagentState::Running) && !st.model_said.contains(*id)
                    })
                    .map(|(id, _)| id.clone());
                if let Some(task) = task {
                    st.model_said.insert(task.clone());
                    let sid = st.claude_session_id.clone();
                    drop(guard);
                    self.emit(AdapterEvent::Subagent {
                        session_id: sid,
                        task,
                        state: SubagentState::Running,
                        parent_tool_use_id: Some(child.clone()),
                        title: None,
                        summary: None,
                        depth: None,
                        model: Some(model),
                        asked: None,
                        usage: None,
                    });
                }
            }
        }
        if let Some(control) = control_line(&event) {
            self.read_control(control);
            return;
        }
        let progressed = {
            let mut guard = self.st.borrow_mut();
            let st = &mut *guard;
            let subagents = &st.subagents;
            note_progress(&mut st.spend, &event, &|task| subagents.contains_key(task))
        };
        if let Some(task) = progressed {
            if string(event.get("subtype")) == Some("task_progress") {
                let (sid, parent, usage) = {
                    let st = self.st.borrow();
                    if st.subagents.get(&task) != Some(&SubagentState::Running) {
                        (None, None, None)
                    } else {
                        (Some(st.claude_session_id.clone()), st.launched_by.get(&task).cloned(), subagent_spent(&st.spend, &task))
                    }
                };
                if let Some(sid) = sid {
                    self.emit(AdapterEvent::Subagent {
                        session_id: sid,
                        task,
                        state: SubagentState::Running,
                        parent_tool_use_id: parent,
                        title: None,
                        summary: None,
                        depth: None,
                        model: None,
                        asked: None,
                        usage,
                    });
                }
            }
        }
        if let Some(launch) = subagent_launch(&event) {
            let started = {
                let mut guard = self.st.borrow_mut();
                let st = &mut *guard;
                st.launched_by.insert(launch.agent_id, launch.tool_use_id);
                subagent_started(&event).map(|started| {
                    st.subagents.insert(started.task.clone(), SubagentState::Running);
                    let prompt = started.parent.as_ref().and_then(|p| st.asked_by.get(p)).cloned();
                    if let Some(parent) = &started.parent {
                        st.asked_by.shift_remove(parent);
                    }
                    (started, prompt, st.claude_session_id.clone())
                })
            };
            if let Some((started, prompt, sid)) = started {
                self.emit(AdapterEvent::Subagent {
                    session_id: sid,
                    task: started.task,
                    state: SubagentState::Running,
                    parent_tool_use_id: started.parent,
                    title: started.title,
                    summary: None,
                    depth: started.depth.map(JsNumber),
                    model: None,
                    asked: prompt.map(|p| subagent_asked(&p)),
                    usage: None,
                });
            }
            return;
        }
        if let Some(tasks) = background_tasks_of(&event) {
            let (sid, count) = {
                let mut st = self.st.borrow_mut();
                for task in &tasks {
                    st.task_names.insert(task.id.clone(), task.description.clone());
                }
                st.background_tasks = tasks.len() as u32;
                st.running_tasks = tasks.iter().map(|t| t.id.clone()).collect();
                (st.claude_session_id.clone(), st.background_tasks)
            };
            self.emit(AdapterEvent::TurnTasks { session_id: sid, running: count, replied: None });
            let mut st = self.st.borrow_mut();
            if st.held_reply.is_some() && st.background_tasks == 0 && !st.busy() {
                self.arm_settle(&mut st);
            }
            if st.background_tasks > 0 {
                if let Some(timer) = st.settle_timer.take() {
                    timer.abort();
                }
            }
            return;
        }
        if interim_end(&self.st.borrow().book, &event) {
            return;
        }
        let ended = task_ended(&event);
        if let Some(end) = &ended {
            let ev = {
                let mut guard = self.st.borrow_mut();
                let st = &mut *guard;
                let running = st.subagents.get(&end.task) == Some(&SubagentState::Running);
                if running || (st.subagents.contains_key(&end.task) && later_end(&st.book, &event)) {
                    st.subagents.insert(end.task.clone(), end.state);
                    Some(AdapterEvent::Subagent {
                        session_id: st.claude_session_id.clone(),
                        task: end.task.clone(),
                        state: end.state,
                        parent_tool_use_id: st.launched_by.get(&end.task).cloned(),
                        title: None,
                        summary: end.summary.clone(),
                        depth: None,
                        model: None,
                        asked: None,
                        usage: subagent_spent_at_end(&st.spend, &end.task, &event),
                    })
                } else {
                    None
                }
            };
            if let Some(ev) = ev {
                self.emit(ev);
            }
        }
        let (_, deltas) = {
            let mut st = self.st.borrow_mut();
            let sid = st.claude_session_id.clone();
            (sid.clone(), end_answer(&mut st.book, &event, &sid))
        };
        for delta in deltas {
            self.emit(AdapterEvent::TurnDelta(delta));
        }
        if let Some((id, status)) = task_finished_of(&event) {
            let mut st = self.st.borrow_mut();
            if st.held_reply.is_some() {
                let name = st.task_names.get(&id).cloned().unwrap_or(id.clone());
                let line = task_finished_line(&name, &status, now_ms(&self.core) - st.held_at);
                st.finished_after.push(line);
            }
            return;
        }
        if ended.is_some() {
            return;
        }
        {
            let st = self.st.borrow();
            if st.held_reply.is_none() && !st.saw_result && drained_notice(&event) {
                return;
            }
        }
        if let Some(c) = compacted_to(&event) {
            {
                let mut st = self.st.borrow_mut();
                st.held_context = Some(c.after);
                st.compacted = true;
            }
            self.emit(AdapterEvent::TurnCompacted { session_id: self.claude_session_id(), before: c.before, after: Some(c.after) });
        }
        self.read_usage(&event);
        if kind == Some("assistant") && string(event.get("parent_tool_use_id")).is_none() {
            if let Some(usage) = rec(event.get("message")).and_then(|m| rec(m.get("usage"))) {
                if !api_error_flag(&event) {
                    self.st.borrow_mut().held_context = Some(held_tokens(usage));
                }
            }
        }
        if kind == Some("rate_limit_event") {
            note_rejected(rec(event.get("rate_limit_info")), &mut self.st.borrow_mut().rejected);
        }
        match api_error_cause(&event) {
            ApiErrorCause::NotOne => {}
            ApiErrorCause::Unclaimed => {
                self.st.borrow_mut().refusal_cause = Some(None);
                return;
            }
            ApiErrorCause::Cause(cause) => {
                self.st.borrow_mut().refusal_cause = Some(Some(cause));
                return;
            }
        }
        let (refusal, fallback) = {
            let st = self.st.borrow();
            (refusal_of(&self.core, st.refusal_cause), st.claude_session_id.clone())
        };
        let normalized = {
            let mut guard = self.st.borrow_mut();
            let st = &mut *guard;
            normalize_event(&event, &fallback, refusal.as_ref(), &mut st.plans, &mut st.book)
        };
        for mut normalized in normalized {
            self.read_normalized(&mut normalized, &event);
        }
    }

    fn read_steer(self: &Rc<Self>, event: &Obj) -> bool {
        let (is_steer, deliver) = {
            let mut guard = self.st.borrow_mut();
            let st = &mut *guard;
            let is_steer = st.steers.read(event);
            if !is_steer {
                return false;
            }
            if st.steers.working() {
                if let Some(timer) = st.settle_timer.take() {
                    timer.abort();
                }
            }
            let deliver = match st.held_reply.clone() {
                Some(held) if st.background_tasks == 0 && !st.steers.open() && !st.woken => {
                    let result = if st.interrupt_requested {
                        let mut r = with_calls_after(st, &held);
                        r.status = TurnStatus::Interrupted;
                        r
                    } else {
                        held_with_finished(st, &held, 0)
                    };
                    Some((result, st.held_told, st.claude_session_id.clone()))
                }
                _ => None,
            };
            (is_steer, deliver)
        };
        if let Some((result, told, sid)) = deliver {
            self.deliver(result, sid, told);
        }
        is_steer
    }

    fn read_usage(self: &Rc<Self>, event: &Obj) {
        let call = if string(event.get("type")) == Some("assistant") && !api_error_flag(event) { rec(event.get("message")) } else { None };
        let call_usage = call.and_then(|c| rec(c.get("usage")));
        let call_id = call.and_then(|c| string(c.get("id")));
        let call_model = call.and_then(|c| string(c.get("model")));
        if let (Some(usage), Some(id), Some(model)) = (call_usage, call_id, call_model) {
            let mut st = self.st.borrow_mut();
            let held = st.calls.get(id).map_or(0.0, |c| c.tokens.output.0);
            let output = held.max(num(usage.get("output_tokens")).unwrap_or(0.0));
            let tokens = UsageTokens {
                input: JsNumber(input_tokens(usage)),
                output: JsNumber(output),
                cached: JsNumber(num(usage.get("cache_read_input_tokens")).unwrap_or(0.0)),
                cache_write: JsNumber(num(usage.get("cache_creation_input_tokens")).unwrap_or(0.0)),
                reasoning: JsNumber(0.0),
            };
            st.calls.insert(id.to_string(), ModelUse { model: model.to_string(), tokens, cost_usd: None });
        }
        let Some((usage, id)) = call_usage.zip(call_id) else {
            return;
        };
        let (sid, fresh_draw) = {
            let mut st = self.st.borrow_mut();
            (st.claude_session_id.clone(), st.drawn.insert(id.to_string()))
        };
        if !fresh_draw {
            return;
        }
        let at = string(event.get("timestamp")).map(date_parse).filter(|t| t.is_finite());
        let drew = held_tokens(usage);
        let own = string(event.get("parent_tool_use_id")).is_none();
        self.emit(AdapterEvent::TurnUsage { session_id: sid, tokens: drew, context: own.then_some(drew), window: None, at });
    }

    fn read_control(self: &Rc<Self>, control: ControlLine) {
        match control {
            ControlLine::Ask { ask, agent_id } => {
                let (skip, sid, ask) = {
                    let mut guard = self.st.borrow_mut();
                    let st = &mut *guard;
                    if st.skips_prompts && ask.tool_name != QUESTION_TOOL {
                        (true, String::new(), ask)
                    } else {
                        let mut ask = ask;
                        if let Some(parent) = agent_id.as_ref().and_then(|id| st.launched_by.get(id)) {
                            ask.parent_tool_use_id = Some(parent.clone());
                        }
                        st.pending.insert(ask.ask_id.clone(), ask.clone());
                        (false, st.claude_session_id.clone(), ask)
                    }
                };
                if skip {
                    if let Ok(line) = control_allow_line(&ask) {
                        fire(self.stream.write(line));
                    }
                    return;
                }
                self.emit(AdapterEvent::PermissionAsk { session_id: sid, ask });
            }
            ControlLine::Cancel { request_id } => {
                let open = self.st.borrow().pending.contains_key(&request_id);
                if open {
                    self.close_ask(&request_id, PermissionOutcome::Cancelled, None);
                }
            }
            ControlLine::Unknown { request_id, subtype } => {
                fire(self.stream.write(control_error_line(&request_id, &subtype)));
            }
            ControlLine::Answer { request_id, error } => {
                let settle = self.st.borrow_mut().asked.shift_remove(&request_id);
                if let Some(tx) = settle {
                    let _ = tx.send(AskReply::Answered { error });
                }
            }
        }
    }

    fn read_normalized(self: &Rc<Self>, normalized: &mut AdapterEvent, event: &Obj) {
        match normalized {
            AdapterEvent::SessionStart { session_id, model, cwd, harness, .. } => {
                let mut guard = self.st.borrow_mut();
                let st = &mut *guard;
                st.claude_session_id = session_id.clone();
                st.init_model = model.clone().or_else(|| st.init_model.take());
                st.saw_init = true;
                if let Some(tx) = st.steer_open.take() {
                    let _ = tx.send(());
                }
                st.harness_cwd = cwd.clone();
                st.shell_cwd = cwd.clone();
                if harness.as_ref().and_then(|h| h.permission_mode.as_deref()) == Some(SKIP_PROMPTS_MODE) {
                    st.skips_prompts = true;
                }
                if st.held_reply.is_some() {
                    st.woken = true;
                    if let Some(timer) = st.settle_timer.take() {
                        timer.abort();
                    }
                }
            }
            AdapterEvent::TurnDelta(delta) if delta.kind == DeltaKind::ToolUse => {
                let moved = {
                    let mut st = self.st.borrow_mut();
                    match (st.shell_cwd.clone(), st.harness_cwd.clone()) {
                        (Some(shell), Some(harness)) => {
                            let moved = shell_cwd_after(delta.tool_name.as_deref(), parse_input(&delta.text).as_ref(), &shell, &harness);
                            if let Some(moved) = &moved {
                                st.shell_cwd = Some(moved.clone());
                            }
                            moved
                        }
                        _ => None,
                    }
                };
                if moved.is_some() {
                    delta.cwd = moved;
                }
            }
            AdapterEvent::TurnDone { session_id, result, .. } => {
                let (session_id, result) = (session_id.clone(), result.clone());
                self.read_turn_done(session_id, result, event);
                return;
            }
            _ => {}
        }
        self.emit(normalized.clone());
    }

    fn read_turn_done(self: &Rc<Self>, session_id: String, result: TurnResult, event: &Obj) {
        enum Next {
            Skip,
            Stop(Vec<String>, TurnResult),
            DeliverHeld(TurnResult),
            Hold(Option<AdapterEvent>),
            Deliver(TurnResult),
        }
        let next = {
            let mut guard = self.st.borrow_mut();
            let st = &mut *guard;
            if st.saw_result {
                Next::Skip
            } else {
                st.woken = false;
                if st.held_reply.is_some() && num(event.get("num_turns")) == Some(0.0) {
                    Next::Skip
                } else {
                    let now = now_ms(&self.core);
                    let own = own_use(result, st.saved.as_ref(), self.fresh, st.spent.as_ref());
                    let result = spanned(st, with_context(own, st.held_context, st.init_model.as_deref()), now);
                    let result_text_blank = js_trim(result.text.as_deref().unwrap_or("")).is_empty();
                    if st.background_tasks > 0 && st.interrupt_requested {
                        let mut lines = Vec::new();
                        for task in st.running_tasks.clone() {
                            let id = format!("wsp-stop-task-{}", st.next_request());
                            lines.push(stop_task_line(&id, &task));
                        }
                        Next::Stop(lines, result)
                    } else if st.interrupt_requested && st.held_reply.is_some() && result_text_blank {
                        let held_text = st.held_reply.as_ref().and_then(|h| h.text.clone()).unwrap_or_default();
                        Next::DeliverHeld(TurnResult { text: Some(held_text), ..result })
                    } else if st.background_tasks > 0 || (st.steers.open() && !st.interrupt_requested) {
                        st.held_reply = Some(result.clone());
                        st.held_at = now;
                        st.finished_after.clear();
                        st.held_told = st.background_tasks > 0;
                        if st.held_told {
                            let running = st.background_tasks;
                            Next::Hold(Some(AdapterEvent::TurnTasks {
                                session_id: st.claude_session_id.clone(),
                                running,
                                replied: Some(result),
                            }))
                        } else {
                            if !st.busy() {
                                self.arm_settle(st);
                            }
                            Next::Hold(None)
                        }
                    } else {
                        Next::Deliver(result)
                    }
                }
            }
        };
        match next {
            Next::Skip => {}
            Next::Stop(lines, result) => {
                for line in lines {
                    fire(self.stream.write(line));
                }
                self.deliver(result, session_id, false);
            }
            Next::DeliverHeld(result) => self.deliver(result, session_id, false),
            Next::Hold(event) => {
                if let Some(event) = event {
                    self.emit(event);
                }
            }
            Next::Deliver(result) => self.deliver(result, session_id, false),
        }
    }

    async fn answer_ask(
        self: &Rc<Self>,
        ask_id: &str,
        option_id: &str,
        outcome: PermissionOutcome,
        deny: &str,
    ) -> Result<AnswerOutcome, ExecFailure> {
        let ask = {
            let mut st = self.st.borrow_mut();
            let ask = match st.pending.get(ask_id) {
                Some(ask) if !st.exited => ask.clone(),
                _ => return Ok(AnswerOutcome::Gone),
            };
            st.pending.shift_remove(ask_id);
            ask
        };
        let line = crate::claude::permissions::control_answer_line(&ask, option_id, deny)
            .map_err(|e| ExecFailure { kind: ExecFailureKind::Other, message: e.0 })?;
        let wrote = match self.stream.write(line).await {
            Ok(wrote) => wrote,
            Err(failure) => {
                let mut st = self.st.borrow_mut();
                if !st.exited {
                    st.pending.insert(ask_id.to_string(), ask);
                }
                return Err(failure);
            }
        };
        if wrote != WriteOutcome::Written {
            return Ok(AnswerOutcome::Gone);
        }
        let sid = self.claude_session_id();
        self.emit(AdapterEvent::PermissionClose {
            session_id: sid,
            ask_id: ask_id.to_string(),
            outcome,
            option_id: Some(option_id.to_string()),
        });
        Ok(AnswerOutcome::Answered)
    }

    fn close_ask(&self, ask_id: &str, outcome: PermissionOutcome, option_id: Option<String>) {
        let sid = {
            let mut st = self.st.borrow_mut();
            st.pending.shift_remove(ask_id);
            st.claude_session_id.clone()
        };
        self.emit(AdapterEvent::PermissionClose { session_id: sid, ask_id: ask_id.to_string(), outcome, option_id });
    }

    async fn steer(self: &Rc<Self>, prompt: &str, id: Option<String>, images: Vec<TurnImage>) -> Result<SteerOutcome, ExecFailure> {
        if !self.st.borrow().saw_init {
            let join = futures::future::select(self.steer_open.clone(), self.finished.clone()).map(|_| ()).boxed_local();
            settles_within(join, STEER_JOIN_MS).await;
        }
        if !self.st.borrow_mut().running() {
            return Ok(SteerOutcome::NotRunning);
        }
        let uuid = id.unwrap_or_else(new_session_id);
        let line = {
            let mut st = self.st.borrow_mut();
            st.steers.add(&uuid);
            user_message_line(prompt, &st.claude_session_id, &images, Some(&uuid))
        };
        let wrote = self.stream.write(line).await?;
        if wrote == WriteOutcome::Written && self.st.borrow().running() {
            return Ok(SteerOutcome::Accepted);
        }
        self.st.borrow_mut().steers.forget(&uuid);
        Ok(SteerOutcome::NotRunning)
    }

    async fn set_access(self: &Rc<Self>, mode: &str) -> Result<AccessOutcome, ExecFailure> {
        if !self.st.borrow().running() {
            return Ok(AccessOutcome::Gone);
        }
        let (request_id, answered) = {
            let mut st = self.st.borrow_mut();
            let id = format!("wsp-set-mode-{}", st.next_request());
            let (tx, rx) = oneshot::channel();
            st.asked.insert(id.clone(), tx);
            (id, rx)
        };
        let wrote = match self.stream.write(set_mode_line(&request_id, mode)).await {
            Ok(wrote) => wrote,
            Err(failure) => return Err(failure),
        };
        if wrote != WriteOutcome::Written {
            self.st.borrow_mut().asked.shift_remove(&request_id);
            return Ok(AccessOutcome::Gone);
        }
        if mode == SKIP_PROMPTS_MODE {
            let targets: Vec<String> = {
                let mut st = self.st.borrow_mut();
                st.skips_prompts = true;
                st.pending.iter().filter(|(_, ask)| ask.tool_name != QUESTION_TOOL).map(|(id, _)| id.clone()).collect()
            };
            for id in targets {
                self.answer_ask(&id, PERMISSION_ALLOW, PermissionOutcome::Allowed, "").await?;
            }
            return Ok(AccessOutcome::Set);
        }
        let outcome = match answered.await {
            Ok(AskReply::Answered { error: None }) => AccessOutcome::Set,
            Ok(AskReply::Answered { error: Some(_) }) => AccessOutcome::Refused,
            _ => AccessOutcome::Gone,
        };
        if outcome == AccessOutcome::Set {
            let targets: Vec<(String, String)> = {
                let st = self.st.borrow();
                st.pending.iter().filter_map(|(id, ask)| mode_option_on(ask, mode).map(|opt| (id.clone(), opt))).collect()
            };
            for (id, option) in targets {
                self.answer_ask(&id, &option, PermissionOutcome::Allowed, "").await?;
            }
        }
        Ok(outcome)
    }

    async fn stop_task(self: &Rc<Self>, task: &str) -> Result<TaskStop, ExecFailure> {
        let (request_id, answered) = {
            let mut st = self.st.borrow_mut();
            let state = st.subagents.get(task).copied();
            if !st.running() || (state.is_some() && state != Some(SubagentState::Running)) {
                return Ok(TaskStop::NotRunning);
            }
            let id = format!("wsp-stop-task-{}", st.next_request());
            let (tx, rx) = oneshot::channel();
            st.asked.insert(id.clone(), tx);
            (id, rx)
        };
        let wait = self.core.deps.stop_task_wait_ms.unwrap_or(STOP_TASK_WAIT_MS);
        let timer = tokio::time::sleep(Duration::from_millis(wait));
        let wrote = match self.stream.write(stop_task_line(&request_id, task)).await {
            Ok(wrote) => wrote,
            Err(failure) => {
                self.st.borrow_mut().asked.shift_remove(&request_id);
                return Err(failure);
            }
        };
        if wrote != WriteOutcome::Written {
            self.st.borrow_mut().asked.shift_remove(&request_id);
            return Ok(TaskStop::NotRunning);
        }
        let answer = tokio::select! {
            reply = answered => Some(reply.unwrap_or(AskReply::Gone)),
            _ = timer => None,
        };
        match answer {
            None => {
                self.st.borrow_mut().asked.shift_remove(&request_id);
                Ok(TaskStop::Refused { error: format!("it did not answer within {}", fmt_duration(wait as f64)) })
            }
            Some(AskReply::Gone) => Ok(TaskStop::NotRunning),
            Some(AskReply::Answered { error: None }) => Ok(TaskStop::Accepted),
            Some(AskReply::Answered { error: Some(error) }) => {
                if error.contains(STOP_TASK_UNSUPPORTED) {
                    Ok(TaskStop::Unsupported)
                } else {
                    Ok(TaskStop::Refused { error })
                }
            }
        }
    }

    async fn interrupt(self: &Rc<Self>) {
        let live = {
            let mut st = self.st.borrow_mut();
            let live = st.running();
            st.interrupt_requested = true;
            if let Some(tx) = st.steer_open.take() {
                let _ = tx.send(());
            }
            live
        };
        let grace = grace_ms(&self.core);
        let held = {
            let mut guard = self.st.borrow_mut();
            let st = &mut *guard;
            match st.held_reply.clone() {
                Some(reply) if st.background_tasks == 0 && !st.busy() && !st.steers.cancels_queued() => {
                    let mut result = with_calls_after(st, &reply);
                    result.status = TurnStatus::Interrupted;
                    Some((result, st.held_told, st.claude_session_id.clone()))
                }
                _ => None,
            }
        };
        if let Some((result, told, sid)) = held {
            self.deliver(result, sid, told);
            settles_within(self.finished.clone().boxed_local(), grace).await;
            return;
        }
        let keeper_up = self.keeper.as_ref().is_some_and(|k| k.up());
        if keeper_up && live {
            let line = {
                let mut st = self.st.borrow_mut();
                let id = format!("wsp-interrupt-{}", st.next_request());
                let cancel = st.steers.cancels_queued();
                interrupt_line(&id, cancel)
            };
            let wrote = self.stream.write(line).await;
            if matches!(wrote, Ok(WriteOutcome::Written)) && settles_within(self.finished.clone().boxed_local(), grace).await {
                return;
            }
        }
        if let Some(keeper) = &self.keeper {
            keeper.release();
        }
        end_run(&*self.stream, grace).await;
    }

    fn kept(&self) -> Option<ClaudeKept> {
        let keeper = self.keeper.clone()?;
        let st = self.st.borrow();
        if !keeper.up() || !st.saw_result {
            return None;
        }
        Some(ClaudeKept::new(Kept {
            core: self.core.clone(),
            keeper,
            local_id: self.local_id.clone(),
            session_id: st.claude_session_id.clone(),
            saved: st.totals.clone(),
            first: None,
            session_file: Some(session_file_in(&self.core.deps.config_dir, &st.claude_session_id)),
        }))
    }
}

impl ClaudeSession {
    pub(crate) fn delegate(local_id: String, now: Arc<Mutex<Arc<ClaudeSession>>>, finished: SharedFuture<TurnResult>) -> ClaudeSession {
        ClaudeSession { inner: SessionInner::Lost(Rc::new(Lost { local_id, now, finished })) }
    }

    /// The turn running now: this turn, or for a start that may have resumed a lost session, the launch running now.
    fn live(&self) -> Rc<Turn> {
        match &self.inner {
            SessionInner::Turn(turn) => turn.clone(),
            SessionInner::Lost(lost) => {
                let now = lost.now.lock().unwrap().clone();
                now.live()
            }
        }
    }

    /// Whether both handles are the same turn (the TS `toBe`).
    pub fn same(&self, other: &ClaudeSession) -> bool {
        match (&self.inner, &other.inner) {
            (SessionInner::Turn(a), SessionInner::Turn(b)) => Rc::ptr_eq(a, b),
            (SessionInner::Lost(a), SessionInner::Lost(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// The registry key, fixed before spawn. adapter.ts:78.
    pub fn local_id(&self) -> String {
        match &self.inner {
            SessionInner::Turn(turn) => turn.local_id.clone(),
            SessionInner::Lost(lost) => lost.local_id.clone(),
        }
    }

    /// The id the CLI's init named; local_id until then. adapter.ts:80.
    pub fn claude_session_id(&self) -> String {
        self.live().claude_session_id()
    }

    /// The line the launch ran (after the sign-in guard and the saved-spend step it is not); `None` on an attach.
    pub fn command(&self) -> Option<String> {
        self.live().command.clone()
    }

    pub fn run(&self) -> Option<String> {
        self.live().stream.run()
    }

    pub fn pid(&self) -> Option<u32> {
        self.live().stream.pid()
    }

    pub fn from(&self) -> Option<u64> {
        match &self.inner {
            SessionInner::Turn(turn) => turn.from,
            SessionInner::Lost(_) => None,
        }
    }

    /// Settles once session.end has been emitted.
    pub fn finished(&self) -> SharedFuture<TurnResult> {
        match &self.inner {
            SessionInner::Turn(turn) => turn.finished.clone(),
            SessionInner::Lost(lost) => lost.finished.clone(),
        }
    }

    pub async fn interrupt(&self) {
        self.live().interrupt().await
    }

    pub fn kept(&self) -> Option<ClaudeKept> {
        self.live().kept()
    }

    /// adapter.ts:1169. `id` is a random uuid where `None`.
    pub async fn steer(&self, prompt: &str, id: Option<String>, images: Vec<TurnImage>) -> Result<SteerOutcome, ExecFailure> {
        self.live().steer(prompt, id, images).await
    }

    /// adapter.ts:1166: the CLI's init declared msg_lifecycle_v1, or it reported a command_lifecycle line.
    pub fn tells_unread(&self) -> bool {
        self.live().st.borrow().steers.reports()
    }

    pub async fn answer(&self, ask_id: &str, answer: PermissionAnswer) -> Result<AnswerOutcome, ExecFailure> {
        self.live().answer_ask(ask_id, &answer.option_id, answer.outcome, &answer.deny_message).await
    }

    pub async fn set_access(&self, mode: &str) -> Result<AccessOutcome, ExecFailure> {
        self.live().set_access(mode).await
    }

    pub async fn stop_task(&self, task: &str) -> Result<TaskStop, ExecFailure> {
        self.live().stop_task(task).await
    }
}

impl ClaudeKept {
    pub(crate) fn new(kept: Kept) -> ClaudeKept {
        ClaudeKept { inner: Rc::new(kept) }
    }

    /// The next turn on the process, its message written at once or once `turn.after` settles. adapter.ts:1274.
    pub fn next(&self, turn: KeptTurn) -> ClaudeSession {
        let k = &self.inner;
        let (stream, from) = k.keeper.turn();
        let session = follow(
            &k.core,
            FollowOptions {
                stream: stream.clone(),
                local_id: k.local_id.clone(),
                announced: false,
                fresh: k.first.is_some(),
                command: k.first.clone(),
                keeper: Some(k.keeper.clone()),
                from,
                saved: k.saved.clone(),
                steered: None,
                on_event: turn.on_event,
            },
        );
        let line = user_message_line(&turn.prompt, &k.session_id, &turn.images, None);
        match turn.after {
            None => fire(stream.write(line)),
            Some(after) => {
                let after = after.shared();
                if stream.write_after(line.clone(), after.clone().boxed_local()).is_none() {
                    tokio::task::spawn_local(async move {
                        if after.await.is_ok() {
                            let _ = stream.write(line).await;
                        }
                    });
                }
            }
        }
        session
    }

    /// `now`: wait 0 before the tree is ended; else result_exit_ms. adapter.ts:1261.
    pub async fn close(&self, now: bool) {
        let wait = if now { 0 } else { exit_ms(&self.inner.core) };
        self.inner.keeper.close(wait, grace_ms(&self.inner.core)).await
    }

    pub fn exited(&self) -> SharedFuture<Option<i32>> {
        self.inner.keeper.exited()
    }

    /// The session's file; `None` on a process warmed ahead of any turn.
    pub fn session_file(&self) -> Option<SessionFile> {
        self.inner.session_file.clone()
    }
}
