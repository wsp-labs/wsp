// SPDX-License-Identifier: AGPL-3.0-only
//! An HTTP stub, a door that refuses the upgrade, and a TCP proxy (packages/runtime/test/tcp-proxy.ts), all on the
//! test's own thread: a proxy freeze stops this thread, as a stalled host loop does.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

/// How long the stub waits before it answers; `Never` holds the request open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Delay {
    Ms(u64),
    Never,
}

/// One HTTP server answering every request with `status` and an empty body after `delay`.
pub struct HttpStub {
    pub port: u16,
    pub status: Rc<Cell<u16>>,
    pub delay: Rc<Cell<Delay>>,
    pub hits: Rc<Cell<u32>>,
    task: JoinHandle<()>,
}

impl HttpStub {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }
    pub fn close(&self) {
        self.task.abort();
    }
}

async fn read_head(sock: &mut TcpStream) -> bool {
    let mut seen = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        match sock.read(&mut buf).await {
            Ok(0) | Err(_) => return false,
            Ok(n) => {
                seen.extend_from_slice(&buf[..n]);
                if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    return true;
                }
            }
        }
    }
}

pub async fn http_stub(status: u16, delay: Delay) -> HttpStub {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let status = Rc::new(Cell::new(status));
    let delay = Rc::new(Cell::new(delay));
    let hits = Rc::new(Cell::new(0));
    let (s, d, h) = (status.clone(), delay.clone(), hits.clone());
    let task = tokio::task::spawn_local(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let (s, d, h) = (s.clone(), d.clone(), h.clone());
            tokio::task::spawn_local(async move {
                if !read_head(&mut sock).await {
                    return;
                }
                h.set(h.get() + 1);
                match d.get() {
                    Delay::Never => std::future::pending::<()>().await,
                    Delay::Ms(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
                }
                let head = format!("HTTP/1.1 {} Stub\r\ncontent-length: 0\r\nconnection: close\r\n\r\n", s.get());
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    HttpStub { port, status, delay, hits, task }
}

/// A door that answers every request, the websocket upgrade included, with `status` and `body`.
pub async fn refusing_door(status: u16, body: &str) -> HttpStub {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Rc::new(Cell::new(0));
    let h = hits.clone();
    let body = body.to_string();
    let task = tokio::task::spawn_local(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let (h, body) = (h.clone(), body.clone());
            tokio::task::spawn_local(async move {
                if !read_head(&mut sock).await {
                    return;
                }
                h.set(h.get() + 1);
                let head = format!(
                    "HTTP/1.1 {status} Refused\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    HttpStub { port, status: Rc::new(Cell::new(status)), delay: Rc::new(Cell::new(Delay::Ms(0))), hits, task }
}

#[derive(Default)]
struct ProxyState {
    stalled: Cell<bool>,
    freeze_ms: Cell<u64>,
    pairs: RefCell<Vec<(JoinHandle<()>, JoinHandle<()>)>>,
}

/// tcp-proxy.ts: forwards to a target port; can stall, cut every link, and freeze this thread after a reply.
pub struct TcpProxy {
    pub port: u16,
    state: Rc<ProxyState>,
    task: JoinHandle<()>,
}

impl TcpProxy {
    /// Destroy every live connection without stopping the listener, like an idle sweep.
    pub fn cut_all(&self) {
        for (a, b) in self.state.pairs.borrow_mut().drain(..) {
            a.abort();
            b.abort();
        }
    }
    /// Keep every socket open but forward nothing.
    pub fn stall(&self) {
        self.state.stalled.set(true);
    }
    pub fn resume(&self) {
        self.state.stalled.set(false);
    }
    pub fn live(&self) -> usize {
        self.state.pairs.borrow().iter().filter(|(a, _)| !a.is_finished()).count()
    }
    /// Forwards the next chunk the target sends, then holds this whole thread still for `ms`.
    pub fn freeze_after_next_reply(&self, ms: u64) {
        self.state.freeze_ms.set(ms);
    }
    pub async fn close(&self) {
        self.cut_all();
        self.task.abort();
        tokio::task::yield_now().await;
    }
}

pub async fn tcp_proxy(target: u16, listen: u16) -> TcpProxy {
    let listener = TcpListener::bind(("127.0.0.1", listen)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Rc::new(ProxyState::default());
    let st = state.clone();
    let task = tokio::task::spawn_local(async move {
        while let Ok((a, _)) = listener.accept().await {
            let Ok(b) = TcpStream::connect(("127.0.0.1", target)).await else { continue };
            let (mut ar, mut aw) = a.into_split();
            let (mut br, mut bw) = b.into_split();
            let (s1, s2) = (st.clone(), st.clone());
            let up = tokio::task::spawn_local(async move {
                let mut buf = [0u8; 16384];
                while let Ok(n) = ar.read(&mut buf).await {
                    if n == 0 || (!s1.stalled.get() && bw.write_all(&buf[..n]).await.is_err()) {
                        break;
                    }
                }
            });
            let down = tokio::task::spawn_local(async move {
                let mut buf = [0u8; 16384];
                while let Ok(n) = br.read(&mut buf).await {
                    if n == 0 || (!s2.stalled.get() && aw.write_all(&buf[..n]).await.is_err()) {
                        break;
                    }
                    let ms = s2.freeze_ms.replace(0);
                    if ms > 0 {
                        std::thread::sleep(Duration::from_millis(ms));
                    }
                }
            });
            st.pairs.borrow_mut().push((up, down));
        }
    });
    TcpProxy { port, state, task }
}
