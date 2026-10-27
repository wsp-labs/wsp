// SPDX-License-Identifier: AGPL-3.0-only
//! The TS tests that reassign `store.put` (runtime-create.test.ts:191): a store that answers like the one it wraps and
//! refuses the writes a test names.
use std::cell::RefCell;
use std::rc::Rc;

use super::Gate;
use async_trait::async_trait;
use serde_json::Value;
use std::ops::ControlFlow;
use wsp_store::sqlite_transcripts::{EventSize, Summed, Total, TranscriptRows};
use wsp_store::store::{BlobMark, Store};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

pub struct WrappedStore {
    inner: Rc<dyn Store>,
    refuse: RefCell<Vec<(String, String)>>,
    unreadable: Rc<std::cell::Cell<bool>>,
    rows: UnreadableRows,
    gated: RefCell<Option<(String, Rc<Gate>, Rc<Gate>)>>,
}

/// The transcript rows of the store a `WrappedStore` wraps, which read as the store's own until a test makes them not
/// read (forget.test.ts:173 `getBlob` answering `undefined` for `transcripts`).
pub struct UnreadableRows {
    inner: Rc<dyn Store>,
    unreadable: Rc<std::cell::Cell<bool>>,
}

impl UnreadableRows {
    fn rows(&self) -> &dyn TranscriptRows {
        self.inner.transcripts().expect("a store that keeps transcript rows")
    }

    fn check(&self) -> Result<(), HostError> {
        if self.unreadable.get() {
            return Err(HostError::new("the rows do not read"));
        }
        Ok(())
    }
}

impl TranscriptRows for UnreadableRows {
    fn all(&self, w: &str) -> Result<Summed, HostError> {
        self.check()?;
        self.rows().all(w)
    }
    fn newest(
        &self,
        w: &str,
        t: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        self.check()?;
        self.rows().newest(w, t, before, each)
    }
    fn count(&self, w: &str, t: &str) -> Result<u64, HostError> {
        self.check()?;
        self.rows().count(w, t)
    }
    fn total(&self, w: &str, t: Option<&str>) -> Result<Total, HostError> {
        self.check()?;
        self.rows().total(w, t)
    }
    fn newest_pos(&self, w: &str) -> Result<i64, HostError> {
        self.check()?;
        self.rows().newest_pos(w)
    }
    fn sizes(&self, w: &str, t: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        self.check()?;
        self.rows().sizes(w, t)
    }
    fn word_events(&self, w: &str, t: &str) -> Result<Vec<SessionEvent>, HostError> {
        self.check()?;
        self.rows().word_events(w, t)
    }
    fn thread(&self, w: &str, t: &str) -> Result<Summed, HostError> {
        self.check()?;
        self.rows().thread(w, t)
    }
    fn at(&self, w: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        self.check()?;
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
        self.check()?;
        self.rows().index(w)
    }
    fn put_index(&self, w: &str, json: &str) -> Result<(), HostError> {
        self.rows().put_index(w, json)
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        self.rows().atomically(f)
    }
}

impl WrappedStore {
    pub fn over(inner: &Rc<dyn Store>) -> Rc<WrappedStore> {
        let unreadable: Rc<std::cell::Cell<bool>> = Rc::default();
        Rc::new(WrappedStore {
            inner: inner.clone(),
            refuse: RefCell::default(),
            rows: UnreadableRows { inner: inner.clone(), unreadable: unreadable.clone() },
            unreadable,
            gated: RefCell::default(),
        })
    }

    /// While `unreadable`, every read of a transcript's rows fails, as the TS's `getBlob` answering `undefined` for
    /// the `transcripts` collection makes the transcript not read.
    pub fn set_unreadable(&self, unreadable: bool) {
        self.unreadable.set(unreadable);
    }

    /// From now on a `put` into `collection` waits at a gate: the first answers `reached` once a write is in it, and
    /// the write goes on once `open` is opened (forget.test.ts:81).
    pub fn gate_puts(&self, collection: &str) -> (Rc<Gate>, Rc<Gate>) {
        let (reached, open) = (Gate::new(), Gate::new());
        *self.gated.borrow_mut() = Some((collection.into(), reached.clone(), open.clone()));
        (reached, open)
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
        let refused = self.refuse.borrow().iter().find(|(c, _)| c == collection).map(|(_, m)| m.clone());
        if let Some(message) = refused {
            return Err(HostError::new(message));
        }
        let gate = self.gated.borrow().as_ref().filter(|(c, _, _)| c == collection).map(|(_, r, o)| (r.clone(), o.clone()));
        if let Some((reached, open)) = gate {
            reached.open();
            open.wait().await;
        }
        self.inner.put(collection, id, value).await
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
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
