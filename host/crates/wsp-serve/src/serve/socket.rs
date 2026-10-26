// SPDX-License-Identifier: AGPL-3.0-only
//! One socket, as the door decides it and the ops read it (serve.ts:493-626). The transport is a `FrameSink`, so
//! every unit but the listener is driven with a recording sink (spec/README.md, "The seams").
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_core::daemon_channel::DaemonChannel;
use wsp_machines::places::helpers::LinkSocket;
use wsp_store::keys::Seal;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};

use url::Url;
use wsp_wire::types::daemon_contract::{DAEMON_AUTH_DEADLINE_PASSED, DAEMON_PRE_AUTH_BYTES_EXCEEDED, PRE_AUTH_MAX_BYTES};
use wsp_wire::wire::devices::ticket_origin;
use wsp_wire::wire::limits::AUTH_DEADLINE_CLOSE;
use wsp_words::app_ports::peer_address;
use wsp_words::words::computer::UNAUTHORIZED;

use super::door::{refused_line, Served};

const UNAUTHORIZED_CLOSE: u16 = 4401;
use wsp_core::sign_in_socket::SignInOp;
use wsp_core::sys_socket::{Send, SocketSide, SysAsk};
use wsp_words::exit::HostError;

use super::served::Payload;
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
        self.stamped.is_none()
    }
    /// `pairedRoad && stamped !== "relayed"`. serve.ts:664.
    pub fn as_paired(&self) -> bool {
        self.paired_road && self.stamped != Some(WorkspaceOrigin::Relayed)
    }
    /// `own_road() && me is the host`: the gate of the ops only this host's own process may ask.
    pub fn host_only(&self) -> bool {
        self.own_road() && matches!(self.me, Some(Authed::Host))
    }
    /// What every verb is handed as where the request came from (serve.ts:855-856, spec D30-D31): `paired` for a
    /// paired computer, else the stamp, else the frame's own word; with the thread beside it where the socket is one.
    pub fn origin(&self, sent: Option<WorkspaceOrigin>) -> Option<Caller> {
        let road = if self.as_paired() { Some(WorkspaceOrigin::Paired) } else { self.stamped.or(sent) };
        match (&self.by, road) {
            (Some(by), Some(origin)) => Some(Caller::Scoped { origin, by: by.clone() }),
            (_, road) => road.map(Caller::Road),
        }
    }
}

/// One socket's handler of every sys op: wsp-core's `sys_socket`, handed this socket. sys-socket.ts:15.
pub type SysRoad = Rc<dyn Fn(SysAsk, Send, Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>;
/// One socket's handler of every sign-in op: wsp-core's `sign_in_socket`, handed this socket. sign-in-socket.ts:26.
pub type SignInRoad = Rc<dyn Fn(SignInOp, Send, Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>;

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
    /// The deadline's timer, cut when the socket is through the door.
    pub(crate) deadline: RefCell<Option<tokio::task::AbortHandle>>,
    pub(crate) detaches: Rc<RefCell<Vec<Box<dyn FnOnce()>>>>,
    pub(crate) slate_holds: Rc<RefCell<SlateHolds>>,
    /// The daemon links this socket holds, by the id it was answered with (serve.ts:559-561).
    pub(crate) channels: Rc<RefCell<IndexMap<String, Rc<dyn DaemonChannel>>>>,
    pub(crate) sign_in_road: RefCell<Option<SignInRoad>>,
    pub(crate) sys_road: RefCell<Option<SysRoad>>,
    /// The device hold a revoke cuts, by its id in `Served::held` (serve.ts:568, 612-613).
    pub(crate) bound: Cell<Option<u64>>,
}

impl Socket {
    /// A socket the listener just upgraded (serve.ts:493-603): redeems a ticket in the query, else starts the
    /// deadline. A refused ticket closes the sink 4401 and the socket reads nothing.
    pub fn opened(served: &Rc<Served>, upgrade: UpgradeHead, sink: Rc<dyn FrameSink>) -> Rc<Socket> {
        let from = match &served.opts.peer_of {
            Some(peer_of) => peer_of(&upgrade),
            None => peer_address(upgrade.peer.map(|peer| peer.ip().to_string()).as_deref()),
        };
        let socket = Rc::new(Socket {
            served: served.clone(),
            sink,
            from,
            road: RefCell::new(Road::default()),
            authed: Cell::new(false),
            deciding: Cell::new(false),
            proving: RefCell::new(None),
            handed_over: Cell::new(false),
            seal: RefCell::new(None),
            seal_expect: RefCell::new(None),
            sent_before_auth: Cell::new(0),
            through: Cell::new(false),
            deadline: RefCell::new(None),
            detaches: Rc::new(RefCell::new(Vec::new())),
            slate_holds: Rc::new(RefCell::new(SlateHolds::default())),
            channels: Rc::new(RefCell::new(IndexMap::new())),
            sign_in_road: RefCell::new(None),
            sys_road: RefCell::new(None),
            bound: Cell::new(None),
            upgrade,
        });
        if let Some(ticket) = ticket_param(&socket.upgrade) {
            let held = served.tickets.borrow_mut().shift_remove(&ticket);
            match held {
                Some(held) if served.now() <= held.expires_at => {
                    let mut road = socket.road.borrow_mut();
                    road.stamped = Some(ticket_origin(held.purpose));
                    road.paired_road = held.paired;
                    road.me = Some(Authed::Host);
                    drop(road);
                    socket.authed.set(true);
                }
                _ => {
                    // A refused ticket's socket reads nothing, as the TS returns before it listens.
                    socket.handed_over.set(true);
                    socket.sink.close(UNAUTHORIZED_CLOSE, UNAUTHORIZED);
                    return socket;
                }
            }
        }
        let channels = socket.channels.clone();
        socket.detach(Box::new(move || {
            let held: Vec<_> = channels.borrow_mut().drain(..).map(|(_, channel)| channel).collect();
            for channel in held {
                channel.close();
            }
        }));
        if !socket.authed.get() {
            let weak = Rc::downgrade(&socket);
            socket.detach(Box::new(move || {
                if let Some(socket) = weak.upgrade() {
                    socket.through_door();
                }
            }));
            let weak = Rc::downgrade(&socket);
            let wait = served.auth_deadline_ms;
            let timer = tokio::task::spawn_local(async move {
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
                if let Some(socket) = weak.upgrade() {
                    socket.through_door();
                    socket.sink.close(AUTH_DEADLINE_CLOSE as u16, DAEMON_AUTH_DEADLINE_PASSED);
                }
            });
            *socket.deadline.borrow_mut() = Some(timer.abort_handle());
        }
        socket
    }
    /// What the door decided, as it stands.
    pub fn road(&self) -> Road {
        self.road.borrow().clone()
    }
    /// Whether the socket is let in.
    pub fn authed(&self) -> bool {
        self.authed.get()
    }
    /// `ws.send`: dropped when not open, sealed where a seal stands (serve.ts:616-620, spec W10).
    pub fn send(&self, payload: Payload) {
        if !self.sink.is_open() {
            return;
        }
        let text = serde_json::Value::Object(payload).to_string();
        match self.seal.borrow_mut().as_mut() {
            None => self.sink.send_text(text),
            Some(seal) => self.sink.send_binary(seal.seal(&text)),
        }
    }
    /// `answering(frame)`: sends, then logs the refusal against the request it answers (serve.ts:621-626, spec
    /// D20-D24). `frame` is the parsed frame (`None` where it did not parse); `said` the schema's issues line.
    pub fn answer(&self, frame: Option<&serde_json::Value>, payload: Payload, said: Option<&str>) {
        let refused = payload.get("ok") == Some(&serde_json::Value::Bool(false));
        if !refused {
            self.send(payload);
            return;
        }
        let line = refused_line(frame, &payload, said);
        self.send(payload);
        if let Some(log) = &self.served.opts.log {
            log(&line);
        }
    }
    /// Raw bytes the socket delivered before it was let in (serve.ts:579-585, spec D4).
    pub fn bytes_arrived(&self, n: usize) {
        if self.through.get() {
            return;
        }
        let sent = self.sent_before_auth.get() + n;
        self.sent_before_auth.set(sent);
        if sent <= PRE_AUTH_MAX_BYTES as usize {
            return;
        }
        self.through_door();
        self.sink.close(UNAUTHORIZED_CLOSE, DAEMON_PRE_AUTH_BYTES_EXCEEDED);
    }
    /// The socket's close: every detach once in push order, and the device hold let go (serve.ts:569-573).
    pub fn closed(&self) {
        let leaves: Vec<_> = self.detaches.borrow_mut().drain(..).collect();
        for leave in leaves {
            leave();
        }
        if let Some(id) = self.bound.take() {
            self.served.held.borrow_mut().shift_remove(&id);
        }
    }
    /// Pushes a leave the socket's close runs.
    pub fn detach(&self, leave: Box<dyn FnOnce()>) {
        self.detaches.borrow_mut().push(leave);
    }
}

/// `?ticket=` of the upgrade's query, the first one.
fn ticket_param(upgrade: &UpgradeHead) -> Option<String> {
    let url = Url::parse(&format!("ws://localhost{}", upgrade.path_and_query)).ok()?;
    let found = url.query_pairs().find(|(key, _)| key == "ticket").map(|(_, value)| value.into_owned());
    found
}

/// What the per-socket roads of wsp-core ask of a socket: whether it is open, and where its leaves go.
impl SocketSide for Socket {
    fn open(&self) -> bool {
        self.sink.is_open()
    }
    fn detach(&self, leave: Box<dyn FnOnce()>) {
        Socket::detach(self, leave)
    }
}
