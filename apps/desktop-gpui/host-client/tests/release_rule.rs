// SPDX-License-Identifier: AGPL-3.0-only
//! The desktop app attaches to a host of its own release or a newer one and says which release that is, and refuses an
//! older one in the words the command line says to it: against a host started for this test, and against a fake host
//! here that answers auth naming another release.

mod common;

use std::path::{Path, PathBuf};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use wsp_host_client::{Env, HostClient};

const TOKEN: &str = "the-owners-token";

fn sha256_hex(text: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, text.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// A host on loopback whose page carries the digest of the token and whose auth answer names `release`.
async fn fake_host(release: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let boot = json!({ "tokenHash": sha256_hex(TOKEN), "wsPath": "/ws", "paired": true, "version": release });
    let page = format!("<script>window.__WSP__ = {boot};</script>");
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let page = page.clone();
            tokio::spawn(async move {
                let mut head = [0u8; 2048];
                let read = tcp.peek(&mut head).await.unwrap_or(0);
                if !String::from_utf8_lossy(&head[..read]).to_ascii_lowercase().contains("upgrade: websocket") {
                    let _ = tcp.read(&mut head).await;
                    let answer = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}", page.len());
                    let _ = tcp.write_all(answer.as_bytes()).await;
                    return;
                }
                let Ok(mut ws) = tokio_tungstenite::accept_async(tcp).await else { return };
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    let asked: Value = serde_json::from_str(&text).unwrap();
                    let mut answer = json!({ "id": asked["id"], "ok": true });
                    if asked["op"] == "auth" {
                        answer["version"] = json!(release);
                        answer["road"] = json!("service");
                    }
                    if ws.send(Message::text(answer.to_string())).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

/// A state folder whose lock names this process on `port`, with the token beside it.
fn locked(dir: &Path, port: u16) -> PathBuf {
    let lock = json!({ "pid": std::process::id(), "port": port, "address": "127.0.0.1", "startedAt": "2026-10-19T00:00:00.000Z" });
    std::fs::write(dir.join("host.lock"), lock.to_string()).unwrap();
    std::fs::write(dir.join("host-token"), format!("{TOKEN}\n")).unwrap();
    dir.join("state.json")
}

fn env_in(dir: &Path) -> Env {
    [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect()
}

/// Catches: the desktop app holding a real host to its own release by another road than the rule, or reading no
/// release off it.
#[tokio::test]
async fn attaches_to_a_host_started_for_the_test_and_reads_its_release() {
    let host = common::Throwaway::start();
    let client = HostClient::connect(&host.state(), &host.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
    let package: Value =
        serde_json::from_str(&std::fs::read_to_string(common::root().join("packages/host/package.json")).unwrap()).unwrap();
    assert_eq!(client.host_release().as_deref(), package["version"].as_str());
}

/// Catches: the desktop app refusing a newer host, as the line's own rule does, or not saying which release it runs.
#[tokio::test]
async fn attaches_to_a_newer_host_and_reads_its_release() {
    let dir = tempfile::tempdir().unwrap();
    let state = locked(dir.path(), fake_host("999.0.0").await);
    let client = HostClient::connect(&state, &env_in(dir.path())).await.unwrap_or_else(|f| panic!("{}", f.message));
    assert_eq!(client.host_release().as_deref(), Some("999.0.0"));
    assert_ne!(HostClient::app_release(), "999.0.0");
}

/// Catches: an older host attached, or refused in other words than the command line says to it.
#[tokio::test]
async fn refuses_an_older_host_in_the_lines_words() {
    let dir = tempfile::tempdir().unwrap();
    let state = locked(dir.path(), fake_host("0.0.1").await);
    let Err(refused) = HostClient::connect(&state, &env_in(dir.path())).await else { panic!("an older host was attached") };
    let want = format!(
        "this line runs wsp {} and the host serving {} runs wsp 0.0.1: restart that host with wsp restart.",
        HostClient::app_release(),
        state.display()
    );
    assert_eq!(refused.message, want);
}
