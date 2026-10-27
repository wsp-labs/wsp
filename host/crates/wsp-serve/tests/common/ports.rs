// SPDX-License-Identifier: AGPL-3.0-only
//! held-port.ts:15 `droppingPort` and gone.test.ts:70 `edgeHolding`: loopback listeners a test stands up so the route a
//! machine's preview answers with leads somewhere that misses, or somewhere the test answers by hand.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

/// A port the test holds for its whole life; closed when dropped.
pub struct HeldPort {
    pub port: u16,
    task: JoinHandle<()>,
}

impl Drop for HeldPort {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A held port that drops every connection unanswered, so each read is the machine's miss.
pub async fn dropping_port() -> HeldPort {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
    let port = listener.local_addr().expect("an address").port();
    let task = tokio::task::spawn_local(async move {
        while let Ok((stream, _)) = listener.accept().await {
            drop(stream);
        }
    });
    HeldPort { port, task }
}

/// A request the edge read and has not answered.
pub struct Held(TcpStream);

/// An edge that answers for the machine, not the daemon, and only when the test says so: a probe waits in `held` until
/// the test answers it (404) or drops it (a failed reach). With `from_daemon` set the answer is the daemon's own
/// (426), which sends nobody to the provider.
pub struct Edge {
    pub port: u16,
    held: Rc<RefCell<VecDeque<Held>>>,
    pub from_daemon: Cell<bool>,
    task: JoinHandle<()>,
}

impl Drop for Edge {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Edge {
    pub async fn start() -> Edge {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let held: Rc<RefCell<VecDeque<Held>>> = Rc::default();
        let kept = held.clone();
        let task = tokio::task::spawn_local(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let kept = kept.clone();
                tokio::task::spawn_local(async move {
                    let mut buf = Vec::new();
                    loop {
                        let mut chunk = [0u8; 2048];
                        match stream.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    kept.borrow_mut().push_back(Held(stream));
                });
            }
        });
        Edge { port, held, from_daemon: Cell::new(false), task }
    }

    pub fn held(&self) -> usize {
        self.held.borrow().len()
    }

    /// `edge.held.shift()`.
    pub fn take(&self) -> Option<Held> {
        self.held.borrow_mut().pop_front()
    }

    /// The first `n` held requests, as `edge.held.splice(0, n)`.
    pub fn take_n(&self, n: usize) -> Vec<Held> {
        let mut held = self.held.borrow_mut();
        let n = n.min(held.len());
        held.drain(..n).collect()
    }

    /// `edge.answer(res)`: 404 for the machine, or 426 where the daemon speaks.
    pub async fn answer(&self, mut res: Held) {
        let (code, word) = if self.from_daemon.get() { (426, "Upgrade Required") } else { (404, "Not Found") };
        let _ = res.0.write_all(format!("HTTP/1.1 {code} {word}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").as_bytes()).await;
        let _ = res.0.shutdown().await;
    }

    /// `edge.drop(res)`: the connection destroyed unanswered.
    pub fn drop_request(&self, res: Held) {
        drop(res);
    }
}
