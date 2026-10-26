// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the daemon link, its channels and tunnels, and the machines' daemon area, against
//! `packages/runtime/src/places/link.ts`, `machines/reach.ts` and `machines/daemon.ts` at 65f6b89c7. Each test names
//! the TS line it holds the port to and fails on `2c4f6cb46` for the reason in its message.
mod common;
mod machines_b_support;

use std::rc::Rc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use common::places::{
    answers_ok, block, obj, record, report, settle, FakeSocket, Reply, Scene, SceneOpts, Script, T0,
};
use machines_b_support::reach_ctx::*;
use machines_b_support::{run as run_reach, FakeMachine};
use wsp_core::reach::{DaemonReach, Frame, ReachError, ReachStats};
use wsp_engine::errors::is_missing;
use wsp_engine::link_backend::LinkAsk;
use wsp_engine::machine::MachineOffers;
use wsp_machines::places::helpers::{LinkSocket, Live};
use wsp_machines::places::link::LinkDoor;
use wsp_machines::places::types::ForwardAsk;
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::machine_link::MachineErrorKind;
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::words::computer::WAKE_STOPPED;

fn current(name: &str) -> PlaceReport {
    report(
        name,
        json!({ "daemonVersion": 200, "login": { "HOME": "/root", "PATH": "/usr/bin" } }),
    )
}

fn ask() -> ForwardAsk {
    ForwardAsk {
        pane: None,
        standing: false,
    }
}

async fn real_until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..400 {
        if ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("never came true: {what}");
}

/// The link door over a scene with the computer `p_spoo` kept, and a socket attached through the door as the
/// computer's own dial attaches it.
async fn attached(scene: &Scene) -> (Rc<LinkDoor>, Rc<FakeSocket>) {
    let link = scene.link();
    scene
        .records
        .keep(record("p_spoo", "spoo", current("spoo")));
    let socket = FakeSocket::new(answers_ok());
    link.attach(
        "p_spoo",
        Rc::clone(&socket) as Rc<dyn LinkSocket>,
        current("spoo"),
        "10.0.0.9",
        1_000,
        None,
    )
    .await
    .expect("attached");
    settle().await;
    (link, socket)
}

// ---------------------------------------------------------------------------------------------------------------
// Finding 1: link.ts:213 `emit({ type: "place.present", placeId, from })` is never sent.

#[test]
fn attach_tells_watchers_the_place_is_present() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let (_link, _socket) = attached(&scene).await;
        let heard = format!("{:?}", scene.events.borrow());
        assert!(
            heard.contains("Present"),
            "link.ts:213 emits place.present {{ placeId: p_spoo, from: 10.0.0.9 }} on every attach; the events an attach \
             sent here: {heard}"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Finding 2: link.ts:477 sends tunnel.write without waiting for its answer; run_tunnel waits for each one before it
// reads the next chunk or writes what the far side pushed.

#[test]
fn an_unanswered_tunnel_write_holds_back_what_the_far_side_sends() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let door = scene.link();
        let script: Script = Rc::new(|op, _| {
            if op == "tunnel.write" {
                Reply::Hang
            } else {
                Reply::Ok(obj(json!({ "ok": true })))
            }
        });
        let socket = scene.linked(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
            script,
        );
        let port = door
            .forward("p1", 34567, ask())
            .await
            .unwrap_or_else(|e| panic!("{}", e.message));
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"a").await.unwrap();
        real_until("the first chunk went up as a tunnel.write", || {
            socket.reach.asked("tunnel.write") == 1
        })
        .await;
        let conns = Rc::clone(&scene.ctx.forwards.borrow().get("p1:34567").unwrap().conns);
        let conn = conns
            .borrow()
            .get("p1")
            .cloned()
            .expect("tunnel p1 is riding");
        conn.write(b"b");
        let mut buf = [0u8; 8];
        let got = tokio::time::timeout(Duration::from_secs(1), client.read(&mut buf)).await;
        assert!(
            got.is_ok(),
            "the daemon pushed tunnel.data while its answer to the first tunnel.write was still out; the TS writes it \
             to the connection at once (link.ts:477 never awaits the write), this pump sat 1 s behind the unanswered write"
        );
    });
}

#[test]
fn a_big_upload_does_not_wait_one_round_trip_per_chunk() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let door = scene.link();
        let script: Script = Rc::new(|op, _| {
            if op == "tunnel.write" {
                Reply::Hang
            } else {
                Reply::Ok(obj(json!({ "ok": true })))
            }
        });
        let socket = scene.linked(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
            script,
        );
        let port = door
            .forward("p1", 34567, ask())
            .await
            .unwrap_or_else(|e| panic!("{}", e.message));
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(&vec![7u8; 200_000]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let sent = socket.reach.asked("tunnel.write");
        assert!(
            sent > 1,
            "200,000 bytes arrived in one burst and the TS sends one tunnel.write per chunk read without waiting for \
             any answer (link.ts:477), so every chunk is out at once; this pump had sent {sent} write(s) and was waiting \
             on the answer to the first, a round trip per 16 KiB"
        );
    });
}

#[test]
fn a_client_that_hangs_up_while_a_write_is_unanswered_is_closed_and_its_tunnel_ended() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let door = scene.link();
        let script: Script = Rc::new(|op, _| {
            if op == "tunnel.write" {
                Reply::Hang
            } else {
                Reply::Ok(obj(json!({ "ok": true })))
            }
        });
        let socket = scene.linked(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
            script,
        );
        let port = door
            .forward("p1", 34567, ask())
            .await
            .unwrap_or_else(|e| panic!("{}", e.message));
        let conns = {
            let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            client.write_all(b"a").await.unwrap();
            real_until("the write went up", || {
                socket.reach.asked("tunnel.write") == 1
            })
            .await;
            Rc::clone(&scene.ctx.forwards.borrow().get("p1:34567").unwrap().conns)
        };
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            socket.reach.asked("tunnel.close") == 1 && conns.borrow().is_empty(),
            "link.ts:470-474 hears the client's close at once, takes the tunnel off the forward and sends tunnel.close \
             whatever is still unanswered; the pump here is parked on the unanswered tunnel.write and never reads the \
             client's EOF, so the socket sits in CLOSE_WAIT with {} tunnel.close sent and {} tunnel(s) still on the forward",
            socket.reach.asked("tunnel.close"),
            conns.borrow().len()
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Finding 3: link.ts:481-483 destroys a connection whose tunnel.open was refused before anything else; run_tunnel holds
// it until the tunnel.close that follows has been answered.

#[test]
fn a_refused_tunnel_is_dropped_before_its_close_is_answered() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let door = scene.link();
        let script: Script = Rc::new(|op, _| match op {
            "tunnel.open" => Reply::Err("nothing answers on that port".into()),
            "tunnel.close" => Reply::Hang,
            _ => Reply::Ok(obj(json!({ "ok": true }))),
        });
        let socket = scene.linked(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
            script,
        );
        let port = door
            .forward("p1", 34567, ask())
            .await
            .unwrap_or_else(|e| panic!("{}", e.message));
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        real_until("the tunnel.close went up", || {
            socket.reach.asked("tunnel.close") == 1
        })
        .await;
        let mut buf = [0u8; 8];
        let got = tokio::time::timeout(Duration::from_secs(1), client.read(&mut buf)).await;
        assert!(
            got.is_ok(),
            "the far side refused the tunnel, so the TS destroyed the connection in the same turn (link.ts:481-483); this \
             connection stayed open with nothing behind it while the tunnel.close went unanswered"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Finding 4: link.ts:243-260 ends a channel at its link's close whether or not the caller ever closed it; here the
// socket's close reaches the channel through a Weak, so a channel nobody closed is never cleaned up.

#[test]
fn a_channel_nobody_closed_is_let_go_when_its_link_closes() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let (link, socket) = attached(&scene).await;
        let channel = link
            .channel("p_spoo", Rc::new(|_e: &Map<String, Value>| {}))
            .expect("a channel on the live link");
        channel
            .send(obj(json!({ "op": "sys.watch" })))
            .await
            .expect("the watch is held");
        assert_eq!(
            scene.setup.channels.borrow().get("p_spoo").map(|c| c.len()),
            Some(1)
        );
        drop(channel);
        socket.drop_link(1006, "gone");
        settle().await;
        let left = scene.setup.channels.borrow().get("p_spoo").map(|c| c.len());
        assert_eq!(
            left, None,
            "the TS socket's close listener (`gone`, link.ts:243-260) holds the channel's own `forget`, so the reader comes \
             off `channels` and the readings hold is let go at the link's close even when nobody called close(); here \
             {left:?} reader(s) stay on `channels` for good"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Finding 5: runtime/src/reach.ts:162-170 rejects a refused frame with the daemon's kind, status and code on the error; the
// channel's send keeps only the words.

struct RefusingReach;

#[async_trait(?Send)]
impl DaemonReach for RefusingReach {
    async fn ready(&self) -> Result<(), ReachError> {
        Ok(())
    }
    async fn request(&self, _op: &str, _params: Frame) -> Result<Frame, ReachError> {
        Err(ReachError {
            message: "no such file".to_string(),
            kind: Some(MachineErrorKind::Missing),
            status: Some(404.0),
            code: Some("ENOENT".to_string()),
        })
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {}
}

#[test]
fn a_channel_send_keeps_what_the_daemon_refused_with() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        let link = scene.link();
        scene
            .records
            .keep(record("p_spoo", "spoo", current("spoo")));
        let socket = FakeSocket::new(answers_ok());
        scene.ctx.live.borrow_mut().insert(
            "p_spoo".to_string(),
            Live {
                socket: Rc::clone(&socket) as Rc<dyn LinkSocket>,
                reach: Rc::new(RefusingReach),
                seen: Box::new(|| {}),
                forward: None,
            },
        );
        let channel = link
            .channel("p_spoo", Rc::new(|_e: &Map<String, Value>| {}))
            .expect("a channel");
        let err = channel
            .send(obj(json!({ "op": "fs.read", "path": "/nope" })))
            .await
            .expect_err("the daemon refused");
        assert_eq!(err.message, "no such file");
        assert!(
            err.kind.is_some() && err.code.as_deref() == Some("ENOENT") && err.status == Some(404.0),
            "runtime/src/reach.ts:162-170 stamps the refusal's kind, status and code on the error the channel's send rejects with (what \
             tells a missing file from a dead link); this send answered kind {:?}, code {:?}, status {:?}",
            err.kind,
            err.code,
            err.status
        );
    });
}

#[test]
fn a_machine_frame_the_daemon_refused_keeps_its_kind_and_status_for_the_engine() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene.records.keep(record(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
        ));
        let socket = FakeSocket::new(answers_ok());
        scene.ctx.live.borrow_mut().insert(
            "p1".to_string(),
            Live {
                socket: Rc::clone(&socket) as Rc<dyn LinkSocket>,
                reach: Rc::new(RefusingReach),
                seen: Box::new(|| {}),
                forward: None,
            },
        );
        let err = scene
            .setup
            .link_to("p1")
            .request("machine.get", Map::new(), LinkAsk::default())
            .await
            .expect_err("the daemon refused");
        assert!(
            is_missing(&err) && err.status() == Some(404),
            "link-backend.ts:62 and runtime/src/reach.ts:162-170: a refusal rejects with the daemon's kind and status, which is how a \
             machine the far side lost reads `missing` (golden.ts:103, orphans.ts:167) and a refused keyed create ends \
             (machines.ts:87); setup.rs maps it to EngineError::Other, so this refusal reads is_missing {}, status {:?}",
            is_missing(&err),
            err.status()
        );
    });
}

struct StatusOnlyReach;

#[async_trait(?Send)]
impl DaemonReach for StatusOnlyReach {
    async fn ready(&self) -> Result<(), ReachError> {
        Ok(())
    }
    async fn request(&self, _op: &str, _params: Frame) -> Result<Frame, ReachError> {
        Err(ReachError {
            message: "cannot be done".to_string(),
            kind: None,
            status: Some(422.0),
            code: None,
        })
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {}
}

// reach.ts:162-170 stamps the status on the error whatever the kind; the engine reads it off a refusal with none too.
#[test]
fn a_machine_frame_refused_with_a_status_and_no_kind_keeps_its_status_for_the_engine() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene.records.keep(record(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
        ));
        let socket = FakeSocket::new(answers_ok());
        scene.ctx.live.borrow_mut().insert(
            "p1".to_string(),
            Live {
                socket: Rc::clone(&socket) as Rc<dyn LinkSocket>,
                reach: Rc::new(StatusOnlyReach),
                seen: Box::new(|| {}),
                forward: None,
            },
        );
        let err = scene
            .setup
            .link_to("p1")
            .request("machine.get", Map::new(), LinkAsk::default())
            .await
            .expect_err("the daemon refused");
        assert_eq!(
            (err.status(), err.kind(), err.to_string().as_str()),
            (Some(422), None, "cannot be done")
        );
    });
}

struct KindOnlyReach;

#[async_trait(?Send)]
impl DaemonReach for KindOnlyReach {
    async fn ready(&self) -> Result<(), ReachError> {
        Ok(())
    }
    async fn request(&self, _op: &str, _params: Frame) -> Result<Frame, ReachError> {
        Err(ReachError {
            message: "no such machine".to_string(),
            kind: Some(MachineErrorKind::Missing),
            status: None,
            code: None,
        })
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {}
}

// The reverse: a kind with no status is no status 0, which a keyed create reads as a refused attempt (machines.ts:87).
#[test]
fn a_machine_frame_refused_with_a_kind_and_no_status_keeps_its_kind_and_reads_no_status() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene.records.keep(record(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
        ));
        let socket = FakeSocket::new(answers_ok());
        scene.ctx.live.borrow_mut().insert(
            "p1".to_string(),
            Live {
                socket: Rc::clone(&socket) as Rc<dyn LinkSocket>,
                reach: Rc::new(KindOnlyReach),
                seen: Box::new(|| {}),
                forward: None,
            },
        );
        let err = scene
            .setup
            .link_to("p1")
            .request("machine.get", Map::new(), LinkAsk::default())
            .await
            .expect_err("the daemon refused");
        assert!(is_missing(&err));
        assert_eq!(err.status(), None, "no status is no status 0");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Finding 6: setup.ts:1079-1080 waits a keyed frame out from the moment its socket failed under it; the Rust waits
// from the last close this host stamped, which on a socket that is closing is the gap before.

#[test]
fn a_keyed_frame_failing_on_a_closing_socket_waits_from_now_for_the_next_link() {
    block(async {
        let scene = Scene::new(SceneOpts::default());
        scene.records.keep(record(
            "p1",
            "old-macbook",
            report("old-macbook", json!({})),
        ));
        let failing: Script = Rc::new(|_, _| Reply::Err("connection lost".into()));
        let old = scene.live("p1", failing);
        scene
            .setup
            .closed_at
            .borrow_mut()
            .insert("p1".to_string(), T0 as u64);
        scene.clock.pass(600_000).await;
        old.open.set(false);
        let link = scene.setup.link_to("p1");
        let asked = tokio::task::spawn_local(async move {
            link.request(
                "machine.create",
                Map::new(),
                LinkAsk {
                    timeout_ms: None,
                    idempotency_key: Some("k1".to_string()),
                },
            )
            .await
        });
        settle().await;
        let _fresh = scene.live("p1", answers_ok());
        scene.setup.woken("p1", true);
        settle().await;
        let outcome = tokio::time::timeout(Duration::from_millis(500), asked).await;
        assert!(
            matches!(&outcome, Ok(Ok(Ok(_)))),
            "setup.ts:1080 waits `relinkWaitMs` from Date.now() when a keyed frame fails on a socket that is closing, and \
             sends it again on the socket that computer opens next; the wait here ran from the close stamped ten minutes \
             ago, was already over, and the frame failed with {outcome:?}"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Finding 7 (the admitted one): machines/reach.ts:255-257 dials even when the stop is already pulled and lets go of the link
// when it lands; the Rust never dials. The control below is the case the report names, a stop after the dial.

#[test]
fn control_a_stop_after_the_dial_lets_the_link_go() {
    run_reach(async {
        let host = reach_host();
        let machine = FakeMachine::new("bx_1");
        *machine.offers.lock().unwrap() = MachineOffers {
            preview_url: true,
            put_bytes: true,
            read_bytes: true,
            ..MachineOffers::default()
        };
        let entry = host.put_live(
            plain_record("ws_1", "web", "napping", "bx_1"),
            machine.clone(),
        );
        let area = reach(&host);
        let stop = wsp_engine::Cancel::new();
        entry.borrow_mut().wake_stop = Some(stop.clone());
        let task = {
            let (a, e) = (area.clone(), entry.clone());
            tokio::task::spawn_local(async move { a.ping_daemon(&e).await })
        };
        machines_b_support::settle().await;
        assert_eq!(plan().dials.borrow().len(), 1, "the dial finished");
        stop.cancel(None);
        machines_b_support::settle().await;
        assert!(task.is_finished());
        assert_eq!(
            task.await.unwrap(),
            Some(format!(
                "daemon on bx_1 did not answer within 900000 ms ({WAKE_STOPPED})"
            ))
        );
        assert_eq!(plan().links.borrow()[0].closed.get(), 1);
    });
}

#[test]
fn a_stop_already_pulled_still_asks_the_machines_own_road_once() {
    run_reach(async {
        let host = reach_host();
        let machine = FakeMachine::new("bx_1");
        *machine.offers.lock().unwrap() = MachineOffers {
            daemon_answers: true,
            put_bytes: true,
            read_bytes: true,
            ..MachineOffers::default()
        };
        machine.daemon_answers.lock().unwrap().push_back(Ok(true));
        let entry = host.put_live(
            plain_record("ws_1", "web", "napping", "bx_1"),
            machine.clone(),
        );
        let area = reach(&host);
        let stop = wsp_engine::Cancel::new();
        stop.cancel(None);
        entry.borrow_mut().wake_stop = Some(stop);
        let said = area.ping_daemon(&entry).await;
        assert_eq!(
            said,
            Some(format!(
                "the daemon on bx_1 could not be asked ({WAKE_STOPPED})"
            ))
        );
        let left = machine.daemon_answers.lock().unwrap().len();
        assert_eq!(
            left, 0,
            "machines/reach.ts:240 evaluates machine.daemonAnswers(...) as the argument of orStopped, so the ask is made once even \
             when the stop is already pulled and its answer is thrown away; the Rust never makes it ({left} answer(s) \
             still queued)"
        );
    });
}

#[test]
fn a_dial_the_stop_walked_away_from_still_lands_and_is_let_go() {
    run_reach(async {
        let host = reach_host();
        let machine = FakeMachine::new("bx_1");
        *machine.offers.lock().unwrap() = MachineOffers {
            preview_url: true,
            put_bytes: true,
            read_bytes: true,
            ..MachineOffers::default()
        };
        let entry = host.put_live(
            plain_record("ws_1", "web", "napping", "bx_1"),
            machine.clone(),
        );
        let area = reach(&host);
        let stop = wsp_engine::Cancel::new();
        stop.cancel(None);
        entry.borrow_mut().wake_stop = Some(stop);
        let said = {
            let (a, e) = (area.clone(), entry.clone());
            tokio::task::spawn_local(async move { a.ping_daemon(&e).await })
        }
        .await
        .unwrap();
        assert_eq!(
            said,
            Some(format!(
                "daemon on bx_1 did not answer within 900000 ms ({WAKE_STOPPED})"
            ))
        );
        machines_b_support::settle().await;
        let dialled = plan().dials.borrow().len();
        let closed = plan()
            .links
            .borrow()
            .iter()
            .map(|l| l.closed.get())
            .sum::<u32>();
        assert_eq!(
            (dialled, closed),
            (1, 1),
            "machines/reach.ts:255-257 starts the dial before it races the stop and closes the link the moment it lands once the \
             stop is pulled; here {dialled} dial(s) were made and {closed} link(s) closed (no link is left open: the \
             dial is dropped before it connects)"
        );
    });
}
