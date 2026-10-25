// SPDX-License-Identifier: AGPL-3.0-only
//! transcripts_rows: record, flush, caps, reads and held words. transcripts.ts:94-160, 257-282, 292-360, 382-506,
//! 546-588, 659-714, the rows road. Spec: spec/transcripts_rows.md.
use futures::future::LocalBoxFuture;
use wsp_core::types::internal::{ThreadWords, TranscriptIndex};
use wsp_store::sqlite_transcripts::EventSize;
use wsp_store::transcript_reader::{ThreadEvents, ThreadRead};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

use super::TranscriptsArea;

pub const TRANSCRIPT_CAP: usize = 5000;
pub const TRANSCRIPT_BYTES: u64 = 4 * 1024 * 1024;
pub const TOOL_RESULT_KEPT: usize = 16 * 1024;
pub const PENDING_FLUSH_BYTES: u64 = 1024 * 1024;
pub const TRANSCRIPT_FLUSH_MS: i64 = 250;
pub const WORDS_HELD: usize = 2 * 1024 * 1024;

/// What a trim weighs: an event's thread (`None` for `''`) and type. Both `SessionEvent` and wsp-store's
/// `EventSize` are trimmed (`dropOldestOf<E>`, transcripts.ts:102).
pub trait Trimmed {
    fn thread(&self) -> Option<&str>;
    fn is_start(&self) -> bool;
}

impl Trimmed for SessionEvent {
    fn thread(&self) -> Option<&str> {
        todo!()
    }
    fn is_start(&self) -> bool {
        todo!()
    }
}

impl Trimmed for EventSize {
    fn thread(&self) -> Option<&str> {
        todo!()
    }
    fn is_start(&self) -> bool {
        todo!()
    }
}

/// transcripts.ts:102 `dropOldestOf`: T8. Drops in place and answers the bytes left.
pub fn drop_oldest_of<E: Trimmed>(events: &mut Vec<E>, bytes: u64, weigh: &dyn Fn(&E) -> u64, gone: &mut dyn FnMut(&E)) -> u64 {
    todo!()
}

/// transcripts.ts:98 `dropOldest`: T8, weighed by `event_bytes`; `bytes` `None` sums them.
pub fn drop_oldest(events: &mut Vec<SessionEvent>, bytes: Option<u64>, gone: &mut dyn FnMut(&SessionEvent)) -> u64 {
    todo!()
}

/// transcripts.ts:663 `keptEvent`: T4.
pub fn kept_event(event: &SessionEvent) -> SessionEvent {
    todo!()
}

/// transcripts.ts:152 `markTrimmed`: T9.
pub fn mark_trimmed(index: &mut TranscriptIndex, thread_id: Option<&str>) {
    todo!()
}

/// A cut `settle_rows` takes out (transcripts.ts:401): the thread and which of its events go.
pub struct Cut<'a> {
    pub thread_id: String,
    pub drops: &'a dyn Fn(&SessionEvent) -> bool,
}

impl TranscriptsArea {
    /// runtime.ts:959 `indexFor`: the held index, an empty one put in first when none. Answers a copy;
    /// changes go through `ctx.transcript_state()`.
    pub fn index_for(&self, workspace_id: &str) -> TranscriptIndex {
        todo!()
    }
    /// transcripts.ts:677: T1.
    pub fn record(&self, event: SessionEvent) {
        todo!()
    }
    /// transcripts.ts:687: T2.
    pub fn record_copies(&self, events: &[SessionEvent]) {
        todo!()
    }
    /// transcripts.ts:690: T3.
    pub fn append(&self, event: SessionEvent) {
        todo!()
    }
    /// transcripts.ts:557: T5. Never fails.
    pub fn flush_transcript(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// transcripts.ts:546: T5.
    pub fn cancel_flush(&self, workspace_id: &str) {
        todo!()
    }
    /// transcripts.ts:293: one turn of the workspace's queue; the caller gets its own result.
    pub fn on_transcript_queue<T: 'static>(&self, workspace_id: &str, step: LocalBoxFuture<'static, Result<T, HostError>>) -> LocalBoxFuture<'static, Result<T, HostError>> {
        todo!()
    }
    /// transcripts.ts:401: T6.
    pub fn settle_rows(&self, workspace_id: &str, cut: Option<Cut<'_>>) -> Result<(), HostError> {
        todo!()
    }
    /// transcripts.ts:385: T7. `drops` `None` takes every event of the thread.
    pub fn drop_from_transcript(&self, workspace_id: &str, thread_id: &str, drops: Option<std::rc::Rc<dyn Fn(&SessionEvent) -> bool>>) -> LocalBoxFuture<'static, Result<(), HostError>> {
        todo!()
    }
    /// transcripts.ts:301: T10.
    pub fn open_transcript(&self, workspace_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        todo!()
    }
    /// transcripts.ts:336: T11.
    pub fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        todo!()
    }
    /// transcripts.ts:347 `transcriptReader.read`: T12.
    pub fn read(&self, workspace_id: &str, thread_id: &str, o: &ThreadRead) -> Result<ThreadEvents, HostError> {
        todo!()
    }
    /// transcripts.ts:259 `loadRowsIndex`: T13, invariant 5.
    pub fn load_index(&self, workspace_id: &str) {
        todo!()
    }
    /// transcripts.ts:476: T14.
    pub fn balance_words(&self) {
        todo!()
    }
    /// transcripts.ts:485: T15.
    pub fn warm_soon(&self) {
        todo!()
    }
    /// transcripts.ts:495: T15. Whether a thread was read back.
    pub fn warm_one(&self) -> bool {
        todo!()
    }
    /// transcripts.ts:454: T15.
    pub fn words_of(&self, workspace_id: &str, thread_id: &str, had: Option<&ThreadWords>) -> Result<ThreadWords, HostError> {
        todo!()
    }
    /// transcripts.ts:507: T16.
    pub fn opened_words(&self, workspace_id: &str, thread_id: &str) {
        todo!()
    }
}

/// The reader a head and a history page read through (transcripts.ts:346): `read` (T12).
#[async_trait::async_trait(?Send)]
impl wsp_store::transcript_reader::TranscriptReader for TranscriptsArea {
    async fn read(&self, workspace_id: &str, thread_id: &str, o: &ThreadRead) -> Result<ThreadEvents, HostError> {
        todo!()
    }
}
