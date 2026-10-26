// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:41-207, 977-990, 1482-1485: a stop on a turn and the holds on a thread's next turn.
//! Spec: spec/sessions_stop.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::FutureExt;
use tokio::task::spawn_local;
use wsp_adapters::port::TaskStop;
use wsp_core::clock::Cancel;
use wsp_core::console;
use wsp_core::context::Joined;
use wsp_core::types::internal::{stop_log_line, EndThread, SessionEntry, ThreadRecord};
use wsp_machines::machines::boot::entries::RowAt;
use wsp_wire::views::session::{SessionStatus, SessionView};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::limits::STOP_REACH_MS;
use wsp_wire::wire::runtime::{task_stop_refused_line, task_stop_unsupported_line, SessionInterruptOutcome, SessionInterruptResult};
use wsp_words::exit::HostError;
use wsp_words::place_state::cap_stopped_line;
use wsp_words::words::thread::TURN_STOPPED_LINE;

use super::{SessionsArea, StopState};
use crate::threads::threads::{Entry, Row, Sessions};
use crate::threads::trimmed_row::trimmed_row;

/// What `end_turn` answers (sessions.ts:151).
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub outcome: SessionInterruptOutcome,
    pub left: Option<String>,
}

/// A stop's answer to the caller: the outcome, and the tree or words it carries where it has any.
fn reply(outcome: SessionInterruptOutcome, under: Vec<String>, error: Option<String>, left: Option<String>) -> SessionInterruptResult {
    SessionInterruptResult { outcome, under: (!under.is_empty()).then_some(under), error, left }
}

fn outcome_word(outcome: SessionInterruptOutcome) -> &'static str {
    match outcome {
        SessionInterruptOutcome::Accepted => "accepted",
        SessionInterruptOutcome::NotRunning => "not-running",
        SessionInterruptOutcome::NotFound => "not-found",
        SessionInterruptOutcome::Refused => "refused",
        SessionInterruptOutcome::Unsupported => "unsupported",
    }
}

fn find_row(sessions: &Sessions, id: &str) -> Option<Row> {
    let by_key = sessions.borrow().get(id).cloned();
    by_key.or_else(|| {
        let rows = sessions.borrow();
        rows.values().find(|r| r.borrow().turn_id == id).cloned()
    })
}

/// A row for a thread the runtime holds only as its record: no handle, no launch, no end.
fn trimmed_entry(view: SessionView, turn_id: &str) -> Row {
    Rc::new(RefCell::new(SessionEntry {
        view,
        turn_id: turn_id.to_string(),
        replaces: None,
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        handle: None,
        end: None,
        turn_live: None,
        run: None,
        from: None,
        asked: None,
        snapshot: None,
        pid: None,
        launch: None,
        calls: None,
        unanswered: false,
        queued: false,
    }))
}

fn row_at(view: &SessionView) -> RowAt {
    RowAt { thread_id: view.thread_id.clone(), workspace_id: view.workspace_id.clone() }
}

/// Forgets a thread's hold once its gate is the one standing (sessions.ts:61-66).
fn forget_hold(stops: &RefCell<StopState>, thread_id: &str, gate: &Joined) {
    let mut state = stops.borrow_mut();
    if state.ending.get(thread_id).is_some_and(|g| g.ptr_eq(gate)) {
        state.ending.shift_remove(thread_id);
        state.waits_for.shift_remove(thread_id);
    }
}

/// One stop of the reach question (sessions.ts:74): settles once, and takes its timer and listener off.
struct Reach {
    done: Cell<bool>,
    answer: RefCell<Option<oneshot::Sender<bool>>>,
    quiet: RefCell<Option<Cancel>>,
    off: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Reach {
    fn settle(&self, away: bool) {
        if self.done.replace(true) {
            return;
        }
        let quiet = self.quiet.borrow_mut().take();
        if let Some(quiet) = quiet {
            quiet();
        }
        let off = self.off.borrow_mut().take();
        if let Some(off) = off {
            off();
        }
        let answer = self.answer.borrow_mut().take();
        if let Some(answer) = answer {
            let _ = answer.send(away);
        }
    }
}

impl SessionsArea {
    /// sessions.ts:55. Rule K1.
    pub fn hold_next(&self, thread_id: &str, until: Joined, box_name: Option<&str>) {
        let prior = self.stops.borrow().ending.get(thread_id).cloned();
        let gate: Joined = match prior {
            None => until,
            Some(prior) => async move {
                prior.await;
                until.await;
            }
            .boxed_local()
            .shared(),
        };
        self.stops.borrow_mut().ending.insert(thread_id.to_string(), gate.clone());
        if let Some(box_name) = box_name {
            self.stops.borrow_mut().waits_for.insert(thread_id.to_string(), box_name.to_string());
            let wakes: Vec<Rc<dyn Fn()>> = self.stops.borrow().behind_turn.get(thread_id).map(|sends| sends.iter().map(|(_, wake)| wake.clone()).collect()).unwrap_or_default();
            for wake in wakes {
                wake();
            }
        }
        let stops = self.stops.clone();
        let id = thread_id.to_string();
        let mine = gate.clone();
        spawn_local(async move {
            mine.clone().await;
            forget_hold(&stops, &id, &mine);
        });
    }

    /// sessions.ts:74. Rule K2.
    pub async fn stop_unheard(&self, place: &str, stopping: Option<Joined>) -> bool {
        if self.ctx.place_away(place) || !self.ctx.has_link(place) {
            return true;
        }
        let (tx, rx) = oneshot::channel::<bool>();
        let reach = Rc::new(Reach { done: Cell::new(false), answer: RefCell::new(Some(tx)), quiet: RefCell::new(None), off: RefCell::new(None) });
        let shown = reach.clone();
        let quiet = self.ctx.clock().schedule(Box::new(move || shown.settle(true)), STOP_REACH_MS);
        *reach.quiet.borrow_mut() = Some(quiet);
        let shown = reach.clone();
        let off = self.ctx.on_place_absent(place, Rc::new(move || shown.settle(true)));
        *reach.off.borrow_mut() = Some(off);
        let ctx = self.ctx.clone();
        let shown = reach.clone();
        let place_owned = place.to_string();
        let has_stop = stopping.is_some();
        spawn_local(async move {
            match ctx.ping(&place_owned).await {
                Ok(()) => {
                    let quiet = shown.quiet.borrow_mut().take();
                    if let Some(quiet) = quiet {
                        quiet();
                    }
                    if !has_stop {
                        shown.settle(false);
                    }
                }
                Err(_) => shown.settle(true),
            }
        });
        if let Some(stopping) = stopping {
            let ctx = self.ctx.clone();
            let shown = reach.clone();
            let place_owned = place.to_string();
            spawn_local(async move {
                stopping.await;
                let away = ctx.place_away(&place_owned);
                shown.settle(away);
            });
        }
        rx.await.unwrap_or(false)
    }

    /// sessions.ts:102. `marked` lets a held turn's next look go on. Rules K3-K9.
    pub async fn stop_turn(&self, marked: Rc<dyn Fn()>, row_id: &str, origin: Option<&Caller>, task: Option<String>) -> Result<SessionInterruptResult, HostError> {
        self.ctx.ready().await?;
        let sessions = self.ctx.sessions();
        let found = find_row(&sessions, row_id);
        let trimmed = match found {
            Some(_) => None,
            None => {
                let records = self.ctx.records();
                let held: Option<ThreadRecord> = records.borrow().get(row_id).cloned();
                held
            }
        };
        let s = match (found, trimmed) {
            (Some(row), _) => row,
            (None, Some(record)) => trimmed_entry(trimmed_row(&*self.ctx, row_id, &record, None), row_id),
            (None, None) => return Ok(reply(SessionInterruptOutcome::NotFound, Vec::new(), None, None)),
        };
        let view = s.borrow().view.clone();
        if self.ctx.entry_of_row(&row_at(&view), origin, None, false).await?.is_none() {
            return Ok(reply(SessionInterruptOutcome::NotFound, Vec::new(), None, None));
        }
        let turn_id = s.borrow().turn_id.clone();
        if task.is_none() {
            let waiting = self.stops.borrow().held_sends.get(row_id).cloned();
            if let Some(waiting) = waiting {
                return Ok(reply(SessionInterruptOutcome::Accepted, Vec::new(), None, Some((waiting.give_up)())));
            }
        }
        if let Some(task) = task.clone() {
            let (running, handle, harness) = {
                let r = s.borrow();
                (r.view.status == SessionStatus::Running, r.handle.clone(), r.view.harness.clone())
            };
            let handle = match handle {
                Some(handle) if running => handle,
                _ => return Ok(reply(SessionInterruptOutcome::NotRunning, Vec::new(), None, None)),
            };
            let agent = self.ctx.agent_label(&harness);
            let Some(stopped) = handle.stop_task(task) else {
                return Ok(reply(SessionInterruptOutcome::Unsupported, Vec::new(), Some(task_stop_unsupported_line(&agent)), None));
            };
            return Ok(match stopped.await? {
                TaskStop::Refused { error } => reply(SessionInterruptOutcome::Refused, Vec::new(), Some(task_stop_refused_line(&agent, &error)), None),
                TaskStop::Unsupported => reply(SessionInterruptOutcome::Unsupported, Vec::new(), Some(task_stop_unsupported_line(&agent)), None),
                TaskStop::Accepted => reply(SessionInterruptOutcome::Accepted, Vec::new(), None, None),
                TaskStop::NotRunning => reply(SessionInterruptOutcome::NotRunning, Vec::new(), None, None),
            });
        }
        let held = self.ctx.cap_held(row_id);
        if let Some(held) = &held {
            self.ctx.cap_stop(row_id, &cap_stopped_line(held.waiting.as_ref().map(|w| &w.wait)));
        }
        marked();
        if self.ctx.drop_queued(&turn_id).await? {
            return Ok(reply(SessionInterruptOutcome::Accepted, Vec::new(), None, None));
        }
        let under = match &view.thread_id {
            Some(thread) => self.ctx.stop_under(thread, origin).await?,
            None => Vec::new(),
        };
        if held.is_some() {
            let launch = s.borrow().launch.clone();
            if let Some(launch) = launch {
                launch.await;
            }
            return Ok(reply(SessionInterruptOutcome::Accepted, under, None, None));
        }
        self.ctx.try_stopped(&turn_id);
        let row_of: Rc<dyn Fn() -> Option<Row>> = {
            let sessions = sessions.clone();
            let id = row_id.to_string();
            Rc::new(move || find_row(&sessions, &id))
        };
        let ended = self.end_turn(s, origin, row_of).await?;
        Ok(reply(ended.outcome, under, None, ended.left))
    }

    /// sessions.ts:151. `row_of` reads the row again by its key. Rules K10-K16.
    pub async fn end_turn(&self, row: Row, origin: Option<&Caller>, row_of: Rc<dyn Fn() -> Option<Row>>) -> Result<Ended, HostError> {
        let launch = {
            let r = row.borrow();
            if r.view.status == SessionStatus::Running && r.handle.is_none() {
                r.launch.clone()
            } else {
                None
            }
        };
        let row = match launch {
            Some(launch) => {
                launch.await;
                row_of().unwrap_or(row)
            }
            None => row,
        };
        let (thread, at) = {
            let r = row.borrow();
            (r.view.thread_id.clone(), row_at(&r.view))
        };
        let entry = match &thread {
            Some(_) => self.ctx.entry_of_row(&at, origin, None, false).await?,
            None => None,
        };
        let thread_id = thread.unwrap_or_default();
        let ends_on: Option<Entry> = match entry {
            Some(entry) if self.ctx.end_thread(&entry, &thread_id, EndThread::default()).is_some() => Some(entry),
            _ => None,
        };
        let (done_tx, done_rx) = oneshot::channel::<()>();
        let mut own: Option<Joined> = None;
        if ends_on.is_some() {
            let gate: Joined = async move {
                let _ = done_rx.await;
            }
            .boxed_local()
            .shared();
            own = Some(gate.clone());
            self.hold_next(&thread_id, gate, None);
        }
        let outcome = self.end_turn_stop(row, ends_on, &thread_id).await;
        let _ = done_tx.send(());
        if let Some(gate) = own {
            forget_hold(&self.stops, &thread_id, &gate);
        }
        outcome
    }

    /// The stop's part of `end_turn` (sessions.ts:166-206), between the hold and its release.
    async fn end_turn_stop(&self, row: Row, ends_on: Option<Entry>, thread_id: &str) -> Result<Ended, HostError> {
        let (flag, has_end) = {
            let r = row.borrow();
            (r.unanswered, r.end.is_some())
        };
        let unanswered = flag && has_end && !self.ctx.line_try(&row);
        let (running, handle, end) = {
            let r = row.borrow();
            (r.view.status == SessionStatus::Running, r.handle.clone(), r.end.clone())
        };
        if !running || (handle.is_none() && !unanswered && (ends_on.is_none() || end.is_none())) {
            if ends_on.is_none() || self.ctx.running_on(thread_id).is_some() || self.ctx.launching_on(thread_id).is_some() {
                return Ok(Ended { outcome: SessionInterruptOutcome::NotRunning, left: None });
            }
            let left = self.end_group(&ends_on, thread_id, EndThread::default()).await?;
            return Ok(Ended { outcome: SessionInterruptOutcome::NotRunning, left });
        }
        let stopping: Option<Joined> = match handle {
            Some(handle) => {
                let interrupt = handle.interrupt();
                let finished = handle.finished();
                let stopping: Joined = async move {
                    interrupt.await;
                    let _ = finished.await;
                }
                .boxed_local()
                .shared();
                spawn_local(stopping.clone());
                Some(stopping)
            }
            None => None,
        };
        let place = match &ends_on {
            Some(entry) => {
                let record = entry.borrow().record.borrow().clone();
                self.ctx.workspace_place(&record)
            }
            None => None,
        };
        if let (Some(place), Some(entry)) = (place, ends_on.as_ref()) {
            if self.stop_unheard(&place, stopping.clone()).await {
                let left = self.end_group(&ends_on, thread_id, EndThread { remove: false, away: true }).await?;
                let end = row.borrow().end.clone();
                if let Some(end) = end {
                    end(left.as_deref().unwrap_or(TURN_STOPPED_LINE), Some(true), Some(true));
                }
                if let Some(Some(paid)) = self.ctx.end_owed(entry, thread_id).await? {
                    let name = self.ctx.place_name_of(&place);
                    self.hold_next(thread_id, paid, Some(&name));
                }
                return Ok(Ended { outcome: SessionInterruptOutcome::Accepted, left });
            }
        }
        let Some(stopping) = stopping else {
            let end = row.borrow().end.clone();
            if ends_on.is_none() {
                if let Some(end) = end {
                    end(TURN_STOPPED_LINE, Some(true), Some(true));
                }
                return Ok(Ended { outcome: SessionInterruptOutcome::Accepted, left: None });
            }
            if let Some(end) = end {
                end(TURN_STOPPED_LINE, Some(true), None);
            }
            let left = self.end_group(&ends_on, thread_id, EndThread::default()).await?;
            return Ok(Ended { outcome: SessionInterruptOutcome::Accepted, left });
        };
        stopping.await;
        let left = self.end_group(&ends_on, thread_id, EndThread::default()).await?;
        Ok(Ended { outcome: SessionInterruptOutcome::Accepted, left })
    }

    /// The kind's `endThread` for the thread, where the kind ends a group; its words where it says any.
    async fn end_group(&self, ends_on: &Option<Entry>, thread_id: &str, o: EndThread) -> Result<Option<String>, HostError> {
        match ends_on {
            Some(entry) => match self.ctx.end_thread(entry, thread_id, o) {
                Some(end) => end.await,
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    /// sessions.ts:977. Rules K17, K18.
    pub async fn interrupt_row(&self, row_id: &str, origin: Option<Caller>, task: Option<String>) -> Result<SessionInterruptResult, HostError> {
        let asking: Rc<dyn Fn()> = if task.is_none() { self.ctx.cap_stopping(row_id) } else { Rc::new(|| {}) };
        let sessions = self.ctx.sessions();
        let thread = find_row(&sessions, row_id)
            .and_then(|row| {
                let thread = row.borrow().view.thread_id.clone();
                thread
            })
            .unwrap_or_else(|| row_id.to_string());
        let result = self.stop_turn(asking.clone(), row_id, origin.as_ref(), task.clone()).await;
        let line = match &result {
            Ok(answer) => stop_log_line(&thread, task.as_deref(), outcome_word(answer.outcome), answer.left.as_deref()),
            Err(e) => stop_log_line(&thread, task.as_deref(), &format!("failed: {}", e.message), None),
        };
        console::warn(&line);
        asking();
        result
    }

    /// sessions.ts:1482. Rule K19.
    pub async fn stop_try(&self, row_id: &str) -> Result<(), HostError> {
        let sessions = self.ctx.sessions();
        let row = sessions.borrow().get(row_id).cloned();
        if let Some(row) = row {
            let id = row_id.to_string();
            let rows = sessions.clone();
            self.end_turn(row, None, Rc::new(move || rows.borrow().get(&id).cloned())).await?;
        }
        Ok(())
    }
}
