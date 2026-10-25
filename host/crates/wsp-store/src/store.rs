// SPDX-License-Identifier: AGPL-3.0-only
//! The persistence port and its in-memory fake, and the refusals a state in another shape gets. Port of
//! packages/runtime/src/store.ts at 65f6b89c7; the spec is spec/store.md. The JSON file store, the state file reader
//! and the not-an-object refusal are the import road Proposal 3 drops (02, "What the Rust host drops").
use std::cell::{Cell, RefCell};

use indexmap::IndexMap;
use serde_json::Value;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::wire::daemon_version::StateShape;
use wsp_words::exit::HostError;

use crate::sqlite_transcripts::TranscriptRows;

/// Persistence port (store.ts:9-24). Blobs are bytes too large for a record (vault archives), one per id. Every value
/// read is the caller's own copy. An error is a plain `Error(message)` in the TS: `HostError` with no kind.
#[async_trait::async_trait(?Send)]
pub trait Store {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError>;
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError>;
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError>;
    /// The ids in the collection, in the order `list` answers their records.
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError>;
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError>;
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError>;
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError>;
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError>;
    /// Which write a blob is: its size and when it was written, `None` where there is no blob (store.ts:19-21).
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError>;
    /// The transcripts as rows, one per event, where the store keeps them so (store.ts:22-23); the memory store has
    /// none.
    fn transcripts(&self) -> Option<&dyn TranscriptRows> {
        None
    }
}

/// The size and write time of a blob (store.ts:26-29). `at` is milliseconds with a fraction on SQLite (rule 2.9) and
/// a write counter on the memory store.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlobMark {
    pub bytes: JsNumber,
    pub at: JsNumber,
}

/// The top-level name the shape document sits under in the TS state file (store.ts:36); the refusals name it.
pub const STATE_SHAPE_KEY: &str = "$shape";

/// The shape this host writes and reads: one past the TS host's `STATE_SHAPE = 2` (02, rule 2.19), so a TS host's
/// `state.db` is refused as an older shape and never read.
pub const STATE_SHAPE: i64 = 3;

/// The build a save records as the writer (store.ts:41): `StateShape` less `shape` and `at`.
#[derive(Clone, Debug, PartialEq)]
pub struct StateWriter {
    pub wsp: String,
    pub daemon: JsInt,
    pub bin: String,
}

/// `{shape: STATE_SHAPE, ...writer, at: new Date().toISOString()}` (store.ts:43): keys `shape, wsp, daemon, bin, at`,
/// `at` as `2026-10-25T17:13:49.123Z`.
pub fn shape_now(writer: &StateWriter) -> StateShape {
    todo!()
}

/// store.ts:48-49.
pub fn state_written_by_newer_line(state_path: &str, wrote: &StateShape) -> String {
    todo!()
}

/// store.ts:53-54.
pub fn state_written_by_older_line(state_path: &str, wrote: Option<&StateShape>) -> String {
    todo!()
}

/// store.ts:60-61.
pub fn state_unreadable_line(state_path: &str, why: &str) -> String {
    todo!()
}

/// store.ts:73-74. `document` is written as `JSON.stringify` writes it.
pub fn state_shape_unreadable_line(state_path: &str, document: &Value) -> String {
    todo!()
}

/// Refuses a shape that is not this build's (store.ts:142-145): none or older, then newer, each in its sentence.
pub fn refuse_other_shape(path: &str, wrote: Option<&StateShape>) -> Result<(), HostError> {
    todo!()
}

/// The in-memory store tests inject (store.ts:76-112). Collections and ids keep JavaScript object key order: array
/// index ids (`"0"`, `"7"`) ascending first, then the rest in first-write order.
pub fn memory_store() -> MemoryStore {
    todo!()
}

pub struct MemoryStore {
    data: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    blobs: RefCell<IndexMap<String, Vec<u8>>>,
    marks: RefCell<IndexMap<String, BlobMark>>,
    writes: Cell<u64>,
}

#[async_trait::async_trait(?Send)]
impl Store for MemoryStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        todo!()
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        todo!()
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        todo!()
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        todo!()
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        todo!()
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        todo!()
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        todo!()
    }
}
