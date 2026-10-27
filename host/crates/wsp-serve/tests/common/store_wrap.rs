// SPDX-License-Identifier: AGPL-3.0-only
//! The TS tests that reassign `store.put` (runtime-create.test.ts:191): a store that answers like the one it wraps and
//! refuses the writes a test names.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::Value;
use wsp_store::sqlite_transcripts::TranscriptRows;
use wsp_store::store::{BlobMark, Store};
use wsp_words::exit::HostError;

pub struct WrappedStore {
    inner: Rc<dyn Store>,
    refuse: RefCell<Vec<(String, String)>>,
}

impl WrappedStore {
    pub fn over(inner: &Rc<dyn Store>) -> Rc<WrappedStore> {
        Rc::new(WrappedStore { inner: inner.clone(), refuse: RefCell::default() })
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
        self.inner.transcripts()
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
