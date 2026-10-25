// SPDX-License-Identifier: AGPL-3.0-only
//! spec/reach.md; ported from packages/runtime/test/reach.test.ts (14 cases) plus the cases the spec pins. The TS ran
//! a real daemon over a fake /proc; here the fake daemon answers as its inbound door does.
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use support::fake_daemon::FakeDaemon;
use support::net::tcp_proxy;
use support::{run_local, settle, until};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;
use wsp_core::reach::{connect_daemon, daemon_ws_url, ReachOptions, TakenSocket};
use wsp_store::keys::{make_seal, seal_keys, Side, SEAL_REFUSAL};
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::daemon_events::DaemonEvent;

use DaemonLinkStatus::{Connecting, Dead, Live, ReauthNeeded};

const TOKEN: &str = "reach-token";

type Statuses = Rc<RefCell<Vec<DaemonLinkStatus>>>;
type Events = Rc<RefCell<Vec<DaemonEvent>>>;

fn opts(url: String, token: Option<&str>) -> ReachOptions {
    ReachOptions {
        preview_url: Some(url),
        socket: None,
        token: token.map(str::to_string),
        on_event: Rc::new(|_| {}),
        on_status: None,
        heartbeat_ms: None,
        seal: None,
        backoff_ms: None,
        clock: None,
        ids: None,
    }
}

fn watching(o: &mut ReachOptions) -> (Statuses, Events) {
    let statuses: Statuses = Rc::default();
    let events: Events = Rc::default();
    let (s, e) = (statuses.clone(), events.clone());
    o.on_status = Some(Rc::new(move |x| s.borrow_mut().push(x)));
    o.on_event = Rc::new(move |x| e.borrow_mut().push(x));
    (statuses, events)
}

fn inbox_seen(events: &Events, path: &str) -> usize {
    events.borrow().iter().filter(|e| matches!(e, DaemonEvent::InboxFile { path: p, .. } if p == path)).count()
}

fn params(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn turns_a_preview_url_into_a_wss_url_carrying_the_edge_token_and_never_ours() {
    let url = daemon_ws_url("https://abc123-7070.preview.getsolari.com/?pt_token=edge").unwrap();
    assert_eq!(url, "wss://abc123-7070.preview.getsolari.com/?pt_token=edge");
    assert!(!url.contains("reach-token"));
    assert_eq!(daemon_ws_url("http://127.0.0.1:7070/x?a=1").unwrap(), "ws://127.0.0.1:7070/x?a=1");
}

#[test]
fn keeps_ws_urls_as_they_are() {
    assert_eq!(daemon_ws_url("ws://127.0.0.1:7070").unwrap(), "ws://127.0.0.1:7070/");
    assert_eq!(daemon_ws_url("wss://edge.example/").unwrap(), "wss://edge.example/");
}

#[test]
fn an_unparsable_preview_url_is_refused() {
    assert!(daemon_ws_url("not a url").is_err());
}

#[test]
fn heartbeats_at_the_configured_interval_and_gets_acks_back() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let mut o = opts(daemon.url(), Some(TOKEN));
        o.heartbeat_ms = Some(100);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        until("three pongs", 5_000, || reach.stats().pongs_received >= 3).await;
        assert!(reach.stats().pings_sent >= 3);
        let pings: Vec<Value> = daemon.frames().into_iter().filter(|f| f["op"] == "ping").collect();
        assert!(pings.len() >= 3);
        assert!(pings.iter().all(|f| f.as_object().unwrap().keys().collect::<Vec<_>>() == ["id", "op"]));
        reach.close();
    });
}

#[test]
fn a_ping_left_unanswered_for_a_beat_cuts_the_socket_and_redials() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        let mut o = opts(format!("ws://127.0.0.1:{}", proxy.port), Some(TOKEN));
        o.heartbeat_ms = Some(50);
        o.backoff_ms = Some(Rc::new(|_| 25));
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        proxy.stall();
        until("a redial", 5_000, || statuses.borrow().len() >= 3).await;
        proxy.resume();
        until("live again", 5_000, || statuses.borrow().iter().filter(|s| **s == Live).count() >= 2).await;
        assert_eq!(statuses.borrow()[..4], [Connecting, Live, Connecting, Live]);
        reach.close();
    });
}

#[test]
fn survives_a_cut_socket_reconnects_resubscribes_and_rescans_the_inbox() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        daemon.add_inbox_file("/inbox/before.bin", 11);
        let proxy = tcp_proxy(daemon.port, 0).await;
        let mut o = opts(format!("ws://127.0.0.1:{}", proxy.port), Some(TOKEN));
        o.heartbeat_ms = Some(100);
        o.backoff_ms = Some(Rc::new(|_| 300));
        let (_, events) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        // The first rescan announced the file already there.
        until("the first rescan", 5_000, || inbox_seen(&events, "/inbox/before.bin") >= 1).await;
        assert_eq!(daemon.ops()[..4], ["auth", "ports.watch", "inbox.watch", "inbox.rescan"]);
        // The connection dies and a file lands while nobody is subscribed, so its push goes to no one.
        proxy.cut_all();
        settle().await;
        daemon.add_inbox_file("/inbox/during-gap.bin", 22);
        // Only a rescan after the reconnect announces the settled file again, and recovers the one lost in the gap.
        until("the second rescan", 5_000, || inbox_seen(&events, "/inbox/before.bin") >= 2).await;
        until("the gap's file", 5_000, || inbox_seen(&events, "/inbox/during-gap.bin") >= 1).await;
        until("the reconnect counted", 5_000, || reach.stats().reconnects >= 1).await;
        // ports.watch and inbox.watch were subscribed again on the new socket.
        daemon.set_listener(9999, 42);
        until("the port", 5_000, || events.borrow().iter().any(|e| matches!(e, DaemonEvent::PortOpen { port, .. } if port.0 == 9999.0))).await;
        daemon.add_inbox_file("/inbox/after.bin", 33);
        until("the file after", 5_000, || inbox_seen(&events, "/inbox/after.bin") >= 1).await;
        let pongs = reach.stats().pongs_received;
        until("a pong on the new socket", 5_000, || reach.stats().pongs_received > pongs).await;
        reach.close();
    });
}

#[test]
fn reports_connecting_then_live_on_first_connect() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let mut o = opts(daemon.url(), Some(TOKEN));
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        assert_eq!(*statuses.borrow(), [Connecting, Live]);
        assert_eq!(reach.status(), Some(Live));
        reach.close();
    });
}

#[test]
fn cycles_connecting_live_across_a_cut_socket_without_duplicate_emissions() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        let mut o = opts(format!("ws://127.0.0.1:{}", proxy.port), Some(TOKEN));
        o.heartbeat_ms = Some(100);
        o.backoff_ms = Some(Rc::new(|_| 25));
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        proxy.cut_all();
        until("live twice", 5_000, || statuses.borrow().iter().filter(|s| **s == Live).count() >= 2).await;
        assert_eq!(*statuses.borrow(), [Connecting, Live, Connecting, Live]);
        reach.close();
    });
}

#[test]
fn rejects_ready_and_reports_reauth_needed_on_a_bad_token() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let mut o = opts(daemon.url(), Some("wrong-token"));
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        let failed = reach.ready().await.err().unwrap();
        assert_eq!(failed.message, "daemon rejected token (4401 daemon token refused; the host holds the current one)");
        assert_eq!(*statuses.borrow(), [Connecting, ReauthNeeded]);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(daemon.sockets(), 1, "reauth is terminal: no redial");
        reach.close();
    });
}

#[test]
fn dials_again_when_the_daemons_auth_deadline_passed_while_this_hosts_loop_stood_still_and_goes_live() {
    run_local(async {
        let daemon = FakeDaemon::start_with(TOKEN, 100);
        let proxy = tcp_proxy(daemon.port, 0).await;
        // The daemon's 101 reaches this host, which then stands still past the daemon's deadline before its auth
        // frame leaves: the daemon closes on the deadline, and the token it never read was not refused.
        proxy.freeze_after_next_reply(400);
        let mut o = opts(format!("ws://127.0.0.1:{}", proxy.port), Some(TOKEN));
        o.backoff_ms = Some(Rc::new(|_| 25));
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        assert!(!statuses.borrow().contains(&ReauthNeeded));
        assert_eq!(reach.status(), Some(Live));
        assert_eq!(reach.stats().reconnects, 0, "the first socket never finished its ritual");
        assert_eq!(daemon.sockets(), 2);
        reach.close();
    });
}

#[test]
fn surfaces_a_post_establishment_4401_as_reauth_needed_and_stops_retrying() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        let port = proxy.port;
        let mut o = opts(format!("ws://127.0.0.1:{port}"), Some(TOKEN));
        o.heartbeat_ms = Some(100);
        o.backoff_ms = Some(Rc::new(|_| 25));
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        // The token stops being honoured mid-session: every redial now gets a 4401.
        proxy.close().await;
        let rejecting = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        let dials = Rc::new(RefCell::new(0));
        let d = dials.clone();
        tokio::task::spawn_local(async move {
            while let Ok((sock, _)) = rejecting.accept().await {
                *d.borrow_mut() += 1;
                if let Ok(mut ws) = tokio_tungstenite::accept_async(sock).await {
                    let _ = ws.send(Message::Close(Some(CloseFrame { code: CloseCode::from(4401), reason: "unauthorized".into() }))).await;
                    let _ = ws.next().await;
                }
            }
        });
        until("reauth-needed", 5_000, || statuses.borrow().contains(&ReauthNeeded)).await;
        assert_eq!(*statuses.borrow(), [Connecting, Live, Connecting, ReauthNeeded]);
        let seen = *dials.borrow();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(*dials.borrow(), seen, "reauth is terminal: retrying cannot fix a bad token");
        assert_eq!(reach.status(), Some(ReauthNeeded));
        reach.close();
    });
}

#[test]
fn reports_dead_once_the_client_closes() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let mut o = opts(daemon.url(), Some(TOKEN));
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        reach.close();
        assert_eq!(*statuses.borrow(), [Connecting, Live, Dead]);
        assert_eq!(reach.status(), Some(Dead));
        assert!(reach.ready().await.is_ok(), "ready settled once and stays resolved");
    });
}

#[test]
fn close_fails_every_pending_request_with_client_closed() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        let reach = connect_daemon(opts(format!("ws://127.0.0.1:{}", proxy.port), Some(TOKEN))).unwrap();
        reach.ready().await.unwrap();
        proxy.stall();
        let asking = reach.clone();
        let pending = tokio::task::spawn_local(async move { asking.request("ping", Map::new()).await });
        settle().await;
        reach.close();
        assert_eq!(pending.await.unwrap().err().unwrap().message, "client closed");
        assert_eq!(reach.request("ping", Map::new()).await.err().unwrap().message, "daemon unreachable");
    });
}

#[test]
fn a_close_before_the_first_connect_rejects_ready() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        proxy.stall();
        let reach = connect_daemon(opts(format!("ws://127.0.0.1:{}", proxy.port), Some(TOKEN))).unwrap();
        settle().await;
        reach.close();
        assert_eq!(reach.ready().await.err().unwrap().message, "client closed");
    });
}

#[test]
fn keeps_retrying_while_the_daemon_is_unreachable_and_connects_once_it_is_back() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let dead = tcp_proxy(daemon.port, 0).await;
        let port = dead.port;
        dead.close().await;
        let mut o = opts(format!("ws://127.0.0.1:{port}"), Some(TOKEN));
        o.heartbeat_ms = Some(100);
        o.backoff_ms = Some(Rc::new(|_| 25));
        let reach = connect_daemon(o).unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(reach.status(), Some(Connecting));
        let _back = tcp_proxy(daemon.port, port).await;
        reach.ready().await.unwrap();
        until("a pong", 5_000, || reach.stats().pongs_received >= 1).await;
        reach.close();
    });
}

#[test]
fn takes_a_socket_somebody_else_opened_without_an_auth_frame_of_its_own_and_is_dead_when_that_socket_goes() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/", proxy.port)).await.unwrap();
        // The daemon's inbound door wants an auth frame first, so this socket is authed the way an inbound one is
        // and then handed over: what this proves is that the reach sends none of its own.
        ws.send(Message::text(json!({"id": 0, "op": "auth", "token": TOKEN}).to_string())).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut o = opts(String::new(), None);
        o.preview_url = None;
        o.socket = Some(TakenSocket::new(ws));
        o.heartbeat_ms = Some(50);
        let (statuses, _) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        assert_eq!(*statuses.borrow(), [Live], "a taken socket never reads connecting");
        until("a pong", 5_000, || reach.stats().pongs_received >= 1).await;
        assert_eq!(daemon.ops().iter().filter(|op| *op == "auth").count(), 1);
        assert_eq!(daemon.ops()[1..4], ["ports.watch", "inbox.watch", "inbox.rescan"]);
        assert_eq!(reach.stats().reconnects, 0);
        proxy.cut_all();
        until("dead", 5_000, || reach.status() == Some(Dead)).await;
        assert_eq!(daemon.sockets(), 1, "nothing redials a taken socket");
        assert_eq!(*statuses.borrow(), [Live, Dead]);
    });
}

#[test]
fn stamps_its_own_id_and_op_over_any_the_callers_params_carry() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let frames: Rc<RefCell<Vec<Value>>> = Rc::default();
        let held_reply: Rc<RefCell<Option<Value>>> = Rc::default();
        let (f, r) = (frames.clone(), held_reply.clone());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        tokio::task::spawn_local(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(sock).await.unwrap();
            let (mut out, mut inn) = ws.split();
            loop {
                tokio::select! {
                    Some(text) = rx.recv() => { out.send(Message::text(text)).await.unwrap(); }
                    Some(Ok(Message::Text(t))) = inn.next() => {
                        let frame: Value = serde_json::from_str(&t).unwrap();
                        f.borrow_mut().push(frame.clone());
                        let reply = json!({"id": frame["id"], "ok": true, "answered": frame["op"]});
                        if frame["op"] == "hold" { *r.borrow_mut() = Some(reply); } else { out.send(Message::text(reply.to_string())).await.unwrap(); }
                    }
                }
            }
        });
        let mut o = opts(format!("ws://127.0.0.1:{port}"), Some(TOKEN));
        o.heartbeat_ms = Some(60_000);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        assert_eq!(frames.borrow()[0].to_string(), format!(r#"{{"token":"{TOKEN}","id":1,"op":"auth"}}"#));
        let holding = reach.clone();
        let held = tokio::task::spawn_local(async move { holding.request("hold", Map::new()).await });
        until("the hold", 5_000, || frames.borrow().iter().any(|f| f["op"] == "hold")).await;
        let held_id = frames.borrow().iter().find(|f| f["op"] == "hold").unwrap()["id"].clone();
        for p in [json!({"id": held_id}), json!({"op": "machine.kill"})] {
            let count = frames.borrow().len();
            let answer = reach.request("ping", params(p.clone())).await.unwrap();
            let sent = frames.borrow()[count].clone();
            assert_ne!(sent["id"], held_id);
            assert_eq!(sent["op"], json!("ping"));
            assert_eq!(answer["answered"], json!("ping"));
            // The caller's key keeps its place: params' keys first, then id, then op.
            let keys: Vec<String> = sent.as_object().unwrap().keys().cloned().collect();
            assert_eq!(keys, if p.get("id").is_some() { vec!["id", "op"] } else { vec!["op", "id"] });
        }
        tx.send(held_reply.borrow_mut().take().unwrap().to_string()).unwrap();
        assert_eq!(held.await.unwrap().unwrap()["answered"], json!("hold"));
        reach.close();
    });
}

#[test]
fn a_refused_request_carries_the_daemons_kind_status_and_code() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
            while let Some(Ok(Message::Text(t))) = ws.next().await {
                let f: Value = serde_json::from_str(&t).unwrap();
                let reply = match f["op"].as_str().unwrap() {
                    "git.read" => json!({"id": f["id"], "ok": false, "error": "no remote", "kind": "missing", "status": 404, "code": "no-remote"}),
                    "odd" => json!({"id": f["id"], "ok": false, "kind": "not-a-kind", "status": "404", "code": 7}),
                    _ => json!({"id": f["id"], "ok": true}),
                };
                ws.send(Message::text(reply.to_string())).await.unwrap();
            }
        });
        let reach = connect_daemon(opts(format!("ws://127.0.0.1:{port}"), Some(TOKEN))).unwrap();
        reach.ready().await.unwrap();
        let refused = reach.request("git.read", Map::new()).await.err().unwrap();
        assert_eq!(refused.message, "no remote");
        assert_eq!(serde_json::to_value(refused.kind).unwrap(), json!("missing"));
        assert_eq!(refused.status, Some(404.0));
        assert_eq!(refused.code.as_deref(), Some("no-remote"));
        let odd = reach.request("odd", Map::new()).await.err().unwrap();
        assert_eq!((odd.message.as_str(), odd.kind, odd.status, odd.code), ("daemon error", None, None, None));
        reach.close();
    });
}

#[test]
fn a_frame_that_is_no_daemon_event_is_dropped() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
            while let Some(Ok(Message::Text(t))) = ws.next().await {
                let f: Value = serde_json::from_str(&t).unwrap();
                if f["op"] == "inbox.rescan" {
                    for e in ["not json", r#"{"type":"no.such.event"}"#, r#"{"type":"inbox.file","path":"/a"}"#, r#"{"type":"inbox.file","path":"/b","bytes":3}"#] {
                        ws.send(Message::text(e)).await.unwrap();
                    }
                }
                ws.send(Message::text(json!({"id": f["id"], "ok": true}).to_string())).await.unwrap();
            }
        });
        let mut o = opts(format!("ws://127.0.0.1:{port}"), Some(TOKEN));
        let (statuses, events) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        reach.ready().await.unwrap();
        assert_eq!(events.borrow().len(), 1);
        assert_eq!(inbox_seen(&events, "/b"), 1);
        assert_eq!(*statuses.borrow(), [Connecting, Live], "an unsealed bad frame cuts nothing");
        reach.close();
    });
}

#[test]
fn refuses_to_be_built_with_neither_a_route_to_dial_nor_a_socket_to_take() {
    let mut o = opts(String::new(), None);
    o.preview_url = None;
    let failed = connect_daemon(o).err().unwrap();
    assert_eq!(failed.message, "connectDaemon needs a previewUrl to dial or an open socket to take over");
    let failed = connect_daemon(opts("ws://127.0.0.1:1".into(), None)).err().unwrap();
    assert_eq!(failed.message, "connectDaemon needs the daemon's token to dial a previewUrl");
}

#[test]
fn a_sealed_link_sends_binary_frames_and_cuts_on_a_frame_that_does_not_open() {
    run_local(async {
        let keys = seal_keys(&[7u8; 32], "pl_test");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen: Rc<RefCell<Vec<String>>> = Rc::default();
        let closes: Rc<RefCell<Vec<(u16, String)>>> = Rc::default();
        let (s, c) = (seen.clone(), closes.clone());
        let place_keys = keys.clone();
        tokio::task::spawn_local(async move {
            let mut place = make_seal(&place_keys, Side::Place);
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
            while let Some(Ok(m)) = ws.next().await {
                match m {
                    Message::Binary(b) => {
                        let text = place.unseal(&b).unwrap();
                        let f: Value = serde_json::from_str(&text).unwrap();
                        s.borrow_mut().push(f["op"].as_str().unwrap().to_string());
                        if f["op"] == "inbox.watch" {
                            // A frame nobody sealed: the reach cuts the link rather than read it.
                            ws.send(Message::text(r#"{"type":"inbox.file","path":"/x","bytes":1}"#)).await.unwrap();
                        }
                        ws.send(Message::binary(place.seal(&json!({"id": f["id"], "ok": true}).to_string()))).await.unwrap();
                    }
                    Message::Close(Some(frame)) => c.borrow_mut().push((u16::from(frame.code), frame.reason.to_string())),
                    _ => {}
                }
            }
        });
        let mut o = opts(format!("ws://127.0.0.1:{port}"), Some(TOKEN));
        o.seal = Some(make_seal(&keys, Side::Host));
        o.backoff_ms = Some(Rc::new(|_| 60_000));
        let (_, events) = watching(&mut o);
        let reach = connect_daemon(o).unwrap();
        until("the cut", 5_000, || !closes.borrow().is_empty()).await;
        assert_eq!(seen.borrow()[..3], ["auth", "ports.watch", "inbox.watch"]);
        assert_eq!(*closes.borrow(), vec![(1002, SEAL_REFUSAL.to_string())]);
        assert!(events.borrow().is_empty());
        reach.close();
    });
}
