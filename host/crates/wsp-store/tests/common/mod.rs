// SPDX-License-Identifier: AGPL-3.0-only
// What every wsp-store test file shares: a poll of the store's futures, the TS oracle's fixtures, events built in the
// schema's key order, and a state folder of the test's own.
#![allow(dead_code)]
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use serde_json::{json, Value};
use wsp_store::store::StateWriter;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::session_events_more::SessionEvent;

/// Runs a store future to its end. The stores never wait on anything (SQLite is synchronous, rule 2.20), so a future
/// that is still pending after its first poll is a bug the test reports.
pub fn run<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    match f.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(out) => out,
        Poll::Pending => panic!("a store future waited, and nothing here wakes it"),
    }
}

/// One fixture the TS oracle wrote (oracle/store.oracle.ts).
pub fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/store").join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// The writer the TS suites name (store.test.ts:19).
pub fn writer() -> StateWriter {
    StateWriter { wsp: "test".into(), daemon: JsInt::from(1), bin: "/usr/local/bin/wsp".into() }
}

/// A folder of the test's own and the state file path in it, which is only an anchor (02).
pub struct Home {
    pub dir: tempfile::TempDir,
}

impl Home {
    pub fn new() -> Home {
        Home { dir: tempfile::tempdir().unwrap() }
    }
    pub fn state(&self) -> PathBuf {
        self.dir.path().join("state.json")
    }
    pub fn db(&self) -> PathBuf {
        self.dir.path().join("state.db")
    }
    pub fn names(&self) -> Vec<String> {
        let mut out: Vec<String> =
            std::fs::read_dir(self.dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        out.sort();
        out
    }
}

/// An event off its JSON, as the oracle wrote it.
pub fn event(v: Value) -> SessionEvent {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"))
}

/// An event back to JSON, to compare with the oracle's.
pub fn value(e: &SessionEvent) -> Value {
    serde_json::to_value(e).unwrap()
}

/// A `session.delta` in the schema's key order (sqlite-transcripts.test.ts:37-38 builds the same keys).
pub fn delta(thread: Option<&str>, pos: Option<i64>, text: &str, kind: &str) -> SessionEvent {
    let mut v = json!({"type": "session.delta", "workspaceId": "w", "sessionId": "s"});
    let o = v.as_object_mut().unwrap();
    if let Some(t) = thread {
        o.insert("threadId".into(), json!(t));
    }
    if let Some(p) = pos {
        o.insert("pos".into(), json!(p));
    }
    o.insert("kind".into(), json!(kind));
    o.insert("text".into(), json!(text));
    event(v)
}

pub fn text(thread: Option<&str>, pos: i64, text: &str) -> SessionEvent {
    delta(thread, Some(pos), text, "text")
}

/// The `pos` of an event, `None` where it carries none.
pub fn pos_of(e: &SessionEvent) -> Option<i64> {
    value(e).get("pos").and_then(Value::as_i64)
}

/// A string's length as JavaScript counts it.
pub fn utf16(s: &str) -> u64 {
    s.encode_utf16().count() as u64
}
