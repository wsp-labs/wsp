// SPDX-License-Identifier: AGPL-3.0-only
//! stub-backend.ts:194 `vaultServer`: one loopback server per backend. GET serves the archive `downloads` gives for the
//! path (the empty tar without it), PUT accepts anything and records it.
use std::cell::RefCell;
use std::rc::Rc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::stub_backend::Put;

/// `gzipSync(Buffer.alloc(1024))`: two zero blocks are a complete empty tar, so downloads are real archives.
pub const EMPTY_TGZ: [u8; 29] = [31, 139, 8, 0, 0, 0, 0, 0, 2, 3, 99, 96, 24, 5, 163, 96, 20, 140, 84, 0, 0, 46, 175, 181, 239, 0, 4, 0, 0];

pub struct VaultServer {
    origin: tokio::sync::OnceCell<String>,
    puts: Rc<RefCell<Vec<Put>>>,
    downloads: Rc<RefCell<Option<Rc<dyn Fn(&str) -> Vec<u8>>>>>,
}

impl VaultServer {
    pub fn new() -> Rc<VaultServer> {
        Rc::new(VaultServer { origin: tokio::sync::OnceCell::new(), puts: Rc::default(), downloads: Rc::default() })
    }

    pub fn puts(&self) -> Vec<Put> {
        self.puts.borrow().clone()
    }

    pub fn set_downloads(&self, f: Rc<dyn Fn(&str) -> Vec<u8>>) {
        *self.downloads.borrow_mut() = Some(f);
    }

    /// The server's origin, listening from the first ask.
    pub async fn origin(&self) -> String {
        self.origin
            .get_or_init(|| async {
                let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
                let origin = format!("http://127.0.0.1:{}", listener.local_addr().expect("an address").port());
                let puts = self.puts.clone();
                let downloads = self.downloads.clone();
                tokio::task::spawn_local(async move {
                    while let Ok((stream, _)) = listener.accept().await {
                        tokio::task::spawn_local(serve(stream, puts.clone(), downloads.clone()));
                    }
                });
                origin
            })
            .await
            .clone()
    }
}

async fn serve(mut stream: TcpStream, puts: Rc<RefCell<Vec<Put>>>, downloads: Rc<RefCell<Option<Rc<dyn Fn(&str) -> Vec<u8>>>>>) {
    let mut buf: Vec<u8> = Vec::new();
    let head_end = loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let request = lines.next().unwrap_or_default().to_string();
    let mut parts = request.split(' ');
    let (method, target) = (parts.next().unwrap_or_default().to_string(), parts.next().unwrap_or_default().to_string());
    let header = |name: &str| -> Option<String> {
        head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string()))
    };
    if method == "PUT" {
        let mut body = buf[head_end..].to_vec();
        if header("transfer-encoding").is_some_and(|t| t.eq_ignore_ascii_case("chunked")) {
            body = read_chunked(&mut stream, body).await;
        } else {
            let want: usize = header("content-length").and_then(|n| n.parse().ok()).unwrap_or(0);
            while body.len() < want {
                let mut chunk = [0u8; 8192];
                match stream.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => body.extend_from_slice(&chunk[..n]),
                }
            }
        }
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or_default();
        let pick = |key: &str| {
            query.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| unescape(v))).unwrap_or_default()
        };
        puts.borrow_mut().push(Put { machine: pick("machine"), path: pick("path"), body });
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nconnection: close\r\ncontent-length: 0\r\n\r\n").await;
    } else {
        let path = target.split('?').next().unwrap_or_default().trim_start_matches("/download").to_string();
        let served = downloads.borrow().clone();
        let body = served.map(|f| f(&path)).unwrap_or_else(|| EMPTY_TGZ.to_vec());
        let head =
            format!("HTTP/1.1 200 OK\r\nconnection: close\r\ncontent-type: application/gzip\r\ncontent-length: {}\r\n\r\n", body.len());
        let _ = stream.write_all(head.as_bytes()).await;
        let _ = stream.write_all(&body).await;
    }
    let _ = stream.shutdown().await;
}

async fn read_chunked(stream: &mut TcpStream, mut raw: Vec<u8>) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        while !raw.windows(2).any(|w| w == b"\r\n") {
            let mut chunk = [0u8; 8192];
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return body,
                Ok(n) => raw.extend_from_slice(&chunk[..n]),
            }
        }
        let at = raw.windows(2).position(|w| w == b"\r\n").unwrap_or(0);
        let size = usize::from_str_radix(String::from_utf8_lossy(&raw[..at]).trim(), 16).unwrap_or(0);
        raw.drain(..at + 2);
        if size == 0 {
            return body;
        }
        while raw.len() < size + 2 {
            let mut chunk = [0u8; 8192];
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return body,
                Ok(n) => raw.extend_from_slice(&chunk[..n]),
            }
        }
        body.extend_from_slice(&raw[..size]);
        raw.drain(..size + 2);
    }
}

fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
