// SPDX-License-Identifier: AGPL-3.0-only
#![allow(dead_code)]
//! What the app asks a host, counted on the wire: a proxy before a test's own throwaway host that reads every op the
//! app sends, and a state folder that points the app at the proxy.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::common::Throwaway;

/// Every op the app sent through the proxy, with whether it named a workspace.
pub type Sent = Arc<Mutex<Vec<(String, bool)>>>;

/// One client frame's op off a websocket stream: the frames after the upgrade, unmasked.
fn frames(stream: &mut Vec<u8>, sent: &Sent) {
    loop {
        if stream.len() < 2 {
            return;
        }
        let (masked, mut len, mut at) = (stream[1] & 0x80 != 0, (stream[1] & 0x7f) as usize, 2);
        if len == 126 {
            if stream.len() < 4 {
                return;
            }
            len = u16::from_be_bytes([stream[2], stream[3]]) as usize;
            at = 4;
        } else if len == 127 {
            if stream.len() < 10 {
                return;
            }
            len = u64::from_be_bytes(stream[2..10].try_into().unwrap()) as usize;
            at = 10;
        }
        let key_at = at;
        if masked {
            at += 4;
        }
        if stream.len() < at + len {
            return;
        }
        let mut payload = stream[at..at + len].to_vec();
        if masked {
            let key = &stream[key_at..key_at + 4];
            payload.iter_mut().enumerate().for_each(|(i, b)| *b ^= key[i % 4]);
        }
        if let Ok(frame) = serde_json::from_slice::<Value>(&payload) {
            if let Some(op) = frame.get("op").and_then(Value::as_str) {
                sent.lock().unwrap().push((op.to_owned(), frame.get("workspaceId").is_some()));
            }
        }
        stream.drain(..at + len);
    }
}

/// A listener that hands each connection to the host on `port` and reads what the app sends; answers its port.
pub fn proxy(port: u16, sent: Sent) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mine = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let Ok(host) = TcpStream::connect(("127.0.0.1", port)) else { continue };
            let (mut from_host, mut to_client) = (host.try_clone().unwrap(), client.try_clone().unwrap());
            std::thread::spawn(move || _ = std::io::copy(&mut from_host, &mut to_client));
            let (mut from_client, mut to_host, sent) = (client, host, sent.clone());
            std::thread::spawn(move || {
                let (mut buf, mut seen, mut upgraded) = ([0u8; 65536], Vec::new(), false);
                while let Ok(n) = from_client.read(&mut buf) {
                    if n == 0 || to_host.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    seen.extend_from_slice(&buf[..n]);
                    if !upgraded {
                        let Some(end) = seen.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                        seen.drain(..end + 4);
                        upgraded = true;
                    }
                    frames(&mut seen, &sent);
                }
            });
        }
    });
    mine
}

/// A state folder beside a copy of the host's, whose lock names the proxy's port: the app dials the proxy.
pub fn through(host: &Throwaway, port: u16) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let from = host.state().parent().unwrap().to_owned();
    for entry in std::fs::read_dir(&from).unwrap().flatten() {
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), dir.path().join(entry.file_name())).unwrap();
        }
    }
    let lock = dir.path().join("host.lock");
    let mut held: Value = serde_json::from_str(&std::fs::read_to_string(&lock).unwrap()).unwrap();
    held["port"] = Value::from(port);
    std::fs::write(&lock, held.to_string()).unwrap();
    let state = dir.path().join("state.json");
    (dir, state)
}

/// The ops the app sends to `host` from now on, and the state folder that reaches it through the proxy.
pub fn proxied(host: &Throwaway) -> (Sent, tempfile::TempDir, PathBuf) {
    let lock: Value = serde_json::from_str(&std::fs::read_to_string(host.state().with_file_name("host.lock")).unwrap()).unwrap();
    let sent: Sent = Arc::default();
    let port = proxy(lock["port"].as_u64().unwrap() as u16, sent.clone());
    let (dir, state) = through(host, port);
    (sent, dir, state)
}

/// How many of `sent` were `op`.
pub fn count(sent: &Sent, op: &str) -> usize {
    sent.lock().unwrap().iter().filter(|(o, _)| o == op).count()
}
