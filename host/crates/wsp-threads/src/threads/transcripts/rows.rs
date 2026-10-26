// SPDX-License-Identifier: AGPL-3.0-only
//! transcripts_rows: record, flush, caps, reads and held words. transcripts.ts:94-160, 257-282, 292-360, 382-506,
//! 546-588, 659-714, the rows road. Spec: spec/transcripts_rows.md.
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::rc::Rc;

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use wsp_core::console;
use wsp_core::types::internal::{
    empty_index, fold_event, held_of, index_bytes, index_of, index_read, newest_of, reindex_threads, thread_words_of,
    transcript_unread_line, ThreadWords, TranscriptIndex,
};
use wsp_store::sqlite_transcripts::{EventSize, TranscriptRows};
use wsp_store::transcript_reader::{event_bytes, pick_newest, ThreadEvents, ThreadRead};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::session::SessionStatus;
use wsp_wire::views::session_events::{copied_from_of, DeltaKind};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_words::exit::HostError;
use wsp_words::js::utf16;
use wsp_words::tool_result::kept_patch;

use super::kept::image_keys_of;
use super::TranscriptsArea;

pub use wsp_core::types::internal::{
    PENDING_FLUSH_BYTES, TOOL_RESULT_KEPT, TRANSCRIPT_BYTES, TRANSCRIPT_CAP, TRANSCRIPT_FLUSH_MS, WORDS_HELD,
};
/// What a trim weighs: an event's thread (`None` for `''`) and type. Both `SessionEvent` and wsp-store's
/// `EventSize` are trimmed (`dropOldestOf<E>`, transcripts.ts:102).
pub trait Trimmed {
    fn thread(&self) -> Option<&str>;
    fn is_start(&self) -> bool;
}

/// Runs the arm's body with the variant's struct bound to `$e`; every session event carries the keys read here.
macro_rules! on_event {
    ($event:expr, $e:ident => $body:expr) => {
        match $event {
            SessionEvent::SessionStartEvent($e) => $body,
            SessionEvent::SessionDeltaEvent($e) => $body,
            SessionEvent::SessionDoneEvent($e) => $body,
            SessionEvent::SessionEndEvent($e) => $body,
            SessionEvent::SessionSteerEvent($e) => $body,
            SessionEvent::SessionNotifyEvent($e) => $body,
            SessionEvent::SessionPermissionEvent($e) => $body,
            SessionEvent::SessionPermissionClosedEvent($e) => $body,
            SessionEvent::SessionCheckpointEvent($e) => $body,
            SessionEvent::SessionChangesEvent($e) => $body,
            SessionEvent::SessionPlanEvent($e) => $body,
            SessionEvent::SessionCompactedEvent($e) => $body,
            SessionEvent::SessionRetryEvent($e) => $body,
            SessionEvent::SessionContextEvent($e) => $body,
            SessionEvent::SessionRunEvent($e) => $body,
            SessionEvent::SessionMovedEvent($e) => $body,
            SessionEvent::SessionBehindEvent($e) => $body,
            SessionEvent::SessionNoticeEvent($e) => $body,
            SessionEvent::SessionEarlierEvent($e) => $body,
            SessionEvent::SessionCappedEvent($e) => $body,
            SessionEvent::SessionUncappedEvent($e) => $body,
            SessionEvent::SessionSubagentEvent($e) => $body,
            SessionEvent::SessionSlateEvent($e) => $body,
            SessionEvent::ThreadWaitedEvent($e) => $body,
        }
    };
}

impl Trimmed for SessionEvent {
    fn thread(&self) -> Option<&str> {
        thread_of(self)
    }
    fn is_start(&self) -> bool {
        matches!(self, SessionEvent::SessionStartEvent(_))
    }
}

impl Trimmed for EventSize {
    fn thread(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }
    fn is_start(&self) -> bool {
        self.kind == "session.start"
    }
}

/// A variant's thread id: optional where the wire says so, always present in the few variants that name it plainly.
trait ThreadKey {
    fn thread_key(&self) -> Option<&str>;
}

impl ThreadKey for Option<String> {
    fn thread_key(&self) -> Option<&str> {
        self.as_deref()
    }
}

impl ThreadKey for String {
    fn thread_key(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

/// An event's thread, None where it names none.
pub(super) fn thread_of(e: &SessionEvent) -> Option<&str> {
    on_event!(e, x => x.thread_id.thread_key())
}

/// The workspace an event belongs to.
pub(super) fn ws_of(e: &SessionEvent) -> &str {
    on_event!(e, x => x.workspace_id.as_str())
}

/// The position an event holds, None where a row from before the stamp has none.
pub(super) fn pos_of(e: &SessionEvent) -> Option<i64> {
    on_event!(e, x => x.pos.as_ref().map(|p| p.get()))
}

/// Sets the event's time (where given) and position, as `{...e, at, pos}` does.
fn stamp(event: &mut SessionEvent, at: Option<i64>, pos: u64) {
    on_event!(event, e => {
        if let Some(at) = at {
            e.at = Some(JsNumber(at as f64));
        }
        e.pos = Some(JsInt::from(pos as i64));
    })
}

/// The event as the bus carries it.
fn union_of(event: &SessionEvent) -> EventUnion {
    match event.clone() {
        SessionEvent::SessionStartEvent(e) => EventUnion::SessionStart(Sequenced { event: e, seq: None }),
        SessionEvent::SessionDeltaEvent(e) => EventUnion::SessionDelta(Sequenced { event: e, seq: None }),
        SessionEvent::SessionDoneEvent(e) => EventUnion::SessionDone(Sequenced { event: e, seq: None }),
        SessionEvent::SessionEndEvent(e) => EventUnion::SessionEnd(Sequenced { event: e, seq: None }),
        SessionEvent::SessionSteerEvent(e) => EventUnion::SessionSteer(Sequenced { event: e, seq: None }),
        SessionEvent::SessionNotifyEvent(e) => EventUnion::SessionNotify(Sequenced { event: e, seq: None }),
        SessionEvent::SessionPermissionEvent(e) => EventUnion::SessionPermission(Sequenced { event: e, seq: None }),
        SessionEvent::SessionPermissionClosedEvent(e) => EventUnion::SessionPermissionClosed(Sequenced { event: e, seq: None }),
        SessionEvent::SessionCheckpointEvent(e) => EventUnion::SessionCheckpoint(Sequenced { event: e, seq: None }),
        SessionEvent::SessionChangesEvent(e) => EventUnion::SessionChanges(Sequenced { event: e, seq: None }),
        SessionEvent::SessionPlanEvent(e) => EventUnion::SessionPlan(Sequenced { event: e, seq: None }),
        SessionEvent::SessionCompactedEvent(e) => EventUnion::SessionCompacted(Sequenced { event: e, seq: None }),
        SessionEvent::SessionRetryEvent(e) => EventUnion::SessionRetry(Sequenced { event: e, seq: None }),
        SessionEvent::SessionContextEvent(e) => EventUnion::SessionContext(Sequenced { event: e, seq: None }),
        SessionEvent::SessionRunEvent(e) => EventUnion::SessionRun(Sequenced { event: e, seq: None }),
        SessionEvent::SessionMovedEvent(e) => EventUnion::SessionMoved(Sequenced { event: e, seq: None }),
        SessionEvent::SessionBehindEvent(e) => EventUnion::SessionBehind(Sequenced { event: e, seq: None }),
        SessionEvent::SessionNoticeEvent(e) => EventUnion::SessionNotice(Sequenced { event: e, seq: None }),
        SessionEvent::SessionEarlierEvent(e) => EventUnion::SessionEarlier(Sequenced { event: e, seq: None }),
        SessionEvent::SessionCappedEvent(e) => EventUnion::SessionCapped(Sequenced { event: e, seq: None }),
        SessionEvent::SessionUncappedEvent(e) => EventUnion::SessionUncapped(Sequenced { event: e, seq: None }),
        SessionEvent::SessionSubagentEvent(e) => EventUnion::SessionSubagent(Sequenced { event: e, seq: None }),
        SessionEvent::SessionSlateEvent(e) => EventUnion::SessionSlate(Sequenced { event: e, seq: None }),
        SessionEvent::ThreadWaitedEvent(e) => EventUnion::ThreadWaited(Sequenced { event: e, seq: None }),
    }
}

/// `dropOldestOf`'s per-thread tally.
#[derive(Default)]
struct Tally {
    count: usize,
    bytes: u64,
}

/// T8 `drop_oldest_of`: nothing changes when the whole list is at most 5,000 events and `bytes` at most 4 MiB. The
/// byte bound applies only when `bytes` is over 4 MiB. Per thread (`""` for none) count events and, when byte bound,
/// weigh them; a thread is over when strictly more than 5,000 events or strictly more than 4 MiB. For each thread
/// over, in first-seen order: drop its events before its newest `session.start` oldest first, then the oldest of that
/// newest turn, never its newest event, stopping once it is under. A thread with no start drops oldest first.
pub fn drop_oldest_of<E: Trimmed>(events: &mut Vec<E>, mut bytes: u64, weigh: &dyn Fn(&E) -> u64, gone: &mut dyn FnMut(&E)) -> u64 {
    let limit_bytes = TRANSCRIPT_BYTES as u64;
    if events.len() <= TRANSCRIPT_CAP && bytes <= limit_bytes {
        return bytes;
    }
    let byte_bound = bytes > limit_bytes;
    let mut tallies: IndexMap<String, Tally> = IndexMap::new();
    for e in events.iter() {
        let tally = tallies.entry(e.thread().unwrap_or("").to_string()).or_default();
        tally.count += 1;
        if byte_bound {
            tally.bytes += weigh(e);
        }
    }
    let is_over = |t: &Tally| t.count > TRANSCRIPT_CAP || t.bytes > limit_bytes;
    let mut over: IndexMap<String, (Vec<usize>, usize)> = IndexMap::new();
    for (key, t) in &tallies {
        if is_over(t) {
            over.insert(key.clone(), (Vec::new(), 0));
        }
    }
    if over.is_empty() {
        return bytes;
    }
    for (i, e) in events.iter().enumerate() {
        if let Some((at, last_start)) = over.get_mut(e.thread().unwrap_or("")) {
            if e.is_start() {
                *last_start = at.len();
            }
            at.push(i);
        }
    }
    let mut dropped = vec![false; events.len()];
    for (key, (at, last_start)) in &over {
        let Some(tally) = tallies.get_mut(key) else { continue };
        let end = at.len().saturating_sub(1);
        for &i in at[..*last_start].iter().chain(at[*last_start..end].iter()) {
            if !is_over(tally) {
                break;
            }
            let w = weigh(&events[i]);
            dropped[i] = true;
            tally.count -= 1;
            if byte_bound {
                tally.bytes = tally.bytes.saturating_sub(w);
            }
            bytes = bytes.saturating_sub(w);
        }
    }
    let mut i = 0;
    events.retain(|e| {
        let drop = dropped[i];
        i += 1;
        if drop {
            gone(e);
        }
        !drop
    });
    bytes
}

/// transcripts.ts:98 `dropOldest`: T8, weighed by `event_bytes`; `bytes` `None` sums them.
pub fn drop_oldest(events: &mut Vec<SessionEvent>, bytes: Option<u64>, gone: &mut dyn FnMut(&SessionEvent)) -> u64 {
    let bytes = bytes.unwrap_or_else(|| events.iter().map(event_bytes).sum());
    drop_oldest_of(events, bytes, &|e: &SessionEvent| event_bytes(e), gone)
}

/// transcripts.ts:663 `keptEvent`: T4.
pub fn kept_event(event: &SessionEvent) -> SessionEvent {
    let SessionEvent::SessionDeltaEvent(delta) = event else {
        return event.clone();
    };
    let result = matches!(delta.kind, DeltaKind::ToolResult);
    let under_parent = delta.parent_tool_use_id.is_some() && matches!(delta.kind, DeltaKind::Text | DeltaKind::Thinking);
    let clipped = utf16::len(&delta.text) > TOOL_RESULT_KEPT && (result || under_parent);
    let patch = match (&delta.patch, result) {
        (Some(patch), true) => Some(kept_patch(patch, TOOL_RESULT_KEPT)),
        _ => None,
    };
    let patch_cut = patch.as_ref().is_some_and(|p| p.patch_cut);
    if !clipped && !patch_cut {
        return event.clone();
    }
    let mut out = delta.clone();
    if clipped {
        out.text = utf16::cut(&delta.text, TOOL_RESULT_KEPT);
        if result {
            out.bytes = Some(delta.bytes.unwrap_or_else(|| JsInt::from(delta.text.len() as i64)));
        }
    }
    if let Some(kept) = patch {
        out.patch = Some(kept.patch);
        if kept.patch_cut {
            out.patch_cut = Some(wsp_frames::True);
        }
    }
    SessionEvent::SessionDeltaEvent(out)
}

/// transcripts.ts:152 `markTrimmed`: T9.
pub fn mark_trimmed(index: &mut TranscriptIndex, thread_id: Option<&str>) {
    if let Some(thread) = thread_id {
        index.trimmed.insert(thread.to_string());
    }
}

/// A cut `settle_rows` takes out (transcripts.ts:401): the thread and which of its events go.
pub struct Cut<'a> {
    pub thread_id: String,
    pub drops: &'a dyn Fn(&SessionEvent) -> bool,
}

/// One workspace's words as the balance ranks them: the workspace, the thread, and the share it may hold.
type Share = (String, String, usize);

impl TranscriptsArea {
    /// runtime.ts:959 `indexFor`: the held index, an empty one put in first when none. Answers a copy;
    /// changes go through `ctx.transcript_state()`.
    pub fn index_for(&self, workspace_id: &str) -> TranscriptIndex {
        self.ctx.transcript_state().with(|s| s.index.entry(workspace_id.to_string()).or_insert_with(empty_index).clone())
    }

    /// The position the held index has issued last, its entry put in first when none (`indexFor(ws).pos`).
    fn issued_pos(&self, workspace_id: &str) -> u64 {
        self.ctx.transcript_state().with(|s| s.index.entry(workspace_id.to_string()).or_insert_with(empty_index).pos)
    }

    /// Whether a thread's older events were dropped by the caps (`trimmedOf`, transcripts.ts:662).
    fn trimmed_of(&self, workspace_id: &str, thread_id: &str) -> bool {
        self.ctx.transcript_state().with(|s| s.index.get(workspace_id).is_some_and(|i| i.trimmed.contains(thread_id)))
    }

    /// transcripts.ts:677: T1.
    pub fn record(&self, event: SessionEvent) {
        let now = self.ctx.clock().now();
        let at = match &event {
            SessionEvent::SessionEarlierEvent(e) => e.at.as_ref().map_or(now, |a| a.0 as i64),
            _ => now,
        };
        let ws = ws_of(&event).to_string();
        let mut stamped = event;
        stamp(&mut stamped, Some(at), self.issued_pos(&ws) + 1);
        self.append(stamped.clone());
        let _ = self.ctx.emit(union_of(&stamped));
        if matches!(stamped, SessionEvent::SessionStartEvent(_) | SessionEvent::SessionEndEvent(_)) {
            if let Some(thread) = thread_of(&stamped) {
                self.ctx.push_head(thread);
            }
        }
    }

    /// transcripts.ts:687: T2.
    pub fn record_copies(&self, events: &[SessionEvent]) {
        for e in events {
            let mut copy = e.clone();
            stamp(&mut copy, None, self.issued_pos(ws_of(e)) + 1);
            self.append(copy);
        }
    }

    /// transcripts.ts:690: T3.
    pub fn append(&self, event: SessionEvent) {
        let ws = ws_of(&event).to_string();
        if let Some(thread) = thread_of(&event).map(str::to_string) {
            let cold = self
                .ctx
                .transcript_state()
                .with(|s| s.index.get(&ws).and_then(|i| i.words.get(&thread)).is_some_and(|w| w.cold == Some(true)));
            if cold {
                let had = self.ctx.transcript_state().with(|s| s.index.get(&ws).and_then(|i| i.words.get(&thread)).cloned());
                if let Ok(words) = self.words_of(&ws, &thread, had.as_ref()) {
                    self.ctx.transcript_state().with(|s| {
                        s.index.entry(ws.clone()).or_insert_with(empty_index).words.insert(thread.clone(), words);
                    });
                }
            }
        }
        let kept = kept_event(&event);
        let size = event_bytes(&kept);
        let unwritten = self.ctx.transcript_state().with(|s| {
            fold_event(s.index.entry(ws.clone()).or_insert_with(empty_index), &kept);
            s.pending.entry(ws.clone()).or_default().push(kept);
            let bytes = s.pending_bytes.entry(ws.clone()).or_insert(0);
            *bytes += size;
            *bytes
        });
        if matches!(event, SessionEvent::SessionEndEvent(_)) || unwritten > PENDING_FLUSH_BYTES as u64 {
            drop(self.flush_transcript(&ws));
        } else if !matches!(event, SessionEvent::SessionDeltaEvent(_)) && !self.transcript_timers.borrow().contains_key(&ws) {
            let me = self.me.clone();
            let key = ws.clone();
            let cancel = self.ctx.clock().schedule(
                Box::new(move || {
                    if let Some(area) = me.upgrade() {
                        drop(area.flush_transcript(&key));
                    }
                }),
                TRANSCRIPT_FLUSH_MS as i64,
            );
            self.transcript_timers.borrow_mut().insert(ws, cancel);
        }
    }

    /// transcripts.ts:557: T5. Never fails.
    pub fn flush_transcript(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.cancel_flush(workspace_id);
        if let Some(queued) = self.flushes_queued.borrow().get(workspace_id).cloned() {
            return queued.boxed_local();
        }
        let Some(area) = self.me.upgrade() else {
            return futures::future::ready(()).boxed_local();
        };
        let ws = workspace_id.to_string();
        let step: LocalBoxFuture<'static, Result<(), HostError>> = async move {
            area.flush_step(&ws);
            Ok(())
        }
        .boxed_local();
        let settled = self.on_transcript_queue(workspace_id, step).map(|_| ()).boxed_local().shared();
        self.flushes_queued.borrow_mut().insert(workspace_id.to_string(), settled.clone());
        settled.boxed_local()
    }

    /// The flush's turn on the queue: its queued mark forgotten, then the pending events settled when there are any.
    fn flush_step(&self, workspace_id: &str) {
        self.flushes_queued.borrow_mut().shift_remove(workspace_id);
        if self.ctx.transcript_state().with(|s| s.pending.contains_key(workspace_id)) {
            let _ = self.settle_rows(workspace_id, None);
        }
    }

    /// transcripts.ts:546: T5.
    pub fn cancel_flush(&self, workspace_id: &str) {
        let cancel = self.transcript_timers.borrow_mut().shift_remove(workspace_id);
        if let Some(cancel) = cancel {
            cancel();
        }
    }

    /// transcripts.ts:293: one turn of the workspace's queue; the caller gets its own result.
    pub fn on_transcript_queue<T: 'static>(
        &self,
        workspace_id: &str,
        step: LocalBoxFuture<'static, Result<T, HostError>>,
    ) -> LocalBoxFuture<'static, Result<T, HostError>> {
        let previous = self.transcript_queue.borrow().get(workspace_id).cloned();
        let (done, answer) = futures::channel::oneshot::channel::<Result<T, HostError>>();
        let link = async move {
            if let Some(previous) = previous {
                previous.await;
            }
            let _ = done.send(step.await);
        }
        .boxed_local()
        .shared();
        self.transcript_queue.borrow_mut().insert(workspace_id.to_string(), link.clone());
        tokio::task::spawn_local(link);
        async move {
            match answer.await {
                Ok(result) => result,
                Err(_) => Err(HostError::new("the transcript queue dropped this step before it ran")),
            }
        }
        .boxed_local()
    }

    /// transcripts.ts:401: T6.
    pub fn settle_rows(&self, workspace_id: &str, cut: Option<Cut<'_>>) -> Result<(), HostError> {
        let rows = self.ctx.rows();
        let ws = workspace_id;
        let pending: Vec<SessionEvent> = self.ctx.transcript_state().with(|s| s.pending.get(ws).cloned().unwrap_or_default());
        // The held index is taken out for the transaction and put back in its place whatever the outcome, since the
        // TS changes the held object in place and a failed transaction leaves those changes in memory.
        let was_held = self.ctx.transcript_state().with(|s| s.index.get_mut(ws).map(std::mem::take));
        let held = was_held.is_some();
        let mut index = was_held.unwrap_or_else(empty_index);
        let mut trim_keys: Vec<String> = Vec::new();
        let outcome = rows.atomically(&mut || -> Result<(), HostError> {
            let appended: Vec<SessionEvent> = match &cut {
                None => pending.clone(),
                Some(c) => pending.iter().filter(|e| !(c.drops)(e)).cloned().collect(),
            };
            rows.append(ws, &appended)?;
            let mut reindex: IndexSet<String> = IndexSet::new();
            if let Some(c) = cut.as_ref() {
                let written: Vec<SessionEvent> = rows.thread(ws, &c.thread_id)?.events.into_iter().filter(|e| (c.drops)(e)).collect();
                let positions: Vec<i64> = written.iter().filter_map(pos_of).collect();
                rows.remove(ws, &positions)?;
                let cutting: Vec<&SessionEvent> = written.iter().chain(pending.iter().filter(|e| (c.drops)(e))).collect();
                if !cutting.is_empty() {
                    reindex.insert(c.thread_id.clone());
                }
                for e in cutting {
                    if let SessionEvent::SessionStartEvent(start) = e {
                        if copied_from_of(e).is_none() {
                            index.facts.shift_remove(&start.session_id);
                        }
                    }
                }
            }
            let mut going: Vec<EventSize> = Vec::new();
            let threads: IndexSet<String> = pending.iter().map(|e| thread_of(e).unwrap_or("").to_string()).collect();
            for thread in &threads {
                let total = rows.total(ws, Some(thread.as_str()))?;
                if total.count > TRANSCRIPT_CAP as u64 || total.size > TRANSCRIPT_BYTES as u64 {
                    let mut sizes = rows.sizes(ws, Some(thread.as_str()))?;
                    drop_oldest_of(&mut sizes, total.size, &|e: &EventSize| e.size, &mut |e| going.push(e.clone()));
                }
            }
            if !going.is_empty() {
                for g in &going {
                    mark_trimmed(&mut index, g.thread());
                }
                let starts: Vec<i64> = going.iter().filter(|g| g.is_start()).map(|g| g.pos).collect();
                for e in rows.at(ws, &starts)? {
                    trim_keys.extend(image_keys_of(&e));
                }
                let positions: Vec<i64> = going.iter().map(|g| g.pos).collect();
                rows.remove(ws, &positions)?;
                for g in &going {
                    reindex.insert(g.thread_id.clone().unwrap_or_default());
                }
            }
            let unread = self.ctx.transcript_state().with(|s| s.unread.contains(ws));
            if !reindex.is_empty() && !unread {
                let mut read: HashMap<String, Vec<SessionEvent>> = HashMap::new();
                for thread in &reindex {
                    read.insert(thread.clone(), rows.thread(ws, thread)?.events);
                }
                reindex_threads(&mut index, &reindex, &|thread| read.get(thread).cloned().unwrap_or_default());
            } else if !reindex.is_empty() {
                let issued = index.pos;
                let trimmed = index.trimmed.clone();
                index = index_of(&rows.all(ws)?.events);
                index.pos = index.pos.max(issued);
                index.trimmed = trimmed;
                self.ctx.transcript_state().with(|s| s.unread.shift_remove(ws));
            }
            let unread = self.ctx.transcript_state().with(|s| s.unread.contains(ws));
            if !unread {
                rows.put_index(ws, &String::from_utf8_lossy(&index_bytes(&index, None, false)))?;
            }
            Ok(())
        });
        if held {
            self.ctx.transcript_state().with(|s| {
                s.index.insert(ws.to_string(), index);
            });
        }
        outcome?;
        if !trim_keys.is_empty() {
            drop(self.drop_kept_keys(ws, &trim_keys));
        }
        self.ctx.transcript_state().with(|s| {
            s.pending.shift_remove(ws);
            s.pending_bytes.shift_remove(ws);
        });
        self.balance_words();
        Ok(())
    }

    /// transcripts.ts:385: T7. `drops` `None` takes every event of the thread.
    pub fn drop_from_transcript(
        &self,
        workspace_id: &str,
        thread_id: &str,
        drops: Option<Rc<dyn Fn(&SessionEvent) -> bool>>,
    ) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let Some(area) = self.me.upgrade() else {
            return futures::future::ready(Ok(())).boxed_local();
        };
        let ws = workspace_id.to_string();
        let thread = thread_id.to_string();
        let step: LocalBoxFuture<'static, Result<(), HostError>> = async move {
            let going = |e: &SessionEvent| thread_of(e) == Some(thread.as_str()) && drops.as_ref().is_none_or(|d| d(e));
            area.settle_rows(&ws, Some(Cut { thread_id: thread.clone(), drops: &going }))
        }
        .boxed_local();
        self.on_transcript_queue(workspace_id, step)
    }

    /// transcripts.ts:301: T10.
    pub fn open_transcript(&self, workspace_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        let kept = self.ctx.rows().all(workspace_id)?;
        let (pending, pending_bytes) = self
            .ctx
            .transcript_state()
            .with(|s| (s.pending.get(workspace_id).cloned().unwrap_or_default(), s.pending_bytes.get(workspace_id).copied().unwrap_or(0)));
        let mut events = kept.events;
        events.extend(pending);
        drop_oldest(&mut events, Some(kept.size + pending_bytes), &mut |_| {});
        Ok(events)
    }

    /// transcripts.ts:336: T11.
    pub fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        let kept = self.ctx.rows().thread(workspace_id, thread_id)?;
        let pending: Vec<SessionEvent> = self.ctx.transcript_state().with(|s| {
            s.pending.get(workspace_id).map(|p| p.iter().filter(|e| thread_of(e) == Some(thread_id)).cloned().collect()).unwrap_or_default()
        });
        let pending_bytes: u64 = pending.iter().map(event_bytes).sum();
        let mut events = kept.events;
        events.extend(pending);
        drop_oldest(&mut events, Some(kept.size + pending_bytes), &mut |_| {});
        Ok(events)
    }

    /// transcripts.ts:347 `transcriptReader.read`: T12.
    pub fn read(&self, workspace_id: &str, thread_id: &str, o: &ThreadRead) -> Result<ThreadEvents, HostError> {
        let (pos, pending) = self.ctx.transcript_state().with(|s| {
            let pos = s.index.get(workspace_id).map_or(0, |i| i.pos);
            let pending: Vec<SessionEvent> = s
                .pending
                .get(workspace_id)
                .map(|p| p.iter().filter(|e| thread_of(e) == Some(thread_id)).cloned().collect())
                .unwrap_or_default();
            (pos, pending)
        });
        let rows = self.ctx.rows();
        // pick_newest takes at most `limit` rows, so no more than that is read off the rows.
        let mut newest: Vec<SessionEvent> = pending.iter().rev().cloned().collect();
        let mut taken = 0usize;
        rows.newest(workspace_id, thread_id, o.before, &mut |e| {
            newest.push(e);
            taken += 1;
            if taken >= o.limit {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })?;
        let events = pick_newest(newest, o);
        let total = rows.count(workspace_id, thread_id)? + pending.len() as u64;
        Ok(ThreadEvents { events, pos: pos as i64, total, trimmed: self.trimmed_of(workspace_id, thread_id) })
    }

    /// transcripts.ts:259 `loadRowsIndex`: T13, invariant 5.
    pub fn load_index(&self, workspace_id: &str) {
        let rows = self.ctx.rows();
        let mut newest: Option<i64> = None;
        if let Err(e) = self.load_rows_index(rows.as_ref(), workspace_id, &mut newest) {
            console::warn(&format!(
                "{}, so search and a send leave it out until it reads",
                transcript_unread_line(workspace_id, &e.message)
            ));
            self.ctx.transcript_state().with(|s| {
                if let Some(newest) = newest {
                    s.index.insert(workspace_id.to_string(), TranscriptIndex { pos: newest as u64, ..empty_index() });
                }
                s.unread.insert(workspace_id.to_string());
            });
        }
    }

    /// The reading `load_index` does, under its one failure road; `newest` is the newest position once it is read.
    fn load_rows_index(&self, rows: &dyn TranscriptRows, workspace_id: &str, newest: &mut Option<i64>) -> Result<(), HostError> {
        *newest = Some(rows.newest_pos(workspace_id)?);
        if let Some(json) = rows.index(workspace_id)? {
            if let Some(kept) = index_read(json.as_bytes()) {
                self.ctx.transcript_state().with(|s| {
                    s.index.insert(workspace_id.to_string(), kept.index);
                });
                self.balance_words();
                return Ok(());
            }
        }
        let events = rows.all(workspace_id)?.events;
        if events.is_empty() {
            return Ok(());
        }
        let index = index_of(&events);
        rows.put_index(workspace_id, &String::from_utf8_lossy(&index_bytes(&index, None, false)))?;
        self.ctx.transcript_state().with(|s| {
            s.index.insert(workspace_id.to_string(), index);
        });
        self.balance_words();
        Ok(())
    }

    /// Every thread's words across every folder with the share each may hold, the one spoken in or opened last
    /// first: all of its words while they fit in what is left of WORDS_HELD, then what is left (transcripts.ts:462).
    /// A thread whose turn runs holds all of its own, since each of its events would read it back.
    fn ranked_shares(&self) -> Vec<Share> {
        let sessions = self.ctx.sessions();
        let running: HashSet<String> = sessions
            .borrow()
            .values()
            .filter_map(|row| {
                let entry = row.borrow();
                match (&entry.view.status, &entry.view.thread_id) {
                    (SessionStatus::Running, Some(thread)) => Some(thread.clone()),
                    _ => None,
                }
            })
            .collect();
        let mut ranked: Vec<(String, String, f64, usize, bool)> = Vec::new();
        self.ctx.transcript_state().with(|s| {
            for (ws, index) in &s.index {
                for (thread, w) in &index.words {
                    let rank = w.last.0.max(w.opened.as_ref().map_or(0.0, |o| o.0));
                    ranked.push((ws.clone(), thread.clone(), rank, w.size, w.cold == Some(true)));
                }
            }
        });
        ranked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal));
        let mut held = 0usize;
        ranked
            .into_iter()
            .map(|(ws, thread, _, size, cold)| {
                let left = WORDS_HELD.saturating_sub(held);
                let share = if !cold && running.contains(&thread) { size } else { size.min(left) };
                held += share;
                (ws, thread, share)
            })
            .collect()
    }

    /// transcripts.ts:476: T14.
    pub fn balance_words(&self) {
        for (ws, thread, share) in self.ranked_shares() {
            self.ctx.transcript_state().with(|s| {
                if let Some(w) = s.index.get_mut(&ws).and_then(|i| i.words.get_mut(&thread)) {
                    if held_of(w) > share {
                        let next = newest_of(w, share);
                        *w = next;
                    }
                }
            });
        }
        self.warm_soon();
    }

    /// transcripts.ts:485: T15.
    pub fn warm_soon(&self) {
        if self.warming.get() {
            return;
        }
        self.warming.set(true);
        let me = self.me.clone();
        let _ = self.ctx.clock().schedule(
            Box::new(move || {
                let Some(area) = me.upgrade() else {
                    return;
                };
                area.warming.set(false);
                if !area.ctx.closed() && area.warm_one() {
                    area.warm_soon();
                }
            }),
            0,
        );
    }

    /// transcripts.ts:495: T15. Whether a thread was read back.
    pub fn warm_one(&self) -> bool {
        for (ws, thread, share) in self.ranked_shares() {
            let cold = self.ctx.transcript_state().with(|s| {
                s.index.get(&ws).and_then(|i| i.words.get(&thread)).filter(|w| w.cold == Some(true) && w.room.unwrap_or(0) < share).cloned()
            });
            let Some(had) = cold else {
                continue;
            };
            return match self.words_of(&ws, &thread, Some(&had)) {
                Ok(back) => {
                    let words = newest_of(&back, share);
                    self.ctx.transcript_state().with(|s| {
                        if let Some(index) = s.index.get_mut(&ws) {
                            index.words.insert(thread.clone(), words);
                        }
                    });
                    true
                }
                Err(_) => false,
            };
        }
        false
    }

    /// transcripts.ts:454: T15.
    pub fn words_of(&self, workspace_id: &str, thread_id: &str, had: Option<&ThreadWords>) -> Result<ThreadWords, HostError> {
        let mut events = self.ctx.rows().word_events(workspace_id, thread_id)?;
        events.extend(self.ctx.transcript_state().with(|s| s.pending.get(workspace_id).cloned().unwrap_or_default()));
        let mut back = thread_words_of(&events, thread_id);
        back.last = JsNumber(back.last.0.max(had.map_or(0.0, |h| h.last.0)));
        if let Some(opened) = had.and_then(|h| h.opened) {
            back.opened = Some(opened);
        }
        Ok(back)
    }

    /// transcripts.ts:507: T16.
    pub fn opened_words(&self, workspace_id: &str, thread_id: &str) {
        let now = self.ctx.clock().now();
        let cold = self.ctx.transcript_state().with(|s| match s.index.get_mut(workspace_id).and_then(|i| i.words.get_mut(thread_id)) {
            Some(w) => {
                w.opened = Some(JsNumber(now as f64));
                w.cold == Some(true)
            }
            None => false,
        });
        if cold {
            self.balance_words();
        }
    }
}

/// The reader a head and a history page read through (transcripts.ts:346): `read` (T12).
#[async_trait::async_trait(?Send)]
impl wsp_store::transcript_reader::TranscriptReader for TranscriptsArea {
    async fn read(&self, workspace_id: &str, thread_id: &str, o: &ThreadRead) -> Result<ThreadEvents, HostError> {
        TranscriptsArea::read(self, workspace_id, thread_id, o)
    }
}
