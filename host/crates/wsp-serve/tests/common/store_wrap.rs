// SPDX-License-Identifier: AGPL-3.0-only
//! The TS tests that reassign `store.put` (runtime-create.test.ts:191): a store that answers like the one it wraps and
//! refuses the writes a test names.
use std::cell::RefCell;
use std::rc::Rc;

use std::ops::ControlFlow;

use async_trait::async_trait;
use serde_json::Value;
use wsp_store::sqlite_transcripts::{EventSize, Summed, Total, TranscriptRows};
use wsp_store::store::{BlobMark, Store};
use wsp_wire::views::session_events::SessionEvent;
use wsp_words::exit::HostError;

use super::Gate;

pub struct WrappedStore {
    inner: Rc<dyn Store>,
    refuse: RefCell<Vec<(String, String)>>,
    refuse_blobs: RefCell<Vec<(String, String)>>,
    refuse_once: RefCell<Vec<(String, String)>>,
    holds: RefCell<Vec<(String, Rc<Gate>)>>,
    puts: RefCell<Vec<String>>,
    rows: FailingRows,
}

/// The transcript rows of the store it wraps, whose reads fail once told to (runtime-comeback.test.ts:812 makes
/// `getBlob("transcripts")` reject: the Rust store keeps transcripts as rows).
pub struct FailingRows {
    inner: Rc<dyn Store>,
    message: RefCell<Option<String>>,
    /// The next `put_index` fails with this, and the ones after it do not (transcript-index.test.ts: "an index write that fails").
    index_once: RefCell<Option<String>>,
}

impl FailingRows {
    fn rows(&self) -> &dyn TranscriptRows {
        self.inner.transcripts().expect("a store that keeps transcript rows")
    }
    fn read(&self) -> Result<(), HostError> {
        match self.message.borrow().clone() {
            Some(message) => Err(HostError::new(message)),
            None => Ok(()),
        }
    }
}

impl TranscriptRows for FailingRows {
    fn all(&self, w: &str) -> Result<Summed, HostError> {
        self.read()?;
        self.rows().all(w)
    }
    fn newest(
        &self,
        w: &str,
        t: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        self.read()?;
        self.rows().newest(w, t, before, each)
    }
    fn count(&self, w: &str, t: &str) -> Result<u64, HostError> {
        self.read()?;
        self.rows().count(w, t)
    }
    fn total(&self, w: &str, t: Option<&str>) -> Result<Total, HostError> {
        self.read()?;
        self.rows().total(w, t)
    }
    fn newest_pos(&self, w: &str) -> Result<i64, HostError> {
        self.read()?;
        self.rows().newest_pos(w)
    }
    fn sizes(&self, w: &str, t: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        self.read()?;
        self.rows().sizes(w, t)
    }
    fn word_events(&self, w: &str, t: &str) -> Result<Vec<SessionEvent>, HostError> {
        self.read()?;
        self.rows().word_events(w, t)
    }
    fn thread(&self, w: &str, t: &str) -> Result<Summed, HostError> {
        self.read()?;
        self.rows().thread(w, t)
    }
    fn at(&self, w: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        self.read()?;
        self.rows().at(w, positions)
    }
    fn append(&self, w: &str, events: &[SessionEvent]) -> Result<(), HostError> {
        self.rows().append(w, events)
    }
    fn remove(&self, w: &str, positions: &[i64]) -> Result<(), HostError> {
        self.rows().remove(w, positions)
    }
    fn clear(&self, w: &str) -> Result<(), HostError> {
        self.rows().clear(w)
    }
    fn index(&self, w: &str) -> Result<Option<String>, HostError> {
        self.read()?;
        self.rows().index(w)
    }
    fn put_index(&self, w: &str, json: &str) -> Result<(), HostError> {
        if let Some(message) = self.index_once.borrow_mut().take() {
            return Err(HostError::new(message));
        }
        self.rows().put_index(w, json)
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        self.rows().atomically(f)
    }
}

impl WrappedStore {
    pub fn over(inner: &Rc<dyn Store>) -> Rc<WrappedStore> {
        Rc::new(WrappedStore {
            inner: inner.clone(),
            refuse: RefCell::default(),
            refuse_blobs: RefCell::default(),
            refuse_once: RefCell::default(),
            holds: RefCell::default(),
            puts: RefCell::default(),
            rows: FailingRows { inner: inner.clone(), message: RefCell::default(), index_once: RefCell::default() },
        })
    }

    /// The next `put` into `collection` fails with `message`, and the ones after it do not (reconcile.test.ts `refusingOnce`).
    pub fn refuse_next_put(&self, collection: &str, message: &str) {
        self.refuse_once.borrow_mut().push((collection.into(), message.into()));
    }

    /// Every `put_blob` into `collection` fails with `message` from now on (session-images.test.ts: the host dies between two writes).
    pub fn refuse_blob_puts(&self, collection: &str, message: &str) {
        self.refuse_blobs.borrow_mut().push((collection.into(), message.into()));
    }

    /// Every `list` of `collection` waits for `gate` first (runtime-comeback.test.ts:313: a store whose sessions listing answers last).
    pub fn hold_list(&self, collection: &str, gate: &Rc<Gate>) {
        self.holds.borrow_mut().push((collection.into(), gate.clone()));
    }

    /// How many `put`s into `collection` the store has seen (`vi.spyOn(store, "put")`).
    pub fn puts_into(&self, collection: &str) -> usize {
        self.puts.borrow().iter().filter(|c| c.as_str() == collection).count()
    }

    /// The next write of a transcript's index fails with `message`.
    pub fn fail_next_index_write(&self, message: &str) {
        *self.rows.index_once.borrow_mut() = Some(message.into());
    }

    /// Every read of a transcript fails with `message` from now on.
    pub fn fail_transcripts(&self, message: &str) {
        *self.rows.message.borrow_mut() = Some(message.into());
    }

    /// Every `put` into `collection` fails with `message` from now on.
    pub fn refuse_puts(&self, collection: &str, message: &str) {
        self.refuse.borrow_mut().push((collection.into(), message.into()));
    }

    /// The wrapper as the `Rc<dyn Store>` a runtime takes.
    pub fn store(self: &Rc<Self>) -> Rc<dyn Store> {
        self.clone()
    }
}

#[async_trait(?Send)]
impl Store for WrappedStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(collection, id).await
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.puts.borrow_mut().push(collection.to_string());
        let refused = self.refuse.borrow().iter().find(|(c, _)| c == collection).map(|(_, m)| m.clone());
        if let Some(message) = refused {
            return Err(HostError::new(message));
        }
        let once = {
            let mut once = self.refuse_once.borrow_mut();
            once.iter().position(|(c, _)| c == collection).map(|at| once.remove(at).1)
        };
        if let Some(message) = once {
            return Err(HostError::new(message));
        }
        self.inner.put(collection, id, value).await
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        let held = self.holds.borrow().iter().find(|(c, _)| c == collection).map(|(_, g)| g.clone());
        if let Some(gate) = held {
            gate.wait().await;
        }
        self.inner.list(collection).await
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(collection).await
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete(collection, id).await
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.inner.get_blob(collection, id).await
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        let refused = self.refuse_blobs.borrow().iter().find(|(c, _)| c == collection).map(|(_, m)| m.clone());
        if let Some(message) = refused {
            return Err(HostError::new(message));
        }
        self.inner.put_blob(collection, id, bytes).await
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(collection, id).await
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(collection, id).await
    }
    fn transcripts(&self) -> Option<&dyn TranscriptRows> {
        self.inner.transcripts().map(|_| &self.rows as &dyn TranscriptRows)
    }
}

/// `vaultPuts(store)`: a store that counts the blobs written to the `vaults` collection and otherwise is the one it wraps.
pub struct CountingStore {
    pub inner: Rc<dyn Store>,
    pub puts: std::cell::Cell<u32>,
}

mod counting {
    use super::*;

    #[async_trait(?Send)]
    impl Store for CountingStore {
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
            if c == "vaults" {
                self.puts.set(self.puts.get() + 1);
            }
            self.inner.put_blob(c, id, bytes).await
        }
        async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
            self.inner.delete_blob(c, id).await
        }
        async fn stat_blob(&self, c: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
            self.inner.stat_blob(c, id).await
        }
        fn transcripts(&self) -> Option<&dyn TranscriptRows> {
            self.inner.transcripts()
        }
    }
}
