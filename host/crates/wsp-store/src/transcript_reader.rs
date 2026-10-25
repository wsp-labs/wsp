// SPDX-License-Identifier: AGPL-3.0-only
//! One thread's events out of a workspace's transcript, and the one rule for an event's JSON and size. Port of
//! packages/runtime/src/transcript-reader.ts at 65f6b89c7 less `numbered`, which only numbers events an older build
//! wrote with no positions (Proposal 3 drops them); the spec is spec/transcript_reader.md.
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

/// What a read of one thread asks for (transcript-reader.ts:8-14): the events under `before` (every one when
/// `None`), newest first, at most `limit` of them (`usize::MAX` for the TS `Infinity`) and no more than `bytes` of
/// UTF-8 JSON as `shape` leaves each, the comma after each counted. `strict` holds even the newest event to the bytes.
#[derive(Clone, Copy)]
pub struct ThreadRead {
    pub before: Option<i64>,
    pub limit: usize,
    pub bytes: u64,
    pub strict: bool,
    pub shape: Option<fn(&SessionEvent) -> SessionEvent>,
}

/// The events read, oldest first; `pos` the newest position the workspace's transcript has issued, `total` how many
/// events of the thread it holds, `trimmed` whether the caps dropped older events of it (transcript-reader.ts:18-24).
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadEvents {
    pub events: Vec<SessionEvent>,
    pub pos: i64,
    pub total: u64,
    pub trimmed: bool,
}

/// What `read_thread` answers: `ThreadEvents` less `pos` and `trimmed`.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadPick {
    pub events: Vec<SessionEvent>,
    pub total: u64,
}

/// One thread's events out of a workspace's transcript (transcript-reader.ts:28-30), what a head and a history page
/// read; the threads area implements it.
#[async_trait::async_trait(?Send)]
pub trait TranscriptReader {
    async fn read(&self, workspace_id: &str, thread_id: &str, o: &ThreadRead) -> Result<ThreadEvents, HostError>;
}

/// The read over a transcript held as one list in the order its positions were issued (transcript-reader.ts:33-40).
pub fn read_thread(events: &[SessionEvent], thread_id: &str, o: &ThreadRead) -> ThreadPick {
    todo!()
}

/// What a read takes of one thread's events given newest first, answered oldest first (transcript-reader.ts:44-56).
pub fn pick_newest(newest: impl IntoIterator<Item = SessionEvent>, o: &ThreadRead) -> Vec<SessionEvent> {
    todo!()
}

/// An event's JSON as a transcript writes it, and the size its byte cap counts (transcript-reader.ts:60-63).
#[derive(Clone, Debug, PartialEq)]
pub struct EventJson {
    pub json: String,
    /// `json.length`: UTF-16 code units, not UTF-8 bytes (rule 2.11).
    pub bytes: u64,
}

/// `JSON.stringify(e)` through `wsp_wire::text::json_stringify`, and its length in UTF-16 units.
pub fn event_json(e: &SessionEvent) -> EventJson {
    todo!()
}

/// transcript-reader.ts:65.
pub fn event_bytes(e: &SessionEvent) -> u64 {
    todo!()
}

/// An event as a head carries it (transcript-reader.ts:77-81): a `tool_result` delta's text past `HEAD_RESULT_CHARS`
/// UTF-16 units cut there with `cut` its whole length, its patch cut by `kept_patch` to as many characters; every
/// other event as it is.
pub fn head_shape(e: &SessionEvent) -> SessionEvent {
    todo!()
}
