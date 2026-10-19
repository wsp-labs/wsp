// SPDX-License-Identifier: AGPL-3.0-only
//! The feed against a fake host on the lock's port whose `sessions.list` carries rows the record cannot read: the feed
//! reads them as sent, skips a row it cannot read at all, and stays on the socket.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::desktop::feed::{self, Fed, FeedState};
use futures::channel::mpsc;
use futures::StreamExt;
use futures_util::SinkExt;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use wsp_host_client::{Env, HostClient};

const TOKEN: &str = "the-owners-token";
const CHILD: &str = "DESKTOP_FEED_ROWS_CHILD";
const SKIPPED: [&str; 2] = [
    "desktop-gpui: menu bar: sessions.list answered row 2 with no id, skipped",
    "desktop-gpui: menu bar: sessions.list answered row 3 with no id, skipped",
];

fn sha256_hex(text: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, text.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// What the fake host was asked: sockets opened, auths, subscriptions.
#[derive(Default)]
struct Asked {
    sockets: AtomicUsize,
    auths: AtomicUsize,
    subscribes: AtomicUsize,
}

/// The answer to one op: a host of this app's release whose rows are a readable one, one with a `pr` and a status
/// no record reads, a number, and an object with no id.
fn answer(op: &str) -> Value {
    match op {
        "auth" => json!({ "version": HostClient::app_release(), "road": "service" }),
        "preferences.get" => json!({ "preferences": {} }),
        "sessions.list" => json!({ "sessions": [
            { "id": "s1", "workspaceId": "ws_mac", "harness": "claude", "status": "running", "threadId": "t1" },
            { "id": "s2", "workspaceId": "ws_mac", "harness": "claude", "status": "dreaming", "threadId": "t2", "pr": 7 },
            42,
            { "workspaceId": "ws_mac", "status": "running" },
        ] }),
        "workspaces.list" => json!({ "workspaces": [{ "id": "ws_mac", "kind": "local", "name": "repo" }] }),
        "places.list" => json!({ "places": [{ "id": "here", "name": "boat" }] }),
        _ => json!({}),
    }
}

/// A host on loopback whose page carries the digest of the token, answering each op by [`answer`].
async fn fake_host(asked: Arc<Asked>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let boot = json!({ "tokenHash": sha256_hex(TOKEN), "wsPath": "/ws", "paired": true, "version": HostClient::app_release() });
    let page = format!("<script>window.__WSP__ = {boot};</script>");
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let (page, asked) = (page.clone(), asked.clone());
            tokio::spawn(async move {
                let mut head = [0u8; 2048];
                let read = tcp.peek(&mut head).await.unwrap_or(0);
                if !String::from_utf8_lossy(&head[..read]).to_ascii_lowercase().contains("upgrade: websocket") {
                    let _ = tcp.read(&mut head).await;
                    let reply = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}", page.len());
                    let _ = tcp.write_all(reply.as_bytes()).await;
                    return;
                }
                let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else { return };
                asked.sockets.fetch_add(1, Ordering::SeqCst);
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    let op = frame["op"].as_str().unwrap_or_default();
                    match op {
                        "auth" => _ = asked.auths.fetch_add(1, Ordering::SeqCst),
                        "events.subscribe" => _ = asked.subscribes.fetch_add(1, Ordering::SeqCst),
                        _ => {}
                    }
                    let mut reply = answer(op);
                    reply["id"] = frame["id"].clone();
                    reply["ok"] = json!(true);
                    if ws.send(Message::text(reply.to_string())).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

/// A state folder whose lock names this process on `port`, with the token beside it.
fn locked(dir: &Path, port: u16) -> std::path::PathBuf {
    let lock = json!({ "pid": std::process::id(), "port": port, "address": "127.0.0.1", "startedAt": "2026-10-19T00:00:00.000Z" });
    std::fs::write(dir.join("host.lock"), lock.to_string()).unwrap();
    std::fs::write(dir.join("host-token"), format!("{TOKEN}\n")).unwrap();
    dir.join("state.json")
}

fn ids(rows: &[Value]) -> Vec<&str> {
    rows.iter().filter_map(|r| r["id"].as_str()).collect()
}

/// Catches: a row of a newer host the record cannot read failing the whole read, so the feed reads the host as lost
/// and dials it again every retry; and a row the feed cannot read at all kept, or skipped without a word. The skip
/// is read off this test run again as a child, since the test harness keeps a test's own stderr.
#[test]
fn a_row_the_record_cannot_read_is_read_as_sent_and_the_feed_stays() {
    if std::env::var_os(CHILD).is_none() {
        let home = tempfile::tempdir().unwrap();
        let run = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["a_row_the_record_cannot_read_is_read_as_sent_and_the_feed_stays", "--exact", "--nocapture", "--test-threads=1"])
            .env_clear()
            .env(CHILD, "1")
            .env("HOME", home.path())
            .output()
            .unwrap();
        let (out, err) = (String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
        assert!(run.status.success(), "the feed's run failed:\n{out}\n{err}");
        for line in SKIPPED {
            assert_eq!(err.lines().filter(|l| *l == line).count(), 1, "{line:?} logged once in:\n{err}");
        }
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let asked = Arc::new(Asked::default());
    let state = locked(dir.path(), runtime.block_on(fake_host(asked.clone())));
    let env: Env = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let (tx, mut rx) = mpsc::unbounded();
    let feed = feed::follow(None, state, env, &runtime, feed::SETTLE, Duration::from_millis(300), tx);

    let mut states: Vec<FeedState> = Vec::new();
    let until = Instant::now() + Duration::from_secs(3);
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        match runtime.block_on(async { tokio::time::timeout(left, rx.next()).await }) {
            Ok(Some(Fed::State(state))) => states.push(*state),
            Ok(Some(Fed::Event(_))) => {}
            Ok(None) => panic!("the feed ended"),
            Err(_) => break,
        }
    }
    let counts = [&asked.sockets, &asked.auths, &asked.subscribes].map(|n| n.load(Ordering::SeqCst));
    assert_eq!(counts, [1, 1, 1], "sockets, auths and subscriptions over 3 s");
    let read = states.iter().find(|s| !s.lost).expect("the feed read the host");
    assert_eq!((read.sessions.len(), ids(&read.sessions)), (2, vec!["s1", "s2"]), "the rows with no id are skipped, the others read");
    assert_eq!(read.sessions[1]["status"], "dreaming", "a row the record cannot read is read as sent");
    assert_eq!(ids(&read.workspaces), ["ws_mac"]);
    assert_eq!(read.label(), "boat");
    assert!(states.iter().all(|s| !s.lost), "the host never read as lost: {:?}", states.iter().map(|s| s.lost).collect::<Vec<_>>());
    drop(feed);
}
