// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of spec/slate_mcp_runs.md: `createSlateMcp`, slate-mcp.ts:558-1045 at 65f6b89c7. The TS cases that call
//! `createSlateMcp` directly are ported whole; what the runtime-wide cases assert of this module is ported against
//! the TS stub servers (run with node) or a server in the test's hands. Records are compared as JSON, since the
//! record type is slates-a's.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::task::LocalSet;

use wsp_core::clock::{real_clock, Clock, FakeClock};
use wsp_slates::pending::slate::{SlateDoc, SlateRunDecl};
use wsp_slates::slate_files::WithFiles;
use wsp_slates::slate_mcp::*;
use wsp_slates::slate_runs::{
    Outcome, Reshape, ReshapeAnswer, RunApprovals, RunBy, RunRecord, RunStartAnswer, HELD_PRESSED, STARTS_PER_MINUTE,
};

// ---- fixtures and fakes ----

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name).to_string_lossy().to_string()
}

fn path_env() -> IndexMap<String, String> {
    let mut env = IndexMap::new();
    env.insert("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
    env
}

/// A TS stub server as the thread's agent would start it, counting its starts in `starts` when given.
fn node_spec(script: &str, starts: Option<&PathBuf>, secrets: Vec<String>) -> McpServerSpec {
    let mut tenv = IndexMap::new();
    if let Some(at) = starts {
        tenv.insert("STUB_STARTS".to_string(), at.to_string_lossy().to_string());
    }
    let t = McpTransport::Stdio { command: "node".into(), args: vec![fixture(script)], env: tenv, cwd: None, tool_timeout_sec: None };
    McpServerSpec {
        transport: t,
        cwd: std::env::temp_dir().to_string_lossy().to_string(),
        env: path_env(),
        secrets,
        open: None,
        post: None,
        sign_in: None,
    }
}

fn decl(v: Value) -> WithFiles {
    WithFiles { decl: serde_json::from_value::<SlateRunDecl>(v).expect("a declaration"), files: None }
}

fn tool(server: &str, name: &str) -> WithFiles {
    decl(json!({ "kind": "tool", "server": server, "tool": name }))
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().expect("an object").clone()
}

fn doc(runs: Value) -> SlateDoc {
    serde_json::from_value(
        json!({ "schema": 2, "root": "root", "values": {}, "derived": {}, "reactions": [], "pieces": {}, "runs": runs, "files": {} }),
    )
    .expect("a document")
}

fn json_of(r: &RunRecord) -> Value {
    serde_json::to_value(r).expect("a record")
}

/// A start's answer as the TS writes it: `{outcome, record}`.
fn ans(a: &RunStartAnswer) -> Value {
    let outcome = match a.outcome {
        Outcome::Running => "running",
        Outcome::Held => "held",
        Outcome::Noop => "noop",
        Outcome::Failed => "failed",
    };
    json!({ "outcome": outcome, "record": json_of(&a.record) })
}

/// The held asks as the wire carries them.
fn asks_json(held: &[wsp_slates::pending::wire::SlateAsk]) -> Value {
    Value::Array(held.iter().map(|a| a.to_json()).collect())
}

#[derive(Default)]
struct Approvals(RefCell<HashSet<(String, String)>>);

impl RunApprovals for Approvals {
    fn has(&self, thread_id: &str, key: &str) -> bool {
        self.0.borrow().contains(&(thread_id.to_string(), key.to_string()))
    }
    fn allow(&self, thread_id: &str, key: &str) {
        self.0.borrow_mut().insert((thread_id.to_string(), key.to_string()));
    }
    fn revoke(&self, thread_id: &str, key: &str) {
        self.0.borrow_mut().remove(&(thread_id.to_string(), key.to_string()));
    }
    fn list(&self, thread_id: &str) -> Vec<String> {
        self.0.borrow().iter().filter(|(t, _)| t == thread_id).map(|(_, k)| k.clone()).collect()
    }
}

/// Secrets by name; the scrub puts `[secret:<name>]` where a plaintext stood.
#[derive(Default)]
struct Secrets(HashMap<String, String>);

impl McpSecrets for Secrets {
    fn plaintext(&self, _thread: &str, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
    fn scrub(&self, _thread: &str, text: &str) -> String {
        let mut out = text.to_string();
        for (name, value) in &self.0 {
            out = out.replace(value.as_str(), &format!("[secret:{name}]"));
        }
        out
    }
}

/// Every record written, in order.
#[derive(Default, Clone)]
struct Records(Rc<RefCell<Vec<(String, String, Value)>>>);

impl Records {
    fn last(&self, run: &str) -> Option<Value> {
        self.0.borrow().iter().rev().find(|(_, r, _)| r == run).map(|(_, _, v)| v.clone())
    }
    fn all(&self, run: &str) -> Vec<Value> {
        self.0.borrow().iter().filter(|(_, r, _)| r == run).map(|(_, _, v)| v.clone()).collect()
    }
}

struct Made {
    mcp: Rc<SlateMcp>,
    records: Records,
    approvals: Rc<Approvals>,
    asks_called: Rc<Cell<u32>>,
    servers_asked: Rc<Cell<u32>>,
}

struct Opts {
    asks: Rc<Cell<ToolAsks>>,
    secrets: Secrets,
    clock: Rc<dyn Clock>,
    idle_ms: Option<u64>,
    own: Option<Rc<dyn Fn(&str, u32, &str) -> Release>>,
    reshape: Option<Rc<dyn Fn(&str, &str, &str) -> Reshape>>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            asks: Rc::new(Cell::new(ToolAsks::Never)),
            secrets: Secrets::default(),
            clock: real_clock(),
            idle_ms: None,
            own: None,
            reshape: None,
        }
    }
}

fn make(server: Rc<dyn Fn(&str, &str) -> Result<McpServerSpec, String>>, o: Opts) -> Made {
    let records = Records::default();
    let approvals = Rc::new(Approvals::default());
    let asks_called = Rc::new(Cell::new(0));
    let servers_asked = Rc::new(Cell::new(0));
    let rec = records.clone();
    let called = asks_called.clone();
    let asked = servers_asked.clone();
    let asks = o.asks.clone();
    let deps = SlateMcpDeps {
        server: Rc::new(move |t: &str, n: &str| {
            asked.set(asked.get() + 1);
            let got = server(t, n);
            async move { got }.boxed_local()
        }),
        on_record: Rc::new(move |t: &str, run: &str, r: &RunRecord| rec.0.borrow_mut().push((t.to_string(), run.to_string(), json_of(r)))),
        on_asks: Some(Rc::new(move |_t: &str| called.set(called.get() + 1))),
        asks: Rc::new(move |_t: &str| asks.get()),
        approvals: approvals.clone(),
        secrets: Rc::new(o.secrets),
        reshape: o.reshape.unwrap_or_else(|| Rc::new(|_t: &str, _c: &str, _i: &str| -> Reshape { panic!("no run here reshapes") })),
        computer: Rc::new(|_t: &str| "this computer".to_string()),
        own: o.own,
        clock: o.clock,
        idle_ms: o.idle_ms,
        call_ms: None,
    };
    Made { mcp: create_slate_mcp(deps), records, approvals, asks_called, servers_asked }
}

fn start(m: &Made, thread: &str, run: &str, d: WithFiles, by: RunBy, args: Value) -> RunStartAnswer {
    let args = obj(args);
    m.mcp.start(McpRunStart { thread_id: thread.into(), run: run.into(), decl: d, by, args: Rc::new(move || args.clone()), last: None })
}

async fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..400 {
        if ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("timed out waiting for {what}");
}

fn state_of(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|v| v.get("state")).and_then(Value::as_str).map(String::from)
}

fn spawned(at: &PathBuf) -> usize {
    std::fs::read_to_string(at).map(|s| s.trim().lines().count()).unwrap_or(0)
}

/// A server in the test's hands over a fake pipe: it answers initialize and tools/list at once, and every call by
/// `answer` (`None` holds it until `release`). Answers go out on a later turn of the loop, as the TS's setImmediate.
struct Hand {
    tools: Value,
    answer: RefCell<Box<dyn FnMut(&str, &Value) -> Option<Value>>>,
    lines: RefCell<Vec<Value>>,
    held: RefCell<Vec<Value>>,
    events: RefCell<Option<McpPipeEvents>>,
    kills: Cell<u32>,
    opens: Cell<u32>,
}

struct HandPipe(Rc<Hand>);

impl McpPipe for HandPipe {
    fn write(&self, text: &str) {
        for line in text.split('\n').filter(|l| !l.is_empty()) {
            let msg: Value = serde_json::from_str(line).expect("the client writes JSON");
            self.0.lines.borrow_mut().push(msg.clone());
            let Some(id) = msg.get("id").cloned() else { continue };
            let method = msg["method"].as_str().unwrap_or("").to_string();
            let result = match method.as_str() {
                "initialize" => Some(json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} } })),
                "tools/list" => Some(json!({ "tools": self.0.tools })),
                _ => (self.0.answer.borrow_mut())(&method, &msg["params"]),
            };
            match result {
                Some(r) => self.0.send(json!({ "jsonrpc": "2.0", "id": id, "result": r })),
                None => self.0.held.borrow_mut().push(id),
            }
        }
    }
    fn kill(&self) {
        self.0.kills.set(self.0.kills.get() + 1);
    }
    fn pid(&self) -> Option<u32> {
        None
    }
}

impl Hand {
    fn new(tools: Value, answer: Box<dyn FnMut(&str, &Value) -> Option<Value>>) -> Rc<Hand> {
        Rc::new(Hand {
            tools,
            answer: RefCell::new(answer),
            lines: RefCell::new(vec![]),
            held: RefCell::new(vec![]),
            events: RefCell::new(None),
            kills: Cell::new(0),
            opens: Cell::new(0),
        })
    }
    fn send(self: &Rc<Self>, msg: Value) {
        let me = self.clone();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            let events = me.events.borrow().clone();
            if let Some(e) = events {
                (e.out)(format!("{msg}\n").as_bytes());
            }
        });
    }
    /// Answers the oldest held call.
    fn release(self: &Rc<Self>, result: Value) {
        let id = self.held.borrow_mut().remove(0);
        self.send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }
    fn spec(self: &Rc<Self>, secrets: Vec<String>) -> McpServerSpec {
        let me = self.clone();
        let open: McpPipeOpen = Rc::new(move |_t: &McpTransport, _s: &McpServerSpec, on: McpPipeEvents| {
            me.opens.set(me.opens.get() + 1);
            *me.events.borrow_mut() = Some(on);
            Rc::new(HandPipe(me.clone())) as Rc<dyn McpPipe>
        });
        let t = McpTransport::Stdio { command: "hand".into(), args: vec![], env: IndexMap::new(), cwd: None, tool_timeout_sec: None };
        McpServerSpec { transport: t, cwd: "/".into(), env: IndexMap::new(), secrets, open: Some(open), post: None, sign_in: None }
    }
    fn notified(&self, method: &str) -> Vec<Value> {
        self.lines.borrow().iter().filter(|l| l["method"] == method && l.get("id").is_none()).map(|l| l["params"].clone()).collect()
    }
    fn calls(&self) -> Vec<Value> {
        self.lines.borrow().iter().filter(|l| l["method"] == "tools/call").cloned().collect()
    }
}

fn text_result(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn served(hand: &Rc<Hand>, secrets: Vec<String>) -> Rc<dyn Fn(&str, &str) -> Result<McpServerSpec, String>> {
    let spec = hand.spec(secrets);
    Rc::new(move |_t: &str, _n: &str| Ok(spec.clone()))
}

fn stub_server(starts: Option<PathBuf>) -> Rc<dyn Fn(&str, &str) -> Result<McpServerSpec, String>> {
    Rc::new(move |_t: &str, name: &str| match name {
        "stub" => Ok(node_spec("stub-mcp.mjs", starts.as_ref(), vec![])),
        "notes" => Ok(node_spec("stub-text-mcp.mjs", starts.as_ref(), vec![])),
        "flood" => Ok(node_spec("stub-flood-mcp.mjs", None, vec![])),
        other => Err(format!("no MCP server called {other} is in Claude Code's config there")),
    })
}

// ---- the connection's life ----

#[tokio::test(flavor = "current_thread")]
async fn closes_a_servers_connection_once_hidden_and_idle_unless_an_always_timer_keeps_it() {
    // TS slate-mcp.test.ts:135-173.
    LocalSet::new()
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let starts = dir.path().join("starts");
            let m = make(
                stub_server(Some(starts.clone())),
                Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), idle_ms: Some(300), ..Opts::default() },
            );
            m.approvals.allow("t", &code_key(&tool("stub", "list_items"), &obj(json!({ "params": { "account": "a", "limit": 1 } }))));
            let call = |n: u64| {
                let records = m.records.clone();
                start(&m, "t", "inbox", tool("stub", "list_items"), RunBy::Person, json!({ "params": { "account": "a", "limit": 1 } }));
                async move {
                    wait_for("the call", || records.last("inbox").map(|r| r["state"] == "done" && r["runs"] == n).unwrap_or(false)).await;
                }
            };
            m.mcp.servers("t", &["stub".into()], &[]);
            m.mcp.shown("t", true);
            call(1).await;
            tokio::time::sleep(Duration::from_millis(700)).await;
            call(2).await;
            assert_eq!(spawned(&starts), 1);
            m.mcp.shown("t", false);
            tokio::time::sleep(Duration::from_millis(900)).await;
            call(3).await;
            assert_eq!(spawned(&starts), 2);
            m.mcp.servers("t", &["stub".into()], &["stub".into()]);
            m.mcp.shown("t", false);
            tokio::time::sleep(Duration::from_millis(900)).await;
            call(4).await;
            assert_eq!(spawned(&starts), 2);
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn closes_a_server_the_document_no_longer_names() {
    // spec rule 20.
    LocalSet::new()
        .run_until(async {
            let hand =
                Hand::new(json!([{ "name": "list", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| Some(text_result("{}"))));
            let m = make(served(&hand, vec![]), Opts::default());
            m.mcp.servers("t", &["hand".into()], &[]);
            start(&m, "t", "a", tool("hand", "list"), RunBy::Agent, json!({}));
            wait_for("done", || state_of(&m.records.last("a")).as_deref() == Some("done")).await;
            assert_eq!((hand.opens.get(), hand.kills.get()), (1, 0));
            m.mcp.servers("t", &[], &[]);
            wait_for("the close", || hand.kills.get() == 1).await;
            start(&m, "t", "a", tool("hand", "list"), RunBy::Agent, json!({}));
            wait_for("done again", || m.records.last("a").map(|r| r["runs"] == 2 && r["state"] == "done").unwrap_or(false)).await;
            assert_eq!(hand.opens.get(), 2);
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn names_a_stdio_servers_process_as_the_threads_tool_server_until_closed() {
    // TS slate-mcp.test.ts:175-204.
    LocalSet::new()
        .run_until(async {
            let owned: Rc<RefCell<Vec<(String, u32, String, Rc<Cell<bool>>)>>> = Rc::default();
            let o = owned.clone();
            let own: Rc<dyn Fn(&str, u32, &str) -> Release> = Rc::new(move |t: &str, pid: u32, kind: &str| {
                let released = Rc::new(Cell::new(false));
                o.borrow_mut().push((t.to_string(), pid, kind.to_string(), released.clone()));
                Box::new(move || released.set(true)) as Release
            });
            let dir = tempfile::tempdir().unwrap();
            let m = make(stub_server(Some(dir.path().join("starts"))), Opts { idle_ms: Some(200), own: Some(own), ..Opts::default() });
            m.mcp.servers("t", &["stub".into()], &[]);
            start(&m, "t", "inbox", tool("stub", "list_items"), RunBy::Person, json!({ "params": { "account": "a", "limit": 1 } }));
            wait_for("done", || state_of(&m.records.last("inbox")).as_deref() == Some("done")).await;
            let rows: Vec<(String, String, bool)> = owned.borrow().iter().map(|(t, _, k, r)| (t.clone(), k.clone(), r.get())).collect();
            assert_eq!(rows, vec![("t".to_string(), "tool".to_string(), false)]);
            let pid = owned.borrow()[0].1;
            assert_eq!(unsafe { libc::kill(pid as i32, 0) }, 0, "the server runs");
            let released = owned.borrow()[0].3.clone();
            wait_for("the release", || released.get()).await;
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_failed_start_says_what_the_server_said_and_connects_again_next_time() {
    // spec rule 3: the server's last stderr line joins the why; a failed connect is forgotten; a server the config
    // does not name fails with its own words.
    LocalSet::new()
        .run_until(async {
            let server: Rc<dyn Fn(&str, &str) -> Result<McpServerSpec, String>> = Rc::new(|_t: &str, name: &str| {
                if name != "broken" {
                    return Err(format!("no MCP server called {name} is in Claude Code's config there"));
                }
                let t = McpTransport::Stdio {
                    command: "bash".into(),
                    args: vec!["-c".into(), "echo first >&2; echo 'cannot find module x' >&2; sleep 0.3; exit 1".into()],
                    env: IndexMap::new(),
                    cwd: None,
                    tool_timeout_sec: None,
                };
                Ok(McpServerSpec { transport: t, cwd: "/".into(), env: path_env(), secrets: vec![], open: None, post: None, sign_in: None })
            });
            let m = make(server, Opts::default());
            start(&m, "t", "b", tool("broken", "x"), RunBy::Agent, json!({}));
            wait_for("failed", || state_of(&m.records.last("b")).as_deref() == Some("failed")).await;
            assert_eq!(
                m.records.last("b").unwrap()["why"],
                "broken did not start: the server exited with 1; it said: cannot find module x"
            );
            start(&m, "t", "b", tool("broken", "x"), RunBy::Agent, json!({}));
            wait_for("failed twice", || m.records.last("b").map(|r| r["runs"] == 2 && r["state"] == "failed").unwrap_or(false)).await;
            assert_eq!(m.servers_asked.get(), 2);
            start(&m, "t", "z", tool("zoho", "x"), RunBy::Agent, json!({}));
            wait_for("refused", || state_of(&m.records.last("z")).as_deref() == Some("failed")).await;
            let z = m.records.last("z").unwrap();
            assert_eq!(z["why"], "no MCP server called zoho is in Claude Code's config there");
            assert_eq!((z["exit"].clone(), z["runs"].clone()), (Value::Null, json!(1)));
            m.mcp.close();
        })
        .await;
}

// ---- caps through the runs ----

fn big_http() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            while !buf.ends_with(b"\r\n\r\n") {
                if s.read(&mut byte).unwrap_or(0) == 0 {
                    break;
                }
                buf.push(byte[0]);
            }
            let head = String::from_utf8_lossy(&buf).to_lowercase();
            let len =
                head.lines().find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            let mut body = vec![0u8; len];
            let _ = s.read_exact(&mut body);
            let msg: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let reply = match (msg.get("id"), msg["method"].as_str()) {
                (None, _) => "HTTP/1.1 202 X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string(),
                (Some(id), Some("initialize")) => ok(json!({ "jsonrpc": "2.0", "id": id, "result": { "protocolVersion": "2025-06-18", "capabilities": { "tools": {} }, "serverInfo": { "name": "big", "version": "1" } } }).to_string()),
                (Some(id), Some("tools/list")) => ok(json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": [{ "name": "huge", "annotations": { "readOnlyHint": true }, "inputSchema": { "type": "object" } }] } }).to_string()),
                (Some(id), _) => ok(format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}}}", "x".repeat(3 * 1024 * 1024))),
            };
            let _ = s.write_all(reply.as_bytes());
        }
    });
    fn ok(body: String) -> String {
        format!("HTTP/1.1 200 X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
    }
    format!("http://127.0.0.1:{port}/mcp")
}

#[tokio::test(flavor = "current_thread")]
async fn cuts_a_server_off_past_2_mb_and_fails_too_deep_or_too_wide_answers() {
    // TS slate-mcp.test.ts:206-254.
    LocalSet::new()
        .run_until(async {
            let url = big_http();
            let flood = stub_server(None);
            let server: Rc<dyn Fn(&str, &str) -> Result<McpServerSpec, String>> = Rc::new(move |t: &str, name: &str| {
                if name == "big" {
                    let tr = McpTransport::Http { url: url.clone(), headers: IndexMap::new() };
                    return Ok(McpServerSpec {
                        transport: tr,
                        cwd: "/".into(),
                        env: IndexMap::new(),
                        secrets: vec![],
                        open: None,
                        post: None,
                        sign_in: None,
                    });
                }
                flood(t, name)
            });
            let m = make(server, Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), ..Opts::default() });
            for (server, name) in [("flood", "deep"), ("flood", "wide"), ("flood", "deeptext"), ("flood", "endless"), ("big", "huge")] {
                m.approvals.allow("t", &code_key(&tool(server, name), &Map::new()));
            }
            m.mcp.servers("t", &["flood".into(), "big".into()], &[]);
            let ended = |run: &'static str, server: &'static str, name: &'static str| {
                start(&m, "t", run, tool(server, name), RunBy::Person, json!({}));
                let records = m.records.clone();
                async move {
                    wait_for(run, || matches!(state_of(&records.last(run)).as_deref(), Some("done") | Some("failed"))).await;
                    records.last(run).unwrap()
                }
            };
            let deep = ended("deep", "flood", "deep").await;
            assert_eq!(deep["state"], "failed");
            assert!(deep["why"].as_str().unwrap().contains("nested over 64 deep"), "{deep}");
            let wide = ended("wide", "flood", "wide").await;
            assert!(wide["why"].as_str().unwrap().contains("over 100000 entries"), "{wide}");
            let text = ended("deeptext", "flood", "deeptext").await;
            assert_eq!((text["state"].clone(), text["text"].clone()), (json!("done"), json!(true)));
            assert!(text.get("json").is_none());
            let endless = ended("endless", "flood", "endless").await;
            assert_eq!(endless["state"], "failed");
            assert!(endless["why"].as_str().unwrap().contains("over 2 MB"), "{endless}");
            let huge = ended("huge", "big", "huge").await;
            assert_eq!(huge["state"], "failed");
            assert!(huge["why"].as_str().unwrap().contains("over 2 MB"), "{huge}");
            m.mcp.close();
        })
        .await;
}

// ---- asking ----

#[tokio::test(flavor = "current_thread")]
async fn skips_the_ask_for_a_read_only_tool_and_asks_every_start_of_a_confirm_run() {
    // TS slate-mcp.test.ts:481-518.
    LocalSet::new()
        .run_until(async {
            let asks = Rc::new(Cell::new(ToolAsks::UnlessReadOnly));
            let m = make(stub_server(None), Opts { asks: asks.clone(), ..Opts::default() });
            start(&m, "t", "list", tool("stub", "list_items"), RunBy::Person, json!({ "params": { "account": "a", "limit": 1 } }));
            wait_for("list done", || state_of(&m.records.last("list")).as_deref() == Some("done")).await;
            // A tool with no hints is one that is not read-only.
            start(
                &m,
                "t",
                "touch",
                decl(json!({ "kind": "tool", "server": "stub", "tool": "touch_item", "description": "Marks item 1 as seen" })),
                RunBy::Person,
                json!({ "id": 1 }),
            );
            wait_for("the touch ask", || m.mcp.held("t").len() == 1).await;
            let held = asks_json(&m.mcp.held("t"));
            assert_eq!(held[0]["kind"], "tool");
            assert_eq!(held[0]["run"], "touch");
            assert_eq!(held[0]["tool"], "touch_item");
            assert_eq!(held[0]["description"], "Marks item 1 as seen");
            assert_eq!(m.records.last("touch").unwrap(), json!({ "state": "held", "why": "needs your approval", "runs": 0 }));
            // At bypass nothing asks but a run that names confirm, which asks on every start and keeps nothing.
            asks.set(ToolAsks::Never);
            let confirmed = decl(json!({ "kind": "tool", "server": "stub", "tool": "touch_item", "confirm": "Mark it seen?" }));
            for n in [1u64, 2] {
                start(&m, "t", "mark", confirmed.clone(), RunBy::Person, json!({ "id": 2 }));
                let ask = || asks_json(&m.mcp.held("t")).as_array().unwrap().iter().find(|a| a["run"] == "mark").cloned();
                wait_for("the mark ask", || ask().is_some()).await;
                let a = ask().unwrap();
                assert_eq!((a["kind"].clone(), a["confirm"].clone()), (json!("tool"), json!("Mark it seen?")));
                m.mcp.approve("t", a["key"].as_str().unwrap(), ApproveScope::Always);
                wait_for("mark done", || m.records.last("mark").map(|r| r["state"] == "done" && r["runs"] == n).unwrap_or(false)).await;
            }
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn asks_once_per_piece_of_code_and_new_arguments_ask_again() {
    // spec rules 8, 11, 15, 16 (TS slate-mcp.test.ts:256-380 at the module): an allowed key runs with no ask, a new
    // argument is a new key, Not now cancels.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), ..Opts::default() });
            let del =
                decl(json!({ "kind": "tool", "server": "stub", "tool": "delete_item", "description": "Deletes the item you picked" }));
            let first = start(&m, "t", "remove", del.clone(), RunBy::Person, json!({ "id": 7 }));
            assert_eq!(ans(&first)["outcome"], "held");
            let key7 = code_key(&del, &obj(json!({ "id": 7 })));
            assert_eq!(asks_json(&m.mcp.held("t"))[0]["key"], json!(key7));
            m.mcp.approve("t", &key7, ApproveScope::Always);
            wait_for("deleted 7", || m.records.last("remove").map(|r| r["out"] == "deleted 7").unwrap_or(false)).await;
            assert_eq!(m.approvals.list("t"), vec![key7.clone()]);
            assert!(m.mcp.owns("t", &key7));
            assert!(!m.mcp.owns("t", "cmd:abc"));
            // The same code again: no ask.
            let again = start(&m, "t", "remove", del.clone(), RunBy::Person, json!({ "id": 7 }));
            assert_eq!(ans(&again)["outcome"], "running");
            wait_for("runs 2", || m.records.last("remove").map(|r| r["runs"] == 2 && r["state"] == "done").unwrap_or(false)).await;
            // A new id is new code.
            let eight = start(&m, "t", "remove", del.clone(), RunBy::Person, json!({ "id": 8 }));
            assert_eq!(ans(&eight)["outcome"], "held");
            let key8 = code_key(&del, &obj(json!({ "id": 8 })));
            assert_ne!(key8, key7);
            m.mcp.deny("t", &key8);
            let last = m.records.last("remove").unwrap();
            assert_eq!(
                (last["state"].clone(), last["why"].clone(), last["exit"].clone(), last["runs"].clone()),
                (json!("cancelled"), json!("you said not to run it"), Value::Null, json!(2))
            );
            assert!(m.mcp.held("t").is_empty());
            // Once lets one start go and keeps nothing.
            start(&m, "t", "remove", del.clone(), RunBy::Person, json!({ "id": 9 }));
            let key9 = code_key(&del, &obj(json!({ "id": 9 })));
            m.mcp.approve("t", &key9, ApproveScope::Once);
            wait_for("deleted 9", || m.records.last("remove").map(|r| r["out"] == "deleted 9").unwrap_or(false)).await;
            assert!(!m.approvals.has("t", &key9));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn at_bypass_starts_a_tool_not_marked_read_only_with_no_ask() {
    // TS slate-mcp.test.ts:382-401 at the module.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts::default());
            let touch = decl(json!({ "kind": "tool", "server": "stub", "tool": "touch_item", "description": "Marks item 1 as seen" }));
            start(&m, "t", "touch", touch.clone(), RunBy::Timer, json!({ "id": 1 }));
            wait_for("touched", || m.records.last("touch").map(|r| r["out"] == "touched 1 on call 1").unwrap_or(false)).await;
            start(&m, "t", "touch", touch, RunBy::Timer, json!({ "id": 1 }));
            wait_for("touched again", || m.records.last("touch").map(|r| r["out"] == "touched 1 on call 2").unwrap_or(false)).await;
            assert!(m.mcp.held("t").is_empty());
            assert!(m.records.all("touch").iter().all(|r| r["state"] != "held"));
            assert_eq!(m.records.last("touch").unwrap()["text"], true);
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_sheet_names_the_tool_its_code_and_dots_for_a_secret() {
    // spec rules 7, 9, 17: the ask the TS printed, its description from the server's list once that arrives.
    LocalSet::new()
        .run_until(async {
            let mut secrets = Secrets::default();
            secrets.0.insert("key".into(), "sk_live_secretvalue".into());
            let m = make(stub_server(None), Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), secrets, ..Opts::default() });
            let held = start(&m, "t", "inbox", tool("stub", "list_items"), RunBy::Person, json!({ "params": { "account": "a", "limit": 2 }, "key": slate_secret_mark("key") }));
            assert_eq!(ans(&held), json!({ "outcome": "held", "record": { "state": "held", "why": "needs your approval", "runs": 0 } }));
            wait_for("the list arrived", || m.asks_called.get() >= 1).await;
            let key = "mcp:stub#cbe9475a35db13ca0174108f7d088b66";
            assert_eq!(
                asks_json(&m.mcp.held("t")),
                json!([{ "key": key, "run": "inbox", "kind": "tool", "server": "stub", "tool": "list_items", "computer": "this computer", "why": "needs your approval", "description": "Lists the newest items in an account.", "args": { "params": { "account": "a", "limit": 2 }, "key": "•••• (19)" } }])
            );
            assert_eq!(m.mcp.named("t", key), Some(NamedAsk { run: "inbox".into(), cmd: "stub.list_items {\"params\":{\"account\":\"a\",\"limit\":2},\"key\":\"•••• (19)\"}".into() }));
            assert_eq!(m.mcp.named("t", "mcp:stub#nothing"), None);
            // An unfilled secret reads as not filled; a resource asks with its uri and no args.
            start(&m, "t", "other", tool("stub", "list_items"), RunBy::Person, json!({ "key": slate_secret_mark("absent") }));
            start(&m, "t", "stat", decl(json!({ "kind": "resource", "server": "stub", "uri": "stub://status", "then": "jq ." })), RunBy::Person, json!({}));
            let asks = asks_json(&m.mcp.held("t"));
            assert_eq!(asks[1]["args"], json!({ "key": "•••• (not filled)" }));
            assert_eq!(asks[2]["tool"], "stub://status");
            assert_eq!(asks[2]["resource"], true);
            assert_eq!(asks[2]["args"], json!({}));
            assert_eq!(asks[2]["then"], "jq .");
            assert!(asks[2].get("description").is_none());
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_then_reading_a_slate_file_carries_its_text_in_the_ask() {
    // TS slate-mcp.test.ts:639-662 at the module: the file's text is on the sheet and in the key, so new code under
    // the same then asks again.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), ..Opts::default() });
            let code = "read -r line; printf '{\"first\":\"%s\"}' \"$line\"";
            let shaped = |text: &str| WithFiles {
                decl: serde_json::from_value(json!({ "kind": "tool", "server": "notes", "tool": "list_text", "then": "sh \"$SLATE_DIR/first.sh\"", "description": "Reads the first line of the inbox" })).unwrap(),
                files: Some([("first.sh".to_string(), text.to_string())].into_iter().collect()),
            };
            start(&m, "t", "first", shaped(code), RunBy::Person, json!({}));
            let ask = asks_json(&m.mcp.held("t"))[0].clone();
            assert_eq!(ask["then"], "sh \"$SLATE_DIR/first.sh\"");
            assert_eq!(ask["files"], json!({ "first.sh": code }));
            assert_eq!(ask["key"], json!(code_key(&shaped(code), &Map::new())));
            let changed = format!("{code}; :");
            assert_ne!(code_key(&shaped(&changed), &Map::new()), code_key(&shaped(code), &Map::new()));
            start(&m, "t", "first", shaped(&changed), RunBy::Person, json!({}));
            assert_eq!(asks_json(&m.mcp.held("t"))[0]["files"], json!({ "first.sh": changed }));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn named_cuts_to_120() {
    // spec rule 17.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), ..Opts::default() });
            start(&m, "t", "long", tool("stub", "list_items"), RunBy::Person, json!({ "q": "x".repeat(300) }));
            let key = code_key(&tool("stub", "list_items"), &obj(json!({ "q": "x".repeat(300) })));
            let named = m.mcp.named("t", &key).unwrap();
            assert_eq!(named.cmd, format!("stub.list_items {{\"q\":\"{}", "x".repeat(120 - "stub.list_items {\"q\":\"".len())));
            assert_eq!(named.cmd.chars().count(), 120);
            m.mcp.close();
        })
        .await;
}

// ---- results ----

#[tokio::test(flavor = "current_thread")]
async fn a_tool_run_fills_json_and_a_text_only_run_says_text() {
    // spec rules 12, 13: the records the TS printed, the clock held at 1000.
    LocalSet::new()
        .run_until(async {
            let mut secrets = Secrets::default();
            secrets.0.insert("key".into(), "sk_live_secretvalue".into());
            let clock = FakeClock::new(1000);
            let m = make(stub_server(None), Opts { secrets, clock: clock.clock(), ..Opts::default() });
            let running = start(&m, "t", "inbox", tool("stub", "list_items"), RunBy::Agent, json!({ "params": { "account": "a", "limit": 2 }, "key": slate_secret_mark("key") }));
            assert_eq!(ans(&running), json!({ "outcome": "running", "record": { "state": "running", "runs": 1, "startedAt": 1000 } }));
            wait_for("inbox", || state_of(&m.records.last("inbox")).as_deref() == Some("done")).await;
            assert_eq!(
                m.records.last("inbox").unwrap(),
                json!({ "state": "done", "exit": 0, "out": "listed 2 with key [secret:key]", "json": { "account": "a", "items": [{ "id": 1, "subject": "item 1 for a" }, { "id": 2, "subject": "item 2 for a" }], "call": 1, "key": "[secret:key]" }, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 })
            );
            start(&m, "t", "del", tool("stub", "delete_item"), RunBy::Agent, json!({ "id": 3 }));
            start(&m, "t", "plain", tool("notes", "list_text"), RunBy::Agent, json!({}));
            wait_for("both", || state_of(&m.records.last("del")).as_deref() == Some("done") && state_of(&m.records.last("plain")).as_deref() == Some("done")).await;
            assert_eq!(m.records.last("del").unwrap(), json!({ "state": "done", "exit": 0, "out": "deleted 3", "text": true, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            assert_eq!(m.records.last("plain").unwrap(), json!({ "state": "done", "exit": 0, "out": "# Inbox\n- 1 | ann | Hello\n- 2 | bo | Invoice", "text": true, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_resource_run_parses_its_one_text() {
    // spec rule 13.
    LocalSet::new()
        .run_until(async {
            let clock = FakeClock::new(1000);
            let m = make(stub_server(None), Opts { clock: clock.clock(), ..Opts::default() });
            start(&m, "t", "stat", decl(json!({ "kind": "resource", "server": "stub", "uri": "stub://status" })), RunBy::Agent, json!({}));
            wait_for("stat", || state_of(&m.records.last("stat")).as_deref() == Some("done")).await;
            assert_eq!(m.records.last("stat").unwrap(), json!({ "state": "done", "exit": 0, "out": "{\"ok\":true,\"calls\":0}", "json": { "ok": true, "calls": 0 }, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_tool_that_says_it_failed_fails() {
    // spec rule 13.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "x", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| Some(json!({ "content": [{ "type": "text", "text": "quota spent" }], "isError": true }))));
            let clock = FakeClock::new(1000);
            let m = make(served(&hand, vec![]), Opts { clock: clock.clock(), ..Opts::default() });
            start(&m, "t", "x", tool("hand", "x"), RunBy::Agent, json!({}));
            wait_for("failed", || state_of(&m.records.last("x")).as_deref() == Some("failed")).await;
            assert_eq!(m.records.last("x").unwrap(), json!({ "state": "failed", "why": "the tool said it failed", "exit": 1, "out": "", "err": "quota spent", "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_missing_tool_or_secret_fails_the_run() {
    // spec rule 12, the records the TS printed.
    LocalSet::new()
        .run_until(async {
            let clock = FakeClock::new(1000);
            let m = make(stub_server(None), Opts { clock: clock.clock(), ..Opts::default() });
            start(&m, "t", "nope", tool("stub", "nope"), RunBy::Agent, json!({}));
            start(&m, "t", "miss", tool("stub", "list_items"), RunBy::Agent, json!({ "key": slate_secret_mark("absent") }));
            wait_for("both failed", || state_of(&m.records.last("nope")).as_deref() == Some("failed") && state_of(&m.records.last("miss")).as_deref() == Some("failed")).await;
            assert_eq!(m.records.last("nope").unwrap(), json!({ "state": "failed", "why": "stub has no tool nope; slate_catalog stub lists its tools", "exit": null, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            assert_eq!(m.records.last("miss").unwrap(), json!({ "state": "failed", "why": "the secret $absent is not filled", "exit": null, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_then_reshapes_a_text_tool_into_json() {
    // spec rule 12: the then gets the cleaned raw text; its JSON replaces json and drops text; a failed then fails
    // the run over the kept exit and out.
    LocalSet::new()
        .run_until(async {
            let fed: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
            let f = fed.clone();
            let reshape: Rc<dyn Fn(&str, &str, &str) -> Reshape> = Rc::new(move |_t: &str, cmd: &str, input: &str| {
                f.borrow_mut().push((cmd.to_string(), input.to_string()));
                let answer = if cmd == "bad" { ReshapeAnswer::Failed { why: "then exited with 3".into(), exit: Some(3), err: "boom".into() } } else { ReshapeAnswer::Json(json!({ "items": [{ "id": 1 }] })) };
                Reshape { done: async move { answer }.boxed_local(), kill: Rc::new(|| {}) }
            });
            let clock = FakeClock::new(1000);
            let m = make(stub_server(None), Opts { clock: clock.clock(), reshape: Some(reshape), ..Opts::default() });
            start(&m, "t", "shaped", decl(json!({ "kind": "tool", "server": "notes", "tool": "list_text", "then": "shape" })), RunBy::Agent, json!({}));
            wait_for("shaped", || state_of(&m.records.last("shaped")).as_deref() == Some("done")).await;
            assert_eq!(m.records.last("shaped").unwrap(), json!({ "state": "done", "exit": 0, "out": "# Inbox\n- 1 | ann | Hello\n- 2 | bo | Invoice", "json": { "items": [{ "id": 1 }] }, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            assert_eq!(*fed.borrow(), vec![("shape".to_string(), "# Inbox\n- 1 | ann | Hello\n- 2 | bo | Invoice".to_string())]);
            start(&m, "t", "broken", decl(json!({ "kind": "tool", "server": "notes", "tool": "list_text", "then": "bad" })), RunBy::Agent, json!({}));
            wait_for("broken", || state_of(&m.records.last("broken")).as_deref() == Some("failed")).await;
            assert_eq!(m.records.last("broken").unwrap(), json!({ "state": "failed", "exit": 3, "out": "# Inbox\n- 1 | ann | Hello\n- 2 | bo | Invoice", "why": "then exited with 3", "err": "boom", "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            // A structured answer feeds the then its cleaned JSON.
            start(&m, "t", "count", decl(json!({ "kind": "tool", "server": "notes", "tool": "count", "then": "shape" })), RunBy::Agent, json!({}));
            wait_for("count", || state_of(&m.records.last("count")).as_deref() == Some("done")).await;
            assert_eq!(fed.borrow().last().unwrap().1, "{\"n\":3}");
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_tools_structured_answer_moves_the_catalog_to_returns_data() {
    // spec rules 13, 22 (TS slate-mcp.test.ts:539-583 at the module).
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts::default());
            let before = m.mcp.catalog("t", "notes").await.unwrap();
            assert!(before.contains("count [text only, read-only]: Counts the calls so far."), "{before}");
            start(&m, "t", "count", tool("notes", "count"), RunBy::Agent, json!({}));
            wait_for("count", || state_of(&m.records.last("count")).as_deref() == Some("done")).await;
            assert_eq!(m.records.last("count").unwrap()["json"], json!({ "n": 1 }));
            assert!(m.records.last("count").unwrap().get("text").is_none());
            let after = m.mcp.catalog("t", "notes").await.unwrap();
            assert!(after.contains("count [returns data, read-only]: Counts the calls so far."), "{after}");
            assert!(after.contains("list_text [text only, read-only]: Lists the inbox as markdown."), "{after}");
            m.mcp.close();
        })
        .await;
}

// ---- budgets, replacing and stopping ----

#[tokio::test(flavor = "current_thread")]
async fn a_persons_presses_meet_the_start_budget() {
    // TS slate-mcp.test.ts:622-637 at the module: the person's 13th start in a minute is held; the agent's own
    // starts have a budget of their own and still go.
    LocalSet::new()
        .run_until(async {
            let hand =
                Hand::new(json!([{ "name": "count", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| Some(text_result("{}"))));
            let m = make(served(&hand, vec![]), Opts::default());
            for n in 1..=STARTS_PER_MINUTE as u64 {
                let r = start(&m, "t", "count", tool("hand", "count"), RunBy::Person, json!({}));
                assert_eq!(ans(&r)["outcome"], "running", "start {n}");
                wait_for("the call ends", || m.records.last("count").is_some_and(|r| r["state"] == "done")).await;
            }
            let held = start(&m, "t", "count", tool("hand", "count"), RunBy::Person, json!({}));
            let held = ans(&held);
            assert_eq!(held["outcome"], "held");
            assert_eq!(held["record"]["why"], HELD_PRESSED);
            assert_eq!(held["record"]["runs"], STARTS_PER_MINUTE as u64);
            let agent = start(&m, "t", "count", tool("hand", "count"), RunBy::Agent, json!({}));
            assert_eq!(ans(&agent)["outcome"], "running");
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn once_beside_a_running_call_is_noop() {
    // spec rule 11.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| None));
            let m = make(served(&hand, vec![]), Opts::default());
            let once = decl(json!({ "kind": "tool", "server": "hand", "tool": "slow", "once": true }));
            start(&m, "t", "s", once.clone(), RunBy::Agent, json!({}));
            wait_for("the call is out", || !hand.held.borrow().is_empty()).await;
            let writes = m.records.all("s").len();
            let again = ans(&start(&m, "t", "s", once, RunBy::Agent, json!({})));
            assert_eq!(again["outcome"], "noop");
            assert_eq!(again["record"]["state"], "running");
            assert_eq!(m.records.all("s").len(), writes);
            assert_eq!(hand.calls().len(), 1);
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_start_held_beside_a_running_call_leaves_it_running() {
    // Invariant: the ask comes before that call stops; the call's own answer still lands.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow" }]), Box::new(|_, _| None));
            let m = make(served(&hand, vec![]), Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), ..Opts::default() });
            let slow = decl(json!({ "kind": "tool", "server": "hand", "tool": "slow", "description": "Slow" }));
            m.approvals.allow("t", &code_key(&slow, &obj(json!({ "n": 1 }))));
            start(&m, "t", "s", slow.clone(), RunBy::Agent, json!({ "n": 1 }));
            wait_for("the call is out", || !hand.held.borrow().is_empty()).await;
            let before = m.asks_called.get();
            let writes = m.records.all("s").len();
            let beside = ans(&start(&m, "t", "s", slow.clone(), RunBy::Agent, json!({ "n": 2 })));
            assert_eq!(beside["outcome"], "held");
            assert_eq!(beside["record"]["state"], "running");
            assert_eq!(m.records.all("s").len(), writes, "no record written for the held start");
            assert_eq!(m.asks_called.get(), before + 1);
            assert!(hand.notified("notifications/cancelled").is_empty());
            hand.release(text_result("first"));
            wait_for("the first call's answer", || m.records.last("s").map(|r| r["out"] == "first").unwrap_or(false)).await;
            assert_eq!(m.records.last("s").unwrap()["state"], "done");
            m.mcp.approve("t", &code_key(&slow, &obj(json!({ "n": 2 }))), ApproveScope::Once);
            wait_for("the second call is out", || hand.calls().len() == 2).await;
            assert_eq!(hand.calls()[1]["params"]["arguments"], json!({ "n": 2 }));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_replaced_start_never_writes_its_late_answer() {
    // Invariant: a start replaces a running call, telling the server; the old answer arriving later writes nothing.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| None));
            let m = make(served(&hand, vec![]), Opts::default());
            start(&m, "t", "s", tool("hand", "slow"), RunBy::Agent, json!({ "n": 1 }));
            wait_for("call 1", || hand.held.borrow().len() == 1).await;
            let first_id = hand.held.borrow()[0].clone();
            start(&m, "t", "s", tool("hand", "slow"), RunBy::Agent, json!({ "n": 2 }));
            wait_for("call 2", || hand.held.borrow().len() == 2).await;
            assert_eq!(hand.notified("notifications/cancelled"), vec![json!({ "requestId": first_id, "reason": "cancelled" })]);
            hand.release(text_result("old"));
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(m.records.all("s").iter().all(|r| r["out"] != "old"));
            hand.release(text_result("new"));
            wait_for("call 2's answer", || m.records.last("s").map(|r| r["out"] == "new").unwrap_or(false)).await;
            assert_eq!(m.records.last("s").unwrap()["runs"], 2);
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_tool_run_started_again_keeps_its_last_result_marked_refreshing() {
    // TS slate-mcp.test.ts:605-620 at the module.
    LocalSet::new()
        .run_until(async {
            let clock = FakeClock::new(1000);
            let m = make(stub_server(None), Opts { clock: clock.clock(), ..Opts::default() });
            start(&m, "t", "count", tool("notes", "count"), RunBy::Person, json!({}));
            wait_for("count 1", || state_of(&m.records.last("count")).as_deref() == Some("done")).await;
            assert_eq!(m.records.last("count").unwrap(), json!({ "state": "done", "exit": 0, "out": "1 calls", "json": { "n": 1 }, "startedAt": 1000, "endedAt": 1000, "ms": 0, "runs": 1 }));
            let running = ans(&start(&m, "t", "count", tool("notes", "count"), RunBy::Person, json!({})));
            assert_eq!(running["record"], json!({ "exit": 0, "out": "1 calls", "json": { "n": 1 }, "endedAt": 1000, "ms": 0, "state": "running", "runs": 2, "startedAt": 1000, "refreshing": true }));
            wait_for("count 2", || m.records.last("count").map(|r| r["runs"] == 2 && r["state"] == "done").unwrap_or(false)).await;
            assert_eq!(m.records.last("count").unwrap()["json"], json!({ "n": 2 }));
            assert!(m.records.last("count").unwrap().get("refreshing").is_none());
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn deny_cancels_a_held_start_and_leaves_a_running_call() {
    // spec rule 16.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow" }]), Box::new(|_, _| None));
            let clock = FakeClock::new(1000);
            let m =
                make(served(&hand, vec![]), Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), clock: clock.clock(), ..Opts::default() });
            let slow = decl(json!({ "kind": "tool", "server": "hand", "tool": "slow", "description": "Slow" }));
            m.approvals.allow("t", &code_key(&slow, &obj(json!({ "n": 1 }))));
            start(&m, "t", "s", slow.clone(), RunBy::Agent, json!({ "n": 1 }));
            wait_for("the call is out", || !hand.held.borrow().is_empty()).await;
            start(&m, "t", "s", slow.clone(), RunBy::Agent, json!({ "n": 2 }));
            m.mcp.deny("t", &code_key(&slow, &obj(json!({ "n": 2 }))));
            assert!(m.mcp.held("t").is_empty());
            assert_eq!(m.records.last("s").unwrap()["state"], "running");
            assert!(hand.notified("notifications/cancelled").is_empty());
            hand.release(text_result("kept"));
            wait_for("the call finished", || m.records.last("s").map(|r| r["out"] == "kept").unwrap_or(false)).await;
            // A held start with nothing running is cancelled, its record keeping no result.
            start(&m, "t", "s", slow.clone(), RunBy::Agent, json!({ "n": 3 }));
            m.mcp.deny("t", &code_key(&slow, &obj(json!({ "n": 3 }))));
            assert_eq!(
                m.records.last("s").unwrap(),
                json!({ "state": "cancelled", "why": "you said not to run it", "exit": null, "runs": 1, "endedAt": 1000 })
            );
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancel_tells_the_server_and_writes_cancelled() {
    // spec rules 14, 19.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| None));
            let clock = FakeClock::new(1000);
            let m = make(served(&hand, vec![]), Opts { clock: clock.clock(), ..Opts::default() });
            start(&m, "t", "s", tool("hand", "slow"), RunBy::Agent, json!({}));
            wait_for("the call is out", || !hand.held.borrow().is_empty()).await;
            let id = hand.held.borrow()[0].clone();
            m.mcp.cancel("t", "s");
            assert_eq!(
                m.records.last("s").unwrap(),
                json!({ "state": "cancelled", "why": "cancelled", "exit": null, "runs": 1, "endedAt": 1000 })
            );
            assert_eq!(hand.notified("notifications/cancelled"), vec![json!({ "requestId": id, "reason": "cancelled" })]);
            hand.release(text_result("late"));
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert_eq!(m.records.last("s").unwrap()["state"], "cancelled");
            // Cancelling a run that is done writes nothing.
            let writes = m.records.all("s").len();
            m.mcp.cancel("t", "s");
            assert_eq!(m.records.all("s").len(), writes);
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stop_all_quiet_writes_nothing() {
    // spec rule 19: quiet stops with no record; with a why every running run is cancelled with it.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| None));
            let m = make(served(&hand, vec![]), Opts::default());
            start(&m, "t", "a", tool("hand", "slow"), RunBy::Agent, json!({}));
            start(&m, "t", "b", tool("hand", "slow"), RunBy::Agent, json!({}));
            wait_for("both calls out", || hand.held.borrow().len() == 2).await;
            let writes = m.records.0.borrow().len();
            m.mcp.stop_all("t", StopAll { quiet: true, why: None });
            assert_eq!(m.records.0.borrow().len(), writes);
            assert_eq!(hand.notified("notifications/cancelled").len(), 2);
            start(&m, "t", "c", tool("hand", "slow"), RunBy::Agent, json!({}));
            wait_for("c out", || hand.held.borrow().len() == 3).await;
            m.mcp.stop_all("t", StopAll { quiet: false, why: Some("the thread was stopped".into()) });
            assert_eq!(m.records.last("c").unwrap()["why"], "the thread was stopped");
            assert_eq!(
                m.records.last("a").unwrap()["state"],
                "cancelled",
                "the quiet stop wrote nothing for a, so the second stop finds it running"
            );
            m.mcp.close();
        })
        .await;
}

// ---- the catalog and the write's refusal ----

#[tokio::test(flavor = "current_thread")]
async fn catalog_answers_the_stub_servers_tools_folded_with_how_the_thread_asks() {
    // spec rule 22: the text the TS printed, verbatim.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts { asks: Rc::new(Cell::new(ToolAsks::Every)), ..Opts::default() });
            assert_eq!(
                m.mcp.catalog("t", "stub").await.unwrap(),
                "stub: an MCP server of this thread's agent, 3 tools, 1 resource.\nThis thread's agent asks before every tool, so the person allows each run's code once, on the slate: new arguments, a new then or a new tool ask again.\nA run on a tool not marked read-only takes description=\"what it does, in one plain line\", which the person reads before allowing it; a write without one is refused.\nA run calls a tool: <run name=\"x\" tool=\"stub.<tool>\" args={{ <property>: <expression> }} every={60} />; a resource: <run name=\"x\" resource=\"stub:<uri>\" />. $x.json holds the structured result (else the text parsed as JSON), $x.out the text, $x.err the error.\nEach tool says whether it returns data (a structured result fills $x.json) or text only (prose for a model: draw $x.out, or reshape it with then=).\n\nlist_items [returns data, read-only]: Lists the newest items in an account.\n  params: object, required\n  params.account: string, required. The account's alias.\n  params.limit: integer, default 10, min 1, max 50\n  key: string. An API key the stub echoes back.\ndelete_item [text only, destructive]: Deletes one item for good.\n  id: integer, required\ntouch_item [text only]: Marks one item as seen.\n  id: integer, required\n\nresource stub://status (application/json): The stub's own status."
            );
            assert_eq!(
                m.mcp.catalog("t", "notes").await.unwrap(),
                "notes: an MCP server of this thread's agent, 2 tools.\nThis thread's agent asks before every tool, so the person allows each run's code once, on the slate: new arguments, a new then or a new tool ask again.\nA run on a tool not marked read-only takes description=\"what it does, in one plain line\", which the person reads before allowing it; a write without one is refused.\nA run calls a tool: <run name=\"x\" tool=\"notes.<tool>\" args={{ <property>: <expression> }} every={60} />; a resource: <run name=\"x\" resource=\"notes:<uri>\" />. $x.json holds the structured result (else the text parsed as JSON), $x.out the text, $x.err the error.\nEach tool says whether it returns data (a structured result fills $x.json) or text only (prose for a model: draw $x.out, or reshape it with then=).\n\nlist_text [text only, read-only]: Lists the inbox as markdown.\n  (no arguments)\ncount [text only, read-only]: Counts the calls so far.\n  (no arguments)"
            );
            assert_eq!(m.mcp.catalog("t", "zoho").await, Err("no MCP server called zoho is in Claude Code's config there".to_string()));
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn refused_runs_refuses_a_new_or_changed_tool_run_with_no_description() {
    // TS slate-mcp.test.ts:403-427 at the module: judged by the server's own list, asked when not held.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts::default());
            let refused = |d: &SlateDoc, before: Option<&SlateDoc>| {
                let mcp = m.mcp.clone();
                let (d, before) = (d.clone(), before.cloned());
                async move { serde_json::to_value(mcp.refused_runs("t", before.as_ref(), Some(&d), vec![]).await).unwrap() }
            };
            let del = doc(json!({ "del": { "kind": "tool", "server": "stub", "tool": "delete_item", "args": { "id": 1 } } }));
            assert_eq!(
                refused(&del, None).await,
                json!([{ "code": "K713", "name": "description-missing", "message": "$del calls stub.delete_item, which its server does not mark read-only, so it needs description=\"...\" saying in one plain line what it does; the person reads it before allowing the run" }])
            );
            let touch = doc(json!({ "touch": { "kind": "tool", "server": "stub", "tool": "touch_item" } }));
            assert_eq!(refused(&touch, None).await[0]["code"], "K713");
            let fine = doc(json!({
                "list": { "kind": "tool", "server": "stub", "tool": "list_items", "args": { "params": { "account": "a" } } },
                "stat": { "kind": "resource", "server": "stub", "uri": "stub://status" },
                "gone": { "kind": "tool", "server": "stub", "tool": "delete_item", "args": { "id": 1 }, "description": "Deletes item 1" }
            }));
            assert_eq!(refused(&fine, None).await, json!([]));
            // Taking the description off a run changes it.
            let bare = doc(json!({ "gone": { "kind": "tool", "server": "stub", "tool": "delete_item", "args": { "id": 1 } } }));
            assert_eq!(refused(&bare, Some(&fine)).await[0]["message"].as_str().unwrap().starts_with("$gone calls stub.delete_item"), true);
            // A server that does not answer marks nothing read-only.
            let unknown = doc(json!({ "x": { "kind": "tool", "server": "zoho", "tool": "list" } }));
            assert_eq!(refused(&unknown, None).await[0]["code"], "K713");
            // What the thread's computer refused stands alone.
            let away = vec![serde_json::from_value(json!({ "code": "K711", "name": "host-away", "message": "m" })).unwrap()];
            assert_eq!(serde_json::to_value(m.mcp.refused_runs("t", None, Some(&del), away).await).unwrap(), json!([{ "code": "K711", "name": "host-away", "message": "m" }]));
            assert!(m.mcp.refused_runs("t", None, None, vec![]).await.is_empty());
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn refused_runs_never_judges_a_run_kept_as_it_was() {
    // spec rule 23: a run equal to the slate before's asks no server at all.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts::default());
            let kept = doc(json!({ "del": { "kind": "tool", "server": "stub", "tool": "delete_item", "args": { "id": 1 } } }));
            assert!(m.mcp.refused_runs("t", Some(&kept), Some(&kept), vec![]).await.is_empty());
            assert_eq!(m.servers_asked.get(), 0);
            m.mcp.close();
        })
        .await;
}

// ---- dropping ----

#[tokio::test(flavor = "current_thread")]
async fn drop_with_keep_leaves_the_server_running() {
    // spec rule 24 (TS slate-box.test.ts:597-633 at the module).
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| None));
            let m = make(served(&hand, vec![]), Opts::default());
            start(&m, "t", "s", tool("hand", "slow"), RunBy::Agent, json!({}));
            wait_for("the call is out", || !hand.held.borrow().is_empty()).await;
            m.mcp.drop_thread("t", true);
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert_eq!(hand.kills.get(), 0);
            assert!(hand.notified("notifications/cancelled").is_empty());
            m.mcp.close();
        })
        .await;
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(json!([{ "name": "slow", "annotations": { "readOnlyHint": true } }]), Box::new(|_, _| None));
            let m = make(served(&hand, vec![]), Opts::default());
            start(&m, "t", "s", tool("hand", "slow"), RunBy::Agent, json!({}));
            wait_for("the call is out", || !hand.held.borrow().is_empty()).await;
            m.mcp.drop_thread("t", false);
            wait_for("the server closed", || hand.kills.get() == 1).await;
            assert_eq!(hand.notified("notifications/cancelled").len(), 1);
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn drop_forgets_lists_and_seen_marks() {
    // spec rule 24: after a drop, the catalog shows count as text only again and a provisional start no longer
    // knows the tool is read-only.
    LocalSet::new()
        .run_until(async {
            let m = make(stub_server(None), Opts { asks: Rc::new(Cell::new(ToolAsks::UnlessReadOnly)), ..Opts::default() });
            m.mcp.catalog("t", "notes").await.unwrap();
            start(&m, "t", "count", tool("notes", "count"), RunBy::Agent, json!({}));
            wait_for("count", || state_of(&m.records.last("count")).as_deref() == Some("done")).await;
            assert_eq!(json_of(&m.mcp.provisional("t", &tool("notes", "count"), None, &Map::new()))["state"], "running");
            m.mcp.drop_thread("t", false);
            // Not listed any more: only the hints of an unlisted tool decide, which is not "asks".
            assert_eq!(json_of(&m.mcp.provisional("t", &tool("notes", "count"), None, &Map::new()))["state"], "running");
            assert_eq!(json_of(&m.mcp.provisional("t", &tool("stub", "touch_item"), None, &Map::new()))["state"], "running");
            let after = m.mcp.catalog("t", "notes").await.unwrap();
            assert!(after.contains("count [text only, read-only]"), "{after}");
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn provisional_is_held_where_it_will_ask_else_running_with_the_kept_result() {
    // spec rule 25, the records the TS printed.
    LocalSet::new()
        .run_until(async {
            let clock = FakeClock::new(1000);
            let asks = Rc::new(Cell::new(ToolAsks::Every));
            let m = make(stub_server(None), Opts { asks: asks.clone(), clock: clock.clock(), ..Opts::default() });
            let d = tool("stub", "list_items");
            assert_eq!(json_of(&m.mcp.provisional("t", &d, None, &Map::new())), json!({ "state": "held", "why": "needs your approval", "runs": 0 }));
            let last: RunRecord = serde_json::from_value(json!({ "state": "done", "runs": 3, "out": "o", "json": { "a": 1 }, "startedAt": 5, "endedAt": 6, "ms": 1, "exit": 0 })).unwrap();
            assert_eq!(json_of(&m.mcp.provisional("t", &d, Some(&last), &Map::new())), json!({ "state": "held", "why": "needs your approval", "runs": 3 }));
            asks.set(ToolAsks::Never);
            assert_eq!(
                json_of(&m.mcp.provisional("t", &d, Some(&last), &Map::new())),
                json!({ "exit": 0, "out": "o", "json": { "a": 1 }, "endedAt": 6, "ms": 1, "state": "running", "runs": 4, "startedAt": 1000, "refreshing": true })
            );
            assert_eq!(json_of(&m.mcp.provisional("t", &d, None, &Map::new())), json!({ "state": "running", "runs": 1, "startedAt": 1000 }));
            m.mcp.close();
        })
        .await;
}

// ---- invariants ----

#[tokio::test(flavor = "current_thread")]
async fn a_secret_reaches_the_call_and_comes_back_scrubbed_from_every_road() {
    // Invariant: the plaintext reaches the server inside the call's arguments only, and nothing written back holds it
    // or the server's own secret value.
    LocalSet::new()
        .run_until(async {
            let hand = Hand::new(
                json!([{ "name": "echo", "annotations": { "readOnlyHint": true }, "description": "Echoes own-s3cr3t-value back." }]),
                Box::new(|_, params| Some(json!({ "content": [{ "type": "text", "text": format!("own own-s3cr3t-value and {}", params["arguments"]["key"].as_str().unwrap_or("")) }], "structuredContent": { "key": params["arguments"]["key"] } }))),
            );
            let mut secrets = Secrets::default();
            secrets.0.insert("key".into(), "sk_live_51HxQ2mZ9vT3pLk".into());
            let m = make(served(&hand, vec!["own-s3cr3t-value".into()]), Opts { secrets, ..Opts::default() });
            start(&m, "t", "e", tool("hand", "echo"), RunBy::Agent, json!({ "key": slate_secret_mark("key") }));
            wait_for("echo", || state_of(&m.records.last("e")).as_deref() == Some("done")).await;
            assert_eq!(hand.calls()[0]["params"]["arguments"], json!({ "key": "sk_live_51HxQ2mZ9vT3pLk" }));
            let rec = m.records.last("e").unwrap();
            assert_eq!(rec["out"], "own *** and [secret:key]");
            assert_eq!(rec["json"], json!({ "key": "[secret:key]" }));
            let catalog = m.mcp.catalog("t", "hand").await.unwrap();
            assert!(catalog.contains("echo [returns data, read-only]: Echoes *** back."), "{catalog}");
            let all = serde_json::to_string(&m.records.0.borrow().iter().map(|(_, _, v)| v.clone()).collect::<Vec<_>>()).unwrap();
            assert!(!all.contains("sk_live_51HxQ2mZ9vT3pLk") && !all.contains("own-s3cr3t-value"), "{all}");
            m.mcp.close();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn one_connection_serves_every_call_of_a_thread() {
    // Invariant (TS slate-mcp.test.ts:356-357): one server per thread and server, a second thread its own.
    LocalSet::new()
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let starts = dir.path().join("starts");
            let m = make(stub_server(Some(starts.clone())), Opts::default());
            for n in 1..=3u64 {
                start(&m, "t", "touch", tool("stub", "touch_item"), RunBy::Agent, json!({ "id": n }));
                wait_for("touched", || m.records.last("touch").map(|r| r["runs"] == n && r["state"] == "done").unwrap_or(false)).await;
            }
            start(&m, "t", "del", tool("stub", "delete_item"), RunBy::Agent, json!({ "id": 1 }));
            wait_for("deleted", || state_of(&m.records.last("del")).as_deref() == Some("done")).await;
            assert_eq!(spawned(&starts), 1);
            assert_eq!(m.records.last("touch").unwrap()["out"], "touched 3 on call 3");
            start(&m, "u", "touch", tool("stub", "touch_item"), RunBy::Agent, json!({ "id": 1 }));
            wait_for("thread u", || m.records.0.borrow().iter().any(|(t, r, v)| t == "u" && r == "touch" && v["state"] == "done")).await;
            assert_eq!(spawned(&starts), 2);
            m.mcp.close();
        })
        .await;
}

#[allow(dead_code)]
fn _uses(_: Arc<()>) {}
