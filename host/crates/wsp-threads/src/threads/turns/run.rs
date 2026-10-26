// SPDX-License-Identifier: AGPL-3.0-only
//! turns.ts:17-290, 569-741: a turn's state, its row, its handle and its ends. Spec: spec/turns_run.md.
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use wsp_adapters::port::{
    AccessOutcome, AnswerOutcome, ExecFailure, ExecFailureKind, HarnessSession, OnEvent, PermissionAnswer, PermissionAsk, SteerOutcome,
    TaskStop, TurnImage,
};
use wsp_core::console;
use wsp_core::types::internal::{write_lines, CallSeen, EndRow, KeptLaunch, SessionEntry, TurnLive};
use wsp_core::types::wiring::{AccessMove, Local, SessionHandle, SteerTaken};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::{PermissionOutcome, SessionStatus, SessionView};
use wsp_wire::views::session_events::{TurnResult, TurnStatus};
use wsp_wire::views::session_events_more::{SessionEvent, SessionPermissionClosedEvent, SessionPermissionEvent};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_wire::wire::helpers::PermissionEffect;
use wsp_wire::wire::runtime::{SessionAnswerOutcome, SessionAnswerResult, SessionStartOutcome};
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;
use wsp_words::thread_read::lead_ask;
use wsp_words::words::computer::thread_word;
use wsp_words::words::tools::{asking_line, denied_line, permission_mode_option_label, picked_options};
use wsp_words::words::turn::{notify_body, turn_lines, NotifyLength};

use super::{Opening, SnapshotFrom, TurnSnapshot, TurnStart, TurnsArea, TurnsCtx};
use crate::threads::threads::{Entry, KeptHand, Row, Waiting};

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

/// Starts a TS `void` future: its body runs now up to its first wait, the rest on the local set.
pub(crate) fn detach(ctx: &Rc<dyn TurnsCtx>, mut work: LocalBoxFuture<'static, ()>) {
    let waker = futures::task::noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);
    if work.as_mut().poll(&mut cx).is_pending() {
        ctx.spawn(work);
    }
}

/// A session refusal as a host error: the transport's own message, marked with the class a silent computer's failure
/// was thrown as, which `computer_silent` reads (turns.ts:547 keeps the error itself).
pub(crate) fn failure(e: ExecFailure) -> HostError {
    let mark = match e.kind {
        ExecFailureKind::Unreached => Some("MachineUnreached"),
        ExecFailureKind::Unreachable => Some("MachineUnreachableError"),
        ExecFailureKind::PlaceAbsent => Some("PlaceAbsent"),
        ExecFailureKind::LaunchUnanswered => Some("LaunchUnanswered"),
        ExecFailureKind::Deadline => Some("DeadlineError"),
        ExecFailureKind::NoAnswer => Some("DaemonUnanswered"),
        ExecFailureKind::Other => None,
    };
    let mut error = HostError::new(e.message);
    error.marks.extend(mark.map(String::from));
    error
}

/// A turn result with only its status set (TS `{ status }`).
pub(crate) fn bare_result(status: TurnStatus) -> TurnResult {
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

/// The row status a turn's reply sets (TS `view.status = reply`).
pub(crate) fn session_status_of(status: TurnStatus) -> SessionStatus {
    match status {
        TurnStatus::Completed => SessionStatus::Completed,
        TurnStatus::Interrupted => SessionStatus::Interrupted,
        TurnStatus::Failed => SessionStatus::Failed,
    }
}

impl TurnState {
    /// The state a `TurnStart` opens with (turns.ts:68-170). Rules N1-N3.
    pub fn new(ctx: Rc<dyn TurnsCtx>, t: &TurnStart) -> Rc<Self> {
        let workspace_id = t.entry.borrow().record.borrow().id.clone();
        let live = t.row.borrow_mut().turn_live.get_or_insert_with(TurnLive::default).clone();
        let written = t.written.clone().unwrap_or_default();
        let counts = Counts {
            lines: written.lines,
            replaying_lines: written.lines,
            subagents: written.subagents,
            replaying_subagents: written.subagents,
            compactions: written.compactions,
            replaying_compactions: written.compactions,
            retries: written.retries,
            replaying_retries: written.retries,
            replaying_told: live.told.unwrap_or(0),
            start_recorded: written.started,
            reply_recorded: written.reply,
        };
        let row_id = t.row.borrow().view.id.clone();
        Rc::new_cyclic(|me: &Weak<TurnState>| {
            let me = me.clone();
            let write_held: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(state) = me.upgrade() {
                    state.write_held();
                }
            });
            ctx.held_at_close().borrow_mut().push(write_held.clone());
            TurnState {
                ctx: ctx.clone(),
                entry: t.entry.clone(),
                workspace_id,
                row: t.row.clone(),
                thread_id: t.thread_id.clone(),
                turn_id: t.turn_id.clone(),
                row_id: RefCell::new(row_id),
                notify: t.notify.clone(),
                notify_by: t.notify_by.clone(),
                notify_road: t.notify_road,
                turn_token: t.turn_token.clone(),
                scope_device_id: t.scope_device_id.clone(),
                outcome: t.outcome,
                opening: t.opening.clone(),
                images_dir: t.images_dir.clone(),
                snapshot: t.snapshot.clone(),
                waiting: t.waiting.clone(),
                keep: t.keep.clone(),
                reopened: t.written.is_some(),
                counts: RefCell::new(counts),
                wrote: t.reports_edits.then(|| RefCell::new(IndexSet::new())),
                changes_read: Cell::new(written.changes),
                changes_out: Cell::new(false),
                end_written: Cell::new(false),
                turn_account: RefCell::new(None),
                held: RefCell::new(None),
                result_held: Cell::new(false),
                ended: Cell::new(false),
                over: Cell::new(false),
                stopped: Cell::new(false),
                anchor: RefCell::new(None),
                kept_why: RefCell::new(None),
                open: RefCell::new(IndexMap::new()),
                tasks_running: Cell::new(false),
                waited: Cell::new(live.waited.map_or(0.0, |n| n.0)),
                waiting_since: Cell::new(live.waiting_since.map(|n| n.0)),
                unread: RefCell::new(Vec::new()),
                session: RefCell::new(None),
                write_held: RefCell::new(Some(write_held)),
            }
        })
    }

    /// The row the notify and cut roads read.
    pub fn row(&self) -> Row {
        self.row.clone()
    }

    /// The row's session id: the agent's own, else the row's id (TS `view.claudeSessionId ?? view.id`).
    pub(crate) fn session_id(&self) -> String {
        let row = self.row.borrow();
        row.view.claude_session_id.clone().unwrap_or_else(|| row.view.id.clone())
    }

    /// The live harness session, once the turn is open.
    pub(crate) fn session(&self) -> Option<Arc<dyn HarnessSession>> {
        self.session.borrow().clone()
    }

    /// TS `void ctx.persistSessions(workspaceId)`.
    pub(crate) fn persist(&self) {
        let ctx = self.ctx.clone();
        let workspace_id = self.workspace_id.clone();
        detach(
            &self.ctx,
            async move {
                let _ = ctx.persist_sessions(&workspace_id).await;
            }
            .boxed_local(),
        );
    }

    /// turns.ts:118. Rule N3.
    pub fn write_held(&self) {
        if self.result_held.get() {
            return;
        }
        let Some((context, window)) = *self.held.borrow() else {
            return;
        };
        self.ctx.record(SessionEvent::SessionContextEvent(wsp_wire::views::session_events_more::SessionContextEvent {
            workspace_id: self.workspace_id.clone(),
            session_id: self.session_id(),
            at: None,
            turn_id: Some(self.turn_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            pos: None,
            copied_from: None,
            context: JsNumber(context),
            window: window.map(JsNumber),
        }));
    }

    /// turns.ts:132. Rule N18.
    pub fn turn_over(self: &Rc<Self>) {
        if self.over.replace(true) {
            return;
        }
        if let Some(write) = self.write_held.borrow_mut().take() {
            self.ctx.held_at_close().borrow_mut().retain(|f| !Rc::ptr_eq(f, &write));
        }
        self.write_held();
        let threads = self.ctx.records();
        let undone: Vec<String> = {
            let mut records = threads.borrow_mut();
            let mut undone = Vec::new();
            for (id, record) in records.iter_mut() {
                if record.workspace_id != self.workspace_id || record.rewound.is_none() {
                    continue;
                }
                record.rewound = None;
                undone.push(id.clone());
            }
            undone
        };
        if !undone.is_empty() {
            self.persist();
            self.ctx.emit(EventUnion::ThreadMarked(Sequenced {
                event: wsp_wire::views::preferences::ThreadMarkedEvent { workspace_id: self.workspace_id.clone(), thread_ids: undone },
                seq: None,
            }));
        }
        let slates = self.ctx.clone();
        let (thread, turn) = (self.thread_id.clone(), self.turn_id.clone());
        detach(
            &self.ctx,
            async move {
                if let Err(e) = slates.slates_turn_ended(&thread, &turn).await {
                    console::warn(&format!(
                        "the slate of thread {} was not kept at the end of turn {turn}: {}",
                        thread_word(&thread),
                        e.message
                    ));
                }
            }
            .boxed_local(),
        );
        let kept = self.ctx.keep_checkpoint(
            &self.entry,
            crate::threads::threads::CheckpointAsk {
                session_id: self.session_id(),
                thread_id: self.thread_id.clone(),
                turn_id: self.turn_id.clone(),
                anchor: self.anchor.borrow().clone(),
                kept: self.kept_why.borrow().clone(),
            },
        );
        let landing = self.ctx.checkpoints_landing();
        landing.borrow_mut().insert(self.thread_id.clone(), kept.clone());
        let thread = self.thread_id.clone();
        detach(
            &self.ctx,
            async move {
                kept.clone().await;
                let mut map = landing.borrow_mut();
                if map.get(&thread).is_some_and(|k| k.ptr_eq(&kept)) {
                    map.shift_remove(&thread);
                }
            }
            .boxed_local(),
        );
    }

    /// turns.ts:80. Rule N19.
    pub fn read_changes(self: &Rc<Self>, session_id: &str) {
        if self.changes_read.get() {
            return;
        }
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        self.changes_read.set(true);
        self.changes_out.set(true);
        let started_at = self.row.borrow().view.started_at.map_or_else(|| self.ctx.clock().now(), |n| n.0 as i64);
        let wrote = self.wrote.as_ref().map(|w| w.borrow().iter().cloned().collect::<BTreeSet<String>>());
        let state = self.clone();
        let session_id = session_id.to_string();
        self.ctx.spawn(Box::pin(async move {
            let from = match snapshot.from {
                SnapshotFrom::Known(commit) => commit,
                SnapshotFrom::Taking(commit) => commit.await,
            };
            let read = match from {
                None => false,
                Some(from) => {
                    let ask = crate::threads::threads::TurnChangesAsk {
                        session_id,
                        turn_id: state.turn_id.clone(),
                        thread_id: state.thread_id.clone(),
                        cwd: snapshot.cwd,
                        from,
                        started_at,
                        wrote,
                    };
                    match state.ctx.read_turn_changes(&state.entry, ask).await {
                        Ok(read) => read,
                        Err(_) => return,
                    }
                }
            };
            state.changes_out.set(false);
            let row_id = state.row_id.borrow().clone();
            let current = state.ctx.sessions().borrow().get(&row_id).cloned();
            let Some(current) = current else {
                return;
            };
            {
                let mut row = current.borrow_mut();
                if !state.end_written.get() || row.turn_id != state.turn_id || row.snapshot.is_none() {
                    return;
                }
                row.snapshot = None;
            }
            if read || !state.ctx.closing() {
                state.persist();
            }
        }));
    }

    /// turns.ts:181. Rule N8.
    pub fn waited_so_far(&self) -> f64 {
        let on_the_person = self.waiting_since.get().map_or(0.0, |since| self.ctx.clock().now() as f64 - since);
        self.waited.get() + on_the_person
    }

    /// turns.ts:185. Rule N8.
    pub fn with_waited(&self, reply: TurnResult) -> TurnResult {
        let on_the_person = self.waited_so_far();
        if on_the_person > 0.0 {
            TurnResult { waited_ms: Some(JsNumber(on_the_person)), ..reply }
        } else {
            reply
        }
    }

    /// turns.ts:194. Rule N9.
    pub fn after_told(&self, result: &TurnResult, held: bool) -> Option<TurnResult> {
        let Some(told) = self.row.borrow().turn_live.as_ref().and_then(|live| live.told_as.clone()) else {
            return if held { None } else { Some(result.clone()) };
        };
        if !held && notify_body(result, Some(NotifyLength::Whole)) != told.body {
            return Some(result.clone());
        }
        let outcome = TurnResult { text: None, error: None, ..result.clone() };
        let added = match (held, told.said.as_deref(), result.text.as_deref()) {
            (true, Some(said), Some(text)) if text.starts_with(said) => js_trim(&text[said.len()..]).to_string(),
            _ => String::new(),
        };
        if !added.is_empty() {
            return Some(TurnResult { text: Some(added), ..outcome });
        }
        if result.status == told.status && !told.promised {
            None
        } else {
            Some(outcome)
        }
    }

    /// turns.ts:207. Rule N4.
    pub fn reads_waiting(&self) {
        let on = !self.open.borrow().is_empty() || self.tasks_running.get();
        if let Some(waiting) = &self.waiting {
            waiting.set(on);
        }
    }

    pub(crate) fn permission_event(&self, session_id: &str, ask: &PermissionAsk) -> SessionPermissionEvent {
        SessionPermissionEvent {
            workspace_id: self.workspace_id.clone(),
            session_id: session_id.to_string(),
            at: None,
            turn_id: Some(self.turn_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            pos: None,
            copied_from: None,
            ask_id: ask.ask_id.clone(),
            tool_name: ask.tool_name.clone(),
            tool_use_id: ask.tool_use_id.clone(),
            parent_tool_use_id: ask.parent_tool_use_id.clone(),
            input: ask.input.clone(),
            detail: ask.detail.clone(),
            options: ask.options.clone(),
        }
    }

    /// turns.ts:215. Rule N4.
    pub fn reads_open(&self) {
        let lead = lead_ask(self.open.borrow().values().cloned());
        match &lead {
            None => {
                self.row.borrow_mut().view.asking = None;
                self.ctx.lead_asks().borrow_mut().shift_remove(&self.thread_id);
            }
            Some(lead) => {
                let asking = asking_line(&self.permission_event(&self.session_id(), lead));
                self.row.borrow_mut().view.asking = Some(asking);
                self.ctx.lead_asks().borrow_mut().insert(self.thread_id.clone(), lead.clone());
            }
        }
        self.reads_waiting();
        let now = self.ctx.clock().now();
        if !self.open.borrow().is_empty() {
            if self.waiting_since.get().is_none() {
                self.waiting_since.set(Some(now as f64));
            }
        } else if let Some(since) = self.waiting_since.get() {
            self.waited.set(self.waited.get() + (now as f64 - since));
            self.waiting_since.set(None);
        }
        {
            let mut row = self.row.borrow_mut();
            let live = row.turn_live.get_or_insert_with(TurnLive::default);
            live.waited = Some(JsNumber(self.waited.get()));
            live.waiting_since = self.waiting_since.get().map(JsNumber);
        }
        self.persist();
    }

    /// turns.ts:238. Rule N5.
    pub fn named(&self, ask: &PermissionAsk) -> PermissionAsk {
        let harness = self.row.borrow().view.harness.clone();
        let mut named = ask.clone();
        for option in named.options.iter_mut() {
            if option.effect != PermissionEffect::Mode {
                continue;
            }
            if let Some(mode) = option.mode.clone() {
                let label = self.ctx.mode_label(&harness, &mode).unwrap_or(mode);
                option.label = permission_mode_option_label(&label);
            }
        }
        named
    }

    /// turns.ts:248. Rule N6.
    pub fn close_ask(&self, ask_id: &str, outcome: PermissionOutcome, option_id: Option<&str>) {
        if self.open.borrow().get(ask_id).is_none() {
            return;
        }
        self.open.borrow_mut().shift_remove(ask_id);
        self.reads_open();
        self.ctx.record(SessionEvent::SessionPermissionClosedEvent(SessionPermissionClosedEvent {
            workspace_id: self.workspace_id.clone(),
            session_id: self.session_id(),
            at: None,
            turn_id: Some(self.turn_id.clone()),
            thread_id: Some(self.thread_id.clone()),
            pos: None,
            copied_from: None,
            ask_id: ask_id.to_string(),
            outcome,
            option_id: option_id.map(String::from),
        }));
    }

    /// turns.ts:265. Rule N6.
    pub fn close_open_asks(&self) {
        let ids: Vec<String> = self.open.borrow().keys().cloned().collect();
        for id in ids {
            self.close_ask(&id, PermissionOutcome::Cancelled, None);
        }
    }

    /// turns.ts:271. Rule N7.
    pub async fn answer(&self, ask_id: &str, option_id: &str, reason: Option<&str>) -> SessionAnswerResult {
        let gone = SessionAnswerResult { outcome: SessionAnswerOutcome::Gone };
        let held = self.open.borrow().get(ask_id).cloned();
        let Some(held) = held else {
            return gone;
        };
        let option = picked_options(&held.options, option_id).and_then(|options| options.into_iter().next());
        let Some(option) = option else {
            return SessionAnswerResult { outcome: SessionAnswerOutcome::NoOption };
        };
        let Some(session) = self.session() else {
            return gone;
        };
        let deny = option.effect == PermissionEffect::Deny;
        let outcome = if deny { PermissionOutcome::Denied } else { PermissionOutcome::Allowed };
        let reason = reason.map(js_trim).filter(|r| !r.is_empty()).filter(|_| deny).map(String::from);
        let answer = PermissionAnswer { option_id: option_id.to_string(), outcome, deny_message: denied_line(reason.as_deref()), reason };
        let Some(pending) = session.answer(ask_id.to_string(), answer) else {
            return gone;
        };
        match pending.await {
            Ok(AnswerOutcome::Answered) => SessionAnswerResult { outcome: SessionAnswerOutcome::Answered },
            _ => gone,
        }
    }

    /// turns.ts:290: the turn's line's try (`ctx.lineTry`).
    pub fn line_try(&self) -> bool {
        self.ctx.line_try(&self.row)
    }

    /// turns.ts:634. Rule N17.
    pub fn end(self: &Rc<Self>, reason: &str, by_stop: bool, unreached: bool) {
        if self.ended.get() || self.row.borrow().view.status != SessionStatus::Running {
            return;
        }
        if by_stop {
            self.stopped.set(true);
        }
        self.close_open_asks();
        self.ended.set(true);
        let row_id = self.row_id.borrow().clone();
        let current = self.ctx.sessions().borrow().get(&row_id).cloned();
        if let Some(current) = current {
            let mut row = current.borrow_mut();
            if row.turn_id == self.turn_id {
                row.snapshot = None;
            }
        }
        self.ctx.settle_cut(&self.row, reason, by_stop, unreached);
        self.persist();
        if let Some(session) = self.session() {
            detach(&self.ctx, session.interrupt());
        }
        self.turn_over();
    }

    /// turns.ts:671. Rules N20-N25.
    pub fn settled(self: &Rc<Self>, status: TurnStatus) {
        let row_id = self.row_id.borrow().clone();
        let current = self.ctx.sessions().borrow().get(&row_id).cloned();
        if let Some(current) = current {
            let mut row = current.borrow_mut();
            if row.turn_token == self.turn_token {
                row.turn_token = None;
            }
            if row.turn_id == self.turn_id && !self.changes_out.get() {
                row.snapshot = None;
            }
        }
        let session = self.session();
        let agent = if self.keep.is_some() { session.as_ref().and_then(|s| s.kept()).flatten() } else { None };
        let mut keeps = false;
        if let Some(agent) = agent {
            let session_id = self.row.borrow().view.claude_session_id.clone();
            match (self.ended.get(), self.turn_token.clone(), session_id, self.keep.clone()) {
                (false, Some(turn_token), Some(session_id), Some(launch)) => {
                    let hold = KeptHand {
                        workspace_id: self.workspace_id.clone(),
                        agent,
                        pid: session.as_ref().and_then(|s| s.pid()),
                        launch,
                        session: Some(session_id),
                        warm: None,
                        turn_token,
                        scope_device_id: self.scope_device_id.clone(),
                        waiting: self.waiting.clone().unwrap_or_else(|| Rc::new(Cell::new(false))),
                    };
                    match self.ctx.hold_kept(&self.thread_id, hold) {
                        None => keeps = true,
                        Some(back) => detach(&self.ctx, back.agent.close(false)),
                    }
                }
                _ => detach(&self.ctx, agent.close(false)),
            }
        }
        if let Some(device) = self.scope_device_id.clone().filter(|_| !keeps) {
            let ctx = self.ctx.clone();
            let thread = self.thread_id.clone();
            detach(
                &self.ctx,
                async move {
                    if let Err(e) = ctx.revoke_device(&device).await {
                        console::warn(&format!("the token of thread {} was not taken away: {}", thread_word(&thread), e.message));
                    }
                }
                .boxed_local(),
            );
        }
        let now = self.ctx.clock().now() as f64;
        if !self.ended.get() {
            self.row.borrow_mut().view.status = session_status_of(status);
            if self.live_reply().is_none() {
                let lines = turn_lines(&bare_result(status), None);
                write_lines(&mut self.row.borrow_mut().view, lines.last_line, lines.failure);
            }
        }
        self.ctx.port_roots_moved(&self.workspace_id);
        {
            let mut row = self.row.borrow_mut();
            if row.view.ended_at.is_none() {
                row.view.ended_at = Some(JsNumber(now));
            }
        }
        let view = self.row.borrow().view.clone();
        if view.status == SessionStatus::Failed {
            self.ctx.end_snooze_for(&view);
        }
        let kept_mode = self.ctx.records().borrow().get(&self.thread_id).and_then(|record| record.permission_mode.clone());
        {
            let mut row = self.row.borrow_mut();
            if let Some(mode) = kept_mode {
                row.view.permission_mode = Some(mode);
            }
            if let Some(calls) = row.calls.as_mut() {
                calls.clear();
            }
        }
        self.ctx.lead_asks().borrow_mut().shift_remove(&self.thread_id);
        self.persist();
        self.end_written.set(true);
        if !self.ended.get() {
            let ctx = self.ctx.clone();
            let row = self.row.clone();
            detach(
                &self.ctx,
                async move {
                    let _ = ctx.refresh_title(&row, true).await;
                }
                .boxed_local(),
            );
            self.read_ends();
        }
        if let Some(dir) = &self.images_dir {
            self.ctx.drop_images(&self.entry, dir);
        }
    }

    pub(crate) fn live_reply(&self) -> Option<TurnStatus> {
        self.row.borrow().turn_live.as_ref().and_then(|live| live.reply)
    }

    /// turns.ts:709-723: the branch, the pull request and the lead read at a turn's end, then the tree's settle again.
    fn read_ends(self: &Rc<Self>) {
        let state = self.clone();
        detach(
            &self.ctx,
            async move {
                let read: Result<(), HostError> = async {
                    state.ctx.read_checkout(&state.entry, true).await?;
                    state.ctx.ran_on_branch(&state.entry, &state.thread_id).await?;
                    state.ctx.read_pull_request(&state.entry, true).await?;
                    state.ctx.read_lead_of(&state.entry).await
                }
                .await;
                if let Err(e) = read {
                    let name = state.entry.borrow().record.borrow().name.clone();
                    console::warn(&format!("the branch of {name} was not read at a turn's end: {}", e.message));
                }
                state.settle_again();
            }
            .boxed_local(),
        );
    }

    /// turns.ts:717-722: the root's workspace settles again when its pull request has settled.
    fn settle_again(&self) {
        let root_thread = self.row.borrow().view.root_thread_id.clone();
        let root = match root_thread {
            None => Some(self.entry.clone()),
            Some(thread) => self.ctx.latest_on(&thread).and_then(|view| self.ctx.live_entry(&view.workspace_id)),
        };
        let Some(root) = root else {
            return;
        };
        if self.ctx.pull_request_state(&root).is_some_and(|state| state != "open") {
            let ctx = self.ctx.clone();
            detach(
                &self.ctx,
                async move {
                    let _ = ctx.settle_tree(&root).await;
                }
                .boxed_local(),
            );
        }
    }
}

/// The handle a turn answers (turns.ts:617-633).
pub struct TurnHandle {
    pub(crate) state: Weak<TurnState>,
    /// The caller's copy keeps the turn's state; the row's own copy holds it only weakly, since the state holds the row.
    pub(crate) _keeps: Option<Rc<TurnState>>,
    pub(crate) row: Weak<RefCell<SessionEntry>>,
    /// The view as the handle was made, for a handle that outlives both its row and its turn.
    pub(crate) seen: SessionView,
    pub(crate) id: String,
    pub(crate) workspace_id: String,
    pub(crate) turn_id: String,
    pub(crate) outcome: SessionStartOutcome,
    pub(crate) finished: Shared<LocalBoxFuture<'static, TurnResult>>,
}

impl TurnHandle {
    fn st(&self) -> Option<Rc<TurnState>> {
        self.state.upgrade()
    }
}

impl SessionHandle for TurnHandle {
    fn id(&self) -> String {
        self.st().map(|s| s.row_id.borrow().clone()).unwrap_or_else(|| self.id.clone())
    }
    fn workspace_id(&self) -> String {
        self.workspace_id.clone()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.finished.clone()
    }
    fn turn_id(&self) -> String {
        self.turn_id.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        self.outcome
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        match (self.row.upgrade(), self.st()) {
            (Some(row), _) => row.borrow().view.clone(),
            (None, Some(state)) => state.row.borrow().view.clone(),
            (None, None) => self.seen.clone(),
        }
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let Some(state) = self.st() else {
            return futures::future::ready(()).boxed_local();
        };
        state.stopped.set(true);
        match state.session() {
            Some(session) => session.interrupt(),
            None => futures::future::ready(()).boxed_local(),
        }
    }
    fn steer(&self, prompt: String, id: Option<String>, images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        let pending = self.st()?.session()?.steer(prompt, id, images)?;
        Some(
            async move {
                pending
                    .await
                    .map(|outcome| match outcome {
                        SteerOutcome::Accepted => SteerTaken::Accepted,
                        SteerOutcome::NotRunning => SteerTaken::NotRunning,
                    })
                    .map_err(failure)
            }
            .boxed_local(),
        )
    }
    fn tells_unread(&self) -> Option<bool> {
        Some(self.st().and_then(|s| s.session()).is_some_and(|session| session.tells_unread() == Some(true)))
    }
    fn answer(&self, ask_id: String, option_id: String, reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        let session = self.st()?.session()?;
        let state = self.st()?;
        if !state.ctx.session_answers(session.as_ref()) {
            return None;
        }
        Some(async move { Ok(state.answer(&ask_id, &option_id, reason.as_deref()).await) }.boxed_local())
    }
    fn set_access(&self, mode: String) -> Option<Local<'static, AccessMove>> {
        let state = self.st()?;
        let pending = state.session()?.set_access(mode.clone())?;
        Some(
            async move {
                let outcome = pending.await.map_err(failure)?;
                Ok(match outcome {
                    AccessOutcome::Set => {
                        state.row.borrow_mut().view.permission_mode = Some(mode);
                        AccessMove::Set
                    }
                    AccessOutcome::Refused => AccessMove::Refused,
                    AccessOutcome::Gone => AccessMove::Gone,
                })
            }
            .boxed_local(),
        )
    }
    fn stop_task(&self, task: String) -> Option<Local<'static, TaskStop>> {
        let pending = self.st()?.session()?.stop_task(task)?;
        Some(async move { pending.await.map_err(failure) }.boxed_local())
    }
}

impl TurnsArea {
    /// turns.ts:19. Rules N10-N16, N26; the switch over events is `TurnState::forward`.
    pub fn run_turn(&self, t: TurnStart) -> Result<Rc<dyn SessionHandle>, HostError> {
        let ctx = self.ctx.clone();
        let state = TurnState::new(ctx.clone(), &t);
        let workspace_id = state.workspace_id.clone();
        state.reads_waiting();
        ctx.idle_hold(&workspace_id);
        let TurnStart { open, resume, .. } = t;
        state.row.borrow_mut().calls = Some(IndexMap::<String, CallSeen>::new());
        let forward_state = Rc::downgrade(&state);
        let on_event: OnEvent = Arc::new(move |event| {
            if let Some(state) = forward_state.upgrade() {
                state.forward(event)
            }
        });
        let started = match open(on_event) {
            Ok(started) => started,
            Err(e) => {
                ctx.idle_release(&workspace_id);
                return Err(e);
            }
        };
        ctx.arm_disk_watch();
        let finished = started.finished();
        let disk_finished = finished.clone();
        let (disk_ctx, disk_entry, disk_ws) = (ctx.clone(), state.entry.clone(), workspace_id.clone());
        detach(
            &ctx,
            async move {
                disk_finished.await;
                disk_ctx.idle_release(&disk_ws);
                if let Err(e) = disk_ctx.read_disk(&disk_entry).await {
                    console::warn(&format!("disk of {disk_ws} not read after its turn: {}", e.message));
                }
            }
            .boxed_local(),
        );
        *state.session.borrow_mut() = Some(started.clone());
        let handle_id = started.local_id();
        {
            let mut row = state.row.borrow_mut();
            if row.view.id.is_empty() || row.view.id == state.turn_id {
                row.view.id = handle_id.clone();
            }
        }
        let row_id = state.row.borrow().view.id.clone();
        *state.row_id.borrow_mut() = row_id.clone();
        if resume.is_some() {
            let sessions = ctx.sessions();
            let resumed = sessions.borrow().get(&handle_id).map(|row| {
                let row = row.borrow();
                (row.view.started_by, row.view.prompt.clone(), row.view.cost_usd)
            });
            if let Some((started_by, prompt, cost)) = resumed {
                let mut row = state.row.borrow_mut();
                row.view.started_by = started_by.or(row.view.started_by);
                if let Some(prompt) = prompt {
                    row.view.prompt = Some(prompt);
                }
                if let Some(cost) = cost {
                    row.view.cost_usd = Some(cost);
                }
            }
        }
        let handle_of = |keeps: bool| -> Rc<dyn SessionHandle> {
            Rc::new(TurnHandle {
                state: Rc::downgrade(&state),
                _keeps: keeps.then(|| state.clone()),
                row: Rc::downgrade(&state.row),
                seen: state.row.borrow().view.clone(),
                id: row_id.clone(),
                workspace_id: workspace_id.clone(),
                turn_id: state.turn_id.clone(),
                outcome: state.outcome,
                finished: finished.clone(),
            })
        };
        let handle = handle_of(true);
        let on_row = handle_of(false);
        let end_state = Rc::downgrade(&state);
        let end: EndRow = Rc::new(move |reason: &str, by_stop: Option<bool>, unreached: Option<bool>| {
            if let Some(state) = end_state.upgrade() {
                state.end(reason, by_stop.unwrap_or(false), unreached.unwrap_or(false));
            }
        });
        let sessions = ctx.sessions();
        if state.turn_id != row_id {
            sessions.borrow_mut().shift_remove(&state.turn_id);
        }
        {
            let mut row = state.row.borrow_mut();
            row.turn_id = state.turn_id.clone();
            row.notify = state.notify.clone();
            row.notify_by = state.notify_by.clone();
            row.notify_road = state.notify_road;
            row.turn_token = state.turn_token.clone();
            row.scope_device_id = state.scope_device_id.clone();
            row.handle = Some(on_row);
            row.end = Some(end);
            row.replaces = None;
            row.launch = None;
            row.unanswered = false;
            row.queued = false;
            row.snapshot = match &state.snapshot {
                Some(TurnSnapshot { from: SnapshotFrom::Known(Some(commit)), .. }) => Some(commit.clone()),
                _ => None,
            };
            if let Some(run) = started.run() {
                row.run = Some(run);
            }
            if let Some(from) = started.from() {
                row.from = Some(from);
            }
            if let Some(pid) = started.pid() {
                row.pid = Some(pid);
            }
        }
        sessions.borrow_mut().insert(row_id.clone(), state.row.clone());
        if let Some(pid) = started.pid() {
            if ctx.shared_daemon(&state.entry) {
                let groups = ctx.turn_groups();
                groups.borrow_mut().entry(workspace_id.clone()).or_default().insert(pid, state.thread_id.clone());
                ctx.arm_roots_recheck();
            }
        }
        ctx.port_roots_moved(&workspace_id);
        state.persist();
        if let Some(TurnSnapshot { from: SnapshotFrom::Taking(commit), .. }) = &state.snapshot {
            let (commit, landing) = (commit.clone(), state.clone());
            let (sessions, turn_id) = (sessions.clone(), state.turn_id.clone());
            detach(
                &ctx,
                async move {
                    let commit = commit.await;
                    let row = sessions.borrow().get(&row_id).cloned();
                    let (Some(commit), Some(row)) = (commit, row) else {
                        return;
                    };
                    let mut row = row.borrow_mut();
                    if row.turn_id != turn_id || row.view.status != SessionStatus::Running {
                        return;
                    }
                    row.snapshot = Some(commit);
                    drop(row);
                    landing.persist();
                }
                .boxed_local(),
            );
        }
        let settle_state = state.clone();
        detach(
            &ctx,
            async move {
                let result = finished.await;
                settle_state.settled(result.status);
                let unread = settle_state.unread.borrow().clone();
                settle_state.ctx.send_back(&settle_state.row, unread, settle_state.stopped.get());
                if !settle_state.ended.get() && result.status == TurnStatus::Completed {
                    let text = result.text.clone().unwrap_or_default();
                    if let Err(e) = settle_state.ctx.take_review(&settle_state.entry, &settle_state.thread_id, &text).await {
                        let name = settle_state.entry.borrow().record.borrow().name.clone();
                        console::warn(&format!("the review in {name} was not read: {}", e.message));
                    }
                }
            }
            .boxed_local(),
        );
        Ok(handle)
    }
}
