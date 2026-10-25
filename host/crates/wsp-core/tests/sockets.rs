// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sockets.md: one socket's live readings (sys-socket.ts), its sign-ins (sign-in-socket.ts) and every forward
//! source as one (forwards.ts). The TS tests reach these only through `serveRuntime`; these drive each handler with a
//! socket, a runtime and a send the test holds, and pin the frames each sends, in order.
mod rest;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt;
use rest::*;
use serde_json::{json, Map, Value};
use tokio::sync::oneshot;
use wsp_core::forwards::{forwards_of, ForwardEvent, ForwardsSource};
use wsp_core::sign_in_socket::{sign_in_op, sign_in_socket, CloudSignIn, SignInOp, SignInRuns, CLOUD_SIGN_IN_UNSERVED, SIGN_IN_OPS};
use wsp_core::sys_socket::{sys_socket, Send, SocketSide, SysAsk, SysWatch};
use wsp_core::types::api::SignInFollowing;
use wsp_core::types::pending::SignInAsk;
use wsp_core::types::wiring::{Detach, SysListener};
use wsp_wire::types::agents_report::{AgentsSignInEvent, AgentsSignInRun, AgentsTarget, McpScope, NO_SIGN_IN_REFUSAL};
use wsp_wire::views::place::{ForwardCloseEvent, ForwardOpenEvent};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::helpers::ReqId;
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::place_link::SYS_SUBSCRIBE_ONE_OF;
use wsp_wire::wire::runtime::AgentsCloudSignInAgent;
use wsp_wire::wire::runtime_ops::RuntimeOp;
use wsp_words::exit::HostError;

/// The socket as the door holds it: open until the test closes it, its leaves run then, in the order registered.
#[derive(Default)]
struct Sock {
    closed: Cell<bool>,
    leaves: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl Sock {
    fn close(&self) {
        self.closed.set(true);
        let leaves: Vec<_> = self.leaves.borrow_mut().drain(..).collect();
        for leave in leaves {
            leave();
        }
    }
    fn leaves(&self) -> usize {
        self.leaves.borrow().len()
    }
}

impl SocketSide for Sock {
    fn open(&self) -> bool {
        !self.closed.get()
    }
    fn detach(&self, leave: Box<dyn FnOnce()>) {
        self.leaves.borrow_mut().push(leave);
    }
}

fn sent() -> (Rc<RefCell<Vec<Value>>>, Send) {
    let out: Rc<RefCell<Vec<Value>>> = Rc::default();
    let o = out.clone();
    (out, Rc::new(move |frame: Map<String, Value>| o.borrow_mut().push(Value::Object(frame))))
}

fn id(n: i64) -> ReqId {
    wire(&json!(n))
}

fn sample(cpu: f64) -> SysSample {
    wire(&json!({ "cpu": cpu, "load1": 0.5, "mem": { "used": 1, "total": 4 }, "disk": { "used": 2, "total": 8 }, "at": 1000 }))
}

/// A runtime whose watches the test answers: each records its ask, hands the listener out, and answers a detach that
/// counts its calls; `hold` keeps the next watch unanswered until sent, `refuse` fails it.
#[derive(Default)]
struct Watches {
    asked: RefCell<Vec<(String, String, Option<Caller>)>>,
    listeners: RefCell<Vec<SysListener>>,
    detached: Rc<RefCell<Vec<String>>>,
    hold: RefCell<Option<oneshot::Receiver<()>>>,
    refuse: Cell<bool>,
}

impl Watches {
    async fn watch(&self, kind: &str, id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
        self.asked.borrow_mut().push((kind.into(), id.into(), origin));
        self.listeners.borrow_mut().push(listener);
        let held = self.hold.borrow_mut().take();
        if let Some(rx) = held {
            let _ = rx.await;
        }
        if self.refuse.get() {
            return Err(HostError::new("that computer is not joined"));
        }
        let (d, key) = (self.detached.clone(), format!("{kind}:{id}"));
        Ok(Box::new(move || d.borrow_mut().push(key)))
    }
}

#[async_trait(?Send)]
impl SysWatch for Watches {
    async fn watch_place_sys(&self, place_id: &str, listener: SysListener) -> Result<Detach, HostError> {
        self.watch("place", place_id, listener, None).await
    }
    async fn watch_workspace_sys(&self, workspace_id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
        self.watch("workspace", workspace_id, listener, origin).await
    }
}

fn ask(n: i64, unsubscribe: bool, workspace: Option<&str>, place: Option<&str>) -> SysAsk {
    SysAsk { id: id(n), unsubscribe, workspace_id: workspace.map(String::from), place_id: place.map(String::from) }
}

// spec: S1
#[test]
fn a_sys_ask_naming_both_or_neither_daemon_is_refused_before_anything_is_sent_or_watched() {
    run_local(async {
        let rt = Rc::new(Watches::default());
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        let (out, send) = sent();
        for a in [ask(1, false, Some("w1"), Some("p1")), ask(2, false, None, None), ask(3, true, Some("w1"), Some("p1"))] {
            let e = handle(a, send.clone(), None).await.unwrap_err();
            assert_eq!(e.message, SYS_SUBSCRIBE_ONE_OF);
        }
        assert_eq!(*out.borrow(), Vec::<Value>::new());
        assert!(rt.asked.borrow().is_empty());
    });
}

// spec: S2, S4 (a workspace's watch carries the origin; its readings are pushed as workspace.sys; the answer is {id, ok})
#[test]
fn a_workspace_subscribe_watches_once_with_the_origin_and_pushes_its_readings() {
    run_local(async {
        let rt = Rc::new(Watches::default());
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        let (out, send) = sent();
        let origin = Some(Caller::Road(WorkspaceOrigin::Here));
        handle(ask(1, false, Some("w1"), None), send.clone(), origin.clone()).await.unwrap();
        assert_eq!(*rt.asked.borrow(), vec![("workspace".to_string(), "w1".to_string(), origin)]);
        assert_eq!(*out.borrow(), vec![json!({ "id": 1, "ok": true })]);
        (rt.listeners.borrow()[0])(sample(0.25));
        assert_eq!(out.borrow()[1], json!({ "type": "workspace.sys", "workspaceId": "w1", "sample": json(&sample(0.25)) }));
        assert_eq!(serde_json::to_string(&out.borrow()[0]).unwrap(), r#"{"id":1,"ok":true}"#);
    });
}

// spec: S3, S4, S8 (a place's watch takes no origin; a second subscribe for the same key answers ok and watches nothing more)
#[test]
fn a_place_subscribe_twice_watches_once_and_a_workspace_of_the_same_id_is_its_own_key() {
    run_local(async {
        let rt = Rc::new(Watches::default());
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        let (out, send) = sent();
        handle(ask(1, false, None, Some("x")), send.clone(), Some(Caller::Road(WorkspaceOrigin::Here))).await.unwrap();
        handle(ask(2, false, None, Some("x")), send.clone(), None).await.unwrap();
        handle(ask(3, false, Some("x"), None), send.clone(), None).await.unwrap();
        assert_eq!(*rt.asked.borrow(), vec![("place".to_string(), "x".to_string(), None), ("workspace".to_string(), "x".to_string(), None)]);
        assert_eq!(*out.borrow(), vec![json!({ "id": 1, "ok": true }), json!({ "id": 2, "ok": true }), json!({ "id": 3, "ok": true })]);
        (rt.listeners.borrow()[0])(sample(0.5));
        assert_eq!(out.borrow()[3], json!({ "type": "place.sys", "placeId": "x", "sample": json(&sample(0.5)) }));
    });
}

// spec: S2 (unsubscribe detaches the one key and answers ok; one for nothing held answers ok too)
#[test]
fn unsubscribe_detaches_that_watch_alone_and_answers_ok_even_for_nothing_held() {
    run_local(async {
        let rt = Rc::new(Watches::default());
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        let (out, send) = sent();
        handle(ask(1, false, None, Some("p1")), send.clone(), None).await.unwrap();
        handle(ask(2, false, Some("w1"), None), send.clone(), None).await.unwrap();
        handle(ask(3, true, None, Some("p1")), send.clone(), None).await.unwrap();
        assert_eq!(*rt.detached.borrow(), ["place:p1"]);
        handle(ask(4, true, None, Some("p1")), send.clone(), None).await.unwrap();
        handle(ask(5, true, Some("nobody"), None), send.clone(), None).await.unwrap();
        assert_eq!(*rt.detached.borrow(), ["place:p1"]);
        assert_eq!(out.borrow().iter().map(|f| f["id"].clone()).collect::<Vec<_>>(), [json!(1), json!(2), json!(3), json!(4), json!(5)]);
        handle(ask(6, false, None, Some("p1")), send.clone(), None).await.unwrap();
        assert_eq!(rt.asked.borrow().len(), 3, "a key unsubscribed is watched again");
    });
}

// spec: S7 (the socket closing detaches every watch it holds)
#[test]
fn the_socket_closing_detaches_every_watch_it_holds() {
    run_local(async {
        let rt = Rc::new(Watches::default());
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        assert_eq!(sock.leaves(), 1, "the leave is registered when the handler is made");
        let (_out, send) = sent();
        handle(ask(1, false, None, Some("p1")), send.clone(), None).await.unwrap();
        handle(ask(2, false, Some("w1"), None), send.clone(), None).await.unwrap();
        sock.close();
        let mut d = rt.detached.borrow().clone();
        d.sort();
        assert_eq!(d, ["place:p1", "workspace:w1"]);
    });
}

// spec: S6 (an unsubscribe while the first reading is in flight: the watch is detached as it lands, both answered ok)
#[test]
fn an_unsubscribe_while_the_watch_is_in_flight_detaches_it_as_it_lands() {
    run_local(async {
        let (tx, rx) = oneshot::channel();
        let rt = Rc::new(Watches { hold: RefCell::new(Some(rx)), ..Watches::default() });
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        let (out, send) = sent();
        let first = tokio::task::spawn_local(handle(ask(1, false, Some("w1"), None), send.clone(), None));
        settle().await;
        handle(ask(2, false, Some("w1"), None), send.clone(), None).await.unwrap();
        assert_eq!(rt.asked.borrow().len(), 1, "a key pending is not watched twice");
        handle(ask(3, true, Some("w1"), None), send.clone(), None).await.unwrap();
        tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        assert_eq!(*rt.detached.borrow(), ["workspace:w1"]);
        assert_eq!(*out.borrow(), vec![json!({ "id": 2, "ok": true }), json!({ "id": 3, "ok": true }), json!({ "id": 1, "ok": true })]);
        handle(ask(4, false, Some("w1"), None), send.clone(), None).await.unwrap();
        assert_eq!(rt.asked.borrow().len(), 2, "nothing of the dropped watch stands");
    });
}

// spec: S6 (the page left while the first reading was in flight: detached at once, still answered ok)
#[test]
fn a_socket_closed_while_the_watch_is_in_flight_detaches_it_as_it_lands() {
    run_local(async {
        let (tx, rx) = oneshot::channel();
        let rt = Rc::new(Watches { hold: RefCell::new(Some(rx)), ..Watches::default() });
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        let (out, send) = sent();
        let first = tokio::task::spawn_local(handle(ask(1, false, None, Some("p1")), send.clone(), None));
        settle().await;
        sock.close();
        tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        assert_eq!(*rt.detached.borrow(), ["place:p1"]);
        assert_eq!(*out.borrow(), vec![json!({ "id": 1, "ok": true })]);
    });
}

// spec: S5 (a watch that fails passes its error on, sends nothing, and leaves the key free)
#[test]
fn a_watch_that_fails_passes_the_error_on_and_the_next_subscribe_watches_again() {
    run_local(async {
        let rt = Rc::new(Watches::default());
        rt.refuse.set(true);
        let sock = Rc::new(Sock::default());
        let handle = sys_socket(rt.clone(), sock.clone());
        let (out, send) = sent();
        let e = handle(ask(1, false, None, Some("p1")), send.clone(), None).await.unwrap_err();
        assert_eq!(e.message, "that computer is not joined");
        assert_eq!(*out.borrow(), Vec::<Value>::new());
        rt.refuse.set(false);
        handle(ask(2, false, None, Some("p1")), send.clone(), None).await.unwrap();
        assert_eq!(rt.asked.borrow().len(), 2);
        assert_eq!(*out.borrow(), vec![json!({ "id": 2, "ok": true })]);
    });
}

fn target() -> AgentsTarget {
    wire(&json!({ "placeId": "here" }))
}

fn event(sign_in_id: &str, state: &str) -> AgentsSignInEvent {
    wire(&json!({ "signInId": sign_in_id, "state": state }))
}

fn frame_of(e: &AgentsSignInEvent) -> Value {
    let mut f = json!({ "type": "agents.signIn" });
    for (k, v) in json(e).as_object().unwrap() {
        f[k] = v.clone();
    }
    f
}

/// The agents' runs as the test scripts them: a start answers the id given and emits what it is told to while it
/// runs; every call is recorded, and every following's leave counts.
#[derive(Default)]
struct Runs {
    calls: RefCell<Vec<Value>>,
    next_id: RefCell<String>,
    emit_during_start: RefCell<Vec<AgentsSignInEvent>>,
    emitters: RefCell<Vec<(String, Rc<dyn Fn(AgentsSignInEvent)>)>>,
    leaves: Rc<RefCell<Vec<String>>>,
    runs: RefCell<Vec<AgentsSignInRun>>,
    refuse: Cell<bool>,
}

impl Runs {
    fn following(&self, sign_in_id: &str) -> SignInFollowing {
        let (l, sid) = (self.leaves.clone(), sign_in_id.to_string());
        SignInFollowing { sign_in_id: sign_in_id.to_string(), leave: Box::new(move || l.borrow_mut().push(sid)) }
    }
    fn emit(&self, sign_in_id: &str, e: AgentsSignInEvent) {
        let f = self.emitters.borrow().iter().find(|(id, _)| id == sign_in_id).map(|(_, f)| f.clone()).expect("an emitter");
        f(e);
    }
}

#[async_trait(?Send)]
impl SignInRuns for Runs {
    async fn sign_in(&self, _target: AgentsTarget, ask: SignInAsk, emit: Rc<dyn Fn(AgentsSignInEvent)>, origin: Option<Caller>) -> Result<SignInFollowing, HostError> {
        self.calls.borrow_mut().push(json!({
            "op": "sign_in", "agent": ask.agent, "server": ask.server, "terminal": ask.terminal,
            "scope": ask.scope.map(|s| json(&s)), "project": ask.project, "origin": origin.is_some(),
        }));
        if self.refuse.get() {
            return Err(HostError::new("claude is not installed there"));
        }
        let sid = self.next_id.borrow().clone();
        for e in self.emit_during_start.borrow_mut().drain(..) {
            emit(e);
        }
        self.emitters.borrow_mut().push((sid.clone(), emit));
        Ok(self.following(&sid))
    }
    fn sign_ins(&self) -> Vec<AgentsSignInRun> {
        self.calls.borrow_mut().push(json!({ "op": "sign_ins" }));
        self.runs.borrow().clone()
    }
    fn sign_in_follow(&self, sign_in_id: &str, emit: Rc<dyn Fn(AgentsSignInEvent)>) -> Result<SignInFollowing, HostError> {
        self.calls.borrow_mut().push(json!({ "op": "follow", "id": sign_in_id }));
        self.emitters.borrow_mut().push((sign_in_id.to_string(), emit));
        Ok(self.following(sign_in_id))
    }
    async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError> {
        self.calls.borrow_mut().push(json!({ "op": "code", "id": sign_in_id, "code": code }));
        Ok(())
    }
    fn sign_in_stop(&self, sign_in_id: &str) {
        self.calls.borrow_mut().push(json!({ "op": "stop", "id": sign_in_id }));
    }
}

fn runs(next: &str) -> Rc<Runs> {
    let r = Rc::new(Runs::default());
    *r.next_id.borrow_mut() = next.into();
    r
}

fn start(n: i64) -> SignInOp {
    SignInOp::SignIn { id: id(n), target: target(), agent: "claude".into(), terminal: None }
}

// spec: I0 (the six ops, and every other op is none of them)
#[test]
fn the_sign_in_ops_are_the_six_and_every_other_op_is_none() {
    assert_eq!(SIGN_IN_OPS, ["agents.signIn", "servers.signIn", "agents.cloudSignIn", "agents.signIns", "agents.signInCode", "agents.signInStop"]);
    let op: RuntimeOp = wire(&json!({ "op": "agents.signInStop", "id": 4, "signInId": "s1" }));
    assert_eq!(sign_in_op(&op), Some(SignInOp::Stop { id: id(4), sign_in_id: "s1".into() }));
    let op: RuntimeOp = wire(&json!({ "op": "servers.signIn", "id": 5, "target": { "placeId": "here" }, "agent": "claude", "name": "linear", "scope": "user" }));
    assert_eq!(
        sign_in_op(&op),
        Some(SignInOp::ServerSignIn { id: id(5), target: target(), agent: "claude".into(), name: "linear".into(), scope: Some(McpScope::User), project: None })
    );
    let op: RuntimeOp = wire(&json!({ "op": "agents.signInLine", "id": 6, "target": { "placeId": "here" }, "agent": "claude" }));
    assert_eq!(sign_in_op(&op), None);
}

// spec: I1 (the answer naming the run goes first; steps pushed while the start ran follow it in order; later steps go straight out)
#[test]
fn a_start_answers_with_its_run_before_the_steps_it_pushed_while_starting() {
    run_local(async {
        let r = runs("s1");
        *r.emit_during_start.borrow_mut() = vec![event("s1", "running"), event("s1", "waiting")];
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (out, send) = sent();
        handle(start(1), send.clone(), Some(Caller::Road(WorkspaceOrigin::Here))).await.unwrap();
        r.emit("s1", event("s1", "signed-in"));
        assert_eq!(
            *out.borrow(),
            vec![
                json!({ "id": 1, "ok": true, "signInId": "s1" }),
                frame_of(&event("s1", "running")),
                frame_of(&event("s1", "waiting")),
                frame_of(&event("s1", "signed-in")),
            ]
        );
        assert_eq!(r.calls.borrow()[0], json!({ "op": "sign_in", "agent": "claude", "server": null, "terminal": null, "scope": null, "project": null, "origin": true }));
        assert_eq!(sock.leaves(), 1);
    });
}

// spec: I1 (a step pushed after the socket closed is dropped)
#[test]
fn a_step_pushed_after_the_socket_closed_is_dropped() {
    run_local(async {
        let r = runs("s1");
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (out, send) = sent();
        handle(start(1), send.clone(), None).await.unwrap();
        sock.close();
        r.emit("s1", event("s1", "signed-in"));
        assert_eq!(*out.borrow(), vec![json!({ "id": 1, "ok": true, "signInId": "s1" })]);
        assert_eq!(*r.leaves.borrow(), ["s1"], "the close ran the following's leave");
        assert!(!r.calls.borrow().iter().any(|c| c["op"] == "stop"), "a leave never ends a run");
    });
}

// spec: I4 (a terminal start asks for the terminal; a server's start carries its name, scope and project)
#[test]
fn a_terminal_start_and_a_servers_start_ask_with_their_own_fields() {
    run_local(async {
        let r = runs("s1");
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (_out, send) = sent();
        handle(SignInOp::SignIn { id: id(1), target: target(), agent: "codex".into(), terminal: Some(true) }, send.clone(), None).await.unwrap();
        *r.next_id.borrow_mut() = "s2".into();
        handle(SignInOp::SignIn { id: id(2), target: target(), agent: "codex".into(), terminal: Some(false) }, send.clone(), None).await.unwrap();
        *r.next_id.borrow_mut() = "s3".into();
        handle(SignInOp::ServerSignIn { id: id(3), target: target(), agent: "claude".into(), name: "linear".into(), scope: Some(McpScope::Local), project: Some("pr_1".into()) }, send.clone(), None).await.unwrap();
        assert_eq!(
            *r.calls.borrow(),
            vec![
                json!({ "op": "sign_in", "agent": "codex", "server": null, "terminal": true, "scope": null, "project": null, "origin": false }),
                json!({ "op": "sign_in", "agent": "codex", "server": null, "terminal": null, "scope": null, "project": null, "origin": false }),
                json!({ "op": "sign_in", "agent": "claude", "server": "linear", "terminal": null, "scope": "local", "project": "pr_1", "origin": false }),
            ]
        );
    });
}

// spec: I2 (a start that joins a run this socket already holds leaves the new following and answers the id alone)
#[test]
fn a_start_joining_a_run_already_held_leaves_the_new_following_and_answers_the_id() {
    run_local(async {
        let r = runs("s1");
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (out, send) = sent();
        handle(start(1), send.clone(), None).await.unwrap();
        *r.emit_during_start.borrow_mut() = vec![event("s1", "running")];
        handle(start(2), send.clone(), None).await.unwrap();
        assert_eq!(*out.borrow(), vec![json!({ "id": 1, "ok": true, "signInId": "s1" }), json!({ "id": 2, "ok": true, "signInId": "s1" })]);
        assert_eq!(*r.leaves.borrow(), ["s1"]);
        assert_eq!(sock.leaves(), 1, "no second leave is registered");
    });
}

// spec: I3 (a socket that closed while the start ran leaves at once and is sent nothing)
#[test]
fn a_socket_closed_before_the_start_answered_leaves_and_hears_nothing() {
    run_local(async {
        let r = runs("s1");
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (out, send) = sent();
        sock.close();
        *r.emit_during_start.borrow_mut() = vec![event("s1", "running")];
        handle(start(1), send.clone(), None).await.unwrap();
        assert_eq!(*out.borrow(), Vec::<Value>::new());
        assert_eq!(*r.leaves.borrow(), ["s1"]);
    });
}

// spec: I5 (a cloud sign-in with no road wired is refused; with one, the road is asked and its run held)
#[test]
fn a_cloud_sign_in_is_refused_unserved_and_otherwise_runs_the_hosts_own_road() {
    run_local(async {
        let r = runs("s1");
        let sock = Rc::new(Sock::default());
        let (out, send) = sent();
        let bare = sign_in_socket(r.clone(), sock.clone(), None);
        let e = bare(SignInOp::CloudSignIn { id: id(1), agent: AgentsCloudSignInAgent::Codex }, send.clone(), None).await.unwrap_err();
        assert_eq!(e.message, CLOUD_SIGN_IN_UNSERVED);
        assert_eq!(*out.borrow(), Vec::<Value>::new());
        let asked: Rc<RefCell<Vec<String>>> = Rc::default();
        let a = asked.clone();
        let leaves = r.leaves.clone();
        let cloud: CloudSignIn = Rc::new(move |agent, emit, origin| {
            a.borrow_mut().push(format!("{} {}", json(&agent), origin.is_some()));
            emit(event("c1", "running"));
            let l = leaves.clone();
            async move { Ok(SignInFollowing { sign_in_id: "c1".into(), leave: Box::new(move || l.borrow_mut().push("c1".into())) }) }.boxed_local()
        });
        let served = sign_in_socket(r.clone(), sock.clone(), Some(cloud));
        served(SignInOp::CloudSignIn { id: id(2), agent: AgentsCloudSignInAgent::Claude }, send.clone(), Some(Caller::Road(WorkspaceOrigin::Here))).await.unwrap();
        assert_eq!(*asked.borrow(), [r#""claude" true"#]);
        assert_eq!(*out.borrow(), vec![json!({ "id": 2, "ok": true, "signInId": "c1" }), frame_of(&event("c1", "running"))]);
        assert!(r.calls.borrow().is_empty(), "the runtime's agents were not asked");
    });
}

// spec: I8 (a start the agents refuse passes the refusal on and sends nothing)
#[test]
fn a_refused_start_passes_the_refusal_on_and_holds_nothing() {
    run_local(async {
        let r = runs("s1");
        r.refuse.set(true);
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (out, send) = sent();
        let e = handle(start(1), send.clone(), None).await.unwrap_err();
        assert_eq!(e.message, "claude is not installed there");
        handle(SignInOp::Stop { id: id(2), sign_in_id: "s1".into() }, send.clone(), None).await.unwrap();
        assert_eq!(*out.borrow(), vec![json!({ "id": 2, "ok": false, "error": NO_SIGN_IN_REFUSAL, "kind": "usage" })]);
        assert_eq!(sock.leaves(), 0);
    });
}

fn run_row(sign_in_id: &str, ended: bool) -> AgentsSignInRun {
    let mut v = json!({ "signInId": sign_in_id, "target": { "placeId": "here" }, "agent": "claude" });
    if ended {
        v["ended"] = json!(true);
    }
    wire(&v)
}

// spec: I6 (the list follows every run not ended and not held, and answers every run, ended ones included)
#[test]
fn the_list_follows_the_live_runs_this_socket_does_not_hold_and_answers_them_all() {
    run_local(async {
        let r = runs("s1");
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (out, send) = sent();
        handle(start(1), send.clone(), None).await.unwrap();
        *r.runs.borrow_mut() = vec![run_row("s1", false), run_row("s2", false), run_row("s3", true)];
        handle(SignInOp::SignIns { id: id(2) }, send.clone(), None).await.unwrap();
        let follows: Vec<Value> = r.calls.borrow().iter().filter(|c| c["op"] == "follow").cloned().collect();
        assert_eq!(follows, vec![json!({ "op": "follow", "id": "s2" })]);
        assert_eq!(out.borrow()[1], json!({ "id": 2, "ok": true, "runs": [json(&run_row("s1", false)), json(&run_row("s2", false)), json(&run_row("s3", true))] }));
        assert_eq!(sock.leaves(), 2);
        r.emit("s2", event("s2", "waiting"));
        assert_eq!(out.borrow()[2], frame_of(&event("s2", "waiting")));
        handle(SignInOp::Code { id: id(3), sign_in_id: "s2".into(), code: "ABCD".into() }, send.clone(), None).await.unwrap();
        handle(SignInOp::Stop { id: id(4), sign_in_id: "s3".into() }, send.clone(), None).await.unwrap();
        assert_eq!(out.borrow()[3], json!({ "id": 3, "ok": true }));
        assert_eq!(out.borrow()[4], json!({ "id": 4, "ok": false, "error": NO_SIGN_IN_REFUSAL, "kind": "usage" }));
    });
}

// spec: I7 (a code or a stop reaches only a run this socket holds; each is answered {id, ok} once done)
#[test]
fn a_code_and_a_stop_reach_a_held_run_and_are_refused_for_any_other() {
    run_local(async {
        let r = runs("s1");
        let sock = Rc::new(Sock::default());
        let handle = sign_in_socket(r.clone(), sock.clone(), None);
        let (out, send) = sent();
        handle(SignInOp::Code { id: id(1), sign_in_id: "s1".into(), code: "1234".into() }, send.clone(), None).await.unwrap();
        handle(start(2), send.clone(), None).await.unwrap();
        handle(SignInOp::Code { id: id(3), sign_in_id: "s1".into(), code: "1234".into() }, send.clone(), None).await.unwrap();
        handle(SignInOp::Stop { id: id(4), sign_in_id: "s1".into() }, send.clone(), None).await.unwrap();
        assert_eq!(
            *out.borrow(),
            vec![
                json!({ "id": 1, "ok": false, "error": NO_SIGN_IN_REFUSAL, "kind": "usage" }),
                json!({ "id": 2, "ok": true, "signInId": "s1" }),
                json!({ "id": 3, "ok": true }),
                json!({ "id": 4, "ok": true }),
            ]
        );
        let ops: Vec<Value> = r.calls.borrow().iter().filter(|c| c["op"] == "code" || c["op"] == "stop").cloned().collect();
        assert_eq!(ops, vec![json!({ "op": "code", "id": "s1", "code": "1234" }), json!({ "op": "stop", "id": "s1" })]);
    });
}

fn forward(ws: &str, port: i64) -> PortForward {
    wire(&json!({ "workspaceId": ws, "port": port, "startedAt": "2026-10-25T00:00:00.000Z", "name": format!("{ws}:{port}"), "kind": "url" }))
}

/// One forward source the test scripts.
struct Source {
    name: &'static str,
    held: Vec<PortForward>,
    stops: bool,
    asked: Rc<RefCell<Vec<String>>>,
    listeners: RefCell<Vec<Rc<dyn Fn(&ForwardEvent)>>>,
    offs: Rc<RefCell<Vec<&'static str>>>,
}

impl ForwardsSource for Source {
    fn list(&self) -> Vec<PortForward> {
        self.held.clone()
    }
    fn stop(&self, workspace_id: &str, port: i64) -> bool {
        self.asked.borrow_mut().push(format!("{} {workspace_id}:{port}", self.name));
        self.stops
    }
    fn on(&self, listener: Rc<dyn Fn(&ForwardEvent)>) -> Detach {
        self.listeners.borrow_mut().push(listener);
        let (offs, name) = (self.offs.clone(), self.name);
        Box::new(move || offs.borrow_mut().push(name))
    }
}

fn source(name: &'static str, held: Vec<PortForward>, stops: bool, asked: &Rc<RefCell<Vec<String>>>, offs: &Rc<RefCell<Vec<&'static str>>>) -> Rc<Source> {
    Rc::new(Source { name, held, stops, asked: asked.clone(), listeners: RefCell::default(), offs: offs.clone() })
}

// spec: W1, W2, W3
#[test]
fn every_forward_source_as_one_lists_in_order_stops_at_the_first_holder_and_hears_each() {
    let asked: Rc<RefCell<Vec<String>>> = Rc::default();
    let offs: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let relay = source("relay", vec![forward("w1", 3000), forward("w2", 5173)], false, &asked, &offs);
    let panes = source("panes", vec![forward("p1", 8080)], true, &asked, &offs);
    let last = source("last", vec![], true, &asked, &offs);
    let all = forwards_of(vec![Some(relay.clone() as Rc<dyn ForwardsSource>), None, Some(panes.clone()), Some(last.clone())]);
    assert_eq!(all.list(), vec![forward("w1", 3000), forward("w2", 5173), forward("p1", 8080)]);
    assert!(all.stop("p1", 8080));
    assert_eq!(*asked.borrow(), ["relay p1:8080", "panes p1:8080"], "the source after the holder is never asked");
    let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
    let h = heard.clone();
    let off = all.on(Rc::new(move |e: &ForwardEvent| {
        h.borrow_mut().push(match e {
            ForwardEvent::Open(o) => json!({ "open": json(&o.forward) }),
            ForwardEvent::Close(c) => json!({ "close": json(c) }),
        })
    }));
    assert_eq!([relay.listeners.borrow().len(), panes.listeners.borrow().len(), last.listeners.borrow().len()], [1, 1, 1]);
    (relay.listeners.borrow()[0])(&ForwardEvent::Close(wire::<ForwardCloseEvent>(&json!({ "workspaceId": "w1", "port": 3000 }))));
    (panes.listeners.borrow()[0])(&ForwardEvent::Open(ForwardOpenEvent { forward: forward("p2", 9000) }));
    assert_eq!(*heard.borrow(), vec![json!({ "close": { "workspaceId": "w1", "port": 3000 } }), json!({ "open": json(&forward("p2", 9000)) })]);
    off();
    assert_eq!(*offs.borrow(), ["relay", "panes", "last"]);
}

// spec: W2 (no source holds it: false, every source asked once)
#[test]
fn a_stop_no_source_holds_is_false_after_asking_each_once() {
    let asked: Rc<RefCell<Vec<String>>> = Rc::default();
    let offs: Rc<RefCell<Vec<&'static str>>> = Rc::default();
    let all = forwards_of(vec![Some(source("a", vec![], false, &asked, &offs) as Rc<dyn ForwardsSource>), Some(source("b", vec![], false, &asked, &offs))]);
    assert!(!all.stop("w9", 1));
    assert_eq!(*asked.borrow(), ["a w9:1", "b w9:1"]);
    let none = forwards_of(vec![None, None]);
    assert_eq!(none.list(), Vec::<PortForward>::new());
    assert!(!none.stop("w9", 1));
    (none.on(Rc::new(|_: &ForwardEvent| {})))();
}
