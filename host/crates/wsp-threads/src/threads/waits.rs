// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/waits.ts at 65f6b89c7. Spec: spec/waits.md.
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::workspace_state::WAITED_ROW_MS;
use wsp_wire::views::session::SessionStatus;
use wsp_wire::views::session_events_more::{SessionEvent, SessionRetryEvent, ThreadWaitedEvent};

use crate::threads::threads::{Row, Sessions};

/// waits.ts:94.
pub const WAIT_SETTLE_MS: i64 = 2_000;
/// waits.ts:98.
pub const RETRY_SETTLE_MS: i64 = 2_000;

/// What the waits area reads of the runtime.
pub trait WaitsCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    fn sessions(&self) -> Sessions;
    /// `ctx.state.closing`, read at the moment of each write.
    fn closing(&self) -> bool;
    /// `ctx.record(event)`: the transcript row, and the bus.
    fn record(&self, event: SessionEvent);
}

/// A retry the host saw (TS `{ attempt, why }`).
#[derive(Clone, Debug, PartialEq)]
pub struct RetryNote {
    pub attempt: u32,
    pub why: String,
}

#[derive(Default)]
struct WaitsState {
    settling: IndexMap<String, f64>,
    said: IndexMap<String, String>,
    unsaid: IndexMap<String, Vec<Rc<RetryNote>>>,
}

/// waits.ts:102. Clones share one state.
#[derive(Clone)]
pub struct WaitsArea {
    ctx: Rc<dyn WaitsCtx>,
    state: Rc<RefCell<WaitsState>>,
}

impl WaitsArea {
    pub fn new(ctx: Rc<dyn WaitsCtx>) -> Self {
        WaitsArea { ctx, state: Rc::new(RefCell::new(WaitsState::default())) }
    }

    /// waits.ts:154. `text` is the host's sentence where it answered with one. Rules W1-W8.
    pub fn note_waited(&self, thread_id: &str, ms: f64, text: Option<&str>) {
        if let Some(text) = text {
            self.write(thread_id, ms, Some(text.strip_prefix("wsp: ").unwrap_or(text)));
            return;
        }
        if !matches!(ms.partial_cmp(&(WAITED_ROW_MS as f64)), Some(std::cmp::Ordering::Greater)) {
            return;
        }
        {
            let mut st = self.state.borrow_mut();
            if let Some(held) = st.settling.get_mut(thread_id) {
                *held = held.max(ms);
                return;
            }
            st.settling.insert(thread_id.to_string(), ms);
        }
        let me = self.clone();
        let thread = thread_id.to_string();
        self.ctx.clock().schedule(Box::new(move || me.settled(&thread)), WAIT_SETTLE_MS);
    }

    /// waits.ts:175. Rules W9, W10.
    pub fn note_retry(&self, thread_id: &str, retry: RetryNote) {
        let Some(row) = self.latest_turn(thread_id) else { return };
        let (turn_id, running) = {
            let r = row.borrow();
            (r.turn_id.clone(), r.view.status == SessionStatus::Running)
        };
        if !running || self.ctx.closing() {
            return;
        }
        let note = Rc::new(retry);
        self.state.borrow_mut().unsaid.entry(turn_id.clone()).or_default().push(note.clone());
        let me = self.clone();
        let thread = thread_id.to_string();
        self.ctx.clock().schedule(Box::new(move || me.retry_settled(&row, &turn_id, &thread, &note)), RETRY_SETTLE_MS);
    }

    /// waits.ts:118. Rule W11.
    pub fn agent_retried(&self, turn_id: &str, why: Option<&str>) {
        let index = {
            let st = self.state.borrow();
            let Some(notes) = st.unsaid.get(turn_id) else { return };
            notes.iter().rposition(|n| Some(n.why.as_str()) == why).or(notes.len().checked_sub(1))
        };
        self.drop_note(turn_id, index);
    }

    /// waits.ts:129-152. The row a write lands on is the thread's latest turn (W1).
    fn latest_turn(&self, thread_id: &str) -> Option<Row> {
        let sessions = self.ctx.sessions();
        let rows = sessions.borrow();
        let mut latest: Option<(Row, f64)> = None;
        for row in rows.values() {
            let (on_thread, started) = {
                let r = row.borrow();
                (r.view.thread_id.as_deref() == Some(thread_id), r.view.started_at.as_ref().map_or(0.0, |n| n.0))
            };
            if on_thread && latest.as_ref().is_none_or(|(_, best)| started >= *best) {
                latest = Some((row.clone(), started));
            }
        }
        latest.map(|(row, _)| row)
    }

    /// waits.ts:129-152. Rules W2-W5.
    fn write(&self, thread_id: &str, ms: f64, said: Option<&str>) {
        let Some(row) = self.latest_turn(thread_id) else { return };
        if self.ctx.closing() {
            return;
        }
        let (workspace_id, session_id, turn_id) = {
            let r = row.borrow();
            (r.view.workspace_id.clone(), r.view.claude_session_id.clone().unwrap_or_else(|| r.turn_id.clone()), r.turn_id.clone())
        };
        if let Some(text) = said {
            if !self.take_said(&turn_id, text) {
                return;
            }
        }
        self.ctx.record(SessionEvent::ThreadWaitedEvent(ThreadWaitedEvent {
            workspace_id,
            session_id,
            at: None,
            turn_id: Some(turn_id),
            thread_id: thread_id.to_string(),
            pos: None,
            copied_from: None,
            ms: js_ms(ms),
            said: said.map(str::to_string),
        }));
    }

    /// Records `text` as the turn's last sentence; false when it already is (W4).
    fn take_said(&self, turn_id: &str, text: &str) -> bool {
        let mut st = self.state.borrow_mut();
        if st.said.get(turn_id).is_some_and(|last| last == text) {
            return false;
        }
        st.said.insert(turn_id.to_string(), text.to_string());
        true
    }

    /// waits.ts:102-111, the settle timer of a wait: the longest hold of the thread writes one row.
    fn settled(&self, thread_id: &str) {
        let ms = self.state.borrow_mut().settling.shift_remove(thread_id);
        if let Some(ms) = ms {
            self.write(thread_id, ms, None);
        }
    }

    /// waits.ts:175-189, the settle timer of one retry (W10).
    fn retry_settled(&self, row: &Row, turn_id: &str, thread_id: &str, note: &Rc<RetryNote>) {
        let index = self.state.borrow().unsaid.get(turn_id).and_then(|notes| notes.iter().position(|n| Rc::ptr_eq(n, note)));
        let (running, workspace_id, session_id) = {
            let r = row.borrow();
            (
                r.view.status == SessionStatus::Running,
                r.view.workspace_id.clone(),
                r.view.claude_session_id.clone().unwrap_or_else(|| turn_id.to_string()),
            )
        };
        if !self.drop_note(turn_id, index) || !running || self.ctx.closing() {
            return;
        }
        self.ctx.record(SessionEvent::SessionRetryEvent(SessionRetryEvent {
            workspace_id,
            session_id,
            at: None,
            turn_id: Some(turn_id.to_string()),
            thread_id: Some(thread_id.to_string()),
            pos: None,
            copied_from: None,
            attempt: JsInt(JsNumber(note.attempt as f64)),
            of: None,
            why: Some(note.why.clone()),
            line: None,
        }));
    }

    /// waits.ts:113-116. Takes the note at `index` out of the turn's list; a turn whose list empties is forgotten.
    fn drop_note(&self, turn_id: &str, index: Option<usize>) -> bool {
        let Some(i) = index else { return false };
        let mut st = self.state.borrow_mut();
        let Some(notes) = st.unsaid.get_mut(turn_id) else { return false };
        notes.remove(i);
        let empty = notes.is_empty();
        if empty {
            st.unsaid.shift_remove(turn_id);
        }
        true
    }
}

/// `Math.max(0, Math.round(ms))`, as the row writes it.
fn js_ms(ms: f64) -> JsInt {
    let r = ms.round();
    JsInt(JsNumber(if r > 0.0 || r.is_nan() { r } else { 0.0 }))
}
