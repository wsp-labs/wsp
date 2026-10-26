// SPDX-License-Identifier: AGPL-3.0-only
//! threads_cap: a computer's threads at once. threads.ts:364-521. Spec: spec/threads_cap.md.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use wsp_core::console;
use wsp_core::context::Joined;
use wsp_core::types::events::{event_type, EventListener};
use wsp_core::types::internal::{HeldStartRecord, HELD_STARTS};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::session::{SessionStatus, SessionView, ThreadCapWait, ThreadCapWaitWhy, ThreadView};
use wsp_wire::views::session_events_more::{
    SessionCappedEvent, SessionCappedEventWhy, SessionEvent, SessionUncappedEvent, SessionUncappedEventWhy,
};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;
use wsp_words::views::session::fold_threads;

use super::ThreadsArea;

/// Where a waiting turn stands: its thread's ids for the uncapped line (`at`), what its row last said, and the two
/// ends of its wait.
#[derive(Clone)]
pub struct CapWaiting {
    pub wait: ThreadCapWait,
    pub workspace_id: String,
    pub thread_id: String,
    pub session_id: String,
    pub wake: Rc<dyn Fn()>,
    pub stop: Rc<dyn Fn(&str)>,
}

/// context.ts:731 `CapHeld`.
#[derive(Clone)]
pub struct CapHeld {
    pub place_id: Option<String>,
    pub lenders: Vec<String>,
    pub order: u64,
    pub stopped: Option<String>,
    /// The stops being read, each settled by its release (`stops`).
    pub stops: Vec<Shared<LocalBoxFuture<'static, ()>>>,
    pub waiting: Option<CapWaiting>,
    /// `(why, atOnce)` once a look let a waiting turn through.
    pub through: Option<(SessionUncappedEventWhy, i64)>,
}

/// `capWait`'s `at` (threads.ts:442).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CapAt {
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub session_id: String,
    pub request_id: Option<String>,
    pub prompt: String,
}

impl ThreadsArea {
    fn next_cap_order(&self) -> u64 {
        let order = self.cap_order.get();
        self.cap_order.set(order + 1);
        order
    }

    fn set_through(&self, turn_id: &str, through: (SessionUncappedEventWhy, i64)) {
        if let Some(held) = self.cap_held.borrow_mut().get_mut(turn_id) {
            held.through = Some(through);
        }
    }

    /// Chains a held start's write behind the ones before it, so writes land in call order (C8).
    fn held_write(&self, write: LocalBoxFuture<'static, Result<(), HostError>>) {
        let previous = self.held_writes.borrow_mut().take();
        let link: Joined = async move {
            if let Some(previous) = previous {
                previous.await;
            }
            if let Err(e) = write.await {
                console::warn(&format!("a held start was not written down: {}", e.message));
            }
        }
        .boxed_local()
        .shared();
        tokio::task::spawn_local(link.clone());
        *self.held_writes.borrow_mut() = Some(link);
    }

    /// threads.ts:392: C1.
    pub fn cap_hold(&self, record: &WorkspaceRecord, turn_id: &str, lender: Option<&str>) {
        if self.cap_held.borrow().contains_key(turn_id) {
            return;
        }
        let place_id = self.ctx.place_id_of(record);
        let order = self.next_cap_order();
        let lenders = lender.map(|lender| vec![lender.to_string()]).unwrap_or_default();
        self.cap_held.borrow_mut().insert(
            turn_id.to_string(),
            CapHeld { place_id, lenders, order, stopped: None, stops: Vec::new(), waiting: None, through: None },
        );
    }

    /// threads.ts:398: C2.
    pub fn cap_lend(&self, turn_id: &str, lender: &str) {
        let wake = {
            let mut held = self.cap_held.borrow_mut();
            let Some(h) = held.get_mut(turn_id) else {
                return;
            };
            if h.lenders.iter().any(|l| l == lender) {
                return;
            }
            h.lenders.push(lender.to_string());
            h.waiting.as_ref().map(|w| w.wake.clone())
        };
        if let Some(wake) = wake {
            wake();
        }
    }

    /// threads.ts:411: C3. Synchronous.
    pub fn cap_full(&self, record: &WorkspaceRecord, turn_id: &str) -> Option<ThreadCapWait> {
        let place_id = self.ctx.place_id_of(record)?;
        let at_once = self.ctx.place_threads_at(&place_id)?;
        let mine = self.cap_held.borrow().get(turn_id).cloned()?;
        let live = self.ctx.live_records();
        let on_place: HashSet<String> =
            live.iter().filter(|w| self.ctx.place_id_of(w).as_deref() == Some(place_id.as_str())).map(|w| w.id.clone()).collect();
        let held_turns: HashSet<String> = self.cap_held.borrow().keys().cloned().collect();
        let sessions = self.ctx.sessions();
        let rows: Vec<(String, SessionView)> = {
            let sessions = sessions.borrow();
            sessions
                .values()
                .map(|row| {
                    let row = row.borrow();
                    (row.turn_id.clone(), row.view.clone())
                })
                .collect()
        };
        let visible: Vec<SessionView> =
            rows.iter().filter(|(turn, _)| turn != turn_id && !held_turns.contains(turn)).map(|(_, view)| view.clone()).collect();
        let here: Vec<ThreadView> = fold_threads(&visible)
            .into_iter()
            .filter(|t| t.status == SessionStatus::Running && on_place.contains(&t.workspace_id))
            .collect();
        let on_here: HashSet<String> = here.iter().map(|t| t.thread_id.clone().unwrap_or_else(|| t.id.clone())).collect();
        let running_here: HashSet<String> = rows
            .iter()
            .filter(|(_, view)| view.status == SessionStatus::Running && view.thread_id.as_ref().is_some_and(|t| on_here.contains(t)))
            .map(|(turn, _)| turn.clone())
            .collect();
        let borrowed: Vec<(String, String)> = self.borrowed.borrow().iter().map(|(turn, lender)| (turn.clone(), lender.clone())).collect();
        let lent: Vec<(String, String)> =
            borrowed.into_iter().filter(|(turn, lender)| running_here.contains(turn) && running_here.contains(lender)).collect();
        let lender = mine.lenders.iter().find(|l| running_here.contains(*l) && !lent.iter().any(|(_, other)| other == *l)).cloned();
        if let Some(lender) = lender {
            self.borrowed.borrow_mut().insert(turn_id.to_string(), lender);
            if mine.waiting.is_some() {
                self.set_through(turn_id, (SessionUncappedEventWhy::Lent, at_once));
            }
            return None;
        }
        let ahead = self
            .cap_held
            .borrow()
            .iter()
            .filter(|(id, h)| id.as_str() != turn_id && h.place_id.as_deref() == Some(place_id.as_str()) && h.order < mine.order)
            .count() as i64;
        let running = here.len() as i64;
        if running - lent.len() as i64 + ahead >= at_once {
            return Some(ThreadCapWait {
                place_id: place_id.clone(),
                place: self.ctx.place_name(&place_id),
                running: JsInt::from(running),
                at_once: JsInt::from(at_once),
                why: None,
            });
        }
        if let Some(waiting) = &mine.waiting {
            let why = if at_once > waiting.wait.at_once.get() { SessionUncappedEventWhy::Raised } else { SessionUncappedEventWhy::Freed };
            self.set_through(turn_id, (why, at_once));
        }
        None
    }

    /// threads.ts:442: C4. Every effect lands in the call; the future only settles.
    pub fn cap_wait(&self, at: CapAt, wait: ThreadCapWait) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let (said, moved, first_wait) = {
            let mut held_map = self.cap_held.borrow_mut();
            let held = held_map.entry(at.turn_id.clone()).or_insert_with(|| CapHeld {
                place_id: Some(wait.place_id.clone()),
                lenders: Vec::new(),
                order: self.next_cap_order(),
                stopped: None,
                stops: Vec::new(),
                waiting: None,
                through: None,
            });
            if let Some(reason) = held.stopped.clone() {
                return futures::future::ready(Err(HostError::new(reason))).boxed_local();
            }
            let said = held.waiting.as_ref().map(|w| w.wait.clone());
            let moved = said.as_ref().is_none_or(|s| s.running != wait.running || s.at_once != wait.at_once || s.place != wait.place);
            (said, moved, held.waiting.is_none())
        };
        if first_wait {
            let row_facts = {
                let sessions = self.ctx.sessions();
                let sessions = sessions.borrow();
                sessions.get(&at.turn_id).map(|row| {
                    let row = row.borrow();
                    (row.view.harness.clone(), row.notify.clone(), row.notify_by.clone(), row.notify_road)
                })
            };
            let (harness, notify, notify_by, notify_road) = row_facts.unwrap_or_else(|| (String::new(), None, None, None));
            let written = HeldStartRecord {
                workspace_id: at.workspace_id.clone(),
                thread_id: at.thread_id.clone(),
                turn_id: at.turn_id.clone(),
                session_id: at.session_id.clone(),
                harness,
                place: wait.place.clone(),
                prompt: Some(at.prompt.clone()),
                notify,
                notify_by: notify_by.and_then(|scope: ThreadScope| serde_json::to_value(scope).ok()),
                notify_road: notify_road.and_then(|road: WorkspaceOrigin| serde_json::to_value(road).ok()),
            };
            let value = serde_json::to_value(&written).unwrap_or(serde_json::Value::Null);
            let store = self.ctx.store();
            let turn = at.turn_id.clone();
            self.held_write(async move { store.put(HELD_STARTS, &turn, value).await }.boxed_local());
        }
        self.ctx.idle_hold(&at.workspace_id);
        let (sender, receiver) = oneshot::channel::<Result<(), String>>();
        let sender = Rc::new(RefCell::new(Some(sender)));
        let over = Rc::new(Cell::new(false));
        let unlisten: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let release: Rc<dyn Fn()> = {
            let (over, unlisten, ctx, workspace_id) = (over.clone(), unlisten.clone(), self.ctx.clone(), at.workspace_id.clone());
            Rc::new(move || {
                over.set(true);
                let off = unlisten.borrow_mut().take();
                if let Some(off) = off {
                    off();
                }
                ctx.idle_release(&workspace_id);
            })
        };
        let wake: Rc<dyn Fn()> = {
            let (over, release, sender) = (over.clone(), release.clone(), sender.clone());
            Rc::new(move || {
                if over.get() {
                    return;
                }
                release();
                let pending = sender.borrow_mut().take();
                if let Some(pending) = pending {
                    let _ = pending.send(Ok(()));
                }
            })
        };
        let stop: Rc<dyn Fn(&str)> = {
            let (over, release, sender) = (over.clone(), release.clone(), sender.clone());
            Rc::new(move |reason: &str| {
                if over.get() {
                    return;
                }
                release();
                let pending = sender.borrow_mut().take();
                if let Some(pending) = pending {
                    let _ = pending.send(Err(reason.to_string()));
                }
            })
        };
        let bus_wake = wake.clone();
        let listener: EventListener = Rc::new(move |e: &EventUnion| {
            if matches!(event_type(e), "session.end" | "place.changed" | "workspace.deleted") {
                bus_wake();
            }
            Ok(())
        });
        let off = self.ctx.bus().on("*", listener);
        *unlisten.borrow_mut() = Some(off);
        if let Some(held) = self.cap_held.borrow_mut().get_mut(&at.turn_id) {
            held.waiting = Some(CapWaiting {
                wait: wait.clone(),
                workspace_id: at.workspace_id.clone(),
                thread_id: at.thread_id.clone(),
                session_id: at.session_id.clone(),
                wake,
                stop,
            });
        }
        if moved {
            let prompt = if said.is_none() { Some(at.prompt.clone()) } else { None };
            self.ctx.record(SessionEvent::SessionCappedEvent(SessionCappedEvent {
                workspace_id: at.workspace_id.clone(),
                session_id: at.session_id.clone(),
                at: None,
                turn_id: Some(at.turn_id.clone()),
                thread_id: Some(at.thread_id.clone()),
                pos: None,
                copied_from: None,
                place_id: wait.place_id.clone(),
                place: wait.place.clone(),
                running: wait.running,
                at_once: wait.at_once,
                why: wait.why.as_ref().map(|why| match why {
                    ThreadCapWaitWhy::Slot => SessionCappedEventWhy::Slot,
                    ThreadCapWaitWhy::Minute => SessionCappedEventWhy::Minute,
                }),
                request_id: at.request_id.clone(),
                prompt,
            }));
        }
        async move {
            match receiver.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(reason)) => Err(HostError::new(reason)),
                Err(_) => std::future::pending().await,
            }
        }
        .boxed_local()
    }

    /// threads.ts:489: C5.
    pub fn cap_stop(&self, turn_id: &str, reason: &str) -> bool {
        let stop = {
            let mut held = self.cap_held.borrow_mut();
            let Some(h) = held.get_mut(turn_id) else {
                return false;
            };
            h.stopped = Some(reason.to_string());
            h.waiting.as_ref().map(|w| w.stop.clone())
        };
        if let Some(stop) = stop {
            stop(reason);
        }
        true
    }

    /// threads.ts:499: C6. The release.
    pub fn cap_stopping(&self, turn_id: &str) -> Box<dyn Fn()> {
        let (sender, receiver) = oneshot::channel::<()>();
        let asking: Shared<LocalBoxFuture<'static, ()>> = async move {
            let _ = receiver.await;
        }
        .boxed_local()
        .shared();
        {
            let mut held = self.cap_held.borrow_mut();
            let Some(h) = held.get_mut(turn_id) else {
                return Box::new(|| {});
            };
            h.stops.push(asking.clone());
        }
        let me = self.me.clone();
        let turn = turn_id.to_string();
        let sender = RefCell::new(Some(sender));
        Box::new(move || {
            if let Some(area) = me.upgrade() {
                if let Some(h) = area.cap_held.borrow_mut().get_mut(&turn) {
                    h.stops.retain(|stop| !stop.ptr_eq(&asking));
                }
            }
            let pending = sender.borrow_mut().take();
            if let Some(pending) = pending {
                let _ = pending.send(());
            }
        })
    }

    /// threads.ts:514: C7.
    pub fn cap_left(&self, turn_id: &str) {
        let removed = self.cap_held.borrow_mut().shift_remove(turn_id);
        let Some(held) = removed else {
            return;
        };
        if held.waiting.is_some() {
            let store = self.ctx.store();
            let turn = turn_id.to_string();
            self.held_write(async move { store.delete(HELD_STARTS, &turn).await }.boxed_local());
        }
        if let (Some(waiting), Some((why, at_once))) = (held.waiting.as_ref(), held.through) {
            self.ctx.record(SessionEvent::SessionUncappedEvent(SessionUncappedEvent {
                workspace_id: waiting.workspace_id.clone(),
                session_id: waiting.session_id.clone(),
                at: None,
                turn_id: Some(turn_id.to_string()),
                thread_id: Some(waiting.thread_id.clone()),
                pos: None,
                copied_from: None,
                why,
                at_once: JsInt::from(at_once),
            }));
        }
        let wakes: Vec<Rc<dyn Fn()>> = self.cap_held.borrow().values().filter_map(|h| h.waiting.as_ref().map(|w| w.wake.clone())).collect();
        for wake in wakes {
            wake();
        }
    }
}
