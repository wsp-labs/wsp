// SPDX-License-Identifier: AGPL-3.0-only
//! `globalThis.fetch`, the one HTTP seam the provider backends (box-backend.ts, solari-backend.ts) take as an option
//! so a test hands in its own. No TS file of its own: the TS reads the platform's.
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::cancel::Cancel;

use crate::errors::{RoadCause, RoadError};

/// What ends a fetch early (errors.ts:261's answer): the token fires when either the cap or the caller's signal does,
/// and `timed_out` tells which, so the failure reads "TimeoutError" for the cap and "AbortError" for the caller.
#[derive(Clone)]
pub struct Abort {
    pub token: CancellationToken,
    pub timed_out: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The caller's cancel where one was given, so a fetch it ended rejects with its reason (spec/README.md, "A cancel and
    /// its words").
    pub caller: Option<Cancel>,
}

#[derive(Clone)]
pub struct FetchRequest {
    pub method: String,
    pub url: String,
    /// In the order the TS object literal writes them.
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub abort: Option<Abort>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl FetchResponse {
    /// `res.headers.get(name)`: case-insensitive, `None` where absent.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    /// `res.text()`: the body as UTF-8, invalid sequences replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// A response whose body is read chunk by chunk (vault.ts's download). `body` is `None` where fetch's `res.body` is
/// null.
pub struct FetchStream {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<futures::stream::LocalBoxStream<'static, Result<Vec<u8>, RoadError>>>,
}

impl FetchStream {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// A fetch that rejects answers a [`RoadError`]: "fetch failed" with its cause, or a TimeoutError or AbortError.
#[async_trait(?Send)]
pub trait Fetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError>;
    /// The same request with the body as a stream; by default the whole body as one chunk.
    async fn fetch_stream(&self, req: FetchRequest) -> Result<FetchStream, RoadError> {
        let res = self.fetch(req).await?;
        let chunk: Result<Vec<u8>, RoadError> = Ok(res.body);
        Ok(FetchStream { status: res.status, headers: res.headers, body: Some(Box::pin(futures::stream::iter(vec![chunk]))) })
    }
}

/// `globalThis.fetch`: the platform's own HTTP client.
pub fn platform_fetch() -> std::sync::Arc<dyn Fetch> {
    std::sync::Arc::new(PlatformFetch)
}

/// The HTTP client every platform fetch shares. Built on the first call, with the ring provider installed for the
/// process: the client's rustls has none of its own (reqwest's `rustls-no-provider`).
fn shared_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::builder().build().expect("the HTTP client builds with its default settings")
    })
}

struct PlatformFetch;

/// What a cancelled fetch rejects with: the cap's TimeoutError, or the caller's own reason as an AbortError.
fn abort_error(abort: &Abort) -> RoadError {
    if abort.timed_out.load(std::sync::atomic::Ordering::SeqCst) {
        return RoadError { name: "TimeoutError".into(), message: "The operation was aborted due to timeout".into(), code: None, cause: None };
    }
    let message = abort.caller.as_ref().and_then(Cancel::given_reason).unwrap_or_else(|| crate::cancel::DEFAULT_REASON.to_string());
    RoadError { name: "AbortError".into(), message, code: None, cause: None }
}

/// node's fetch rejects with TypeError "fetch failed" and the system error under it as its cause.
fn fetch_failed(e: &reqwest::Error) -> RoadError {
    let mut code = None;
    let mut inner = None;
    let mut cur: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(e);
    while let Some(err) = cur {
        inner = Some(err.to_string());
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            code = io_code(io.kind()).map(str::to_string);
        }
        cur = err.source();
    }
    let words = inner.unwrap_or_else(|| e.to_string());
    if code.is_none() && (words.contains("dns error") || words.contains("failed to lookup address")) {
        code = Some("ENOTFOUND".to_string());
    }
    RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: words, code, errors: None }),
    }
}

fn io_code(kind: std::io::ErrorKind) -> Option<&'static str> {
    use std::io::ErrorKind::*;
    Some(match kind {
        ConnectionRefused => "ECONNREFUSED",
        ConnectionReset => "ECONNRESET",
        TimedOut => "ETIMEDOUT",
        BrokenPipe => "EPIPE",
        _ => return None,
    })
}

/// Waits for `work` unless the fetch's token fires first.
async fn until_ended<T>(abort: Option<&Abort>, work: impl std::future::Future<Output = Result<T, RoadError>>) -> Result<T, RoadError> {
    let Some(abort) = abort else { return work.await };
    tokio::select! {
        out = work => out,
        _ = abort.token.cancelled() => Err(abort_error(abort)),
    }
}

async fn send(req: FetchRequest) -> Result<reqwest::Response, RoadError> {
    let method = reqwest::Method::from_bytes(req.method.as_bytes()).map_err(|_| RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: format!("invalid method {}", req.method), code: None, errors: None }),
    })?;
    let mut builder = shared_client().request(method, &req.url);
    for (name, value) in &req.headers {
        builder = builder.header(name, value);
    }
    if let Some(body) = req.body {
        builder = builder.body(body);
    }
    builder.send().await.map_err(|e| fetch_failed(&e))
}

fn header_pairs(res: &reqwest::Response) -> Vec<(String, String)> {
    res.headers().iter().map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned())).collect()
}

#[async_trait(?Send)]
impl Fetch for PlatformFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let abort = req.abort.clone();
        let res = until_ended(abort.as_ref(), send(req)).await?;
        let status = res.status().as_u16();
        let headers = header_pairs(&res);
        let body = until_ended(abort.as_ref(), async { res.bytes().await.map_err(|e| fetch_failed(&e)) }).await?;
        Ok(FetchResponse { status, headers, body: body.to_vec() })
    }

    async fn fetch_stream(&self, req: FetchRequest) -> Result<FetchStream, RoadError> {
        use futures::StreamExt;
        let abort = req.abort.clone();
        let res = until_ended(abort.as_ref(), send(req)).await?;
        let status = res.status().as_u16();
        let headers = header_pairs(&res);
        let chunks = res.bytes_stream().map(|c| c.map(|b| b.to_vec()).map_err(|e| fetch_failed(&e)));
        let body: futures::stream::LocalBoxStream<'static, Result<Vec<u8>, RoadError>> = match abort {
            None => Box::pin(chunks),
            Some(abort) => Box::pin(futures::stream::unfold(Some((Box::pin(chunks), abort)), |state| async move {
                let (mut chunks, abort) = state?;
                tokio::select! {
                    item = chunks.next() => item.map(|item| (item, Some((chunks, abort)))),
                    _ = abort.token.cancelled() => Some((Err(abort_error(&abort)), None)),
                }
            })),
        };
        Ok(FetchStream { status, headers, body: Some(body) })
    }
}
