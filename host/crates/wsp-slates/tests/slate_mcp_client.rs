// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of spec/slate_mcp_client.md: slate-mcp.ts:1-556 at 65f6b89c7. Expected values are the TS's own, from
//! test/slate-mcp.test.ts or printed by the TS under node.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::task::LocalSet;

use wsp_core::clock::real_clock;
use wsp_slates::pending::slate::{SlateDoc, SlateRunDecl};
use wsp_slates::slate_files::WithFiles;
use wsp_slates::slate_mcp::*;

fn decl(v: Value) -> SlateRunDecl {
    serde_json::from_value(v).expect("a declaration")
}

fn doc(runs: Value) -> SlateDoc {
    serde_json::from_value(
        json!({ "schema": 2, "root": "root", "values": {}, "derived": {}, "reactions": [], "pieces": {}, "runs": runs, "files": {} }),
    )
    .expect("a document")
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().expect("an object").clone()
}

fn with(d: Value, files: Option<Value>) -> WithFiles {
    WithFiles { decl: decl(d), files: files.map(|f| serde_json::from_value::<IndexMap<String, String>>(f).expect("files")) }
}

fn away(machine: Option<&str>) -> Away {
    Away { computer: "spoo".into(), host: "laptop".into(), machine: machine.map(String::from) }
}

const HOST_AWAY_A: &str =
    "$a runs on=\"host\", which is laptop with the sign-ins there, and this thread runs on spoo: drop on=\"host\" and it runs on spoo";

fn nest(n: usize) -> Value {
    let mut v = json!(1);
    for _ in 0..n {
        v = json!([v]);
    }
    v
}

fn deep_text(depth: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(depth), "}".repeat(depth))
}

// ---- refusals of a write from away ----

#[test]
fn host_away_says_both_computers() {
    // spec rule 1, the words printed by the TS.
    assert_eq!(
        host_away(Some(&away(None)), "deploy").as_deref(),
        Some("$deploy runs on=\"host\", which is laptop with the sign-ins there, and this thread runs on spoo: drop on=\"host\" and it runs on spoo")
    );
    assert_eq!(host_away(None, "deploy"), None);
}

#[tokio::test(flavor = "current_thread")]
async fn refused_away_refuses_host_runs_from_away() {
    // spec rule 2: only the run on="host", by name.
    let d = doc(json!({ "a": { "kind": "cmd", "cmd": "x", "on": "host" }, "b": { "kind": "cmd", "cmd": "y" } }));
    let got = refused_away(None, Some(&d), Some(&away(None)), None).await;
    assert_eq!(serde_json::to_value(&got).unwrap(), json!([{ "code": "K711", "name": "host-away", "message": HOST_AWAY_A }]));
}

#[tokio::test(flavor = "current_thread")]
async fn refused_away_on_a_machine_refuses_every_tool_run_once_and_asks_nothing() {
    // spec rule 2: one K712 naming every tool and resource run; missing_on is never called.
    let d = doc(json!({
        "a": { "kind": "cmd", "cmd": "x", "on": "host" },
        "t": { "kind": "tool", "server": "s", "tool": "x" },
        "r": { "kind": "resource", "server": "s", "uri": "u" }
    }));
    let called = Rc::new(Cell::new(false));
    let seen = called.clone();
    let missing: MissingOnFn = Rc::new(move |_servers: Vec<String>| {
        seen.set(true);
        async { panic!("missing_on asked for a machine on a box") }.boxed_local()
    });
    let got = refused_away(None, Some(&d), Some(&away(Some("m1"))), Some(missing)).await;
    assert_eq!(
        serde_json::to_value(&got).unwrap(),
        json!([
            { "code": "K711", "name": "host-away", "message": HOST_AWAY_A },
            { "code": "K712", "name": "machine-on-box", "message": "$t, $r call an MCP server, and this thread runs on m1, a machine wsp made on spoo, whose server would start on spoo outside it: a slate's tool run goes only to a thread in a folder on spoo or on a cloud machine, so start the thread in the project's folder on spoo" }
        ])
    );
    assert!(!called.get());
}

#[tokio::test(flavor = "current_thread")]
async fn refused_away_asks_only_servers_the_slate_before_did_not_name() {
    // spec rule 2: `kept` was named before, so it is not asked; `new` once though two runs name it.
    let before = doc(json!({ "k": { "kind": "tool", "server": "kept", "tool": "x" } }));
    let d = doc(json!({
        "k": { "kind": "tool", "server": "kept", "tool": "x" },
        "n": { "kind": "tool", "server": "new", "tool": "x" },
        "m": { "kind": "resource", "server": "new", "uri": "u" },
        "o": { "kind": "tool", "server": "offs", "tool": "x" }
    }));
    let asked = Rc::new(RefCell::new(Vec::<Vec<String>>::new()));
    let log = asked.clone();
    let missing: MissingOnFn = Rc::new(move |servers: Vec<String>| {
        log.borrow_mut().push(servers);
        async {
            MissingOn {
                agent_id: "claude".into(),
                agent_name: "Claude Code".into(),
                thread: "t1".into(),
                missing: vec!["new".into()],
                off: vec!["offs is off in Claude Code's config on spoo".into()],
            }
        }
        .boxed_local()
    });
    let got = refused_away(Some(&before), Some(&d), Some(&away(None)), Some(missing)).await;
    assert_eq!(*asked.borrow(), vec![vec!["new".to_string(), "offs".to_string()]]);
    assert_eq!(
        serde_json::to_value(&got).unwrap(),
        json!([
            { "code": "K710", "name": "server-not-set-up", "message": "new is not set up on spoo: add it to Claude Code's MCP config there with wsp servers add new t1 --agent claude --command \"<its command line>\" (or --url <its address>), then write the run again" },
            { "code": "K714", "name": "server-off", "message": "offs is off in Claude Code's config on spoo" }
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn refused_away_with_no_away_or_no_doc_refuses_nothing() {
    // spec rule 2.
    let d = doc(json!({ "a": { "kind": "cmd", "cmd": "x", "on": "host" } }));
    assert!(refused_away(None, Some(&d), None, None).await.is_empty());
    assert!(refused_away(None, None, Some(&away(None)), None).await.is_empty());
    // A write whose servers were all named before asks nothing and refuses nothing.
    let tools = doc(json!({ "t": { "kind": "tool", "server": "s", "tool": "x" } }));
    let missing: MissingOnFn = Rc::new(|_s: Vec<String>| async { panic!("asked about a kept server") }.boxed_local());
    assert!(refused_away(Some(&tools), Some(&tools), Some(&away(None)), Some(missing)).await.is_empty());
}

// ---- the approval key ----

#[test]
fn code_key_keys_what_is_sent() {
    // TS slate-mcp.test.ts:520-537.
    let base = json!({ "kind": "tool", "server": "cf", "tool": "execute", "args": { "code": "return 1", "since": { "bind": "$since" } } });
    let sent = obj(json!({ "code": "return 1", "since": "2026-10-13T00:00:00Z", "token": slate_secret_mark("cf_token") }));
    let with_key = |extra: Value| {
        let mut d = base.clone();
        for (k, v) in extra.as_object().unwrap() {
            d[k] = v.clone();
        }
        d
    };
    let key = code_key(&with(base.clone(), None), &sent);
    assert!(regex_like_key(&key, "cf"), "{key}");
    assert_eq!(code_key(&with(with_key(json!({ "description": "Reads the traffic" })), None), &sent), key);
    assert_eq!(code_key(&with(with_key(json!({ "every": 60 })), None), &sent), key);
    let reordered = obj(json!({ "token": slate_secret_mark("cf_token"), "since": "2026-10-13T00:00:00Z", "code": "return 1" }));
    assert_eq!(code_key(&with(base.clone(), None), &reordered), key);
    let mut later = sent.clone();
    later.insert("since".into(), json!("2026-10-13T01:00:00Z"));
    assert_ne!(code_key(&with(base.clone(), None), &later), key);
    let mut code2 = sent.clone();
    code2.insert("code".into(), json!("return 2"));
    assert_ne!(code_key(&with(base.clone(), None), &code2), key);
    let mut other = sent.clone();
    other.insert("token".into(), json!(slate_secret_mark("other_token")));
    assert_ne!(code_key(&with(base.clone(), None), &other), key);
    assert_ne!(code_key(&with(with_key(json!({ "tool": "search" })), None), &sent), key);
    assert_ne!(code_key(&with(with_key(json!({ "server": "cf2" })), None), &sent), key);
    assert_ne!(code_key(&with(with_key(json!({ "then": "jq ." })), None), &sent), key);
    assert_ne!(
        code_key(&with(with_key(json!({ "then": "sh x.sh" })), Some(json!({ "x.sh": "a" }))), &sent),
        code_key(&with(with_key(json!({ "then": "sh x.sh" })), Some(json!({ "x.sh": "b" }))), &sent)
    );
    assert_ne!(
        code_key(&with(json!({ "kind": "resource", "server": "cf", "uri": "cf://a" }), None), &Map::new()),
        code_key(&with(json!({ "kind": "resource", "server": "cf", "uri": "cf://b" }), None), &Map::new())
    );
}

fn regex_like_key(key: &str, server: &str) -> bool {
    let prefix = format!("mcp:{server}#");
    key.starts_with(&prefix)
        && key.len() == prefix.len() + 32
        && key[prefix.len()..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[test]
fn code_key_matches_the_ts_hashes() {
    // spec rule 3: the hashes the TS printed.
    let sent = obj(json!({ "code": "return 1", "since": "2026-10-13T00:00:00Z", "token": slate_secret_mark("cf_token") }));
    let tool = json!({ "kind": "tool", "server": "cf", "tool": "execute", "args": { "code": "return 1", "since": { "bind": "$since" } } });
    assert_eq!(code_key(&with(tool.clone(), None), &sent), "mcp:cf#996f1355ae8f176e493b83d3f838d67d");
    assert_eq!(
        code_key(&with(json!({ "kind": "resource", "server": "cf", "uri": "cf://a" }), None), &Map::new()),
        "mcp:cf#5b4053a011e2b3022238ae93d02bed6c"
    );
    let mut then = tool;
    then["then"] = json!("sh x.sh");
    assert_eq!(code_key(&with(then, Some(json!({ "x.sh": "a", "a.sh": "b" }))), &sent), "mcp:cf#77d7a8ffb7e8bb661ae4b3039a09da2c");
    let stub = json!({ "kind": "tool", "server": "stub", "tool": "list_items" });
    assert_eq!(
        code_key(&with(stub, None), &obj(json!({ "params": { "account": "a", "limit": 1 } }))),
        "mcp:stub#298f5d9100fc1c99ef0297baa6c00a07"
    );
}

#[test]
fn code_key_sorts_files_as_javascript_sorts_pairs() {
    // spec rule 3 and its after-port note: "a+,y" sorts before "a,z", whatever order the files came in.
    let d = json!({ "kind": "tool", "server": "s", "tool": "t", "then": "x" });
    let one = code_key(&with(d.clone(), Some(json!({ "a": "z", "a+": "y" }))), &Map::new());
    let two = code_key(&with(d, Some(json!({ "a+": "y", "a": "z" }))), &Map::new());
    assert_eq!(one, "mcp:s#24f8e8350374e0724522b13c6c46b49f");
    assert_eq!(two, one);
}

#[test]
fn secret_mark_is_nul_framed() {
    // spec rule 4.
    assert_eq!(slate_secret_mark("k"), "\u{0}wsp-secret:k\u{0}");
    assert_eq!(SECRET_MARK, "\u{0}wsp-secret:");
}

// ---- shape and size ----

#[test]
fn shape_refusal_refuses_past_64_deep_and_100000_entries() {
    // spec rule 6, measured against the TS walk.
    assert_eq!(shape_refusal(&nest(64)), None);
    assert_eq!(shape_refusal(&nest(65)).as_deref(), Some("the server's answer is nested over 64 deep"));
    assert_eq!(shape_refusal(&Value::Array(vec![json!(0); 100_000])), None);
    assert_eq!(shape_refusal(&Value::Array(vec![json!(0); 100_001])).as_deref(), Some("the server's answer holds over 100000 entries"));
    // The outer container's own children count too: 2 + 50000 + 49999.
    let split = json!([Value::Array(vec![json!(0); 50_000]), Value::Array(vec![json!(0); 49_999])]);
    assert_eq!(shape_refusal(&split).as_deref(), Some("the server's answer holds over 100000 entries"));
    assert_eq!(shape_refusal(&json!("text")), None);
    assert_eq!(shape_refusal(&Value::Null), None);
}

#[test]
fn parsed_text_keeps_deep_json_as_text() {
    // spec rule 7: a text too deep for any parser is None, never a crash.
    assert_eq!(parsed_text(&deep_text(100_000)), None);
    assert_eq!(parsed_text(&serde_json::to_string(&nest(65)).unwrap()), None);
    assert_eq!(parsed_text("{\"a\":1}"), Some(json!({ "a": 1 })));
    assert_eq!(parsed_text("not json"), None);
    assert_eq!(parsed_text("3"), Some(json!(3)));
}

// ---- the stdio client over a pipe in the test's hands ----

#[derive(Default)]
struct FakePipe {
    writes: RefCell<Vec<String>>,
    kills: Cell<u32>,
}

impl McpPipe for FakePipe {
    fn write(&self, text: &str) {
        self.writes.borrow_mut().push(text.to_string());
    }
    fn kill(&self) {
        self.kills.set(self.kills.get() + 1);
    }
    fn pid(&self) -> Option<u32> {
        None
    }
}

struct Piped {
    client: Rc<dyn McpClient>,
    pipe: Rc<FakePipe>,
    events: Rc<RefCell<Option<McpPipeEvents>>>,
    closes: Rc<Cell<u32>>,
}

impl Piped {
    fn out(&self, bytes: &[u8]) {
        let events = self.events.borrow().clone().expect("opened");
        (events.out)(bytes);
    }
    fn err(&self, text: &str) {
        let events = self.events.borrow().clone().expect("opened");
        (events.err)(text);
    }
    fn exit(&self, why: &str) {
        let events = self.events.borrow().clone().expect("opened");
        (events.exit)(why);
    }
    fn written(&self) -> Vec<String> {
        self.pipe.writes.borrow().clone()
    }
}

fn stdio_transport() -> McpTransport {
    McpTransport::Stdio { command: "stub".into(), args: vec![], env: IndexMap::new(), cwd: None, tool_timeout_sec: None }
}

fn piped() -> Piped {
    let pipe = Rc::new(FakePipe::default());
    let events = Rc::new(RefCell::new(None));
    let (p, e) = (pipe.clone(), events.clone());
    let open: McpPipeOpen = Rc::new(move |_t: &McpTransport, _s: &McpServerSpec, on: McpPipeEvents| {
        *e.borrow_mut() = Some(on);
        p.clone() as Rc<dyn McpPipe>
    });
    let t = stdio_transport();
    let spec = McpServerSpec {
        transport: t.clone(),
        cwd: "/".into(),
        env: IndexMap::new(),
        secrets: vec![],
        open: Some(open),
        post: None,
        sign_in: None,
    };
    let closes = Rc::new(Cell::new(0));
    let c = closes.clone();
    let client = stdio_client(&t, &spec, Box::new(move || c.set(c.get() + 1)), real_clock());
    Piped { client, pipe, events, closes }
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_client_answers_by_id_and_skips_noise() {
    // spec rule 8: ids from 1, the exact line written, noise skipped, an answer split across chunks.
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let (id, answer) = p.client.request("tools/list", obj(json!({ "x": 1 })), 5_000);
            assert_eq!(id, 1);
            assert_eq!(p.written(), vec!["{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{\"x\":1}}\n".to_string()]);
            p.out(b"\n   \ngarbage\n{\"id\":\"1\",\"result\":2}\n{\"jsonrpc\":\"2.0\",\"id\":99,\"result\":1}\n{\"jsonrpc\":\"2.0\",\"id\":1,");
            p.out(b"\"result\":{\"ok\":true}}\n");
            assert_eq!(answer.await, Ok(json!({ "ok": true })));
            let (id2, second) = p.client.request("x", Map::new(), 5_000);
            assert_eq!(id2, 2);
            p.out(b"{\"jsonrpc\":\"2.0\",\"id\":2}\n");
            assert_eq!(second.await, Ok(Value::Null));
            p.client.notify("notifications/initialized", None);
            assert_eq!(p.written().last().unwrap(), "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n");
            p.client.notify("notifications/cancelled", Some(obj(json!({ "requestId": 2, "reason": "cancelled" }))));
            assert_eq!(p.written().last().unwrap(), "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":2,\"reason\":\"cancelled\"}}\n");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_client_answers_a_server_ping_and_refuses_other_server_requests() {
    // spec rule 8.
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let (_id, answer) = p.client.request("tools/call", Map::new(), 5_000);
            p.out(b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"sampling/createMessage\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n");
            assert_eq!(
                p.written()[1..].to_vec(),
                vec![
                    "{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{}}\n".to_string(),
                    "{\"jsonrpc\":\"2.0\",\"id\":8,\"error\":{\"code\":-32601,\"message\":\"not offered\"}}\n".to_string(),
                ]
            );
            p.out(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"error\":{\"code\":-32000,\"message\":\"bad\"}}\n");
            assert_eq!(answer.await, Err("the server answered with error -32000: bad".to_string()));
        })
        .await;
    assert_eq!(rpc_failed(&json!({ "message": 3 })), "the server answered with error ?");
    assert_eq!(rpc_failed(&json!({ "code": 5 })), "the server answered with error 5");
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_client_fails_waiting_requests_once_on_exit_and_kills_the_pipe() {
    // spec rule 8: finish runs once.
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let (_a, first) = p.client.request("a", Map::new(), 5_000);
            let (_b, second) = p.client.request("b", Map::new(), 5_000);
            p.exit("the server exited with 1");
            p.exit("the server exited with 2");
            assert_eq!(first.await, Err("the server exited with 1".to_string()));
            assert_eq!(second.await, Err("the server exited with 1".to_string()));
            assert_eq!((p.pipe.kills.get(), p.closes.get(), p.client.closed()), (1, 1, true));
            let before = p.written().len();
            let (_c, late) = p.client.request("c", Map::new(), 5_000);
            assert_eq!(late.await, Err("the server's connection is closed".to_string()));
            p.client.notify("n", None);
            assert_eq!(p.written().len(), before);
            p.client.close();
            assert_eq!((p.pipe.kills.get(), p.closes.get()), (1, 1));
        })
        .await;
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let (_a, waiting) = p.client.request("a", Map::new(), 5_000);
            p.client.close();
            assert_eq!(waiting.await, Err("the connection was closed".to_string()));
            assert_eq!((p.pipe.kills.get(), p.closes.get()), (1, 1));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_client_times_out_a_request_and_forgets_its_id() {
    // spec rule 8: round(1500 / 1000) is 2, as Math.round; a late answer is skipped.
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let (_id, answer) = p.client.request("slow", Map::new(), 1_500);
            assert_eq!(answer.await, Err("the server did not answer slow within 2 s".to_string()));
            p.out(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":1}\n");
            assert!(!p.client.closed());
            let (id, next) = p.client.request("next", Map::new(), 5_000);
            assert_eq!(id, 2);
            p.out(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":\"ok\"}\n");
            assert_eq!(next.await, Ok(json!("ok")));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_client_fails_a_too_deep_answer_with_the_depth_words() {
    // spec rule 8: a line too deep for a parser with a recursion limit still fails its request with the TS's words
    // at once, never by the deadline. Also the real flood fixture's answers.
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let (_id, answer) = p.client.request("tools/call", Map::new(), 60_000);
            let line =
                format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"content\":[],\"structuredContent\":{}}}}}\n", deep_text(100_000));
            p.out(line.as_bytes());
            let got = tokio::time::timeout(std::time::Duration::from_secs(5), answer).await.expect("answered before the deadline");
            assert_eq!(got, Err("the server's answer is nested over 64 deep".to_string()));
            let (_w, wide) = p.client.request("tools/call", Map::new(), 60_000);
            let zeros = vec!["0"; 200_000].join(",");
            p.out(format!("{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"content\":[],\"structuredContent\":[{zeros}]}}}}\n").as_bytes());
            assert_eq!(wide.await, Err("the server's answer holds over 100000 entries".to_string()));
            assert!(!p.client.closed());
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_client_cuts_an_endless_line_past_2_mb() {
    // spec rule 8: a remainder past 2 MiB with no newline closes the client with TOO_BIG; a line of exactly 2 MiB is read.
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let head = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"";
            let tail = "\"}";
            let fill = MESSAGE_MAX_BYTES - head.len() - tail.len();
            let (_id, exact) = p.client.request("big", Map::new(), 60_000);
            p.out(format!("{head}{}{tail}\n", "x".repeat(fill)).as_bytes());
            assert_eq!(exact.await, Ok(json!("x".repeat(fill))));
            let (_id2, endless) = p.client.request("endless", Map::new(), 60_000);
            let chunk = vec![b'x'; 64 * 1024];
            for _ in 0..48 {
                p.out(&chunk);
            }
            assert_eq!(endless.await, Err(TOO_BIG.to_string()));
            assert_eq!((p.client.closed(), p.pipe.kills.get(), p.closes.get()), (true, 1, 1));
        })
        .await;
    LocalSet::new()
        .run_until(async {
            let p = piped();
            let (_id, over) = p.client.request("over", Map::new(), 60_000);
            p.out(format!("{}\n", "y".repeat(MESSAGE_MAX_BYTES + 1)).as_bytes());
            assert_eq!(over.await, Err(TOO_BIG.to_string()));
            assert!(p.client.closed());
        })
        .await;
    assert_eq!(TOO_BIG, "the server sent a message over 2 MB");
}

#[tokio::test(flavor = "current_thread")]
async fn stdio_client_keeps_the_last_4000_characters_of_stderr() {
    // spec rule 8.
    LocalSet::new()
        .run_until(async {
            let p = piped();
            p.err(&"a".repeat(3_000));
            p.err(&"b".repeat(3_000));
            assert_eq!(p.client.said(), format!("{}{}", "a".repeat(1_000), "b".repeat(3_000)));
            assert_eq!(p.client.pid(), None);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn spawn_here_runs_in_its_own_group_with_only_the_spec_environment() {
    // spec rule 9: cwd, the spec's env overlaid by the transport's and nothing of the host's, its own group, a
    // SIGKILL to the group, and the exit's words.
    LocalSet::new()
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let cwd = std::fs::canonicalize(dir.path()).unwrap().to_string_lossy().to_string();
            let script = "printf '%s|%s|%s|%s|' \"$FOO\" \"$BAR\" \"${HOME-unset}\" \"$(pwd)\"; ps -o pgid= -p $$ | tr -d ' '; sleep 30";
            let mut tenv = IndexMap::new();
            tenv.insert("BAR".to_string(), "from the transport".to_string());
            tenv.insert("FOO".to_string(), "transport wins".to_string());
            let t = McpTransport::Stdio {
                command: "bash".into(),
                args: vec!["-c".into(), script.into()],
                env: tenv,
                cwd: None,
                tool_timeout_sec: None,
            };
            let mut senv = IndexMap::new();
            senv.insert("FOO".to_string(), "from the spec".to_string());
            senv.insert("PATH".to_string(), std::env::var("PATH").unwrap());
            let spec =
                McpServerSpec { transport: t.clone(), cwd: cwd.clone(), env: senv, secrets: vec![], open: None, post: None, sign_in: None };
            let out = Rc::new(RefCell::new(Vec::<u8>::new()));
            let exits = Rc::new(RefCell::new(Vec::<String>::new()));
            let (o, x) = (out.clone(), exits.clone());
            let events = McpPipeEvents {
                out: Rc::new(move |b: &[u8]| o.borrow_mut().extend_from_slice(b)),
                err: Rc::new(|_: &str| {}),
                exit: Rc::new(move |w: &str| x.borrow_mut().push(w.to_string())),
            };
            let pipe = (spawn_here())(&t, &spec, events);
            let pid = pipe.pid().expect("a pid here");
            for _ in 0..100 {
                if String::from_utf8_lossy(&out.borrow()).ends_with('\n') {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            assert_eq!(
                String::from_utf8_lossy(&out.borrow()).to_string(),
                format!("transport wins|from the transport|unset|{cwd}|{pid}\n")
            );
            pipe.kill();
            for _ in 0..100 {
                if !exits.borrow().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            assert_eq!(*exits.borrow(), vec!["the server exited on SIGKILL".to_string()]);
            assert_eq!(unsafe { libc::kill(-(pid as i32), 0) }, -1, "the group is gone");
            pipe.kill();

            let gone = McpTransport::Stdio {
                command: "/nonexistent/wsp-stub".into(),
                args: vec![],
                env: IndexMap::new(),
                cwd: None,
                tool_timeout_sec: None,
            };
            let said = Rc::new(RefCell::new(Vec::<String>::new()));
            let s = said.clone();
            let events = McpPipeEvents {
                out: Rc::new(|_: &[u8]| {}),
                err: Rc::new(|_: &str| {}),
                exit: Rc::new(move |w: &str| s.borrow_mut().push(w.to_string())),
            };
            let _dead = (spawn_here())(&gone, &spec, events);
            for _ in 0..40 {
                if !said.borrow().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            assert_eq!(said.borrow().len(), 1);
            assert!(said.borrow()[0].starts_with("/nonexistent/wsp-stub could not be started: "), "{:?}", said.borrow());
        })
        .await;
}

// ---- the http client against a server in the test's hands ----

#[derive(Clone, Debug)]
struct Seen {
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

type Route = Arc<dyn Fn(&Seen) -> Vec<u8> + Send + Sync>;

/// An HTTP/1.1 server on a thread of its own: each request read whole, recorded, answered by `route`, then closed.
fn serve(route: Route) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
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
            let head = String::from_utf8_lossy(&buf).to_string();
            let mut lines = head.split("\r\n");
            let path = lines.next().unwrap_or("").split(' ').nth(1).unwrap_or("").to_string();
            let mut headers = HashMap::new();
            for l in lines {
                if let Some((k, v)) = l.split_once(':') {
                    headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
                }
            }
            let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
            let mut body = vec![0u8; len];
            let _ = s.read_exact(&mut body);
            let req = Seen { path, headers, body: String::from_utf8_lossy(&body).to_string() };
            log.lock().unwrap().push(req.clone());
            let _ = s.write_all(&route(&req));
        }
    });
    (format!("http://127.0.0.1:{port}"), seen)
}

fn answer(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n", body.len());
    for (k, v) in headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn id_of(req: &Seen) -> Value {
    serde_json::from_str::<Value>(&req.body).ok().and_then(|v| v.get("id").cloned()).unwrap_or(Value::Null)
}

fn http_spec(url: &str, headers: &[(&str, &str)]) -> (McpTransport, McpServerSpec) {
    let t = McpTransport::Http { url: url.to_string(), headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() };
    let spec = McpServerSpec {
        transport: t.clone(),
        cwd: "/".into(),
        env: IndexMap::new(),
        secrets: vec![],
        open: None,
        post: None,
        sign_in: None,
    };
    (t, spec)
}

#[tokio::test(flavor = "current_thread")]
async fn http_client_sends_the_headers_and_keeps_the_session() {
    // spec rule 10.
    LocalSet::new()
        .run_until(async {
            let route: Route = Arc::new(|req: &Seen| {
                let body = json!({ "jsonrpc": "2.0", "id": id_of(req), "result": { "path": req.path } }).to_string();
                answer(200, &[("content-type", "application/json"), ("mcp-session-id", "s-1")], body.as_bytes())
            });
            let (base, seen) = serve(route);
            let (t, spec) = http_spec(&format!("{base}/mcp"), &[("X-Key", "k1")]);
            let client = http_client(&t, &spec, Box::new(|| {}));
            let (id, first) = client.request("initialize", obj(json!({ "a": 1 })), 5_000);
            assert_eq!(id, 1);
            assert_eq!(first.await, Ok(json!({ "path": "/mcp" })));
            let (_id, second) = client.request("tools/list", Map::new(), 5_000);
            assert_eq!(second.await, Ok(json!({ "path": "/mcp" })));
            let seen = seen.lock().unwrap().clone();
            assert_eq!(seen.len(), 2);
            for req in &seen {
                assert_eq!(req.headers.get("x-key").map(String::as_str), Some("k1"));
                assert_eq!(req.headers.get("content-type").map(String::as_str), Some("application/json"));
                assert_eq!(req.headers.get("accept").map(String::as_str), Some("application/json, text/event-stream"));
                assert_eq!(req.headers.get("mcp-protocol-version").map(String::as_str), Some("2025-06-18"));
            }
            assert_eq!(seen[0].headers.get("mcp-session-id"), None);
            assert_eq!(seen[1].headers.get("mcp-session-id").map(String::as_str), Some("s-1"));
            assert_eq!(
                serde_json::from_str::<Value>(&seen[0].body).unwrap(),
                json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "a": 1 } })
            );
            assert_eq!(seen[0].body, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"a\":1}}");
            assert_eq!(client.said(), "");
            assert_eq!(client.pid(), None);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn http_client_reads_an_event_stream_answer() {
    // spec rule 10: the data lines, the first with our id answers.
    LocalSet::new()
        .run_until(async {
            let route: Route = Arc::new(|req: &Seen| {
                let other = json!({ "jsonrpc": "2.0", "id": 99, "result": "not ours" });
                let ours = json!({ "jsonrpc": "2.0", "id": id_of(req), "result": { "zones": ["acme.dev"] } });
                let body = format!("event: message\ndata: {other}\n\ndata:   {ours}  \n\n");
                answer(200, &[("content-type", "text/event-stream")], body.as_bytes())
            });
            let (base, _seen) = serve(route);
            let (t, spec) = http_spec(&format!("{base}/mcp"), &[]);
            let client = http_client(&t, &spec, Box::new(|| {}));
            let (_id, got) = client.request("tools/call", Map::new(), 5_000);
            assert_eq!(got.await, Ok(json!({ "zones": ["acme.dev"] })));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn http_client_refuses_a_sign_in_status_and_a_failed_status() {
    // spec rule 10.
    LocalSet::new()
        .run_until(async {
            let route: Route = Arc::new(|req: &Seen| match req.path.as_str() {
                "/401" => answer(401, &[], b""),
                "/403" => answer(403, &[], b""),
                "/500" => answer(500, &[], b"boom"),
                _ => answer(200, &[("content-type", "application/json")], b"nope"),
            });
            let (base, _seen) = serve(route);
            for (path, words) in [
                ("/401", "its address wants a sign-in (401); sign in where the agent keeps it"),
                ("/403", "its address wants a sign-in (403); sign in where the agent keeps it"),
                ("/500", "its address answered tools/list with 500"),
                ("/junk", "its answer to tools/list could not be read"),
            ] {
                let (t, spec) = http_spec(&format!("{base}{path}"), &[]);
                let client = http_client(&t, &spec, Box::new(|| {}));
                let (_id, got) = client.request("tools/list", Map::new(), 5_000);
                assert_eq!(got.await, Err(words.to_string()), "{path}");
            }
            let (t, spec) = http_spec(&format!("{base}/x"), &[]);
            let closes = Rc::new(Cell::new(0));
            let c = closes.clone();
            let client = http_client(&t, &spec, Box::new(move || c.set(c.get() + 1)));
            client.close();
            client.close();
            assert_eq!((closes.get(), client.closed()), (1, true));
            let (_id, late) = client.request("x", Map::new(), 5_000);
            assert_eq!(late.await, Err("the server's connection is closed".to_string()));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn http_client_cuts_a_body_over_2_mb() {
    // spec rule 11, TS slate-mcp.test.ts:206-249 (the http half): a 3 MiB answer is cut, declared or counted.
    LocalSet::new()
        .run_until(async {
            let route: Route = Arc::new(|req: &Seen| {
                let body = format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}}}",
                    id_of(req),
                    "x".repeat(3 * 1024 * 1024)
                );
                if req.path == "/chunked" {
                    let mut out =
                        b"HTTP/1.1 200 X\r\ncontent-type: application/json\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n"
                            .to_vec();
                    for part in body.as_bytes().chunks(64 * 1024) {
                        out.extend_from_slice(format!("{:x}\r\n", part.len()).as_bytes());
                        out.extend_from_slice(part);
                        out.extend_from_slice(b"\r\n");
                    }
                    out.extend_from_slice(b"0\r\n\r\n");
                    out
                } else {
                    answer(200, &[("content-type", "application/json")], body.as_bytes())
                }
            });
            let (base, _seen) = serve(route);
            for path in ["/declared", "/chunked"] {
                let (t, spec) = http_spec(&format!("{base}{path}"), &[]);
                let client = http_client(&t, &spec, Box::new(|| {}));
                let (_id, got) = client.request("tools/call", Map::new(), 10_000);
                assert_eq!(got.await, Err(TOO_BIG.to_string()), "{path}");
            }
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn fetch_here_follows_no_redirect() {
    // spec rule 11: a redirect fails the message, its target is never asked; a failed status's body is not read.
    LocalSet::new()
        .run_until(async {
            let route: Route = Arc::new(|req: &Seen| match req.path.as_str() {
                "/moved" => answer(302, &[("location", "/elsewhere")], b""),
                "/denied" => answer(403, &[("content-type", "text/plain"), ("mcp-session-id", "s-9")], b"secret body"),
                _ => answer(200, &[], b"{}"),
            });
            let (base, seen) = serve(route);
            let post = fetch_here();
            let (t, _spec) = http_spec(&format!("{base}/moved"), &[]);
            let req = McpHttpRequest { headers: IndexMap::new(), body: "{}".into(), ms: 5_000 };
            assert!(post.send(&t, req.clone()).await.is_err());
            assert_eq!(seen.lock().unwrap().iter().map(|s| s.path.clone()).collect::<Vec<_>>(), vec!["/moved".to_string()]);
            let (denied, _spec) = http_spec(&format!("{base}/denied"), &[]);
            assert_eq!(
                post.send(&denied, req).await,
                Ok(McpHttpAnswer {
                    status: 403,
                    content_type: Some("text/plain".into()),
                    session_id: Some("s-9".into()),
                    body: String::new()
                })
            );
        })
        .await;
}

// ---- the schema ----

#[test]
fn fold_schema_folds_one_line_per_property() {
    // TS slate-mcp.test.ts:119-133.
    assert_eq!(
        fold_schema(
            &json!({ "properties": { "params": { "type": "object", "required": ["account"], "properties": { "account": { "type": "string", "description": "The alias. More words." }, "limit": { "type": "integer", "maximum": 50 } } }, "tags": { "type": "array", "items": { "type": "string" } }, "mode": { "enum": ["a", "b"] } }, "required": ["params"] })
        ),
        vec![
            "  params: object, required",
            "  params.account: string, required. The alias.",
            "  params.limit: integer, max 50",
            "  tags: string[]",
            "  mode: \"a\" | \"b\""
        ]
    );
    assert_eq!(
        fold_schema(
            &json!({ "$defs": { "In": { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" }, "folder": { "anyOf": [{ "type": "string" }, { "type": "null" }], "default": null, "description": "Folder name, e.g. 'Inbox'." } } } }, "properties": { "params": { "$ref": "#/$defs/In" } }, "required": ["params"] })
        ),
        vec![
            "  params: object, required",
            "  params.id: string, required",
            "  params.folder: string | null, default null. Folder name, e.g. 'Inbox'."
        ]
    );
}

#[test]
fn fold_schema_stops_three_deep_and_reads_types() {
    // spec rule 12, printed by the TS.
    assert_eq!(
        fold_schema(&json!({ "properties": {
            "a": { "type": "object", "properties": { "b": { "type": "object", "properties": { "c": { "type": "object", "properties": { "d": { "type": "string" } } } } } } },
            "xs": { "type": "array", "items": { "type": "object", "properties": { "y": { "type": ["string", "null"], "format": "date", "maxLength": 9, "minimum": 0 } } } },
            "k": { "const": 3 },
            "o": { "oneOf": [{ "type": "string" }, { "type": "array" }] },
            "n": {},
            "bad": 7
        } })),
        vec![
            "  a: object",
            "  a.b: object",
            "  a.b.c: object",
            "  xs: object[]",
            "  xs[].y: string | null, min 0, max length 9, format date",
            "  k: 3",
            "  o: string | array",
            "  n: any"
        ]
    );
    assert_eq!(
        fold_schema(&json!({ "properties": { "a": { "allOf": [{ "type": "integer" }], "description": "Count." } } })),
        vec!["  a: integer. Count."]
    );
}

#[test]
fn fold_schema_cuts_descriptions_to_a_sentence() {
    // spec rule 12: the sentence rule, printed by the TS.
    let long = "x".repeat(200);
    assert_eq!(
        fold_schema(&json!({ "properties": {
            "a": { "type": "string", "description": "  First   one!  Second." },
            "b": { "description": "e.g. this. Not cut" },
            "c": { "description": long },
            "d": { "description": "" },
            "e": { "description": "Ends (with) brackets? (Yes) more" }
        } })),
        vec![
            "  a: string. First one!".to_string(),
            "  b: any. e.g. this.".to_string(),
            format!("  c: any. {}…", "x".repeat(159)),
            "  d: any".to_string(),
            "  e: any. Ends (with) brackets?".to_string(),
        ]
    );
    assert_eq!(sentence(Some(&json!("One. Two.")), 160).as_deref(), Some("One."));
    assert_eq!(sentence(Some(&json!(3)), 160), None);
    assert_eq!(sentence(None, 160), None);
    assert_eq!(sentence(Some(&json!("abcdef")), 4).as_deref(), Some("abc…"));
}

#[test]
fn fold_schema_survives_a_ref_loop() {
    // spec rule 12: eight steps, then untyped.
    assert_eq!(
        fold_schema(&json!({ "$defs": { "A": { "$ref": "#/$defs/A" } }, "properties": { "a": { "$ref": "#/$defs/A" } } })),
        vec!["  a: any"]
    );
}

#[test]
fn fold_schema_of_nothing_is_empty() {
    // spec rule 12.
    for schema in [Value::Null, json!({}), json!({ "properties": 3 }), json!("x")] {
        assert_eq!(fold_schema(&schema), Vec::<String>::new(), "{schema}");
    }
}

#[test]
fn hints_say_returns_data_or_text_only_and_each_true_annotation() {
    // spec rule 12.
    let tool = |v: Value| -> McpToolInfo { serde_json::from_value(v).unwrap() };
    assert_eq!(hints_of(&tool(json!({ "name": "a" })), None), vec!["text only"]);
    assert_eq!(hints_of(&tool(json!({ "name": "a", "outputSchema": {} })), None), vec!["returns data"]);
    assert_eq!(hints_of(&tool(json!({ "name": "a" })), Some(true)), vec!["returns data"]);
    assert_eq!(hints_of(&tool(json!({ "name": "a" })), Some(false)), vec!["text only"]);
    assert_eq!(
        hints_of(
            &tool(
                json!({ "name": "a", "annotations": { "readOnlyHint": true, "destructiveHint": true, "idempotentHint": true, "openWorldHint": true } })
            ),
            None
        ),
        vec!["text only", "read-only", "destructive", "idempotent", "reaches outside"]
    );
    assert_eq!(
        hints_of(&tool(json!({ "name": "a", "annotations": { "readOnlyHint": "true", "destructiveHint": false } })), None),
        vec!["text only"]
    );
    assert_eq!(asks_line(ToolAsks::Never), "This thread's agent runs every tool without asking, and so do its slate's runs.");
    assert_eq!(asks_line(ToolAsks::Every), "This thread's agent asks before every tool, so the person allows each run's code once, on the slate: new arguments, a new then or a new tool ask again.");
    assert_eq!(
        asks_line(ToolAsks::UnlessReadOnly),
        "This thread's agent asks before a tool not marked read-only, so the person allows each such run's code once, on the slate: new arguments, a new then or a new tool ask again."
    );
}
