// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the wsp-slates port for faults that input can reach: a stack overflow, a stall, an unbounded buffer.
//! Each test fails on the code at 975f96f48 and says why; each names the TS line whose behaviour it pins.
//!
//! A stack overflow aborts the whole process, so those tests run their body in a child copy of this test binary
//! (`in_child`) on a thread with an 8 MiB stack, the size of a main thread, and read the child's exit status.
mod support;

use std::rc::Rc;
use std::time::Duration;

use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{as_thread, temp_root, FakeHost, MemStore, THREAD};
use wsp_slates::lazy_slates::lazy_slates;
use wsp_slates::pending::slate::*;
use wsp_slates::slate_files::WithFiles;
use wsp_slates::slate_mcp::{
    create_slate_mcp, fold_schema, McpPipe, McpPipeEvents, McpPipeOpen, McpRunStart, McpSecrets, McpServerSpec, McpTransport, SlateMcpDeps,
    ToolAsks,
};
use wsp_slates::slate_runs::{reshape_answer, DefaultApprovals, ReshapeAnswer, RoadEnd, RunBy, RunRecord};
use wsp_slates::slates::{create_slates, SlateTarget, WriteParams};

const CHILD: &str = "RV_PANICS_CHILD";
const MAIN_THREAD_STACK: usize = 8 * 1024 * 1024;

/// Runs `body` in a child copy of this binary and fails when the child dies, as a stack overflow kills a host.
fn in_child(name: &str, body: fn()) {
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(MAIN_THREAD_STACK).spawn(body).expect("a thread").join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().expect("this binary"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .output()
        .expect("the child starts");
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).rev().take(4).collect();
        panic!("the whole process died ({}) instead of answering: {}", out.status, tail.into_iter().rev().collect::<Vec<_>>().join(" | "));
    }
}

/// Whether `work` ends within `secs`; a thread that overruns is left to the process's exit.
fn finishes_within(secs: u64, work: impl FnOnce() + Send + 'static) -> bool {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .stack_size(MAIN_THREAD_STACK)
        .spawn(move || {
            work();
            let _ = tx.send(());
        })
        .expect("a thread");
    rx.recv_timeout(Duration::from_secs(secs)).is_ok()
}

fn eval_context() -> SlateEvalContext {
    SlateEvalContext { now: 0, item: None, index: None, resolve: Rc::new(|_: &str| None) }
}

fn batch_context() -> SlateBatchContext {
    SlateBatchContext { eval: eval_context(), by: "agent".into(), event: None, reaction_sends: true, react: None, start: None }
}

// ---- a stack overflow on nested input ----

fn deep_start_literal() {
    let n = 60_000;
    let text = format!(
        r#"<slate><value name="x" start={{{}{}}} /><column><text>{{len($x)}}</text></column></slate>"#,
        "[".repeat(n),
        "]".repeat(n)
    );
    let r = parse_slate(&text);
    assert!(r.document.is_none(), "a literal that deep is not a document");
    assert!(r.errors.iter().any(|e| e.code == "S503"), "{:?}", r.errors);
}

// syntax.ts:211 `value` recurses once per [ and is not capped; syntax.ts:747-750 turns node's stack overflow into S503 for
// the agent (node answers S503 at 8,000 or more brackets). Rust: syntax/literal.rs `Literal::value` -> `list`, no depth cap;
// a 120 KB text (under the 128 KiB cap) of brackets aborts the host.
#[test]
fn a_start_literal_nested_60000_deep_is_refused_with_s503_not_a_stack_overflow() {
    in_child("a_start_literal_nested_60000_deep_is_refused_with_s503_not_a_stack_overflow", deep_start_literal);
}

fn deep_path_write() {
    let doc = parse_slate(r#"<slate><value name="x" start={null} /><column><text>{len($x)}</text></column></slate>"#)
        .document
        .expect("a document");
    let values = slate_start_values(&doc);
    let path = format!("$x{}", "[0]".repeat(20_000));
    let out = run_slate_batch(&doc, &values, vec![SlateWrite { path, value: json!(1) }], batch_context());
    assert!(!out.changed.contains(&"$x".to_string()), "TS stores nothing: {:?}", out.changed);
    std::mem::forget(out);
}

// paths.ts:63 `write` recurses once per segment and throws RangeError past about 4,700 segments, and batch.ts:127
// JSON.stringify(next) throws it past about 3,600; either way the batch throws and the agent's slate_state call fails with
// nothing stored. Rust: paths.rs `write_at` has no depth cap; a path of 20,000 `[0]` segments (60 KB in one slate_state
// `values` key) aborts the host.
#[test]
fn a_write_path_20000_segments_deep_is_refused_not_a_stack_overflow() {
    in_child("a_write_path_20000_segments_deep_is_refused_not_a_stack_overflow", deep_path_write);
}

fn nesting_presses() {
    let text = format!(
        r#"<slate><value name="x" start={{null}} /><column><button id="b" label="Nest" onPress={{set($x, {}$x{})}} /><text>{{len($x)}}</text></column></slate>"#,
        "[".repeat(14),
        "]".repeat(14)
    );
    let parsed = parse_slate(&text);
    let doc = parsed.document.expect("a document");
    let mut values = slate_start_values(&doc);
    for _ in 0..1_500 {
        let mut ctx = batch_context();
        ctx.by = "person".into();
        ctx.event = Some(BatchEvent { piece: "b".into(), kind: "press".into(), item: None, index: None, row_action: None });
        let out = run_slate_batch(&doc, &values, vec![], ctx);
        values = out.values.clone();
        std::mem::forget(out);
    }
    std::mem::forget(values);
}

// batch.ts:127 `set` writes the value, then JSON.stringify(next) throws RangeError once the value is about 3,600 deep (node:
// press 259 of this button, 14 deep a press, throws; press 3,639 of one doing set($x, [$x]) does; the press fails and the
// value stays), so no stored value passes that. Rust: batch.rs `set` has no depth cap, so the same button pressed 1,500 times
// (21,000 deep) aborts the host past about 6,000 deep, in `serde_json::to_value` or the stringify it feeds. Each press is
// one the person or an agent's slate_event makes; none is refused.
#[test]
fn a_button_that_nests_its_value_on_every_press_is_refused_not_a_stack_overflow() {
    in_child("a_button_that_nests_its_value_on_every_press_is_refused_not_a_stack_overflow", nesting_presses);
}

fn recursive_schema() {
    let schema = json!({
        "type": "object",
        "properties": { "value": { "$ref": "#/$defs/Json" } },
        "$defs": { "Json": { "anyOf": [{ "type": "string" }, { "type": "array", "items": { "$ref": "#/$defs/Json" } }] } },
    });
    let _ = fold_schema(&schema);
}

// slate-mcp.ts:514 `typeOf` recurses through items and anyOf with no bound on a $ref cycle, so node throws RangeError and
// slate_catalog answers an error for that server; foldSchema is the same call at slate-mcp.ts:530. Rust: client.rs `type_of`
// has no bound either, and a tool's inputSchema from any MCP server (a recursive JSON-value type, as pydantic writes one)
// aborts the host on `slate_catalog <server>`.
#[test]
fn a_recursive_tool_schema_does_not_take_the_process_down() {
    in_child("a_recursive_tool_schema_does_not_take_the_process_down", recursive_schema);
}

fn long_derived_chain() {
    let n = 2_500;
    let body: String = (0..n).map(|i| format!("<derived name=\"d{i}\" value={{$d{}}} />", i + 1)).collect();
    let text = format!("<slate><value name=\"v\" start={{1}} />{body}<column><text>x</text></column></slate>");
    let r = parse_slate(&text);
    assert!(r.document.is_none(), "2,500 derived values are over the limit of 32");
}

// validate.ts:226 `derivedType` and :279 `derivedCycles` recurse once per derived value that reads the next; with 1,000 or
// more in a chain (a 37 KB text) node throws RangeError out of parseSlate and slate_write answers an error. Rust: validate/
// driver.rs `derived_visit` recurses the same way with a 5 KB frame in a debug build, and 1,500 derived values (55 KB of
// text, under the 64 KB document cap) abort the process; the limit error S512 is added but the walk runs first.
#[test]
fn a_chain_of_2500_derived_values_is_answered_not_a_stack_overflow() {
    in_child("a_chain_of_2500_derived_values_is_answered_not_a_stack_overflow", long_derived_chain);
}

// ---- a stall: work that grows with the square of the input ----

// syntax.ts:167 `balanced` finds a brace's end with closingBrace(src, i + 1), which reads only as far as the brace; node
// parses 5,000 holes (15 KB) in 8 ms and 20,000 (60 KB) in 57 ms. Rust: syntax/reader.rs:293 `balanced` encodes the whole text
// to UTF-16, counts the prefix and walks to the byte offset for every brace, so the time grows with braces x text: 5,000
// holes take 9.7 s and 20,000 take 117 s in a debug build; in a release build 43,000 (129 KB, under the 128 KiB text cap)
// take 16.3 s where node takes 1.5 s, while the host's one thread does nothing else.
#[test]
fn a_text_of_20000_braces_parses_in_linear_time() {
    let text = format!("<slate><column><text>{}</text></column></slate>", "{1}".repeat(20_000));
    let done = finishes_within(2, move || {
        let c = compile_slate_text(&text);
        assert!(c.document.is_some());
    });
    assert!(done, "20,000 holes were still being read after 2 s (node: 57 ms)");
}

// syntax.ts:244 reads each scalar with /^(...)/.exec(text.slice(i)), a slice that costs nothing. Rust: syntax/literal.rs:153
// `scalar` copies the rest of the text (`String::from_utf16_lossy(&self.t[self.i..])`) for every number, true, false or
// null, so a list of n numbers costs n x the text: 8,000 numbers (16 KB) take about 5 s and 32,000 (64 KB) take 84 s in a
// debug build, and 63,000 (126 KB) take 7.8 s in a release build; node reads 63,000 in 22 ms.
#[test]
fn a_start_list_of_63000_numbers_parses_in_linear_time() {
    let body = "1,".repeat(63_000);
    let text = format!(r#"<slate><value name="x" start={{[{body}]}} /><column><text>{{len($x)}}</text></column></slate>"#);
    let done = finishes_within(2, move || {
        let r = parse_slate(&text);
        let said: Vec<(&str, &str)> = r.errors.iter().map(|e| (e.code.as_str(), e.message.as_str())).collect();
        assert_eq!(said, [("D208", "the document is over 64 KB")], "node's answer for this text");
    });
    assert!(done, "a list of 63,000 numbers was still being read after 2 s (node: 22 ms)");
}

// syntax.ts:176-178 dedent shifts the leading blank lines off an array, which node does in constant time per shift. Rust:
// syntax/text.rs:9 `lines.remove(0)` moves every later line each time, so a raw block (<diagram>, or a <file>) of 120,000
// blank lines takes 2.3 s in a debug build; node reads the same text in a few milliseconds. Smaller of the stalls here.
#[test]
fn a_raw_block_of_120000_blank_lines_parses_in_linear_time() {
    let text = format!("<slate><column><diagram>{}x</diagram></column></slate>", "\n".repeat(120_000));
    let done = finishes_within(1, move || {
        let c = compile_slate_text(&text);
        assert!(c.document.is_some());
    });
    assert!(done, "120,000 blank lines were still being read after 1 s");
}

// ---- a wrong answer ----

// slate-runs.ts:290 and :784 read a command's output with JSON.parse, which node runs without a depth limit (a 100,000-deep
// array parses), so a `then` or a run that prints JSON nested 200 deep gives its `json`. Rust: road.rs:139 and table.rs:264
// read it with serde_json::from_str, which stops at 128 deep, so the run has no `json` and a `then` fails with
// "then printed no JSON: [[[[...".
#[test]
fn json_a_command_prints_nested_200_deep_is_kept() {
    let out = format!("{}1{}", "[".repeat(200), "]".repeat(200));
    let end = RoadEnd { code: Some(0), out, ..RoadEnd::default() };
    let answer = reshape_answer(&end, 60.0, &|text: &str| text.to_string());
    let kept = matches!(&answer, ReshapeAnswer::Json(Value::Array(_)));
    if !kept {
        let shown = format!("{answer:?}");
        panic!("the json was dropped: {}", shown.chars().take(120).collect::<String>());
    }
    std::mem::forget(answer);
}

// ---- a panic inside a task the host spawned ----

struct ServerPipe {
    events: McpPipeEvents,
}

impl McpPipe for ServerPipe {
    fn write(&self, text: &str) {
        for line in text.split('\n').filter(|l| !l.is_empty()) {
            let msg: Value = serde_json::from_str(line).expect("the client writes JSON");
            let Some(id) = msg.get("id").cloned() else { continue };
            let result = match msg["method"].as_str().unwrap_or("") {
                "initialize" => json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} } }),
                "tools/list" => {
                    json!({ "tools": [{ "name": "echo", "inputSchema": { "type": "object" }, "annotations": { "readOnlyHint": true } }] })
                }
                _ => json!({ "content": [{ "type": "text", "text": "hi" }] }),
            };
            let reply = json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string();
            let out = self.events.out.clone();
            tokio::task::spawn_local(async move {
                tokio::task::yield_now().await;
                out(format!("{reply}\n").as_bytes());
            });
        }
    }
    fn kill(&self) {}
    fn pid(&self) -> Option<u32> {
        None
    }
}

struct NoSecrets;

impl McpSecrets for NoSecrets {
    fn plaintext(&self, _thread: &str, _name: &str) -> Option<String> {
        None
    }
    fn scrub(&self, _thread: &str, text: &str) -> String {
        text.to_string()
    }
}

// slate-mcp.ts:819-820 calls the server with `toolTimeoutSec * 1000` as a JavaScript number; a huge one reaches setTimeout,
// which node holds to 1 ms, and the run ends done or failed. Rust: runs.rs:695 `tool_timeout_sec.map(|s| s * 1000)` is a u64
// multiply in a task the host spawned: a config value above 18,446,744,073,709,551 seconds panics there in a debug build
// (a release build wraps), the task dies without a word, and the run reads `running` for ever.
#[test]
fn a_huge_tool_timeout_in_the_servers_config_does_not_leave_the_run_running_for_ever() {
    support::local(async {
        let open: McpPipeOpen =
            Rc::new(|_t: &McpTransport, _s: &McpServerSpec, events: McpPipeEvents| Rc::new(ServerPipe { events }) as Rc<dyn McpPipe>);
        let transport =
            McpTransport::Stdio { command: "x".into(), args: vec![], env: IndexMap::new(), cwd: None, tool_timeout_sec: Some(u64::MAX) };
        let spec = McpServerSpec {
            transport,
            cwd: "/".into(),
            env: IndexMap::new(),
            secrets: vec![],
            open: Some(open),
            post: None,
            sign_in: None,
        };
        let records: Rc<std::cell::RefCell<Vec<Value>>> = Rc::default();
        let sink = records.clone();
        let deps = SlateMcpDeps {
            server: Rc::new(move |_t: &str, _n: &str| {
                let spec = spec.clone();
                async move { Ok(spec) }.boxed_local()
            }),
            on_record: Rc::new(move |_t: &str, _run: &str, r: &RunRecord| {
                sink.borrow_mut().push(serde_json::to_value(r).expect("a record"))
            }),
            on_asks: None,
            asks: Rc::new(|_t: &str| ToolAsks::Never),
            approvals: Rc::new(DefaultApprovals::default()),
            secrets: Rc::new(NoSecrets),
            reshape: Rc::new(|_t: &str, _c: &str, _i: &str| panic!("no run here reshapes")),
            computer: Rc::new(|_t: &str| "this computer".to_string()),
            own: None,
            clock: wsp_core::clock::real_clock(),
            idle_ms: None,
            call_ms: None,
        };
        let mcp = create_slate_mcp(deps);
        let decl = serde_json::from_value::<SlateRunDecl>(json!({ "kind": "tool", "server": "s", "tool": "echo" })).expect("a declaration");
        mcp.start(McpRunStart {
            thread_id: "t".into(),
            run: "r".into(),
            decl: WithFiles { decl, files: None },
            by: RunBy::Person,
            args: Rc::new(serde_json::Map::new),
            last: None,
        });
        let mut state = String::new();
        for _ in 0..120 {
            state = records.borrow().last().and_then(|r| r["state"].as_str()).unwrap_or("").to_string();
            if state == "done" || state == "failed" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(state == "done" || state == "failed", "the run still reads {state:?} after 3 s");
        mcp.close();
    });
}

fn first_call_with_stdout_closed() {
    // SAFETY: plain fd calls on this process's own stdout; the original is put back before the body ends.
    let saved = unsafe { libc::dup(1) };
    let mut fds = [0i32; 2];
    unsafe {
        libc::pipe(fds.as_mut_ptr());
        libc::close(fds[0]);
        libc::dup2(fds[1], 1);
        libc::close(fds[1]);
    }
    let wrote = std::cell::Cell::new(false);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        support::local(async {
            let root = temp_root("closed-stdout");
            let host = FakeHost::new(root.path(), MemStore::new());
            let area = lazy_slates(host, Box::new(create_slates));
            let target = SlateTarget { thread_id: Some(THREAD.into()), turn_token: None };
            let text = r#"<slate><column><text>hi</text></column></slate>"#;
            let params = WriteParams { target, text: Some(text.into()), ..Default::default() };
            wrote.set(area.write(params, Some(&as_thread(THREAD, THREAD))).await.is_ok());
        })
    }));
    unsafe {
        libc::dup2(saved, 1);
        libc::close(saved);
    }
    assert!(outcome.is_ok(), "the first slate call panicked because the host's stdout is a closed pipe");
    assert!(wrote.get(), "the first write was refused");
}

// lazy-slates.ts logs the load with console.log, which never throws; wsp-core's console::warn is written to drop a closed
// stderr for the same reason. Rust: lazy_slates.rs:70 `println!` panics on a broken pipe, in the task that makes the area, so
// the first slate call of a host whose stdout is a pipe nobody reads (`wsp host | head`) panics (the area is already stored,
// so the next call works). A nit: one failed call, on a host started with its stdout closed.
#[test]
fn the_load_line_does_not_take_the_first_slate_call_down_when_stdout_is_a_closed_pipe() {
    in_child("the_load_line_does_not_take_the_first_slate_call_down_when_stdout_is_a_closed_pipe", first_call_with_stdout_closed);
}
