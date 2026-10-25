// SPDX-License-Identifier: AGPL-3.0-only
//! The token beside the state file leaves only for the owner's own host: a listener on the lock's port that serves no
//! wsp page, or a wsp page carrying another token's digest, is refused and never sent it, and a host started for the
//! test, whose page carries the digest of its token, is attached.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// A listener on loopback that answers a plain request with `page` and upgrades any socket, taking whatever token it
/// is handed; every frame it was sent.
async fn squatter(page: String) -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let heard = Arc::new(Mutex::new(Vec::new()));
    let noted = heard.clone();
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let (page, noted) = (page.clone(), noted.clone());
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
                    noted.lock().unwrap().push(text.to_string());
                    let asked: Value = serde_json::from_str(&text).unwrap();
                    if ws.send(Message::text(json!({ "id": asked["id"], "ok": true }).to_string())).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    (port, heard)
}

fn locked(dir: &Path, port: u16) -> PathBuf {
    let lock = json!({ "pid": std::process::id(), "port": port, "address": "127.0.0.1", "startedAt": "2026-10-19T00:00:00.000Z" });
    std::fs::write(dir.join("host.lock"), lock.to_string()).unwrap();
    std::fs::write(dir.join("host-token"), format!("{TOKEN}\n")).unwrap();
    dir.join("state.json")
}

fn env_in(dir: &Path) -> Env {
    [("HOME".to_owned(), dir.to_string_lossy().into_owned())].into_iter().collect()
}

fn wont_attach(dir: &Path, port: u16, why: &str) -> String {
    format!(
        "a host (pid {}) holds {} on port {port} but {why}: stop that process or run wsp down, then open wsp again",
        std::process::id(),
        dir.join("host.lock").display()
    )
}

/// Catches: the token sent to whatever answers on the lock's port, as a live pid alone let it.
#[tokio::test]
async fn a_port_serving_no_wsp_page_never_reads_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let (port, heard) = squatter("<html><body>a dev server</body></html>".to_owned()).await;
    let state = locked(dir.path(), port);
    let Err(refused) = HostClient::connect(&state, &env_in(dir.path())).await else { panic!("attached to a page no wsp host serves") };
    assert_eq!(refused.message, wont_attach(dir.path(), port, "no wsp host answers there"));
    assert_eq!(refused.kind.as_deref(), Some(wsp_mcp::LOCK_REFUSED));
    let heard = heard.lock().unwrap();
    assert!(heard.iter().all(|frame| !frame.contains(TOKEN)), "the listener read the token: {heard:?}");
}

/// Catches: the token sent to a wsp page that carries another token's digest.
#[tokio::test]
async fn a_page_carrying_another_tokens_digest_never_reads_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let boot = json!({ "tokenHash": sha256_hex("another-token"), "wsPath": "/ws", "paired": true, "version": HostClient::app_release() });
    let (port, heard) = squatter(format!("<script>window.__WSP__ = {boot};</script>")).await;
    let state = locked(dir.path(), port);
    let Err(refused) = HostClient::connect(&state, &env_in(dir.path())).await else { panic!("attached to another token's page") };
    assert_eq!(
        refused.message,
        wont_attach(dir.path(), port, "the page it serves carries another token's digest than the file beside this state")
    );
    let heard = heard.lock().unwrap();
    assert!(heard.iter().all(|frame| !frame.contains(TOKEN)), "the listener read the token: {heard:?}");
}

/// Catches: the checks refusing the owner's own host, whose page carries the digest of the token file it wrote.
#[tokio::test]
async fn a_host_started_for_the_test_is_attached() {
    let host = common::Throwaway::start();
    HostClient::connect(&host.state(), &host.env()).await.unwrap_or_else(|f| panic!("{}", f.message));
}
