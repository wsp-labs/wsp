// SPDX-License-Identifier: AGPL-3.0-only
//! ws-client.ts at 65f6b89c7: `WsClient`, the protocol client the TS tests dial a served runtime with (auth as the first
//! frame, then request and reply, every pushed event kept in `events`), and `createOverWire`.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use futures::channel::oneshot;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct WsClient {
    sink: tokio::sync::Mutex<SplitSink<Ws, Message>>,
    next_id: Cell<u64>,
    pending: Rc<RefCell<HashMap<u64, oneshot::Sender<Value>>>>,
    /// Every frame the host pushed: one with an `op`, or one with a `type` that answers no request.
    pub events: Rc<RefCell<Vec<Value>>>,
    closed: Rc<Cell<bool>>,
}

impl WsClient {
    /// `WsClient.connect(port, { token })`: dial, then authenticate as the first frame.
    pub async fn connect(port: u16, token: &str) -> Rc<WsClient> {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/")).await.expect("a socket");
        let (sink, mut stream) = ws.split();
        let pending: Rc<RefCell<HashMap<u64, oneshot::Sender<Value>>>> = Rc::default();
        let events: Rc<RefCell<Vec<Value>>> = Rc::default();
        let closed = Rc::new(Cell::new(false));
        let (p, e, c) = (pending.clone(), events.clone(), closed.clone());
        tokio::task::spawn_local(async move {
            while let Some(Ok(frame)) = stream.next().await {
                let Message::Text(text) = frame else { continue };
                let m: Value = serde_json::from_str(&text).expect("a frame is JSON");
                if m.get("op").is_some_and(Value::is_string) {
                    e.borrow_mut().push(m);
                    continue;
                }
                let waiting = m["id"].as_u64().and_then(|id| p.borrow_mut().remove(&id));
                match waiting {
                    Some(reply) => {
                        let _ = reply.send(m);
                    }
                    None if m.get("type").is_some_and(|t| !t.is_null()) => e.borrow_mut().push(m),
                    None => {}
                }
            }
            c.set(true);
        });
        let client = Rc::new(WsClient { sink: tokio::sync::Mutex::new(sink), next_id: Cell::new(1), pending, events, closed });
        let res = client.request("auth", json!({ "token": token })).await;
        assert_eq!(res["ok"], json!(true), "auth failed: {}", res["error"]);
        client
    }

    /// `request(op, params)`: the reply that carries this request's id.
    pub async fn request(&self, op: &str, params: Value) -> Value {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let (tx, rx) = oneshot::channel();
        self.pending.borrow_mut().insert(id, tx);
        let mut frame = params.as_object().cloned().unwrap_or_default();
        frame.insert("id".into(), json!(id));
        frame.insert("op".into(), json!(op));
        self.sink.lock().await.send(Message::Text(Value::Object(frame).to_string().into())).await.expect("the request is sent");
        rx.await.expect("the host answers")
    }

    /// `received(c, n)` of serve-replay.test.ts:47: the events once the client holds `n`.
    pub async fn received(&self, n: usize) -> Vec<Value> {
        while self.events.borrow().len() < n {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        self.events.borrow().clone()
    }

    pub async fn close(&self) {
        let _ = self.sink.lock().await.close().await;
    }

    /// `closed()`: settles once the host's end of the socket is gone.
    pub async fn closed(&self) {
        while !self.closed.get() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
}

thread_local! {
    static WIRED: Cell<u32> = const { Cell::new(0) };
}

/// `createOverWire(c, name, body)`: a project over the wire and a workspace of it.
pub async fn create_over_wire(c: &WsClient, name: &str, body: Value) -> Value {
    let n = WIRED.with(|w| {
        w.set(w.get() + 1);
        w.get()
    });
    let answer = c.request("projects.add", json!({ "source": format!("https://github.com/wsp/wire-{n}.git"), "on": "default" })).await;
    let mut ask = body.as_object().cloned().unwrap_or_default();
    ask.insert("name".into(), json!(name));
    if let Some(project) = answer["project"]["id"].as_str() {
        ask.insert("project".into(), json!(project));
    }
    c.request("workspaces.create", Value::Object(ask)).await
}
