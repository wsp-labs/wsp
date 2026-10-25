// SPDX-License-Identifier: AGPL-3.0-only
//! serve_workspaces: events, status, the workspace verbs, daemon channels, readings and exec. Spec:
//! spec/serve_workspaces.md. Each case is the TS case ported at this crate's seam: the fake runtime is scripted with
//! what the area would have answered, and the test reads what the socket sent and what the runtime was asked.
mod support;
use support::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use futures::{FutureExt, StreamExt};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use support::fake_places::dbg;
use wsp_adapters::port::{ExecFailure, ExecFailureKind, ExecStream, SharedFuture, WriteOutcome};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, Frame};
use wsp_core::devices::DeviceDoor;
use wsp_core::status::{Reconcile, StatusListOptions, StatusReader};
use wsp_core::types::api::*;
use wsp_core::types::events::Since;
use wsp_core::types::wiring::{CreatedWorkspace, RunningExec, SizeAsk};
use wsp_serve::serve::door::Served;
use wsp_serve::serve::served::ForwardsSource;
use wsp_serve::serve::socket::Socket;
use wsp_serve::serve::workspaces::NO_SUCH_CHANNEL;
use wsp_serve::serve::{PlaceDoctor, ReleaseDoor, ServeOptions, Unlisten};
use wsp_wire::types::release::{ReleaseChangedEvent, ReleaseView};
use wsp_wire::views::place::DoctorLineEvent;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{PortProbeView, PortReachView};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_more::{WorkspaceStatus, WorkspaceView};
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::place_link::{DAEMON_OPEN_ONE_OF, PLACES_TICKET_REFUSAL};
use wsp_words::exit::{HostError, Kind};
use wsp_words::workspace_state::absent_computer;

// ---- helpers -------------------------------------------------------------------------------------------------

fn de<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).expect("the test's value parses as its type")
}

fn err(message: &str) -> HostError {
    HostError::new(message)
}

fn refused(id: u64, error: &str, kind: Option<&str>) -> String {
    let mut v = json!({ "id": id, "ok": false, "error": error });
    if let Some(k) = kind {
        v["kind"] = json!(k);
    }
    v.to_string()
}

fn base(rt: &Rc<FakeRuntime>) -> ServeOptions {
    ServeOptions { devices: Some(rt.devices.clone() as Rc<dyn DeviceDoor>), now: Some(Rc::new(|| 1_000_000)), ..opts() }
}

async fn ticketed(served: &Rc<Served>, purpose: &str) -> (Rc<Socket>, Rc<RecordingSink>) {
    let (host, hsink) = host_socket(served).await;
    let issued = request(&host, &hsink, 1, "ticket.issue", json!({ "purpose": purpose })).await;
    let ticket = issued["ticket"].as_str().expect("a ticket").to_string();
    open(served, &format!("?ticket={ticket}"))
}

async fn settle() {
    for _ in 0..40 {
        tokio::task::yield_now().await;
    }
}

fn ws_json(id: &str, screen: bool) -> Value {
    let mut v = json!({
        "id": id, "name": "x", "machineId": "m1", "phase": "running", "golden": "snap_g", "createdAt": "2026-01-01T00:00:00.000Z",
        "project": { "id": "pr_1", "name": "lab", "path": "/home/z/lab", "computer": "here" }
    });
    if screen {
        v["screen"] = json!({ "streamUrl": "wss://m1.preview.example/stream?token=minted" });
    }
    v
}

fn ws(id: &str) -> WorkspaceView {
    de(ws_json(id, true))
}

fn status_json(id: &str, url: Option<&str>) -> Value {
    let mut v = ws_json(id, true);
    v["machineState"] = json!("running");
    v["reach"] = match url {
        Some(u) => json!({ "state": "reachable", "url": u, "expiresAt": 1_800_000_000_000u64 }),
        None => json!({ "state": "reachable" }),
    };
    v["size"] = json!({ "cpu": 2, "memMb": 4096 });
    v["rateUsdPerHour"] = json!(0.5);
    v
}

fn ev(workspace: &str, seq: u64) -> EventUnion {
    FakeBus::event(ev_json(workspace, seq))
}

fn ev_json(workspace: &str, seq: u64) -> Value {
    json!({ "type": "workspace.napped", "workspaceId": workspace, "seq": seq })
}

fn cost(workspace: &str, accrued: f64) -> EventUnion {
    FakeBus::event(json!({
        "type": "workspace.cost", "workspaceId": workspace, "phase": "running", "rateUsdPerHour": 0.5, "awakeMs": 1,
        "accruedUsd": accrued, "at": "1970-01-01T00:00:00.000Z"
    }))
}

fn since(head: u64, events: Vec<EventUnion>, gap: bool) -> Since {
    Since { stream: "stream-a".into(), head, events, gap }
}

fn map(v: Value) -> Map<String, Value> {
    v.as_object().expect("an object").clone()
}

fn is_hex12(s: &str) -> bool {
    s.len() == 12 && s.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn frames_of(sink: &RecordingSink, id: u64) -> Vec<Value> {
    sink.frames().into_iter().skip_while(|f| f.get("id") != Some(&json!(id))).collect()
}

/// A forwards source the host holds: its listeners are the test's to call.
struct FakeForwards {
    listeners: Rc<RefCell<Vec<(u64, Rc<dyn Fn(&Value)>)>>>,
    next: Cell<u64>,
}

impl FakeForwards {
    fn new() -> Rc<FakeForwards> {
        Rc::new(FakeForwards { listeners: Rc::new(RefCell::new(Vec::new())), next: Cell::new(0) })
    }
    fn emit(&self, e: &Value) {
        let heard: Vec<Rc<dyn Fn(&Value)>> = self.listeners.borrow().iter().map(|(_, f)| f.clone()).collect();
        for f in heard {
            f(e);
        }
    }
}

impl ForwardsSource for FakeForwards {
    fn list(&self) -> Vec<PortForward> {
        Vec::new()
    }
    fn stop(&self, _workspace_id: &str, _port: u16) -> bool {
        false
    }
    fn on(&self, f: Rc<dyn Fn(&Value)>) -> Box<dyn FnOnce()> {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.listeners.borrow_mut().push((id, f));
        let listeners = self.listeners.clone();
        Box::new(move || listeners.borrow_mut().retain(|(i, _)| *i != id))
    }
}

struct FakeDoctor {
    script: Rc<Script>,
    lines: RefCell<Vec<(String, String)>>,
    listeners: Rc<RefCell<Vec<(u64, Rc<dyn Fn(&DoctorLineEvent)>)>>>,
    next: Cell<u64>,
}

#[async_trait(?Send)]
impl PlaceDoctor for FakeDoctor {
    async fn run(&self, place_id: &str, doctor_id: &str, _project: Option<&str>) -> Result<i32, HostError> {
        let call = self.script.record("doctor.run", json!({ "placeId": place_id, "doctorId": doctor_id }));
        let lines = self.lines.borrow().clone();
        for (line, stream) in lines {
            let e: DoctorLineEvent = de(json!({ "doctorId": doctor_id, "line": line, "stream": stream }));
            let heard: Vec<Rc<dyn Fn(&DoctorLineEvent)>> = self.listeners.borrow().iter().map(|(_, f)| f.clone()).collect();
            for f in heard {
                f(&e);
            }
        }
        self.script.answer::<Result<i32, HostError>>(&call).unwrap_or(Ok(1))
    }
    fn on(&self, f: Rc<dyn Fn(&DoctorLineEvent)>) -> Unlisten {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.listeners.borrow_mut().push((id, f));
        let listeners = self.listeners.clone();
        Box::new(move || listeners.borrow_mut().retain(|(i, _)| *i != id))
    }
}

struct FakeRelease {
    listeners: Rc<RefCell<Vec<Rc<dyn Fn(&ReleaseChangedEvent)>>>>,
}

#[async_trait(?Send)]
impl ReleaseDoor for FakeRelease {
    fn get(&self) -> ReleaseView {
        panic!("a subscribe never reads the release")
    }
    async fn check(&self, _force: bool) -> Result<ReleaseView, HostError> {
        panic!("a subscribe never checks the release")
    }
    fn on(&self, f: Rc<dyn Fn(&ReleaseChangedEvent)>) -> Unlisten {
        self.listeners.borrow_mut().push(f);
        let listeners = self.listeners.clone();
        Box::new(move || listeners.borrow_mut().clear())
    }
}

/// A daemon channel the test drives: its `send` answers what is scripted under `channel.send` (an empty reply
/// unscripted), `close` is recorded under `channel.close`, and `end` is the daemon socket going.
struct FakeChannel {
    script: Rc<Script>,
    name: String,
    ended: RefCell<Option<ChannelEnd>>,
    wake: tokio::sync::Notify,
}

impl FakeChannel {
    fn new(script: &Rc<Script>, name: &str) -> Rc<FakeChannel> {
        Rc::new(FakeChannel { script: script.clone(), name: name.to_string(), ended: RefCell::new(None), wake: tokio::sync::Notify::new() })
    }
    fn end(&self, code: u16, reason: &str) {
        self.ended.borrow_mut().get_or_insert(ChannelEnd { code, reason: reason.to_string() });
        self.wake.notify_waiters();
    }
}

#[async_trait(?Send)]
impl DaemonChannel for FakeChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let call = self.script.record("channel.send", json!({ "channel": self.name, "frame": Value::Object(frame) }));
        self.script.answer::<Result<Frame, ChannelError>>(&call).unwrap_or_else(|| Ok(Frame::new()))
    }
    fn close(&self) {
        self.script.record("channel.close", json!({ "channel": self.name }));
        self.end(1000, "");
    }
    async fn closed(&self) -> ChannelEnd {
        loop {
            let woken = self.wake.notified();
            if let Some(end) = self.ended.borrow().clone() {
                return end;
            }
            woken.await;
        }
    }
}

/// Scripts the next `workspaces.daemonChannel` to answer `ch` and opens it from `socket`.
async fn open_channel(rt: &Rc<FakeRuntime>, socket: &Rc<Socket>, sink: &RecordingSink, id: u64, ch: &Rc<FakeChannel>) -> String {
    rt.script.once::<Result<Rc<dyn DaemonChannel>, HostError>>("workspaces.daemonChannel", Ok(ch.clone() as Rc<dyn DaemonChannel>));
    let opened = request(socket, sink, id, "daemon.open", json!({ "workspaceId": "ws_1" })).await;
    assert_eq!(opened["ok"], json!(true), "{opened}");
    opened["channel"].as_str().expect("a channel id").to_string()
}

// ---- S1: events ----------------------------------------------------------------------------------------------

// S1. serve.test.ts:345
#[test]
fn events_reach_only_subscribed_sockets() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (sub, sub_sink) = host_socket(&served).await;
        let (_quiet, quiet_sink) = host_socket(&served).await;
        let reply = request(&sub, &sub_sink, 1, "events.subscribe", json!({})).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "seq": 0, "stream": "stream-a" }).to_string());
        let reads: Vec<String> = rt.script.names().into_iter().filter(|n| n.starts_with("events.")).collect();
        assert_eq!(reads, vec!["events.since", "events.on"], "the replay is read and the listener taken in one step");
        assert_eq!(rt.script.calls_to("events.since")[0].args, json!({ "after": null, "stream": null }));
        assert_eq!(rt.script.calls_to("events.on")[0].args, json!({ "type": "*" }));

        rt.bus.emit(&ev("ws_1", 1));
        rt.bus.emit(&ev("ws_2", 2));
        assert_eq!(sub_sink.pushes(), vec![ev_json("ws_1", 1), ev_json("ws_2", 2)]);
        assert!(quiet_sink.pushes().is_empty(), "a socket that did not subscribe hears nothing");

        sub.closed();
        assert_eq!(rt.bus.listening(), 0, "the subscription dies with the socket");
        rt.bus.emit(&ev("ws_3", 3));
        assert_eq!(sub_sink.pushes().len(), 2);
    })
}

// S1. serve-replay.test.ts:83-186
#[test]
fn subscribe_answers_head_and_stream_then_replays_after_the_cursor() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", since(14, vec![ev("ws_1", 13), ev("ws_1", 14)], false));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 12, "stream": "stream-a" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "seq": 14, "stream": "stream-a" }).to_string(), "no gap key unless there is a gap");
        assert_eq!(rt.script.calls_to("events.since")[0].args, json!({ "after": 12, "stream": "stream-a" }));
        let after_reply: Vec<Value> = frames_of(&sink, 2);
        assert_eq!(after_reply[1..].to_vec(), vec![ev_json("ws_1", 13), ev_json("ws_1", 14)], "the reply goes before the replayed events");
        rt.bus.emit(&ev("ws_1", 15));
        assert_eq!(sink.pushes(), vec![ev_json("ws_1", 13), ev_json("ws_1", 14), ev_json("ws_1", 15)], "once, in order, then live continues");
    })
}

// S1. serve-replay.test.ts:136 (a cursor older than the ring)
#[test]
fn a_cursor_older_than_the_ring_is_a_gap_with_no_replay() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", since(5011, vec![], true));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 1 })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "seq": 5011, "stream": "stream-a", "gap": true }).to_string());
        assert!(sink.pushes().is_empty(), "a gap replays nothing");
        rt.bus.emit(&ev("ws_1", 5012));
        assert_eq!(sink.pushes(), vec![ev_json("ws_1", 5012)]);
    })
}

// S1. serve-replay.test.ts:136 (a cursor from a runtime that restarted is past head)
#[test]
fn a_cursor_past_head_is_a_gap() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", since(11, vec![], true));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 90 })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "seq": 11, "stream": "stream-a", "gap": true }).to_string());
        rt.bus.emit(&ev("ws_1", 12));
        assert_eq!(sink.pushes(), vec![ev_json("ws_1", 12)]);
    })
}

// S1. serve-replay.test.ts:136 (the old stream's cursor, the new runtime's stream)
#[test]
fn a_cursor_from_another_stream_is_a_gap() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", Since { stream: "stream-b".into(), head: 19, events: vec![], gap: true });
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 12, "stream": "stream-a" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "seq": 19, "stream": "stream-b", "gap": true }).to_string(), "the bus's own stream is the one named");
        assert_eq!(rt.script.calls_to("events.since")[0].args, json!({ "after": 12, "stream": "stream-a" }), "the client's stream is what the bus compares");
        assert!(sink.pushes().is_empty());
    })
}

// S1. serve.test.ts:1326 and the runtime's one rule: seen_by decides, shown_to words the frame
#[test]
fn an_event_the_caller_may_not_see_is_never_sent() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.on("workspaces.seenBy", |call| -> bool { call.args["event"]["workspaceId"] != json!("ws_hidden") });
        rt.script.on("workspaces.shownTo", |call| -> Value {
            let mut shown = call.args["event"].clone();
            shown["shownTo"] = call.args["origin"].clone();
            shown
        });
        rt.script.once::<Since>("events.since", since(2, vec![ev("ws_hidden", 1), ev("ws_ok", 2)], false));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        request(&socket, &sink, 2, "events.subscribe", json!({})).await;
        rt.bus.emit(&ev("ws_hidden", 3));
        rt.bus.emit(&ev("ws_ok", 4));
        let mut first = ev_json("ws_ok", 2);
        first["shownTo"] = Value::Null;
        let mut second = ev_json("ws_ok", 4);
        second["shownTo"] = Value::Null;
        assert_eq!(sink.pushes(), vec![first, second], "a hidden event is never sent, replayed or live; the rest go out as shown_to words them");

        // the frame's own word is the origin an unstamped socket is read by
        let (relayed, relayed_sink) = host_socket(&served).await;
        request(&relayed, &relayed_sink, 3, "events.subscribe", json!({ "origin": "relayed" })).await;
        rt.bus.emit(&ev("ws_ok", 5));
        let last = relayed_sink.pushes().pop().expect("an event");
        assert_eq!(last["shownTo"], json!("relayed"));
        let seen = rt.script.calls_to("workspaces.seenBy");
        assert_eq!(seen.last().unwrap().args["origin"], json!("relayed"));
    })
}

// S1. serve.ts:1170-1188: one cost_moved per subscribe
#[test]
fn a_cost_tick_that_moved_nothing_is_dropped() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (first, first_sink) = host_socket(&served).await;
        request(&first, &first_sink, 1, "events.subscribe", json!({})).await;
        rt.bus.emit(&cost("ws_a", 0.5));
        let (second, second_sink) = host_socket(&served).await;
        request(&second, &second_sink, 1, "events.subscribe", json!({})).await;
        rt.bus.emit(&cost("ws_a", 0.5));
        assert_eq!(first_sink.pushes().len(), 1, "the same figures are dropped on the socket that already heard them");
        assert_eq!(second_sink.pushes().len(), 1, "each socket has its own memory of what it was sent");
        rt.bus.emit(&cost("ws_a", 0.75));
        assert_eq!((first_sink.pushes().len(), second_sink.pushes().len()), (2, 2), "a figure that moved goes out");
    })
}

// S1. serve.test.ts:1257, 1274
#[test]
fn doctor_lines_reach_subscribers_before_the_reply_and_are_never_replayed() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let rows = vec![de::<PlaceView>(json!({ "id": "p_1", "kind": "computer", "name": "spoo", "default": false }))];
        rt.script.on("places.list", move |_| -> Result<Vec<PlaceView>, HostError> { Ok(rows.clone()) });
        let doctor = Rc::new(FakeDoctor {
            script: rt.script.clone(),
            lines: RefCell::new(vec![("spoo answers".into(), "out".into()), ("DOCTOR FAIL: spoo".into(), "err".into())]),
            listeners: Rc::new(RefCell::new(Vec::new())),
            next: Cell::new(0),
        });
        let served = served_with(&rt, ServeOptions { doctor: Some(doctor.clone()), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        request(&socket, &sink, 1, "events.subscribe", json!({})).await;
        let reply = request(&socket, &sink, 2, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_1" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "code": 1 }).to_string());
        let frames = sink.frames();
        let at = frames.iter().position(|f| f.get("id") == Some(&json!(2))).unwrap();
        assert_eq!(
            frames[at - 2..at].to_vec(),
            vec![
                json!({ "type": "doctor.line", "doctorId": "d_1", "line": "spoo answers", "stream": "out" }),
                json!({ "type": "doctor.line", "doctorId": "d_1", "line": "DOCTOR FAIL: spoo", "stream": "err" }),
            ],
            "every line landed before the reply, in the order the road said them, with no sequence"
        );

        // a socket that subscribes after the road spoke is replayed none of them
        let (late, late_sink) = host_socket(&served).await;
        let sub = request(&late, &late_sink, 3, "events.subscribe", json!({})).await;
        assert_eq!(sub["ok"], json!(true));
        let listed = request(&late, &late_sink, 4, "places.list", json!({})).await;
        assert_eq!(listed["ok"], json!(true), "a round trip after the subscription");
        assert!(late_sink.pushes().is_empty());

        socket.closed();
        assert!(doctor.listeners.borrow().is_empty(), "the doctor listener is let go with the socket");
    })
}

// S1. serve.ts:1170-1188: the release door's changes ride the same pass
#[test]
fn release_changes_reach_subscribers() {
    support::block(async {
        let rt = FakeRuntime::new();
        let release = Rc::new(FakeRelease { listeners: Rc::new(RefCell::new(Vec::new())) });
        let served = served_with(&rt, ServeOptions { release: Some(release.clone()), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let (_quiet, quiet_sink) = host_socket(&served).await;
        request(&socket, &sink, 1, "events.subscribe", json!({})).await;
        let changed: ReleaseChangedEvent = de(json!({ "release": { "state": "checking" } }));
        let heard: Vec<Rc<dyn Fn(&ReleaseChangedEvent)>> = release.listeners.borrow().clone();
        assert_eq!(heard.len(), 1);
        for f in heard {
            f(&changed);
        }
        assert_eq!(sink.pushes(), vec![json!({ "type": "release.changed", "release": { "state": "checking" } })]);
        assert!(quiet_sink.pushes().is_empty());
        socket.closed();
        assert!(release.listeners.borrow().is_empty());
    })
}

// S1. serve.test.ts:1335
#[test]
fn creates_follow_the_replay_once_and_a_plain_subscribe_hears_none() {
    support::block(async {
        let rt = FakeRuntime::new();
        let refused_create: WorkspaceCreatingEvent = de(json!({ "workspaceId": "ws_r", "name": "refused", "stage": "failed", "message": "Snapshot not found", "elapsedMs": 3 }));
        let held_create: WorkspaceCreatingEvent = de(json!({ "workspaceId": "ws_h", "name": "held", "stage": "fork-requested", "message": "forking", "elapsedMs": 0 }));
        let creating = vec![refused_create, held_create];
        rt.script.on("workspaces.creating", move |_| creating.clone());
        rt.script.once::<Since>("events.since", since(7, vec![ev("ws_1", 7)], false));
        let served = served_with(&rt, base(&rt));
        let (window, window_sink) = host_socket(&served).await;
        let reply = request(&window, &window_sink, 2, "events.subscribe", json!({ "creates": true })).await;
        assert_eq!(reply["ok"], json!(true));
        let heard = frames_of(&window_sink, 2);
        assert_eq!(heard.len(), 4, "the reply, the replay, then the creates: {heard:?}");
        assert_eq!(heard[1], ev_json("ws_1", 7));
        assert_eq!(heard[2]["type"], json!("workspace.creating"));
        assert_eq!((heard[2]["name"].clone(), heard[2]["stage"].clone(), heard[2]["message"].clone()), (json!("refused"), json!("failed"), json!("Snapshot not found")));
        assert_eq!((heard[3]["name"].clone(), heard[3]["stage"].clone()), (json!("held"), json!("fork-requested")));
        for e in &heard[2..] {
            assert!(e.get("seq").is_none(), "a snapshot frame is no position in the stream");
        }

        let (plain, plain_sink) = host_socket(&served).await;
        request(&plain, &plain_sink, 3, "events.subscribe", json!({})).await;
        assert!(plain_sink.pushes().is_empty(), "a plain subscribe hears neither");
        assert_eq!(rt.script.calls_to("workspaces.creating").len(), 1, "only creates: true reads the creates in flight");
    })
}

// S1. serve.test.ts:911
#[test]
fn forward_events_reach_subscribers_and_die_with_the_socket() {
    support::block(async {
        let rt = FakeRuntime::new();
        let source = FakeForwards::new();
        *rt.forwards.borrow_mut() = Some(source.clone() as Rc<dyn ForwardsSource>);
        let served = served_with(&rt, base(&rt));
        let (sub, sub_sink) = host_socket(&served).await;
        let (_quiet, quiet_sink) = host_socket(&served).await;
        request(&sub, &sub_sink, 1, "events.subscribe", json!({})).await;
        let f = json!({ "workspaceId": "ws_1", "port": 8123, "startedAt": "2026-09-04T10:00:00.000Z", "name": "api", "kind": "url" });
        let open = json!({ "type": "forward.open", "forward": f });
        let close = json!({ "type": "forward.close", "workspaceId": "ws_1", "port": 8123 });
        source.emit(&open);
        source.emit(&close);
        assert_eq!(sub_sink.pushes(), vec![open, close]);
        assert!(quiet_sink.pushes().is_empty());
        assert_eq!(source.listeners.borrow().len(), 1);
        sub.closed();
        assert_eq!(source.listeners.borrow().len(), 0, "the subscription dies with the socket");
    })
}

// ---- S2, S3: status ------------------------------------------------------------------------------------------

// S2. status.test.ts
#[test]
fn status_subscribe_watches_before_the_snapshot() {
    support::block(async {
        let rt = FakeRuntime::new();
        let detached = Rc::new(Cell::new(0));
        let counter = detached.clone();
        rt.script.once::<Rc<dyn Fn()>>("status.watch", Rc::new(move || counter.set(counter.get() + 1)));
        let status: WorkspaceStatus = de(status_json("ws_1", Some("https://m1-3000.preview.example/?pt_token=e")));
        rt.script.once::<Result<Vec<WorkspaceStatus>, HostError>>("status.list", Ok(vec![status]));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "status.subscribe", json!({})).await;
        assert_eq!(reply["ok"], json!(true));
        assert_eq!(reply["statuses"], json!([status_json("ws_1", Some("https://m1-3000.preview.example/?pt_token=e"))]), "the app reads the stream off the status its own socket subscribes to");
        let order: Vec<String> = rt.script.names().into_iter().filter(|n| n.starts_with("status.")).collect();
        assert_eq!(order, vec!["status.watch", "status.list"], "watch before the snapshot");
        assert_eq!(rt.script.calls_to("status.list")[0].args, json!({ "opts": null, "origin": null }));
        assert_eq!(detached.get(), 0);
        socket.closed();
        assert_eq!(detached.get(), 1, "the watcher is released with the socket");
    })
}

// S3. status.test.ts
#[test]
fn status_list_reads_the_table_and_drops_the_route() {
    support::block(async {
        let rt = FakeRuntime::new();
        let status: WorkspaceStatus = de(status_json("ws_1", Some("https://m1-3000.preview.example/?pt_token=e")));
        rt.script.once::<Result<Vec<WorkspaceStatus>, HostError>>("status.list", Ok(vec![status]));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "status.list", json!({})).await;
        assert_eq!(reply["ok"], json!(true));
        let rows = reply["statuses"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["reach"], json!({ "state": "reachable" }), "the minted route is dropped");
        assert!(!reply.to_string().contains("pt_token"), "{reply}");
        assert!(rows[0].get("screen").is_none());
        let want = StatusListOptions { probe_timeout_ms: None, reader: Some(StatusReader::Table), prompt_ms: None, reconcile: Some(Reconcile::OnFailure), zombie_probe: Some(false) };
        assert_eq!(rt.script.calls_to("status.list")[0].args, json!({ "opts": dbg(&want), "origin": null }));
    })
}

// ---- S4, S5: the workspace verbs -----------------------------------------------------------------------------

// S4, S5. runtime.test.ts:288
#[test]
fn create_strips_origin_and_splits_notice_and_sub() {
    support::block(async {
        let rt = FakeRuntime::new();
        let created: CreatedWorkspace = de({
            let mut v = ws_json("ws_1", true);
            v["notice"] = json!("the clone is cold");
            v
        });
        rt.script.once::<Result<CreatedWorkspace, HostError>>("workspaces.create", Ok(created));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "workspaces.create", json!({ "project": "pr_1", "name": "x", "golden": "snap_g", "origin": "relayed" })).await;
        let keys: Vec<&String> = reply.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["id", "ok", "workspace", "notice"], "{reply}");
        assert_eq!(reply["workspace"], ws_json("ws_1", false), "the display stream is not in the reply, nor the notice in the workspace");
        assert_eq!(reply["notice"], json!("the clone is cold"));
        let asked = &rt.script.calls_to("workspaces.create")[0].args;
        assert_eq!(asked["origin"], json!("relayed"), "the frame's word is the origin of an unstamped socket");
        let opts = asked["opts"].as_str().unwrap();
        assert!(opts.contains("name: \"x\"") && opts.contains("snap_g"), "{opts}");
        assert!(!opts.contains("relayed"), "the client's origin never reaches the create as an option: {opts}");
        assert!(!opts.contains("Relayed"), "{opts}");
    })
}

// S4. runtime.test.ts:288
#[test]
fn every_workspace_answer_goes_out_without_the_display_stream() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let size = SizeAsk { cpu: de(json!(4)), mem_mb: de(json!(8192)) };
        // (op, frame, the call it makes, the args it passes)
        let cases: Vec<(&str, Value, &str, Value)> = vec![
            ("workspaces.resolve", json!({ "ref": "x" }), "workspaces.resolve", json!({ "reference": "x", "origin": null })),
            ("workspaces.get", json!({ "workspaceId": "ws_1", "threadId": "t_1" }), "workspaces.get", json!({ "id": "ws_1", "origin": null, "threadId": "t_1" })),
            ("workspaces.nap", json!({ "workspaceId": "ws_1" }), "workspaces.nap", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.wake", json!({ "workspaceId": "ws_1" }), "workspaces.wake", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.resize", json!({ "workspaceId": "ws_1", "size": { "cpu": 4, "memMb": 8192 } }), "workspaces.resize", json!({ "id": "ws_1", "size": dbg(&size), "origin": null })),
            ("workspaces.stopWake", json!({ "workspaceId": "ws_1" }), "workspaces.stopWake", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.upgrade", json!({ "workspaceId": "ws_1" }), "workspaces.upgrade", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.rename", json!({ "workspaceId": "ws_1", "name": "y" }), "workspaces.rename", json!({ "id": "ws_1", "name": "y", "origin": null })),
            ("workspaces.look", json!({ "workspaceId": "ws_1", "theme": null }), "workspaces.look", json!({ "id": "ws_1", "look": { "theme": null }, "origin": null })),
            ("folder.make", json!({ "project": "pr_1" }), "workspaces.folder", json!({ "project": "pr_1", "origin": null })),
            ("workspaces.rebuild", json!({ "workspaceId": "ws_1" }), "workspaces.rebuild", json!({ "id": "ws_1", "origin": null })),
        ];
        for (i, (op, fields, call, args)) in cases.into_iter().enumerate() {
            let id = 10 + i as u64;
            rt.script.once::<Result<WorkspaceView, HostError>>(call, Ok(ws("ws_1")));
            let reply = request(&socket, &sink, id, op, fields).await;
            assert_eq!(reply.to_string(), json!({ "id": id, "ok": true, "workspace": ws_json("ws_1", false) }).to_string(), "{op}");
            assert_eq!(rt.script.calls_to(call)[0].args, args, "{op}");
        }
        rt.script.once::<Result<Vec<WorkspaceView>, HostError>>("workspaces.list", Ok(vec![ws("ws_1"), ws("ws_2")]));
        let listed = request(&socket, &sink, 40, "workspaces.list", json!({})).await;
        assert_eq!(listed.to_string(), json!({ "id": 40, "ok": true, "workspaces": [ws_json("ws_1", false), ws_json("ws_2", false)] }).to_string());
        assert!(!sink.texts.borrow().iter().any(|t| t.contains("streamUrl") || t.contains("minted")), "no reply carries the minted stream");
    })
}

// ---- S6-S9 ---------------------------------------------------------------------------------------------------

// S6. serve.ts:1214-1394 (the table: every field only when sent)
#[test]
fn spread_verbs_pass_only_the_fields_sent() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let ask = |call: &str, ask: String| (call.to_string(), json!({ "ask": ask, "origin": null }));
        // (op, frame, the runtime call it makes, the args it passes). The areas answer nothing here: the args are what
        // is read, and the refusal `unscripted call` is the area's.
        let mut cases: Vec<(&str, Value, String, Value)> = Vec::new();
        let mut add = |op: &'static str, fields: Value, expect: (String, Value)| cases.push((op, fields, expect.0, expect.1));
        add("workspaces.landing", json!({ "project": "p", "sized": true }), ("workspaces.landing".into(), json!({ "project": "p", "sized": true, "origin": null })));
        add("workspaces.landing", json!({ "project": "p" }), ("workspaces.landing".into(), json!({ "project": "p", "sized": false, "origin": null })));
        add("workspaces.bringBack", json!({ "workspaceId": "ws_1", "title": "t" }), ("workspaces.bringBack".into(), json!({ "workspaceId": "ws_1", "title": "t", "body": null, "origin": null })));
        add("worktree.make", json!({ "project": "p", "branch": "feat/x" }), ("workspaces.worktree".into(), json!({ "project": "p", "branch": "feat/x", "origin": null })));
        add("workspaces.checkout", json!({ "workspaceId": "ws_1", "fresh": true }), ("workspaces.checkout".into(), json!({ "id": "ws_1", "origin": null, "fresh": true })));
        add(
            "workspaces.discard",
            json!({ "workspaceId": "ws_1", "path": "a.txt", "check": true, "threadId": "t_1" }),
            ask("workspaces.discard", dbg_s(&DiscardAsk { workspace_id: "ws_1".into(), path: "a.txt".into(), check: Some(true), thread_id: Some("t_1".into()) })),
        );
        add(
            "workspaces.discard",
            json!({ "workspaceId": "ws_1", "path": "a.txt", "check": false }),
            ask("workspaces.discard", dbg_s(&DiscardAsk { workspace_id: "ws_1".into(), path: "a.txt".into(), check: None, thread_id: None })),
        );
        add(
            "workspaces.commit",
            json!({ "workspaceId": "ws_1", "message": "m", "paths": ["a", "b"], "threadId": "t_1" }),
            ask("workspaces.commit", dbg_s(&CommitAsk { workspace_id: "ws_1".into(), message: "m".into(), paths: Some(vec!["a".into(), "b".into()]), thread_id: Some("t_1".into()) })),
        );
        add(
            "workspaces.commit",
            json!({ "workspaceId": "ws_1", "message": "m" }),
            ask("workspaces.commit", dbg_s(&CommitAsk { workspace_id: "ws_1".into(), message: "m".into(), paths: None, thread_id: None })),
        );
        add(
            "workspaces.commitDraft",
            json!({ "workspaceId": "ws_1", "paths": ["a"] }),
            ask("workspaces.commitDraft", dbg_s(&CommitDraftAsk { workspace_id: "ws_1".into(), paths: Some(vec!["a".into()]), thread_id: None })),
        );
        add(
            "workspaces.viewed",
            json!({ "workspaceId": "ws_1", "path": "a", "blob": "b1" }),
            ("workspaces.viewed".into(), json!({ "workspaceId": "ws_1", "mark": dbg_s(&ViewedMark { path: "a".into(), blob: Some("b1".into()) }), "origin": null })),
        );
        add(
            "workspaces.viewed",
            json!({ "workspaceId": "ws_1", "path": "a", "blob": null }),
            ("workspaces.viewed".into(), json!({ "workspaceId": "ws_1", "mark": dbg_s(&ViewedMark { path: "a".into(), blob: None }), "origin": null })),
        );
        add("workspaces.viewed", json!({ "workspaceId": "ws_1" }), ("workspaces.viewed".into(), json!({ "workspaceId": "ws_1", "mark": null, "origin": null })));
        add("workspaces.pullRequestView", json!({ "workspaceId": "ws_1", "fresh": true }), ("workspaces.pullRequestView".into(), json!({ "workspaceId": "ws_1", "fresh": true, "origin": null })));
        add("workspaces.pullRequestView", json!({ "workspaceId": "ws_1", "fresh": false }), ("workspaces.pullRequestView".into(), json!({ "workspaceId": "ws_1", "fresh": null, "origin": null })));
        add("workspaces.pullRequestDiff", json!({ "workspaceId": "ws_1" }), ("workspaces.pullRequestDiff".into(), json!({ "workspaceId": "ws_1", "origin": null })));
        add(
            "workspaces.pullRequestReply",
            json!({ "workspaceId": "ws_1", "body": "ok", "replyTo": 3, "threadId": "t_1" }),
            ask("workspaces.pullRequestReply", dbg_s(&PullRequestReplyAsk { workspace_id: "ws_1".into(), reply_to: Some(3), thread_id: Some("t_1".into()), body: "ok".into() })),
        );
        add(
            "workspaces.pullRequestReply",
            json!({ "workspaceId": "ws_1", "body": "ok" }),
            ask("workspaces.pullRequestReply", dbg_s(&PullRequestReplyAsk { workspace_id: "ws_1".into(), reply_to: None, thread_id: None, body: "ok".into() })),
        );
        add(
            "workspaces.pullRequestResolve",
            json!({ "workspaceId": "ws_1", "threadId": "c1", "resolved": true }),
            ("workspaces.pullRequestResolve".into(), json!({ "workspaceId": "ws_1", "threadId": "c1", "resolved": true, "origin": null })),
        );
        add(
            "workspaces.pullRequestReact",
            json!({ "workspaceId": "ws_1", "subject": "s1", "content": "+1", "on": true }),
            ("workspaces.pullRequestReact".into(), json!({ "workspaceId": "ws_1", "subject": "s1", "content": "+1", "on": true, "origin": null })),
        );
        add(
            "workspaces.pullRequestSend",
            json!({ "workspaceId": "ws_1", "items": [{ "kind": "comment", "id": 5 }], "threadId": "t_1" }),
            ("workspaces.pullRequestSend".into(), json!({ "workspaceId": "ws_1", "items": [{ "kind": "comment", "id": 5 }], "threadId": "t_1", "origin": null })),
        );
        add(
            "workspaces.fix",
            json!({ "workspaceId": "ws_1", "check": "ci", "child": "fix-1", "threadId": "t_1", "childThreadId": "t_2" }),
            ask("workspaces.fix", dbg_s(&FixAsk { workspace_id: "ws_1".into(), check: Some("ci".into()), child: Some("fix-1".into()), thread_id: Some("t_1".into()), child_thread_id: Some("t_2".into()) })),
        );
        add("workspaces.fix", json!({ "workspaceId": "ws_1" }), ask("workspaces.fix", dbg_s(&FixAsk { workspace_id: "ws_1".into(), ..FixAsk::default() })));
        add(
            "workspaces.merge",
            json!({ "workspaceId": "ws_1", "method": "squash", "whenChecksPass": true, "head": "abc", "threadId": "t_1" }),
            ask(
                "workspaces.merge",
                dbg_s(&MergeAsk { workspace_id: "ws_1".into(), method: Some(wsp_frames::MergeMethod::Squash), when_checks_pass: Some(true), head: Some("abc".into()), thread_id: Some("t_1".into()) }),
            ),
        );
        add("workspaces.merge", json!({ "workspaceId": "ws_1" }), ask("workspaces.merge", dbg_s(&MergeAsk { workspace_id: "ws_1".into(), ..MergeAsk::default() })));
        add("workspaces.update", json!({ "workspaceId": "ws_1", "threadId": "t_1" }), ("workspaces.update".into(), json!({ "workspaceId": "ws_1", "threadId": "t_1", "origin": null })));
        add("workspaces.update", json!({ "workspaceId": "ws_1" }), ("workspaces.update".into(), json!({ "workspaceId": "ws_1", "threadId": null, "origin": null })));
        add(
            "workspaces.mergeIn",
            json!({ "workspaceId": "ws_1", "child": "ws_2", "threadId": "t_1", "childThreadId": "t_2" }),
            ask("workspaces.mergeIn", dbg_s(&MergeInAsk { workspace_id: "ws_1".into(), child: "ws_2".into(), thread_id: Some("t_1".into()), child_thread_id: Some("t_2".into()) })),
        );
        add(
            "workspaces.reviewDraft",
            json!({ "workspaceId": "ws_1", "summary": "looks fine" }),
            ask("workspaces.reviewDraft", dbg_s(&ReviewDraftAsk { workspace_id: "ws_1".into(), summary: Some("looks fine".into()), verdict: None, on: None })),
        );
        add(
            "workspaces.reviewDraft",
            json!({ "workspaceId": "ws_1", "on": [{ "id": "c1", "on": true }] }),
            ask("workspaces.reviewDraft", dbg_s(&ReviewDraftAsk { workspace_id: "ws_1".into(), summary: None, verdict: None, on: Some(vec![("c1".into(), true)]) })),
        );
        add("workspaces.reviewPost", json!({ "workspaceId": "ws_1", "threadId": "t_1" }), ("workspaces.reviewPost".into(), json!({ "workspaceId": "ws_1", "threadId": "t_1", "origin": null })));

        let mut seen: Vec<(String, usize)> = Vec::new();
        for (i, (op, fields, call, args)) in cases.into_iter().enumerate() {
            let before = rt.script.calls_to(&call).len();
            let reply = request(&socket, &sink, 100 + i as u64, op, fields.clone()).await;
            let made = rt.script.calls_to(&call);
            assert_eq!(made.len(), before + 1, "{op} {fields}: made no {call}: {reply}");
            assert_eq!(made[before].args, args, "{op} {fields}");
            seen.push((call, before));
        }
        assert!(!seen.is_empty());
    })
}

fn dbg_s<T: std::fmt::Debug>(x: &T) -> String {
    format!("{x:?}")
}

// S6. serve.ts:1214-1394 (the answer is spread after ok, in the area's own order)
#[test]
fn spread_verbs_spread_the_answer_after_ok() {
    support::block(async {
        let rt = FakeRuntime::new();
        let made: wsp_wire::views::workspace_more::WorktreeMade = de(json!({ "path": "/home/z/lab-feat", "branch": "feat/x", "made": true }));
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorktreeMade, HostError>>("workspaces.worktree", Ok(made));
        rt.script.once::<Result<wsp_wire::types::changes::CheckoutReply, HostError>>("workspaces.checkout", Ok(de(json!({}))));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "worktree.make", json!({ "project": "p", "branch": "feat/x" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "path": "/home/z/lab-feat", "branch": "feat/x", "made": true }).to_string());
        let checked = request(&socket, &sink, 2, "workspaces.checkout", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(checked.to_string(), json!({ "id": 2, "ok": true }).to_string());
    })
}

// S7. serve.ts:1344-1376
#[test]
fn start_and_review_hand_the_workspace_out() {
    support::block(async {
        let rt = FakeRuntime::new();
        let started = json!({ "workspace": ws_json("ws_1", true), "threadId": "t_1", "sessionId": "s_1" });
        rt.script.once::<Result<wsp_wire::views::harness::StartResult, HostError>>("workspaces.start", Ok(de(started.clone())));
        rt.script.once::<Result<wsp_wire::views::harness::StartResult, HostError>>("workspaces.review", Ok(de(started)));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "workspaces.start", json!({ "url": "https://github.com/o/r/pull/1", "project": "pr_1", "agent": "claude", "access": "full" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "workspace": ws_json("ws_1", false), "threadId": "t_1", "sessionId": "s_1" }).to_string());
        let want = StartFromLinkAsk { url: "https://github.com/o/r/pull/1".into(), project: Some("pr_1".into()), agent: Some("claude".into()), model: None, effort: None, access: Some(wsp_wire::types::thread_defaults::AccessChoice::Full) };
        assert_eq!(rt.script.calls_to("workspaces.start")[0].args, json!({ "ask": dbg_s(&want), "origin": null }));

        let reviewed = request(&socket, &sink, 2, "workspaces.review", json!({ "workspaceId": "ws_1", "model": "opus" })).await;
        assert_eq!(reviewed["workspace"], ws_json("ws_1", false));
        let want = ReviewAsk { url: None, workspace_id: Some("ws_1".into()), agent: None, model: Some("opus".into()), effort: None };
        assert_eq!(rt.script.calls_to("workspaces.review")[0].args, json!({ "ask": dbg_s(&want), "origin": null }));
    })
}

// S8. serve.ts:1214-1394
#[test]
fn unit_verbs_answer_ok_after_the_await() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let verbs: Vec<(&str, Value, &str, Value)> = vec![
            ("workspaces.restartDaemon", json!({ "workspaceId": "ws_1" }), "workspaces.restartDaemon", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.delete", json!({ "workspaceId": "ws_1" }), "workspaces.delete", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.forget", json!({ "workspaceId": "ws_1" }), "workspaces.forget", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.touch", json!({ "workspaceId": "ws_1" }), "workspaces.touch", json!({ "id": "ws_1", "origin": null })),
        ];
        for (i, (op, fields, call, args)) in verbs.into_iter().enumerate() {
            let id = 10 + 2 * i as u64;
            rt.script.once::<Result<(), HostError>>(call, Err(err("the machine said no")));
            rt.script.once::<Result<(), HostError>>(call, Ok(()));
            let failed = request(&socket, &sink, id, op, fields.clone()).await;
            assert_eq!(failed.to_string(), refused(id, "the machine said no", None), "{op}: no ok before the await");
            let done = request(&socket, &sink, id + 1, op, fields).await;
            assert_eq!(done.to_string(), json!({ "id": id + 1, "ok": true }).to_string(), "{op}");
            assert_eq!(rt.script.calls_to(call)[0].args, args, "{op}");
        }

        rt.script.once::<Result<(), HostError>>("workspaces.worktreeRemove", Ok(()));
        rt.script.once::<Result<(), HostError>>("workspaces.worktreeRemove", Ok(()));
        let removed = request(&socket, &sink, 40, "worktree.remove", json!({ "project": "p", "branch": "feat/x", "force": false, "check": false })).await;
        assert_eq!(removed.to_string(), json!({ "id": 40, "ok": true }).to_string());
        request(&socket, &sink, 41, "worktree.remove", json!({ "project": "p", "branch": "feat/x", "check": true })).await;
        let asked = rt.script.calls_to("workspaces.worktreeRemove");
        let first = WorktreeRemoveAsk { project: "p".into(), branch: "feat/x".into(), force: Some(false), check: None };
        let second = WorktreeRemoveAsk { project: "p".into(), branch: "feat/x".into(), force: None, check: Some(true) };
        assert_eq!(asked[0].args, json!({ "ask": dbg_s(&first), "origin": null }), "check is passed only when true; force as sent");
        assert_eq!(asked[1].args, json!({ "ask": dbg_s(&second), "origin": null }));
    })
}

// S9. serve.test.ts:676, 693
#[test]
fn port_reach_and_probe_name_their_field() {
    support::block(async {
        let rt = FakeRuntime::new();
        let reach: PortReachView = de(json!({ "url": "https://m1-3000.preview.example/?pt_token=e", "expiresAt": 1_800_000_000_000u64 }));
        let probe: PortProbeView = de(json!({ "status": 403, "body": "Blocked request. This host is not allowed. To allow this host, add it to server.allowedHosts" }));
        rt.script.once::<Result<PortReachView, HostError>>("workspaces.portReach", Ok(reach));
        rt.script.once::<Result<PortReachView, HostError>>("workspaces.portReach", Err(err("no such workspace: ws_nobody")));
        rt.script.once::<Result<PortProbeView, HostError>>("workspaces.portProbe", Ok(probe));
        rt.script.once::<Result<PortProbeView, HostError>>("workspaces.portProbe", Err(err("no such workspace: ws_nobody")));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let got = request(&socket, &sink, 1, "workspaces.portReach", json!({ "workspaceId": "ws_1", "port": 3000 })).await;
        assert_eq!(got.to_string(), json!({ "id": 1, "ok": true, "reach": { "url": "https://m1-3000.preview.example/?pt_token=e", "expiresAt": 1_800_000_000_000u64 } }).to_string());
        assert_eq!(rt.script.calls_to("workspaces.portReach")[0].args, json!({ "id": "ws_1", "port": 3000, "origin": null }));
        let missing = request(&socket, &sink, 2, "workspaces.portReach", json!({ "workspaceId": "ws_nobody", "port": 3000 })).await;
        assert_eq!(missing.to_string(), refused(2, "no such workspace: ws_nobody", None));

        let probed = request(&socket, &sink, 3, "workspaces.portProbe", json!({ "workspaceId": "ws_1", "port": 5173 })).await;
        assert_eq!(probed["probe"]["status"], json!(403));
        assert!(probed["probe"]["body"].as_str().unwrap().contains("server.allowedHosts"));
        assert_eq!(rt.script.calls_to("workspaces.portProbe")[0].args, json!({ "id": "ws_1", "port": 5173, "origin": null }));
        let missing = request(&socket, &sink, 4, "workspaces.portProbe", json!({ "workspaceId": "ws_nobody", "port": 5173 })).await;
        assert_eq!(missing["ok"], json!(false));
    })
}

// ---- S10-S16: daemon channels --------------------------------------------------------------------------------

// S10. serve-daemon.test.ts:198-348
#[test]
fn daemon_open_names_one_of_place_and_workspace() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let both = request(&socket, &sink, 1, "daemon.open", json!({ "placeId": "p_1", "workspaceId": "ws_1" })).await;
        assert_eq!(both.to_string(), refused(1, DAEMON_OPEN_ONE_OF, None));
        let neither = request(&socket, &sink, 2, "daemon.open", json!({})).await;
        assert_eq!(neither.to_string(), refused(2, DAEMON_OPEN_ONE_OF, None));
        assert!(rt.daemon_listeners.borrow().is_empty());
        assert!(rt.script.names().iter().all(|n| !n.contains("hereChannel") && !n.contains("daemonChannel") && n != "places.channel"));
    })
}

// S10. serve-daemon.test.ts:~300 (a socket let in on a ticket is refused)
#[test]
fn daemon_open_here_is_the_own_road() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let ch = FakeChannel::new(&rt.script, "here-channel");
        rt.script.once::<Result<Rc<dyn DaemonChannel>, HostError>>("runtime.hereChannel", Ok(ch.clone() as Rc<dyn DaemonChannel>));
        let served = served_with(&rt, base(&rt));
        let (host, host_sink) = host_socket(&served).await;
        let opened = request(&host, &host_sink, 1, "daemon.open", json!({ "placeId": "here" })).await;
        assert_eq!(opened["ok"], json!(true));
        assert!(is_hex12(opened["channel"].as_str().unwrap()), "6 random bytes in lower hex: {opened}");
        assert_eq!(rt.script.calls_to("runtime.hereChannel").len(), 1);
        assert_eq!(rt.daemon_listeners.borrow().len(), 1, "the serve hands the runtime its on_event");

        let (ticketed_socket, ticketed_sink) = ticketed(&served, "connect").await;
        let refused_reply = request(&ticketed_socket, &ticketed_sink, 2, "daemon.open", json!({ "placeId": "here" })).await;
        assert_eq!(refused_reply.to_string(), refused(2, PLACES_TICKET_REFUSAL, Some("ticket")));
        assert_eq!(rt.script.calls_to("runtime.hereChannel").len(), 1, "a ticketed socket opens no channel");
    })
}

// S10. serve.ts:1407-1467 (another place: the place door's channel, wrapped by place_kills)
#[test]
fn daemon_open_a_joined_place_is_wrapped_by_place_kills() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let ch = FakeChannel::new(&rt.script, "p1-channel");
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(ch.clone() as Rc<dyn DaemonChannel>));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let opened = request(&socket, &sink, 1, "daemon.open", json!({ "placeId": "p_1" })).await;
        assert_eq!(opened["ok"], json!(true));
        assert_eq!(rt.script.calls_to("places.channel")[0].args, json!({ "placeId": "p_1", "onEvent": "_" }));
        assert_eq!(rt.script.calls_to("runtime.placeKills").len(), 1, "the channel is wrapped by place_kills");
        let place = rt.places.borrow().clone().unwrap();
        assert_eq!(place.channel_listeners.borrow().len(), 1);
        let on_event = place.channel_listeners.borrow()[0].clone();
        let channel = opened["channel"].as_str().unwrap().to_string();
        on_event(map(json!({ "type": "daemon.hello" })));
        assert_eq!(sink.pushes(), vec![json!({ "type": "daemon.event", "channel": channel, "event": { "type": "daemon.hello" } })]);
    })
}

// S10. serve-daemon.test.ts:312
#[test]
fn daemon_open_an_absent_computer_says_so() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<String>("places.nameOf", "spoo".to_string());
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "daemon.open", json!({ "placeId": "p_9" })).await;
        assert_eq!(reply.to_string(), refused(1, &absent_computer("spoo", None).sentence, None));
        assert_eq!(rt.script.calls_to("places.channel").len(), 1);
        assert!(rt.script.calls_to("runtime.placeKills").is_empty());
    })
}

// S10. serve-daemon.test.ts:198-290
#[test]
fn daemon_open_carries_the_workspace_refusal() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let cases = vec![
            (err("m1 is not answering yet"), None),
            (HostError::with_kind("the machine refused this host's daemon token (4401 unauthorized)", Kind::Reauth), Some("reauth")),
            (HostError::with_kind("the connection was refused with 403: cross-origin websocket denied", Kind::Refused), Some("refused")),
            (err("no such workspace: ws_nobody"), None),
        ];
        for (i, (e, kind)) in cases.into_iter().enumerate() {
            let id = 1 + i as u64;
            let said = e.message.clone();
            rt.script.once::<Result<Rc<dyn DaemonChannel>, HostError>>("workspaces.daemonChannel", Err(e));
            let reply = request(&socket, &sink, id, "daemon.open", json!({ "workspaceId": "ws_1" })).await;
            assert_eq!(reply.to_string(), refused(id, &said, kind));
        }
        assert_eq!(rt.script.calls_to("workspaces.daemonChannel")[0].args, json!({ "id": "ws_1", "onEvent": "_", "origin": null }));

        // a socket on a relay ticket asks as the machine's road, and the runtime's refusal is the reply
        rt.script.once::<Result<Rc<dyn DaemonChannel>, HostError>>("workspaces.daemonChannel", Err(err("a request relayed from a machine cannot drive ws_1")));
        let (relayed, relayed_sink) = ticketed(&served, "relay").await;
        let reply = request(&relayed, &relayed_sink, 9, "daemon.open", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(reply.to_string(), refused(9, "a request relayed from a machine cannot drive ws_1", None));
        assert_eq!(rt.script.calls_to("workspaces.daemonChannel").last().unwrap().args["origin"], json!("relayed"));
    })
}

// S11. serve-daemon.test.ts:86
#[test]
fn daemon_open_replies_before_any_event_and_queues_the_hello() {
    support::block(async {
        let rt = FakeRuntime::new();
        let ch = FakeChannel::new(&rt.script, "c1");
        let in_flight = rt.clone();
        let answered = ch.clone();
        rt.script.on("workspaces.daemonChannel", move |_| -> Result<Rc<dyn DaemonChannel>, HostError> {
            // the daemon pushes its hello while the dial is still in flight
            let listener = in_flight.daemon_listeners.borrow()[0].clone();
            listener(map(json!({ "type": "daemon.hello", "version": 54 })));
            listener(map(json!({ "type": "ports.changed", "ports": [] })));
            Ok(answered.clone() as Rc<dyn DaemonChannel>)
        });
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let opened = request(&socket, &sink, 1, "daemon.open", json!({ "workspaceId": "ws_1" })).await;
        let channel = opened["channel"].as_str().unwrap().to_string();
        assert!(is_hex12(&channel));
        let got = frames_of(&sink, 1);
        assert_eq!(got[0].to_string(), json!({ "id": 1, "ok": true, "channel": channel }).to_string());
        assert_eq!(
            got[1..].iter().map(|f| f.to_string()).collect::<Vec<_>>(),
            vec![
                json!({ "type": "daemon.event", "channel": channel, "event": { "type": "daemon.hello", "version": 54 } }).to_string(),
                json!({ "type": "daemon.event", "channel": channel, "event": { "type": "ports.changed", "ports": [] } }).to_string(),
            ],
            "a page hears a channel's id before anything arrives on it, and the queued events keep their order"
        );
        // a later event goes out at once
        let listener = rt.daemon_listeners.borrow()[0].clone();
        listener(map(json!({ "type": "pty.data", "data": "x" })));
        assert_eq!(sink.pushes().last().unwrap(), &json!({ "type": "daemon.event", "channel": channel, "event": { "type": "pty.data", "data": "x" } }));
    })
}

// S12. serve.ts:1436
#[test]
fn a_socket_gone_mid_dial_closes_the_channel() {
    support::block(async {
        let rt = FakeRuntime::new();
        let ch = FakeChannel::new(&rt.script, "c1");
        let (socket, sink) = {
            let served = served_with(&rt, base(&rt));
            let (socket, sink) = host_socket(&served).await;
            (socket, sink)
        };
        let page = sink.clone();
        let answered = ch.clone();
        rt.script.on("workspaces.daemonChannel", move |_| -> Result<Rc<dyn DaemonChannel>, HostError> {
            page.open.set(false);
            Ok(answered.clone() as Rc<dyn DaemonChannel>)
        });
        send(&socket, frame(1, "daemon.open", json!({ "workspaceId": "ws_1" }))).await;
        assert!(sink.reply(1).is_none(), "nothing is sent to a socket that left");
        assert_eq!(rt.script.calls_to("channel.close").len(), 1, "the machine keeps no socket for a tab that is gone");
        assert!(sink.pushes().is_empty());
    })
}

// S13. serve-daemon.test.ts:149
#[test]
fn a_channel_is_never_reachable_from_another_socket() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (owner, owner_sink) = host_socket(&served).await;
        let (other, other_sink) = host_socket(&served).await;
        let ch = FakeChannel::new(&rt.script, "c1");
        let channel = open_channel(&rt, &owner, &owner_sink, 1, &ch).await;

        let stolen = request(&other, &other_sink, 2, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(stolen.to_string(), refused(2, NO_SUCH_CHANNEL, None));
        let closed = request(&other, &other_sink, 3, "daemon.close", json!({ "channel": channel })).await;
        assert_eq!(closed.to_string(), refused(3, NO_SUCH_CHANNEL, None));
        assert!(rt.script.calls_to("channel.send").is_empty());
        assert!(rt.script.calls_to("channel.close").is_empty(), "another socket cannot close it");

        let mine = request(&owner, &owner_sink, 4, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(mine["ok"], json!(true));
        assert_eq!(rt.script.calls_to("channel.send")[0].args, json!({ "channel": "c1", "frame": { "op": "ping" } }));
    })
}

// S14. serve-daemon.test.ts:120
#[test]
fn daemon_send_carries_the_daemons_refusal_under_ok() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let ch = FakeChannel::new(&rt.script, "c1");
        let channel = open_channel(&rt, &socket, &sink, 1, &ch).await;
        let refusal = map(json!({ "ok": false, "code": "bad-request", "error": "path must be a string" }));
        rt.script.once::<Result<Frame, ChannelError>>("channel.send", Ok(refusal));
        let reply = request(&socket, &sink, 2, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(
            reply.to_string(),
            json!({ "id": 2, "ok": true, "reply": { "ok": false, "code": "bad-request", "error": "path must be a string" } }).to_string(),
            "the daemon's refusal rides inside an ok reply, as it wrote it"
        );
        rt.script.once::<Result<Frame, ChannelError>>("channel.send", Err(ChannelError::Unanswered { message: "daemon unreachable".into(), cause: None }));
        let broken = request(&socket, &sink, 3, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(broken.to_string(), refused(3, "daemon unreachable", None), "a channel whose daemon socket ended fails the send");
    })
}

// S15. serve-daemon.test.ts:177
#[test]
fn a_daemon_end_is_said_once_and_a_page_close_says_nothing() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let cut = FakeChannel::new(&rt.script, "cut");
        let asked = FakeChannel::new(&rt.script, "asked");
        let cut_id = open_channel(&rt, &socket, &sink, 1, &cut).await;
        let asked_id = open_channel(&rt, &socket, &sink, 2, &asked).await;
        let closed = request(&socket, &sink, 3, "daemon.close", json!({ "channel": asked_id })).await;
        assert_eq!(closed.to_string(), json!({ "id": 3, "ok": true }).to_string());
        assert_eq!(rt.script.calls_to("channel.close")[0].args, json!({ "channel": "asked" }));

        cut.end(1006, "gone");
        cut.end(1000, "ignored: it ended once already");
        settle().await;
        let said: Vec<Value> = sink.pushes().into_iter().filter(|f| f["type"] == json!("daemon.closed")).collect();
        assert_eq!(said, vec![json!({ "type": "daemon.closed", "channel": cut_id, "code": 1006, "reason": "gone" })], "once, with the daemon's own code and reason");
        assert!(said.iter().all(|f| f["channel"] != json!(asked_id)), "a close the page asked for says nothing then or later");

        let after = request(&socket, &sink, 4, "daemon.send", json!({ "channel": cut_id, "frame": { "op": "ping" } })).await;
        assert_eq!(after.to_string(), refused(4, NO_SUCH_CHANNEL, None), "the channel was forgotten");
    })
}

// S16. serve-daemon.test.ts:165
#[test]
fn a_socket_that_goes_takes_its_channels() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let first = FakeChannel::new(&rt.script, "first");
        let second = FakeChannel::new(&rt.script, "second");
        open_channel(&rt, &socket, &sink, 1, &first).await;
        open_channel(&rt, &socket, &sink, 2, &second).await;
        assert!(rt.script.calls_to("channel.close").is_empty());
        socket.closed();
        let mut closed: Vec<String> = rt.script.calls_to("channel.close").iter().map(|c| c.args["channel"].as_str().unwrap().to_string()).collect();
        closed.sort();
        assert_eq!(closed, vec!["first", "second"], "the socket's close closes every channel it holds");
        settle().await;
        assert!(sink.pushes().is_empty(), "a channel the socket's close ended says nothing");
    })
}

// ---- S17: readings -------------------------------------------------------------------------------------------

// S17. serve-sys.test.ts:177
#[test]
fn sys_subscribe_on_a_place_is_the_own_road() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, base(&rt));
        let (ticket, ticket_sink) = ticketed(&served, "relay").await;
        let gated = request(&ticket, &ticket_sink, 5, "sys.subscribe", json!({ "placeId": "p_1" })).await;
        assert_eq!(gated.to_string(), refused(5, PLACES_TICKET_REFUSAL, Some("ticket")));
        assert!(rt.script.calls_to("sys.answer").is_empty(), "the road is never reached");

        // a workspace's readings, and any unsubscribe, are not the places gate's
        let of_workspace = request(&ticket, &ticket_sink, 6, "sys.subscribe", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(of_workspace.to_string(), json!({ "id": 6, "ok": true }).to_string());
        let unsub = request(&ticket, &ticket_sink, 7, "sys.unsubscribe", json!({ "placeId": "p_1" })).await;
        assert_eq!(unsub["ok"], json!(true));
        let ops: Vec<Value> = rt.script.calls_to("sys.answer").iter().map(|c| c.args["op"].clone()).collect();
        assert_eq!(ops, vec![json!("sys.subscribe"), json!("sys.unsubscribe")]);

        let (host, host_sink) = host_socket(&served).await;
        let own = request(&host, &host_sink, 8, "sys.subscribe", json!({ "placeId": "p_1" })).await;
        assert_eq!(own.to_string(), json!({ "id": 8, "ok": true }).to_string());
        assert_eq!(rt.script.calls_to("sys.answer").last().unwrap().args["placeId"], json!("p_1"));
    })
}

// ---- S18: exec -----------------------------------------------------------------------------------------------

struct FakeExec {
    lines: Mutex<Option<mpsc::UnboundedReceiver<Result<String, ExecFailure>>>>,
    exited: SharedFuture<Option<i32>>,
    torn: AtomicUsize,
}

struct ExecHandle {
    lines: mpsc::UnboundedSender<Result<String, ExecFailure>>,
    exit: Option<oneshot::Sender<Option<i32>>>,
}

impl ExecHandle {
    fn say(&self, line: &str) {
        self.lines.unbounded_send(Ok(line.to_string())).unwrap();
    }
    fn finish(mut self, code: Option<i32>) {
        let exit = self.exit.take().unwrap();
        drop(self.lines);
        let _ = exit.send(code);
    }
}

fn fake_exec() -> (Arc<FakeExec>, ExecHandle) {
    let (lines_tx, lines_rx) = mpsc::unbounded();
    let (exit_tx, exit_rx) = oneshot::channel::<Option<i32>>();
    let exited = async move { exit_rx.await.unwrap_or(None) }.boxed().shared();
    (Arc::new(FakeExec { lines: Mutex::new(Some(lines_rx)), exited, torn: AtomicUsize::new(0) }), ExecHandle { lines: lines_tx, exit: Some(exit_tx) })
}

impl ExecStream for FakeExec {
    fn lines(&self) -> BoxStream<'static, Result<String, ExecFailure>> {
        match self.lines.lock().unwrap().take() {
            Some(rx) => rx.boxed(),
            None => futures::stream::empty().boxed(),
        }
    }
    fn teardown(&self) {
        self.torn.fetch_add(1, Ordering::SeqCst);
    }
    fn kill(&self) {}
    fn write(&self, _line: String) -> BoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        async { Ok(WriteOutcome::Written) }.boxed()
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.exited.clone()
    }
}

fn run_of(exec: &Arc<FakeExec>, ran_in: Option<&str>) -> RunningExec {
    RunningExec { stream: exec.clone() as Arc<dyn ExecStream>, ran_in: ran_in.map(str::to_string) }
}

fn exec_frames(sink: &RecordingSink) -> Vec<Value> {
    sink.pushes().into_iter().filter(|f| f["type"].as_str().map_or(false, |t| t.starts_with("exec."))).collect()
}

// S18. serve.test.ts:992
#[test]
fn exec_replies_then_streams_lines_and_exit_to_the_asker_only() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (asker, asker_sink) = host_socket(&served).await;
        let (other, other_sink) = host_socket(&served).await;
        request(&other, &other_sink, 1, "events.subscribe", json!({})).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        let argv = json!(["sh", "-c", "printf 'one\\ntwo\\n'; exit 3"]);
        let started = request(&asker, &asker_sink, 2, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": argv })).await;
        assert_eq!(started["ok"], json!(true));
        let exec_id = started["execId"].as_str().unwrap().to_string();
        assert!(is_hex12(&exec_id));
        assert_eq!(started.to_string(), json!({ "id": 2, "ok": true, "execId": exec_id }).to_string(), "no cwd where the stream names none");
        assert_eq!(rt.script.calls_to("workspaces.execStream")[0].args, json!({ "id": "ws_1", "argv": argv, "cwd": null, "origin": null }));

        handle.say("one");
        handle.say("two");
        handle.finish(Some(3));
        settle().await;
        assert_eq!(
            exec_frames(&asker_sink),
            vec![
                json!({ "type": "exec.output", "execId": exec_id, "text": "one" }),
                json!({ "type": "exec.output", "execId": exec_id, "text": "two" }),
                json!({ "type": "exec.exit", "execId": exec_id, "exitCode": 3 }),
            ]
        );
        assert!(exec_frames(&other_sink).is_empty(), "nothing reaches another socket");
        let all = asker_sink.frames();
        let reply_at = all.iter().position(|f| f.get("id") == Some(&json!(2))).unwrap();
        let first_line_at = all.iter().position(|f| f["type"] == json!("exec.output")).unwrap();
        assert!(reply_at < first_line_at, "the reply goes first");

        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Err(err("no such workspace: ws_nope")));
        let missing = request(&asker, &asker_sink, 3, "workspaces.exec", json!({ "workspaceId": "ws_nope", "argv": ["true"] })).await;
        assert_eq!(missing.to_string(), refused(3, "no such workspace: ws_nope", None));
    })
}

// S18. serve.test.ts:1033
#[test]
fn exec_names_the_folder_it_ran_in() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, Some("/root/work/my proj"))));
        let reply = request(&socket, &sink, 1, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": ["git", "status"], "cwd": "/root/work/my proj" })).await;
        let exec_id = reply["execId"].as_str().unwrap();
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "execId": exec_id, "cwd": "/root/work/my proj" }).to_string());
        assert_eq!(rt.script.calls_to("workspaces.execStream")[0].args["cwd"], json!("/root/work/my proj"));
        handle.finish(Some(0));
        settle().await;
        assert_eq!(exec_frames(&sink).last().unwrap()["type"], json!("exec.exit"));
    })
}

// S18. serve.test.ts:1072
#[test]
fn the_asking_socket_closing_ends_the_command() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, _handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        let started = request(&socket, &sink, 1, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": ["sleep", "600"] })).await;
        assert_eq!(started["ok"], json!(true));
        assert_eq!(exec.torn.load(Ordering::SeqCst), 0);
        socket.closed();
        assert_eq!(exec.torn.load(Ordering::SeqCst), 1, "the socket's close tears the stream down");
    })
}

// S18. serve.test.ts:1088
#[test]
fn an_ended_exec_is_not_torn_down_again() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        request(&socket, &sink, 1, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": ["true"] })).await;
        handle.finish(Some(0));
        settle().await;
        assert_eq!(exec_frames(&sink).len(), 1, "the exit was said");
        socket.closed();
        assert_eq!(exec.torn.load(Ordering::SeqCst), 0, "a command that has exited is not signalled again when its socket goes");
    })
}

// S18. serve.ts:1656-1678
#[test]
fn a_failed_read_rides_the_exit() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        let started = request(&socket, &sink, 1, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": ["sleep", "600"] })).await;
        let exec_id = started["execId"].as_str().unwrap().to_string();
        handle.say("partial");
        handle.lines.unbounded_send(Err(ExecFailure { kind: ExecFailureKind::Other, message: "connection lost".into() })).unwrap();
        handle.finish(None);
        settle().await;
        assert_eq!(
            exec_frames(&sink),
            vec![
                json!({ "type": "exec.output", "execId": exec_id, "text": "partial" }),
                json!({ "type": "exec.exit", "execId": exec_id, "exitCode": null, "error": "connection lost" }),
            ],
            "the exit carries the failure of reading the lines, once"
        );
    })
}
