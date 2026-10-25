// SPDX-License-Identifier: AGPL-3.0-only
//! What the wsp-core tests share: the owner-thread runner, waits, an in-memory store, seeded ids, and the network
//! fakes (an HTTP stub, a refusing door, a TCP proxy and a fake daemon).
#![allow(dead_code)]

pub mod fake_daemon;
pub mod net;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_core::ids::Ids;
use wsp_core::store::{BlobMark, Store, StoreError};

/// Runs a test body on one thread with a `LocalSet`, as the host runs (rule 3.1).
pub fn run_local<F: Future<Output = ()>>(body: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    tokio::task::LocalSet::new().block_on(&rt, body);
}

/// Lets every spawned task that can run, run: the TS tests' `setImmediate` wait.
pub async fn settle() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

/// Waits in real time until `cond` holds, or panics after `ms`.
pub async fn until(what: &str, ms: u64, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// A store over JSON text, so every read parses afresh (rule 2.27). Writes are counted, and a list, a put or a
/// delete can be made to fail.
#[derive(Default)]
pub struct MemStore {
    pub data: RefCell<IndexMap<String, IndexMap<String, String>>>,
    pub blobs: RefCell<IndexMap<String, IndexMap<String, (Vec<u8>, f64)>>>,
    pub puts: Cell<u32>,
    pub deletes: Cell<u32>,
    pub fail_list: Cell<bool>,
    pub fail_put: Cell<bool>,
    pub fail_delete: Cell<bool>,
}

impl MemStore {
    pub fn new() -> std::rc::Rc<MemStore> {
        std::rc::Rc::new(MemStore::default())
    }
    /// The stored text of one document, as the state file holds it.
    pub fn text(&self, collection: &str, id: &str) -> Option<String> {
        self.data.borrow().get(collection).and_then(|c| c.get(id)).cloned()
    }
    pub fn ids(&self, collection: &str) -> Vec<String> {
        self.data.borrow().get(collection).map(|c| c.keys().cloned().collect()).unwrap_or_default()
    }
    pub fn seed(&self, collection: &str, id: &str, value: Value) {
        self.data.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value.to_string());
    }
}

#[async_trait(?Send)]
impl Store for MemStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, StoreError> {
        Ok(self.text(collection, id).map(|t| serde_json::from_str(&t).unwrap()))
    }
    async fn put(&self, collection: &str, id: &str, value: &Value) -> Result<(), StoreError> {
        if self.fail_put.get() {
            return Err(StoreError("the disk is full".into()));
        }
        self.puts.set(self.puts.get() + 1);
        self.data.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value.to_string());
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, StoreError> {
        if self.fail_list.get() {
            return Err(StoreError("the store is gone".into()));
        }
        let data = self.data.borrow();
        Ok(data.get(collection).map(|c| c.values().map(|t| serde_json::from_str(t).unwrap()).collect()).unwrap_or_default())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, StoreError> {
        Ok(self.ids(collection))
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), StoreError> {
        if self.fail_delete.get() {
            return Err(StoreError("the disk is read-only".into()));
        }
        self.deletes.set(self.deletes.get() + 1);
        if let Some(c) = self.data.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.blobs.borrow().get(collection).and_then(|c| c.get(id)).map(|(b, _)| b.clone()))
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: &[u8]) -> Result<(), StoreError> {
        self.blobs.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), (bytes.to_vec(), 0.0));
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), StoreError> {
        if let Some(c) = self.blobs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, StoreError> {
        Ok(self.blobs.borrow().get(collection).and_then(|c| c.get(id)).map(|(b, at)| BlobMark { bytes: b.len() as u64, at: *at }))
    }
}

/// Ids a test can predict: byte `k` of the `n`th draw is `(n * 31 + k) % 256`; uuids count up; `random` is fixed.
pub struct SeqIds {
    pub draws: Cell<u64>,
    pub random: Cell<f64>,
}

impl SeqIds {
    pub fn new() -> std::rc::Rc<SeqIds> {
        std::rc::Rc::new(SeqIds { draws: Cell::new(0), random: Cell::new(0.0) })
    }
    /// The bytes the `n`th draw of `len` gives (draws count from 1).
    pub fn expected(n: u64, len: usize) -> Vec<u8> {
        (0..len).map(|k| ((n * 31 + k as u64) % 256) as u8).collect()
    }
}

impl Ids for SeqIds {
    fn uuid(&self) -> String {
        let n = self.draws.get() + 1;
        self.draws.set(n);
        format!("00000000-0000-4000-8000-{n:012}")
    }
    fn bytes(&self, len: usize) -> Vec<u8> {
        let n = self.draws.get() + 1;
        self.draws.set(n);
        SeqIds::expected(n, len)
    }
    fn random(&self) -> f64 {
        self.random.get()
    }
}
