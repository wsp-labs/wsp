// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/box-panels.test.ts:375-435 (the pane ports) and :314 (listenNear) at the pane ports' seam. Spec: spec/places_pane_ports.md.
mod common;

use std::cell::{Cell, RefCell};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

use indexmap::IndexMap;
use tokio::net::TcpStream;
use wsp_machines::places::helpers::{listen_near, Forward, ForwardConn, OnConn};
use wsp_machines::places::pane_ports::{ForwardEvent, PaneAsker, PaneForwards, PaneOnConn, PanePorts, PanePortsDeps};
use wsp_wire::wire::limits::{PortForward, PortForwardKind, RelayPort, FORWARD_MAX_PER_TARGET};
use wsp_wire::wire::place_link::{pane_forward_cap_line, pane_forward_floor_line, pane_forward_gone_line, pane_forward_quiet_line, pane_forward_stopped_line};
use wsp_words::exit::{HostError, Kind};

use common::places::*;

static PORTS: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    PORTS.lock().unwrap_or_else(|e| e.into_inner())
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").expect("a port").local_addr().expect("an address").port()
}

fn free_ports(n: usize) -> Vec<u16> {
    let held: Vec<TcpListener> = (0..n).map(|_| TcpListener::bind("127.0.0.1:0").expect("a port")).collect();
    held.iter().map(|l| l.local_addr().expect("an address").port()).collect()
}

/// A port with the `n - 1` above it free too, so a walk up from it is exact.
fn run_of(n: u16) -> u16 {
    loop {
        let first = TcpListener::bind("127.0.0.1:0").expect("a port");
        let p = first.local_addr().expect("an address").port();
        if p as u32 + n as u32 + 1 > 65_535 {
            continue;
        }
        let mut held = vec![first];
        let all_free = (1..n).all(|i| match TcpListener::bind(("127.0.0.1", p + i)) {
            Ok(l) => {
                held.push(l);
                true
            }
            Err(_) => false,
        });
        if all_free {
            return p;
        }
    }
}

fn has_v6() -> bool {
    TcpListener::bind("[::1]:0").is_ok()
}

async fn connects(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).await.is_ok()
}

async fn gone(port: u16) -> bool {
    for _ in 0..50 {
        if !connects(port).await {
            return true;
        }
        settle().await;
    }
    false
}

fn asker(workspace_id: &str, name: &str) -> PaneAsker {
    PaneAsker { workspace_id: workspace_id.to_string(), name: name.to_string() }
}

fn view(workspace_id: &str, port: u16, name: &str, at: i64) -> PortForward {
    PortForward { workspace_id: workspace_id.to_string(), port: RelayPort((port as i64).into()), started_at: iso(at), name: name.to_string(), kind: PortForwardKind::Url }
}

fn open(v: &PortForward) -> ForwardEvent {
    ForwardEvent::Open { forward: v.clone() }
}

fn close(workspace_id: &str, port: u16) -> ForwardEvent {
    ForwardEvent::Close { workspace_id: workspace_id.to_string(), port }
}

fn refused(got: Result<u16, HostError>, line: String) {
    let e = got.expect_err("a refusal");
    assert_eq!(e.message, line);
    assert_eq!(e.kind, Some(Kind::Usage));
}

struct FakeConn {
    destroyed: Cell<bool>,
}

impl FakeConn {
    fn new() -> Rc<FakeConn> {
        Rc::new(FakeConn { destroyed: Cell::new(false) })
    }
}

impl ForwardConn for FakeConn {
    fn write(&self, _bytes: &[u8]) {}
    fn end(&self) {}
    fn destroy(&self) {
        self.destroyed.set(true);
    }
}

type Conns = Rc<RefCell<IndexMap<String, Rc<dyn ForwardConn>>>>;

struct Pane {
    clock: Rc<TestClock>,
    forwards: Rc<RefCell<IndexMap<String, Forward>>>,
    ports: Rc<PanePorts>,
    events: Rc<RefCell<Vec<ForwardEvent>>>,
    accepted: Rc<Cell<usize>>,
    streams: Rc<RefCell<Vec<TcpStream>>>,
    on_conn: PaneOnConn,
}

impl Pane {
    fn new() -> Pane {
        let clock = TestClock::new();
        let forwards = Rc::new(RefCell::new(IndexMap::new()));
        let ports = PanePorts::new(PanePortsDeps { forwards: Rc::clone(&forwards), clock: clock.clone() });
        let events = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&events);
        let _ = ports.on(Rc::new(move |e: &ForwardEvent| log.borrow_mut().push(e.clone())));
        let accepted = Rc::new(Cell::new(0));
        let streams = Rc::new(RefCell::new(Vec::new()));
        let (count, keep) = (Rc::clone(&accepted), Rc::clone(&streams));
        let on_conn: PaneOnConn = Rc::new(move |stream, used| {
            count.set(count.get() + 1);
            keep.borrow_mut().push(stream);
            used();
        });
        Pane { clock, forwards, ports, events, accepted, streams, on_conn }
    }

    async fn reach(&self, key: &str, port: u32, workspace_id: &str) -> Result<u16, HostError> {
        let name = if workspace_id == "ws1" { "spoo-ts" } else { "elsewhere" };
        self.ports.reach(key, port, asker(workspace_id, name), Rc::clone(&self.on_conn)).await
    }

    fn conns(&self, key: &str) -> Conns {
        Rc::clone(&self.forwards.borrow().get(key).expect("a forward under the key").conns)
    }

    fn keys(&self) -> Vec<String> {
        self.forwards.borrow().keys().cloned().collect()
    }

    fn events(&self) -> Vec<ForwardEvent> {
        self.events.borrow().clone()
    }
}

#[test]
fn opens_at_the_asked_number_and_says_so() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        let v = view("ws1", p, "spoo-ts", T0);
        assert_eq!(pane.ports.list(), vec![v.clone()]);
        assert_eq!(pane.events(), vec![open(&v)]);
        assert_eq!(pane.keys(), vec!["k".to_string()]);
        assert_eq!(pane.forwards.borrow()["k"].local_port, p);
        assert!(connects(p).await);
        until("the connection reached the pane's handler", || pane.accepted.get() == 1).await;
        assert_eq!(pane.streams.borrow().len(), 1);
    });
}

#[test]
fn stands_while_the_pane_keeps_asking_and_goes_once_it_stops() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        assert!(connects(p).await);
        for _ in 0..11 {
            pane.clock.pass(60_000).await;
            assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        }
        assert!(connects(p).await);
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0))]);
        pane.clock.pass(120_000).await;
        assert!(gone(p).await);
        assert!(pane.ports.list().is_empty());
        assert!(pane.keys().is_empty());
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0)), close("ws1", p)]);
        assert_eq!(pane.clock.pending(), 0);
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        assert!(connects(p).await);
    });
}

#[test]
fn goes_after_a_quiet_hour_tells_the_pane_once_and_opens_again() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        for minute in 0..60 {
            pane.clock.pass(60_000).await;
            if minute < 59 {
                assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
            }
        }
        assert!(gone(p).await);
        assert!(pane.ports.list().is_empty());
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0)), close("ws1", p)]);
        refused(pane.reach("k", p as u32, "ws1").await, pane_forward_quiet_line(p as i64));
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        assert!(connects(p).await);
        assert_eq!(pane.ports.list(), vec![view("ws1", p, "spoo-ts", T0 + 3_600_000)]);
    });
}

#[test]
fn an_open_connection_holds_it_past_the_hold() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        let conns = pane.conns("k");
        let conn = FakeConn::new();
        conns.borrow_mut().insert("t1".to_string(), conn.clone());
        pane.clock.pass(200_000).await;
        assert_eq!(pane.ports.list(), vec![view("ws1", p, "spoo-ts", T0)]);
        assert_eq!(pane.keys(), vec!["k".to_string()]);
        assert!(!conn.destroyed.get());
        assert!(connects(p).await);
        conns.borrow_mut().shift_remove("t1");
        pane.clock.pass(9_999).await;
        assert_eq!(pane.ports.list().len(), 1);
        pane.clock.pass(1).await;
        assert!(gone(p).await);
        assert!(pane.ports.list().is_empty());
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0)), close("ws1", p)]);
        assert!(!conn.destroyed.get());
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
    });
}

#[test]
fn a_connection_moves_the_quiet_hour() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        for minute in 1..=89 {
            pane.clock.pass(60_000).await;
            assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
            if minute == 30 {
                let client = TcpStream::connect(("127.0.0.1", p)).await.expect("a connection");
                until("the connection reached the pane's handler", || pane.accepted.get() == 1).await;
                drop(client);
            }
        }
        assert_eq!(pane.ports.standing("k", p as u32).unwrap(), p);
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0))]);
        pane.clock.pass(60_000).await;
        assert!(gone(p).await);
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0)), close("ws1", p)]);
        refused(pane.reach("k", p as u32, "ws1").await, pane_forward_quiet_line(p as i64));
    });
}

#[test]
fn refuses_a_port_below_1024_or_above_65535() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        for bad in [80u32, 1023, 65_536] {
            refused(pane.reach("k", bad, "ws1").await, pane_forward_floor_line(bad as i64));
        }
        assert!(pane.ports.list().is_empty());
        assert!(pane.keys().is_empty());
        assert!(pane.events().is_empty());

        let _top = TcpListener::bind(("127.0.0.1", 65_535));
        let e = pane.reach("k", 65_535, "ws1").await.expect_err("65535 passes the floor and finds no free number");
        assert_eq!(e.message, "no port from 65535 to 65555 is free on this computer");

        let _bottom = TcpListener::bind(("127.0.0.1", 1024));
        let local = pane.reach("k", 1024, "ws1").await.expect("1024 passes the floor");
        assert!(local > 1024 && local <= 1044, "{local}");
        assert_eq!(pane.ports.list(), vec![view("ws1", local, "spoo-ts", T0)]);
    });
}

#[test]
fn refuses_the_seventeenth_for_one_workspace_and_not_another() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let cap = FORWARD_MAX_PER_TARGET as usize;
        let ports = free_ports(cap + 3);
        for (i, p) in ports[..cap].iter().enumerate() {
            assert_eq!(pane.reach(&format!("k{i}"), *p as u32, "ws1").await.unwrap(), *p);
        }
        refused(pane.reach("k16", ports[cap] as u32, "ws1").await, pane_forward_cap_line("spoo-ts", FORWARD_MAX_PER_TARGET));
        assert!(!connects(ports[cap]).await);
        assert_eq!(pane.ports.list().len(), cap);
        assert_eq!(pane.forwards.borrow().len(), cap);

        assert_eq!(pane.reach("o", ports[cap + 1] as u32, "ws2").await.unwrap(), ports[cap + 1]);
        assert_eq!(pane.ports.list().len(), cap + 1);

        assert!(pane.ports.stop("ws1", ports[0]));
        assert_eq!(pane.reach("k16", ports[cap] as u32, "ws1").await.unwrap(), ports[cap]);
        refused(pane.reach("k17", ports[cap + 2] as u32, "ws1").await, pane_forward_cap_line("spoo-ts", FORWARD_MAX_PER_TARGET));
        assert!(!connects(ports[cap + 2]).await);
    });
}

#[test]
fn the_ports_list_stops_one_and_its_pane_is_told_once() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let ports = free_ports(3);
        let (p1, p2, p3) = (ports[0], ports[1], ports[2]);
        assert_eq!(pane.reach("k1", p1 as u32, "ws1").await.unwrap(), p1);
        assert_eq!(pane.reach("k2", p2 as u32, "ws1").await.unwrap(), p2);
        assert_eq!(pane.reach("k3", p3 as u32, "ws2").await.unwrap(), p3);
        let (v1, v2, v3) = (view("ws1", p1, "spoo-ts", T0), view("ws1", p2, "spoo-ts", T0), view("ws2", p3, "elsewhere", T0));
        assert_eq!(pane.ports.list(), vec![v1.clone(), v2.clone(), v3.clone()]);
        assert_eq!(pane.ports.standing("k2", p2 as u32).unwrap(), p2);

        let conn = FakeConn::new();
        pane.conns("k1").borrow_mut().insert("t1".to_string(), conn.clone());
        assert!(pane.ports.stop("ws1", p1));
        assert!(conn.destroyed.get());
        assert!(gone(p1).await);
        assert_eq!(pane.ports.list(), vec![v2.clone(), v3.clone()]);
        assert_eq!(pane.keys(), vec!["k2".to_string(), "k3".to_string()]);
        let told = vec![open(&v1), open(&v2), open(&v3), close("ws1", p1)];
        assert_eq!(pane.events(), told);

        let e = pane.ports.standing("k1", p1 as u32).expect_err("a forward that does not stand");
        assert_eq!(e.message, pane_forward_gone_line(p1 as i64));
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(gone(p1).await);
        assert_eq!(pane.events(), told);

        refused(pane.reach("k1", p1 as u32, "ws1").await, pane_forward_stopped_line(p1 as i64));
        assert!(gone(p1).await);
        assert_eq!(pane.events(), told);

        assert_eq!(pane.reach("k1", p1 as u32, "ws1").await.unwrap(), p1);
        assert!(connects(p1).await);
        assert_eq!(pane.ports.list(), vec![v2, v3, view("ws1", p1, "spoo-ts", T0)]);
    });
}

#[test]
fn stop_of_a_port_nobody_holds_is_false_and_changes_nothing() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        assert!(!pane.ports.stop("ws1", 4000));
        assert!(pane.events().is_empty());

        let p = run_of(2);
        let blocker = TcpListener::bind(("127.0.0.1", p)).expect("the asked number taken");
        let local = pane.reach("k", p as u32, "ws1").await.unwrap();
        assert!(local > p && local <= p + 20, "{local}");
        let v = view("ws1", local, "spoo-ts", T0);
        assert_eq!(pane.ports.standing("k", p as u32).unwrap(), local);

        assert!(!pane.ports.stop("ws1", p));
        assert!(!pane.ports.stop("ws2", local));
        assert_eq!(pane.ports.list(), vec![v.clone()]);
        assert_eq!(pane.events(), vec![open(&v)]);
        assert_eq!(pane.keys(), vec!["k".to_string()]);
        assert!(connects(local).await);
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), local);

        assert!(pane.ports.stop("ws1", local));
        assert!(gone(local).await);
        drop(blocker);
    });
}

#[test]
fn a_stop_after_the_hold_leaves_no_line() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        let conn = FakeConn::new();
        pane.conns("k").borrow_mut().insert("t1".to_string(), conn.clone());
        pane.clock.pass(130_000).await;
        assert_eq!(pane.ports.list().len(), 1);

        assert!(pane.ports.stop("ws1", p));
        assert!(conn.destroyed.get());
        assert!(gone(p).await);
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0)), close("ws1", p)]);
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        assert!(connects(p).await);
    });
}

#[test]
fn close_keeps_a_newer_forward_under_the_same_key() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let ports = free_ports(2);
        let (p1, p2) = (ports[0], ports[1]);
        assert_eq!(pane.reach("k", p1 as u32, "ws1").await.unwrap(), p1);
        assert_eq!(pane.reach("other", p2 as u32, "ws1").await.unwrap(), p2);
        let older = {
            let mut forwards = pane.forwards.borrow_mut();
            let newer = forwards.shift_remove("other").expect("the second forward");
            forwards.insert("k".to_string(), newer).expect("the first forward")
        };

        assert!(pane.ports.stop("ws1", p1));
        assert!(gone(p1).await);
        assert_eq!(pane.keys(), vec!["k".to_string()]);
        assert_eq!(pane.forwards.borrow()["k"].local_port, p2);
        assert!(connects(p2).await);
        assert_eq!(pane.ports.list(), vec![view("ws1", p2, "spoo-ts", T0)]);
        assert_eq!(pane.events().last(), Some(&close("ws1", p1)));
        drop(older);
    });
}

#[test]
fn a_held_forward_is_held_again_without_the_checks() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let cap = FORWARD_MAX_PER_TARGET as usize;
        let ports = free_ports(cap);
        for (i, p) in ports.iter().enumerate() {
            assert_eq!(pane.reach(&format!("k{i}"), *p as u32, "ws1").await.unwrap(), *p);
        }
        pane.clock.pass(100_000).await;
        assert_eq!(pane.reach("k0", 80, "ws1").await.unwrap(), ports[0]);
        assert_eq!(pane.ports.list().len(), cap);

        pane.clock.pass(100_000).await;
        assert_eq!(pane.ports.list(), vec![view("ws1", ports[0], "spoo-ts", T0)]);
        assert!(connects(ports[0]).await);
        pane.clock.pass(20_000).await;
        assert!(gone(ports[0]).await);
        assert!(pane.ports.list().is_empty());
    });
}

#[test]
fn a_timer_for_a_closed_forward_does_nothing() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        pane.clock.pass(60_000).await;
        assert!(pane.ports.stop("ws1", p));
        assert_eq!(pane.clock.pending(), 0);
        refused(pane.reach("k", p as u32, "ws1").await, pane_forward_stopped_line(p as i64));
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        let reopened = view("ws1", p, "spoo-ts", T0 + 60_000);
        assert_eq!(pane.ports.list(), vec![reopened.clone()]);

        pane.clock.pass(60_000).await;
        assert_eq!(pane.ports.list(), vec![reopened.clone()]);
        assert!(connects(p).await);
        assert_eq!(pane.events(), vec![open(&view("ws1", p, "spoo-ts", T0)), close("ws1", p), open(&reopened)]);

        pane.clock.pass(60_000).await;
        assert!(gone(p).await);
        assert!(pane.ports.list().is_empty());
        assert_eq!(pane.events().last(), Some(&close("ws1", p)));
        assert_eq!(pane.clock.pending(), 0);
    });
}

#[test]
fn on_answers_an_unsubscribe() {
    let _only = serial();
    block(async {
        let pane = Pane::new();
        let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let tag = |name: &'static str| {
            let log = Rc::clone(&log);
            Rc::new(move |e: &ForwardEvent| {
                let what = match e {
                    ForwardEvent::Open { forward } => format!("{name}:open:{}", forward.port.get()),
                    ForwardEvent::Close { port, .. } => format!("{name}:close:{port}"),
                };
                log.borrow_mut().push(what);
            })
        };
        let first = pane.ports.on(tag("a"));
        let _second = pane.ports.on(tag("b"));
        let p = free_port();
        assert_eq!(pane.reach("k", p as u32, "ws1").await.unwrap(), p);
        assert_eq!(*log.borrow(), vec![format!("a:open:{p}"), format!("b:open:{p}")]);

        first();
        assert!(pane.ports.stop("ws1", p));
        assert_eq!(*log.borrow(), vec![format!("a:open:{p}"), format!("b:open:{p}"), format!("b:close:{p}")]);
        assert_eq!(pane.events().len(), 2);
    });
}

fn counting() -> (OnConn, Rc<Cell<usize>>, Rc<RefCell<Vec<TcpStream>>>) {
    let accepted = Rc::new(Cell::new(0));
    let streams = Rc::new(RefCell::new(Vec::new()));
    let (count, keep) = (Rc::clone(&accepted), Rc::clone(&streams));
    let on_conn: OnConn = Rc::new(move |stream| {
        count.set(count.get() + 1);
        keep.borrow_mut().push(stream);
    });
    (on_conn, accepted, streams)
}

#[test]
fn listen_near_takes_the_next_free_number() {
    let _only = serial();
    block(async {
        let p = run_of(2);
        let blocker = TcpListener::bind(("127.0.0.1", p)).expect("the asked number taken");
        let (on_conn, accepted, _streams) = counting();
        let (listeners, at) = listen_near(p, Rc::clone(&on_conn)).await.unwrap();
        assert_eq!(at, p + 1);
        assert_eq!(listeners[0].addr.port(), at);
        assert!(connects(at).await);
        until("the connection reached the handler", || accepted.get() == 1).await;
        for l in &listeners {
            l.close();
        }
        assert!(gone(at).await);
        drop(blocker);

        if has_v6() {
            let p = run_of(2);
            let blocker = TcpListener::bind(("::1", p)).expect("the asked number taken on ::1");
            let (listeners, at) = listen_near(p, on_conn).await.unwrap();
            assert_eq!(at, p + 1);
            assert!(gone(p).await);
            for l in &listeners {
                l.close();
            }
            drop(blocker);
        }
    });
}

#[test]
fn listen_near_holds_both_loopbacks() {
    let _only = serial();
    block(async {
        let p = free_port();
        let (on_conn, accepted, _streams) = counting();
        let (listeners, at) = listen_near(p, on_conn).await.unwrap();
        assert_eq!(at, p);
        let addrs: Vec<String> = listeners.iter().map(|l| l.addr.to_string()).collect();
        if has_v6() {
            assert_eq!(addrs, vec![format!("127.0.0.1:{p}"), format!("[::1]:{p}")]);
            TcpStream::connect(("::1", p)).await.expect("a connection on ::1");
            TcpStream::connect(("127.0.0.1", p)).await.expect("a connection on 127.0.0.1");
            until("both connections reached the handler", || accepted.get() == 2).await;
        } else {
            assert_eq!(addrs, vec![format!("127.0.0.1:{p}")]);
        }
        for l in &listeners {
            l.close();
        }
        assert!(gone(p).await);
    });
}

#[test]
fn listen_near_says_no_port_is_free() {
    let _only = serial();
    block(async {
        let p = run_of(21);
        let blockers: Vec<TcpListener> = (0..=20).map(|i| TcpListener::bind(("127.0.0.1", p + i)).expect("a number taken")).collect();
        let (on_conn, _accepted, _streams) = counting();
        let e = listen_near(p, Rc::clone(&on_conn)).await.err().expect("no number free");
        assert_eq!(e.message, format!("no port from {p} to {} is free on this computer", p as u32 + 20));
        drop(blockers);

        let _top = TcpListener::bind(("127.0.0.1", 65_535));
        let e = listen_near(65_535, on_conn).await.err().expect("no number free at the top");
        assert_eq!(e.message, "no port from 65535 to 65555 is free on this computer");
    });
}
