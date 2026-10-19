// SPDX-License-Identifier: AGPL-3.0-only
//! Identical shared reads asked while one is in flight go as one request, as today's client sends them
//! (`SHARED_READS` and `request` in `protocol/client.ts`): against a fake host here that answers only when told.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use wsp_host_client::{Env, HostClient};

const TOKEN: &str = "the-owners-token";
/// Long enough for a second frame already sent to reach the fake host.
const QUIET: Duration = Duration::from_millis(300);

fn sha256_hex(text: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, text.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// A host on loopback that lets in any socket and answers its subscribe at once; every other frame it is sent comes out
/// of `asked`, and whatever goes into `say` is written to the socket.
struct Fake {
    port: u16,
    asked: mpsc::UnboundedReceiver<Value>,
    say: mpsc::UnboundedSender<Value>,
}

async fn fake_host() -> Fake {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let boot = json!({ "tokenHash": sha256_hex(TOKEN), "wsPath": "/ws", "paired": true, "version": "999.0.0" });
    let page = format!("<script>window.__WSP__ = {boot};</script>");
    let (heard, asked) = mpsc::unbounded_channel();
    let (say, mut said) = mpsc::unbounded_channel::<Value>();
    tokio::spawn(async move {
        loop {
            let Ok((mut tcp, _)) = listener.accept().await else { return };
            let mut head = [0u8; 2048];
            let read = tcp.peek(&mut head).await.unwrap_or(0);
            if !String::from_utf8_lossy(&head[..read]).to_ascii_lowercase().contains("upgrade: websocket") {
                let _ = tcp.read(&mut head).await;
                let answer = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}", page.len());
                let _ = tcp.write_all(answer.as_bytes()).await;
                continue;
            }
            let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else { return };
            loop {
                tokio::select! {
                    read = ws.next() => {
                        let Some(Ok(Message::Text(text))) = read else { return };
                        let asked: Value = serde_json::from_str(&text).unwrap();
                        let answer = match asked["op"].as_str() {
                            Some("auth") => json!({ "id": asked["id"], "ok": true, "version": "999.0.0", "road": "service" }),
                            Some("events.subscribe") => json!({ "id": asked["id"], "ok": true }),
                            _ => {
                                _ = heard.send(asked);
                                continue;
                            }
                        };
                        if ws.send(Message::text(answer.to_string())).await.is_err() {
                            return;
                        }
                    }
                    out = said.recv() => {
                        let Some(out) = out else { return };
                        if ws.send(Message::text(out.to_string())).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });
    Fake { port, asked, say }
}

impl Fake {
    async fn next_ask(&mut self) -> Value {
        tokio::time::timeout(Duration::from_secs(5), self.asked.recv()).await.expect("a frame reaches the host").unwrap()
    }

    async fn no_ask(&mut self) {
        if let Ok(Some(frame)) = tokio::time::timeout(QUIET, self.asked.recv()).await {
            panic!("the host was sent another frame: {frame}");
        }
    }

    fn answer(&self, asked: &Value, body: Value) {
        let mut reply = body;
        reply["id"] = asked["id"].clone();
        reply["ok"] = json!(true);
        self.say.send(reply).unwrap();
    }
}

fn locked(dir: &Path, port: u16) -> PathBuf {
    let lock = json!({ "pid": std::process::id(), "port": port, "address": "127.0.0.1", "startedAt": "2026-10-19T00:00:00.000Z" });
    std::fs::write(dir.join("host.lock"), lock.to_string()).unwrap();
    std::fs::write(dir.join("host-token"), format!("{TOKEN}\n")).unwrap();
    dir.join("state.json")
}

async fn connected(dir: &Path, fake: &Fake) -> Arc<HostClient> {
    let env: Env = [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect();
    Arc::new(HostClient::connect(&locked(dir, fake.port), &env).await.unwrap_or_else(|f| panic!("{}", f.message)))
}

fn ask(client: &Arc<HostClient>, op: &'static str, params: Value) -> tokio::task::JoinHandle<Result<Map<String, Value>, String>> {
    let client = client.clone();
    let Value::Object(params) = params else { panic!("params are an object") };
    tokio::spawn(async move { client.call_raw(op, params).await.map_err(|f| f.message) })
}

/// The answer an ask got, failing rather than waiting on one that never comes.
async fn answered(asked: tokio::task::JoinHandle<Result<Map<String, Value>, String>>) -> Result<Map<String, Value>, String> {
    tokio::time::timeout(Duration::from_secs(5), asked).await.expect("the ask settles").unwrap()
}

/// Catches: a second identical shared read sending its own frame while the first is out, or either ask not getting the
/// one answer.
#[tokio::test]
async fn two_identical_reads_while_one_is_out_send_one_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = fake_host().await;
    let client = connected(dir.path(), &fake).await;
    let first = ask(&client, "workspaces.list", json!({}));
    let asked = fake.next_ask().await;
    assert_eq!(asked["op"], "workspaces.list");
    let second = ask(&client, "workspaces.list", json!({}));
    fake.no_ask().await;
    fake.answer(&asked, json!({ "workspaces": ["w1"] }));
    for joined in [first, second] {
        assert_eq!(answered(joined).await.unwrap(), json!({ "workspaces": ["w1"] }).as_object().unwrap().clone());
    }
}

/// Catches: an ask after an event joining a read sent before the event, whose answer may be stale; or that ask going
/// out before the held read settled, so its answer could land first.
#[tokio::test]
async fn an_ask_after_an_event_does_not_join_a_read_sent_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = fake_host().await;
    let client = connected(dir.path(), &fake).await;
    let mut events = client.events().await.unwrap();
    let first = ask(&client, "workspaces.list", json!({}));
    let asked = fake.next_ask().await;
    fake.say.send(json!({ "type": "workspace.deleted", "workspaceId": "w1", "seq": 1 })).unwrap();
    let heard = tokio::time::timeout(Duration::from_secs(5), events.next()).await.unwrap();
    assert!(heard.is_some(), "the event reached this socket");
    let second = ask(&client, "workspaces.list", json!({}));
    fake.no_ask().await;
    fake.answer(&asked, json!({ "workspaces": ["w1"] }));
    assert_eq!(answered(first).await.unwrap()["workspaces"], json!(["w1"]));
    let again = fake.next_ask().await;
    assert_eq!(again["op"], "workspaces.list", "the later ask goes once the held read settled");
    fake.answer(&again, json!({ "workspaces": [] }));
    assert_eq!(answered(second).await.unwrap()["workspaces"], json!([]), "with its own answer");
}

/// Catches: a daemon channel's frame counted as an event, ending the sharing as an event does, which today's client
/// never counts (`#heard` skips them).
#[tokio::test]
async fn a_daemon_frame_since_does_not_end_the_sharing() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = fake_host().await;
    let client = connected(dir.path(), &fake).await;
    let mut events = client.events().await.unwrap();
    let first = ask(&client, "places.list", json!({}));
    let asked = fake.next_ask().await;
    fake.say.send(json!({ "type": "daemon.event", "channel": "c1", "event": { "type": "pty.data", "ptyId": "p", "data": "x" } })).unwrap();
    let heard = tokio::time::timeout(Duration::from_secs(5), events.next()).await.unwrap();
    assert!(heard.is_some(), "the frame reached this socket");
    let second = ask(&client, "places.list", json!({}));
    fake.no_ask().await;
    fake.answer(&asked, json!({ "places": [] }));
    for joined in [first, second] {
        assert_eq!(answered(joined).await.unwrap()["places"], json!([]));
    }
}

/// Catches: reads with different words, or two asks of an op off the list, sharing one request.
#[tokio::test]
async fn different_words_and_ops_off_the_list_do_not_join() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = fake_host().await;
    let client = connected(dir.path(), &fake).await;
    let plain = ask(&client, "workspaces.get", json!({ "workspaceId": "w1" }));
    let one = fake.next_ask().await;
    let other = ask(&client, "workspaces.get", json!({ "workspaceId": "w2" }));
    let two = fake.next_ask().await;
    assert_eq!((one["workspaceId"].as_str(), two["workspaceId"].as_str()), (Some("w1"), Some("w2")));
    let act = ask(&client, "sessions.list", json!({}));
    let act_again = ask(&client, "sessions.list", json!({}));
    let (three, four) = (fake.next_ask().await, fake.next_ask().await);
    assert_eq!((three["op"].as_str(), four["op"].as_str()), (Some("sessions.list"), Some("sessions.list")));
    for (asked, waiting) in [(one, plain), (two, other), (three, act), (four, act_again)] {
        fake.answer(&asked, json!({}));
        answered(waiting).await.unwrap();
    }
}

/// Catches: a refused read kept in flight, so the next ask is handed the old refusal instead of asking again.
#[tokio::test]
async fn a_failed_read_does_not_stick() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = fake_host().await;
    let client = connected(dir.path(), &fake).await;
    let first = ask(&client, "release.get", json!({}));
    let asked = fake.next_ask().await;
    fake.say.send(json!({ "id": asked["id"], "ok": false, "error": "not now" })).unwrap();
    assert_eq!(answered(first).await, Err("not now".to_owned()));
    let again = ask(&client, "release.get", json!({}));
    let asked = fake.next_ask().await;
    fake.answer(&asked, json!({ "release": "r" }));
    assert_eq!(answered(again).await.unwrap()["release"], json!("r"));
}

/// Catches: the read run inside the first asker, so letting that ask go ends the read the others share.
#[tokio::test]
async fn an_asker_let_go_of_leaves_the_read_to_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = fake_host().await;
    let client = connected(dir.path(), &fake).await;
    let first = ask(&client, "account.get", json!({}));
    let asked = fake.next_ask().await;
    let second = ask(&client, "account.get", json!({}));
    fake.no_ask().await;
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    fake.answer(&asked, json!({ "account": "a" }));
    assert_eq!(answered(second).await.unwrap()["account"], json!("a"));
}

/// Catches: a refusal handed to the first asker alone, the ask that joined it left with another error or none.
#[tokio::test]
async fn a_refusal_reaches_every_ask_that_joined() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = fake_host().await;
    let client = connected(dir.path(), &fake).await;
    let first = ask(&client, "devices.list", json!({}));
    let asked = fake.next_ask().await;
    let second = ask(&client, "devices.list", json!({}));
    fake.no_ask().await;
    fake.say.send(json!({ "id": asked["id"], "ok": false, "error": "not on this socket" })).unwrap();
    for joined in [first, second] {
        assert_eq!(answered(joined).await, Err("not on this socket".to_owned()));
    }
}
