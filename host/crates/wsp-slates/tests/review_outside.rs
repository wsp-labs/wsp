// SPDX-License-Identifier: AGPL-3.0-only
//! Review of what wsp-slates reaches outside the process: the image fetch, the MCP http client, the box roads and the
//! stdio spawn. Every test here fails on the current code and pins what the TS does (file:line in the comment above it).
//! Servers are loopback listeners; nothing reaches the internet. The timer, box and file-read parts held (no test).

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wsp_slates::slate_image_fetch::{fetch_slate_image, FetchRoad, LookedUp};
use wsp_slates::slate_images::{slate_image, SlateImageRoads};
use wsp_slates::slate_mcp::{
    fetch_here, spawn_here, McpHttpAnswer, McpHttpRequest, McpPipeEvents, McpPost, McpServerSpec, McpSignIn, McpTransport,
};
use wsp_slates::slate_mcp_on::renew_on_refusal;

const PNG_HEX: &str = "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d4944415478da63f8cfc0f01f0005000201a3b0d8f60000000049454e44ae426082";

fn png() -> Vec<u8> {
    (0..PNG_HEX.len()).step_by(2).map(|i| u8::from_str_radix(&PNG_HEX[i..i + 2], 16).unwrap()).collect()
}

/// A road that lets 127.0.0.1 through and resolves every name to it.
struct Loopback {
    timeout: u64,
}

impl FetchRoad for Loopback {
    fn lookup(&self, _host: &str) -> LocalBoxFuture<'static, Result<Vec<LookedUp>, String>> {
        async { Ok(vec![LookedUp { address: "127.0.0.1".into(), family: 4 }]) }.boxed_local()
    }

    fn refuses(&self, _address: &str) -> Option<String> {
        None
    }

    fn timeout_ms(&self) -> u64 {
        self.timeout
    }
}

fn road(timeout: u64) -> Option<Rc<dyn FetchRoad>> {
    Some(Rc::new(Loopback { timeout }))
}

type Handler = Box<dyn FnOnce(TcpStream) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

/// One connection accepted, its request head read to the blank line, then `handler` has the socket.
async fn serve_once(handler: Handler) -> u16 {
    serve_once_keeping(handler, Arc::new(std::sync::Mutex::new(String::new()))).await
}

/// As `serve_once`, the request head kept in `head`.
async fn serve_once_keeping(handler: Handler, head: Arc<std::sync::Mutex<String>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else { return };
        let mut got = Vec::new();
        let mut chunk = [0u8; 4096];
        while !got.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => got.extend_from_slice(&chunk[..n]),
            }
        }
        *head.lock().unwrap() = String::from_utf8_lossy(&got).into_owned();
        handler(stream).await;
    });
    port
}

fn handler<F, Fut>(f: F) -> Handler
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    Box::new(move |s| Box::pin(f(s)))
}

fn said(answer: &Value) -> String {
    if let Some(p) = answer.get("problem") {
        return format!("{} {}", p["code"].as_str().unwrap(), p["message"].as_str().unwrap());
    }
    format!("shown {}", answer["mediaType"].as_str().unwrap_or("?"))
}

async fn fetched(port: u16, timeout: u64) -> (String, Duration) {
    let url = format!("http://img.test:{port}/a.png");
    let started = Instant::now();
    let answer = tokio::time::timeout(Duration::from_secs(20), fetch_slate_image(&url, "img.test", road(timeout)))
        .await
        .expect("the fetch outlived its own deadline");
    (said(&serde_json::to_value(&answer).unwrap()).replace(&url, "<url>"), started.elapsed())
}

/// A server that answers its one request 404 and keeps the head of it.
async fn serve_head() -> (u16, Arc<std::sync::Mutex<String>>) {
    let head = Arc::new(std::sync::Mutex::new(String::new()));
    let port = serve_once_keeping(
        handler(|mut s| async move {
            let _ = s.write_all(b"HTTP/1.1 404 Nope\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
            let _ = s.shutdown().await;
        }),
        head.clone(),
    )
    .await;
    (port, head)
}

/// Writes 64 KiB blocks of `byte` up to `cap` bytes, counting what the socket took; stops when a write fails.
async fn pour(stream: &mut TcpStream, byte: u8, cap: usize, sent: &AtomicUsize) {
    let block = vec![byte; 64 * 1024];
    while sent.load(Ordering::SeqCst) < cap {
        if stream.write_all(&block).await.is_err() {
            return;
        }
        sent.fetch_add(block.len(), Ordering::SeqCst);
    }
}

// ---- part 1: the image fetch ----

// TS slate-image-fetch.ts:112: node's http client stops a response head at 16 KiB ("Parse Error: Header overflow").
#[tokio::test(flavor = "current_thread")]
async fn a_header_line_that_never_ends_is_cut_at_node_s_head_cap_not_read_until_the_deadline() {
    let sent = Arc::new(AtomicUsize::new(0));
    let counted = sent.clone();
    let port = serve_once(handler(move |mut s| async move {
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nx-pad: ").await;
        pour(&mut s, b'a', 64 * 1024 * 1024, &counted).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }))
    .await;
    let (got, took) = fetched(port, 3_000).await;
    let seen = sent.load(Ordering::SeqCst);
    assert!(got.starts_with("R903 ") && got.contains("could not be read"), "{got} after {took:?}, the server poured {seen} bytes");
    assert!(seen < 32 * 1024 * 1024, "the client read {seen} bytes of one header line before it gave up (took {took:?}, said {got})");
}

// TS slate-image-fetch.ts:133 and :163: a body shorter than its content-length is "the answer stopped part way".
#[tokio::test(flavor = "current_thread")]
async fn a_body_that_stops_short_of_its_content_length_is_refused_not_shown() {
    let port = serve_once(handler(|mut s| async move {
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 1000\r\nconnection: close\r\n\r\n").await;
        let _ = s.write_all(&png()).await;
        let _ = s.shutdown().await;
    }))
    .await;
    let (got, _) = fetched(port, 3_000).await;
    assert_eq!(got, "R903 <url> could not be read: the answer stopped part way");
}

// TS slate-image-fetch.ts:112 (node's http client): a 1xx answer is skipped and the final answer is read.
#[tokio::test(flavor = "current_thread")]
async fn an_early_hints_answer_before_the_image_is_skipped() {
    let port = serve_once(handler(|mut s| async move {
        let _ = s.write_all(b"HTTP/1.1 103 Early Hints\r\nlink: </x.css>; rel=preload\r\n\r\n").await;
        let head = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", png().len());
        let _ = s.write_all(head.as_bytes()).await;
        let _ = s.write_all(&png()).await;
        let _ = s.shutdown().await;
    }))
    .await;
    let (got, _) = fetched(port, 3_000).await;
    assert_eq!(got, "shown image/png");
}

// TS slate-image-fetch.ts:112 (node's http client reads header bytes as latin1, so any byte is a header byte).
#[tokio::test(flavor = "current_thread")]
async fn a_header_with_a_byte_that_is_not_utf8_does_not_fail_the_fetch() {
    let port = serve_once(handler(|mut s| async move {
        let mut head = b"HTTP/1.1 200 OK\r\nx-name: caf".to_vec();
        head.push(0xe9);
        head.extend_from_slice(format!("\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", png().len()).as_bytes());
        let _ = s.write_all(&head).await;
        let _ = s.write_all(&png()).await;
        let _ = s.shutdown().await;
    }))
    .await;
    let (got, _) = fetched(port, 3_000).await;
    assert_eq!(got, "shown image/png");
}

/// A server that accepts and never answers, counting the connections it holds open.
async fn serve_holding(live: Arc<AtomicUsize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            live.fetch_add(1, Ordering::SeqCst);
            let live = live.clone();
            tokio::spawn(async move {
                let mut sink = [0u8; 1024];
                while matches!(stream.read(&mut sink).await, Ok(n) if n > 0) {}
                live.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    port
}

// TS slate-images.ts:39-50 (a read in a turn gives its place back when it ends, and nothing in the TS can be dropped half way).
#[tokio::test(flavor = "current_thread")]
async fn a_read_dropped_after_it_was_handed_a_turn_gives_the_turn_back() {
    use futures::StreamExt;
    let live = Arc::new(AtomicUsize::new(0));
    let port = serve_holding(live.clone()).await;
    let url = format!("http://img.test:{port}/a.png");
    let read = || {
        slate_image(
            &url,
            SlateImageRoads {
                thread: "t-turns".into(),
                have: None,
                folder: None,
                on: None,
                allowed: Rc::new(|_| true),
                fetch: road(60_000),
            },
        )
    };
    let mut first: Vec<_> = (0..4).map(|_| Box::pin(read())).collect();
    for f in first.iter_mut() {
        assert!(futures::poll!(f.as_mut()).is_pending());
    }
    let mut fifth = Box::pin(read());
    assert!(futures::poll!(fifth.as_mut()).is_pending());
    drop(first.remove(0));
    drop(fifth);
    drop(first);
    let mut again: futures::stream::FuturesUnordered<_> = (0..4).map(|_| read()).collect();
    let _ = tokio::time::timeout(Duration::from_millis(600), again.next()).await;
    let held = live.load(Ordering::SeqCst);
    assert_eq!(held, 4, "four reads of one thread may run at once, and {held} did after a queued read was dropped");
}

// ---- part 2: the MCP http client ----

async fn posted(port: u16, ms: u64) -> (Result<McpHttpAnswer, String>, Duration) {
    let t = McpTransport::Http { url: format!("http://127.0.0.1:{port}/mcp"), headers: IndexMap::new() };
    let req = McpHttpRequest { headers: IndexMap::new(), body: "{}".into(), ms };
    let started = Instant::now();
    let local = tokio::task::LocalSet::new();
    let got = local
        .run_until(tokio::time::timeout(Duration::from_secs(20), fetch_here().send(&t, req)))
        .await
        .expect("the post outlived its own deadline");
    (got, started.elapsed())
}

// TS slate-mcp.ts:416 (undici): a response head over 16 KiB fails the message at once, "fetch failed" (UND_ERR_HEADERS_OVERFLOW).
#[tokio::test(flavor = "current_thread")]
async fn mcp_a_header_line_that_never_ends_fails_at_the_head_cap_not_at_the_deadline() {
    let sent = Arc::new(AtomicUsize::new(0));
    let counted = sent.clone();
    let port = serve_once(handler(move |mut s| async move {
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nx-pad: ").await;
        pour(&mut s, b'a', 64 * 1024 * 1024, &counted).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }))
    .await;
    let (got, took) = posted(port, 3_000).await;
    let seen = sent.load(Ordering::SeqCst);
    assert_eq!(got, Err("fetch failed".to_string()), "after {took:?}, the server poured {seen} bytes");
    assert!(seen < 32 * 1024 * 1024, "the client read {seen} bytes of one header line before it gave up");
}

// TS slate-mcp.ts:416 (undici): a 1xx answer is skipped and the final answer is read.
#[tokio::test(flavor = "current_thread")]
async fn mcp_an_early_hints_answer_before_the_reply_is_skipped() {
    let port = serve_once(handler(|mut s| async move {
        let _ = s.write_all(b"HTTP/1.1 103 Early Hints\r\nlink: </x>; rel=preload\r\n\r\n").await;
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").await;
        let _ = s.shutdown().await;
    }))
    .await;
    let (got, _) = posted(port, 3_000).await;
    assert_eq!(got.map(|a| (a.status, a.body)), Ok((200, "{}".to_string())));
}

// TS slate-mcp.ts:416 (undici reads header bytes as latin1, so any byte is a header byte).
#[tokio::test(flavor = "current_thread")]
async fn mcp_a_header_with_a_byte_that_is_not_utf8_does_not_fail_the_post() {
    let port = serve_once(handler(|mut s| async move {
        let mut head = b"HTTP/1.1 200 OK\r\nx-name: caf".to_vec();
        head.push(0xe9);
        head.extend_from_slice(b"\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}");
        let _ = s.write_all(&head).await;
        let _ = s.shutdown().await;
    }))
    .await;
    let (got, _) = posted(port, 3_000).await;
    assert_eq!(got.map(|a| (a.status, a.body)), Ok((200, "{}".to_string())));
}

// TS slate-mcp.ts:264 (reader.read() of the fetch at :416): a body shorter than its content-length rejects with "terminated".
#[tokio::test(flavor = "current_thread")]
async fn mcp_a_body_that_stops_short_of_its_content_length_says_terminated() {
    let port = serve_once(handler(|mut s| async move {
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 1000\r\nconnection: close\r\n\r\n0123456789").await;
        let _ = s.shutdown().await;
    }))
    .await;
    let (got, _) = posted(port, 3_000).await;
    assert_eq!(got.map(|a| a.status), Err("terminated".to_string()));
}

// TS slate-mcp.ts:416: redirect "error" fails on any 301, 302, 303, 307 or 308 answer, with or without a Location.
#[tokio::test(flavor = "current_thread")]
async fn mcp_a_redirect_status_with_no_location_still_fails_the_message() {
    let port = serve_once(handler(|mut s| async move {
        let _ = s.write_all(b"HTTP/1.1 302 Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
        let _ = s.shutdown().await;
    }))
    .await;
    let (got, _) = posted(port, 3_000).await;
    assert_eq!(got.map(|a| a.status), Err("fetch failed".to_string()));
}

// TS slate-mcp.ts:416 (undici sends user-agent: node): a server that refuses a request with no user agent never sees one here.
#[tokio::test(flavor = "current_thread")]
async fn mcp_the_post_names_a_user_agent() {
    let (port, head) = serve_head().await;
    let _ = posted(port, 3_000).await;
    let head = head.lock().unwrap().clone();
    assert!(head.to_ascii_lowercase().contains("\r\nuser-agent:"), "the request head had no user-agent: {head:?}");
}

// ---- part 4: box roads and the sign-in renewal ----

/// Answers the n-th message of a connection as `plan` says: a status after a pause in ms.
struct Planned {
    plan: Vec<(u16, u64)>,
    sent: std::cell::Cell<usize>,
}

impl McpPost for Planned {
    fn send(&self, _t: &McpTransport, _req: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>> {
        let at = self.sent.get();
        self.sent.set(at + 1);
        let (status, pause) = self.plan.get(at).copied().unwrap_or((200, 0));
        async move {
            tokio::time::sleep(Duration::from_millis(pause)).await;
            Ok(McpHttpAnswer { status, content_type: None, session_id: None, body: String::new() })
        }
        .boxed_local()
    }

    fn close(&self) {}
}

// TS slate-mcp-on.ts:104 (`renewing ??= ...finally(() => (renewing = undefined))`): only a refusal that lands while a
// renewal runs shares it; one that lands after it ended asks the agent's own word again.
#[tokio::test(flavor = "current_thread")]
async fn a_refusal_that_lands_after_a_renewal_ended_renews_again() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let renews = Rc::new(std::cell::Cell::new(0u32));
            let counted = renews.clone();
            let sign_in = McpSignIn {
                bearer: serde_json::from_value(json!({ "file": "~/t.json", "at": ["a"], "field": "accessToken" })).unwrap(),
                renew: Rc::new(move || {
                    counted.set(counted.get() + 1);
                    async { true }.boxed_local()
                }),
                line: "claude mcp login x".into(),
                read: None,
            };
            let inner: Rc<dyn McpPost> = Rc::new(Planned { plan: vec![(401, 0), (401, 150)], sent: std::cell::Cell::new(0) });
            let post = renew_on_refusal("box", Some(sign_in), inner);
            let t = McpTransport::Http { url: "http://x".into(), headers: IndexMap::new() };
            let req = || McpHttpRequest { headers: IndexMap::new(), body: "{}".into(), ms: 5_000 };
            let (a, b) = (post.send(&t, req()), post.send(&t, req()));
            let (a, b) = futures::join!(a, b);
            assert_eq!((a.unwrap().status, b.unwrap().status), (200, 200));
            assert_eq!(renews.get(), 2, "the second message was refused 150 ms after the renewal ended");
        })
        .await;
}

// TS slate-mcp.ts:318 (`${command} could not be started: ${e.message}`, node's message is "spawn <command> ENOENT").
#[tokio::test(flavor = "current_thread")]
async fn mcp_a_server_command_that_is_not_there_says_spawn_enoent() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let t = McpTransport::Stdio {
                command: "/nonexistent/wsp-stub".into(),
                args: vec![],
                env: IndexMap::new(),
                cwd: None,
                tool_timeout_sec: None,
            };
            let spec = McpServerSpec {
                transport: t.clone(),
                cwd: "/".into(),
                env: IndexMap::new(),
                secrets: vec![],
                open: None,
                post: None,
                sign_in: None,
            };
            let said = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
            let kept = said.clone();
            let events = McpPipeEvents {
                out: Rc::new(|_: &[u8]| {}),
                err: Rc::new(|_: &str| {}),
                exit: Rc::new(move |why: &str| kept.borrow_mut().push(why.to_string())),
            };
            let _pipe = (spawn_here())(&t, &spec, events);
            for _ in 0..40 {
                if !said.borrow().is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            assert_eq!(*said.borrow(), vec!["/nonexistent/wsp-stub could not be started: spawn /nonexistent/wsp-stub ENOENT".to_string()]);
        })
        .await;
}
