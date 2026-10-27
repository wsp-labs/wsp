// SPDX-License-Identifier: AGPL-3.0-only
//! What every machines-rest test builds on: an in-memory store that keeps first-write order, a clock the test moves,
//! and a log of warn lines. Written here (rule 13.3) so a test fails on the unit's own `todo!()`, never on wsp-core's
//! or wsp-store's stubs.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::clock::{Cancel, Clock, Timer};
use wsp_store::store::{BlobMark, Store};
use wsp_wire::types::js_number::JsNumber;
use wsp_words::exit::HostError;

/// A store over two ordered maps. A put keeps a key's first place (rowid order, rule 2.5); a delete then a put goes
/// last. `fail_puts` makes every later `put` of that collection fail, `calls` logs every write in order.
#[derive(Default)]
pub struct MemStore {
    pub docs: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    pub blobs: RefCell<IndexMap<String, IndexMap<String, Vec<u8>>>>,
    pub fail_puts: RefCell<Vec<String>>,
    pub calls: RefCell<Vec<String>>,
    pub stamp: Cell<f64>,
}

impl MemStore {
    pub fn new() -> Rc<MemStore> {
        Rc::new(MemStore::default())
    }
    /// The stored document, as JSON.
    pub fn doc(&self, collection: &str, id: &str) -> Option<Value> {
        self.docs
            .borrow()
            .get(collection)
            .and_then(|c| c.get(id).cloned())
    }
    /// The ids of a collection, in store order.
    pub fn ids(&self, collection: &str) -> Vec<String> {
        self.docs
            .borrow()
            .get(collection)
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default()
    }
    pub fn blob(&self, collection: &str, id: &str) -> Option<Vec<u8>> {
        self.blobs
            .borrow()
            .get(collection)
            .and_then(|c| c.get(id).cloned())
    }
    pub fn blob_ids(&self, collection: &str) -> Vec<String> {
        self.blobs
            .borrow()
            .get(collection)
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default()
    }
    /// Seeds a document without logging a call.
    pub fn seed(&self, collection: &str, id: &str, value: Value) {
        self.docs
            .borrow_mut()
            .entry(collection.to_string())
            .or_default()
            .insert(id.to_string(), value);
    }
    pub fn seed_blob(&self, collection: &str, id: &str, bytes: &[u8]) {
        self.blobs
            .borrow_mut()
            .entry(collection.to_string())
            .or_default()
            .insert(id.to_string(), bytes.to_vec());
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

#[async_trait(?Send)]
impl Store for MemStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.doc(collection, id))
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.calls
            .borrow_mut()
            .push(format!("put {collection}/{id}"));
        if self.fail_puts.borrow().iter().any(|c| c == collection) {
            return Err(HostError::new(format!("put {collection} refused")));
        }
        self.docs
            .borrow_mut()
            .entry(collection.to_string())
            .or_default()
            .insert(id.to_string(), value);
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        Ok(self
            .docs
            .borrow()
            .get(collection)
            .map(|c| c.values().cloned().collect())
            .unwrap_or_default())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.ids(collection))
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.calls
            .borrow_mut()
            .push(format!("delete {collection}/{id}"));
        if let Some(c) = self.docs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(self.blob(collection, id))
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.calls
            .borrow_mut()
            .push(format!("putBlob {collection}/{id}"));
        self.blobs
            .borrow_mut()
            .entry(collection.to_string())
            .or_default()
            .insert(id.to_string(), bytes);
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.calls
            .borrow_mut()
            .push(format!("deleteBlob {collection}/{id}"));
        if let Some(c) = self.blobs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        let bytes = self.blob(collection, id);
        let at = self.stamp.get() + 1.0;
        self.stamp.set(at);
        Ok(bytes.map(|b| BlobMark {
            bytes: JsNumber::from(b.len() as f64),
            at: JsNumber::from(at),
        }))
    }
}

/// A clock the test moves with `advance`; timers due run in due order, ties in scheduling order.
#[derive(Default)]
pub struct TestClock {
    pub now: Cell<i64>,
    pub timers: Rc<RefCell<Vec<(i64, u64, Option<Timer>)>>>,
    pub seq: Cell<u64>,
    /// Every delay `schedule` was asked, in order.
    pub asked: RefCell<Vec<i64>>,
}

impl TestClock {
    pub fn at(now: i64) -> Rc<TestClock> {
        let c = TestClock::default();
        c.now.set(now);
        Rc::new(c)
    }
    /// Moves the clock and runs every timer due by then.
    pub fn advance(&self, ms: i64) {
        let until = self.now.get() + ms;
        loop {
            let next = {
                let timers = self.timers.borrow();
                timers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.2.is_some() && t.0 <= until)
                    .min_by_key(|(_, t)| (t.0, t.1))
                    .map(|(i, t)| (i, t.0))
            };
            let Some((i, at)) = next else { break };
            self.now.set(at.max(self.now.get()));
            let f = self.timers.borrow_mut()[i].2.take();
            if let Some(f) = f {
                f();
            }
        }
        self.now.set(until);
    }
    /// How many timers are armed and not cancelled or fired.
    pub fn armed(&self) -> usize {
        self.timers
            .borrow()
            .iter()
            .filter(|t| t.2.is_some())
            .count()
    }
}

impl Clock for TestClock {
    fn now(&self) -> i64 {
        self.now.get()
    }
    fn schedule(&self, f: Timer, ms: i64) -> Cancel {
        self.asked.borrow_mut().push(ms);
        let id = self.seq.get();
        self.seq.set(id + 1);
        self.timers
            .borrow_mut()
            .push((self.now.get() + ms.max(0), id, Some(f)));
        let timers = self.timers.clone();
        Rc::new(move || {
            for t in timers.borrow_mut().iter_mut() {
                if t.1 == id {
                    t.2 = None;
                }
            }
        })
    }
}

/// Lines a fake's `warn` received.
#[derive(Default)]
pub struct Warned(pub RefCell<Vec<String>>);

impl Warned {
    pub fn push(&self, line: &str) {
        self.0.borrow_mut().push(line.to_string());
    }
    pub fn lines(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}

/// Runs a future on a current-thread runtime inside a `LocalSet`, as the host does (rule 3.1).
pub fn run<F: std::future::Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, f)
}

/// A host error with this message and no kind (`new Error(message)`), built from its fields so no wsp-words body runs.
pub fn plain_error(message: &str) -> HostError {
    HostError::new(message)
}
