use support::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::channel::{mpsc, oneshot};
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use support::fake_places::dbg;
use wsp_adapters::port::{ExecFailure, ExecFailureKind, ExecStream, SharedFuture, WriteOutcome};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, Frame};
use wsp_core::devices::DeviceDoor;
use wsp_core::forwards::{ForwardEvent, ForwardsSource};
use wsp_core::status::{Reconcile, StatusListOptions, StatusReader};
use wsp_core::types::api::*;
use wsp_core::types::events::Since;
use wsp_core::types::wiring::Detach;
use wsp_core::types::wiring::{CreatedWorkspace, RunningExec, SizeAsk};
use wsp_serve::serve::door::Served;
use wsp_serve::serve::socket::Socket;
use wsp_serve::serve::workspaces::NO_SUCH_CHANNEL;
use wsp_serve::serve::{PlaceDoctor, ReleaseDoor, ServeOptions, Unlisten};
use wsp_wire::types::release::{ReleaseChangedEvent, ReleaseView};
use wsp_wire::views::place::{DoctorLineEvent, ForwardCloseEvent, ForwardOpenEvent};
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
    listeners: Rc<RefCell<Vec<(u64, Rc<dyn Fn(&ForwardEvent)>)>>>,
    next: Cell<u64>,
}

impl FakeForwards {
    fn new() -> Rc<FakeForwards> {
        Rc::new(FakeForwards { listeners: Rc::new(RefCell::new(Vec::new())), next: Cell::new(0) })
    }
    fn emit(&self, e: &ForwardEvent) {
        let heard: Vec<Rc<dyn Fn(&ForwardEvent)>> = self.listeners.borrow().iter().map(|(_, f)| f.clone()).collect();
        for f in heard {
            f(e);
        }
    }
}

impl ForwardsSource for FakeForwards {
    fn list(&self) -> Vec<PortForward> {
        Vec::new()
    }
    fn stop(&self, _workspace_id: &str, _port: i64) -> bool {
        false
    }
    fn on(&self, f: Rc<dyn Fn(&ForwardEvent)>) -> Detach {
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

fn dbg_s<T: std::fmt::Debug>(x: &T) -> String {
    format!("{x:?}")
}
