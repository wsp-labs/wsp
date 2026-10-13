// SPDX-License-Identifier: AGPL-3.0-only
//! The pipes and the doors on a real daemon: the host's side played by WebSockets dialled with `pipe` on their auth
//! frame, and an agent's side by plain connections to the HTTP door and the gh door.

use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::net::{TcpStream, UnixStream};
use tokio::time::{timeout, Instant};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use wsp_daemon::{Daemon, Doors, Options};
use wsp_frames::{numbers, words};

const TOKEN: &str = "test-token-123";
const WAIT: Duration = Duration::from_secs(10);
/// How long a case watches a pipe to say nothing reached it.
const QUIET: Duration = Duration::from_millis(1500);

type Pipe = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Running {
    addr: SocketAddr,
    http: SocketAddr,
    gh: PathBuf,
    root: tempfile::TempDir,
    token: tempfile::NamedTempFile,
    _wsp: tempfile::TempDir,
}

/// A cloud guest's daemon on the loopback with both doors, the HTTP one on a port the kernel picks and the gh one
/// in a fresh folder, holding a connection as long as the case says.
async fn start(hold_max: Duration) -> Running {
    start_at(hold_max, "127.0.0.1:0".parse().unwrap()).await
}

/// The same with the HTTP door asked for at the address given.
async fn start_at(hold_max: Duration, http: SocketAddr) -> Running {
    let mut token = tempfile::NamedTempFile::new().unwrap();
    writeln!(token, "{TOKEN}").unwrap();
    let root = tempfile::tempdir().unwrap();
    let wsp = tempfile::tempdir().unwrap();
    let gh = wsp.path().join("broker.sock");
    let mut options = Options::new(token.path());
    options.readings_interval_ms = Some(86_400_000);
    options.host = "127.0.0.1".to_owned();
    options.port = 0;
    options.root = Some(root.path().to_path_buf());
    // The guest's own roots file sits under /root, which a runner that is not root cannot read.
    options.roots_path = Some(wsp.path().join("roots"));
    options.manifest_path = Some(root.path().join("manifest.json"));
    options.doors = Some(Doors { http, gh: gh.clone(), hold_max });
    let daemon = Daemon::bind_with(options, Box::new(|_| {})).await.unwrap();
    let addr = daemon.local_addr();
    let http = daemon.http_door().unwrap_or(http);
    tokio::spawn(daemon.run());
    Running { addr, http, gh, root, token, _wsp: wsp }
}

/// One pipe as the host dials it: the auth frame with `pipe`, answered as every auth frame is, and then nothing until
/// a connection takes it.
async fn pipe(addr: SocketAddr) -> Pipe {
    let (mut ws, _) = connect_async(format!("ws://{addr}/")).await.unwrap();
    ws.send(Message::text(json!({"id": 0, "op": "auth", "token": TOKEN, "pipe": true}).to_string())).await.unwrap();
    match next(&mut ws, WAIT).await {
        Some(Message::Text(t)) => assert_eq!(serde_json::from_str::<Value>(&t).unwrap(), json!({"id": 0, "ok": true})),
        other => panic!("the pipe's auth frame was not answered: {other:?}"),
    }
    ws
}

/// The next frame on a pipe, or nothing within the wait.
async fn next(ws: &mut Pipe, wait: Duration) -> Option<Message> {
    loop {
        match timeout(wait, ws.next()).await {
            Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => continue,
            Ok(Some(Ok(message))) => return Some(message),
            _ => return None,
        }
    }
}

/// The open frame a pipe hears when a connection takes it, which is the first frame after the auth answer: a hello
/// there would read to the host as a daemon that serves no pipes.
async fn opened(ws: &mut Pipe) -> Value {
    let open: Value = match next(ws, WAIT).await {
        Some(Message::Text(t)) => serde_json::from_str(&t).unwrap(),
        other => panic!("no pipe.open on the pipe: {other:?}"),
    };
    assert_eq!(open["op"], json!("pipe.open"), "{open}");
    open
}

/// The pipe from here on as the byte stream it carries, the way the host's broker reads it.
fn bytes_of(ws: Pipe) -> DuplexStream {
    let (ours, theirs) = tokio::io::duplex(1 << 20);
    let (mut read, mut write) = tokio::io::split(theirs);
    let (mut sink, mut stream) = ws.split();
    tokio::spawn(async move {
        while let Some(Ok(message)) = stream.next().await {
            match message {
                Message::Binary(b) => {
                    if write.write_all(&b).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        let _ = write.shutdown().await;
    });
    tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        while let Ok(n) = read.read(&mut buf).await {
            if n == 0 || sink.send(Message::binary(buf[..n].to_vec())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });
    ours
}

/// Everything a stream says up to its blank line.
async fn head<S: AsyncRead + Unpin>(s: &mut S) -> String {
    let mut got = Vec::new();
    let mut byte = [0u8; 1];
    while !got.ends_with(b"\r\n\r\n") {
        assert_eq!(timeout(WAIT, s.read(&mut byte)).await.expect("the head arrived").unwrap(), 1, "the stream ended inside the head");
        got.push(byte[0]);
    }
    String::from_utf8(got).unwrap()
}

async fn exactly<S: AsyncRead + Unpin>(s: &mut S, n: usize) -> Vec<u8> {
    let mut got = vec![0u8; n];
    timeout(WAIT, s.read_exact(&mut got)).await.expect("the bytes arrived").unwrap();
    got
}

/// Bytes nobody would type, so a slip of one byte anywhere shows.
fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + i / 251) as u8).collect()
}

#[tokio::test]
async fn six_megabytes_up_reach_the_host_unchanged_on_the_http_door_and_the_answer_comes_back() {
    let d = start(Duration::from_secs(3600)).await;
    let mut ws = pipe(d.addr).await;
    let body = pattern(6 * 1024 * 1024);
    let request =
        [format!("POST /v1/messages HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes(), body].concat();
    let mut client = TcpStream::connect(d.http).await.unwrap();
    let (mut client_read, mut client_write) = client.split();
    let sent = request.clone();
    let send = async move {
        client_write.write_all(&sent).await.unwrap();
    };
    let host = async {
        let open = opened(&mut ws).await;
        assert_eq!((open["op"].as_str(), open["door"].as_str()), (Some("pipe.open"), Some("http")), "{open}");
        assert!(open["heldMs"].is_u64(), "{open}");
        let mut carried = bytes_of(ws);
        let got = exactly(&mut carried, request.len()).await;
        assert!(got == request, "the request reached the host changed");
        carried.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await.unwrap();
        carried
    };
    let ((), _carried) = tokio::join!(send, host);
    let answer = exactly(&mut client_read, 40).await;
    assert_eq!(answer, b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
}

/// Ten minutes of a stream on tokio's paused clock: every timer the daemon holds runs on that clock, so one that
/// cut a quiet or a long stream would have fired. The bytes arrive whole and in order. Nothing here waits under a
/// timer of its own once the clock is paused: the runtime jumps to the next timer whenever it idles, which on Linux
/// is while a chunk is still crossing the loopback.
#[tokio::test(flavor = "current_thread")]
async fn a_stream_ten_minutes_long_comes_down_unchanged() {
    let d = start(Duration::from_secs(3600)).await;
    let mut ws = pipe(d.addr).await;
    let mut client = TcpStream::connect(d.http).await.unwrap();
    client.write_all(b"POST /v1/messages HTTP/1.1\r\nContent-Length: 0\r\n\r\n").await.unwrap();
    opened(&mut ws).await;
    let mut carried = bytes_of(ws);
    head(&mut carried).await;
    let began = Instant::now();
    tokio::time::pause();
    let mut sent = Vec::new();
    for i in 0..20 {
        let chunk = format!("event: content_block_delta\ndata: {{\"i\":{i}}}\n\n").into_bytes();
        carried.write_all(&chunk).await.unwrap();
        let mut got = vec![0u8; chunk.len()];
        client.read_exact(&mut got).await.expect("the stream was cut");
        assert_eq!(got, chunk);
        sent.extend(chunk);
        tokio::time::advance(Duration::from_secs(30)).await;
    }
    assert!(began.elapsed() >= Duration::from_secs(600), "the clock moved {:?}", began.elapsed());
    drop(carried);
    let mut rest = Vec::new();
    client.read_to_end(&mut rest).await.unwrap();
    assert!(rest.is_empty(), "bytes the host never sent: {rest:?}");
}

#[tokio::test]
async fn a_websocket_upgrade_through_the_http_door_passes_and_its_frames_flow_both_ways() {
    let d = start(Duration::from_secs(3600)).await;
    let mut ws = pipe(d.addr).await;
    let host = tokio::spawn(async move {
        opened(&mut ws).await;
        let mut server = tokio_tungstenite::accept_async(bytes_of(ws)).await.expect("the upgrade reached the host whole");
        while let Some(Ok(message)) = server.next().await {
            if let Message::Text(t) = message {
                server.send(Message::text(format!("echo:{t}"))).await.unwrap();
            }
        }
    });
    let stream = TcpStream::connect(d.http).await.unwrap();
    let (mut agent, _) = tokio_tungstenite::client_async("ws://127.0.0.1/backend-api/codex/responses", stream).await.unwrap();
    for word in ["one", "two"] {
        agent.send(Message::text(word)).await.unwrap();
        let back = timeout(WAIT, agent.next()).await.unwrap().unwrap().unwrap();
        assert_eq!(back, Message::text(format!("echo:{word}")));
    }
    drop(agent);
    let _ = timeout(WAIT, host).await;
}

#[tokio::test]
async fn the_gh_door_reaches_a_pipe_named_for_it() {
    let d = start(Duration::from_secs(3600)).await;
    let mut ws = pipe(d.addr).await;
    let mut gh = UnixStream::connect(&d.gh).await.unwrap();
    let request = b"GET /repos/acme/lab HTTP/1.1\r\nHost: api.github.com\r\nAuthorization: token wsp-turn\r\n\r\n";
    gh.write_all(request).await.unwrap();
    let open = opened(&mut ws).await;
    assert_eq!(open["door"], json!("gh"), "{open}");
    let mut carried = bytes_of(ws);
    assert_eq!(exactly(&mut carried, request.len()).await, request);
    carried.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").await.unwrap();
    drop(carried);
    let mut answer = Vec::new();
    timeout(WAIT, gh.read_to_end(&mut answer)).await.unwrap().unwrap();
    assert_eq!(answer, b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
}

#[tokio::test]
async fn the_http_door_answers_on_the_v6_loopback_at_the_same_port() {
    let d = start(Duration::from_secs(3600)).await;
    let mut ws = pipe(d.addr).await;
    let mut client = TcpStream::connect(SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, d.http.port()))).await.unwrap();
    let request = b"GET /backend-api/codex/models HTTP/1.1\r\nHost: localhost:7071\r\n\r\n";
    client.write_all(request).await.unwrap();
    assert_eq!(opened(&mut ws).await["door"], json!("http"));
    let mut carried = bytes_of(ws);
    assert_eq!(exactly(&mut carried, request.len()).await, request);
}

#[tokio::test]
async fn the_gh_door_points_gh_at_itself_as_it_opens() {
    let d = start(Duration::from_secs(3600)).await;
    let config = d.gh.parent().unwrap().join("gh").join("config.yml");
    assert_eq!(std::fs::read_to_string(&config).unwrap(), format!("http_unix_socket: {}\n", d.gh.display()));
}

#[tokio::test]
async fn with_no_pipe_a_connection_is_held_then_forwarded_with_how_long_it_waited() {
    let d = start(Duration::from_secs(3600)).await;
    let mut client = TcpStream::connect(d.http).await.unwrap();
    let request = b"POST /v1/messages HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
    client.write_all(request).await.unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let mut ws = pipe(d.addr).await;
    let open = opened(&mut ws).await;
    let held = open["heldMs"].as_u64().unwrap();
    assert!((550..WAIT.as_millis() as u64).contains(&held), "held {held} ms");
    let mut carried = bytes_of(ws);
    assert_eq!(exactly(&mut carried, request.len()).await, request);
}

#[tokio::test]
async fn a_held_connection_whose_client_left_is_never_forwarded() {
    let d = start(Duration::from_secs(3600)).await;
    let mut left = TcpStream::connect(d.http).await.unwrap();
    left.write_all(b"POST /v1/messages HTTP/1.1\r\nX-Which: abandoned\r\nContent-Length: 0\r\n\r\n").await.unwrap();
    drop(left);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut ws = pipe(d.addr).await;
    assert!(next(&mut ws, QUIET).await.is_none(), "a connection whose client left reached the host");
    // The pipe was free all along: the next connection takes it, and what it carries is that connection's.
    let mut client = TcpStream::connect(d.http).await.unwrap();
    let request = b"POST /v1/messages HTTP/1.1\r\nX-Which: live\r\nContent-Length: 0\r\n\r\n";
    client.write_all(request).await.unwrap();
    opened(&mut ws).await;
    let mut carried = bytes_of(ws);
    assert_eq!(exactly(&mut carried, request.len()).await, request);
}

#[tokio::test]
async fn a_connection_held_past_the_hold_is_closed_and_never_forwarded() {
    let d = start(Duration::from_millis(400)).await;
    let mut client = TcpStream::connect(d.http).await.unwrap();
    client.write_all(b"POST /v1/messages HTTP/1.1\r\nContent-Length: 0\r\n\r\n").await.unwrap();
    let since = Instant::now();
    let mut rest = Vec::new();
    timeout(WAIT, client.read_to_end(&mut rest)).await.expect("the held connection was closed").ok();
    assert!(since.elapsed() >= Duration::from_millis(350), "closed after {:?}", since.elapsed());
    assert!(rest.is_empty());
    let mut ws = pipe(d.addr).await;
    assert!(next(&mut ws, QUIET).await.is_none(), "a connection closed for its hold reached the host");
}

#[tokio::test]
async fn a_pipe_dropped_mid_response_closes_its_connection_at_once_and_nothing_is_resent() {
    let d = start(Duration::from_secs(3600)).await;
    let mut first = pipe(d.addr).await;
    let mut client = TcpStream::connect(d.http).await.unwrap();
    client.write_all(b"POST /v1/messages HTTP/1.1\r\nContent-Length: 0\r\n\r\n").await.unwrap();
    opened(&mut first).await;
    let mut spare = pipe(d.addr).await;
    let partial = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nhalf of";
    first.send(Message::binary(partial.to_vec())).await.unwrap();
    assert_eq!(exactly(&mut client, partial.len()).await, partial);
    // The host goes with no close frame, as a host whose computer slept does.
    drop(first);
    let since = Instant::now();
    let mut rest = Vec::new();
    timeout(Duration::from_secs(2), client.read_to_end(&mut rest)).await.expect("the connection was left open").ok();
    assert!(since.elapsed() < Duration::from_secs(2));
    assert!(rest.is_empty(), "bytes after the pipe dropped: {rest:?}");
    assert!(next(&mut spare, QUIET).await.is_none(), "the connection was sent again on another pipe");
}

#[tokio::test]
async fn a_pipe_past_the_pool_is_closed_with_the_sentence_and_the_parked_ones_stay() {
    let d = start(Duration::from_secs(3600)).await;
    let mut parked = Vec::new();
    for _ in 0..numbers::PIPE_POOL_MAX {
        parked.push(pipe(d.addr).await);
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut past = pipe(d.addr).await;
    match next(&mut past, WAIT).await {
        Some(Message::Close(Some(frame))) => assert_eq!(frame.reason.as_str(), words::PIPE_POOL_FULL),
        other => panic!("the pipe past the pool was not closed with the sentence: {other:?}"),
    }
    for ws in &mut parked {
        assert!(next(ws, Duration::from_millis(20)).await.is_none(), "a parked pipe was closed for the one past the pool");
    }
}

#[tokio::test]
async fn a_rotation_takes_a_parked_pipe_with_the_old_token() {
    let d = start(Duration::from_secs(3600)).await;
    let mut ws = pipe(d.addr).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    std::fs::write(d.token.path(), "the-next-token\n").unwrap();
    match next(&mut ws, WAIT).await {
        Some(Message::Close(Some(frame))) => {
            assert_eq!((u16::from(frame.code), frame.reason.as_str()), (words::AUTH_CLOSE_CODE, words::AUTH_TOKEN_ROTATED))
        }
        other => panic!("a parked pipe outlived its token: {other:?}"),
    }
}

/// A control socket past the auth frame and the hello.
async fn control(addr: SocketAddr) -> Pipe {
    let (mut ws, _) = connect_async(format!("ws://{addr}/")).await.unwrap();
    ws.send(Message::text(json!({"id": 1, "op": "auth", "token": TOKEN}).to_string())).await.unwrap();
    loop {
        if let Some(Message::Text(t)) = next(&mut ws, WAIT).await {
            if serde_json::from_str::<Value>(&t).unwrap()["type"] == "daemon.hello" {
                return ws;
            }
        }
    }
}

async fn reply(ws: &mut Pipe, id: u64) -> Value {
    loop {
        let Some(Message::Text(t)) = next(ws, Duration::from_secs(60)).await else { panic!("no reply to {id}") };
        let frame: Value = serde_json::from_str(&t).unwrap();
        if frame["id"] == json!(id) {
            return frame;
        }
    }
}

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A checkout on `wsp/fix`, one commit ahead of main, whose origin is the remote given and whose origin/main is
/// already known, so a push asks the network for nothing but the push itself.
fn checkout(root: &Path, remote: &str) -> PathBuf {
    let at = root.join("lab");
    std::fs::create_dir_all(&at).unwrap();
    git(&at, &["init", "-q", "-b", "main"]);
    std::fs::write(at.join("a.txt"), "a\n").unwrap();
    git(&at, &["add", "."]);
    git(&at, &["commit", "-qm", "a"]);
    git(&at, &["remote", "add", "origin", remote]);
    // Whatever becomes of the op's own proxy, this checkout's git never leaves the machine nor asks a helper of the
    // computer's for a credential: the op's proxy for github.com outranks this one where it is set.
    git(&at, &["config", "http.proxy", "http://127.0.0.1:9"]);
    git(&at, &["config", "credential.helper", ""]);
    git(&at, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&at, &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
    git(&at, &["checkout", "-qb", "wsp/fix"]);
    std::fs::write(at.join("b.txt"), "b\n").unwrap();
    git(&at, &["add", "."]);
    git(&at, &["commit", "-qm", "b"]);
    at
}

/// The host's side of every pipe it keeps parked: each request head as it arrived, answered 403 and closed, which
/// is how a host refuses and keeps git from asking anybody for a password.
fn refusing_host(addr: SocketAddr, pipes: usize) -> tokio::sync::mpsc::UnboundedReceiver<String> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    for _ in 0..pipes {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut ws = pipe(addr).await;
            if let Some(Message::Text(_)) = next(&mut ws, Duration::from_secs(120)).await {
                let mut carried = bytes_of(ws);
                let _ = tx.send(head(&mut carried).await);
                let _ = carried.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: 4\r\n\r\nwsp\n").await;
            }
        });
    }
    rx
}

#[tokio::test]
async fn git_push_with_a_token_reaches_the_http_door_with_that_token_as_its_proxy_password_and_nothing_else() {
    let d = start(Duration::from_secs(3600)).await;
    let at = checkout(d.root.path(), "https://github.com/acme/lab.git");
    let mut heads = refusing_host(d.addr, 4);
    let mut ws = control(d.addr).await;
    let token = "wsp-op-token-7";
    let cwd = at.to_string_lossy();
    ws.send(Message::text(json!({"id": 2, "op": "git.push", "cwd": cwd, "token": token}).to_string())).await.unwrap();
    let answered = reply(&mut ws, 2).await;
    assert_eq!(answered["ok"], json!(false), "a push the host refused reads as pushed: {answered}");

    let mut seen = Vec::new();
    while let Ok(head) = heads.try_recv() {
        seen.push(head);
    }
    assert!(!seen.is_empty(), "git never reached the HTTP door: {answered}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("wsp:{token}"));
    let proxy = format!("Proxy-Authorization: Basic {encoded}");
    assert!(seen[0].starts_with("GET http://github.com/acme/lab.git/info/refs?service=git-receive-pack HTTP/1.1\r\n"), "{}", seen[0]);
    for head in &seen {
        let holding: Vec<&str> = head.lines().filter(|l| l.contains(token) || l.contains(&encoded)).collect();
        assert_eq!(holding, [proxy.as_str()], "the token rode somewhere but the proxy password: {head}");
        assert!(!head.to_ascii_lowercase().contains("\nauthorization:"), "git sent a credential of its own: {head}");
    }
}

#[tokio::test]
async fn git_push_without_a_token_runs_as_it_always_has_and_nothing_reaches_a_door() {
    let d = start(Duration::from_secs(3600)).await;
    let bare = d.root.path().join("lab.git");
    git(d.root.path(), &["init", "-q", "--bare", "-b", "main", "lab.git"]);
    let at = checkout(d.root.path(), &bare.to_string_lossy());
    git(&at, &["push", "-q", "origin", "main"]);
    let mut heads = refusing_host(d.addr, 1);
    let mut ws = control(d.addr).await;
    let cwd = at.to_string_lossy();
    ws.send(Message::text(json!({"id": 2, "op": "git.push", "cwd": cwd}).to_string())).await.unwrap();
    let answered = reply(&mut ws, 2).await;
    assert_eq!(answered["ok"], json!(true), "{answered}");
    assert_eq!(answered["branch"], json!("wsp/fix"), "{answered}");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(heads.try_recv().is_err(), "a push with no token went through the door");
}

#[tokio::test]
async fn a_pipe_to_a_daemon_that_serves_no_doors_is_closed_rather_than_parked() {
    let mut token = tempfile::NamedTempFile::new().unwrap();
    writeln!(token, "{TOKEN}").unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut options = Options::new(token.path());
    options.readings_interval_ms = Some(86_400_000);
    options.host = "127.0.0.1".to_owned();
    options.port = 0;
    options.root = Some(root.path().to_path_buf());
    options.manifest_path = Some(root.path().join("manifest.json"));
    options.kind = "local".to_owned();
    options.doors = Some(Doors { http: "127.0.0.1:0".parse().unwrap(), gh: root.path().join("broker.sock"), hold_max: WAIT });
    let daemon = Daemon::bind_with(options, Box::new(|_| {})).await.unwrap();
    assert_eq!(daemon.http_door(), None, "a daemon that is no provider's machine bound the HTTP door");
    let addr = daemon.local_addr();
    tokio::spawn(daemon.run());
    assert!(!root.path().join("broker.sock").exists(), "a daemon that is no provider's machine bound the gh door");
    let mut ws = pipe(addr).await;
    assert!(matches!(next(&mut ws, WAIT).await, Some(Message::Close(Some(_)))), "the pipe was parked where nothing takes it");
}

/// A process that took the HTTP door's port before the daemon did hears no op's token: the daemon names to git only a
/// door it bound, and refuses the op instead.
#[tokio::test]
async fn an_op_token_never_goes_to_a_port_the_daemon_could_not_bind() {
    let squatter = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let held = squatter.local_addr().unwrap();
    let d = start_at(Duration::from_secs(3600), held).await;
    let at = checkout(d.root.path(), "https://github.com/acme/lab.git");
    let mut ws = control(d.addr).await;
    let cwd = at.to_string_lossy();
    ws.send(Message::text(json!({"id": 2, "op": "git.push", "cwd": cwd, "token": "wsp-op-token-7"}).to_string())).await.unwrap();
    let answered = reply(&mut ws, 2).await;
    assert_eq!((answered["ok"].clone(), answered["error"].clone()), (json!(false), json!(words::OP_TOKEN_NO_DOOR)), "{answered}");
    assert!(timeout(QUIET, squatter.accept()).await.is_err(), "the process holding the port was dialled");
}
