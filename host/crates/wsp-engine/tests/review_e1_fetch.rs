// SPDX-License-Identifier: AGPL-3.0-only
//! Review of packet e1: fetch.rs and errors.rs against node 22 (undici 6.29), each expected value measured on aws-mumbai.
//! The tests that drop the network (`unshare -n`) need root on Linux.

use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::*;
use wsp_engine::fetch::{platform_fetch, FetchRequest, FetchResponse};

async fn read_head(s: &mut TcpStream) -> Vec<u8> {
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    while !got.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut buf).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        got.extend_from_slice(&buf[..n]);
    }
    got
}

/// Serves each connection with `f` until the test ends.
async fn serve<F, Fut>(f: F) -> u16
where
    F: Fn(TcpStream) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let f = std::sync::Arc::new(f);
    tokio::spawn(async move {
        loop {
            let Ok((s, _)) = l.accept().await else { return };
            let f = f.clone();
            tokio::spawn(async move { f(s).await });
        }
    });
    port
}

async fn closed_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    l.local_addr().unwrap().port()
}

fn get(url: &str) -> FetchRequest {
    FetchRequest { method: "GET".into(), url: url.into(), headers: vec![], body: None, abort: None }
}

fn post(url: &str, body: Vec<u8>) -> FetchRequest {
    FetchRequest { method: "POST".into(), url: url.into(), headers: vec![], body: Some(body), abort: None }
}

fn describe(r: &Result<FetchResponse, RoadError>) -> String {
    match r {
        Ok(res) => format!("ok status={} body={}B", res.status, res.body.len()),
        Err(e) => {
            let c = &e.cause;
            format!(
                "name={} msg={:?} code={:?} | cause.msg={:?} cause.code={:?}",
                e.name,
                e.message,
                e.code,
                c.as_ref().map(|c| c.message.clone()),
                c.as_ref().and_then(|c| c.code.clone())
            )
        }
    }
}

fn readings(r: &Result<FetchResponse, RoadError>) -> String {
    match r {
        Ok(_) => "ok".into(),
        Err(e) => {
            let e = EngineError::Road(e.clone());
            format!(
                "isNetworkError={} neverSent={} farEndAnswered={} isCapped={} roadCode={:?} roadFailed={} failureWords={:?}",
                is_network_error(&e),
                never_sent(&e),
                far_end_answered(&e),
                is_capped(&e),
                road_code(&e),
                road_failed(&e),
                failure_words(&e)
            )
        }
    }
}

fn say(label: &str, r: &Result<FetchResponse, RoadError>) {
    println!("RUST {label}: {}\n     {}", describe(r), readings(r));
}

// ---------------------------------------------------------------------------------------------------------------

/// Runs the named test again in a process with no network at all (`unshare -n`: a loopback that is down, no route, no
/// resolver), which is what a computer that is offline looks like. True where this call is the parent and the child
/// passed; the child itself runs the body.
fn offline_child(test: &str) -> bool {
    if std::env::var("E1_OFFLINE_CHILD").is_ok() {
        return false;
    }
    let out = std::process::Command::new("unshare")
        .arg("-n")
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env("E1_OFFLINE_CHILD", "1")
        .output()
        .expect("unshare -n must be runnable (root on a Linux box)");
    println!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the child process with no network failed: {}", out.status);
    true
}

/// node, on a computer with no route out (`unshare -n`): `http://1.2.3.4:81/` is TypeError "fetch failed" whose cause
/// is `connect ENETUNREACH 1.2.3.4:81`, code ENETUNREACH; errors.ts:270 lists ENETUNREACH, so roadCode is
/// "ENETUNREACH" and roadFailed is true.
#[tokio::test]
async fn a_connect_with_no_route_out_reads_as_offline_enetunreach() {
    if offline_child("a_connect_with_no_route_out_reads_as_offline_enetunreach") {
        return;
    }
    let r = platform_fetch().fetch(get("http://1.2.3.4:81/")).await;
    say("offline connect to 1.2.3.4:81", &r);
    let e = EngineError::Road(r.unwrap_err());
    assert_eq!(road_code(&e).as_deref(), Some("ENETUNREACH"));
    assert!(road_failed(&e));
}

/// node, on a computer with no network (`unshare -n`): `http://no-such-host.invalid/` is "fetch failed" with cause
/// `getaddrinfo EAI_AGAIN no-such-host.invalid`, code EAI_AGAIN: the resolver could not be asked. roadCode is
/// "EAI_AGAIN" (the code the retry sentence names), not ENOTFOUND.
#[tokio::test]
async fn a_name_the_resolver_could_not_be_asked_is_eai_again_not_enotfound() {
    if offline_child("a_name_the_resolver_could_not_be_asked_is_eai_again_not_enotfound") {
        return;
    }
    let r = platform_fetch().fetch(get("http://no-such-host.invalid/")).await;
    say("offline name", &r);
    let e = EngineError::Road(r.unwrap_err());
    assert_eq!(road_code(&e).as_deref(), Some("EAI_AGAIN"));
}

/// The coordinator's case (f), node on this box: `http://[2001:db8::1]:81/` is "fetch failed", cause code ENETUNREACH
/// (no IPv6 default route), so roadCode is "ENETUNREACH" and roadFailed is true.
#[tokio::test]
async fn an_address_with_no_route_reads_as_offline_on_this_box() {
    let r = platform_fetch().fetch(get("http://[2001:db8::1]:81/")).await;
    say("f", &r);
    let e = EngineError::Road(r.unwrap_err());
    assert_eq!(road_code(&e).as_deref(), Some("ENETUNREACH"));
    assert!(road_failed(&e));
    assert!(is_network_error(&e));
    assert!(!never_sent(&e));
}

/// The coordinator's case (g), node on this box: `http://10.255.255.1:81/` fails after 10.5 s as "fetch failed" with
/// cause ConnectTimeoutError, code UND_ERR_CONNECT_TIMEOUT, which is in UNSENT_CODES: never_sent is true. Given a
/// 15 s cap the call must have ended on its own before the cap.
#[tokio::test]
async fn a_connect_that_never_completes_gives_up_at_ten_seconds_as_never_sent() {
    let mut req = get("http://10.255.255.1:81/");
    req.abort = abort(Some(15_000.0), None);
    let t = Instant::now();
    let r = platform_fetch().fetch(req).await;
    let took = t.elapsed();
    say("g", &r);
    println!("     (g took {took:?})");
    let road = r.unwrap_err();
    assert_eq!(road.name, "TypeError", "it ended as {} after {took:?}", road.name);
    assert_eq!(road.cause.as_ref().and_then(|c| c.code.as_deref()), Some("UND_ERR_CONNECT_TIMEOUT"));
    let e = EngineError::Road(road);
    assert!(never_sent(&e));
    assert!(is_network_error(&e));
    assert!(took < Duration::from_secs(12), "{took:?}");
}

/// node on this box, no cap, a server that takes the request and never answers: after 301 s the call rejects with
/// "fetch failed", cause HeadersTimeoutError code UND_ERR_HEADERS_TIMEOUT (undici's default headers timeout is 300 s;
/// its body timeout is 300 s of silence too, and its connect timeout 10 s). Run with `--ignored`: it waits 310 s.
#[tokio::test]
#[ignore]
async fn a_server_that_never_answers_ends_the_call_in_about_five_minutes() {
    let p = serve(|s| async move {
        let _keep = s;
        tokio::time::sleep(Duration::from_secs(3600)).await;
    })
    .await;
    let mut req = post(&format!("http://127.0.0.1:{p}/"), b"x".to_vec());
    req.abort = None;
    let t = Instant::now();
    let r = tokio::time::timeout(Duration::from_secs(310), platform_fetch().fetch(req)).await;
    println!("RUST silent server, no cap: {:?} after {:?}", r.as_ref().map(|r| describe(r)), t.elapsed());
    let r = r.expect("still waiting after 310 s: the call has no headers timeout");
    assert_eq!(r.unwrap_err().message, "fetch failed");
}

/// node, a refused connect: `fetch failed: connect ECONNREFUSED 127.0.0.1:<port>`. The address is what a person needs
/// to read in the failure line (failureWords reaches serve.ts's `error` and the host log).
#[tokio::test]
async fn the_failure_words_of_a_refused_connect_name_the_address() {
    let p = closed_port().await;
    let r = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await;
    assert_eq!(failure_words(&EngineError::Road(r.unwrap_err())), format!("fetch failed: connect ECONNREFUSED 127.0.0.1:{p}"));
}

/// node, an unknown host: `fetch failed: getaddrinfo ENOTFOUND no-such-host.invalid`.
#[tokio::test]
async fn the_failure_words_of_an_unknown_host_name_the_host() {
    let r = platform_fetch().fetch(get("http://no-such-host.invalid/")).await;
    assert_eq!(failure_words(&EngineError::Road(r.unwrap_err())), "fetch failed: getaddrinfo ENOTFOUND no-such-host.invalid");
}

/// node, a server that answers 200 with content-length 100, sends 10 bytes and closes: `res.text()` rejects with
/// TypeError "terminated", cause SocketError code UND_ERR_SOCKET ("other side closed"). One that sends a bad chunk size:
/// "terminated", cause HPE_INVALID_CHUNK_SIZE, which is no network code, so isNetworkError is false.
#[tokio::test]
async fn a_body_that_fails_mid_read_is_terminated_with_the_socket_code() {
    let p = serve(|mut s| async move {
        read_head(&mut s).await;
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\n0123456789").await;
        let _ = s.shutdown().await;
    })
    .await;
    let road = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await.unwrap_err();
    assert_eq!((road.name.as_str(), road.message.as_str()), ("TypeError", "terminated"));
    assert_eq!(road.cause.as_ref().and_then(|c| c.code.as_deref()), Some("UND_ERR_SOCKET"));
    assert!(is_network_error(&EngineError::Road(road)));
}

#[tokio::test]
async fn a_body_with_a_bad_chunk_is_not_a_network_error() {
    let p = serve(|mut s| async move {
        read_head(&mut s).await;
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\nZZ\r\n").await;
        let _ = s.shutdown().await;
    })
    .await;
    let road = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await.unwrap_err();
    assert!(!is_network_error(&EngineError::Road(road)));
}

/// node, a server that closes without answering (FIN, no response): cause SocketError code UND_ERR_SOCKET.
#[tokio::test]
async fn a_close_before_any_answer_carries_und_err_socket() {
    let p = serve(|mut s| async move {
        let _ = s.shutdown().await;
    })
    .await;
    let road = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await.unwrap_err();
    assert_eq!(road.cause.as_ref().and_then(|c| c.code.as_deref()), Some("UND_ERR_SOCKET"));
}

/// node, `res.headers.get("x-dup")` of two `x-dup` headers: "a, b" (and retry-after 5 and 9 read "5, 9").
#[test]
fn a_repeated_header_reads_joined_with_a_comma_and_space() {
    let res = FetchResponse { status: 200, headers: vec![("x-dup".into(), "a".into()), ("x-dup".into(), "b".into())], body: vec![] };
    assert_eq!(res.header("x-dup"), Some("a, b"));
}

/// node, `res.text()` of a body that starts with the UTF-8 byte order mark: the mark is stripped, and JSON.parse of the
/// text works.
#[test]
fn a_leading_byte_order_mark_is_not_part_of_the_text() {
    let mut body = vec![0xef, 0xbb, 0xbf];
    body.extend_from_slice(br#"{"a":1}"#);
    let res = FetchResponse { status: 200, headers: vec![], body };
    assert_eq!(res.text(), r#"{"a":1}"#);
}

/// node, a 200 with `content-encoding: gzip` and a gzipped body: `res.text()` is the decoded text (node sends
/// `accept-encoding: gzip, deflate`).
#[tokio::test]
async fn a_gzip_body_is_decoded_as_fetch_does() {
    let p = serve(|mut s| async move {
        use std::io::Write;
        read_head(&mut s).await;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(br#"{"a":1}"#).unwrap();
        let gz = gz.finish().unwrap();
        let mut v =
            format!("HTTP/1.1 200 OK\r\ncontent-encoding: gzip\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", gz.len()).into_bytes();
        v.extend_from_slice(&gz);
        let _ = s.write_all(&v).await;
        let _ = s.shutdown().await;
    })
    .await;
    let res = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await.unwrap();
    assert_eq!(res.text(), r#"{"a":1}"#);
}

/// node, a header whose value has the byte 0xE9: `headers.get` reads it as latin1, "café".
#[tokio::test]
async fn a_header_value_reads_one_character_per_byte() {
    let p = serve(|mut s| async move {
        read_head(&mut s).await;
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nx-l: caf\xe9\r\ncontent-length: 1\r\nconnection: close\r\n\r\nx").await;
        let _ = s.shutdown().await;
    })
    .await;
    let res = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await.unwrap();
    assert_eq!(res.header("x-l"), Some("caf\u{e9}"));
}

/// node sends `user-agent: node` on every request (and `accept-encoding: gzip, deflate`).
#[tokio::test]
async fn a_request_carries_a_user_agent() {
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let p = serve(move |mut s| {
        let tx = tx.clone();
        async move {
            let head = String::from_utf8_lossy(&read_head(&mut s).await).to_lowercase();
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(head);
            }
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
        }
    })
    .await;
    let _ = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await.unwrap();
    let head = rx.await.unwrap();
    assert!(head.contains("\r\nuser-agent: "), "{head:?}");
}

/// node, a 204: `res.body` is null (vault.ts:79 reads null as "no body"); fetch_stream's `body` is `None` there.
#[tokio::test]
async fn a_no_content_answer_has_no_body_stream() {
    let p = serve(|mut s| async move {
        read_head(&mut s).await;
        let _ = s.write_all(b"HTTP/1.1 204 No Content\r\nconnection: close\r\n\r\n").await;
        let _ = s.shutdown().await;
    })
    .await;
    let res = platform_fetch().fetch_stream(get(&format!("http://127.0.0.1:{p}/"))).await.unwrap();
    assert_eq!(res.status, 204);
    assert!(res.body.is_none());
}

/// node, a signal already aborted: fetch rejects with its reason at once and opens no connection.
#[tokio::test]
async fn a_request_whose_caller_cancelled_before_it_began_never_connects() {
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let a2 = accepted.clone();
    let p = serve(move |s| {
        let a2 = a2.clone();
        async move {
            a2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _keep = s;
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    })
    .await;
    let f = platform_fetch();
    for _ in 0..40 {
        let c = Cancel::new();
        c.cancel(Some("already stopped"));
        let mut req = get(&format!("http://127.0.0.1:{p}/"));
        req.abort = abort(None, Some(&c));
        let road = f.fetch(req).await.unwrap_err();
        assert_eq!((road.name.as_str(), road.message.as_str()), ("AbortError", "already stopped"));
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        accepted.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "connections opened for requests that were cancelled before they began"
    );
}

/// node's `AbortSignal.timeout` is an unref'd timer that holds nothing once the call has ended; after 200 fetches
/// that all answered, nothing of theirs is still running.
#[tokio::test]
async fn a_finished_fetch_leaves_no_task_waiting_on_its_cap_or_its_caller() {
    let p = serve(|mut s| async move {
        read_head(&mut s).await;
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
    })
    .await;
    let f = platform_fetch();
    let caller = Cancel::new();
    let before = tokio::runtime::Handle::current().metrics().num_alive_tasks();
    for i in 0..200 {
        let mut req = get(&format!("http://127.0.0.1:{p}/"));
        req.abort = if i % 2 == 0 { abort(Some(3_600_000.0), None) } else { abort(None, Some(&caller)) };
        f.fetch(req).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = tokio::runtime::Handle::current().metrics().num_alive_tasks();
    println!("alive tasks before {before}, after {after}");
    assert!(after < before + 10, "alive tasks {before} -> {after} after 200 finished fetches");
}

/// node's `abort(...)` helper (errors.ts:261) is a plain function: it builds a signal wherever it is called.
#[test]
fn abort_builds_its_signal_outside_a_runtime() {
    let made = std::panic::catch_unwind(|| abort(Some(1000.0), None).is_some());
    assert_eq!(made.ok(), Some(true));
}

/// node, `fetch("not a url")`: TypeError "Failed to parse URL from not a url", cause code ERR_INVALID_URL. It is not
/// "fetch failed" and carries no network code, so isNetworkError is false and untilReached throws it at once.
#[tokio::test]
async fn a_url_that_does_not_parse_is_not_a_network_error() {
    for url in ["not a url", "", "http://", "http://exa mple.com/", "http://127.0.0.1:99999/"] {
        let road = platform_fetch().fetch(get(url)).await.unwrap_err();
        let e = EngineError::Road(road);
        assert!(!is_network_error(&e), "{url:?} read as a network error: {e}");
        assert_ne!(e.to_string(), "fetch failed", "{url:?}");
    }
}

/// node, an `Authorization` value `Bearer abc\n` (a key pasted with its newline): the header's leading and trailing
/// whitespace is stripped and the request goes out with `authorization: Bearer abc`.
#[tokio::test]
async fn a_header_value_is_stripped_of_surrounding_whitespace_as_fetch_does() {
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let p = serve(move |mut s| {
        let tx = tx.clone();
        async move {
            let head = String::from_utf8_lossy(&read_head(&mut s).await).to_string();
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(head);
            }
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
        }
    })
    .await;
    let mut req = get(&format!("http://127.0.0.1:{p}/"));
    req.headers.push(("Authorization".into(), "Bearer abc\n".into()));
    let r = platform_fetch().fetch(req).await;
    assert!(r.is_ok(), "{}", describe(&r));
    let head = rx.await.unwrap().to_lowercase();
    assert!(head.contains("\r\nauthorization: bearer abc\r\n"), "{head:?}");
}

/// node, a header value with a newline inside: TypeError `Headers.append: "..." is an invalid header value.`, which is
/// no network error (the key is wrong, and asking again will not change that).
#[tokio::test]
async fn a_header_value_that_cannot_be_sent_is_not_a_network_error() {
    let p = closed_port().await;
    let mut req = get(&format!("http://127.0.0.1:{p}/"));
    req.headers.push(("Authorization".into(), "Bearer a\nb".into()));
    let road = platform_fetch().fetch(req).await.unwrap_err();
    assert!(!is_network_error(&EngineError::Road(road)));
}

fn env_child(test: &str, vars: &[(&str, &str)]) -> bool {
    if std::env::var("E1_ENV_CHILD").is_ok() {
        return false;
    }
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", test, "--nocapture", "--test-threads=1"]).env("E1_ENV_CHILD", "1");
    for (k, v) in vars {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    println!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the child process failed: {}", out.status);
    true
}

async fn one_plain_answer() -> u16 {
    serve(|mut s| async move {
        read_head(&mut s).await;
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok").await;
    })
    .await
}

/// node's fetch ignores HTTP_PROXY and HTTPS_PROXY unless NODE_USE_ENV_PROXY is set; a plain request to a local server
/// answers whatever they say.
#[tokio::test]
async fn the_proxy_variables_do_not_change_a_plain_fetch() {
    if env_child(
        "the_proxy_variables_do_not_change_a_plain_fetch",
        &[("HTTP_PROXY", "http://127.0.0.1:1"), ("http_proxy", "http://127.0.0.1:1"), ("ALL_PROXY", "http://127.0.0.1:1")],
    ) {
        return;
    }
    let p = one_plain_answer().await;
    let r = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await;
    assert!(r.is_ok(), "{}", describe(&r));
}

/// A computer with no CA bundle at all (SSL_CERT_FILE and SSL_CERT_DIR point nowhere): a plain http fetch still
/// answers; only an https one cannot be verified.
#[tokio::test]
async fn a_computer_with_no_ca_bundle_still_makes_a_plain_fetch() {
    if env_child(
        "a_computer_with_no_ca_bundle_still_makes_a_plain_fetch",
        &[("SSL_CERT_FILE", "/nonexistent/ca.pem"), ("SSL_CERT_DIR", "/nonexistent/certs")],
    ) {
        return;
    }
    let p = one_plain_answer().await;
    let r = platform_fetch().fetch(get(&format!("http://127.0.0.1:{p}/"))).await;
    assert!(r.is_ok(), "{}", describe(&r));
    let r = platform_fetch().fetch(get("https://example.com/")).await;
    println!("RUST https with no CA bundle: {}", describe(&r));
}
