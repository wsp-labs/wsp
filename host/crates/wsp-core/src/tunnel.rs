// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/tunnel.ts at 65f6b89c7: one connection on this computer carried over a daemon link
//! as a tunnel, its three ops and the two frames that come back. Spec: spec/tunnels.md.
use std::cell::RefCell;
use std::rc::Rc;
use std::task::Poll;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;

use crate::node_base64::node_base64;
use crate::reach::Frame;

/// How one op reaches the far side, settling when it answers. tunnel.ts:15.
pub type TunnelLink = Rc<dyn Fn(&str, Frame) -> LocalBoxFuture<'static, Result<Frame, HostError>>>;

/// One carried connection as the code that pushes frames to it holds it: node's `Socket` write side. Cheap to clone;
/// every clone is the same connection. The fields are the implementer's.
#[derive(Clone)]
pub struct TunnelConn {
    state: Rc<RefCell<ConnState>>,
}

impl std::fmt::Debug for TunnelConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TunnelConn")
    }
}

enum Cmd {
    Write(Vec<u8>),
    End(Option<Vec<u8>>),
    Destroy,
}

/// What a carried connection holds while it lives: the queue its writer task reads, and the stop its reader task obeys.
#[derive(Default)]
struct ConnState {
    cmds: Option<mpsc::UnboundedSender<Cmd>>,
    stop: Option<oneshot::Sender<()>>,
}

impl TunnelConn {
    /// `socket.write(bytes)`: queued behind every earlier write; nothing once the connection is ended or destroyed.
    pub fn write(&self, bytes: Vec<u8>) {
        let cmds = self.state.borrow().cmds.clone();
        if let Some(cmds) = cmds {
            let _ = cmds.send(Cmd::Write(bytes));
        }
    }

    /// `socket.end(last)`: `last` written, then the write half shut once every queued write is out; reading goes on.
    pub fn end(&self, last: Option<Vec<u8>>) {
        let cmds = self.state.borrow_mut().cmds.take();
        if let Some(cmds) = cmds {
            let _ = cmds.send(Cmd::End(last));
        }
    }

    /// `socket.destroy()`: both halves closed now, queued writes dropped. A second call does nothing.
    pub fn destroy(&self) {
        let (cmds, stop) = {
            let mut state = self.state.borrow_mut();
            (state.cmds.take(), state.stop.take())
        };
        if let Some(cmds) = cmds {
            let _ = cmds.send(Cmd::Destroy);
        }
        if let Some(stop) = stop {
            let _ = stop.send(());
        }
    }
}

/// Every connection one road carries, by the tunnel id it opened under (tunnel.ts:21 `conns`).
pub type TunnelConns = Rc<RefCell<IndexMap<String, TunnelConn>>>;

/// tunnel.ts:17.
pub struct TunnelOptions {
    /// The port on the far side this connection is carried to.
    pub port: u16,
    pub conns: TunnelConns,
    /// The id this connection takes; the caller mints it.
    pub tunnel_id: String,
    /// Called on every chunk moved either way, and once at the start.
    pub touched: Option<Rc<dyn Fn()>>,
    /// The far side would not open the tunnel; without one the connection is destroyed.
    pub refused: Option<Rc<dyn Fn(&TunnelConn)>>,
    /// Called once the far side holds it open.
    pub opened: Option<Rc<dyn Fn()>>,
}

/// Carries one connection over the link: registered in `conns` at once, nothing read until `tunnel.open` answers, then
/// every chunk read sent as `tunnel.write` with base64 `data`; `tunnel.close` sent once when the connection closes.
/// Spawns on the current `LocalSet` and returns at once. tunnel.ts:33.
pub fn plumb_tunnel(link: TunnelLink, socket: TcpStream, opts: TunnelOptions) {
    let (cmds, cmd_rx) = mpsc::unbounded_channel();
    let (stop, stop_rx) = oneshot::channel();
    let conn = TunnelConn { state: Rc::new(RefCell::new(ConnState { cmds: Some(cmds), stop: Some(stop) })) };
    let TunnelOptions { port, conns, tunnel_id, touched, refused, opened } = opts;
    conns.borrow_mut().insert(tunnel_id.clone(), conn.clone());
    if let Some(touched) = &touched {
        touched();
    }
    let (rd, wr) = socket.into_split();
    tokio::task::spawn_local(write_side(wr, cmd_rx, conn.clone()));
    let read = ReadSide { link, port, conns, tunnel_id, touched, refused, opened, conn };
    tokio::task::spawn_local(read_side(rd, stop_rx, read));
}

struct ReadSide {
    link: TunnelLink,
    port: u16,
    conns: TunnelConns,
    tunnel_id: String,
    touched: Option<Rc<dyn Fn()>>,
    refused: Option<Rc<dyn Fn(&TunnelConn)>>,
    opened: Option<Rc<dyn Fn()>>,
    conn: TunnelConn,
}

async fn write_side(mut wr: OwnedWriteHalf, mut cmds: mpsc::UnboundedReceiver<Cmd>, conn: TunnelConn) {
    while let Some(cmd) = cmds.recv().await {
        match cmd {
            Cmd::Write(bytes) => {
                if wr.write_all(&bytes).await.is_err() {
                    conn.destroy();
                    return;
                }
            }
            Cmd::End(last) => {
                if let Some(last) = last {
                    let _ = wr.write_all(&last).await;
                }
                let _ = wr.shutdown().await;
                return;
            }
            Cmd::Destroy => return,
        }
    }
}

async fn read_side(mut rd: OwnedReadHalf, mut stop: oneshot::Receiver<()>, p: ReadSide) {
    let id = p.tunnel_id.clone();
    let carrying = match (p.link)("tunnel.open", frame([("tunnelId", json!(id)), ("port", Value::from(p.port))])).await {
        Ok(_) => {
            if let Some(opened) = &p.opened {
                opened();
            }
            true
        }
        Err(_) => {
            match &p.refused {
                Some(refused) => refused(&p.conn),
                None => p.conn.destroy(),
            }
            false
        }
    };
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        tokio::select! {
            _ = &mut stop => break,
            read = rd.read(&mut buf) => match read {
                Ok(n) if n > 0 => {
                    if !carrying {
                        continue;
                    }
                    if let Some(touched) = &p.touched {
                        touched();
                    }
                    let data = STANDARD.encode(&buf[..n]);
                    let mut write = (p.link)("tunnel.write", frame([("tunnelId", json!(id)), ("data", json!(data))]));
                    match futures::poll!(write.as_mut()) {
                        Poll::Ready(Err(_)) => {
                            p.conn.destroy();
                            break;
                        }
                        Poll::Ready(Ok(_)) => {}
                        Poll::Pending => {
                            let conn = p.conn.clone();
                            tokio::task::spawn_local(async move {
                                if write.await.is_err() {
                                    conn.destroy();
                                }
                            });
                        }
                    }
                }
                _ => break,
            },
        }
    }
    p.conn.end(None);
    p.conns.borrow_mut().shift_remove(&id);
    let _ = (p.link)("tunnel.close", frame([("tunnelId", json!(id))])).await;
}

fn frame<const N: usize>(pairs: [(&str, Value); N]) -> Frame {
    pairs.into_iter().map(|(key, value)| (key.to_string(), value)).collect()
}

/// A pushed frame handed to the connection that opened the tunnel it names: `tunnel.end` ends it, `tunnel.data` is
/// decoded and written (calling `touched` first). True when it was one of `conns`'; any other event, or an id not in
/// `conns`, is false and touches nothing. tunnel.ts:59.
pub fn tunnel_frame(conns: &TunnelConns, event: &DaemonEvent, touched: Option<&dyn Fn()>) -> bool {
    let held = |id: &str| conns.borrow().get(id).cloned();
    match event {
        DaemonEvent::TunnelEnd { tunnel_id, .. } => match held(tunnel_id) {
            Some(conn) => {
                conn.end(None);
                true
            }
            None => false,
        },
        DaemonEvent::TunnelData { tunnel_id, data, .. } => match held(tunnel_id) {
            Some(conn) => {
                if let Some(touched) = touched {
                    touched();
                }
                conn.write(node_base64(data));
                true
            }
            None => false,
        },
        _ => false,
    }
}
