// SPDX-License-Identifier: AGPL-3.0-only
//! The pipes and the two doors they serve, on a machine whose credentials the host keeps. A pipe is a WebSocket the
//! host dials and this daemon parks; a local process that connects to a door takes one, and from then on that
//! connection's bytes cross it as binary frames both ways, unchanged, until either end closes. A pipe carries one
//! connection and is never used again, and nothing it carried is ever sent a second time: a pipe that drops ends
//! its connection, and the agent's own retry is the only resend. With no pipe parked the door holds the connection
//! and reads nothing, so the bytes the client sent wait in the kernel until the host dials again.

use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{FutureExt, SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, Interest};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::time::{sleep, sleep_until, Instant};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use wsp_frames::{numbers, words, PipeDoor, PipeOpen};

use crate::{Ctx, Outbound, Outgoing};

/// How often a held connection looks again at whether its client is still there, so one abandoned while the host is
/// away gives its socket back before the hour is up. A look reads nothing.
const GONE_EVERY: Duration = Duration::from_secs(1);
/// The most of a local connection's bytes one binary frame carries.
const CHUNK_BYTES: usize = 64 * 1024;
/// The gh configuration folder beside the gh door, `/root/.wsp/gh` on a machine: the daemon's own gh and every
/// turn's and terminal's gh run with it, and a turn's launch writes the same file.
const GH_CONFIG: &str = "gh";

/// Where the two doors are and how long a connection waits on one for a pipe. The binary names the guest's; a case
/// names its own loopback port and socket file and a hold it can wait out.
#[derive(Debug, Clone)]
pub struct Doors {
    pub http: SocketAddr,
    pub gh: PathBuf,
    pub hold_max: Duration,
}

impl Doors {
    pub fn guest() -> Doors {
        Doors {
            http: SocketAddr::from((Ipv4Addr::LOCALHOST, numbers::BROKER_PORT)),
            gh: PathBuf::from(numbers::BROKER_SOCKET_PATH),
            hold_max: Duration::from_millis(numbers::PIPE_HOLD_MAX_MS),
        }
    }
}

/// The doors as bound: the HTTP door on the v4 loopback with its own address, which a case that asked for port zero
/// reads back, and on `[::1]` at the same port; and the gh door's file. Any of them is missing where it could not be
/// bound, and `said` holds why.
pub(crate) struct Bound {
    http: Vec<TcpListener>,
    gh: Option<UnixListener>,
    pub(crate) doors: Doors,
    said: Vec<String>,
}

impl Bound {
    /// Whether the v4 HTTP door is this daemon's: the address an op's git is pointed at.
    pub(crate) fn serves_http(&self) -> bool {
        !self.http.is_empty()
    }
}

/// Binds both doors, the HTTP one on both loopbacks, since `localhost` reads as `::1` first on a machine and Codex
/// is pointed at `localhost`. The gh door writes gh's configuration naming it as it opens, so a terminal opened before
/// any turn finds it. A door that cannot be bound leaves the daemon serving without it: a machine with no door still
/// answers its link, which is how the host learns anything is wrong.
pub(crate) async fn bind(mut doors: Doors) -> Bound {
    let mut said = Vec::new();
    let mut http = Vec::new();
    match TcpListener::bind(doors.http).await {
        Ok(v4) => {
            doors.http = v4.local_addr().unwrap_or(doors.http);
            http.push(v4);
            let v6 = SocketAddr::from((Ipv6Addr::LOCALHOST, doors.http.port()));
            match TcpListener::bind(v6).await {
                Ok(listener) => http.push(listener),
                Err(e) => said.push(format!("the HTTP door is not served on {v6}: {e}")),
            }
        }
        Err(e) => said.push(format!("the HTTP door is not served: {}: {e}", doors.http)),
    }
    let gh = match crate::relay::listen_open_socket(&doors.gh) {
        Ok(listener) => {
            let config = format!("http_unix_socket: {}\n", doors.gh.display());
            if let Err(e) = write_config(&gh_config(&doors), config.as_bytes()) {
                said.push(format!("gh is not pointed at its door: {}: {e}", gh_config(&doors).display()));
            }
            Some(listener)
        }
        Err(e) => {
            said.push(format!("the gh door is not served: {}: {e}", doors.gh.display()));
            None
        }
    };
    Bound { http, gh, doors, said }
}

fn gh_config(doors: &Doors) -> PathBuf {
    doors.gh.parent().unwrap_or_else(|| Path::new("/")).join(GH_CONFIG)
}

/// Accepts on both doors for as long as the daemon runs; each connection waits for a pipe on its own task. A door
/// that could not be bound is said here, once the daemon serves, since what it says before that is its start's own.
pub(crate) fn serve(bound: Bound, ctx: &Arc<Ctx>) {
    for line in &bound.said {
        ctx.log(line);
    }
    for listener in bound.http {
        let pipes = Arc::clone(&ctx.pipes);
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let _ = stream.set_nodelay(true);
                tokio::spawn(Arc::clone(&pipes).hold(Local::Tcp(stream), PipeDoor::Http));
            }
        });
    }
    if let Some(listener) = bound.gh {
        let pipes = Arc::clone(&ctx.pipes);
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(Arc::clone(&pipes).hold(Local::Unix(stream), PipeDoor::Gh));
            }
        });
    }
}

/// One local connection on a door.
pub(crate) enum Local {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl Local {
    /// Whether the client has left, read off what the kernel last said of the socket and never off its bytes: a
    /// client that sent its request and then closed still has that request waiting, and only its end says it left.
    fn gone(&self) -> bool {
        let ready = match self {
            Local::Tcp(s) => s.ready(Interest::READABLE).now_or_never(),
            Local::Unix(s) => s.ready(Interest::READABLE).now_or_never(),
        };
        match ready {
            Some(Ok(ready)) => ready.is_read_closed(),
            Some(Err(_)) => true,
            None => false,
        }
    }

    fn split(self) -> (Box<dyn AsyncRead + Send + Unpin>, Box<dyn AsyncWrite + Send + Unpin>) {
        match self {
            Local::Tcp(s) => {
                let (r, w) = s.into_split();
                (Box::new(r), Box::new(w))
            }
            Local::Unix(s) => {
                let (r, w) = s.into_split();
                (Box::new(r), Box::new(w))
            }
        }
    }
}

/// A local connection handed to a pipe, with the door it came in on and when it arrived.
struct Taken {
    local: Local,
    door: PipeDoor,
    since: Instant,
}

/// A parked pipe as the pool holds it: the way to hand it a connection.
struct Idle {
    key: u64,
    give: oneshot::Sender<Taken>,
}

/// The parked pipes, oldest first, the bell each held connection waits on, and how long one may wait.
pub(crate) struct Pipes {
    idle: Mutex<VecDeque<Idle>>,
    parked: Notify,
    hold_max: Duration,
}

impl Pipes {
    pub(crate) fn new(hold_max: Duration) -> Pipes {
        Pipes { idle: Mutex::default(), parked: Notify::new(), hold_max }
    }

    /// Parks one pipe, or refuses it past the pool's cap.
    fn park(&self, key: u64, give: oneshot::Sender<Taken>) -> bool {
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() >= numbers::PIPE_POOL_MAX {
            return false;
        }
        idle.push_back(Idle { key, give });
        drop(idle);
        self.parked.notify_waiters();
        true
    }

    /// Takes a pipe out of the pool; false where a connection already took it.
    fn unpark(&self, key: u64) -> bool {
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        let before = idle.len();
        idle.retain(|held| held.key != key);
        idle.len() != before
    }

    /// Hands the connection to the oldest parked pipe, under the lock, so a pipe that finds itself gone from the
    /// pool knows the connection is already in its hands. The connection comes back where no pipe is parked.
    fn give(&self, mut taken: Taken) -> Option<Taken> {
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(held) = idle.pop_front() {
            match held.give.send(taken) {
                Ok(()) => return None,
                Err(back) => taken = back,
            }
        }
        Some(taken)
    }

    #[cfg(test)]
    pub(crate) fn parked(&self) -> usize {
        self.idle.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// One local connection from its accept until a pipe takes it: held while no pipe is parked, dropped the moment
    /// its client is seen gone, and closed once it has waited as long as a hold may.
    async fn hold(self: Arc<Self>, local: Local, door: PipeDoor) {
        self.wait(Taken { local, door, since: Instant::now() }).await;
    }

    /// The same for a connection that already waited, which a pipe that died before carrying a byte of it gives
    /// back with its own arrival, so the hour is counted from there.
    async fn wait(&self, mut taken: Taken) {
        let until = taken.since + self.hold_max;
        loop {
            let bell = self.parked.notified();
            tokio::pin!(bell);
            bell.as_mut().enable();
            if taken.local.gone() {
                return;
            }
            match self.give(taken) {
                None => return,
                Some(back) => taken = back,
            }
            tokio::select! {
                () = bell => {}
                () = sleep(GONE_EVERY) => {}
                () = sleep_until(until) => return,
            }
        }
    }
}

/// A socket that passed the door with `pipe` on its auth frame: parked until a local connection takes it, then that
/// connection's bytes both ways. It answers no op and hears nothing the daemon pushes; its token is held, so a
/// rotation takes it as it takes every socket the old token opened.
pub(crate) async fn park<S>(mut ws: WebSocketStream<S>, ctx: &Arc<Ctx>, token: String)
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    let key = ctx.next_key();
    let (give, mut taken) = oneshot::channel();
    if !ctx.pipes.park(key, give) {
        close(&mut ws, CloseCode::Again, words::PIPE_POOL_FULL).await;
        return;
    }
    let (tx, mut rx) = mpsc::unbounded_channel();
    ctx.add_tokened(key, token, Outbound(tx));
    let mut rotated = false;
    let got = loop {
        tokio::select! {
            got = &mut taken => break got.ok(),
            out = rx.recv() => {
                rotated = matches!(out, Some(Outgoing::Rotated));
                break None;
            }
            frame = ws.next() => match frame {
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                _ => break None,
            },
        }
    };
    // Out of the pool before anything else, and where a connection took it in the same breath, that connection is
    // already in hand: it goes back to wait, since not one of its bytes has been read.
    let got = match got {
        Some(got) => Some(got),
        None if !ctx.pipes.unpark(key) => taken.try_recv().ok(),
        None => None,
    };
    if let Some(got) = got {
        if rotated {
            ctx.remove_authed(key);
            close(&mut ws, CloseCode::from(words::AUTH_CLOSE_CODE), words::AUTH_TOKEN_ROTATED).await;
            return ctx.pipes.wait(got).await;
        }
        carry(ws, got, &mut rx, &ctx.pipes).await;
    } else if rotated {
        close(&mut ws, CloseCode::from(words::AUTH_CLOSE_CODE), words::AUTH_TOKEN_ROTATED).await;
    }
    ctx.remove_authed(key);
}

/// One connection across one pipe: the open frame, then its bytes both ways until either end closes. Where the
/// pipe goes first the connection is closed at once, half answered or not, and nothing of it is sent again; the
/// one connection that goes back to wait is one whose open frame could not be written, which had sent nothing.
async fn carry<S>(mut ws: WebSocketStream<S>, taken: Taken, rotated: &mut mpsc::UnboundedReceiver<Outgoing>, pipes: &Pipes)
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    let held_ms = u64::try_from(taken.since.elapsed().as_millis()).unwrap_or(u64::MAX);
    let open = crate::frame_text(&PipeOpen::new(taken.door, held_ms));
    if ws.send(Message::text(open)).await.is_err() {
        return pipes.wait(taken).await;
    }
    let (mut read, mut write) = taken.local.split();
    let (mut sink, mut stream) = ws.split();
    let up = async {
        let mut buf = vec![0u8; CHUNK_BYTES];
        loop {
            match read.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if sink.send(Message::binary(buf[..n].to_vec())).await.is_err() {
                        return false;
                    }
                }
            }
        }
        true
    };
    let down = async {
        while let Some(Ok(message)) = stream.next().await {
            match message {
                Message::Binary(bytes) => {
                    if write.write_all(&bytes).await.is_err() {
                        break;
                    }
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
                Message::Text(_) | Message::Close(_) => break,
            }
        }
    };
    let ended_here = tokio::select! {
        sent = up => sent,
        () = down => false,
        _ = rotated.recv() => false,
    };
    // Only a client that finished sending is a pipe this side closes; every other end already took it down.
    if ended_here {
        let _ = sink.send(Message::Close(None)).await;
    }
    let _ = write.shutdown().await;
}

async fn close<S>(ws: &mut WebSocketStream<S>, code: CloseCode, reason: &str)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let frame = CloseFrame { code, reason: reason.to_owned().into() };
    let _ = tokio::time::timeout(Duration::from_secs(5), ws.close(Some(frame))).await;
}

/// What the git and gh of one op run with on a machine whose credentials the host keeps: every GitHub remote form
/// rewritten to plain HTTP so the HTTP door is git's proxy, with the op's token as the proxy password and no
/// credential helper asked; and for gh, the token and the configuration that sends it down the gh door.
pub(crate) struct OpEnv {
    pub(crate) git: Vec<(String, String)>,
    pub(crate) gh: Vec<(String, String)>,
}

pub(crate) fn op_env(token: &str, doors: &Doors) -> OpEnv {
    let proxy = format!("http://wsp:{}@{}", userinfo(token), doors.http);
    let pairs = [
        ("url.http://github.com/.insteadOf", "https://github.com/"),
        ("url.http://github.com/.insteadOf", "git@github.com:"),
        ("url.http://github.com/.insteadOf", "ssh://git@github.com/"),
        ("url.http://github.com/.insteadOf", "ssh://git@ssh.github.com:443/"),
        ("http.http://github.com/.proxy", proxy.as_str()),
        ("http.proxyAuthMethod", "basic"),
        ("credential.helper", ""),
    ];
    let mut git = vec![("GIT_CONFIG_COUNT".to_owned(), pairs.len().to_string())];
    for (i, (key, value)) in pairs.iter().enumerate() {
        git.push((format!("GIT_CONFIG_KEY_{i}"), (*key).to_owned()));
        git.push((format!("GIT_CONFIG_VALUE_{i}"), (*value).to_owned()));
    }
    let gh = vec![
        ("GH_TOKEN".to_owned(), token.to_owned()),
        ("GH_CONFIG_DIR".to_owned(), gh_config(doors).to_string_lossy().into_owned()),
        ("GH_TELEMETRY".to_owned(), "false".to_owned()),
    ];
    OpEnv { git, gh }
}

/// Written through the folder's descriptor with no link at the name followed, since anything on the machine may
/// write in the daemon's folder.
#[cfg(target_os = "linux")]
fn write_config(folder: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::create_dir_all(folder)?;
    wsp_runtime::bundle::write_file_in(folder, "config.yml", bytes, 0o600).map_err(|e| io::Error::other(e.to_string()))
}

/// No daemon serves the doors off Linux; a case on another computer writes the file as it is.
#[cfg(not(target_os = "linux"))]
fn write_config(folder: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::create_dir_all(folder)?;
    std::fs::write(folder.join("config.yml"), bytes)
}

/// A token as the password of a URL: every byte but the unreserved ones escaped, so a token holding `@` or `:`
/// cannot move where the proxy is.
fn userinfo(token: &str) -> String {
    token
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => char::from(b).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_reaches_the_proxy_url_as_its_password_and_nothing_in_it_moves_the_host() {
        let doors =
            Doors { http: SocketAddr::from((Ipv4Addr::LOCALHOST, 7071)), gh: PathBuf::from("/x/broker.sock"), hold_max: Duration::ZERO };
        assert_eq!(format!("http://wsp:{}@{}", userinfo("ab-C_9.~"), doors.http), "http://wsp:ab-C_9.~@127.0.0.1:7071");
        assert_eq!(userinfo("a@evil:1/x"), "a%40evil%3A1%2Fx");
    }

    #[tokio::test]
    async fn a_pipe_past_the_pool_is_refused_and_one_taken_out_frees_its_place() {
        let pipes = Pipes::new(Duration::from_secs(60));
        let mut held = Vec::new();
        for key in 0..numbers::PIPE_POOL_MAX as u64 {
            let (give, taken) = oneshot::channel();
            assert!(pipes.park(key, give));
            held.push(taken);
        }
        let (give, _) = oneshot::channel();
        assert!(!pipes.park(999, give), "a pipe past the cap was parked");
        assert!(pipes.unpark(3));
        assert!(!pipes.unpark(3), "a pipe was taken out twice");
        let (give, _) = oneshot::channel();
        assert!(pipes.park(1000, give));
        assert_eq!(pipes.parked(), numbers::PIPE_POOL_MAX);
    }

    /// A pipe whose task ended between being parked and being handed a connection is passed over, and the connection
    /// reaches the next one whole.
    #[tokio::test]
    async fn a_connection_skips_a_pipe_that_went_and_reaches_the_next() {
        let pipes = Pipes::new(Duration::from_secs(60));
        let (gone, dropped) = oneshot::channel();
        drop(dropped);
        assert!(pipes.park(1, gone));
        let (live, mut taken) = oneshot::channel();
        assert!(pipes.park(2, live));
        let (a, _b) = UnixStream::pair().unwrap();
        assert!(pipes.give(Taken { local: Local::Unix(a), door: PipeDoor::Gh, since: Instant::now() }).is_none());
        assert_eq!(taken.try_recv().map(|t| t.door).ok(), Some(PipeDoor::Gh));
        assert_eq!(pipes.parked(), 0);
    }

    #[tokio::test]
    async fn a_client_that_sent_its_request_and_left_reads_as_gone_and_one_still_there_does_not() {
        let (stays, _held) = UnixStream::pair().unwrap();
        let (leaves, mut client) = UnixStream::pair().unwrap();
        client.write_all(b"POST /v1/messages HTTP/1.1\r\n\r\n").await.unwrap();
        drop(client);
        let stays = Local::Unix(stays);
        let leaves = Local::Unix(leaves);
        let seen = async {
            while !leaves.gone() {
                tokio::task::yield_now().await;
            }
        };
        assert!(tokio::time::timeout(Duration::from_secs(5), seen).await.is_ok(), "a client that left never read as gone");
        assert!(!stays.gone(), "a client still there read as gone");
    }
}
