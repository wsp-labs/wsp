// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448): the slates, which the round left to their 500-level cap. Each
//! case runs in a child copy of this binary on a 1 MiB thread and fails when the child dies or the case fails.
mod support;

use std::os::unix::process::ExitStatusExt;
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{as_thread, temp_root, FakeHost, MemStore, THREAD};
use wsp_slates::slates::{ReadParams, SlateTarget, Slates, StateParams, WriteParams};
use wsp_wire::views::workspace::Caller;

const CHILD: &str = "REVIEW_2448_CHILD";

fn in_child(name: &str, depth: usize, body: fn(usize)) {
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(move || body(depth)).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .output()
        .unwrap();
    let died = out.status.signal().is_some();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = stderr.lines().filter(|l| l.contains("HostError") || l.contains("overflowed")).take(1).collect();
    assert!(
        out.status.success(),
        "{depth}: {} ({}): {}",
        if died { "the whole process died" } else { "the case failed" },
        out.status,
        tail.join(" | ")
    );
}

fn lists(n: usize, leaf: Value) -> Value {
    let mut v = leaf;
    for _ in 0..n {
        v = Value::Array(vec![v]);
    }
    v
}

fn record_text(n: usize) -> String {
    format!("{}1{}", "{a: ".repeat(n), "}".repeat(n))
}

fn agent() -> Caller {
    as_thread(THREAD, THREAD)
}

fn target() -> SlateTarget {
    SlateTarget { thread_id: Some(THREAD.into()), turn_token: None }
}

fn text_write(text: &str) -> WriteParams {
    WriteParams { text: Some(text.into()), ..Default::default() }
}

fn written_and_read(text: String) {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("r2448-");
        let host = FakeHost::new(root.path(), MemStore::new());
        let slates = host.slates();
        slates.write(text_write(&text), Some(&agent())).await.expect("a write");
        slates.settled().await;
        let read =
            ReadParams { target: target(), values: Some(vec!["*".into()]), text: Some(true), sketch: Some(true), document: Some(true) };
        let read = slates.read(read, Some(&agent())).await.expect("a read");
        let _ = serde_json::to_string(&read.to_json());
    }));
}

// A start literal passes the 500-level literal cap (syntax/literal.rs:6), but `write` walks a record start value by
// recursion and dies at about 465 levels on 1 MiB in a debug build (lists hold to 500; release holds). slate_write text
// comes from a thread's tool call. node (slate.ts) writes and reads it.
fn record_start_body(levels: usize) {
    written_and_read(format!(
        r#"<slate><value name="v" start={{{}}} /><column><text>{{len($v)}}</text></column></slate>"#,
        record_text(levels)
    ));
}

#[test]
fn a_start_value_of_500_nested_records_is_written_and_read() {
    in_child("a_start_value_of_500_nested_records_is_written_and_read", 500, record_start_body);
}

// Derived values pass no depth check: each holds 16 expression levels (expr_depth), and 32 may chain. Four derived
// values, each the one before inside 15 lists, 60 levels in all with no deep value anywhere, overflow `write` on 1 MiB in
// a debug build. node writes and reads it.
fn four_derived_body(steps: usize) {
    let mut out = String::from(r#"<slate><value name="v" start={[1]} />"#);
    for i in 0..steps {
        let prev = if i == 0 { "$v".to_string() } else { format!("$d{}", i - 1) };
        out.push_str(&format!(r#"<derived name="d{i}" value={{{}{prev}{}}} />"#, "[".repeat(15), "]".repeat(15)));
    }
    out.push_str(&format!(r#"<column><text>{{len($d{})}}</text></column></slate>"#, steps - 1));
    written_and_read(out);
}

#[test]
fn four_derived_values_of_fifteen_nested_lists_each_are_written_and_read() {
    in_child("four_derived_values_of_fifteen_nested_lists_each_are_written_and_read", 4, four_derived_body);
}

// Not from the round, and not a stack: a slate value may nest 500 levels (paths.rs:119), the sqlite store reads a row back
// at serde_json's 128 (wsp-store sqlite_store.rs:520 parse_row), and `load` lists every slate row at once (access.rs:196).
// So after a restart one slate holding a value 126 levels deep makes every slate on the host fail to load, another
// thread's shallow one included. node's JSON.parse reads the row.
struct SqlHost {
    inner: Rc<FakeHost>,
    store: Rc<dyn wsp_store::store::Store>,
}

#[async_trait::async_trait(?Send)]
impl wsp_slates::slates::SlatesDeps for SqlHost {
    fn store(&self) -> Rc<dyn wsp_store::store::Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn wsp_core::clock::Clock> {
        wsp_slates::slates::SlatesDeps::clock(&*self.inner)
    }
    fn record(&self, event: wsp_wire::types::slate_wire::SessionSlateEvent) {
        self.inner.record(event)
    }
    fn emit(&self, event: wsp_slates::slates::SlateEmit) {
        self.inner.emit(event)
    }
    fn thread(&self, thread_id: &str) -> Option<wsp_slates::slates::SlateThreadFacts> {
        self.inner.thread(thread_id)
    }
    fn under(&self, lead: &str) -> Vec<String> {
        self.inner.under(lead)
    }
    fn thread_of_token(&self, token: &str) -> Result<String, wsp_words::exit::HostError> {
        self.inner.thread_of_token(token)
    }
    fn sources(&self, thread_id: &str, workspace_id: &str) -> Rc<dyn wsp_slates::slate_sources::SlateSourceContext> {
        self.inner.sources(thread_id, workspace_id)
    }
    async fn deliver(&self, o: wsp_slates::slates::Delivery) -> Result<wsp_slates::slates::Delivered, wsp_words::exit::HostError> {
        self.inner.deliver(o).await
    }
    fn run_env(&self) -> IndexMap<String, String> {
        self.inner.run_env()
    }
    fn secrets_file(&self) -> Option<std::path::PathBuf> {
        self.inner.secrets_file()
    }
    fn slates_dir(&self) -> Option<std::path::PathBuf> {
        Some(self.inner.slates_dir())
    }
}

fn sqlite_area(root: &std::path::Path) -> (Rc<SqlHost>, Rc<dyn Slates>) {
    let writer = wsp_store::store::StateWriter {
        wsp: "test".into(),
        daemon: wsp_wire::types::js_number::JsInt::from(1),
        bin: "/usr/local/bin/wsp".into(),
    };
    let store: Rc<dyn wsp_store::store::Store> =
        Rc::new(wsp_store::sqlite_store::sqlite_store(&root.join("state").join("state.json"), writer));
    let inner = FakeHost::new(root, MemStore::new());
    let host = Rc::new(SqlHost { inner, store });
    let slates = wsp_slates::slates::create_slates(host.clone());
    (host, slates)
}

const OTHER: &str = "22222222-aaaa-4bbb-8ccc-000000000002";

fn restart_body(depth: usize) {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("c-sqlite-");
        std::fs::create_dir_all(root.path().join("state")).unwrap();
        let other_target = SlateTarget { thread_id: Some(OTHER.into()), turn_token: None };
        {
            let (host, slates) = sqlite_area(root.path());
            host.inner.add_thread(OTHER, OTHER, Some(&host.inner.folder().display().to_string()));
            let text = r#"<slate><value name="v" start={null} /><column><text>{len($v)}</text></column></slate>"#;
            // another thread's slate, which holds nothing deep
            slates
                .write(
                    WriteParams { target: other_target.clone(), text: Some(text.into()), ..Default::default() },
                    Some(&as_thread(OTHER, OTHER)),
                )
                .await
                .expect("a write");
            slates.write(text_write(text), Some(&agent())).await.expect("a write");
            slates.settled().await;
            let mut values = IndexMap::new();
            values.insert("$v".to_string(), lists(depth, json!("x")));
            let _ = slates.state(StateParams { target: target(), values: Some(values), ..Default::default() }, Some(&agent())).await;
            slates.settled().await;
            slates.close();
        }
        let (host, again) = sqlite_area(root.path());
        host.inner.add_thread(OTHER, OTHER, Some(&host.inner.folder().display().to_string()));
        let deep = again.get(THREAD).await;
        let shallow = again.get(OTHER).await;
        assert!(shallow.expect("another thread's slate loads again").is_some());
        assert!(deep.expect("a slate a host stored is one it loads again").is_some());
    }));
}

#[test]
fn a_slate_holding_a_value_200_deep_loads_again_after_a_restart() {
    in_child("a_slate_holding_a_value_200_deep_loads_again_after_a_restart", 200, restart_body);
}

// Ruling 2 of the round 2 brief: a slate value the slates accept always loads after a restart. A value may nest 500 levels
// (SLATE_NEST_LIMIT counts value levels plus path segments), and the store reads a row back to 506.
#[test]
fn a_slate_holding_a_value_499_deep_loads_again_after_a_restart() {
    in_child("a_slate_holding_a_value_499_deep_loads_again_after_a_restart", 499, restart_body);
}

#[test]
fn a_slate_holding_a_value_500_deep_loads_again_after_a_restart() {
    in_child("a_slate_holding_a_value_500_deep_loads_again_after_a_restart", 500, restart_body);
}

fn records(n: usize, leaf: Value) -> Value {
    let mut v = leaf;
    for _ in 0..n {
        v = json!({ "a": v });
    }
    v
}

// After the restart the deep slate is read and written again, not only listed: the record is cloned and walked on a 1 MiB stack.
fn restart_then_use_body(depth: usize) {
    support::local(Box::pin(async move {
        support::own_group();
        for deep in [lists(depth, json!("x")), records(depth, json!("x"))] {
            let root = temp_root("c-sqlite-");
            std::fs::create_dir_all(root.path().join("state")).unwrap();
            {
                let (_host, slates) = sqlite_area(root.path());
                let text = r#"<slate><value name="v" start={null} /><column><text>{len($v)}</text></column></slate>"#;
                slates.write(text_write(text), Some(&agent())).await.expect("a write");
                slates.settled().await;
                let mut values = IndexMap::new();
                values.insert("$v".to_string(), deep.clone());
                slates
                    .state(StateParams { target: target(), values: Some(values), ..Default::default() }, Some(&agent()))
                    .await
                    .expect("a deep value");
                slates.settled().await;
                slates.close();
            }
            let (_host, again) = sqlite_area(root.path());
            let read =
                ReadParams { target: target(), values: Some(vec!["*".into()]), text: Some(true), sketch: Some(true), document: Some(true) };
            let read = again.read(read, Some(&agent())).await.expect("the deep slate is read after a restart");
            let _ = serde_json::to_string(&read.to_json());
            let mut values = IndexMap::new();
            values.insert("$v".to_string(), json!(1));
            again
                .state(StateParams { target: target(), values: Some(values), ..Default::default() }, Some(&agent()))
                .await
                .expect("and written");
            again.settled().await;
            again.close();
            std::mem::forget(deep);
        }
    }));
}

#[test]
fn a_slate_holding_a_value_499_deep_is_read_and_written_after_a_restart() {
    in_child("a_slate_holding_a_value_499_deep_is_read_and_written_after_a_restart", 499, restart_then_use_body);
}

// A start literal of 500 levels sits at the deepest place a record holds one: record, previous, document, values, declaration and
// then the 500 of the literal, 505 levels in the row the store keeps. It loads again after a restart, so does the slate it
// replaced (`previous`), and both are read.
fn restart_start_body(depth: usize) {
    support::local(Box::pin(async move {
        support::own_group();
        let root = temp_root("c-sqlite-");
        std::fs::create_dir_all(root.path().join("state")).unwrap();
        {
            let (_host, slates) = sqlite_area(root.path());
            let deep = format!(
                r#"<slate><value name="v" start={{{}1{}}} /><column><text>{{len($v)}}</text></column></slate>"#,
                "[".repeat(depth),
                "]".repeat(depth)
            );
            slates.write(text_write(&deep), Some(&agent())).await.expect("a deep start value");
            slates.settled().await;
            let small = r#"<slate><value name="v" start={null} /><column><text>{len($v)}</text></column></slate>"#;
            slates.write(text_write(small), Some(&agent())).await.expect("a second write keeps the first as previous");
            slates.settled().await;
            slates.close();
        }
        let (_host, again) = sqlite_area(root.path());
        let read =
            ReadParams { target: target(), values: Some(vec!["*".into()]), text: Some(true), sketch: Some(true), document: Some(true) };
        let read = again.read(read, Some(&agent())).await.expect("the slate is read after a restart");
        let _ = serde_json::to_string(&read.to_json());
    }));
}

#[test]
fn a_slate_with_a_start_literal_500_deep_in_its_previous_loads_again_after_a_restart() {
    in_child("a_slate_with_a_start_literal_500_deep_in_its_previous_loads_again_after_a_restart", 500, restart_start_body);
}
