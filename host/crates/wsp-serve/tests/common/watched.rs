// SPDX-License-Identifier: AGPL-3.0-only
//! The TS tests that reassign what a store hands out (`store.putBlob`, `rows.all`, `rows.wordEvents`): a store that
//! answers like the one it wraps and counts or fails those calls.
use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::Value;
use wsp_store::sqlite_transcripts::{EventSize, Summed, Total, TranscriptRows};
use wsp_store::store::{BlobMark, Store};
use wsp_words::exit::HostError;

/// What the TS tests reassign on the store: `store.putBlob`, `rows.all`, `rows.wordEvents`. A store that answers like
/// the one it wraps, keeps the collection of each blob written, counts the reads of a folder's rows and of a thread's
/// words, and fails the folder read while `fail_all` is set.
pub struct Watched {
    inner: Rc<dyn Store>,
    /// The collection of each blob written, in order.
    pub blobs: RefCell<Vec<String>>,
    pub rows: WatchedRows,
}

pub struct WatchedRows {
    inner: Rc<dyn Store>,
    /// How many times a folder's whole transcript was read (`rows.all`).
    pub folder_reads: Cell<u32>,
    /// How many times a thread's words were read (`rows.wordEvents`).
    pub word_reads: Cell<u32>,
    /// Fails the folder read, as `rows.all = () => { throw ... }` does.
    pub fail_all: Cell<bool>,
}

impl WatchedRows {
    fn real(&self) -> &dyn TranscriptRows {
        self.inner.transcripts().expect("a store with rows")
    }
}

impl Watched {
    pub fn over(inner: &Rc<dyn Store>) -> Rc<Watched> {
        let rows = WatchedRows { inner: inner.clone(), folder_reads: Cell::new(0), word_reads: Cell::new(0), fail_all: Cell::new(false) };
        Rc::new(Watched { inner: inner.clone(), blobs: RefCell::default(), rows })
    }
    pub fn store(self: &Rc<Self>) -> Rc<dyn Store> {
        self.clone()
    }
}

impl TranscriptRows for WatchedRows {
    fn all(&self, workspace_id: &str) -> Result<Summed, HostError> {
        self.folder_reads.set(self.folder_reads.get() + 1);
        if self.fail_all.get() {
            return Err(HostError::new("disk I/O error"));
        }
        self.real().all(workspace_id)
    }
    fn newest(
        &self,
        workspace_id: &str,
        thread_id: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(wsp_wire::views::session_events::SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        self.real().newest(workspace_id, thread_id, before, each)
    }
    fn count(&self, workspace_id: &str, thread_id: &str) -> Result<u64, HostError> {
        self.real().count(workspace_id, thread_id)
    }
    fn total(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Total, HostError> {
        self.real().total(workspace_id, thread_id)
    }
    fn newest_pos(&self, workspace_id: &str) -> Result<i64, HostError> {
        self.real().newest_pos(workspace_id)
    }
    fn sizes(&self, workspace_id: &str, thread_id: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        self.real().sizes(workspace_id, thread_id)
    }
    fn word_events(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<wsp_wire::views::session_events::SessionEvent>, HostError> {
        self.word_reads.set(self.word_reads.get() + 1);
        self.real().word_events(workspace_id, thread_id)
    }
    fn thread(&self, workspace_id: &str, thread_id: &str) -> Result<Summed, HostError> {
        self.real().thread(workspace_id, thread_id)
    }
    fn at(&self, workspace_id: &str, positions: &[i64]) -> Result<Vec<wsp_wire::views::session_events::SessionEvent>, HostError> {
        self.real().at(workspace_id, positions)
    }
    fn append(&self, workspace_id: &str, events: &[wsp_wire::views::session_events::SessionEvent]) -> Result<(), HostError> {
        self.real().append(workspace_id, events)
    }
    fn remove(&self, workspace_id: &str, positions: &[i64]) -> Result<(), HostError> {
        self.real().remove(workspace_id, positions)
    }
    fn clear(&self, workspace_id: &str) -> Result<(), HostError> {
        self.real().clear(workspace_id)
    }
    fn index(&self, workspace_id: &str) -> Result<Option<String>, HostError> {
        self.real().index(workspace_id)
    }
    fn put_index(&self, workspace_id: &str, json: &str) -> Result<(), HostError> {
        self.real().put_index(workspace_id, json)
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        self.real().atomically(f)
    }
}

#[async_trait(?Send)]
impl Store for Watched {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, v: Value) -> Result<(), HostError> {
        self.inner.put(c, id, v).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        self.inner.list(c).await
    }
    async fn keys(&self, c: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(c).await
    }
    async fn delete(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete(c, id).await
    }
    async fn get_blob(&self, c: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.inner.get_blob(c, id).await
    }
    async fn put_blob(&self, c: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.blobs.borrow_mut().push(c.to_string());
        self.inner.put_blob(c, id, bytes).await
    }
    async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(c, id).await
    }
    async fn stat_blob(&self, c: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(c, id).await
    }
    fn transcripts(&self) -> Option<&dyn TranscriptRows> {
        Some(&self.rows)
    }
}
