// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/codex-subagents.test.ts at 65f6b89c7, case for case. A Codex turn's subagents through the runtime: the
//! real Codex adapter over a scripted app server, its child threads written as session.subagent rows and listed under the
//! thread, the lead's reply held while a child runs, and one child stopped by itself. The server's lines take the shape of T3
//! Code's recorded multi-agent wire (codexMultiAgentWire.json, codex-cli 0.145.0, MIT): each child thread prints its own turn
//! on the lead's stdout under its own id. Each test names the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::FutureExt as _;
use serde_json::{json, Value};
use tokio::sync::Notify;
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::port::{ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, HarnessAdapter, SharedFuture, WriteOutcome};
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::SessionHandle;
use wsp_words::views::session::fold_threads;

const LEAD: &str = "019fcfd6-17bb-72f0-ae12-a1f2dee6e3e5";
const LEAD_TURN: &str = "019fcfd6-1806-7de1-8564-de69fd55bffb";
const CHILD: &str = "019fcfd6-2883-77e0-9013-4410ede70371";
const CHILD_TURN: &str = "019fcfd6-28bf-7e00-a873-4554526dc845";

/// One app server process: what it prints, what wsp wrote to it, and an exit on stdin EOF as the real one makes.
struct Server {
    queue: RefCell<VecDeque<String>>,
    wake: Notify,
    done: Cell<bool>,
    code: Cell<Option<i32>>,
    exit: Rc<Gate>,
    written: RefCell<Vec<Value>>,
}

impl Server {
    fn push(&self, lines: Vec<Value>) {
        self.queue.borrow_mut().extend(lines.iter().map(Value::to_string));
        self.wake.notify_waiters();
    }
    fn end(&self, code: Option<i32>) {
        if self.done.replace(true) {
            return;
        }
        self.code.set(code);
        self.exit.open();
        self.wake.notify_waiters();
    }
    fn wrote(&self, method: &str) -> bool {
        self.written.borrow().iter().any(|m| m["method"] == method)
    }
}

type OnWrite = Rc<dyn Fn(&Value, &Server)>;

struct Servers {
    servers: Rc<RefCell<Vec<Rc<Server>>>>,
    on_write: OnWrite,
}

struct ServerStream {
    server: Rc<Server>,
    on_write: OnWrite,
}

impl ExecStream for ServerStream {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        let server = self.server.clone();
        Box::pin(futures::stream::unfold(server, |server| async move {
            let waiting = server.clone();
            loop {
                let woken = waiting.wake.notified();
                tokio::pin!(woken);
                woken.as_mut().enable();
                let next = server.queue.borrow_mut().pop_front();
                if let Some(line) = next {
                    return Some((Ok(line), server));
                }
                if server.done.get() {
                    return None;
                }
                woken.await;
            }
        }))
    }
    fn teardown(&self) {
        self.server.end(Some(0));
    }
    fn kill(&self) {
        self.server.end(None);
    }
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        let (server, on_write) = (self.server.clone(), self.on_write.clone());
        async move {
            if server.done.get() {
                return Ok(WriteOutcome::Gone);
            }
            let message: Value = serde_json::from_str(&line).expect("a json line");
            server.written.borrow_mut().push(message.clone());
            on_write(&message, &server);
            Ok(WriteOutcome::Written)
        }
        .boxed_local()
    }
    fn close_input(&self) {
        let server = self.server.clone();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            server.end(Some(0));
        });
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        let server = self.server.clone();
        async move {
            server.exit.wait().await;
            server.code.get()
        }
        .boxed_local()
        .shared()
    }
}

impl ExecStreamFactory for Servers {
    fn launch(&self, _command: &str, o: ExecOptions) -> Arc<dyn ExecStream> {
        let server = Rc::new(Server {
            queue: RefCell::default(),
            wake: Notify::new(),
            done: Cell::new(false),
            code: Cell::new(None),
            exit: Gate::new(),
            written: RefCell::default(),
        });
        self.servers.borrow_mut().push(server.clone());
        let stream = Arc::new(ServerStream { server, on_write: self.on_write.clone() });
        for line in o.input.unwrap_or_default() {
            let _ = stream.write(line).now_or_never();
        }
        stream
    }
}

fn scripted_servers(on_write: impl Fn(&Value, &Server) + 'static) -> (Arc<dyn ExecStreamFactory>, Rc<RefCell<Vec<Rc<Server>>>>) {
    let servers: Rc<RefCell<Vec<Rc<Server>>>> = Rc::default();
    (Arc::new(Servers { servers: servers.clone(), on_write: Rc::new(on_write) }), servers)
}

fn turn_started(thread: &str, turn: &str) -> Value {
    json!({ "method": "turn/started", "params": { "threadId": thread, "turn": { "id": turn, "items": [], "itemsView": "notLoaded", "status": "inProgress", "error": null } } })
}

fn turn_completed(thread: &str, turn: &str, status: &str) -> Value {
    json!({ "method": "turn/completed", "params": { "threadId": thread, "turn": { "id": turn, "items": [], "itemsView": "notLoaded", "status": status, "error": null } } })
}

fn message(thread: &str, turn: &str, id: &str, text: &str) -> Value {
    json!({ "method": "item/completed", "params": { "item": { "type": "agentMessage", "id": id, "text": text }, "threadId": thread, "turnId": turn } })
}

fn spawned(child: &str, call: &str) -> Value {
    json!({
        "method": "item/completed",
        "params": { "item": { "type": "subAgentActivity", "id": call, "kind": "started", "agentThreadId": child, "agentPath": "/root/alpha" }, "threadId": LEAD, "turnId": LEAD_TURN, "completedAtMs": 1785898346687u64 },
    })
}

fn usage(thread: &str, turn: &str, last: i64) -> Value {
    json!({
        "method": "thread/tokenUsage/updated",
        "params": { "threadId": thread, "turnId": turn, "tokenUsage": { "total": { "inputTokens": last, "outputTokens": 0 }, "last": { "inputTokens": last, "outputTokens": 0 }, "modelContextWindow": 258_400 } },
    })
}

/// A lead that spawns one child each turn and ends its own turn without waiting on it.
fn codex_server() -> (Arc<dyn ExecStreamFactory>, Rc<RefCell<Vec<Rc<Server>>>>) {
    scripted_servers(|m, server| {
        let method = m["method"].as_str().unwrap_or_default();
        if method == "thread/start" || method == "thread/resume" {
            server.push(vec![
                json!({ "id": "wsp-initialize", "result": {} }),
                json!({ "id": "wsp-thread", "result": { "thread": { "id": LEAD, "historyMode": "paginated", "turns": [] }, "model": "gpt-5.6-luna", "cwd": "/root/work" } }),
            ]);
        }
        if method == "turn/start" {
            server.push(vec![
                turn_started(LEAD, LEAD_TURN),
                spawned(CHILD, "call_S2JP"),
                turn_started(CHILD, CHILD_TURN),
                usage(CHILD, CHILD_TURN, 20_756),
            ]);
        }
        if method == "turn/interrupt" {
            let params = &m["params"];
            server.push(vec![
                json!({ "id": m["id"], "result": {} }),
                turn_completed(
                    params["threadId"].as_str().unwrap_or_default(),
                    params["turnId"].as_str().unwrap_or_default(),
                    "interrupted",
                ),
            ]);
        }
    })
}

struct Begun {
    fx: Fx,
    ws: String,
    handle: Rc<dyn SessionHandle>,
    servers: Rc<RefCell<Vec<Rc<Server>>>>,
}

async fn begin() -> Begun {
    let (exec, servers) = codex_server();
    let adapter: HarnessAdapterFactory = Arc::new(move |_ctx| {
        let adapter = create_codex_adapter(CodexAdapterDeps::new(exec.clone(), "/root/.codex", "codex login"))
            .map_err(|e| wsp_words::exit::HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    });
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).adapter("codex", adapter).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let opts = StartOptions { prompt: "spawn one agent and end your turn".into(), harness: Some("codex".into()), ..Default::default() };
    let handle = fx.rt.sessions().start(&ws, opts, None).await.expect("a start");
    until(|| async { servers.borrow().last().is_some_and(|s| s.wrote("turn/start")) }).await;
    Begun { fx, ws, handle, servers }
}

async fn rows_of(b: &Begun) -> Vec<Value> {
    b.fx.rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list").iter().map(j).collect()
}

async fn subagent_rows(b: &Begun) -> Vec<Value> {
    b.fx.rt.sessions().history(&b.ws, None).await.expect("a history").iter().map(j).filter(|e| e["type"] == "session.subagent").collect()
}

// "a Codex turn's subagents through the runtime" > "writes the child's start, its thread's spend and its end as rows, holds the lead's reply until the child ends, and keeps the child under the thread"
#[test]
fn writes_the_childs_start_its_threads_spend_and_its_end_as_rows_holds_the_leads_reply_until_the_child_ends_and_keeps_the_child_under_the_thread(
) {
    drive(|| async {
        let b = begin().await;
        let spend = json!({ "tokens": { "input": 20_756, "cached": 0, "cacheWrite": 0, "output": 0, "reasoning": 0 } });
        until(|| async { rows_of(&b).await.first().is_some_and(|r| r["subagents"][0].get("usage").is_some()) }).await;
        let mut child = rows_of(&b).await[0]["subagents"].clone();
        assert!(child[0]["startedAt"].is_number());
        child[0].as_object_mut().expect("an object").remove("startedAt");
        assert_eq!(
            child,
            json!([{ "id": CHILD, "title": "alpha", "state": "running", "parentToolUseId": "call_S2JP", "depth": 1, "usage": spend }])
        );

        let server = b.servers.borrow()[0].clone();
        server.push(vec![message(LEAD, LEAD_TURN, "lead_m1", "spawned it"), turn_completed(LEAD, LEAD_TURN, "completed")]);
        sleep_ms(20).await;
        assert_eq!(rows_of(&b).await[0]["status"], "running");

        server.push(vec![message(CHILD, CHILD_TURN, "child_m1", "slept and done"), turn_completed(CHILD, CHILD_TURN, "completed")]);
        until(|| async { rows_of(&b).await[0]["status"] == "completed" }).await;
        let row = rows_of(&b).await.remove(0);
        let states = |v: &Value| {
            v["subagents"].as_array().expect("subagents").iter().map(|c| (c["id"].clone(), c["state"].clone())).collect::<Vec<_>>()
        };
        assert_eq!(states(&row), [(json!(CHILD), json!("done"))]);
        let list = b.fx.rt.sessions().list(Some(b.ws.clone()), None).await.expect("a list");
        let folded = j(&fold_threads(&list)[0]);
        assert_eq!(states(&folded), [(json!(CHILD), json!("done"))]);
        let done =
            b.fx.rt
                .sessions()
                .history(&b.ws, None)
                .await
                .expect("a history")
                .iter()
                .map(j)
                .find(|e| e["type"] == "session.done")
                .expect("a done");
        assert_eq!(done["result"]["status"], "completed");
        assert_eq!(done["result"]["text"], "spawned it");
        let said = subagent_rows(&b).await;
        let shape: Vec<(Value, Value, Value)> =
            said.iter().map(|e| (e["task"].clone(), e["state"].clone(), e.get("usage").cloned().unwrap_or(Value::Null))).collect();
        assert_eq!(
            shape,
            [
                (json!(CHILD), json!("running"), Value::Null),
                (json!(CHILD), json!("running"), spend.clone()),
                (json!(CHILD), json!("done"), spend.clone())
            ]
        );
        assert_eq!(said[2]["parentToolUseId"], "call_S2JP");
        assert_eq!(said[2]["summary"], "slept and done");
        assert_eq!(row["subagents"][0]["usage"], spend);
        b.fx.rt.close().await.expect("a close");
    });
}

// "a Codex turn's subagents through the runtime" > "stops one child by itself through the adapter's turn/interrupt on the child's thread, the lead running on"
#[test]
fn stops_one_child_by_itself_through_the_adapters_turn_interrupt_on_the_childs_thread_the_lead_running_on() {
    drive(|| async {
        let b = begin().await;
        until(|| async { rows_of(&b).await.first().is_some_and(|r| r["subagents"].as_array().is_some_and(|s| s.len() == 1)) }).await;
        let stopped = b.fx.rt.sessions().interrupt(&b.handle.id(), None, Some(CHILD.into())).await.expect("an interrupt");
        assert_eq!(j(&stopped), json!({ "outcome": "accepted" }));
        let server = b.servers.borrow()[0].clone();
        let asked: Vec<Value> =
            server.written.borrow().iter().filter(|m| m["method"] == "turn/interrupt").map(|m| m["params"].clone()).collect();
        assert_eq!(asked, [json!({ "threadId": CHILD, "turnId": CHILD_TURN })]);
        until(|| async { rows_of(&b).await[0]["subagents"][0]["state"] == "stopped" }).await;
        assert_eq!(rows_of(&b).await[0]["status"], "running");

        server.push(vec![message(LEAD, LEAD_TURN, "lead_m1", "the child was stopped"), turn_completed(LEAD, LEAD_TURN, "completed")]);
        until(|| async { rows_of(&b).await[0]["status"] == "completed" }).await;
        let shape: Vec<(Value, Value)> = subagent_rows(&b).await.iter().map(|e| (e["task"].clone(), e["state"].clone())).collect();
        assert_eq!(shape, [(json!(CHILD), json!("running")), (json!(CHILD), json!("running")), (json!(CHILD), json!("stopped"))]);
        b.fx.rt.close().await.expect("a close");
    });
}
