// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/tunnel.ts at 65f6b89c7: one connection on this computer carried over a daemon link
//! as a tunnel, its three ops and the two frames that come back. Spec: spec/tunnels.md.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use tokio::net::TcpStream;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;

use crate::reach::Frame;

/// How one op reaches the far side, settling when it answers. tunnel.ts:15.
pub type TunnelLink = Rc<dyn Fn(&str, Frame) -> LocalBoxFuture<'static, Result<Frame, HostError>>>;

/// One carried connection as the code that pushes frames to it holds it: node's `Socket` write side. Cheap to clone;
/// every clone is the same connection. The fields are the implementer's.
#[derive(Clone)]
pub struct TunnelConn {
    state: Rc<RefCell<ConnState>>,
}

/// What a carried connection holds while it lives; the implementer's to fill.
#[derive(Default)]
struct ConnState {}

impl TunnelConn {
    /// `socket.write(bytes)`: queued behind every earlier write; nothing once the connection is ended or destroyed.
    pub fn write(&self, bytes: Vec<u8>) {
        todo!()
    }

    /// `socket.end(last)`: `last` written, then the write half shut once every queued write is out; reading goes on.
    pub fn end(&self, last: Option<Vec<u8>>) {
        todo!()
    }

    /// `socket.destroy()`: both halves closed now, queued writes dropped. A second call does nothing.
    pub fn destroy(&self) {
        todo!()
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
    todo!()
}

/// A pushed frame handed to the connection that opened the tunnel it names: `tunnel.end` ends it, `tunnel.data` is
/// decoded and written (calling `touched` first). True when it was one of `conns`'; any other event, or an id not in
/// `conns`, is false and touches nothing. tunnel.ts:59.
pub fn tunnel_frame(conns: &TunnelConns, event: &DaemonEvent, touched: Option<&dyn Fn()>) -> bool {
    todo!()
}
