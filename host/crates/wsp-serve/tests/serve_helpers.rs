// SPDX-License-Identifier: AGPL-3.0-only
//! The seven serve-*.ts helpers, called directly: the app-process hub, the slate, project, broker, processes and
//! conversations answers, and a sessions.start off the socket. Spec: spec/serve_helpers.md.
mod support;
use support::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::{json, Value};
use wsp_core::types::api::{OutsideOpening, Placed, PlacedStart, ProjectAdded, ProjectRemoved, Runtime, SeedAsk, SettleAsk};
use wsp_core::types::api::{ConversationAsk, WorkspacesApi};
use wsp_serve::serve::served::{Payload, ServedRuntime};
use wsp_serve::serve_app_procs::{
    app_procs_hub_of, is_app_procs_request, AppProcsHub, AppProcsSocket, Push, APP_PROCS_OFF_ROAD, APP_PROCS_UNOFFERED, APP_PROCS_WATCH_OFF_ROAD,
};
use wsp_serve::serve_broker::{answer_broker, BrokerDoor, CloudSignIn, BROKER_UNSERVED};
use wsp_serve::serve_conversations::answer_conversations;
use wsp_serve::serve_processes::answer_processes;
use wsp_serve::serve_projects::{answer_project, thread_places};
use wsp_serve::serve_slates::{answer_slate, hold_key, SlateDoor, SlateHolds};
use wsp_serve::serve_start::{serve_start, start_options_of, AnswerFn, StartFn, StartFolder, StartRoads};
use wsp_wire::types::projects::BRANCH_ON_A_THREAD_LINE;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::session::SessionOrigin;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::place_link::PLACES_TICKET_REFUSAL;
use wsp_wire::wire::runtime::{BrokerStatus, ProviderKeyWorks, SessionStartResult};
use wsp_wire::wire::runtime_ops::{parse_runtime_request, RuntimeRequest};
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::words::thread::{FORK_BESIDE_FIX, FORK_BESIDE_LINE, FORK_RESUME_FIX, FORK_RESUME_LINE};

fn req(op: &str, fields: Value) -> RuntimeRequest {
    let f = frame(1, op, fields);
    parse_runtime_request(&f).unwrap_or_else(|e| panic!("the test's frame does not pass the schema: {e:?}; {f}"))
}

fn here() -> Option<Caller> {
    Some(Caller::Road(WorkspaceOrigin::Here))
}

fn from<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("fixture does not parse: {e}; {v}"))
}

fn args_of(rt: &FakeRuntime, name: &str, n: usize) -> Value {
    rt.script.calls_to(name).get(n).unwrap_or_else(|| panic!("no call {n} of {name}; calls: {:?}", rt.script.names())).args.clone()
}

// ---------------------------------------------------------------------------------------------------------------
// The app's processes: the hub called directly (serve-app-procs.ts).

const PROC_MAIN: fn() -> Value = || json!({ "pid": 501, "kind": "main" });

struct Sock {
    pushes: Rc<RefCell<Vec<Value>>>,
    socket: AppProcsSocket,
}

fn sock(own: bool) -> Sock {
    let pushes = Rc::new(RefCell::new(Vec::new()));
    let kept = pushes.clone();
    let send: Push = Rc::new(move |p: Payload| kept.borrow_mut().push(Value::Object(p)));
    Sock { pushes, socket: AppProcsSocket { send, detaches: Rc::new(RefCell::new(Vec::new())), own } }
}

impl Sock {
    fn ask(&self, hub: &Rc<AppProcsHub>, op: &str, fields: Value) -> Result<Value, HostError> {
        hub.answer(&req(op, fields), &self.socket).map(Value::Object)
    }
    fn close(&self) {
        let leaves: Vec<Box<dyn FnOnce()>> = self.socket.detaches.borrow_mut().drain(..).collect();
        for leave in leaves {
            leave();
        }
    }
    fn pushed(&self) -> Vec<Value> {
        self.pushes.borrow().clone()
    }
}

fn wanted(s: &Sock) -> Vec<Value> {
    s.pushed().into_iter().filter(|p| p["type"] == "app.procs.wanted").map(|p| p["wanted"].clone()).collect()
}

// H1.
#[test]
fn only_appprocs_ops_are_the_hubs() {
    assert!(is_app_procs_request(&req("appProcs.watch", json!({}))));
    assert!(is_app_procs_request(&req("appProcs.set", json!({ "procs": [] }))));
    assert!(!is_app_procs_request(&req("projects.list", json!({}))));
}

// H3. serve-app-procs.test.ts:102
#[test]
fn the_hub_takes_nothing_off_the_own_road() {
    let hub = AppProcsHub::new();
    let relayed = sock(false);
    let offer = relayed.ask(&hub, "appProcs.offer", json!({})).unwrap_err();
    assert_eq!(offer.message, APP_PROCS_OFF_ROAD);
    let watch = relayed.ask(&hub, "appProcs.watch", json!({})).unwrap_err();
    assert_eq!(watch.message, APP_PROCS_WATCH_OFF_ROAD);
    assert!(relayed.pushed().is_empty());
    assert!(relayed.socket.detaches.borrow().is_empty(), "a refused ask joins nothing");
}

// H4. serve-app-procs.test.ts:109
#[test]
fn the_hub_keeps_one_close_per_socket() {
    let hub = AppProcsHub::new();
    let s = sock(true);
    for _ in 0..50 {
        s.ask(&hub, "appProcs.watch", json!({})).unwrap();
        s.ask(&hub, "appProcs.unwatch", json!({})).unwrap();
    }
    assert_eq!(s.socket.detaches.borrow().len(), 1);
}

// H8. serve-app-procs.test.ts:119
#[test]
fn the_hub_names_both_apps_processes() {
    let hub = AppProcsHub::new();
    let page = sock(true);
    let one = sock(true);
    let two = sock(true);
    page.ask(&hub, "appProcs.watch", json!({})).unwrap();
    one.ask(&hub, "appProcs.offer", json!({})).unwrap();
    two.ask(&hub, "appProcs.offer", json!({})).unwrap();
    assert_eq!(one.ask(&hub, "appProcs.set", json!({ "procs": [PROC_MAIN()] })).unwrap(), json!({}));
    two.ask(&hub, "appProcs.set", json!({ "procs": [{ "pid": 900, "kind": "main" }] })).unwrap();
    let last = page.pushed().last().cloned().unwrap();
    assert_eq!(last, json!({ "type": "app.procs", "procs": [PROC_MAIN(), { "pid": 900, "kind": "main" }] }));
    one.close();
    let last = page.pushed().last().cloned().unwrap();
    assert_eq!(last, json!({ "type": "app.procs", "procs": [{ "pid": 900, "kind": "main" }] }));
}

// H2. serve-app-procs.ts:92-102
#[test]
fn two_runtimes_have_two_hubs() {
    let a: Rc<dyn std::any::Any> = Rc::new(1u8);
    let b: Rc<dyn std::any::Any> = Rc::new(2u8);
    assert!(Rc::ptr_eq(&app_procs_hub_of(&a), &app_procs_hub_of(&a)));
    assert!(!Rc::ptr_eq(&app_procs_hub_of(&a), &app_procs_hub_of(&b)));
    let page = sock(true);
    let app = sock(true);
    app_procs_hub_of(&a).answer(&req("appProcs.offer", json!({})), &app.socket).unwrap();
    let seen = page.ask(&app_procs_hub_of(&a), "appProcs.watch", json!({})).unwrap();
    assert_eq!(seen, json!({ "procs": [] }));
    assert_eq!(wanted(&app), vec![json!(true)], "a page on the same runtime asks the app");
    let other = sock(true);
    other.ask(&app_procs_hub_of(&b), "appProcs.watch", json!({})).unwrap();
    assert_eq!(wanted(&app), vec![json!(true)], "a page on another runtime asks nobody here");
}

// H5, H7.
#[test]
fn the_first_watcher_turns_every_offer_on_once_and_an_offer_says_whether_anyone_watches() {
    let hub = AppProcsHub::new();
    let app = sock(true);
    assert_eq!(app.ask(&hub, "appProcs.offer", json!({})).unwrap(), json!({ "wanted": false }));
    let page = sock(true);
    assert_eq!(page.ask(&hub, "appProcs.watch", json!({})).unwrap(), json!({ "procs": [] }));
    assert_eq!(wanted(&app), vec![json!(true)]);
    let other = sock(true);
    other.ask(&hub, "appProcs.watch", json!({})).unwrap();
    assert_eq!(wanted(&app), vec![json!(true)], "the second watcher rides the one ask");
    let late = sock(true);
    assert_eq!(late.ask(&hub, "appProcs.offer", json!({})).unwrap(), json!({ "wanted": true }));
}

// H5, H7. A re-offer keeps the list the socket already named.
#[test]
fn offering_again_keeps_the_list_and_a_watch_answers_every_offer_in_order() {
    let hub = AppProcsHub::new();
    let one = sock(true);
    let two = sock(true);
    let page = sock(true);
    one.ask(&hub, "appProcs.offer", json!({})).unwrap();
    two.ask(&hub, "appProcs.offer", json!({})).unwrap();
    page.ask(&hub, "appProcs.watch", json!({})).unwrap();
    one.ask(&hub, "appProcs.set", json!({ "procs": [PROC_MAIN()] })).unwrap();
    two.ask(&hub, "appProcs.set", json!({ "procs": [{ "pid": 900, "kind": "gpu" }] })).unwrap();
    one.ask(&hub, "appProcs.offer", json!({})).unwrap();
    let other = sock(true);
    assert_eq!(other.ask(&hub, "appProcs.watch", json!({})).unwrap(), json!({ "procs": [PROC_MAIN(), { "pid": 900, "kind": "gpu" }] }));
}

// H6. serve-app-procs.test.ts:46
#[test]
fn unwatching_the_last_page_empties_every_offer_and_says_so() {
    let hub = AppProcsHub::new();
    let app = sock(true);
    let page = sock(true);
    let stranger = sock(true);
    app.ask(&hub, "appProcs.offer", json!({})).unwrap();
    page.ask(&hub, "appProcs.watch", json!({})).unwrap();
    app.ask(&hub, "appProcs.set", json!({ "procs": [PROC_MAIN()] })).unwrap();
    assert_eq!(stranger.ask(&hub, "appProcs.unwatch", json!({})).unwrap(), json!({}));
    assert_eq!(wanted(&app), vec![json!(true)], "a socket that was not watching changes nothing");
    assert_eq!(page.ask(&hub, "appProcs.unwatch", json!({})).unwrap(), json!({}));
    assert_eq!(wanted(&app), vec![json!(true), json!(false)]);
    let next = sock(true);
    assert_eq!(next.ask(&hub, "appProcs.watch", json!({})).unwrap(), json!({ "procs": [] }), "what was read for pages that have gone is not handed on");
}

// H8. serve-app-procs.test.ts:77
#[test]
fn set_comes_after_offer_and_with_no_watcher_the_list_is_dropped() {
    let hub = AppProcsHub::new();
    let app = sock(true);
    let refused = app.ask(&hub, "appProcs.set", json!({ "procs": [PROC_MAIN()] })).unwrap_err();
    assert_eq!(refused.message, APP_PROCS_UNOFFERED);
    app.ask(&hub, "appProcs.offer", json!({})).unwrap();
    assert_eq!(app.ask(&hub, "appProcs.set", json!({ "procs": [PROC_MAIN()] })).unwrap(), json!({}));
    let page = sock(true);
    assert_eq!(page.ask(&hub, "appProcs.watch", json!({})).unwrap(), json!({ "procs": [] }));
    assert!(page.pushed().is_empty(), "a list sent while nobody watched is never pushed");
}

// H4. serve-app-procs.ts:60-66: a close unwatches and, for an offering socket, drops its offer and tells watchers.
#[test]
fn a_close_drops_the_offer_and_tells_every_watcher() {
    let hub = AppProcsHub::new();
    let app = sock(true);
    let page = sock(true);
    page.ask(&hub, "appProcs.watch", json!({})).unwrap();
    app.ask(&hub, "appProcs.offer", json!({})).unwrap();
    app.ask(&hub, "appProcs.set", json!({ "procs": [PROC_MAIN()] })).unwrap();
    app.close();
    assert_eq!(page.pushed().last().cloned().unwrap(), json!({ "type": "app.procs", "procs": [] }));
    page.close();
    assert_eq!(wanted(&app), vec![json!(true)], "the offer was already dropped, so nothing is asked of it");
}

// ---------------------------------------------------------------------------------------------------------------
// The slate answers (serve-slates.ts).

struct Slates {
    log: Rc<RefCell<Vec<Value>>>,
    held: Cell<u64>,
}

fn slates() -> (Rc<dyn SlateDoor>, Rc<RefCell<Vec<Value>>>) {
    let log = Rc::new(RefCell::new(Vec::new()));
    (Rc::new(Slates { log: log.clone(), held: Cell::new(0) }), log)
}

impl Slates {
    fn note(&self, call: &str, args: Value) {
        self.log.borrow_mut().push(json!({ "call": call, "args": args }));
    }
}

fn payload(v: Value) -> Payload {
    match v {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

#[async_trait(?Send)]
impl SlateDoor for Slates {
    async fn get(&self, thread_id: &str) -> Result<Value, HostError> {
        self.note("get", json!({ "threadId": thread_id }));
        Ok(json!({ "version": 4 }))
    }
    async fn write(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError> {
        self.note("write", json!({ "params": params, "origin": support::script::caller_json(&origin) }));
        Ok(payload(json!({ "version": 5, "from": "write" })))
    }
    async fn state(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError> {
        self.note("state", json!({ "params": params, "origin": support::script::caller_json(&origin) }));
        Ok(payload(json!({ "from": "state" })))
    }
    async fn read(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError> {
        self.note("read", json!({ "params": params, "origin": support::script::caller_json(&origin) }));
        Ok(payload(json!({ "from": "read" })))
    }
    async fn catalog(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError> {
        self.note("catalog", json!({ "params": params, "origin": support::script::caller_json(&origin) }));
        Ok(payload(json!({ "from": "catalog" })))
    }
    async fn shown(&self, thread_id: &str) -> Result<(), HostError> {
        self.note("shown", json!({ "threadId": thread_id }));
        Ok(())
    }
    async fn event(&self, params: Payload) -> Result<Payload, HostError> {
        self.note("event", json!({ "params": params }));
        Ok(payload(json!({ "from": "event" })))
    }
    async fn approve(&self, thread_id: &str, key: &str, scope: &str) -> Result<(), HostError> {
        self.note("approve", json!({ "threadId": thread_id, "key": key, "scope": scope }));
        Ok(())
    }
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError> {
        self.note("cancel", json!({ "threadId": thread_id, "run": run }));
        Ok(())
    }
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError> {
        self.note("revoke", json!({ "threadId": thread_id, "key": key }));
        Ok(())
    }
    fn subscribe(&self, thread_id: &str, sources: Vec<String>) -> Box<dyn FnOnce()> {
        let n = self.held.get() + 1;
        self.held.set(n);
        self.note("subscribe", json!({ "n": n, "threadId": thread_id, "sources": sources }));
        let log = self.log.clone();
        Box::new(move || log.borrow_mut().push(json!({ "call": "release", "args": { "n": n } })))
    }
    async fn resolve(&self, thread_id: &str, paths: Vec<String>) -> Result<Payload, HostError> {
        self.note("resolve", json!({ "threadId": thread_id, "paths": paths }));
        Ok(payload(json!({ "from": "resolve" })))
    }
    async fn image(&self, thread_id: &str, src: &str, have: Option<String>) -> Result<Payload, HostError> {
        self.note("image", json!({ "threadId": thread_id, "src": src, "have": have }));
        Ok(payload(json!({ "from": "image" })))
    }
}

struct SlateRig {
    door: Rc<dyn SlateDoor>,
    log: Rc<RefCell<Vec<Value>>>,
    holds: Rc<RefCell<SlateHolds>>,
    detaches: Rc<RefCell<Vec<Box<dyn FnOnce()>>>>,
}

fn slate_rig() -> SlateRig {
    let (door, log) = slates();
    SlateRig { door, log, holds: Rc::new(RefCell::new(SlateHolds::default())), detaches: Rc::new(RefCell::new(Vec::new())) }
}

impl SlateRig {
    async fn ask(&self, op: &str, fields: Value) -> Result<Value, HostError> {
        let kept = self.detaches.clone();
        let detach = move |leave: Box<dyn FnOnce()>| kept.borrow_mut().push(leave);
        answer_slate(&self.door, &req(op, fields), here(), &self.holds, &detach).await.map(Value::Object)
    }
    fn calls(&self) -> Vec<Value> {
        self.log.borrow().clone()
    }
    fn releases(&self) -> Vec<u64> {
        self.calls().iter().filter(|c| c["call"] == "release").map(|c| c["args"]["n"].as_u64().unwrap()).collect()
    }
    fn close(&self) {
        let leaves: Vec<Box<dyn FnOnce()>> = self.detaches.borrow_mut().drain(..).collect();
        for leave in leaves {
            leave();
        }
    }
}

// H22. slates.test.ts
#[test]
fn slate_answers_split_params_and_origin() {
    block(async {
        let rig = slate_rig();
        assert_eq!(rig.ask("slates.get", json!({ "threadId": "t1" })).await.unwrap(), json!({ "slate": { "version": 4 } }));
        let wrote = rig.ask("slates.write", json!({ "threadId": "t1", "text": "hi", "origin": "paired" })).await.unwrap();
        assert_eq!(wrote, json!({ "version": 5, "from": "write" }));
        rig.ask("slates.state", json!({ "threadId": "t1", "start": ["run"] })).await.unwrap();
        rig.ask("slates.read", json!({ "threadId": "t1", "values": ["a"] })).await.unwrap();
        rig.ask("slates.catalog", json!({ "name": "runs" })).await.unwrap();
        assert_eq!(rig.ask("slates.shown", json!({ "threadId": "t1" })).await.unwrap(), json!({}));
        assert_eq!(rig.ask("slates.approve", json!({ "threadId": "t1", "key": "k", "scope": "once" })).await.unwrap(), json!({}));
        assert_eq!(rig.ask("slates.cancel", json!({ "threadId": "t1", "run": "r1" })).await.unwrap(), json!({}));
        assert_eq!(rig.ask("slates.revoke", json!({ "threadId": "t1", "key": "k" })).await.unwrap(), json!({}));
        let event = rig
            .ask("slates.event", json!({ "threadId": "t1", "version": 4, "piece": "p", "event": "press", "requestId": "q", "origin": "paired" }))
            .await
            .unwrap();
        assert_eq!(event, json!({ "from": "event" }));
        rig.ask("slates.resolve", json!({ "threadId": "t1", "paths": ["a.png"] })).await.unwrap();
        rig.ask("slates.image", json!({ "threadId": "t1", "src": "a.png" })).await.unwrap();
        rig.ask("slates.image", json!({ "threadId": "t1", "src": "a.png", "have": "h" })).await.unwrap();

        let calls = rig.calls();
        let at = |i: usize| calls[i].clone();
        assert_eq!(at(0), json!({ "call": "get", "args": { "threadId": "t1" } }));
        assert_eq!(at(1), json!({ "call": "write", "args": { "params": { "threadId": "t1", "text": "hi" }, "origin": "here" } }));
        assert_eq!(at(2), json!({ "call": "state", "args": { "params": { "threadId": "t1", "start": ["run"] }, "origin": "here" } }));
        assert_eq!(at(3)["args"]["params"], json!({ "threadId": "t1", "values": ["a"] }));
        assert_eq!(at(4)["args"], json!({ "params": { "name": "runs" }, "origin": "here" }));
        assert_eq!(at(5), json!({ "call": "shown", "args": { "threadId": "t1" } }));
        assert_eq!(at(6), json!({ "call": "approve", "args": { "threadId": "t1", "key": "k", "scope": "once" } }));
        assert_eq!(at(7), json!({ "call": "cancel", "args": { "threadId": "t1", "run": "r1" } }));
        assert_eq!(at(8), json!({ "call": "revoke", "args": { "threadId": "t1", "key": "k" } }));
        assert_eq!(
            at(9),
            json!({ "call": "event", "args": { "params": { "threadId": "t1", "version": 4, "piece": "p", "event": "press", "requestId": "q" } } }),
            "the event's params are the frame less op, id and origin, and carry no origin"
        );
        assert_eq!(at(10), json!({ "call": "resolve", "args": { "threadId": "t1", "paths": ["a.png"] } }));
        assert_eq!(at(11), json!({ "call": "image", "args": { "threadId": "t1", "src": "a.png", "have": null } }));
        assert_eq!(at(12)["args"]["have"], json!("h"));
    })
}

// H23. slates.test.ts
#[test]
fn slate_unsubscribe_lets_go_of_the_oldest_hold() {
    block(async {
        let rig = slate_rig();
        let sub = json!({ "threadId": "t1", "sources": ["a"] });
        assert_eq!(rig.ask("slates.subscribe", sub.clone()).await.unwrap(), json!({}));
        assert_eq!(rig.ask("slates.subscribe", sub.clone()).await.unwrap(), json!({}));
        assert_eq!(rig.holds.borrow().held.len(), 1);
        assert_eq!(rig.holds.borrow().held[&hold_key("t1", &["a".to_string()])].len(), 2);
        assert_eq!(rig.ask("slates.unsubscribe", sub.clone()).await.unwrap(), json!({}));
        assert_eq!(rig.releases(), vec![1], "the oldest hold goes first");
        assert_eq!(rig.holds.borrow().held.len(), 1, "one hold is still under the key");
        rig.ask("slates.unsubscribe", sub.clone()).await.unwrap();
        assert_eq!(rig.releases(), vec![1, 2]);
        assert!(rig.holds.borrow().held.is_empty(), "the key is forgotten when none is left");
        let before = rig.calls().len();
        assert_eq!(rig.ask("slates.unsubscribe", json!({ "threadId": "t9", "sources": ["z"] })).await.unwrap(), json!({}));
        assert_eq!(rig.calls().len(), before, "an unknown key does nothing");
    })
}

// H23.
#[test]
fn slate_subscribe_sources_in_any_order_share_a_key() {
    block(async {
        assert_eq!(hold_key("t1", &["b".to_string(), "a".to_string()]), "t1\u{0}a,b");
        assert_eq!(hold_key("t1", &["a".to_string(), "b".to_string()]), hold_key("t1", &["b".to_string(), "a".to_string()]));
        assert_ne!(hold_key("t1", &["a".to_string()]), hold_key("t2", &["a".to_string()]));
        let rig = slate_rig();
        rig.ask("slates.subscribe", json!({ "threadId": "t1", "sources": ["b", "a"] })).await.unwrap();
        rig.ask("slates.unsubscribe", json!({ "threadId": "t1", "sources": ["a", "b"] })).await.unwrap();
        assert_eq!(rig.releases(), vec![1]);
        let first = rig.calls().into_iter().find(|c| c["call"] == "subscribe").unwrap();
        assert_eq!(first["args"], json!({ "n": 1, "threadId": "t1", "sources": ["b", "a"] }), "the runtime is handed the sources as sent");
    })
}

// H23. A close runs every detach; a hold already let go is not let go twice.
#[test]
fn a_socket_close_lets_go_of_every_slate_hold() {
    block(async {
        let rig = slate_rig();
        let sub = json!({ "threadId": "t1", "sources": ["a"] });
        rig.ask("slates.subscribe", sub.clone()).await.unwrap();
        rig.ask("slates.subscribe", sub.clone()).await.unwrap();
        rig.ask("slates.subscribe", json!({ "threadId": "t2", "sources": ["a"] })).await.unwrap();
        rig.ask("slates.unsubscribe", sub.clone()).await.unwrap();
        assert_eq!(rig.releases(), vec![1]);
        assert_eq!(rig.detaches.borrow().len(), 3, "every hold is on the socket's detaches");
        rig.close();
        assert_eq!(rig.releases(), vec![1, 2, 3], "each hold is let go once, the first not again");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The broker answers (serve-broker.ts).

struct Broker {
    calls: Rc<RefCell<Vec<String>>>,
    status: Option<Value>,
    key: Option<Value>,
}

#[async_trait(?Send)]
impl BrokerDoor for Broker {
    async fn status(&self) -> Option<Result<BrokerStatus, HostError>> {
        self.calls.borrow_mut().push("status".into());
        self.status.clone().map(|v| Ok(from(v)))
    }
    async fn check_key(&self, provider: &str, key: &str) -> Option<Result<ProviderKeyWorks, HostError>> {
        self.calls.borrow_mut().push(format!("checkKey {provider} {key}"));
        self.key.clone().map(|v| Ok(from(v)))
    }
    fn cloud_sign_in(&self) -> Option<CloudSignIn> {
        None
    }
}

fn broker(status: Option<Value>, key: Option<Value>) -> (Rc<dyn BrokerDoor>, Rc<RefCell<Vec<String>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    (Rc::new(Broker { calls: calls.clone(), status, key }), calls)
}

// H24. serve.ts:875
#[test]
fn broker_status_unserved() {
    block(async {
        let status = req("broker.status", json!({}));
        let none = answer_broker(None, &status, true).await.unwrap_err();
        assert_eq!(none.message, BROKER_UNSERVED);
        let (door, calls) = broker(None, None);
        for own in [true, false] {
            let e = answer_broker(Some(&door), &status, own).await.unwrap_err();
            assert_eq!(e.message, BROKER_UNSERVED, "a door with no status road refuses on any road");
            assert_eq!(e.kind, None);
        }
        assert_eq!(*calls.borrow(), vec!["status".to_string(), "status".to_string()]);
        let (served, _) = broker(Some(json!({ "github": "zingzy" })), None);
        let said = answer_broker(Some(&served), &status, false).await.unwrap();
        assert_eq!(Value::Object(said), json!({ "github": "zingzy" }), "the door's answer is spread");
    })
}

// H24. serve.ts:875
#[test]
fn check_key_off_the_own_road_is_ticket_before_the_door() {
    block(async {
        let check = req("providers.checkKey", json!({ "provider": "box", "key": "k-1234" }));
        let works = json!({ "works": true, "account": "acme", "plan": "pro", "running": 1, "limit": 4 });
        let (door, calls) = broker(None, Some(works.clone()));
        let off = answer_broker(Some(&door), &check, false).await.unwrap_err();
        assert_eq!(off.message, PLACES_TICKET_REFUSAL);
        assert_eq!(off.kind, Some(Kind::Ticket));
        assert!(calls.borrow().is_empty(), "the door is not read for a socket off the own road");
        let none = answer_broker(None, &check, false).await.unwrap_err();
        assert_eq!(none.kind, Some(Kind::Ticket), "the road is refused before the missing door is");
        let unserved = answer_broker(None, &check, true).await.unwrap_err();
        assert_eq!(unserved.message, BROKER_UNSERVED);
        let (no_key, _) = broker(Some(json!({})), None);
        assert_eq!(answer_broker(Some(&no_key), &check, true).await.unwrap_err().message, BROKER_UNSERVED);
        let said = answer_broker(Some(&door), &check, true).await.unwrap();
        assert_eq!(Value::Object(said), works);
        assert_eq!(*calls.borrow(), vec!["checkKey box k-1234".to_string()]);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// threadPlaces (serve-projects.ts:35-44), spawn-guard.test.ts

fn scope(workspace_id: &str) -> Caller {
    Caller::Scoped {
        origin: WorkspaceOrigin::Here,
        by: from(json!({ "kind": "thread", "threadId": "th_1", "workspaceId": workspace_id, "rootThreadId": "th_0" })),
    }
}

fn workspace(id: &str, kind: &str) -> wsp_wire::views::workspace_more::WorkspaceView {
    from(json!({
        "id": id, "name": id, "machineId": "m_1", "phase": "running", "kind": kind, "golden": "default", "createdAt": "2026-10-25T00:00:00.000Z",
        "project": { "id": "pr_1", "name": "app", "path": "/app", "computer": "pl_1" },
    }))
}

fn project(id: &str, computer: &str) -> wsp_wire::views::workspace::ProjectView {
    from(json!({
        "id": id, "name": id, "computer": computer, "source": { "kind": "folder", "path": "/app" }, "path": "/app", "remote": "",
        "defaultBranch": "main", "memoryKey": "k", "memoryDir": "/m", "createdAt": "2026-10-25T00:00:00.000Z",
    }))
}

fn place(id: &str, kind: &str) -> Value {
    json!({ "id": id, "kind": kind, "name": id, "default": false })
}

type Rows = Vec<wsp_wire::views::place_more::PlaceView>;

fn script_rows(rt: &FakeRuntime, rows: Vec<Value>) {
    let rows: Rows = rows.into_iter().map(from).collect();
    rt.script.once::<Result<Rows, HostError>>("places.list", Ok(rows));
}

fn ids(rows: &Rows) -> Vec<String> {
    rows.iter().map(|r| r.id.clone()).collect()
}

// H20. spawn-guard.test.ts
#[test]
fn thread_places_on_a_machine_are_the_providers_alone() {
    block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "cloud")));
        script_rows(&rt, vec![place("pl_1", "computer"), place("box", "provider")]);
        let rows = thread_places(&*rt, Some(scope("ws_1")), 5).await.unwrap();
        assert_eq!(ids(&rows), vec!["box".to_string()]);
        assert!(rt.script.calls_to("projects.list").is_empty(), "a machine's thread reaches no project's computer");
        assert_eq!(args_of(&rt, "workspaces.get", 0)["id"], json!("ws_1"));
        assert_eq!(args_of(&rt, "places.list", 0)["now"], json!(5));
    })
}

// H20. spawn-guard.test.ts
#[test]
fn thread_places_in_a_folder_reach_its_projects_computers() {
    block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "local")));
        rt.script.once::<Result<Vec<wsp_wire::views::workspace::ProjectView>, HostError>>("projects.list", Ok(vec![project("pr_1", "pl_1")]));
        script_rows(&rt, vec![place("pl_1", "computer"), place("pl_2", "computer"), place("box", "provider")]);
        let rows = thread_places(&*rt, Some(scope("ws_1")), 5).await.unwrap();
        assert_eq!(ids(&rows), vec!["pl_1".to_string(), "box".to_string()], "the computer holding a project, and every provider");
        assert_eq!(rt.script.names(), vec!["workspaces.get", "projects.list", "places.list"]);
    })
}

// H20. A failed read of the workspace is no workspace; no scope is no workspace; no place door is no rows.
#[test]
fn thread_places_without_a_workspace_or_a_door_read_the_providers_or_nothing() {
    block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorkspaceView, HostError>>("workspaces.get", Err(HostError::new("no workspace ws_9")));
        script_rows(&rt, vec![place("pl_1", "computer"), place("box", "provider")]);
        let rows = thread_places(&*rt, Some(scope("ws_9")), 5).await.expect("a failed workspace read is no workspace, never a failure");
        assert_eq!(ids(&rows), vec!["box".to_string()]);
        assert!(rt.script.calls_to("projects.list").is_empty());

        let rt = FakeRuntime::with_places();
        script_rows(&rt, vec![place("pl_1", "computer"), place("box", "provider")]);
        let rows = thread_places(&*rt, here(), 5).await.unwrap();
        assert_eq!(ids(&rows), vec!["box".to_string()]);
        assert!(rt.script.calls_to("workspaces.get").is_empty(), "no thread names no workspace to read");

        let bare = FakeRuntime::new();
        assert!(thread_places(&*bare, None, 5).await.unwrap().is_empty());
        assert!(bare.script.calls_to("places.list").is_empty());
    })
}

// H20. spawn-guard.test.ts
#[test]
fn thread_places_drop_the_road_and_label_providers() {
    block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "local")));
        rt.script.once::<Result<Vec<wsp_wire::views::workspace::ProjectView>, HostError>>("projects.list", Ok(vec![project("pr_1", "pl_1")]));
        let mut computer = place("pl_1", "computer");
        computer["road"] = json!({ "ssh": "me@box" });
        let mut provider = place("box", "provider");
        provider["road"] = json!({ "from": "relay" });
        script_rows(&rt, vec![computer, provider]);
        let rows = thread_places(&*rt, Some(scope("ws_1")), 5).await.unwrap();
        let said = place_said(&PlaceRef { id: "box".into(), name: "box".into(), kind: Some("provider".into()) });
        let shown: Vec<Value> = rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect();
        assert_eq!(shown[0], json!({ "id": "pl_1", "kind": "computer", "name": "pl_1", "default": false }), "a computer's row loses its road and gains no label");
        assert_eq!(shown[1], json!({ "id": "box", "kind": "provider", "name": "box", "label": said, "default": false }));
        assert!(shown[1].get("road").is_none());
    })
}

// ---------------------------------------------------------------------------------------------------------------
// answerProject, answerProcesses, answerConversations: only the fields sent ride.

// H21. projects.test.ts
#[test]
fn project_answers_pass_only_the_fields_sent() {
    block(async {
        let rt = FakeRuntime::new();
        let view = project("pr_1", "pl_1");
        rt.script.once::<Result<ProjectAdded, HostError>>("projects.add", Ok(ProjectAdded { project: view.clone(), notice: Some("kept".into()) }));
        let added = answer_project(rt.projects(), &req("projects.add", json!({ "source": "/app" })), here()).await.unwrap();
        assert_eq!(Value::Object(added).to_string(), json!({ "project": serde_json::to_value(&view).unwrap(), "notice": "kept" }).to_string());
        rt.script.once::<Result<ProjectAdded, HostError>>("projects.add", Ok(ProjectAdded { project: view.clone(), notice: None }));
        let added = answer_project(rt.projects(), &req("projects.add", json!({ "source": "/app", "on": "pl_1", "name": "n", "base": "main", "into": "/x" })), here()).await.unwrap();
        assert_eq!(Value::Object(added), json!({ "project": serde_json::to_value(&view).unwrap() }), "no notice, no key");
        let ask = |n: usize| args_of(&rt, "projects.add", n)["ask"].clone();
        assert_eq!(ask(0), json!(format!("{:?}", wsp_core::types::api::ProjectAddAsk { source: "/app".into(), ..Default::default() })));
        let second = format!("{:?}", wsp_core::types::api::ProjectAddAsk {
            source: "/app".into(),
            on: Some("pl_1".into()),
            name: Some("n".into()),
            base: Some("main".into()),
            into: Some("/x".into()),
            seed: None,
        });
        assert_eq!(ask(1), json!(second));
        assert_eq!(args_of(&rt, "projects.add", 0)["origin"], json!("here"));

        rt.script.once::<Result<_, HostError>>("projects.seed", Err::<wsp_wire::views::workspace::ProjectSeed, _>(HostError::new("stop")));
        rt.script.once::<Result<_, HostError>>("projects.seed", Err::<wsp_wire::views::workspace::ProjectSeed, _>(HostError::new("stop")));
        answer_project(rt.projects(), &req("projects.seed", json!({ "project": "pr_1" })), here()).await.unwrap_err();
        answer_project(rt.projects(), &req("projects.seed", json!({ "project": "pr_1", "keep": ["a"], "cut": ["b"], "none": true, "save": false })), here()).await.unwrap_err();
        assert_eq!(args_of(&rt, "projects.seed", 0)["reference"], json!("pr_1"));
        assert_eq!(args_of(&rt, "projects.seed", 0)["ask"], json!(format!("{:?}", SeedAsk::default())));
        let second = SeedAsk { keep: Some(vec!["a".into()]), cut: Some(vec!["b".into()]), none: Some(true), save: None };
        assert_eq!(args_of(&rt, "projects.seed", 1)["ask"], json!(format!("{second:?}")), "none and save ride only when true");

        rt.script.once::<Result<Vec<wsp_wire::views::workspace::ProjectView>, HostError>>("projects.list", Ok(vec![]));
        let listed = answer_project(rt.projects(), &req("projects.list", json!({})), here()).await.unwrap();
        assert_eq!(Value::Object(listed), json!({ "projects": [] }));

        rt.script.once::<Result<_, HostError>>("projects.defaults", Ok(indexmap::IndexMap::<String, wsp_wire::types::thread_defaults::ThreadDefaults>::new()));
        let defaults = answer_project(rt.projects(), &req("projects.defaults", json!({})), here()).await.unwrap();
        assert_eq!(Value::Object(defaults), json!({ "defaults": {} }));

        rt.script.once::<Result<_, HostError>>("projects.resolve", Ok(view.clone()));
        let resolved = answer_project(rt.projects(), &req("projects.resolve", json!({ "ref": "app" })), here()).await.unwrap();
        assert_eq!(Value::Object(resolved), json!({ "project": serde_json::to_value(&view).unwrap() }));
        assert_eq!(args_of(&rt, "projects.resolve", 0)["reference"], json!("app"));

        rt.script.once::<Result<_, HostError>>("projects.branch", Ok(wsp_wire::views::workspace_more::ProjectBranch { branch: None, folder: true }));
        let branch = answer_project(rt.projects(), &req("projects.branch", json!({ "projectId": "pr_1" })), here()).await.unwrap();
        assert_eq!(Value::Object(branch), json!({ "branch": null, "folder": true }));

        rt.script.once::<Result<_, HostError>>("projects.remove", Ok(ProjectRemoved { said: "removed app".into(), unsaved: None }));
        rt.script.once::<Result<_, HostError>>("projects.remove", Ok(ProjectRemoved { said: "removed app".into(), unsaved: Some("3 files".into()) }));
        let plain = answer_project(rt.projects(), &req("projects.remove", json!({ "projectId": "pr_1", "force": false, "check": false })), here()).await.unwrap();
        assert!(plain.get("unsaved").is_none());
        answer_project(rt.projects(), &req("projects.remove", json!({ "projectId": "pr_1", "force": true, "check": true })), here()).await.unwrap();
        let first = args_of(&rt, "projects.remove", 0);
        assert_eq!((first["id"].clone(), first["force"].clone(), first["check"].clone()), (json!("pr_1"), Value::Null, Value::Null), "false is not sent");
        let second = args_of(&rt, "projects.remove", 1);
        assert_eq!((second["force"].clone(), second["check"].clone()), (json!(true), json!(true)));

        rt.script.once::<Result<_, HostError>>("projects.icons", Ok(indexmap::IndexMap::<String, Option<String>>::from([("h1".to_string(), None::<String>)])));
        let icons = answer_project(rt.projects(), &req("projects.icons", json!({ "hashes": ["h1"] })), here()).await.unwrap();
        assert_eq!(Value::Object(icons), json!({ "icons": { "h1": null } }));
    })
}

// H25, H26. serve-refusal.test.ts:50
#[test]
fn processes_and_conversations_pass_only_the_fields_sent() {
    block(async {
        let rt = FakeRuntime::new();
        let door: &dyn ServedRuntime = &*rt;

        let view = json!({ "computer": "here", "at": 1, "threads": [], "wsp": [], "unowned": [] });
        rt.script.once::<Result<wsp_wire::views::activity::ActivityView, HostError>>("activity.read", Ok(from(view.clone())));
        let read = answer_processes(door, &req("activity.read", json!({ "thread": "t1" })), here()).await.unwrap();
        assert_eq!(Value::Object(read), json!({ "activity": view }));
        let a = args_of(&rt, "activity.read", 0);
        assert_eq!((a["computer"].clone(), a["thread"].clone()), (Value::Null, json!("t1")), "computer is passed only when present");

        rt.script.once::<Result<Payload, HostError>>("activity.end", Ok(payload(json!({ "ended": [4200] }))));
        let ended = answer_processes(door, &req("activity.end", json!({ "computer": "pl_1", "pid": 4200, "startedAt": 7 })), here()).await.unwrap();
        assert_eq!(Value::Object(ended), json!({ "ended": [4200] }));
        let a = args_of(&rt, "activity.end", 0);
        assert_eq!((a["computer"].clone(), a["pid"].clone(), a["startedAt"].clone()), (json!("pl_1"), json!(4200), json!(7.0)));

        let settled = json!({ "settled": [], "left": [] });
        rt.script.once::<Result<wsp_wire::wire::runtime::SessionSettleResult, HostError>>("sessions.settle", Ok(from(settled.clone())));
        rt.script.once::<Result<wsp_wire::wire::runtime::SessionSettleResult, HostError>>("sessions.settle", Ok(from(settled.clone())));
        let plain = answer_processes(door, &req("sessions.settle", json!({ "threadIds": ["t1"], "finished": false })), here()).await.unwrap();
        assert_eq!(Value::Object(plain), settled);
        answer_processes(door, &req("sessions.settle", json!({ "threadIds": ["t1", "t2"], "finished": true, "processes": "stop" })), here()).await.unwrap();
        let first = args_of(&rt, "sessions.settle", 0);
        assert_eq!(first["threadIds"], json!(["t1"]));
        assert_eq!(first["ask"], json!(format!("{:?}", SettleAsk::default())), "finished false is not sent");
        let second = args_of(&rt, "sessions.settle", 1);
        assert_eq!(second["threadIds"], json!(["t1", "t2"]));
        let ask = SettleAsk { finished: Some(true), processes: Some(wsp_wire::views::activity::ProcessesAsk::Stop) };
        assert_eq!(second["ask"], json!(format!("{ask:?}")));

        rt.script.once::<Result<_, HostError>>("sessions.delete", Err::<wsp_core::types::api::ThreadDeleted, _>(HostError::new("stop")));
        let e = answer_processes(door, &req("sessions.delete", json!({ "threadId": "t1", "check": true })), here()).await.unwrap_err();
        assert_eq!(e.message, "stop");
        let delete = args_of(&rt, "sessions.delete", 0);
        assert_eq!(delete["threadId"], json!("t1"));
        let ask = wsp_core::types::api::DeleteThreadAsk { processes: None, check: Some(true) };
        assert_eq!(delete["ask"], json!(format!("{ask:?}")));

        rt.script.once::<Result<wsp_wire::types::conversations::ConversationsAnswer, HostError>>("conversations.list", Ok(from(json!({ "rows": [], "held": [] }))));
        let listed = answer_conversations(rt.conversations(), &req("conversations.list", json!({ "project": "p" })), here()).await.unwrap();
        assert_eq!(Value::Object(listed), json!({ "rows": [], "held": [] }));
        let a = args_of(&rt, "conversations.list", 0);
        assert_eq!((a["project"].clone(), a["agent"].clone()), (json!("p"), Value::Null));

        rt.script.once::<Result<_, HostError>>("conversations.read", Err::<wsp_wire::types::conversations::ConversationRead, _>(HostError::new("stop")));
        rt.script.once::<Result<_, HostError>>("conversations.read", Err::<wsp_wire::types::conversations::ConversationRead, _>(HostError::new("stop")));
        answer_conversations(rt.conversations(), &req("conversations.read", json!({ "agent": "claude", "conversation": "c1", "preview": false })), here()).await.unwrap_err();
        answer_conversations(rt.conversations(), &req("conversations.read", json!({ "project": "p", "agent": "claude", "conversation": "c1", "preview": true })), here()).await.unwrap_err();
        let plain = ConversationAsk { project: None, agent: "claude".into(), id: "c1".into(), preview: None };
        assert_eq!(args_of(&rt, "conversations.read", 0)["ask"], json!(format!("{plain:?}")), "id is the frame's conversation; preview false is not sent");
        let full = ConversationAsk { project: Some("p".into()), agent: "claude".into(), id: "c1".into(), preview: Some(true) };
        assert_eq!(args_of(&rt, "conversations.read", 1)["ask"], json!(format!("{full:?}")));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// serveStart (serve-start.ts)

struct Roads {
    log: Rc<RefCell<Vec<Value>>>,
    folder: StartFolder,
    answers: Vec<SessionStartResult>,
}

fn roads(folder: StartFolder, answers: Vec<SessionStartResult>) -> (Rc<Roads>, Rc<RefCell<Vec<Value>>>) {
    let log = Rc::new(RefCell::new(Vec::new()));
    (Rc::new(Roads { log: log.clone(), folder, answers }), log)
}

#[async_trait(?Send)]
impl StartRoads for Roads {
    async fn start_at(&self, _workspaces: &dyn WorkspacesApi, msg: &RuntimeRequest, origin: Option<Caller>, opens: bool) -> Result<StartFolder, HostError> {
        self.log.borrow_mut().push(json!({ "call": "startAt", "opens": opens, "op": msg.op.name(), "origin": support::script::caller_json(&origin) }));
        Ok(self.folder.clone())
    }
    async fn answered_start(&self, answer_held: bool, start: StartFn, answer: AnswerFn) -> Result<(), HostError> {
        self.log.borrow_mut().push(json!({ "call": "answeredStart", "answerHeld": answer_held }));
        for a in &self.answers {
            answer(a.clone());
        }
        start(None).await?;
        Ok(())
    }
}

fn folder() -> StartFolder {
    StartFolder { workspace_id: "ws_7".into(), cwd: Some("/work".into()), setup: Some("make".into()) }
}

fn quiet_reply() -> AnswerFn {
    Rc::new(|_| {})
}

fn start_msg(fields: Value) -> RuntimeRequest {
    let mut all = json!({ "prompt": "go" });
    for (k, v) in fields.as_object().unwrap() {
        all[k] = v.clone();
    }
    req("sessions.start", all)
}

fn stopped<T>(rt: &FakeRuntime, name: &str, n: usize)
where
    T: 'static,
{
    for _ in 0..n {
        rt.script.once::<Result<T, HostError>>(name, Err(HostError::new("stop")));
    }
}

type Handle = Rc<dyn wsp_core::types::wiring::SessionHandle>;

// H10, H11. sub-spawn.test.ts
#[test]
fn start_refusals_in_order() {
    block(async {
        let rt = FakeRuntime::new();
        let (r, log) = roads(folder(), vec![]);
        let run = |fields: Value| {
            let rt = rt.clone();
            let r = r.clone();
            async move { serve_start(&*rt, &*r, &start_msg(fields), here(), quiet_reply()).await.unwrap_err() }
        };
        let branch = run(json!({ "workspaceId": "ws_1", "branch": "b" })).await;
        assert_eq!(branch.message, BRANCH_ON_A_THREAD_LINE);
        assert_eq!(branch.kind, Some(Kind::Usage));
        let branch_first = run(json!({ "workspaceId": "ws_1", "branch": "b", "fork": { "threadId": "t1" } })).await;
        assert_eq!(branch_first.message, BRANCH_ON_A_THREAD_LINE, "the branch is refused before the fork is looked at");

        let beside = usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX);
        for beside_field in [json!({ "workspaceId": "ws_1" }), json!({ "project": "p" }), json!({ "cwd": "/x" }), json!({ "thread": "t2" }), json!({ "replaces": "t3" })] {
            let mut fields = json!({ "fork": { "threadId": "t1" } });
            fields[beside_field.as_object().unwrap().keys().next().unwrap().as_str()] = beside_field.as_object().unwrap().values().next().unwrap().clone();
            assert_eq!(run(fields).await, beside, "a fork beside {beside_field}");
        }
        let resume = run(json!({ "fork": { "threadId": "t1" }, "resume": { "id": "c1" } })).await;
        assert_eq!(resume, usage_refusal(FORK_RESUME_LINE, FORK_RESUME_FIX));
        let both = run(json!({ "fork": { "threadId": "t1" }, "resume": { "id": "c1" }, "project": "p" })).await;
        assert_eq!(both, beside, "beside is checked before resume");

        assert!(rt.script.calls().is_empty(), "nothing is read before a refusal");
        assert!(log.borrow().is_empty());
    })
}

// H12. sub-spawn.test.ts
#[test]
fn a_fork_reads_nothing_else() {
    block(async {
        let rt = FakeRuntime::new();
        stopped::<Handle>(&rt, "sessions.fork", 1);
        let (r, log) = roads(folder(), vec![]);
        let msg = start_msg(json!({ "fork": { "threadId": "t1" }, "branch": "feature", "model": "opus", "answerHeld": true, "title": "T" }));
        let e = serve_start(&*rt, &*r, &msg, here(), quiet_reply()).await.unwrap_err();
        assert_eq!(e.message, "stop", "the fork's own answer comes back");
        assert_eq!(rt.script.names(), vec!["sessions.fork"], "no conversation placed, no folder read, no start");
        assert_eq!(*log.borrow(), vec![json!({ "call": "answeredStart", "answerHeld": true })]);
        let a = args_of(&rt, "sessions.fork", 0);
        assert_eq!(a["origin"], json!("here"));
        assert_eq!(a["opts"]["fork"], json!({ "threadId": "t1" }));
        assert_eq!(a["opts"]["branch"], json!("feature"));
        let start = &a["opts"]["start"];
        assert_eq!((start["prompt"].clone(), start["model"].clone(), start["title"].clone()), (json!("go"), json!("opus"), json!("T")));
        assert_eq!((start["cwd"].clone(), start["setup"].clone(), start["thread"].clone()), (Value::Null, Value::Null, Value::Null), "a fork has no folder of its own");
        assert_eq!(start["outside"], Value::Null);
    })
}

// H13. folder-threads.test.ts
#[test]
fn a_resume_is_read_before_the_folder() {
    block(async {
        let rt = FakeRuntime::new();
        let outside = OutsideOpening { harness: "claude".into(), id: "c9".into(), copy: true, project: "app".into(), cwd: Some("/w".into()), title: "old".into(), earlier: vec![] };
        rt.script.once::<Result<Placed, HostError>>("conversations.placed", Ok(Placed { workspace_id: "ws_resumed".into(), cwd: Some("/w".into()), outside }));
        stopped::<Handle>(&rt, "sessions.start", 1);
        let (r, log) = roads(folder(), vec![]);
        let msg = start_msg(json!({ "resume": { "id": "c9", "copy": true }, "project": "app", "harness": "claude" }));
        serve_start(&*rt, &*r, &msg, here(), quiet_reply()).await.unwrap_err();
        assert_eq!(rt.script.names(), vec!["conversations.placed", "sessions.start"]);
        assert!(log.borrow().iter().all(|c| c["call"] != "startAt"), "where the conversation answers it is the folder");
        let placed = args_of(&rt, "conversations.placed", 0);
        let ask = PlacedStart { workspace_id: None, project: Some("app".into()), harness: Some("claude".into()), thread: None, branch: None, cwd: None };
        assert_eq!(placed["start"], json!(format!("{ask:?}")));
        assert_eq!(placed["resume"], json!({ "id": "c9", "copy": true }));
        let start = args_of(&rt, "sessions.start", 0);
        assert_eq!(start["workspaceId"], json!("ws_resumed"));
        assert_eq!(start["opts"]["cwd"], json!("/w"));
        assert!(start["opts"]["outside"].as_str().unwrap().contains("c9"), "the outside conversation rides the start");
    })
}

// H13. folder-threads.test.ts
#[test]
fn opens_only_without_a_thread() {
    block(async {
        let rt = FakeRuntime::new();
        stopped::<Handle>(&rt, "sessions.start", 2);
        let (r, log) = roads(folder(), vec![]);
        serve_start(&*rt, &*r, &start_msg(json!({ "project": "app" })), here(), quiet_reply()).await.unwrap_err();
        serve_start(&*rt, &*r, &start_msg(json!({ "thread": "t1", "workspaceId": "ws_1" })), here(), quiet_reply()).await.unwrap_err();
        let reads: Vec<Value> = log.borrow().iter().filter(|c| c["call"] == "startAt").cloned().collect();
        assert_eq!(reads.len(), 2);
        assert_eq!((reads[0]["opens"].clone(), reads[1]["opens"].clone()), (json!(true), json!(false)));
        assert_eq!(reads[0]["op"], json!("sessions.start"));
        assert!(rt.script.calls_to("conversations.placed").is_empty(), "no resume reads no conversation");
        let first = args_of(&rt, "sessions.start", 0);
        assert_eq!(first["workspaceId"], json!("ws_7"), "the folder's record");
        assert_eq!((first["opts"]["cwd"].clone(), first["opts"]["setup"].clone()), (json!("/work"), json!("make")));
        assert_eq!(first["opts"]["outside"], Value::Null);
        assert_eq!(first["origin"], json!("here"));
    })
}

// H14. Each answer the start road makes reaches the reply, in turn, whatever the road's own words are.
#[test]
fn answers_reach_the_reply_each_in_turn() {
    block(async {
        let rt = FakeRuntime::new();
        stopped::<Handle>(&rt, "sessions.start", 1);
        let held = json!({ "session": { "id": "s1", "workspaceId": "ws_7", "harness": "claude", "status": "running" }, "outcome": "held", "turnId": "tu_1" });
        let started = json!({ "session": { "id": "s1", "workspaceId": "ws_7", "harness": "claude", "status": "running" }, "outcome": "started", "turnId": "tu_1" });
        let (r, _) = roads(folder(), vec![from(held), from(started)]);
        let seen: Rc<RefCell<Vec<Value>>> = Rc::new(RefCell::new(Vec::new()));
        let kept = seen.clone();
        let reply: AnswerFn = Rc::new(move |a| kept.borrow_mut().push(serde_json::to_value(&a).unwrap()));
        serve_start(&*rt, &*r, &start_msg(json!({ "project": "app", "answerHeld": true })), here(), reply).await.unwrap_err();
        let outcomes: Vec<Value> = seen.borrow().iter().map(|a| a["outcome"].clone()).collect();
        assert_eq!(outcomes, vec![json!("held"), json!("started")]);
    })
}

// H15. The fields a frame names ride by hand; nothing else does.
#[test]
fn start_options_copy_only_the_named_fields_in_order() {
    let msg = start_msg(json!({
        "workspaceId": "ws_1", "project": "app", "branch": "b", "harness": "codex", "thread": "t1", "cwd": "/ignored", "model": "m", "effort": "high",
        "permissionMode": "plan", "access": "full", "contextWindow": "1m", "fast": true, "startedBy": "cli", "requestId": "rq", "answerHeld": true,
        "followed": true, "attempt": "a1", "notify": ["x"], "turnToken": "tt", "title": "T", "replaces": "t0",
    }));
    let at = StartFolder { workspace_id: "ws_7".into(), cwd: Some("/work".into()), setup: Some("make".into()) };
    let held: wsp_serve::serve_start::OnHeld = Rc::new(|_| {});
    let o = start_options_of(&msg, &at, None, Some(held));
    assert_eq!(o.prompt, "go");
    assert!(o.on_held.is_some());
    assert_eq!(o.followed, Some(true));
    assert_eq!(o.harness.as_deref(), Some("codex"));
    assert_eq!(o.thread.as_deref(), Some("t1"));
    assert_eq!(o.cwd.as_deref(), Some("/work"), "the folder's cwd, never the frame's");
    assert_eq!(o.setup.as_deref(), Some("make"));
    assert_eq!((o.model.as_deref(), o.effort.as_deref(), o.permission_mode.as_deref(), o.context_window.as_deref()), (Some("m"), Some("high"), Some("plan"), Some("1m")));
    assert_eq!(o.access, Some(AccessChoice::Full));
    assert_eq!(o.fast, Some(true));
    assert_eq!(o.started_by, Some(SessionOrigin::Cli));
    assert_eq!((o.request_id.as_deref(), o.attempt.as_deref(), o.turn_token.as_deref()), (Some("rq"), Some("a1"), Some("tt")));
    assert_eq!(o.notify, Some(vec!["x".to_string()]));
    assert_eq!((o.title.as_deref(), o.replaces.as_deref()), (Some("T"), Some("t0")));
    assert!(o.r#as.is_none() && o.mcp_servers.is_none() && o.behind.is_none() && o.after_limit.is_none() && o.fork.is_none());
    assert!(!o.via_slate && !o.wakes_lead && !o.owed && !o.asks_until_stopped, "what the frame cannot name stays off");

    let bare = start_options_of(&start_msg(json!({ "followed": false })), &StartFolder::default(), None, None);
    assert_eq!(bare.prompt, "go");
    assert!(bare.followed.is_none(), "followed rides only when true");
    assert!(bare.on_held.is_none() && bare.harness.is_none() && bare.cwd.is_none() && bare.setup.is_none() && bare.outside.is_none());
    assert!(bare.model.is_none() && bare.access.is_none() && bare.fast.is_none() && bare.notify.is_none() && bare.attachments.is_none());
}
