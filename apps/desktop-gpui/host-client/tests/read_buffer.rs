// SPDX-License-Identifier: AGPL-3.0-only
//! What the socket to the host keeps once a long answer has been read. The websocket library reads each frame into one
//! buffer and hands the frame out of it as the message; a buffer no message shares any more is taken back whole, at
//! the size of the longest frame it read, for as long as the socket lives. A fake host answers one call with a 12 MB
//! frame and the calls after it with small ones, and the live heap, through a counting allocator, has to come back to
//! where it stood. The fake host writes its frames by hand, so no buffer of its own lands in the count. The only test
//! in its binary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};

use serde_json::{json, Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wsp_host_client::{Env, HostClient};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

// SAFETY: every call goes to the system allocator with the caller's layout; the count beside it allocates nothing.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size as isize - layout.size() as isize, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

const TOKEN: &str = "the-owners-token";
/// The long answer, as long as `sessions.list` reads on a host of a few thousand threads, under the 16 MB frame the
/// library takes.
const LONG: usize = 12 * 1024 * 1024;
/// Answers read after it, each about as long as a pushed row.
const AFTER: usize = 16;
const SMALL: usize = 1024;
/// What may stay on the heap once the long answer is gone.
const BOUND: isize = 2 * 1024 * 1024;

fn mb(bytes: isize) -> f64 {
    bytes as f64 / 1_048_576.
}

fn sha256_hex(text: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, text.as_bytes()).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// One frame from the client, unmasked: its opcode and its payload.
async fn read_frame(tcp: &mut TcpStream) -> Option<(u8, Vec<u8>)> {
    let mut head = [0u8; 2];
    tcp.read_exact(&mut head).await.ok()?;
    let len = match head[1] & 0x7f {
        126 => tcp.read_u16().await.ok()? as usize,
        127 => tcp.read_u64().await.ok()? as usize,
        n => n as usize,
    };
    let mut mask = [0u8; 4];
    if head[1] & 0x80 != 0 {
        tcp.read_exact(&mut mask).await.ok()?;
    }
    let mut payload = vec![0u8; len];
    tcp.read_exact(&mut payload).await.ok()?;
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
    Some((head[0] & 0x0f, payload))
}

/// One text frame from the host, which a server never masks.
async fn write_text(tcp: &mut TcpStream, text: &str) -> std::io::Result<()> {
    let mut head = vec![0x81u8];
    match text.len() {
        n if n < 126 => head.push(n as u8),
        n if n <= u16::MAX as usize => {
            head.push(126);
            head.extend((n as u16).to_be_bytes());
        }
        n => {
            head.push(127);
            head.extend((n as u64).to_be_bytes());
        }
    }
    tcp.write_all(&head).await?;
    tcp.write_all(text.as_bytes()).await
}

/// A host on loopback that answers `long` with a frame of `LONG` bytes and any other call with a small one.
async fn fake_host() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let boot = json!({ "tokenHash": sha256_hex(TOKEN), "wsPath": "/ws", "paired": true, "version": wsp_mcp::own_release() });
    let page = format!("<script>window.__WSP__ = {boot};</script>");
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let page = page.clone();
            tokio::spawn(async move {
                let mut head = vec![0u8; 4096];
                let mut read = 0;
                while !head[..read].windows(4).any(|w| w == b"\r\n\r\n") {
                    match tcp.read(&mut head[read..]).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => read += n,
                    }
                }
                let asked = String::from_utf8_lossy(&head[..read]).into_owned();
                let key =
                    asked.lines().find_map(|l| l.to_ascii_lowercase().starts_with("sec-websocket-key:").then(|| l[18..].trim().to_owned()));
                let Some(key) = key else {
                    let answer = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}", page.len());
                    let _ = tcp.write_all(answer.as_bytes()).await;
                    return;
                };
                let accept = tokio_tungstenite::tungstenite::handshake::derive_accept_key(key.as_bytes());
                let upgraded = format!(
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
                );
                if tcp.write_all(upgraded.as_bytes()).await.is_err() {
                    return;
                }
                while let Some((opcode, payload)) = read_frame(&mut tcp).await {
                    if opcode == 8 {
                        return;
                    }
                    if opcode != 1 {
                        continue;
                    }
                    let asked: Value = serde_json::from_slice(&payload).unwrap();
                    let answer = if asked["op"] == "long" {
                        format!(r#"{{"id":{},"ok":true,"blob":"{}"}}"#, asked["id"], "x".repeat(LONG))
                    } else {
                        json!({ "id": asked["id"], "ok": true, "said": "y".repeat(SMALL) }).to_string()
                    };
                    if write_text(&mut tcp, &answer).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

/// A state folder whose lock names this process on `port`, with the token beside it.
fn locked(dir: &Path, port: u16) -> PathBuf {
    let lock = json!({ "pid": std::process::id(), "port": port, "address": "127.0.0.1", "startedAt": "2026-10-24T00:00:00.000Z" });
    std::fs::write(dir.join("host.lock"), lock.to_string()).unwrap();
    std::fs::write(dir.join("host-token"), format!("{TOKEN}\n")).unwrap();
    dir.join("state.json")
}

#[tokio::test]
async fn a_long_answer_leaves_no_buffer_of_its_length_behind() {
    let dir = tempfile::tempdir().unwrap();
    let port = fake_host().await;
    let env: Env = [("HOME".to_owned(), dir.path().to_string_lossy().into_owned())].into_iter().collect();
    let client = HostClient::connect(&locked(dir.path(), port), &env).await.unwrap_or_else(|f| panic!("{}", f.message));
    for _ in 0..AFTER {
        client.call_raw("small", Map::new()).await.unwrap_or_else(|f| panic!("{}", f.message));
    }
    let before = LIVE.load(Ordering::Relaxed);
    let long = client.call_raw("long", Map::new()).await.unwrap_or_else(|f| panic!("{}", f.message));
    assert_eq!(long["blob"].as_str().map(str::len), Some(LONG), "the long answer came whole");
    drop(long);
    for _ in 0..AFTER {
        client.call_raw("small", Map::new()).await.unwrap_or_else(|f| panic!("{}", f.message));
    }
    let after = LIVE.load(Ordering::Relaxed);
    println!("live heap {:+.1} MB once a {:.0} MB answer and {AFTER} small ones were read", mb(after - before), mb(LONG as isize));
    assert!(
        after - before < BOUND,
        "the socket kept {:.1} MB once a {:.0} MB answer was read and gone, past the {:.1} MB bound",
        mb(after - before),
        mb(LONG as isize),
        mb(BOUND)
    );
}
