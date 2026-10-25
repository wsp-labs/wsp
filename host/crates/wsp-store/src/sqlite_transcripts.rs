// SPDX-License-Identifier: AGPL-3.0-only
//! Every workspace's transcript as rows of the state database, one per event, so a flush appends what a turn wrote
//! and one thread's window is read off the `(workspace, thread, pos)` index. Port of
//! packages/runtime/src/sqlite-transcripts.ts at 65f6b89c7 less the blob move (`moveBlobs`, `indexOfBlob`), which
//! Proposal 3 drops; the spec is spec/sqlite_transcripts.md. The tables are in `sqlite_store::SCHEMA`.
use std::cell::Cell;
use std::ops::ControlFlow;
use std::rc::Rc;

use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

/// An event as a trim weighs it: its place, its thread (`None` for `''`), its type and its size
/// (sqlite-transcripts.ts:26-31).
#[derive(Clone, Debug, PartialEq)]
pub struct EventSize {
    pub pos: i64,
    pub thread_id: Option<String>,
    /// The event's `type`, as `session.delta`.
    pub kind: String,
    pub size: u64,
}

/// Events oldest first and their sizes summed.
#[derive(Clone, Debug, PartialEq)]
pub struct Summed {
    pub events: Vec<SessionEvent>,
    pub size: u64,
}

/// How many events and their sizes summed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Total {
    pub count: u64,
    pub size: u64,
}

/// The transcripts module of a state database (sqlite-transcripts.ts:35-64). Every call is one statement or one
/// transaction, so a reader on another connection sees a flush whole or not at all. Every call reaches the database
/// through the store's `ready()`, so a call can refuse as any store call can.
pub trait TranscriptRows {
    /// Every event the workspace's transcript keeps, oldest first, and their sizes summed.
    fn all(&self, workspace_id: &str) -> Result<Summed, HostError>;
    /// One thread's events newest first, below `before` where it is given, handed to `each` one at a time; `each`
    /// answering `Break` stops the read. `each` runs while the connection is in use: it must not call the store.
    fn newest(
        &self,
        workspace_id: &str,
        thread_id: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError>;
    /// How many events of the thread the transcript keeps.
    fn count(&self, workspace_id: &str, thread_id: &str) -> Result<u64, HostError>;
    /// How many events the transcript keeps and their sizes summed, of one thread where it is named (`""` for none).
    fn total(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Total, HostError>;
    /// The newest position the transcript keeps, 0 where it keeps none.
    fn newest_pos(&self, workspace_id: &str) -> Result<i64, HostError>;
    /// Every event's size, oldest first, of one thread where it is named.
    fn sizes(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Vec<EventSize>, HostError>;
    /// One thread's events that can carry its words, oldest first: `session.start`, `session.steer`,
    /// `session.earlier`, and a `session.delta` whose `kind` is `text`.
    fn word_events(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError>;
    /// One thread's events, oldest first, and their sizes summed.
    fn thread(&self, workspace_id: &str, thread_id: &str) -> Result<Summed, HostError>;
    /// The events at these positions, in the order asked, a position kept by no row giving nothing.
    fn at(&self, workspace_id: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError>;
    /// Events after every one the transcript keeps, each at its own `pos`, in one transaction.
    fn append(&self, workspace_id: &str, events: &[SessionEvent]) -> Result<(), HostError>;
    fn remove(&self, workspace_id: &str, positions: &[i64]) -> Result<(), HostError>;
    /// The transcript and its index, gone, in one transaction.
    fn clear(&self, workspace_id: &str) -> Result<(), HostError>;
    fn index(&self, workspace_id: &str) -> Result<Option<String>, HostError>;
    fn put_index(&self, workspace_id: &str, json: &str) -> Result<(), HostError>;
    /// `f` in one `begin immediate` transaction, undone whole if it fails; a call inside one joins it.
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError>;
}

/// The connection a transcript call runs on: the store's `ready()`, which may open the database at the first call.
pub type DbHandle = Rc<dyn Fn() -> Result<Rc<rusqlite::Connection>, HostError>>;

/// The module's rows on the database `db` answers (sqlite-transcripts.ts:74).
pub struct SqliteTranscripts {
    db: DbHandle,
    /// How deep `atomically` is, so an inner call joins the outer transaction (sqlite-transcripts.ts:82-99).
    depth: Cell<u32>,
}

impl SqliteTranscripts {
    pub fn new(db: DbHandle) -> SqliteTranscripts {
        todo!()
    }
}

/// The events a thread's words come from (sqlite-transcripts.ts:246), as SQL.
pub const WORDS: &str = "(type in ('session.start', 'session.steer', 'session.earlier') or (type = 'session.delta' and json_extract(json, '$.kind') = 'text'))";

impl TranscriptRows for SqliteTranscripts {
    fn all(&self, workspace_id: &str) -> Result<Summed, HostError> {
        todo!()
    }
    fn newest(
        &self,
        workspace_id: &str,
        thread_id: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        todo!()
    }
    fn count(&self, workspace_id: &str, thread_id: &str) -> Result<u64, HostError> {
        todo!()
    }
    fn total(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Total, HostError> {
        todo!()
    }
    fn newest_pos(&self, workspace_id: &str) -> Result<i64, HostError> {
        todo!()
    }
    fn sizes(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        todo!()
    }
    fn word_events(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<SessionEvent>, HostError> {
        todo!()
    }
    fn thread(&self, workspace_id: &str, thread_id: &str) -> Result<Summed, HostError> {
        todo!()
    }
    fn at(&self, workspace_id: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        todo!()
    }
    fn append(&self, workspace_id: &str, events: &[SessionEvent]) -> Result<(), HostError> {
        todo!()
    }
    fn remove(&self, workspace_id: &str, positions: &[i64]) -> Result<(), HostError> {
        todo!()
    }
    fn clear(&self, workspace_id: &str) -> Result<(), HostError> {
        todo!()
    }
    fn index(&self, workspace_id: &str) -> Result<Option<String>, HostError> {
        todo!()
    }
    fn put_index(&self, workspace_id: &str, json: &str) -> Result<(), HostError> {
        todo!()
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        todo!()
    }
}
