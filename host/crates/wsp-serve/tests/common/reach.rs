// SPDX-License-Identifier: AGPL-3.0-only
//! What runtime-reach.test.ts (packages/runtime/test/runtime-reach.test.ts at 65f6b89c7) wires around its cases: the
//! preview edge in front of a machine whose daemon port answers a prompt 502 and, once `up` holds, 426 (`daemonPort`), the
//! route a machine's `previewUrl` mints for it, and the token line the guest is handed.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use wsp_core::clock::FakeClock;

use wsp_engine::machine::Machine as _;

use super::stub_backend::StubMachine;

/// A preview edge in front of a machine whose daemon port answers nothing: the shape a dead daemon has on the wire (a
/// prompt 502), and 426 once `up` is set, which is the daemon answering again.
pub struct DaemonPort {
    pub port: u16,
    hits: Rc<Cell<u32>>,
    task: JoinHandle<()>,
}

impl Drop for DaemonPort {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl DaemonPort {
    pub fn hits(&self) -> u32 {
        self.hits.get()
    }
}

/// `daemonPort(up)`.
pub async fn daemon_port(up: impl Fn() -> bool + 'static) -> DaemonPort {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
    let port = listener.local_addr().expect("an address").port();
    let hits: Rc<Cell<u32>> = Rc::default();
    let (counted, up) = (hits.clone(), Rc::new(up));
    let task = tokio::task::spawn_local(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let (counted, up) = (counted.clone(), up.clone());
            tokio::task::spawn_local(async move {
                let mut head = [0u8; 4096];
                let _ = stream.read(&mut head).await;
                counted.set(counted.get() + 1);
                let reply: &[u8] = if up() {
                    b"HTTP/1.1 426 Upgrade Required\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                } else {
                    b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                };
                let _ = stream.write_all(reply).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    DaemonPort { port, hits, task }
}

/// `m.previewUrl = async port => ({ url: `http://127.0.0.1:${port}/?port=${p}`, token: "e", expiresAt: now + 3_600_000 })`,
/// the expiry read off `clock` where the test runs on one.
pub fn route_through(m: &Arc<StubMachine>, edge_port: u16, clock: Option<FakeClock>) {
    m.set_preview(Some(Rc::new(move |port| {
        let now = clock.as_ref().map_or_else(super::now_ms, FakeClock::now);
        let reach = wsp_engine::machine::PreviewReach {
            url: format!("http://127.0.0.1:{edge_port}/?port={port}"),
            token: "e".into(),
            expires_at: wsp_wire::types::JsNumber((now + 3_600_000) as f64),
        };
        Box::pin(async move { Ok(reach) })
    })));
}

/// `m.previewUrl = async port => ({ url: `https://${id}-${port}.preview.example/?pt_token=${token}`, token, expiresAt })`.
pub fn preview_example(m: &Arc<StubMachine>, token: &'static str, minted: Option<Rc<Cell<u32>>>) {
    let id = m.id().to_string();
    m.set_preview(Some(Rc::new(move |port| {
        if let Some(minted) = &minted {
            minted.set(minted.get() + 1);
        }
        let reach = wsp_engine::machine::PreviewReach {
            url: format!("https://{id}-{port}.preview.example/?pt_token={token}"),
            token: token.into(),
            expires_at: wsp_wire::types::JsNumber((super::now_ms() + 3_600_000) as f64),
        };
        Box::pin(async move { Ok(reach) })
    })));
}

/// The token a guest was handed in `cmd`: the `WSP_DAEMON_TOKEN='<token>'` line, as `/^WSP_DAEMON_TOKEN='([^']*)'$/m` reads it.
pub fn token_written(cmd: &str) -> Option<String> {
    cmd.lines().find_map(|line| {
        let inner = line.strip_prefix("WSP_DAEMON_TOKEN='")?.strip_suffix('\'')?;
        (!inner.contains('\'')).then(|| inner.to_string())
    })
}
