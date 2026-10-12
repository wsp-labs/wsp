// SPDX-License-Identifier: AGPL-3.0-only
//! http.post: one POST to an http MCP server, made from this computer for a slate whose thread runs here. The address
//! is dialled from here, so a server on this computer's own loopback answers, and the bearer of a sign-in the agent
//! keeps here is read here, as the home's owner and through no link, and goes on the request alone: it never leaves
//! this computer, not even in the answer. HTTP/1.1, one connection per request, closed after the answer, and no
//! redirect followed, so the headers reach the address named and nothing else.

use std::collections::BTreeMap;
use std::io::Read;
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use nix::fcntl::{open, OFlag};
use nix::sys::stat::{fstat, Mode};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use wsp_frames::{numbers, words, DaemonErrorCode, DaemonOp, HttpBearer, HttpPostReply, RequestId};

use crate::ops::{answer, not_built, refuse, Conn};
use crate::paths::OpError;
use crate::Ctx;

/// A call's deadline where the host names none: the host's own for a tool call.
const DEFAULT_MS: u64 = 60_000;
/// The most a status line and its headers may be.
const HEAD_MAX: usize = 64 * 1024;
/// The most an agent's sign-in file is read to: Claude Code's holds a few entries of a few hundred bytes each.
const SIGN_IN_MAX: u64 = 1024 * 1024;
/// Names the request line and the body's length own, which a config's headers never set.
const OWN_HEADERS: [&str; 4] = ["host", "content-length", "connection", "transfer-encoding"];

pub(crate) async fn serve(conn: &Arc<Conn>, ctx: &Ctx, id: Option<RequestId>, name: &str, frame: &Value) -> String {
    match serde_json::from_value::<DaemonOp>(frame.clone()) {
        Ok(DaemonOp::HttpPost { url, headers, body, bearer, timeout_ms }) => {
            // The home's sign-ins are the person's: the host asks on its own road, a socket inside a workspace never.
            if !crate::roads::host_road_serves(&conn.road, ctx.is_place()) {
                return refuse(id, DaemonErrorCode::Forbidden, words::NOT_ON_THIS_ROAD);
            }
            let home = crate::place::place_home(ctx.options.home.as_deref());
            let ms = timeout_ms.map_or(DEFAULT_MS, u64::from);
            answer(id, post(&home, &url, &headers, body, bearer, Duration::from_millis(ms)).await)
        }
        Ok(_) => refuse(id, DaemonErrorCode::Unsupported, not_built(name)),
        Err(e) => refuse(id, DaemonErrorCode::BadRequest, e.to_string()),
    }
}

/// The POST and its answer, the sign-in read first where one is named and no Authorization header is given.
pub(crate) async fn post(
    home: &Path,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: String,
    bearer: Option<HttpBearer>,
    within: Duration,
) -> Result<HttpPostReply, OpError> {
    let target = target(url).map_err(|e| OpError::coded(DaemonErrorCode::BadRequest, e))?;
    if let Some((name, _)) = headers.iter().find(|(k, v)| !is_token(k) || v.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0)) {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("header {name:?} is not one a request can carry")));
    }
    let given = headers.keys().any(|k| k.eq_ignore_ascii_case("authorization"));
    let token = match bearer {
        Some(b) if !given => {
            let home = home.to_path_buf();
            tokio::task::spawn_blocking(move || bearer_of(&home, &b)).await.map_err(|e| OpError::plain(e.to_string()))??
        }
        _ => None,
    };
    let mut head =
        format!("POST {} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nConnection: close\r\n", target.path, target.authority, body.len());
    for (k, v) in headers {
        if !OWN_HEADERS.contains(&k.to_ascii_lowercase().as_str()) {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
    }
    if let Some(token) = token.filter(|t| !t.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0)) {
        head.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    head.push_str("\r\n");
    let mut request = head.into_bytes();
    request.extend_from_slice(body.as_bytes());
    let exchange = async {
        let tcp = TcpStream::connect((target.host.as_str(), target.port))
            .await
            .map_err(|e| format!("{} did not answer: {e}", target.authority))?;
        if !target.tls {
            return exchange(tcp, &request).await;
        }
        let name = rustls::pki_types::ServerName::try_from(target.host.clone()).map_err(|e| format!("{}: {e}", target.host))?;
        let tls =
            tokio_rustls::TlsConnector::from(tls_config()).connect(name, tcp).await.map_err(|e| format!("{}: {e}", target.authority))?;
        exchange(tls, &request).await
    };
    match tokio::time::timeout(within, exchange).await {
        Ok(answer) => answer.map_err(OpError::plain),
        Err(_) => Err(OpError::plain(format!("{} did not answer within {} s", target.authority, within.as_secs()))),
    }
}

/// The bearer the sign-in file holds at the entry named, read as the home's owner; none where the file, the entry
/// or the token is not there. A link on the way, a file past the cap or a file that is no JSON is refused.
fn bearer_of(home: &Path, b: &HttpBearer) -> Result<Option<String>, OpError> {
    let Some(rel) = b.file.strip_prefix("~/") else {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("{} is not a file under the home", b.file)));
    };
    let held = open(home, OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC, Mode::empty())
        .map_err(|e| OpError::plain(format!("{}: {e}", home.display())))?;
    let owner = fstat(held.as_fd()).map_err(|e| OpError::plain(format!("{}: {e}", home.display())))?;
    // A switch that did not take reads nothing: the read is never made as root over a home somebody else owns.
    let _owner = crate::usage_logs::as_owner(owner.st_uid, owner.st_gid)
        .map_err(|()| OpError::plain(format!("{} could not be read as its owner", b.file)))?;
    let Some(file) = crate::beneath::file(&held, rel).map_err(OpError::plain)? else { return Ok(None) };
    let mut text = String::new();
    file.take(SIGN_IN_MAX + 1).read_to_string(&mut text).map_err(|e| OpError::plain(format!("{}: {e}", b.file)))?;
    if text.len() as u64 > SIGN_IN_MAX {
        return Err(OpError::plain(format!("{} is over {SIGN_IN_MAX} bytes", b.file)));
    }
    let json: Value = serde_json::from_str(&text).map_err(|_| OpError::plain(format!("{} is not JSON", b.file)))?;
    let entry = b.at.iter().try_fold(&json, |at, key| at.get(key));
    Ok(entry.and_then(|e| e.get(&b.field)).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_owned))
}

/// The roots every TLS dial trusts, the ones the daemon's own wss dial carries.
fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    Arc::clone(CONFIG.get_or_init(|| {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("ring offers the default protocol versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Arc::new(config)
    }))
}

/// The request written, then the answer read to its end: its length, its chunks, or the connection's close.
async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(mut stream: S, request: &[u8]) -> Result<HttpPostReply, String> {
    stream.write_all(request).await.map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    loop {
        let (status, content_type, session_id, length, chunked, at) = loop {
            let mut found = [httparse::EMPTY_HEADER; 64];
            let mut res = httparse::Response::new(&mut found);
            match res.parse(&buf).map_err(|e| format!("the server's answer could not be read: {e}"))? {
                httparse::Status::Complete(at) => {
                    let header = |name: &str| {
                        res.headers
                            .iter()
                            .find(|h| h.name.eq_ignore_ascii_case(name))
                            .map(|h| String::from_utf8_lossy(h.value).trim().to_owned())
                    };
                    let chunked = header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
                    let length = header("content-length").and_then(|v| v.parse::<usize>().ok());
                    break (res.code.unwrap_or(0), header("content-type"), header("mcp-session-id"), length, chunked, at);
                }
                httparse::Status::Partial if buf.len() > HEAD_MAX => return Err(format!("the server's headers are over {HEAD_MAX} bytes")),
                httparse::Status::Partial => {
                    if read_more(&mut stream, &mut buf).await? == 0 {
                        return Err("the server closed the connection before it answered".to_owned());
                    }
                }
            }
        };
        buf.drain(..at);
        // An interim answer is followed by the real one on the same connection.
        if (100..200).contains(&status) {
            continue;
        }
        let body = if status == 204 || status == 304 {
            Vec::new()
        } else if chunked {
            unchunked(&mut stream, buf).await?
        } else {
            sized(&mut stream, buf, length).await?
        };
        return Ok(HttpPostReply { status, content_type, session_id, body: String::from_utf8_lossy(&body).into_owned() });
    }
}

fn too_big() -> String {
    format!("the server sent a message over {} bytes", numbers::EXEC_OUTPUT_MAX)
}

async fn read_more<S: AsyncRead + Unpin>(stream: &mut S, buf: &mut Vec<u8>) -> Result<usize, String> {
    let mut chunk = [0u8; 16 * 1024];
    let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
    buf.extend_from_slice(&chunk[..n]);
    if buf.len() > numbers::EXEC_OUTPUT_MAX + HEAD_MAX {
        return Err(too_big());
    }
    Ok(n)
}

/// A body of a known length, or to the close where none is said.
async fn sized<S: AsyncRead + Unpin>(stream: &mut S, mut buf: Vec<u8>, length: Option<usize>) -> Result<Vec<u8>, String> {
    if length.is_some_and(|n| n > numbers::EXEC_OUTPUT_MAX) {
        return Err(too_big());
    }
    while length.is_none_or(|n| buf.len() < n) {
        if read_more(stream, &mut buf).await? == 0 {
            if length.is_some() {
                return Err("the server closed the connection inside its answer".to_owned());
            }
            break;
        }
    }
    if let Some(n) = length {
        buf.truncate(n);
    }
    if buf.len() > numbers::EXEC_OUTPUT_MAX {
        return Err(too_big());
    }
    Ok(buf)
}

/// A chunked body joined, to its last chunk.
async fn unchunked<S: AsyncRead + Unpin>(stream: &mut S, mut buf: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    loop {
        let line = loop {
            if let Some(at) = buf.windows(2).position(|w| w == b"\r\n") {
                break at;
            }
            if read_more(stream, &mut buf).await? == 0 {
                return Err("the server closed the connection inside its answer".to_owned());
            }
        };
        let size_text = String::from_utf8_lossy(&buf[..line]).into_owned();
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| format!("a chunk's size could not be read: {size_text:?}"))?;
        if size == 0 {
            return Ok(body);
        }
        if body.len() + size > numbers::EXEC_OUTPUT_MAX {
            return Err(too_big());
        }
        while buf.len() < line + 2 + size + 2 {
            if read_more(stream, &mut buf).await? == 0 {
                return Err("the server closed the connection inside its answer".to_owned());
            }
        }
        body.extend_from_slice(&buf[line + 2..line + 2 + size]);
        buf.drain(..line + 2 + size + 2);
    }
}

/// Whether a header's name is one HTTP takes: a token, no separators or spaces.
fn is_token(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

#[derive(Debug, PartialEq)]
struct Target {
    tls: bool,
    /// The host and port as the address names them, for the Host header.
    authority: String,
    host: String,
    port: u16,
    /// The path and query, never the fragment.
    path: String,
}

/// An http or https address read for a dial; one carrying a login is refused, since its password would go out as
/// the Host header's neighbour.
fn target(url: &str) -> Result<Target, String> {
    let lower = url.to_ascii_lowercase();
    let (tls, rest) = if lower.starts_with("https://") {
        (true, &url[8..])
    } else if lower.starts_with("http://") {
        (false, &url[7..])
    } else {
        return Err(format!("{url} is not an http or https address"));
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.is_empty() || authority.contains('@') {
        return Err(format!("{url} names no host this daemon dials"));
    }
    let tail = tail.split('#').next().unwrap_or_default();
    let path = if tail.is_empty() {
        "/".to_owned()
    } else if tail.starts_with('?') {
        format!("/{tail}")
    } else {
        tail.to_owned()
    };
    let (host, port) = match authority.strip_prefix('[') {
        Some(v6) => {
            let close = v6.find(']').ok_or_else(|| format!("{url} names no host this daemon dials"))?;
            (v6[..close].to_owned(), v6[close + 1..].strip_prefix(':'))
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host.to_owned(), Some(port)),
            None => (authority.to_owned(), None),
        },
    };
    let port = match port {
        Some(p) => p.parse::<u16>().map_err(|_| format!("{url} names no port this daemon dials"))?,
        None if tls => 443,
        None => 80,
    };
    Ok(Target { tls, authority: authority.to_owned(), host, port, path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tokio::net::TcpListener;

    #[test]
    fn an_address_reads_as_its_scheme_host_port_and_path_and_one_with_a_login_is_refused() {
        let t = target("https://mcp.example.com/mcp?x=1#frag").unwrap();
        assert_eq!(
            t,
            Target { tls: true, authority: "mcp.example.com".into(), host: "mcp.example.com".into(), port: 443, path: "/mcp?x=1".into() }
        );
        let t = target("http://127.0.0.1:8931").unwrap();
        assert_eq!((t.tls, t.host.as_str(), t.port, t.path.as_str()), (false, "127.0.0.1", 8931, "/"));
        let t = target("HTTP://[::1]:9/mcp").unwrap();
        assert_eq!((t.host.as_str(), t.port, t.authority.as_str()), ("::1", 9, "[::1]:9"));
        assert!(target("http://ada:pw@example.com/").is_err());
        assert!(target("ftp://example.com/").is_err());
        assert!(target("http://example.com:99999/").is_err());
    }

    /// A server on this computer's loopback that answers each request with `answer`, handing back what it was sent.
    async fn loopback(answer: &'static [u8]) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let seen = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut got = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = s.read(&mut chunk).await.unwrap();
                got.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&got);
                if let Some(at) = text.find("\r\n\r\n") {
                    let length = text
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if got.len() >= at + 4 + length {
                        break;
                    }
                }
            }
            s.write_all(answer).await.unwrap();
            s.shutdown().await.unwrap();
            String::from_utf8_lossy(&got).into_owned()
        });
        (url, seen)
    }

    fn home_with(file: &str, text: &str) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::File::create(path).unwrap().write_all(text.as_bytes()).unwrap();
        home
    }

    fn claude_bearer() -> HttpBearer {
        HttpBearer {
            file: "~/.claude/.credentials.json".into(),
            at: vec!["mcpOAuth".into(), "acme|0123456789abcdef".into()],
            field: "accessToken".into(),
        }
    }

    #[tokio::test]
    async fn a_loopback_server_is_posted_to_with_the_sign_in_kept_in_the_home_and_its_answer_comes_back() {
        let home = home_with(
            ".claude/.credentials.json",
            r#"{"mcpOAuth":{"acme|0123456789abcdef":{"accessToken":"tok_TESTONLY_7d1e","expiresAt":1}}}"#,
        );
        let (url, seen) = loopback(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: s-1\r\nContent-Length: 36\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}").await;
        let headers = BTreeMap::from([("Accept".to_owned(), "application/json, text/event-stream".to_owned())]);
        let got = post(
            home.path(),
            &url,
            &headers,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#.into(),
            Some(claude_bearer()),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert_eq!(
            got,
            HttpPostReply {
                status: 200,
                content_type: Some("application/json".into()),
                session_id: Some("s-1".into()),
                body: r#"{"jsonrpc":"2.0","id":1,"result":{}}"#.into()
            }
        );
        let sent = seen.await.unwrap();
        assert!(sent.starts_with("POST /mcp HTTP/1.1\r\n"), "{sent}");
        assert!(sent.contains("\r\nAuthorization: Bearer tok_TESTONLY_7d1e\r\n"), "{sent}");
        assert!(sent.contains("\r\nAccept: application/json, text/event-stream\r\n"), "{sent}");
        assert!(sent.ends_with(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#), "{sent}");
    }

    #[tokio::test]
    async fn a_chunked_event_stream_is_joined_and_a_missing_sign_in_sends_no_bearer() {
        let home = home_with(".claude/.credentials.json", r#"{"mcpOAuth":{"other|0":{"accessToken":"tok_TESTONLY_x"}}}"#);
        let (url, seen) = loopback(b"HTTP/1.1 401 Unauthorized\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n6\r\ndata: \r\n3;x=1\r\n{}\n\r\n0\r\n\r\n").await;
        let got = post(home.path(), &url, &BTreeMap::new(), "{}".into(), Some(claude_bearer()), Duration::from_secs(5)).await.unwrap();
        assert_eq!((got.status, got.body.as_str()), (401, "data: {}\n"));
        assert!(!seen.await.unwrap().contains("Authorization"));
    }

    #[tokio::test]
    async fn a_given_authorization_header_stands_and_the_sign_in_is_not_read() {
        let home = home_with(".claude/.credentials.json", "not json");
        let (url, seen) = loopback(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n").await;
        let headers = BTreeMap::from([("authorization".to_owned(), "Bearer from-config".to_owned())]);
        let got = post(home.path(), &url, &headers, "{}".into(), Some(claude_bearer()), Duration::from_secs(5)).await.unwrap();
        assert_eq!((got.status, got.body.as_str()), (202, ""));
        let sent = seen.await.unwrap();
        assert_eq!(sent.matches("uthorization").count(), 1, "{sent}");
        assert!(sent.contains("authorization: Bearer from-config\r\n"), "{sent}");
    }

    #[test]
    fn a_sign_in_file_behind_a_link_is_none_one_past_the_cap_or_not_json_is_refused_and_one_not_there_is_none() {
        let home = home_with("elsewhere/.credentials.json", r#"{"mcpOAuth":{"acme|0123456789abcdef":{"accessToken":"tok_TESTONLY_9"}}}"#);
        std::os::unix::fs::symlink(home.path().join("elsewhere"), home.path().join(".claude")).unwrap();
        assert_eq!(bearer_of(home.path(), &claude_bearer()).unwrap(), None);
        std::fs::remove_file(home.path().join(".claude")).unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        std::os::unix::fs::symlink(home.path().join("elsewhere/.credentials.json"), home.path().join(".claude/.credentials.json")).unwrap();
        assert!(bearer_of(home.path(), &claude_bearer()).unwrap_err().message.contains("a link stands"));
        let home = home_with(".claude/.credentials.json", "{");
        assert!(bearer_of(home.path(), &claude_bearer()).unwrap_err().message.contains("is not JSON"));
        let home = home_with(".claude/.credentials.json", &" ".repeat(SIGN_IN_MAX as usize + 1));
        assert!(bearer_of(home.path(), &claude_bearer()).unwrap_err().message.contains("is over"));
        let home = tempfile::tempdir().unwrap();
        assert_eq!(bearer_of(home.path(), &claude_bearer()).unwrap(), None);
        let home = home_with(".claude/.credentials.json", r#"{"mcpOAuth":{"acme|0123456789abcdef":{"accessToken":""}}}"#);
        assert_eq!(bearer_of(home.path(), &claude_bearer()).unwrap(), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn as_root_the_sign_in_is_read_as_the_homes_owner_so_a_root_only_file_reads_nothing() {
        if !nix::unistd::geteuid().is_root() {
            eprintln!("not root; a read as the home's owner is the read this daemon already makes");
            return;
        }
        const NOBODY: u32 = 65534;
        let home = home_with(".claude/.credentials.json", r#"{"mcpOAuth":{"acme|0123456789abcdef":{"accessToken":"tok_TESTONLY_root"}}}"#);
        let file = home.path().join(".claude/.credentials.json");
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
        for path in [home.path().to_path_buf(), home.path().join(".claude")] {
            std::os::unix::fs::chown(&path, Some(NOBODY), Some(NOBODY)).unwrap();
        }
        assert!(bearer_of(home.path(), &claude_bearer()).unwrap_err().message.contains("Permission denied"));
        std::os::unix::fs::chown(&file, Some(NOBODY), Some(NOBODY)).unwrap();
        assert_eq!(bearer_of(home.path(), &claude_bearer()).unwrap().as_deref(), Some("tok_TESTONLY_root"));
    }

    #[tokio::test]
    async fn a_place_daemons_own_inbound_socket_is_refused() {
        use crate::ops::{tests, Road};
        let (b, (inbound, _rx)) = (tests::place_bench(), tests::conn_on(None, Road::Inbound));
        let frame = serde_json::json!({"id": 1, "op": "http.post", "url": "http://127.0.0.1:1/", "headers": {}, "body": ""});
        assert_eq!(tests::reply(&b, &inbound, frame).await["error"], words::NOT_ON_THIS_ROAD);
    }

    #[tokio::test]
    async fn a_header_that_would_break_the_request_is_refused_before_anything_is_dialled() {
        let home = tempfile::tempdir().unwrap();
        for (k, v) in [("X-A", "b\r\nHost: evil"), ("Bad Name", "v")] {
            let headers = BTreeMap::from([(k.to_owned(), v.to_owned())]);
            let e = post(home.path(), "http://127.0.0.1:1/", &headers, String::new(), None, Duration::from_secs(1)).await.unwrap_err();
            assert!(e.message.contains("is not one a request can carry"), "{}", e.message);
        }
    }
}
