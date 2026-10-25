// SPDX-License-Identifier: AGPL-3.0-only
//! What the wsp-core tests share: the owner-thread runner, waits, an in-memory store, seeded ids, the network fakes
//! (an HTTP stub, a refusing door, a TCP proxy and a fake daemon), and the exec tests' own-process-group runner,
//! strays helper, gate loop, scripted guest machine and stream readers. Each test file includes what it uses.
#![allow(dead_code)]

pub mod fake_daemon;
pub mod guest;
pub mod net;
pub mod own_group;
pub mod status;
pub mod strays;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_adapters::port::{ExecFailure, ExecStream};
use wsp_core::ids::Ids;
use wsp_core::types::pending::{BlobMark, Store};
use wsp_wire::types::js_number::JsNumber;
use wsp_words::exit::HostError;

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

/// A plain error, built from its fields (wsp-words' constructors are not implemented yet).
pub fn broke(message: &str) -> HostError {
    HostError { message: message.into(), fix: None, kind: None, errors: None, warnings: None, left: None, marks: vec![] }
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
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.text(collection, id).map(|t| serde_json::from_str(&t).unwrap()))
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        if self.fail_put.get() {
            return Err(broke("the disk is full"));
        }
        self.puts.set(self.puts.get() + 1);
        self.data.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value.to_string());
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        if self.fail_list.get() {
            return Err(broke("the store is gone"));
        }
        let data = self.data.borrow();
        Ok(data.get(collection).map(|c| c.values().map(|t| serde_json::from_str(t).unwrap()).collect()).unwrap_or_default())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.ids(collection))
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if self.fail_delete.get() {
            return Err(broke("the disk is read-only"));
        }
        self.deletes.set(self.deletes.get() + 1);
        if let Some(c) = self.data.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(self.blobs.borrow().get(collection).and_then(|c| c.get(id)).map(|(b, _)| b.clone()))
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.blobs.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), (bytes, 0.0));
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if let Some(c) = self.blobs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        Ok(self.blobs.borrow().get(collection).and_then(|c| c.get(id)).map(|(b, at)| BlobMark { bytes: JsNumber(b.len() as f64), at: JsNumber(*at) }))
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

/// Bash lines that give a Mac what a Linux machine's runs lean on; empty on Linux (protocol/test/linux-shell.ts).
pub fn linux_shell_prelude() -> &'static str {
    if std::path::Path::new("/proc/1/stat").exists() {
        ""
    } else {
        "setsid() { exec perl -e 'setpgrp(0, 0); exec @ARGV or die $!' -- \"$@\"; }\nbase64() { local a=(); for x in \"$@\"; do [ \"$x\" = \"-w0\" ] || a+=(\"$x\"); done; /usr/bin/base64 \"${a[@]}\"; }\nexport -f setsid base64\n"
    }
}

/// A shell loop that waits for the gate file, running `each` on every pass, and ends its shell once the gate's folder
/// is gone or after `limit_s` seconds (protocol/test/stub-script.ts `gateLoop`).
pub fn gate_loop(gate: &str, each: &str, limit_s: u64) -> String {
    let pass = if each.is_empty() { String::new() } else { format!("{each}; ") };
    format!(
        "_g=\"{gate}\"; _end=$(($(date +%s) + {limit_s})); _n=0; while [ ! -f \"$_g\" ]; do {{ [ -d \"${{_g%/*}}\" ] && {{ [ $((_n % 20)) -ne 0 ] || [ \"$(date +%s)\" -le $_end ]; }}; }} || exit 1; {pass}sleep 0.05; _n=$((_n + 1)); done"
    )
}

/// `gate_loop` with no pass and the 30 s limit.
pub fn gate(gate: &str) -> String {
    gate_loop(gate, "", 30)
}

/// Every line of a stream, or the lines before its failure and the failure.
pub async fn collect(stream: &dyn ExecStream) -> (Vec<String>, Option<ExecFailure>) {
    let mut lines = stream.lines();
    let mut out = Vec::new();
    while let Some(next) = lines.next().await {
        match next {
            Ok(line) => out.push(line),
            Err(e) => return (out, Some(e)),
        }
    }
    (out, None)
}

/// Every line of a stream that must end without a failure.
pub async fn lines_ok(stream: &dyn ExecStream) -> Vec<String> {
    let (lines, failure) = collect(stream).await;
    assert!(failure.is_none(), "the stream failed after {lines:?}: {failure:?}");
    lines
}

/// The failure a stream must end on, with the lines before it.
pub async fn lines_err(stream: &dyn ExecStream) -> (Vec<String>, ExecFailure) {
    let (lines, failure) = collect(stream).await;
    (lines.clone(), failure.unwrap_or_else(|| panic!("the stream ended without a failure after {lines:?}")))
}

/// Waits until `check` holds, polling every 20 ms, failing after `timeout_ms` with `what`.
pub async fn wait_for(what: &str, timeout_ms: u64, mut check: impl FnMut() -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    while !check() {
        assert!(std::time::Instant::now() < until, "waited {timeout_ms} ms for {what}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// A real sleep.
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

/// Races `fut` against `ms`; `None` when the time ran out first.
pub async fn within<T>(ms: u64, fut: impl std::future::Future<Output = T>) -> Option<T> {
    tokio::time::timeout(std::time::Duration::from_millis(ms), fut).await.ok()
}

/// The text of a file, or "" where it cannot be read.
pub fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// A file's permission bits as octal text, `600` say.
pub fn mode_of(path: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    format!("{:o}", std::fs::metadata(path).expect(path).permissions().mode() & 0o777)
}

/// The names in a folder, sorted.
pub fn names(dir: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// Every file under `dir`, at any depth, whose bytes hold `value`.
pub fn holding(dir: &str, value: &str) -> Vec<String> {
    fn walk(dir: &std::path::Path, value: &str, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, value, out);
            } else if std::fs::read(&p).map(|b| String::from_utf8_lossy(&b).contains(value)).unwrap_or(false) {
                out.push(p.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(std::path::Path::new(dir), value, &mut out);
    out.sort();
    out
}

/// An exec's exit code as the wire carries it.
pub fn code(n: i64) -> wsp_wire::types::js_number::JsInt {
    wsp_wire::types::js_number::JsInt(wsp_wire::types::js_number::JsNumber::from(n))
}

/// `shell_quote` as the protocol writes it, for test text only: `'...'`, an inner `'` as `'\''`.
pub fn sq(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
