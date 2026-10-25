// SPDX-License-Identifier: AGPL-3.0-only
//! spec/tunnels.md; ported from packages/runtime/test/place-forward.test.ts (4 cases), plus the tunnel cases the
//! spec pins over real loopback sockets: nothing read before the open answers, a refused tunnel's own answer, a
//! failed write dropping the connection, `touched` and `opened`, the frames for an id nobody holds, and the close.
mod rest;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use futures::FutureExt;
use indexmap::IndexMap;
use rest::*;
use serde_json::{json, Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use wsp_core::place_forward::{open_place_forward, PlaceForward};
use wsp_core::reach::{DaemonReach, Frame, ReachError, ReachStats};
use wsp_core::tunnel::{plumb_tunnel, tunnel_frame, TunnelConn, TunnelConns, TunnelLink, TunnelOptions};
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

/// A link that answers the three tunnel ops and records what it was asked, as a place's daemon does
/// (place-forward.test.ts:117). `refuse` fails tunnel.open; `hold` keeps tunnel.open unanswered until sent;
/// `fail_writes` fails every tunnel.write.
#[derive(Default)]
struct Link {
    asked: RefCell<Vec<(String, Frame)>>,
    refuse: bool,
    fail_writes: bool,
    hold: RefCell<Option<oneshot::Receiver<()>>>,
}

impl Link {
    fn asked(&self) -> Vec<(String, Value)> {
        self.asked.borrow().iter().map(|(op, p)| (op.clone(), Value::Object(p.clone()))).collect()
    }
    fn ops(&self) -> Vec<String> {
        self.asked.borrow().iter().map(|(op, _)| op.clone()).collect()
    }
    async fn answer(&self, op: &str, params: Frame) -> Result<Frame, String> {
        self.asked.borrow_mut().push((op.to_string(), params));
        if op == "tunnel.open" {
            let held = self.hold.borrow_mut().take();
            if let Some(rx) = held {
                let _ = rx.await;
            }
            if self.refuse {
                return Err("nothing answers on that port".into());
            }
        }
        if op == "tunnel.write" && self.fail_writes {
            return Err("the link went".into());
        }
        Ok(Map::new())
    }
}

#[async_trait(?Send)]
impl DaemonReach for Link {
    async fn ready(&self) -> Result<(), ReachError> {
        Ok(())
    }
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, ReachError> {
        self.answer(op, params).await.map_err(|message| ReachError { message, kind: None, status: None, code: None })
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {}
}

fn tunnel_link(link: &Rc<Link>) -> TunnelLink {
    let link = link.clone();
    Rc::new(move |op: &str, params: Frame| {
        let (link, op) = (link.clone(), op.to_string());
        async move { link.answer(&op, params).await.map_err(HostError::new) }.boxed_local()
    })
}

async fn until(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..400 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}");
}

async fn dial(port: u16) -> std::io::Result<TcpStream> {
    TcpStream::connect(("127.0.0.1", port)).await
}

/// A connected pair: the far end the test holds as the browser, and the near end the road carries.
async fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (browser, accepted) = tokio::join!(dial(port), listener.accept());
    (browser.unwrap(), accepted.unwrap().0)
}

async fn read_some(s: &mut TcpStream) -> String {
    let mut buf = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(2), s.read(&mut buf)).await.expect("bytes came").unwrap();
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// Reads until the far side shuts its write half (0 bytes), answering everything read before it.
async fn read_to_end(s: &mut TcpStream) -> String {
    let mut out = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), s.read_to_end(&mut out)).await.expect("the far side ended").ok();
    String::from_utf8_lossy(&out).into_owned()
}

fn tunnel_id(link: &Link) -> String {
    link.asked()[0].1["tunnelId"].as_str().unwrap().to_string()
}

fn opts(conns: &TunnelConns, id: &str) -> TunnelOptions {
    TunnelOptions { port: 7000, conns: conns.clone(), tunnel_id: id.into(), touched: None, refused: None, opened: None }
}

// TS: place-forward.test.ts:140
#[test]
fn opens_one_tunnel_on_the_link_per_connection_on_the_port_that_computer_said_its_daemon_bound() {
    run_local(async {
        let link = Rc::new(Link::default());
        let forward = open_place_forward(link.clone(), 34567).await.unwrap();
        assert!(forward.port > 0);
        let mut socket = dial(forward.port).await.unwrap();
        socket.write_all(b"hello").await.unwrap();
        until("the write is carried", || link.ops().contains(&"tunnel.write".to_string())).await;
        let asked = link.asked();
        assert_eq!(asked[0].0, "tunnel.open");
        assert_eq!(asked[0].1["port"], 34567);
        let id = tunnel_id(&link);
        assert_eq!(id, "pl1");
        assert_eq!(asked.iter().find(|(op, _)| op == "tunnel.write").unwrap().1, json!({ "tunnelId": "pl1", "data": b64("hello") }));
        forward.event(&DaemonEvent::TunnelData { tunnel_id: id, data: b64("there"), machine_id: None });
        assert_eq!(read_some(&mut socket).await, "there");
        forward.close();
    });
}

// TS: place-forward.test.ts:157
#[test]
fn ends_the_connection_when_the_far_side_ends_its_tunnel_and_closes_the_tunnel_when_the_connection_goes() {
    run_local(async {
        let link = Rc::new(Link::default());
        let forward = open_place_forward(link.clone(), 1234).await.unwrap();
        let mut socket = dial(forward.port).await.unwrap();
        socket.write_all(b"x").await.unwrap();
        until("the write is carried", || link.ops().contains(&"tunnel.write".to_string())).await;
        let id = tunnel_id(&link);
        forward.event(&DaemonEvent::TunnelEnd { tunnel_id: id.clone(), machine_id: None });
        assert_eq!(read_to_end(&mut socket).await, "");
        drop(socket);
        until("the tunnel is closed", || link.ops().contains(&"tunnel.close".to_string())).await;
        let close = link.asked().into_iter().find(|(op, _)| op == "tunnel.close").unwrap();
        assert_eq!(close.1, json!({ "tunnelId": id }));
        forward.close();
    });
}

// TS: place-forward.test.ts:172
#[test]
fn drops_a_connection_the_far_side_would_not_open_a_tunnel_for_rather_than_holding_it_on_nothing() {
    run_local(async {
        let link = Rc::new(Link { refuse: true, ..Link::default() });
        let forward = open_place_forward(link.clone(), 1234).await.unwrap();
        let mut socket = dial(forward.port).await.unwrap();
        let _ = socket.write_all(b"x").await;
        assert_eq!(read_to_end(&mut socket).await, "", "the connection is closed with nothing said");
        assert!(link.ops().contains(&"tunnel.open".to_string()));
        assert!(!link.ops().contains(&"tunnel.write".to_string()));
        forward.close();
    });
}

// TS: place-forward.test.ts:182
#[test]
fn stops_answering_once_it_is_closed_so_a_link_that_went_takes_its_port_with_it() {
    run_local(async {
        let link = Rc::new(Link::default());
        let forward = open_place_forward(link.clone(), 1234).await.unwrap();
        let port = forward.port;
        forward.close();
        settle().await;
        assert!(dial(port).await.is_err());
    });
}

// spec: F2, F3 (each connection is its own tunnel, pl1 then pl2; close destroys every connection riding it, twice is nothing)
#[test]
fn a_forward_numbers_its_tunnels_and_close_drops_every_connection_once() {
    run_local(async {
        let link = Rc::new(Link::default());
        let forward: PlaceForward = open_place_forward(link.clone(), 4321).await.unwrap();
        let mut a = dial(forward.port).await.unwrap();
        let mut b = dial(forward.port).await.unwrap();
        until("both tunnels opened", || link.ops().iter().filter(|op| *op == "tunnel.open").count() == 2).await;
        let mut ids: Vec<String> = link.asked().into_iter().filter(|(op, _)| op == "tunnel.open").map(|(_, p)| p["tunnelId"].as_str().unwrap().to_string()).collect();
        ids.sort();
        assert_eq!(ids, ["pl1", "pl2"]);
        forward.close();
        forward.close();
        assert_eq!(read_to_end(&mut a).await, "");
        assert_eq!(read_to_end(&mut b).await, "");
        let other = open_place_forward(link.clone(), 4321).await.unwrap();
        let _c = dial(other.port).await.unwrap();
        until("a third tunnel opened", || link.ops().iter().filter(|op| *op == "tunnel.open").count() == 3).await;
        assert_eq!(link.asked().into_iter().filter(|(op, _)| op == "tunnel.open").last().unwrap().1["tunnelId"], "pl1", "each forward counts its own");
        other.close();
    });
}

// spec: F4 (a frame for a tunnel this forward does not hold, or any other event, touches nothing)
#[test]
fn a_forward_ignores_a_frame_for_a_tunnel_it_does_not_hold() {
    run_local(async {
        let link = Rc::new(Link::default());
        let forward = open_place_forward(link.clone(), 1).await.unwrap();
        forward.event(&DaemonEvent::TunnelData { tunnel_id: "pl9".into(), data: b64("x"), machine_id: None });
        forward.event(&DaemonEvent::TunnelEnd { tunnel_id: "pl9".into(), machine_id: None });
        assert_eq!(link.ops(), Vec::<String>::new());
        forward.close();
    });
}

// spec: T1, T2 (registered at once; nothing read is sent before tunnel.open answers; then every chunk in order)
#[test]
fn bytes_read_before_the_open_answers_wait_and_go_after_it_in_order() {
    run_local(async {
        let (tx, rx) = oneshot::channel();
        let link = Rc::new(Link { hold: RefCell::new(Some(rx)), ..Link::default() });
        let conns: TunnelConns = Rc::new(RefCell::new(IndexMap::new()));
        let (mut browser, near) = pair().await;
        plumb_tunnel(tunnel_link(&link), near, opts(&conns, "t1"));
        assert_eq!(conns.borrow().keys().cloned().collect::<Vec<_>>(), ["t1"], "the connection is held before the open answers");
        browser.write_all(b"early").await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(link.ops(), ["tunnel.open"]);
        assert_eq!(link.asked()[0].1, json!({ "tunnelId": "t1", "port": 7000 }));
        tx.send(()).unwrap();
        until("the early bytes are carried", || link.ops().len() >= 2).await;
        let written: String = link.asked().into_iter().filter(|(op, _)| op == "tunnel.write").map(|(_, p)| String::from_utf8(base64::engine::general_purpose::STANDARD.decode(p["data"].as_str().unwrap()).unwrap()).unwrap()).collect();
        assert_eq!(written, "early");
        drop(browser);
        until("the tunnel is closed", || link.ops().last().map(String::as_str) == Some("tunnel.close")).await;
        assert!(conns.borrow().is_empty(), "a closed connection leaves the map");
    });
}

// spec: T3 (a refused tunnel hands the connection to `refused`, which answers it; the close still sends tunnel.close)
#[test]
fn a_refused_tunnel_is_answered_by_the_callers_own_words_and_still_closed_on_the_link() {
    run_local(async {
        let link = Rc::new(Link { refuse: true, ..Link::default() });
        let conns: TunnelConns = Rc::new(RefCell::new(IndexMap::new()));
        let (mut browser, near) = pair().await;
        let mut o = opts(&conns, "t1");
        o.refused = Some(Rc::new(|conn: &TunnelConn| conn.end(Some(b"HTTP/1.1 502 Bad Gateway\r\n\r\n".to_vec()))));
        plumb_tunnel(tunnel_link(&link), near, o);
        assert_eq!(read_to_end(&mut browser).await, "HTTP/1.1 502 Bad Gateway\r\n\r\n");
        drop(browser);
        until("the tunnel is closed", || link.ops().contains(&"tunnel.close".to_string())).await;
        assert_eq!(link.ops(), ["tunnel.open", "tunnel.close"]);
        assert!(conns.borrow().is_empty());
    });
}

// spec: T4 (a tunnel.write the link fails destroys the connection)
#[test]
fn a_write_the_link_fails_drops_the_connection() {
    run_local(async {
        let link = Rc::new(Link { fail_writes: true, ..Link::default() });
        let conns: TunnelConns = Rc::new(RefCell::new(IndexMap::new()));
        let (mut browser, near) = pair().await;
        plumb_tunnel(tunnel_link(&link), near, opts(&conns, "t1"));
        until("the tunnel opened", || link.ops() == ["tunnel.open"]).await;
        browser.write_all(b"x").await.unwrap();
        assert_eq!(read_to_end(&mut browser).await, "");
        until("the tunnel is closed", || link.ops().contains(&"tunnel.close".to_string())).await;
        assert!(conns.borrow().is_empty());
    });
}

// spec: T5 (touched once at the start and once per chunk either way; opened once after the open answers)
#[test]
fn touched_hears_the_start_and_every_chunk_and_opened_hears_the_open_once() {
    run_local(async {
        let link = Rc::new(Link::default());
        let conns: TunnelConns = Rc::new(RefCell::new(IndexMap::new()));
        let (mut browser, near) = pair().await;
        let touches = Rc::new(RefCell::new(0));
        let opens = Rc::new(RefCell::new(0));
        let mut o = opts(&conns, "t1");
        let (t, op) = (touches.clone(), opens.clone());
        o.touched = Some(Rc::new(move || *t.borrow_mut() += 1));
        o.opened = Some(Rc::new(move || *op.borrow_mut() += 1));
        plumb_tunnel(tunnel_link(&link), near, o);
        assert_eq!(*touches.borrow(), 1, "touched at the start, before the open answers");
        until("the tunnel opened", || *opens.borrow() == 1).await;
        browser.write_all(b"one").await.unwrap();
        until("the chunk is carried", || link.ops().contains(&"tunnel.write".to_string())).await;
        assert_eq!(*touches.borrow(), 2);
        let t2 = touches.clone();
        let event = DaemonEvent::TunnelData { tunnel_id: "t1".into(), data: b64("back"), machine_id: None };
        assert!(tunnel_frame(&conns, &event, Some(&move || *t2.borrow_mut() += 1)));
        assert_eq!(read_some(&mut browser).await, "back");
        assert_eq!(*touches.borrow(), 3);
        assert_eq!(*opens.borrow(), 1);
    });
}

// spec: T6 (tunnel_frame: true only for tunnel.data and tunnel.end naming a held id; nothing else is touched)
#[test]
fn tunnel_frame_answers_whether_the_frame_was_one_of_these_connections() {
    run_local(async {
        let link = Rc::new(Link::default());
        let conns: TunnelConns = Rc::new(RefCell::new(IndexMap::new()));
        let (mut browser, near) = pair().await;
        plumb_tunnel(tunnel_link(&link), near, opts(&conns, "t1"));
        until("the tunnel opened", || link.ops() == ["tunnel.open"]).await;
        let touched = Rc::new(RefCell::new(0));
        let t = touched.clone();
        let touch = move || *t.borrow_mut() += 1;
        assert!(!tunnel_frame(&conns, &DaemonEvent::TunnelData { tunnel_id: "t9".into(), data: b64("x"), machine_id: None }, Some(&touch)));
        assert!(!tunnel_frame(&conns, &DaemonEvent::TunnelEnd { tunnel_id: "t9".into(), machine_id: None }, Some(&touch)));
        let other = DaemonEvent::PtyData { pty_id: "t1".into(), data: b64("x") };
        assert!(!tunnel_frame(&conns, &other, Some(&touch)));
        assert_eq!(*touched.borrow(), 0);
        assert!(tunnel_frame(&conns, &DaemonEvent::TunnelEnd { tunnel_id: "t1".into(), machine_id: None }, Some(&touch)));
        assert_eq!(*touched.borrow(), 0, "an end touches nothing");
        assert_eq!(read_to_end(&mut browser).await, "");
        assert_eq!(conns.borrow().len(), 1, "an ended connection is held until it closes");
    });
}
