// SPDX-License-Identifier: AGPL-3.0-only
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use support::status::*;
use support::{run_local, settle, until};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;
use wsp_core::daemon_channel::{open_daemon_channel, ChannelError, DaemonChannelOptions};
use wsp_core::place_forward::open_place_forward;
use wsp_core::reach::{connect_daemon, DaemonReach, Frame, ReachError, ReachOptions, ReachStats};
use wsp_core::status::{probe_reach, ProbeOptions, Reconcile, StatusListOptions, StatusWatchOptions};
use wsp_core::types::events::{event_bus, EventBus};
use wsp_engine::errors::EngineError;
use wsp_engine::machine::ExecResult;
use wsp_wire::views::workspace::ReachState;
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::daemon_events::parse_daemon_event;

fn chan_options(url: String, events: &Rc<RefCell<Vec<Value>>>) -> DaemonChannelOptions {
    let events = events.clone();
    DaemonChannelOptions {
        url,
        token: "t".into(),
        on_event: Rc::new(move |e| events.borrow_mut().push(e)),
        connect_timeout_ms: Some(5_000),
        clock: None,
    }
}

async fn read_head(sock: &mut tokio::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut b = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        if sock.read(&mut b).await.unwrap() == 0 {
            break;
        }
        buf.push(b[0]);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn reach_opts(url: String) -> ReachOptions {
    ReachOptions {
        preview_url: Some(url),
        socket: None,
        token: Some("t".into()),
        on_event: Rc::new(|_| {}),
        on_status: None,
        heartbeat_ms: None,
        seal: None,
        backoff_ms: None,
        clock: None,
        ids: None,
    }
}

struct FakeReach {
    seen: RefCell<Vec<(String, Instant)>>,
    delays: Vec<(&'static str, u64)>,
}

impl FakeReach {
    fn new(delays: &[(&'static str, u64)]) -> Rc<FakeReach> {
        Rc::new(FakeReach { seen: RefCell::default(), delays: delays.to_vec() })
    }
    fn count(&self, op: &str) -> usize {
        self.seen.borrow().iter().filter(|(o, _)| o == op).count()
    }
}

#[async_trait(?Send)]
impl DaemonReach for FakeReach {
    async fn ready(&self) -> Result<(), ReachError> {
        Ok(())
    }
    async fn request(&self, op: &str, _params: Frame) -> Result<Frame, ReachError> {
        self.seen.borrow_mut().push((op.to_string(), Instant::now()));
        if let Some((_, ms)) = self.delays.iter().find(|(o, _)| *o == op) {
            tokio::time::sleep(Duration::from_millis(*ms)).await;
        }
        Ok(Frame::new())
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        Some(DaemonLinkStatus::Live)
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {}
}

fn exec_of(code: i64, stdout: &str, stderr: &str) -> Result<ExecResult, EngineError> {
    Ok(serde_json::from_value(json!({"exitCode": code, "stdout": stdout, "stderr": stderr})).unwrap())
}

async fn zombie_lines(exec: Option<Result<ExecResult, EngineError>>, gap: i64) -> Vec<String> {
    let rig = rig();
    rig.fc.advance(20);
    let roads = Rc::new(Roads::default());
    *roads.exec.borrow_mut() = exec;
    *roads.answers.borrow_mut() = Some(Ok(true));
    let mut rec = record(view("ws_z", "m1", "running"), &roads);
    answering(&mut rec, &roads);
    rec.unreached = Some("the provider cannot reach the machine".into());
    rig.put(rec);
    let warned = wsp_core::console::capture();
    let t = rig.tracker(StatusWatchOptions { probe_timeout_ms: Some(10), zombie_probe_timeout_ms: Some(30), ..Default::default() });
    let o = StatusListOptions { reconcile: Some(Reconcile::OnFailure), ..Default::default() };
    t.list(o).await.unwrap();
    settle().await;
    rig.fc.advance(gap);
    t.list(o).await.unwrap();
    warned.lines()
}

fn line(since_s: &str, tail: &str) -> Vec<String> {
    vec![format!(
        "zombie on m1 (workspace ws_z): m1 reports running at the provider; reach unreachable since 2026-10-25T10:00:00.020Z ({since_s} s); exec probe \"echo ok\" {tail}"
    )]
}

fn gated_exec(roads: &Rc<Roads>, gate: Rc<RefCell<Option<tokio::sync::oneshot::Receiver<ExecResult>>>>) -> wsp_core::status::RecordExec {
    let calls = Rc::new(std::cell::Cell::new(0u32));
    let r = roads.clone();
    Rc::new(move |_cmd, _timeout| {
        r.exec_asks.set(r.exec_asks.get() + 1);
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            return Box::pin(async { Ok(serde_json::from_value(json!({"exitCode": 1, "stdout": "", "stderr": "first"})).unwrap()) });
        }
        let rx = gate.borrow_mut().take();
        Box::pin(async move {
            match rx {
                Some(rx) => rx.await.map_err(|_| EngineError::Other { message: "gate dropped".into(), code: None }),
                None => std::future::pending().await,
            }
        })
    })
}

// F1 BLOCKER: TS packages/runtime/src/tunnel.ts:40 (the socket's close, whoever started it, sends tunnel.close and ends the connection)
#[test]
fn a_client_that_half_closes_gets_the_forwards_close_back() {
    run_local(async {
        let reach = FakeReach::new(&[]);
        let fwd = open_place_forward(reach.clone(), 4000).await.unwrap();
        let mut client = tokio::net::TcpStream::connect(("127.0.0.1", fwd.port)).await.unwrap();
        until("tunnel.open", 2_000, || reach.count("tunnel.open") == 1).await;
        client.shutdown().await.unwrap();
        until("tunnel.close", 2_000, || reach.count("tunnel.close") == 1).await;
        let mut b = [0u8; 8];
        let r = tokio::time::timeout(Duration::from_secs(2), client.read(&mut b)).await;
        assert!(matches!(r, Ok(Ok(0))), "the forward kept its side of the connection open after the client closed: {r:?}");
    });
}

// F2 MAJOR: TS packages/runtime/src/status.ts:808 (the tick is a WorkspaceCostEvent with its type; the document and history() carry it)
#[test]
fn a_stored_point_and_a_history_point_carry_their_type() {
    run_local(async {
        let rig = rig();
        rig.fc.advance(20);
        let roads = Rc::new(Roads::default());
        let mut rec = record(view("ws_z", "m1", "running"), &roads);
        rec.rate_usd_per_hour = 0.1;
        let made = rec.view.clone();
        rig.put(rec);
        let t = rig.tracker(StatusWatchOptions::default());
        rig.bus.fire(&created(&made));
        until("the document is written", 2_000, || rig.store.puts.get() >= 1).await;
        let doc: Value = serde_json::from_str(&rig.store.text("cost-histories", "ws_z").unwrap()).unwrap();
        assert_eq!(doc["points"][0]["type"], "workspace.cost", "{}", doc["points"][0]);
        let h = t.history("ws_z").await;
        assert_eq!(serde_json::to_value(&h[0]).unwrap()["type"], "workspace.cost");
    });
}

// F3 MAJOR: TS packages/runtime/src/daemon-channel.ts:108 (ws reads the door's body through Node's http parser, so a chunked body arrives decoded and ends where it ends)
#[test]
fn a_chunked_refusal_body_is_read_decoded() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            read_head(&mut sock).await;
            let head = "HTTP/1.1 502 Bad Gateway\r\ncontent-type: text/plain\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n";
            sock.write_all(head.as_bytes()).await.unwrap();
            sock.write_all(b"12\r\nupstream is asleep\r\n0\r\n\r\n").await.unwrap();
            let _ = sock.shutdown().await;
        });
        let events = Rc::new(RefCell::new(Vec::new()));
        let err = match open_daemon_channel(chan_options(format!("http://127.0.0.1:{port}/"), &events)).await {
            Ok(_) => panic!("a 502 door opened"),
            Err(e) => e,
        };
        match err {
            ChannelError::Door(d) => assert_eq!(d.message, "the connection was refused with 502: upstream is asleep"),
            other => panic!("not a door error: {other:?}"),
        }
    });
}

// F3 MAJOR (second case): TS packages/runtime/src/daemon-channel.ts:108-121
#[test]
fn a_chunked_refusal_from_a_door_that_keeps_the_connection_open_still_answers_as_a_door() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            read_head(&mut sock).await;
            sock.write_all(
                b"HTTP/1.1 502 Bad Gateway\r\ncontent-type: text/plain\r\ntransfer-encoding: chunked\r\nconnection: keep-alive\r\n\r\n",
            )
            .await
            .unwrap();
            sock.write_all(b"12\r\nupstream is asleep\r\n0\r\n\r\n").await.unwrap();
            std::future::pending::<()>().await;
        });
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut o = chan_options(format!("http://127.0.0.1:{port}/"), &events);
        o.connect_timeout_ms = Some(2_000);
        let err = match open_daemon_channel(o).await {
            Ok(_) => panic!("a 502 door opened"),
            Err(e) => e,
        };
        match err {
            ChannelError::Door(d) => assert_eq!(d.message, "the connection was refused with 502: upstream is asleep"),
            other => panic!("not a door error: {other:?}"),
        }
    });
}

// F4 MAJOR: TS packages/runtime/src/daemon-channel.ts:102 (ws.close(1000) destroys the socket after 30 s with no close reply)
#[test]
fn close_against_a_silent_peer_still_settles_closed() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
            while let Some(Ok(m)) = ws.next().await {
                if let Message::Text(t) = m {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    ws.send(Message::text(json!({"id": v["id"], "ok": true}).to_string())).await.unwrap();
                    break;
                }
            }
            let _hold = ws;
            std::future::pending::<()>().await;
        });
        let events = Rc::new(RefCell::new(Vec::new()));
        let ch = open_daemon_channel(chan_options(format!("ws://127.0.0.1:{port}/"), &events)).await.expect("opens");
        ch.close();
        let end = tokio::time::timeout(Duration::from_secs(34), ch.closed()).await;
        assert!(end.is_ok(), "closed never settled 34 s after close() against a peer that sends no close frame");
    });
}

// F4 MAJOR (the held link): TS packages/runtime/src/reach.ts:260 (ws.close(1000) destroys the socket after 30 s with no close reply)
#[test]
fn reach_close_against_a_silent_peer_still_drops_the_connection() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let dropped = Rc::new(std::cell::Cell::new(false));
        let flag = dropped.clone();
        tokio::task::spawn_local(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(sock).await.unwrap();
            let mut answered = 0;
            while answered < 4 {
                if let Some(Ok(Message::Text(t))) = ws.next().await {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    ws.send(Message::text(json!({"id": v["id"], "ok": true}).to_string())).await.unwrap();
                    answered += 1;
                }
            }
            let mut raw = ws.into_inner();
            let mut buf = [0u8; 256];
            loop {
                match raw.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            flag.set(true);
        });
        let reach = connect_daemon(reach_opts(format!("http://127.0.0.1:{port}/"))).unwrap();
        reach.ready().await.unwrap();
        reach.close();
        until("the held link's socket is dropped", 34_000, || dropped.get()).await;
    });
}

// F5 MAJOR: TS packages/runtime/src/daemon-channel.ts:137 (ws accepts a frame up to 100 MiB, as the daemon's door does)
#[test]
fn a_20_mib_reply_is_delivered() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async_with_config(
                sock,
                Some(tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default().max_message_size(None).max_frame_size(None)),
            )
            .await
            .unwrap();
            while let Some(Ok(m)) = ws.next().await {
                if let Message::Text(t) = m {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    if v["op"] == "auth" {
                        ws.send(Message::text(json!({"id": v["id"], "ok": true}).to_string())).await.unwrap();
                    } else {
                        let big = "x".repeat(20 * 1024 * 1024);
                        ws.send(Message::text(json!({"id": v["id"], "ok": true, "data": big}).to_string())).await.unwrap();
                    }
                }
            }
        });
        let events = Rc::new(RefCell::new(Vec::new()));
        let ch = open_daemon_channel(chan_options(format!("ws://127.0.0.1:{port}/"), &events)).await.expect("opens");
        let mut f = serde_json::Map::new();
        f.insert("op".into(), json!("files.read"));
        let reply = ch.send(f).await.expect("a 20 MiB reply arrives");
        assert_eq!(reply["data"].as_str().map(str::len), Some(20 * 1024 * 1024));
    });
}

// F6 MAJOR: TS packages/runtime/src/tunnel.ts:47-50 (every chunk is sent as it is read; the reply is not waited on)
#[test]
fn a_second_chunk_does_not_wait_for_the_first_chunks_reply() {
    run_local(async {
        let reach = FakeReach::new(&[("tunnel.write", 400)]);
        let fwd = open_place_forward(reach.clone(), 4000).await.unwrap();
        let mut client = tokio::net::TcpStream::connect(("127.0.0.1", fwd.port)).await.unwrap();
        until("tunnel.open", 2_000, || reach.count("tunnel.open") == 1).await;
        client.write_all(b"a").await.unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        client.write_all(b"b").await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(reach.count("tunnel.write"), 2, "the second chunk waited behind the first chunk's 400 ms reply");
    });
}

// F7 MAJOR: TS packages/runtime/src/status.ts:618-621 (the reason and the verdict use the suspect the probe was started for, which another read may have dropped)
#[test]
fn a_probe_that_ends_after_another_read_dropped_the_suspect_quotes_the_spell_it_began() {
    run_local(async {
        let rig = rig();
        rig.fc.advance(20);
        let roads = Rc::new(Roads::default());
        *roads.answers.borrow_mut() = Some(Ok(true));
        let mut rec = record(view("ws_z", "m1", "running"), &roads);
        answering(&mut rec, &roads);
        let (tx, rx) = tokio::sync::oneshot::channel();
        rec.exec = gated_exec(&roads, Rc::new(RefCell::new(Some(rx))));
        let line_of_provider = "the provider cannot reach the machine";
        rec.unreached = Some(line_of_provider.into());
        rig.put(rec);
        let warned = wsp_core::console::capture();
        let t = rig.tracker(StatusWatchOptions { zombie_probe_timeout_ms: Some(5_000), ..Default::default() });
        let o = StatusListOptions { reconcile: Some(Reconcile::OnFailure), ..Default::default() };
        t.list(o).await.unwrap();
        settle().await;
        rig.fc.advance(190_000);
        let t2 = t.clone();
        let a = tokio::task::spawn_local(async move { t2.list(o).await.unwrap() });
        settle().await;
        assert_eq!(roads.exec_asks.get(), 2, "the second probe is in flight");
        rig.records.borrow_mut()[0].unreached = None;
        let b = t.list(o).await.unwrap();
        assert_eq!(b[0].reach.state, ReachState::Reachable);
        tx.send(serde_json::from_value(json!({"exitCode": 1, "stdout": "", "stderr": "dead"})).unwrap()).unwrap();
        a.await.unwrap();
        let lines = warned.lines();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("reach unreachable since 2026-10-25T10:00:00.020Z (190 s)"), "{}", lines[0]);
        rig.records.borrow_mut()[0].unreached = Some(line_of_provider.into());
        let c = t.list(o).await.unwrap();
        assert_ne!(c[0].reach.state, ReachState::Zombie, "a verdict the TS wrote on a dropped suspect stands in the Rust tracker");
    });
}

// F8 MAJOR: TS packages/runtime/src/status.ts:92-94 (the body is read before the elapsed time is measured)
#[test]
fn a_body_that_arrives_late_makes_the_answer_slow() {
    run_local(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::task::spawn_local(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            read_head(&mut sock).await;
            sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\n").await.unwrap();
            sock.flush().await.unwrap();
            tokio::time::sleep(Duration::from_millis(400)).await;
            let _ = sock.write_all(b"hello").await;
            let _ = sock.shutdown().await;
        });
        let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        let p = probe_reach(&format!("http://127.0.0.1:{port}/"), ProbeOptions { prompt_ms: 100, timeout_ms: 3_000 }, &now).await;
        assert_eq!(p.state, ReachState::Slow, "{p:?}");
    });
}

// F9 MINOR: TS packages/runtime/src/status.ts:659 (the row is `{...view, rateUsdPerHour: NaN}` and goes out as null)
#[test]
fn a_non_finite_rate_does_not_panic_the_row_builder() {
    run_local(async {
        let rig = rig();
        let roads = Rc::new(Roads::default());
        let mut rec = record(view("ws_1", "sbx_1", "running"), &roads);
        rec.rate_usd_per_hour = f64::NAN;
        rig.put(rec);
        let t = rig.tracker(StatusWatchOptions::default());
        let rows = t.list(StatusListOptions::default()).await.unwrap();
        assert_eq!(rows.len(), 1);
    });
}

// F9 MINOR: TS packages/runtime/src/status.ts:659 (idleAt rides the row as given)
#[test]
fn a_non_finite_idle_at_does_not_panic_the_row_builder() {
    run_local(async {
        let rig = rig();
        let roads = Rc::new(Roads::default());
        let mut rec = record(view("ws_1", "sbx_1", "running"), &roads);
        rec.idle_at = Some(f64::INFINITY);
        rig.put(rec);
        let t = rig.tracker(StatusWatchOptions::default());
        let rows = t.list(StatusListOptions::default()).await.unwrap();
        assert_eq!(rows.len(), 1);
    });
}

// F9 MINOR: TS packages/runtime/src/status.ts:807-831 (a tick with a NaN rate is emitted and the loop goes on)
#[test]
fn a_record_with_a_non_finite_rate_does_not_stop_the_cost_tick_for_the_next_record() {
    run_local(async {
        let rig = rig();
        let roads = Rc::new(Roads::default());
        let mut a = record(view("ws_a", "m1", "running"), &roads);
        a.rate_usd_per_hour = f64::NAN;
        let mut b = record(view("ws_b", "m2", "running"), &roads);
        b.rate_usd_per_hour = 0.1;
        rig.put(a);
        rig.put(b);
        let t = rig.tracker(StatusWatchOptions::default());
        let stop = t.watch(StatusWatchOptions { cost_interval_ms: Some(1_000), poll_interval_ms: Some(3_600_000), ..Default::default() });
        settle().await;
        rig.fc.advance(1_000);
        settle().await;
        stop();
        let ids: Vec<String> = rig.bus.costs().iter().map(|c| c["workspaceId"].as_str().unwrap().to_string()).collect();
        assert!(ids.contains(&"ws_b".to_string()), "the cost tick died on ws_a and metered nobody after it: {ids:?}");
    });
}

// F10 MINOR: TS packages/runtime/src/tunnel.ts:68 (Buffer.from(data, "base64") reads unpadded base64)
#[test]
fn unpadded_base64_in_a_tunnel_data_frame_is_written() {
    run_local(async {
        let reach = FakeReach::new(&[]);
        let fwd = open_place_forward(reach.clone(), 4000).await.unwrap();
        let mut client = tokio::net::TcpStream::connect(("127.0.0.1", fwd.port)).await.unwrap();
        until("tunnel.open", 2_000, || reach.count("tunnel.open") == 1).await;
        let e = parse_daemon_event(&json!({"type": "tunnel.data", "tunnelId": "pl1", "data": "aGk"})).unwrap();
        fwd.event(&e);
        let mut b = [0u8; 8];
        let n = tokio::time::timeout(Duration::from_secs(1), client.read(&mut b)).await;
        assert!(matches!(n, Ok(Ok(2))) && &b[..2] == b"hi", "got {n:?}");
    });
}

// F11 MINOR: TS packages/runtime/src/types/events.ts:72 (a removed listener leaves the Set)
#[test]
fn a_listener_that_unlistens_leaves_no_slot_behind() {
    let bus = event_bus("s".into());
    for _ in 0..1000 {
        let off = bus.on("*", Rc::new(|_e| Ok(())));
        off();
    }
    let slots = bus.state.borrow().listeners.get("*").map_or(0, |l| l.len());
    assert_eq!(slots, 0, "1000 listeners came and went, and the bus still walks {slots} slots on every emit");
}

// F12 NIT: TS packages/runtime/src/status.ts:556 (slice(0, 200) cuts a surrogate pair in half and the line carries the lone half)
#[test]
fn zombie_stderr_cut_inside_a_surrogate_pair() {
    run_local(async {
        let stderr = format!("{}\u{1F600}{}", "a".repeat(199), "b".repeat(50));
        let got = zombie_lines(Some(exec_of(2, "", &stderr)), 181_000).await;
        assert_eq!(got, line("181", &format!("failed after 0 ms (exit 2: {}\u{FFFD})", "a".repeat(199))));
    });
}
