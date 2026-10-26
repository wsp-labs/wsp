// SPDX-License-Identifier: AGPL-3.0-only
//! `portProbe`'s fetch (workspaces.ts:1190-1202): `fetch(url, {redirect: "manual", signal: AbortSignal.timeout(PORT_PROBE_TIMEOUT_MS)})`
//! then `readBodyUpTo(res, PORT_PROBE_BODY_CAP)`. Redirect and cap expectations are what node 22 printed for the same
//! server: a 302 answers 302 with its own body, a 5000-byte body is cut at 2048.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wsp_core::types::wiring::{PORT_PROBE_BODY_CAP, PORT_PROBE_TIMEOUT_MS};
use wsp_engine::errors::{RoadCause, RoadError};
use wsp_engine::fetch::{platform_fetch_manual, Fetch, FetchRequest, FetchResponse, FetchStream};
use wsp_serve::runtime::probe::probe_port;

async fn answer(mut s: TcpStream) {
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    while !got.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut buf).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        got.extend_from_slice(&buf[..n]);
    }
    let head = String::from_utf8_lossy(&got).to_string();
    let (status, extra, body) = if head.starts_with("GET /r ") {
        ("302 Found", "location: /final\r\n", format!("moved {}", "x".repeat(5000)))
    } else if head.starts_with("GET /big ") {
        ("200 OK", "", "y".repeat(5000))
    } else {
        ("200 OK", "", "final".to_string())
    };
    let wire = format!("HTTP/1.1 {status}\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
    let _ = s.write_all(wire.as_bytes()).await;
}

async fn serve() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((s, _)) = l.accept().await else { return };
            tokio::spawn(answer(s));
        }
    });
    port
}

fn view(of: &wsp_wire::views::workspace::PortProbeView) -> Value {
    serde_json::to_value(of).unwrap()
}

#[tokio::test]
async fn a_redirect_is_answered_not_followed_and_its_body_is_cut() {
    let port = serve().await;
    let got = probe_port(&*platform_fetch_manual(), &format!("http://127.0.0.1:{port}/r")).await.unwrap();
    let want_body = format!("moved {}", "x".repeat(2042));
    assert_eq!(view(&got), json!({ "status": 302, "body": want_body }));
}

#[tokio::test]
async fn a_long_body_is_cut_at_the_cap() {
    let port = serve().await;
    let got = probe_port(&*platform_fetch_manual(), &format!("http://127.0.0.1:{port}/big")).await.unwrap();
    assert_eq!(PORT_PROBE_BODY_CAP, 2048);
    assert_eq!(view(&got), json!({ "status": 200, "body": "y".repeat(2048) }));
}

#[tokio::test]
async fn a_short_body_is_whole() {
    let port = serve().await;
    let got = probe_port(&*platform_fetch_manual(), &format!("http://127.0.0.1:{port}/final")).await.unwrap();
    assert_eq!(view(&got), json!({ "status": 200, "body": "final" }));
}

/// A fetch that records what it was asked and answers a body of 1000-byte chunks, counting the chunks pulled.
struct Scripted {
    asked: RefCell<Vec<(String, String)>>,
    pulled: Rc<Cell<usize>>,
    never: bool,
}

#[async_trait(?Send)]
impl Fetch for Scripted {
    async fn fetch(&self, _req: FetchRequest) -> Result<FetchResponse, RoadError> {
        unreachable!("the probe reads its body as a stream")
    }
    async fn fetch_stream(&self, req: FetchRequest) -> Result<FetchStream, RoadError> {
        self.asked.borrow_mut().push((req.method.clone(), req.url.clone()));
        if self.never {
            let abort = req.abort.expect("the probe arms a timeout");
            abort.token.cancelled().await;
            return Err(RoadError {
                name: "TimeoutError".into(),
                message: "The operation was aborted due to timeout".into(),
                code: None,
                cause: None,
            });
        }
        let pulled = self.pulled.clone();
        let body = stream::iter(0..10)
            .map(move |_| {
                pulled.set(pulled.get() + 1);
                Ok::<_, RoadError>(vec![b'z'; 1000])
            })
            .boxed_local();
        Ok(FetchStream { status: 200, headers: vec![], body: Some(body) })
    }
}

#[tokio::test]
async fn the_probe_is_a_get_and_pulls_no_chunk_past_the_cap() {
    let fetch = Scripted { asked: RefCell::new(vec![]), pulled: Rc::new(Cell::new(0)), never: false };
    let got = probe_port(&fetch, "http://127.0.0.1:1/x").await.unwrap();
    assert_eq!(fetch.asked.borrow().clone(), vec![("GET".to_string(), "http://127.0.0.1:1/x".to_string())]);
    assert_eq!(fetch.pulled.get(), 3, "3 chunks of 1000 reach 2048; no fourth is read");
    assert_eq!(view(&got), json!({ "status": 200, "body": "z".repeat(2048) }));
}

#[tokio::test(start_paused = true)]
async fn a_probe_nothing_answers_fails_at_the_timeout_with_the_timeout_words() {
    let fetch = Scripted { asked: RefCell::new(vec![]), pulled: Rc::new(Cell::new(0)), never: true };
    let asking = probe_port(&fetch, "http://127.0.0.1:1/x");
    tokio::pin!(asking);
    tokio::time::advance(Duration::from_millis(PORT_PROBE_TIMEOUT_MS - 1)).await;
    assert!(futures::poll!(asking.as_mut()).is_pending(), "still waiting just before the timeout");
    tokio::time::advance(Duration::from_millis(2)).await;
    let err = asking.await.expect_err("timed out");
    assert_eq!(err.message, "The operation was aborted due to timeout");
}

#[tokio::test]
async fn a_failed_fetch_says_what_fetch_says() {
    struct Refused;
    #[async_trait(?Send)]
    impl Fetch for Refused {
        async fn fetch(&self, _req: FetchRequest) -> Result<FetchResponse, RoadError> {
            unreachable!()
        }
        async fn fetch_stream(&self, _req: FetchRequest) -> Result<FetchStream, RoadError> {
            Err(RoadError {
                name: "TypeError".into(),
                message: "fetch failed".into(),
                code: None,
                cause: Some(RoadCause {
                    message: "connect ECONNREFUSED 127.0.0.1:1".into(),
                    code: Some("ECONNREFUSED".into()),
                    errors: None,
                }),
            })
        }
    }
    let err = probe_port(&Refused, "http://127.0.0.1:1/x").await.expect_err("refused");
    assert_eq!(err.message, "fetch failed");
}
