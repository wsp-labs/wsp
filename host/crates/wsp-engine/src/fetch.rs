// SPDX-License-Identifier: AGPL-3.0-only
//! `globalThis.fetch`, the one HTTP seam the provider backends (box-backend.ts, solari-backend.ts) take as an option
//! so a test hands in its own. No TS file of its own: the TS reads the platform's, which is node's undici. This one
//! answers as undici does: its error codes and words, its timeouts, its header and body reading, none of the system's
//! proxy variables.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::cancel::Cancel;
use crate::errors::{RoadCause, RoadError};

/// undici's default connect timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// undici's default headers timeout and body timeout: both wait 300 s of silence.
const SILENCE_TIMEOUT: Duration = Duration::from_secs(300);
/// undici follows 20 redirects.
const REDIRECTS: usize = 20;

/// What ends a fetch early (errors.ts:261's answer): the token fires when either the cap or the caller's signal does,
/// and `timed_out` tells which, so the failure reads "TimeoutError" for the cap and "AbortError" for the caller.
#[derive(Clone)]
pub struct Abort {
    pub token: CancellationToken,
    pub timed_out: Arc<AtomicBool>,
    /// The caller's cancel where one was given, so a fetch it ended rejects with its reason (spec/README.md, "A cancel and
    /// its words").
    pub caller: Option<Cancel>,
    wiring: Arc<Wiring>,
}

/// The one task that fires the token at the cap or at the caller's cancel. It lives as long as any clone of the
/// [`Abort`] does, as the TS's unref'd timer holds nothing once the call is over, and it starts once there is a runtime
/// to start it on.
struct Wiring {
    cap: Option<Duration>,
    caller: Option<Cancel>,
    token: CancellationToken,
    timed_out: Arc<AtomicBool>,
    task: Mutex<Option<tokio::task::AbortHandle>>,
}

impl Wiring {
    fn arm(&self) {
        let mut task = self.task.lock().unwrap();
        if task.is_some() || self.token.is_cancelled() || (self.cap.is_none() && self.caller.is_none()) {
            return;
        }
        let Ok(rt) = tokio::runtime::Handle::try_current() else { return };
        let (cap, caller, token, timed_out) = (self.cap, self.caller.clone(), self.token.clone(), self.timed_out.clone());
        let handle = rt.spawn(async move {
            tokio::select! {
                _ = async { match cap { Some(wait) => tokio::time::sleep(wait).await, None => std::future::pending().await } } => {
                    timed_out.store(true, Ordering::SeqCst);
                    token.cancel();
                }
                _ = async { match &caller { Some(c) => c.cancelled().await, None => std::future::pending().await } } => token.cancel(),
                _ = token.cancelled() => {}
            }
        });
        *task = Some(handle.abort_handle());
    }
}

impl Drop for Wiring {
    fn drop(&mut self) {
        if let Some(task) = self.task.get_mut().unwrap().take() {
            task.abort();
        }
    }
}

impl Abort {
    /// A cap and a caller's cancel as one token. A caller already cancelled has fired it before this returns.
    pub fn new(cap: Option<Duration>, caller: Option<&Cancel>) -> Abort {
        let token = CancellationToken::new();
        let timed_out = Arc::new(AtomicBool::new(false));
        if caller.is_some_and(Cancel::is_cancelled) {
            token.cancel();
        }
        let wiring =
            Arc::new(Wiring { cap, caller: caller.cloned(), token: token.clone(), timed_out: timed_out.clone(), task: Mutex::new(None) });
        wiring.arm();
        Abort { token, timed_out, caller: caller.cloned(), wiring }
    }

    /// Starts the cap's and the caller's watch where `new` had no runtime to start it on.
    pub fn arm(&self) {
        self.wiring.arm();
    }
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

/// `res.headers.get(name)`: case-insensitive, a name sent twice joined with ", ", `None` where absent. A response this
/// module makes holds each name once already; the join below serves one built by hand with a name twice, and keeps its
/// text for the life of the process, as a slice of the response cannot hold two separated values.
fn header_of<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    let mut found = headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
    let first = found.next()?;
    let rest: Vec<&str> = found.collect();
    if rest.is_empty() {
        return Some(first);
    }
    static JOINED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let joined = std::iter::once(first).chain(rest).collect::<Vec<_>>().join(", ");
    let mut seen = JOINED.lock().unwrap();
    if let Some(known) = seen.iter().find(|k| **k == joined) {
        return Some(known);
    }
    let leaked: &'static str = Box::leak(joined.into_boxed_str());
    seen.push(leaked);
    Some(leaked)
}

#[derive(Debug, Clone, PartialEq)]
pub struct FetchResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl FetchResponse {
    /// `res.headers.get(name)`.
    pub fn header(&self, name: &str) -> Option<&str> {
        header_of(&self.headers, name)
    }
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    /// `res.text()`: the body as UTF-8 with invalid sequences replaced, a leading byte order mark dropped.
    pub fn text(&self) -> String {
        let text = String::from_utf8_lossy(&self.body);
        text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
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
        header_of(&self.headers, name)
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
pub fn platform_fetch() -> Arc<dyn Fetch> {
    Arc::new(PlatformFetch { manual: false })
}

/// `globalThis.fetch` with `{ redirect: "manual" }`: a 3xx answer is the answer, its `location` left unfollowed.
pub fn platform_fetch_manual() -> Arc<dyn Fetch> {
    Arc::new(PlatformFetch { manual: true })
}

struct PlatformFetch {
    manual: bool,
}

fn builder(manual: bool) -> reqwest::ClientBuilder {
    let redirects = if manual { reqwest::redirect::Policy::none() } else { reqwest::redirect::Policy::limited(REDIRECTS) };
    reqwest::Client::builder().no_proxy().connect_timeout(CONNECT_TIMEOUT).read_timeout(SILENCE_TIMEOUT).redirect(redirects)
}

/// The client for a plain `http://` request. It holds an empty root store and is never asked to verify a certificate,
/// so a computer with no CA bundle still makes it: nothing is read from the system.
fn plain_client(manual: bool) -> Result<reqwest::Client, RoadError> {
    static CLIENTS: [Mutex<Option<reqwest::Client>>; 2] = [Mutex::new(None), Mutex::new(None)];
    let mut slot = CLIENTS[usize::from(manual)].lock().unwrap();
    if let Some(client) = slot.as_ref() {
        return Ok(client.clone());
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map(|c| c.with_root_certificates(rustls::RootCertStore::empty()).with_no_client_auth());
    let client = roots
        .map_err(|e| e.to_string())
        .and_then(|config| builder(manual).tls_backend_preconfigured(config).build().map_err(|e| e.to_string()))
        .map_err(|why| fetch_failed_with(&why, None))?;
    *slot = Some(client.clone());
    Ok(client)
}

/// The client for an `https://` request: the system's roots, loaded on the first one. A computer with none cannot verify
/// the peer, and node's answer to that is its own error, which this request gets; the next one tries again.
fn tls_client(manual: bool) -> Result<reqwest::Client, RoadError> {
    static CLIENTS: [Mutex<Option<reqwest::Client>>; 2] = [Mutex::new(None), Mutex::new(None)];
    let mut slot = CLIENTS[usize::from(manual)].lock().unwrap();
    if let Some(client) = slot.as_ref() {
        return Ok(client.clone());
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = builder(manual)
        .build()
        .map_err(|_| fetch_failed_with("unable to get local issuer certificate", Some("UNABLE_TO_GET_ISSUER_CERT_LOCALLY")))?;
    *slot = Some(client.clone());
    Ok(client)
}

fn fetch_failed_with(words: &str, code: Option<&str>) -> RoadError {
    RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: words.to_string(), code: code.map(str::to_string), errors: None }),
    }
}

fn type_error(message: String) -> RoadError {
    RoadError { name: "TypeError".into(), message, code: None, cause: None }
}

/// What a cancelled fetch rejects with: the cap's TimeoutError, or the caller's own reason as an AbortError.
fn abort_error(abort: &Abort) -> RoadError {
    if abort.timed_out.load(Ordering::SeqCst) {
        return RoadError {
            name: "TimeoutError".into(),
            message: "The operation was aborted due to timeout".into(),
            code: None,
            cause: None,
        };
    }
    let message = abort.caller.as_ref().and_then(Cancel::given_reason).unwrap_or_else(|| crate::cancel::DEFAULT_REASON.to_string());
    RoadError { name: "AbortError".into(), message, code: None, cause: None }
}

/// Where a request goes, for the words of a failure that names the address.
struct Target {
    host: String,
    port: u16,
}

impl Target {
    fn of(url: &url::Url) -> Target {
        let host = url.host_str().unwrap_or_default().trim_start_matches('[').trim_end_matches(']').to_string();
        Target { host, port: url.port_or_known_default().unwrap_or(80) }
    }

    /// The addresses a connect to this host tried: the literal, or what the name resolves to now.
    async fn addresses(&self) -> Vec<std::net::SocketAddr> {
        if let Ok(ip) = self.host.parse::<std::net::IpAddr>() {
            return vec![std::net::SocketAddr::new(ip, self.port)];
        }
        tokio::net::lookup_host((self.host.as_str(), self.port)).await.map(Iterator::collect).unwrap_or_default()
    }
}

/// node's name for a system error, as `err.code` carries it.
fn errno_code(errno: i32) -> Option<&'static str> {
    Some(match errno {
        libc::ECONNREFUSED => "ECONNREFUSED",
        libc::ECONNRESET => "ECONNRESET",
        libc::ECONNABORTED => "ECONNABORTED",
        libc::ETIMEDOUT => "ETIMEDOUT",
        libc::EPIPE => "EPIPE",
        libc::ENETUNREACH => "ENETUNREACH",
        libc::EHOSTUNREACH => "EHOSTUNREACH",
        libc::ENETDOWN => "ENETDOWN",
        libc::EHOSTDOWN => "EHOSTDOWN",
        libc::EADDRNOTAVAIL => "EADDRNOTAVAIL",
        libc::EAFNOSUPPORT => "EAFNOSUPPORT",
        libc::EACCES => "EACCES",
        libc::EPERM => "EPERM",
        libc::ENOTCONN => "ENOTCONN",
        libc::EMFILE => "EMFILE",
        _ => return None,
    })
}

fn kind_code(kind: std::io::ErrorKind) -> Option<&'static str> {
    use std::io::ErrorKind::*;
    Some(match kind {
        ConnectionRefused => "ECONNREFUSED",
        ConnectionReset => "ECONNRESET",
        TimedOut => "ETIMEDOUT",
        BrokenPipe => "EPIPE",
        _ => return None,
    })
}

/// What a failure's chain says, read once: the system error under it, and whether its words name a lookup that failed,
/// a close, or a bad chunk.
struct Chain {
    code: Option<&'static str>,
    texts: Vec<String>,
}

impl Chain {
    fn of(e: &reqwest::Error) -> Chain {
        let mut code = None;
        let mut texts = Vec::new();
        let mut cur: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(e);
        while let Some(err) = cur {
            texts.push(err.to_string());
            if let Some(io) = err.downcast_ref::<std::io::Error>() {
                code = code.or_else(|| io.raw_os_error().and_then(errno_code)).or_else(|| kind_code(io.kind()));
            }
            cur = err.source();
        }
        Chain { code, texts }
    }

    fn said(&self, needle: &str) -> bool {
        self.texts.iter().any(|t| t.contains(needle))
    }

    fn innermost(&self, fallback: &str) -> String {
        self.texts.last().cloned().unwrap_or_else(|| fallback.to_string())
    }

    /// Whether the far end closed the connection with the answer unfinished (undici's `UND_ERR_SOCKET`).
    fn closed(&self) -> bool {
        self.said("connection closed before message completed")
            || self.said("unexpected end of file")
            || self.said("unexpected EOF")
            || self.said("end of file before message length reached")
    }
}

/// getaddrinfo's refusal as node's code: the resolver that cannot be asked is EAI_AGAIN, a name nobody has ENOTFOUND.
fn lookup_code(chain: &Chain) -> &'static str {
    if chain.said("Temporary failure") {
        "EAI_AGAIN"
    } else if chain.said("Non-recoverable") {
        "EAI_FAIL"
    } else {
        "ENOTFOUND"
    }
}

fn cause(message: String, code: Option<&str>) -> RoadCause {
    RoadCause { message, code: code.map(str::to_string), errors: None }
}

/// A failure before any answer: node's TypeError "fetch failed" with the system error under it.
async fn send_error(e: &reqwest::Error, target: &Target) -> RoadError {
    let chain = Chain::of(e);
    let c = if e.is_timeout() && e.is_connect() {
        let tried = target.addresses().await.first().map_or_else(|| format!("{}:{}", target.host, target.port), |a| a.to_string());
        cause(
            format!("Connect Timeout Error (attempted address: {tried}, timeout: {}ms)", CONNECT_TIMEOUT.as_millis()),
            Some("UND_ERR_CONNECT_TIMEOUT"),
        )
    } else if e.is_timeout() {
        cause("Headers Timeout Error".into(), Some("UND_ERR_HEADERS_TIMEOUT"))
    } else if chain.said("failed to lookup address information") || chain.said("dns error") {
        let code = lookup_code(&chain);
        cause(format!("getaddrinfo {code} {}", target.host), Some(code))
    } else if let (Some(code), true) = (chain.code, e.is_connect()) {
        let tried = target.addresses().await;
        let one = |a: &std::net::SocketAddr| cause(format!("connect {code} {}:{}", a.ip(), a.port()), Some(code));
        match tried.as_slice() {
            [] => cause(format!("connect {code} {}:{}", target.host, target.port), Some(code)),
            [only] => one(only),
            many => RoadCause { message: String::new(), code: Some(code.to_string()), errors: Some(many.iter().map(one).collect()) },
        }
    } else if let Some(code) = chain.code {
        let syscall = if code == "EPIPE" { "write" } else { "read" };
        cause(format!("{syscall} {code}"), Some(code))
    } else if chain.closed() {
        cause("other side closed".into(), Some("UND_ERR_SOCKET"))
    } else if chain.said("invalid chunk size") {
        cause("Invalid character in chunk size".into(), Some("HPE_INVALID_CHUNK_SIZE"))
    } else {
        cause(chain.innermost(&e.to_string()), None)
    };
    RoadError { name: "TypeError".into(), message: "fetch failed".into(), code: None, cause: Some(c) }
}

/// A failure after the headers: node's TypeError "terminated" with the reason under it.
fn body_error(e: &reqwest::Error) -> RoadError {
    let chain = Chain::of(e);
    let c = if e.is_timeout() {
        cause("Body Timeout Error".into(), Some("UND_ERR_BODY_TIMEOUT"))
    } else if let Some(code) = chain.code {
        cause(format!("read {code}"), Some(code))
    } else if chain.said("invalid chunk size") {
        cause("Invalid character in chunk size".into(), Some("HPE_INVALID_CHUNK_SIZE"))
    } else if chain.closed() {
        cause("other side closed".into(), Some("UND_ERR_SOCKET"))
    } else {
        cause(chain.innermost(&e.to_string()), None)
    };
    RoadError { name: "TypeError".into(), message: "terminated".into(), code: None, cause: Some(c) }
}

/// Waits for `work` unless the fetch's token fires first.
async fn until_ended<T>(abort: Option<&Abort>, work: impl std::future::Future<Output = Result<T, RoadError>>) -> Result<T, RoadError> {
    let Some(abort) = abort else { return work.await };
    tokio::select! {
        biased;
        _ = abort.token.cancelled() => Err(abort_error(abort)),
        out = work => out,
    }
}

/// `fetch`'s own checks on a request, in its words: the URL, the method, each header.
struct Prepared {
    method: reqwest::Method,
    url: url::Url,
    headers: reqwest::header::HeaderMap,
    body: Option<Vec<u8>>,
}

fn prepare(req: FetchRequest) -> Result<Prepared, RoadError> {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    let url = url::Url::parse(&req.url).map_err(|_| RoadError {
        name: "TypeError".into(),
        message: format!("Failed to parse URL from {}", req.url),
        code: None,
        cause: Some(cause("Invalid URL".into(), Some("ERR_INVALID_URL"))),
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(fetch_failed_with("unknown scheme", None));
    }
    let upper = req.method.to_ascii_uppercase();
    let method = if ["DELETE", "GET", "HEAD", "OPTIONS", "POST", "PUT"].contains(&upper.as_str()) { upper } else { req.method.clone() };
    let method =
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| type_error(format!("'{}' is not a valid HTTP method.", req.method)))?;
    let mut headers = HeaderMap::new();
    for (name, value) in &req.headers {
        let key = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| type_error(format!("Headers.append: \"{name}\" is an invalid header name.")))?;
        let stripped = value.trim_matches(|c| matches!(c, '\t' | '\n' | '\r' | ' '));
        let bytes: Option<Vec<u8>> = stripped.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect();
        let sent = bytes
            .filter(|b| !b.iter().any(|&x| matches!(x, 0 | b'\r' | b'\n')))
            .and_then(|b| HeaderValue::from_bytes(&b).ok())
            .ok_or_else(|| type_error(format!("Headers.append: \"{value}\" is an invalid header value.")))?;
        headers.append(key, sent);
    }
    headers.entry("user-agent").or_insert(HeaderValue::from_static("node"));
    headers.entry("accept-encoding").or_insert(HeaderValue::from_static("gzip, deflate"));
    Ok(Prepared { method, url, headers, body: req.body })
}

/// `res.headers`: one entry per name, a name sent twice joined with ", ", each value one character per byte.
fn header_pairs(res: &reqwest::Response) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (k, v) in res.headers() {
        let value: String = v.as_bytes().iter().map(|&b| char::from(b)).collect();
        match out.iter_mut().find(|(name, _)| name == k.as_str()) {
            Some((_, held)) => {
                held.push_str(", ");
                held.push_str(&value);
            }
            None => out.push((k.as_str().to_string(), value)),
        }
    }
    out
}

/// A body fetch decodes by its `content-encoding`: gzip and deflate, which is what `accept-encoding` offered.
fn decoded(headers: &[(String, String)], body: Vec<u8>) -> Result<Vec<u8>, RoadError> {
    use std::io::Read;
    let Some(said) = header_of(headers, "content-encoding") else { return Ok(body) };
    let mut body = body;
    for coding in said.split(',').map(|c| c.trim().to_ascii_lowercase()).rev() {
        let mut out = Vec::new();
        let read = match coding.as_str() {
            "gzip" | "x-gzip" => flate2::read::GzDecoder::new(body.as_slice()).read_to_end(&mut out),
            "deflate" => flate2::read::ZlibDecoder::new(body.as_slice()).read_to_end(&mut out).or_else(|_| {
                out.clear();
                flate2::read::DeflateDecoder::new(body.as_slice()).read_to_end(&mut out)
            }),
            _ => continue,
        };
        if read.is_err() {
            return Err(RoadError {
                name: "TypeError".into(),
                message: "terminated".into(),
                code: None,
                cause: Some(cause("incorrect header check".into(), Some("Z_DATA_ERROR"))),
            });
        }
        body = out;
    }
    Ok(body)
}

async fn send(req: FetchRequest, manual: bool) -> Result<(Prepared, reqwest::Response, Target), RoadError> {
    let prepared = prepare(req)?;
    let target = Target::of(&prepared.url);
    let client = if prepared.url.scheme() == "https" { tls_client(manual)? } else { plain_client(manual)? };
    let mut builder = client.request(prepared.method.clone(), prepared.url.clone()).headers(prepared.headers.clone());
    if let Some(body) = prepared.body.clone() {
        builder = builder.body(body);
    }
    match builder.send().await {
        Ok(res) => Ok((prepared, res, target)),
        Err(e) => Err(send_error(&e, &target).await),
    }
}

/// A caller that cancelled before the request began gets its answer without a connection.
fn started(abort: Option<&Abort>) -> Result<(), RoadError> {
    match abort {
        Some(abort) => {
            abort.arm();
            if abort.token.is_cancelled() {
                return Err(abort_error(abort));
            }
            Ok(())
        }
        None => Ok(()),
    }
}

#[async_trait(?Send)]
impl Fetch for PlatformFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let abort = req.abort.clone();
        started(abort.as_ref())?;
        let (_, res, _) = until_ended(abort.as_ref(), send(req, self.manual)).await?;
        let status = res.status().as_u16();
        let headers = header_pairs(&res);
        let body = until_ended(abort.as_ref(), async { res.bytes().await.map_err(|e| body_error(&e)) }).await?;
        let body = decoded(&headers, body.to_vec())?;
        Ok(FetchResponse { status, headers, body })
    }

    async fn fetch_stream(&self, req: FetchRequest) -> Result<FetchStream, RoadError> {
        use futures::StreamExt;
        let abort = req.abort.clone();
        started(abort.as_ref())?;
        let (prepared, res, _) = until_ended(abort.as_ref(), send(req, self.manual)).await?;
        let status = res.status().as_u16();
        let headers = header_pairs(&res);
        if prepared.method == reqwest::Method::HEAD || matches!(status, 101 | 204 | 205 | 304) {
            return Ok(FetchStream { status, headers, body: None });
        }
        let encoded = header_of(&headers, "content-encoding")
            .is_some_and(|e| e.to_ascii_lowercase().contains("gzip") || e.to_ascii_lowercase().contains("deflate"));
        let chunks = res.bytes_stream().map(|c| c.map(|b| b.to_vec()).map_err(|e| body_error(&e)));
        let chunks: futures::stream::LocalBoxStream<'static, Result<Vec<u8>, RoadError>> = if encoded {
            let held = headers.clone();
            Box::pin(futures::stream::once(async move {
                let mut whole = Vec::new();
                let mut chunks = Box::pin(chunks);
                while let Some(chunk) = chunks.next().await {
                    whole.extend_from_slice(&chunk?);
                }
                decoded(&held, whole)
            }))
        } else {
            Box::pin(chunks)
        };
        let body: futures::stream::LocalBoxStream<'static, Result<Vec<u8>, RoadError>> = match abort {
            None => chunks,
            Some(abort) => Box::pin(futures::stream::unfold(Some((chunks, abort)), |state| async move {
                let (mut chunks, abort) = state?;
                tokio::select! {
                    biased;
                    _ = abort.token.cancelled() => Some((Err(abort_error(&abort)), None)),
                    item = chunks.next() => item.map(|item| (item, Some((chunks, abort)))),
                }
            })),
        };
        Ok(FetchStream { status, headers, body: Some(body) })
    }
}
