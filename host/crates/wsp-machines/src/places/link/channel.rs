// SPDX-License-Identifier: AGPL-3.0-only
//! The channel to one computer's daemon over its link, the tunnel that carries a forward's connections and the interval the seen stamp rides.

use std::cell::{Cell, RefCell};
use std::future::poll_fn;
use std::net::SocketAddr;
use std::rc::Rc;
use std::task::Poll;

use crate::local_task::spawn as spawn_local;
use base64::Engine as _;
use futures::channel::{mpsc, oneshot};
use futures::future::{LocalBoxFuture, Shared};
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::io::AsyncReadExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use wsp_core::clock::Cancel as ClockCancel;
use wsp_core::types::pending::{Clock, DaemonReach};
use wsp_wire::wire::machine_link::{DaemonResponse, SysSample};
use wsp_words::exit::{HostError, Kind};
use wsp_words::workspace_state::absent_computer;

use super::LinkDoor;
use crate::places::context::PlaceDoorContext;
use crate::places::helpers::{refused_host, Cancel, ForwardConn, LinkSocket, LoopbackListener};
use crate::places::readings::{PlaceReadings, Release};
use crate::places::setup::PlaceSetupArea;
use crate::places::types::PlaceResult;

/// A join answered and waiting for its prove (views.ts:319).
/// A channel to one computer's daemon over the link it holds (link.ts:232).
pub struct PlaceChannel {
    ctx: Rc<PlaceDoorContext>,
    readings: Rc<PlaceReadings>,
    setup: Rc<PlaceSetupArea>,
    place_id: String,
    link: Rc<dyn DaemonReach>,
    socket: Rc<dyn LinkSocket>,
    reader: u64,
    state: RefCell<ChannelState>,
}

type HeldWatch = Shared<LocalBoxFuture<'static, Result<Release, HostError>>>;

#[derive(Default)]
struct ChannelState {
    close_listener: Option<u64>,
    readings: Option<HeldWatch>,
    ended: Option<ChannelGone>,
    waiting: Vec<oneshot::Sender<ChannelGone>>,
}

/// How a channel ended: the socket's code and reason, or `1000, "closed here"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelGone {
    pub code: u16,
    pub reason: String,
}

fn daemon_answer(v: Value) -> PlaceResult<DaemonResponse> {
    serde_json::from_value(v).map_err(|e| HostError::new(e.to_string()))
}

fn frame(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

impl PlaceChannel {
    pub(super) fn open(
        door: &LinkDoor,
        place_id: &str,
        link: Rc<dyn DaemonReach>,
        socket: Rc<dyn LinkSocket>,
        reader: u64,
    ) -> Rc<PlaceChannel> {
        let (ctx, readings, setup) = (
            Rc::clone(&door.ctx),
            Rc::clone(&door.readings),
            Rc::clone(&door.setup),
        );
        let channel = Rc::new(PlaceChannel {
            ctx,
            readings,
            setup,
            place_id: place_id.to_string(),
            link,
            socket: Rc::clone(&socket),
            reader,
            state: RefCell::new(ChannelState::default()),
        });
        let held = Rc::clone(&channel);
        let close_listener = socket.on_close(Box::new(move |code, reason| {
            held.gone(code, reason);
        }));
        channel.state.borrow_mut().close_listener = Some(close_listener);
        channel
    }

    /// One frame up the link (`op` and the rest its params), `sys.watch` and `sys.unwatch` held here.
    pub async fn send(self: &Rc<Self>, frame: Map<String, Value>) -> PlaceResult<DaemonResponse> {
        let mut params = frame;
        let op = params
            .shift_remove("op")
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let live = self
            .ctx
            .live
            .borrow()
            .get(&self.place_id)
            .map(|held| Rc::clone(&held.reach));
        let Some(on) = live.filter(|on| Rc::ptr_eq(on, &self.link)) else {
            let name = self
                .ctx
                .kept
                .borrow()
                .get(&self.place_id)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| self.place_id.clone());
            return Err(HostError::with_kind(
                absent_computer(&name, None).sentence,
                Kind::parse("absent"),
            ));
        };
        if op == "sys.watch" {
            let answer = self.watch_hold().await;
            return match answer {
                Ok(()) => daemon_answer(json!({ "id": null, "ok": true })),
                Err(e) => daemon_answer(json!({ "id": null, "ok": false, "error": e.message })),
            };
        }
        if op == "sys.unwatch" {
            self.unhold();
            return daemon_answer(json!({ "id": null, "ok": true }));
        }
        let mut said = on.request(&op, params).await.map_err(refused_host)?;
        said.entry("id").or_insert(Value::Null);
        daemon_answer(Value::Object(said))
    }

    /// Settles once with how the channel ended.
    pub async fn closed(&self) -> ChannelGone {
        let waiting = {
            let mut state = self.state.borrow_mut();
            if let Some(gone) = &state.ended {
                return gone.clone();
            }
            let (tx, rx) = oneshot::channel();
            state.waiting.push(tx);
            rx
        };
        waiting.await.unwrap_or_else(|_| ChannelGone {
            code: 1000,
            reason: "closed here".to_string(),
        })
    }

    pub fn close(self: &Rc<Self>) {
        self.forget();
        self.end(ChannelGone {
            code: 1000,
            reason: "closed here".to_string(),
        });
    }

    /// This channel's hold on the computer's one readings watch, taken once for every `sys.watch` that asks.
    async fn watch_hold(&self) -> Result<(), HostError> {
        let standing = self.state.borrow().readings.clone();
        let held = match standing {
            Some(held) => held,
            None => {
                let held = self.watch_future();
                self.state.borrow_mut().readings = Some(held.clone());
                held
            }
        };
        match held.await {
            Ok(_) => Ok(()),
            Err(e) => {
                self.state.borrow_mut().readings = None;
                Err(e)
            }
        }
    }

    fn watch_future(&self) -> HeldWatch {
        let readings = Rc::clone(&self.readings);
        let place = self.place_id.clone();
        async move { readings.watch(&place, Rc::new(|_: &SysSample| {})).await }
            .boxed_local()
            .shared()
    }

    /// The channel's hold let go: the shared watch stops once its last reader has let go too.
    fn unhold(&self) {
        let held = self.state.borrow_mut().readings.take();
        if let Some(held) = held {
            spawn_local(async move {
                if let Ok(release) = held.await {
                    release();
                }
            });
        }
    }

    /// The channel's reader, its close listener and its hold come off with the channel.
    fn forget(&self) {
        let listener = self.state.borrow_mut().close_listener.take();
        {
            let mut channels = self.setup.channels.borrow_mut();
            let emptied = match channels.get_mut(&self.place_id) {
                Some(readers) => {
                    readers.shift_remove(&self.reader);
                    readers.is_empty()
                }
                None => false,
            };
            if emptied {
                channels.shift_remove(&self.place_id);
            }
        }
        if let Some(id) = listener {
            self.socket.off_close(id);
        }
        self.unhold();
    }

    fn gone(&self, code: u16, reason: String) {
        self.forget();
        self.end(ChannelGone { code, reason });
    }

    fn end(&self, gone: ChannelGone) {
        let waiting = {
            let mut state = self.state.borrow_mut();
            if state.ended.is_some() {
                return;
            }
            state.ended = Some(gone.clone());
            std::mem::take(&mut state.waiting)
        };
        for tx in waiting {
            let _ = tx.send(gone.clone());
        }
    }
}

/// One connection riding a forward: the bytes the far side sends it, and what ends it.
pub(super) enum Inbound {
    Bytes(Vec<u8>),
    End,
    Destroy,
}

pub(super) struct TunnelConn {
    pub(super) inbound: mpsc::UnboundedSender<Inbound>,
}

impl ForwardConn for TunnelConn {
    fn write(&self, bytes: &[u8]) {
        let _ = self.inbound.unbounded_send(Inbound::Bytes(bytes.to_vec()));
    }
    fn end(&self) {
        let _ = self.inbound.unbounded_send(Inbound::End);
    }
    fn destroy(&self) {
        let _ = self.inbound.unbounded_send(Inbound::Destroy);
    }
}

pub(super) type Conns = Rc<RefCell<IndexMap<String, Rc<dyn ForwardConn>>>>;

/// Runs one tunnel's connection: the open, the bytes each way, then the close the far side is told of.
pub(super) async fn run_tunnel(
    conn: TcpStream,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
    reach: Rc<dyn DaemonReach>,
    into: Conns,
    id: String,
    port: u16,
    used: Option<Rc<dyn Fn()>>,
) {
    let opened = reach
        .request(
            "tunnel.open",
            frame(json!({ "tunnelId": id, "port": port })),
        )
        .await;
    if opened.is_ok() {
        let (mut rd, mut wr) = conn.into_split();
        pump(&mut rd, &mut wr, &mut inbound, &reach, &id).await;
    } else {
        drop(conn);
    }
    into.borrow_mut().shift_remove(&id);
    if let Some(used) = &used {
        used();
    }
    let _ = reach
        .request("tunnel.close", frame(json!({ "tunnelId": id })))
        .await;
}

async fn read_chunk(rd: &mut OwnedReadHalf) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0u8; 16 * 1024];
    let n = rd.read(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

/// Carries bytes both ways until either end of the connection ends or fails. A `tunnel.write` goes out as its chunk is
/// read and its answer is waited on beside the reads, never before them (link.ts:477); one that fails ends the
/// connection.
async fn pump(
    rd: &mut OwnedReadHalf,
    wr: &mut OwnedWriteHalf,
    inbound: &mut mpsc::UnboundedReceiver<Inbound>,
    reach: &Rc<dyn DaemonReach>,
    id: &str,
) {
    use tokio::io::AsyncWriteExt;
    let mut writes = FuturesUnordered::new();
    loop {
        tokio::select! {
            chunk = read_chunk(rd) => match chunk {
                Ok(bytes) if !bytes.is_empty() => {
                    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    let mut ask = reach.request("tunnel.write", frame(json!({ "tunnelId": id, "data": data })));
                    match futures::poll!(ask.as_mut()) {
                        Poll::Ready(Err(_)) => return,
                        Poll::Ready(Ok(_)) => {}
                        Poll::Pending => writes.push(ask),
                    }
                }
                _ => return,
            },
            Some(answer) = writes.next(), if !writes.is_empty() => {
                if answer.is_err() {
                    return;
                }
            }
            msg = inbound.next() => match msg {
                Some(Inbound::Bytes(bytes)) => {
                    if wr.write_all(&bytes).await.is_err() {
                        return;
                    }
                }
                Some(Inbound::End) => {
                    let _ = wr.shutdown().await;
                }
                Some(Inbound::Destroy) | None => return,
            },
        }
    }
}

/// Listens on loopback on a port the system picks, handing each connection to `on_conn` (link.ts:489).
pub(super) async fn listen_loopback(
    on_conn: Rc<dyn Fn(TcpStream)>,
) -> Result<(Rc<LoopbackListener>, u16), HostError> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .map_err(|e| HostError::new(e.to_string()))?;
    let addr = listener
        .local_addr()
        .map_err(|e| HostError::new(e.to_string()))?;
    let slot = Rc::new(RefCell::new(Some(listener)));
    let task = spawn_local({
        let slot = Rc::clone(&slot);
        async move {
            while let Some(stream) = accept_next(&slot).await {
                on_conn(stream);
            }
        }
    });
    let stop = task.map(|task| task.abort_handle());
    let close: Box<dyn FnOnce()> = Box::new(move || {
        let listener = slot.borrow_mut().take();
        drop(listener);
        if let Some(stop) = &stop {
            stop.abort();
        }
    });
    Ok((
        Rc::new(LoopbackListener {
            addr,
            close: RefCell::new(Some(close)),
        }),
        addr.port(),
    ))
}

async fn accept_next(slot: &RefCell<Option<TcpListener>>) -> Option<TcpStream> {
    poll_fn(|cx| {
        let held = slot.borrow();
        let Some(listener) = held.as_ref() else {
            return Poll::Ready(None);
        };
        match listener.poll_accept(cx) {
            Poll::Ready(Ok((stream, _))) => Poll::Ready(Some(stream)),
            Poll::Ready(Err(_)) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    })
    .await
}

/// Runs `tick` every `ms` on `clock` until the answer runs (the TS interval and its `clearInterval`).
pub(super) fn every(clock: Rc<dyn Clock>, ms: u64, tick: Rc<dyn Fn()>) -> Cancel {
    let stopped = Rc::new(Cell::new(false));
    let armed: Rc<RefCell<Option<ClockCancel>>> = Rc::new(RefCell::new(None));
    arm(clock, ms, tick, Rc::clone(&stopped), Rc::clone(&armed));
    Box::new(move || {
        stopped.set(true);
        let cancel = armed.borrow_mut().take();
        if let Some(cancel) = cancel {
            cancel();
        }
    })
}

fn arm(
    clock: Rc<dyn Clock>,
    ms: u64,
    tick: Rc<dyn Fn()>,
    stopped: Rc<Cell<bool>>,
    armed: Rc<RefCell<Option<ClockCancel>>>,
) {
    let next = (
        Rc::clone(&clock),
        Rc::clone(&tick),
        Rc::clone(&stopped),
        Rc::clone(&armed),
    );
    let cancel = clock.schedule(
        Box::new(move || {
            let (clock, tick, stopped, armed) = next;
            if stopped.get() {
                return;
            }
            tick();
            arm(clock, ms, tick, stopped, armed);
        }),
        ms as i64,
    );
    *armed.borrow_mut() = Some(cancel);
}
