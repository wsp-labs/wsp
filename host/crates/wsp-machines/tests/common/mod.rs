// SPDX-License-Identifier: AGPL-3.0-only
//! What every test of the machines-a slice shares (and, as submodules, the fakes places slice a's tests stand on): the oracle fixtures, the scene lookup, the fake clock and ids the
//! oracles mock, the pid names, and a store that records the calls a scene's oracle records.
#![allow(dead_code)]

pub mod backend;
pub mod places;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_core::types::pending::{BlobMark, Clock, Store};
use wsp_core::clock::{Cancel, Timer};
use wsp_core::ids::Ids;
use wsp_wire::types::js_number::JsNumber;
use wsp_words::exit::HostError;

/// The scene clock every oracle starts at.
pub const T0: i64 = 1_790_000_000_000;

/// The pid the fixtures call `dead`: above Linux's pid_max, so no process holds it.
pub const DEAD_PID: u32 = 99_999_999;

/// `tests/fixtures/oracle/<name>.json`, parsed.
pub fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle").join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()))
}

/// The scenes of a fixture: the top-level array, or its `scenes` member.
pub fn scenes(fixture: &Value) -> &Vec<Value> {
    match fixture {
        Value::Array(rows) => rows,
        Value::Object(o) => o.get("scenes").and_then(Value::as_array).expect("fixture has scenes"),
        _ => panic!("fixture is neither an array nor an object"),
    }
}

/// The scene of that name, exactly as the oracle titled it.
pub fn scene<'a>(fixture: &'a Value, name: &str) -> &'a Value {
    scenes(fixture)
        .iter()
        .find(|s| s["name"] == name)
        .unwrap_or_else(|| panic!("no scene named {name:?}"))
}

/// A scene's title as a Rust test name: lowercase, every run of other characters one `_`, cut at 80 (rule 13.10).
pub fn snake(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    out.chars().take(80).collect()
}

/// The pid a fixture names: `self`, `alive` (the parent process, live while the test runs) or `dead`.
pub fn pid_of(name: &str) -> u32 {
    match name {
        "self" => std::process::id(),
        "alive" => std::os::unix::process::parent_id(),
        "dead" => DEAD_PID,
        other => other.parse().unwrap_or_else(|_| panic!("pid name {other:?}")),
    }
}

/// The name a fixture gives a pid.
pub fn pid_name(pid: u32) -> Value {
    if pid == std::process::id() {
        json!("self")
    } else if pid == std::os::unix::process::parent_id() {
        json!("alive")
    } else if pid == DEAD_PID {
        json!("dead")
    } else {
        json!(pid)
    }
}

/// Every `"pid"` member of a value with its fixture name, as the oracles write them.
pub fn scrub_pids(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| match (k.as_str(), x.as_u64()) {
                    ("pid", Some(p)) => (k.clone(), pid_name(p as u32)),
                    _ => (k.clone(), scrub_pids(x)),
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(scrub_pids).collect()),
        other => other.clone(),
    }
}

/// A scene's log, in order, as the oracle's fake writes it: one JSON array per call.
#[derive(Clone, Default)]
pub struct Log(pub Rc<RefCell<Vec<Value>>>);

impl Log {
    pub fn push(&self, entry: Value) {
        self.0.borrow_mut().push(entry);
    }
    pub fn take(&self) -> Vec<Value> {
        self.0.borrow().clone()
    }
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

/// Lets every task the scene started run to where it waits, as the oracles' `flush()` does with 20 to 80 `setImmediate`
/// rounds.
pub async fn flush() {
    for _ in 0..200 {
        tokio::task::yield_now().await;
    }
}

/// Runs a scene on one thread with a `LocalSet`, as the host's owner thread runs (chapter 03, Proposal 2).
pub fn run_local<F: std::future::Future<Output = ()>>(f: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, f);
}

/// The oracles' `fakeClock(T0)`: `advance` runs each due timer at its own time, earliest first, then sets the target.
pub struct FakeClock {
    now: RefCell<i64>,
    seq: RefCell<u64>,
    timers: Rc<RefCell<IndexMap<u64, (i64, Option<Box<dyn FnOnce()>>)>>>,
}

impl FakeClock {
    pub fn new(start: i64) -> Rc<Self> {
        Rc::new(FakeClock { now: RefCell::new(start), seq: RefCell::new(0), timers: Rc::new(RefCell::new(IndexMap::new())) })
    }

    pub fn advance(&self, ms: i64) {
        let target = *self.now.borrow() + ms;
        loop {
            let next = {
                let timers = self.timers.borrow();
                timers.iter().filter(|(_, (at, _))| *at <= target).min_by_key(|(id, (at, _))| (*at, **id)).map(|(id, (at, _))| (*id, *at))
            };
            let Some((id, at)) = next else { break };
            let run = self.timers.borrow_mut().shift_remove(&id).and_then(|(_, f)| f);
            *self.now.borrow_mut() = at;
            if let Some(run) = run {
                run();
            }
        }
        *self.now.borrow_mut() = target;
    }

    pub fn pending(&self) -> usize {
        self.timers.borrow().len()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        *self.now.borrow()
    }

    fn schedule(&self, run: Timer, ms: i64) -> Cancel {
        let id = {
            let mut seq = self.seq.borrow_mut();
            *seq += 1;
            *seq
        };
        let at = *self.now.borrow() + ms;
        self.timers.borrow_mut().insert(id, (at, Some(run)));
        let timers = Rc::clone(&self.timers);
        Rc::new(move || {
            timers.borrow_mut().shift_remove(&id);
        })
    }
}

/// The ids the oracles mock: the `k`-th `bytes(n)` is byte `k` repeated `n` times (`randomBytes` answered with
/// `Buffer.alloc(n, k)`); `uuid()` counts on its own.
#[derive(Default)]
pub struct FakeIds {
    bytes: RefCell<u8>,
    uuids: RefCell<u32>,
}

impl Ids for FakeIds {
    fn uuid(&self) -> String {
        let mut n = self.uuids.borrow_mut();
        *n += 1;
        format!("00000000-0000-4000-8000-{:012x}", *n)
    }

    fn bytes(&self, n: usize) -> Vec<u8> {
        let mut k = self.bytes.borrow_mut();
        *k = k.wrapping_add(1);
        vec![*k; n]
    }

    fn random(&self) -> f64 {
        0.0
    }
}

/// Which store calls a scene's oracle writes to its log.
#[derive(Clone, Copy, Default)]
pub struct StoreLogs {
    pub get: bool,
    pub put: bool,
    pub delete: bool,
    pub list: bool,
    pub blobs: bool,
}

/// A memory store whose collections keep insertion order, logging the calls `logs` names as the oracles do:
/// `["store.get", c, k]`, `["store.put", c, k, v]` (pids scrubbed), `["store.delete", c, k]`, `["store.list", c]`,
/// `["store.putBlob", c, k, text]`, `["store.getBlob", c, k]`, `["store.deleteBlob", c, k]`.
pub struct MemStore {
    pub rows: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    pub blobs: RefCell<IndexMap<String, IndexMap<String, Vec<u8>>>>,
    pub log: Log,
    pub logs: StoreLogs,
}

impl MemStore {
    /// A store seeded from a fixture's `{collection: {key: row}}`, pid names turned back into pids.
    pub fn seeded(seed: &Value, log: Log, logs: StoreLogs) -> Rc<Self> {
        let mut rows: IndexMap<String, IndexMap<String, Value>> = IndexMap::new();
        if let Some(o) = seed.as_object() {
            for (c, entries) in o {
                let table = rows.entry(c.clone()).or_default();
                for (k, v) in entries.as_object().into_iter().flatten() {
                    table.insert(k.clone(), unscrub_pids(v));
                }
            }
        }
        Rc::new(MemStore { rows: RefCell::new(rows), blobs: RefCell::new(IndexMap::new()), log, logs })
    }

    /// One collection as `[[key, row], ...]`, pids named, as the oracles write `rows(...)`.
    pub fn table(&self, collection: &str) -> Value {
        let rows = self.rows.borrow();
        Value::Array(rows.get(collection).into_iter().flatten().map(|(k, v)| json!([k, scrub_pids(v)])).collect())
    }
}

/// The pids a fixture names, turned back into numbers.
pub fn unscrub_pids(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| match (k.as_str(), x.as_str()) {
                    ("pid", Some(name)) => (k.clone(), json!(pid_of(name))),
                    _ => (k.clone(), unscrub_pids(x)),
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(unscrub_pids).collect()),
        other => other.clone(),
    }
}

#[async_trait(?Send)]
impl Store for MemStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        if self.logs.get {
            self.log.push(json!(["store.get", collection, id]));
        }
        Ok(self.rows.borrow().get(collection).and_then(|t| t.get(id)).cloned())
    }

    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        if self.logs.put {
            self.log.push(json!(["store.put", collection, id, scrub_pids(&value)]));
        }
        self.rows.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value);
        Ok(())
    }

    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        if self.logs.list {
            self.log.push(json!(["store.list", collection]));
        }
        Ok(self.rows.borrow().get(collection).map(|t| t.values().cloned().collect()).unwrap_or_default())
    }

    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.rows.borrow().get(collection).map(|t| t.keys().cloned().collect()).unwrap_or_default())
    }

    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if self.logs.delete {
            self.log.push(json!(["store.delete", collection, id]));
        }
        if let Some(t) = self.rows.borrow_mut().get_mut(collection) {
            t.shift_remove(id);
        }
        Ok(())
    }

    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        if self.logs.blobs {
            self.log.push(json!(["store.getBlob", collection, id]));
        }
        Ok(self.blobs.borrow().get(collection).and_then(|t| t.get(id)).cloned())
    }

    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        if self.logs.blobs {
            self.log.push(json!(["store.putBlob", collection, id, String::from_utf8_lossy(&bytes)]));
        }
        self.blobs.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), bytes);
        Ok(())
    }

    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if self.logs.blobs {
            self.log.push(json!(["store.deleteBlob", collection, id]));
        }
        if let Some(t) = self.blobs.borrow_mut().get_mut(collection) {
            t.shift_remove(id);
        }
        Ok(())
    }

    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        Ok(self.blobs.borrow().get(collection).and_then(|t| t.get(id)).map(|b| BlobMark { bytes: JsNumber(b.len() as f64), at: JsNumber(0.0) }))
    }
}

/// The entries a scene's act writes itself, not the area: what a call answered, where the act stood, what a wait
/// saw. Their place against the area's calls follows JavaScript's microtask order (the TS resumes the act after a
/// detached task's first steps; a Rust `spawn_local` runs after the act), so they are compared as a list of their own.
pub const MARKERS: &[&str] = &[
    "answer", "answered", "rejected", "mark", "paid", "joined", "same", "during", "after", "unread", "unreadAt", "revivedAt",
    "of", "turnRuns", "isHeldAway after", "inflight inside", "waiting", "settled before the deploy ends", "settled after",
];

fn is_marker(entry: &Value) -> bool {
    entry.get(0).and_then(Value::as_str).is_some_and(|name| MARKERS.contains(&name))
}

/// Asserts a scene's log equals the fixture's: the area's calls in strict order, entry by entry, so a failure names
/// the first that differs; the act's own markers (`MARKERS`) in their own order.
pub fn assert_log(scene: &Value, got: &[Value]) {
    let want = scene["log"].as_array().expect("scene has a log");
    let (want_marks, want_calls): (Vec<&Value>, Vec<&Value>) = want.iter().partition(|e| is_marker(e));
    let (got_marks, got_calls): (Vec<&Value>, Vec<&Value>) = got.iter().partition(|e| is_marker(e));
    for (i, (w, g)) in want_calls.iter().zip(got_calls.iter()).enumerate() {
        assert_eq!(g, w, "scene {:?}: call {i} differs", scene["name"]);
    }
    assert_eq!(got_calls.len(), want_calls.len(), "scene {:?}: calls differ in number; got {:#?}", scene["name"], got_calls);
    assert_eq!(got_marks, want_marks, "scene {:?}: the act's markers differ", scene["name"]);
}
