// SPDX-License-Identifier: AGPL-3.0-only
//! The persistence port and its in-memory fake, and the refusals a state in another shape gets. Port of
//! packages/runtime/src/store.ts at 65f6b89c7; the spec is spec/store.md. The JSON file store, the state file reader
//! and the not-an-object refusal are the import road Proposal 3 drops (02, "What the Rust host drops").
use std::cell::{Cell, RefCell};

use indexmap::IndexMap;
use serde_json::Value;
use wsp_wire::text::json_stringify;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::wire::daemon_version::{state_writer_words, StateShape};
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

/// The shape this host writes and reads: the TS host's own `STATE_SHAPE = 2` (02, rule 2.19), so a TS host's `state.db`
/// is opened as it is and any other shape is refused. Its one home is wsp-wire.
pub use wsp_wire::wire::daemon_version::STATE_SHAPE;

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
    StateShape {
        shape: JsInt::from(STATE_SHAPE),
        wsp: writer.wsp.clone(),
        daemon: writer.daemon,
        bin: writer.bin.clone(),
        at: jiff::Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    }
}

/// store.ts:48-49.
pub fn state_written_by_newer_line(state_path: &str, wrote: &StateShape) -> String {
    format!(
        "{state_path} was written by a newer wsp (state shape {}; this wsp reads {STATE_SHAPE}; written {} by {}): run that wsp, or move the file aside",
        wrote.shape,
        wrote.at,
        state_writer_words(wrote)
    )
}

/// store.ts:53-54.
pub fn state_written_by_older_line(state_path: &str, wrote: Option<&StateShape>) -> String {
    let said = match wrote {
        None => "none recorded".to_string(),
        Some(w) => format!("shape {}, written {} by {}", w.shape, w.at, state_writer_words(w)),
    };
    format!(
        "{state_path} holds state in an older shape ({said}; this wsp reads shape {STATE_SHAPE}): move the file aside and this wsp starts a fresh one"
    )
}

/// store.ts:60-61.
pub fn state_unreadable_line(state_path: &str, why: &str) -> String {
    format!(
        "{state_path} does not read as a state file ({why}), so no wsp can read the records in it: move the file aside, or put back a copy a wsp wrote"
    )
}

/// store.ts:73-74. `document` is written as `JSON.stringify` writes it.
pub fn state_shape_unreadable_line(state_path: &str, document: &Value) -> String {
    format!(
        "{state_path} has a {STATE_SHAPE_KEY} that is not a shape document ({}), so no wsp can say which build wrote it or in which shape: move the file aside, or put back the document a wsp writes",
        json_stringify(document)
    )
}

/// Refuses a shape that is not this build's (store.ts:142-145): none or older, then newer, each in its sentence.
pub fn refuse_other_shape(path: &str, wrote: Option<&StateShape>) -> Result<(), HostError> {
    match wrote {
        None => Err(HostError::new(state_written_by_older_line(path, None))),
        Some(w) if w.shape.get() < STATE_SHAPE => Err(HostError::new(state_written_by_older_line(path, wrote))),
        Some(w) if w.shape.get() > STATE_SHAPE => Err(HostError::new(state_written_by_newer_line(path, w))),
        Some(_) => Ok(()),
    }
}

/// The in-memory store tests inject (store.ts:76-112). Collections and ids keep JavaScript object key order: array
/// index ids (`"0"`, `"7"`) ascending first, then the rest in first-write order.
pub fn memory_store() -> MemoryStore {
    MemoryStore {
        data: RefCell::new(IndexMap::new()),
        blobs: RefCell::new(IndexMap::new()),
        marks: RefCell::new(IndexMap::new()),
        writes: Cell::new(0),
    }
}

pub struct MemoryStore {
    data: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    blobs: RefCell<IndexMap<String, Vec<u8>>>,
    marks: RefCell<IndexMap<String, BlobMark>>,
    writes: Cell<u64>,
}

/// An id as an object key sorts: a canonical array index (`"0"` to `"4294967294"`) by its number, before every other
/// key (the same rule `json_stringify` writes an object's keys by).
fn index_of(id: &str) -> Option<u64> {
    let canonical = id == "0" || (!id.is_empty() && !id.starts_with('0') && id.bytes().all(|b| b.is_ascii_digit()));
    canonical.then(|| id.parse::<u64>().ok().filter(|n| *n < 4_294_967_295)).flatten()
}

fn blob_name(collection: &str, id: &str) -> String {
    format!("{collection}/{id}")
}

impl MemoryStore {
    fn in_key_order<'a>(&self, records: &'a IndexMap<String, Value>) -> Vec<(&'a String, &'a Value)> {
        let mut entries: Vec<(&String, &Value)> = records.iter().collect();
        entries.sort_by_key(|(id, _)| index_of(id).map_or((1, 0), |n| (0, n)));
        entries
    }
}

#[async_trait::async_trait(?Send)]
impl Store for MemoryStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.data.borrow().get(collection).and_then(|records| records.get(id)).cloned())
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.data.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value);
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        let data = self.data.borrow();
        Ok(data.get(collection).map(|records| self.in_key_order(records).into_iter().map(|(_, v)| v.clone()).collect()).unwrap_or_default())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        let data = self.data.borrow();
        Ok(data
            .get(collection)
            .map(|records| self.in_key_order(records).into_iter().map(|(id, _)| id.clone()).collect())
            .unwrap_or_default())
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if let Some(records) = self.data.borrow_mut().get_mut(collection) {
            records.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(self.blobs.borrow().get(&blob_name(collection, id)).cloned())
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        let name = blob_name(collection, id);
        self.writes.set(self.writes.get() + 1);
        let mark = BlobMark { bytes: JsNumber(bytes.len() as f64), at: JsNumber(self.writes.get() as f64) };
        self.blobs.borrow_mut().insert(name.clone(), bytes);
        self.marks.borrow_mut().insert(name, mark);
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        let name = blob_name(collection, id);
        self.blobs.borrow_mut().shift_remove(&name);
        self.marks.borrow_mut().shift_remove(&name);
        Ok(())
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        Ok(self.marks.borrow().get(&blob_name(collection, id)).copied())
    }
}
