// SPDX-License-Identifier: AGPL-3.0-only
//! One socket, as the door decides it and the ops read it (serve.ts:493-626). The transport is a `FrameSink`, so
//! every unit but the listener is driven with a recording sink (spec/README.md, "The seams").
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::daemon_channel::DaemonChannel;
use wsp_machines::places::helpers::LinkSocket;
use wsp_store::keys::Seal;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};

use super::door::Served;
use super::served::{Payload, SocketOps};
use super::{Authed, UpgradeHead};
use crate::serve_slates::SlateHolds;

/// The socket's transport. The websocket implements it; tests record what it is told.
pub trait FrameSink {
    /// `readyState === OPEN`.
    fn is_open(&self) -> bool;
    fn send_text(&self, text: String);
    /// A sealed frame.
    fn send_binary(&self, bytes: Vec<u8>);
    /// A close frame with this code and reason.
    fn close(&self, code: u16, reason: &str);
    /// Cuts the socket with no close handshake.
    fn terminate(&self);
    /// Stops reading frames off the socket; the place link reads it again once its own listener stands
    /// (serve.ts:700-702).
    fn pause(&self);
    /// The socket as the place door's link takes it once a place proved (serve.ts:706).
    fn link(self: Rc<Self>) -> Rc<dyn LinkSocket>;
}

/// A frame as it came off the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Incoming {
    Text(String),
    Binary(Vec<u8>),
}

/// What the door decided this socket is (serve.ts:505-538). Decided when it is let in and never again, except that a
/// thread's token read at the auth frame sets `stamped` and `by`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Road {
    /// The road the host stamped over whatever the client's frames say: a ticket's, or a thread's.
    pub stamped: Option<WorkspaceOrigin>,
    /// A computer the person paired: its own token opened this socket, or the ticket it redeemed was minted by one.
    pub paired_road: bool,
    /// Who this socket is once let in.
    pub me: Option<Authed>,
    /// The thread this socket is, when its token was one the host minted into a turn's launch.
    pub by: Option<ThreadScope>,
}

impl Road {
    /// `stamped === undefined`: the person's own road. serve.ts:538.
    pub fn own_road(&self) -> bool {
        todo!()
    }
    /// `pairedRoad && stamped !== "relayed"`. serve.ts:664.
    pub fn as_paired(&self) -> bool {
        todo!()
    }
    /// `own_road() && me is the host`: the gate of the ops only this host's own process may ask.
    pub fn host_only(&self) -> bool {
        todo!()
    }
    /// What every verb is handed as where the request came from (serve.ts:855-856, spec D30-D31): `paired` for a
    /// paired computer, else the stamp, else the frame's own word; with the thread beside it where the socket is one.
    pub fn origin(&self, sent: Option<WorkspaceOrigin>) -> Option<Caller> {
        todo!()
    }
}

/// One live socket. Its fields are the door's state; `Served` holds what every socket of one server shares.
pub struct Socket {
    pub(crate) served: Rc<Served>,
    pub(crate) sink: Rc<dyn FrameSink>,
    pub(crate) upgrade: UpgradeHead,
    /// Where the socket came from, as the app shows it beside a computer that joined (serve.ts:496).
    pub(crate) from: String,
    pub(crate) road: RefCell<Road>,
    pub(crate) authed: Cell<bool>,
    pub(crate) deciding: Cell<bool>,
    pub(crate) proving: RefCell<Option<(String, Vec<u8>)>>,
    pub(crate) handed_over: Cell<bool>,
    pub(crate) seal: RefCell<Option<Seal>>,
    pub(crate) seal_expect: RefCell<Option<Vec<u8>>>,
    pub(crate) sent_before_auth: Cell<usize>,
    /// The door's two limits are on while this is false (serve.ts:594-603).
    pub(crate) through: Cell<bool>,
    pub(crate) detaches: Rc<RefCell<Vec<Box<dyn FnOnce()>>>>,
    pub(crate) slate_holds: Rc<RefCell<SlateHolds>>,
    /// The daemon links this socket holds, by the id it was answered with (serve.ts:559-561).
    pub(crate) channels: Rc<RefCell<IndexMap<String, Rc<dyn DaemonChannel>>>>,
    pub(crate) sign_in_ops: RefCell<Option<Rc<dyn SocketOps>>>,
    pub(crate) sys_ops: RefCell<Option<Rc<dyn SocketOps>>>,
    /// The device hold a revoke cuts, by its id in `Served::held` (serve.ts:568, 612-613).
    pub(crate) bound: Cell<Option<u64>>,
}

impl Socket {
    /// A socket the listener just upgraded (serve.ts:493-603): redeems a ticket in the query, else starts the
    /// deadline. A refused ticket closes the sink 4401 and the socket reads nothing.
    pub fn opened(served: &Rc<Served>, upgrade: UpgradeHead, sink: Rc<dyn FrameSink>) -> Rc<Socket> {
        todo!()
    }
    /// What the door decided, as it stands.
    pub fn road(&self) -> Road {
        todo!()
    }
    /// Whether the socket is let in.
    pub fn authed(&self) -> bool {
        todo!()
    }
    /// `ws.send`: dropped when not open, sealed where a seal stands (serve.ts:616-620, spec W10).
    pub fn send(&self, payload: Payload) {
        todo!()
    }
    /// `answering(frame)`: sends, then logs the refusal against the request it answers (serve.ts:621-626, spec
    /// D20-D24). `frame` is the parsed frame (`None` where it did not parse); `said` the schema's issues line.
    pub fn answer(&self, frame: Option<&serde_json::Value>, payload: Payload, said: Option<&str>) {
        todo!()
    }
    /// Raw bytes the socket delivered before it was let in (serve.ts:579-585, spec D4).
    pub fn bytes_arrived(&self, n: usize) {
        todo!()
    }
    /// The socket's close: every detach once in push order, and the device hold let go (serve.ts:569-573).
    pub fn closed(&self) {
        todo!()
    }
    /// Pushes a leave the socket's close runs.
    pub fn detach(&self, leave: Box<dyn FnOnce()>) {
        todo!()
    }
}
