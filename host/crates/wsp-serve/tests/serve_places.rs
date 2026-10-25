// SPDX-License-Identifier: AGPL-3.0-only
//! serve_places: identity re-asks, the person's computers, recipes, account, devices and tickets. Spec:
//! spec/serve_places.md. Each case is the TS case ported at this crate's seam: the fake runtime is scripted with what
//! the area would have answered, and the test reads what the socket sent and what the runtime was asked.
mod support;
use support::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use support::fake_places::dbg;
use wsp_core::devices::{DeviceDoor, HeldDevice, IssuedCode};
use wsp_core::types::wiring::{RecipeShelf, ResolvedRecipe, ShelvedRecipe};
use wsp_machines::places::types::{AddAsk, PlaceAdded, PlaceFor, PlaceSetUp, RemoveAsk, SetUpAsk, NO_PLACE_DOOR};
use wsp_serve::serve::door::Served;
use wsp_serve::serve::socket::Socket;
use wsp_serve::serve::{AccountDoor, OpenedDoor, PlaceDoctor, PlaceDoorControl, Released, ServeOptions, Unlisten};
use wsp_wire::types::recipe_file::{RecipeFile, NO_RECIPE, NO_RECIPES, RECIPES_TICKET_REFUSAL};
use wsp_wire::views::place::DoctorLineEvent;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::wire::devices::{
    join_token, AccountView, ACCOUNT_TICKET_REFUSAL, ACCOUNT_UNSERVED, DEVICES_TICKET_REFUSAL, DEVICE_AUTH_REFUSAL, PAIR_CODE_REFUSAL,
    PAIR_ISSUE_REFUSAL,
};
use wsp_wire::wire::machine_link::{is_joined_computer, no_such_place_refusal};
use wsp_wire::wire::place_link::{
    MINT_JOIN_REFUSAL, PLACES_TICKET_REFUSAL, PLACE_DOOR_REFUSAL, PLACE_DOOR_UNSERVED, PLACE_UNKNOWN_REFUSAL, SSH_HOSTS_REFUSAL,
};
use wsp_words::exit::HostError;
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::recipe_file::{no_picks_refusal, recipe_from_here_refusal, recipe_summary};
use wsp_words::words::computer::RELAY_TICKET_REFUSAL;
use wsp_words::words::places::join_roads;
use wsp_words::words::units::{doctor_row_refusal, doctor_running_line, DOCTOR_UNSERVED};
use wsp_words::words::computer::UNAUTHORIZED;

// ---- helpers -------------------------------------------------------------------------------------------------

fn de<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).expect("the test's value parses as its type")
}

fn err(message: &str) -> HostError {
    HostError::new(message)
}

fn row(id: &str, kind: &str, name: &str) -> PlaceView {
    de(json!({ "id": id, "kind": kind, "name": name, "default": false }))
}

fn table(p: &PlaceView) -> String {
    let v = serde_json::to_value(p).unwrap();
    wsp_words::place_name::table_name(&PlaceRef { id: p.id.clone(), name: p.name.clone(), kind: v["kind"].as_str().map(str::to_string) })
}

fn file(name: &str) -> RecipeFile {
    de(json!({ "name": name }))
}

fn shelved(slug: &str, name: &str) -> ShelvedRecipe {
    de(json!({ "slug": slug, "file": { "name": name }, "savedAt": "2026-01-02T00:00:00.000Z" }))
}

/// A refusal as the socket writes it: `id`, `ok`, `error`, then `kind` where the case has one.
fn refused(id: u64, error: &str, kind: Option<&str>) -> String {
    let mut v = json!({ "id": id, "ok": false, "error": error });
    if let Some(k) = kind {
        v["kind"] = json!(k);
    }
    v.to_string()
}

/// `{id, ok: true, ...fields}`: an area's answer spread after `ok`, in the area's own order.
fn spread(id: u64, fields: &Value) -> String {
    let mut v = json!({ "id": id, "ok": true });
    for (k, f) in fields.as_object().expect("an object") {
        v[k] = f.clone();
    }
    v.to_string()
}

fn base(rt: &Rc<FakeRuntime>) -> ServeOptions {
    ServeOptions { devices: Some(rt.devices.clone() as Rc<dyn DeviceDoor>), now: Some(Rc::new(|| 1_000_000)), pair_ttl_ms: Some(600_000), ..opts() }
}

fn held(id: &str, scope: Option<Value>) -> HeldDevice {
    let mut view = json!({ "id": id, "name": "laptop", "createdAt": "2026-01-01T00:00:00.000Z", "lastSeenAt": "2026-01-01T00:00:00.000Z" });
    if let Some(s) = scope {
        view["scope"] = s;
    }
    HeldDevice { view: de(view), road: None }
}

fn thread_scope() -> Value {
    json!({ "kind": "thread", "threadId": "t_1", "workspaceId": "ws_1", "rootThreadId": "t_1" })
}

/// The device door answers each token with its device, and nobody for any other.
fn script_devices(rt: &Rc<FakeRuntime>, tokens: Vec<(&'static str, HeldDevice)>) {
    rt.script.on("devices.match", move |call| -> Result<Option<HeldDevice>, HostError> {
        let token = call.args["token"].as_str().unwrap_or_default();
        Ok(tokens.iter().find(|(t, _)| *t == token).map(|(_, d)| d.clone()))
    });
}

async fn device_socket(served: &Rc<Served>, token: &str) -> (Rc<Socket>, Rc<support::RecordingSink>) {
    let (socket, sink) = open(served, "");
    let reply = request(&socket, &sink, 0, "auth", json!({ "token": token })).await;
    assert_eq!(reply["ok"], json!(true), "a device token lets a socket in: {reply}");
    (socket, sink)
}

async fn ticketed(served: &Rc<Served>, purpose: &str) -> (Rc<Socket>, Rc<support::RecordingSink>) {
    let (host, hsink) = host_socket(served).await;
    let issued = request(&host, &hsink, 1, "ticket.issue", json!({ "purpose": purpose })).await;
    let ticket = issued["ticket"].as_str().expect("a ticket").to_string();
    open(served, &format!("?ticket={ticket}"))
}

fn no_places_calls(rt: &Rc<FakeRuntime>) -> bool {
    rt.script.names().iter().all(|n| !n.starts_with("places."))
}

struct FakeDoor {
    script: Rc<Script>,
}

#[async_trait(?Send)]
impl PlaceDoorControl for FakeDoor {
    async fn open(&self) -> Result<OpenedDoor, HostError> {
        self.script.call("door.open", json!({}))
    }
}

struct FakeAccount {
    script: Rc<Script>,
}

#[async_trait(?Send)]
impl AccountDoor for FakeAccount {
    async fn read(&self) -> Result<AccountView, HostError> {
        self.script.call("account.read", json!({}))
    }
}

struct FakeShelf {
    script: Rc<Script>,
}

#[async_trait(?Send)]
impl RecipeShelf for FakeShelf {
    async fn list(&self) -> Result<Vec<ShelvedRecipe>, HostError> {
        self.script.call("recipes.list", json!({}))
    }
    async fn read(&self, word: &str) -> Result<ShelvedRecipe, HostError> {
        self.script.call("recipes.read", json!({ "word": word }))
    }
    async fn get(&self, word: &str) -> Result<(ShelvedRecipe, String), HostError> {
        self.script.call("recipes.get", json!({ "word": word }))
    }
    async fn save(&self, file: Value) -> Result<ShelvedRecipe, HostError> {
        self.script.call("recipes.save", json!({ "file": file }))
    }
    async fn remove(&self, word: &str) -> Result<ShelvedRecipe, HostError> {
        self.script.call("recipes.remove", json!({ "word": word }))
    }
    async fn options(&self, folders: Option<Vec<(String, String)>>) -> Result<Value, HostError> {
        self.script.call("recipes.options", json!({ "folders": folders }))
    }
    async fn resolve(&self, slug: &str) -> Result<ResolvedRecipe, HostError> {
        self.script.call("recipes.resolve", json!({ "slug": slug }))
    }
    async fn resolve_file(&self, file: RecipeFile) -> Result<ResolvedRecipe, HostError> {
        self.script.call("recipes.resolveFile", json!({ "file": serde_json::to_value(&file).unwrap() }))
    }
}

struct FakeDoctor {
    script: Rc<Script>,
    hold: Cell<bool>,
    gate: tokio::sync::Notify,
    lines: RefCell<Vec<(String, String)>>,
    listeners: Rc<RefCell<Vec<(u64, Rc<dyn Fn(&DoctorLineEvent)>)>>>,
    next: Cell<u64>,
}

impl FakeDoctor {
    fn new(script: Rc<Script>) -> Rc<FakeDoctor> {
        Rc::new(FakeDoctor {
            script,
            hold: Cell::new(false),
            gate: tokio::sync::Notify::new(),
            lines: RefCell::new(Vec::new()),
            listeners: Rc::new(RefCell::new(Vec::new())),
            next: Cell::new(0),
        })
    }
}

#[async_trait(?Send)]
impl PlaceDoctor for FakeDoctor {
    async fn run(&self, place_id: &str, doctor_id: &str, project: Option<&str>) -> Result<i32, HostError> {
        let mut args = json!({ "placeId": place_id, "doctorId": doctor_id });
        if let Some(p) = project {
            args["project"] = json!(p);
        }
        let call = self.script.record("doctor.run", args);
        let lines = self.lines.borrow().clone();
        for (line, stream) in lines {
            let e: DoctorLineEvent = de(json!({ "doctorId": doctor_id, "line": line, "stream": stream }));
            let heard: Vec<Rc<dyn Fn(&DoctorLineEvent)>> = self.listeners.borrow().iter().map(|(_, f)| f.clone()).collect();
            for f in heard {
                f(&e);
            }
        }
        if self.hold.get() {
            self.gate.notified().await;
        }
        self.script.answer::<Result<i32, HostError>>(&call).unwrap_or(Ok(0))
    }
    fn on(&self, f: Rc<dyn Fn(&DoctorLineEvent)>) -> Unlisten {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.listeners.borrow_mut().push((id, f));
        let listeners = self.listeners.clone();
        Box::new(move || listeners.borrow_mut().retain(|(i, _)| *i != id))
    }
}

fn opened(relay: Option<&str>) -> OpenedDoor {
    let mut view = json!({ "port": 14621, "addresses": ["http://192.168.1.20:14621", "http://10.0.0.5:14621"], "hostKey": "ignored" });
    if let Some(r) = relay {
        view["relay"] = json!(r);
    }
    OpenedDoor { view: de(view), back_port: Some(14622) }
}

// ---- gates ---------------------------------------------------------------------------------------------------

// gates. serve-refusal.test.ts:164
#[test]
fn every_places_op_is_refused_kind_ticket_on_a_ticketed_socket() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let (log, lines) = log_lines();
        let served = served_with(&rt, ServeOptions { log: Some(log), ..base(&rt) });
        let (socket, sink) = ticketed(&served, "connect").await;
        let choices = json!({ "name": "web" });
        let ops: Vec<(&str, Value)> = vec![
            ("places.list", json!({})),
            ("places.update", json!({ "placeId": "p_1" })),
            ("places.remove", json!({ "placeId": "p_1" })),
            ("places.holds", json!({ "placeId": "p_1" })),
            ("places.dial", json!({ "placeId": "p_1" })),
            ("places.set", json!({ "placeId": "p_1", "threads": 2 })),
            ("places.follow", json!({ "placeId": "p_1", "recipe": "web" })),
            ("places.skip", json!({ "placeId": "p_1", "row": "clis/git" })),
            ("places.estimate", json!({ "ref": "spoo", "choices": choices })),
            ("places.setupLog", json!({ "placeId": "p_1" })),
            ("places.loginLanded", json!({ "placeId": "p_1", "agent": "claude" })),
            ("places.doctor", json!({ "placeId": "p_1", "doctorId": "d_1" })),
            ("places.add", json!({ "address": "root@10.0.0.5" })),
            ("places.setup", json!({ "ref": "spoo" })),
            ("places.choose", json!({ "ref": "spoo", "choices": choices })),
        ];
        for (i, (op, fields)) in ops.iter().enumerate() {
            let id = 10 + i as u64;
            let reply = request(&socket, &sink, id, op, fields.clone()).await;
            assert_eq!(reply.to_string(), refused(id, PLACES_TICKET_REFUSAL, Some("ticket")), "{op}");
        }
        assert!(no_places_calls(&rt), "a ticketed socket reaches no place door call: {:?}", rt.script.names());
        assert!(rt.script.calls_to("door.open").is_empty());
        let logged = lines.borrow().clone();
        assert_eq!(logged.len(), ops.len());
        for (line, (op, _)) in logged.iter().zip(ops.iter()) {
            assert!(line.starts_with(&format!("refused {op} kind=ticket: ")), "{line}");
        }
    })
}

// gates. pairing.test.ts
#[test]
fn host_only_ops_refuse_a_device_socket_in_their_own_sentence() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        script_devices(&rt, vec![("dev-token", held("d_1", None))]);
        let served = served_with(&rt, ServeOptions { door: Some(Rc::new(FakeDoor { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = device_socket(&served, "dev-token").await;
        let r = request(&socket, &sink, 1, "pair.issue", json!({})).await;
        assert_eq!(r.to_string(), refused(1, PAIR_ISSUE_REFUSAL, None));
        let r = request(&socket, &sink, 2, "places.mint", json!({})).await;
        assert_eq!(r.to_string(), refused(2, MINT_JOIN_REFUSAL, None));
        let r = request(&socket, &sink, 3, "places.sshHosts", json!({})).await;
        assert_eq!(r.to_string(), refused(3, SSH_HOSTS_REFUSAL, None));
        assert!(rt.script.calls_to("devices.issue").is_empty());
        assert!(rt.script.calls_to("door.open").is_empty());
        assert!(rt.script.calls_to("places.hostKey").is_empty());
    })
}

// ---- P1, P2, P3 ----------------------------------------------------------------------------------------------

// P1. serve.test.ts:54
#[test]
fn auth_again_names_the_release_again() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, ServeOptions { released: Some(Released { version: Some("0.3.8".into()), road: Some("launchd".into()) }), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let again = request(&socket, &sink, 7, "auth", json!({ "token": TOKEN })).await;
        assert_eq!(again.to_string(), json!({ "id": 7, "ok": true, "version": "0.3.8", "road": "launchd" }).to_string());

        let bare = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&bare).await;
        let again = request(&socket, &sink, 8, "auth", json!({ "token": TOKEN })).await;
        assert_eq!(again.to_string(), json!({ "id": 8, "ok": true }).to_string());
    })
}

// P2. pairing.test.ts
#[test]
fn pair_issue_answers_the_code_and_the_host_key() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<IssuedCode, HostError>>("devices.issue", Ok(IssuedCode { code: "ABCD2345".into(), expires_at: 1_600_000 }));
        rt.script.once::<Result<IssuedCode, HostError>>("devices.issue", Ok(IssuedCode { code: "WXYZ6789".into(), expires_at: 1_600_000 }));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "pair.issue", json!({})).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "code": "ABCD2345", "expiresAt": 1_600_000, "hostKey": "SHA256:fake-host-key" }).to_string());
        let reply = request(&socket, &sink, 2, "pair.issue", json!({ "here": true })).await;
        assert_eq!(reply["code"], json!("WXYZ6789"));
        let asked = rt.script.calls_to("devices.issue");
        assert_eq!(asked[0].args, json!({ "now": 1_000_000, "ttlMs": 600_000, "here": false }));
        assert_eq!(asked[1].args, json!({ "now": 1_000_000, "ttlMs": 600_000, "here": true }));
    })
}

// P2. pairing.test.ts: a runtime with no place door answers the code alone
#[test]
fn pair_issue_without_a_place_door_answers_the_code_alone() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Result<IssuedCode, HostError>>("devices.issue", Ok(IssuedCode { code: "ABCD2345".into(), expires_at: 1_600_000 }));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "pair.issue", json!({})).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "code": "ABCD2345", "expiresAt": 1_600_000 }).to_string());
        assert!(reply.get("hostKey").is_none());
    })
}

// P2. serve.ts:396-401: no device door refuses through the catch
#[test]
fn pair_issue_without_a_device_door_refuses() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let (failed, told) = {
            let told = Rc::new(RefCell::new(Vec::<String>::new()));
            let kept = told.clone();
            let f: wsp_serve::serve::FailedFn = Rc::new(move |op: &str, e: &HostError| kept.borrow_mut().push(format!("{op}: {}", e.message)));
            (f, told)
        };
        let served = served_with(&rt, ServeOptions { devices: None, failed: Some(failed), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "pair.issue", json!({})).await;
        assert_eq!(reply.to_string(), refused(1, wsp_core::devices::NO_DEVICE_DOOR, None));
        assert_eq!(told.borrow().clone(), vec![format!("pair.issue: {}", wsp_core::devices::NO_DEVICE_DOOR)]);
    })
}

// P2. pairing.test.ts
#[test]
fn pair_issue_on_a_device_socket_is_refused() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        script_devices(&rt, vec![("dev-token", held("d_1", None))]);
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = device_socket(&served, "dev-token").await;
        let reply = request(&socket, &sink, 1, "pair.issue", json!({})).await;
        assert_eq!(reply.to_string(), refused(1, PAIR_ISSUE_REFUSAL, None));
        assert!(rt.script.calls_to("devices.issue").is_empty());
    })
}

// P3. serve.ts:898-917
#[test]
fn identity_ops_once_in_are_refused_and_the_socket_stays() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let report = json!({
            "name": "spoo", "platform": "linux", "arch": "x64", "os": "Ubuntu 24.04", "shape": { "cpu": 4, "memMb": 4096 },
            "login": { "HOME": "/root" }, "runsWorkspaces": true, "engine": "none", "daemonVersion": 54,
            "wsp": ["/usr/local/bin/wsp"], "dialed": "http://192.168.1.20:14621", "agents": []
        });
        let cases: Vec<(&str, Value, &str)> = vec![
            ("pair.redeem", json!({ "code": "ABCD2345", "name": "laptop" }), PAIR_CODE_REFUSAL),
            ("device.auth", json!({ "publicKey": b64(44, 1), "name": "laptop", "signature": b64(64, 2) }), DEVICE_AUTH_REFUSAL),
            ("seal.open", json!({ "nonce": b64(32, 3), "ephemeral": b64(32, 4) }), UNAUTHORIZED),
            ("place.join", json!({ "publicKey": b64(44, 1), "nonce": b64(32, 3) }), PLACE_UNKNOWN_REFUSAL),
            ("place.auth", json!({ "placeId": "p_1", "nonce": b64(32, 3) }), PLACE_UNKNOWN_REFUSAL),
            ("place.prove", json!({ "signature": b64(64, 2), "report": report }), PLACE_UNKNOWN_REFUSAL),
        ];
        for (i, (op, fields, error)) in cases.into_iter().enumerate() {
            let id = 20 + i as u64;
            let reply = request(&socket, &sink, id, op, fields).await;
            assert_eq!(reply.to_string(), refused(id, error, None), "{op}");
        }
        assert_eq!(sink.closed(), None, "none of them closes the socket");
        let after = request(&socket, &sink, 30, "places.list", json!({})).await;
        assert_eq!(after.to_string(), refused(30, NO_PLACE_DOOR, None), "the socket still answers");
    })
}

// ---- P4 ------------------------------------------------------------------------------------------------------

// P4. places.test.ts
#[test]
fn places_list_answers_rows_adds_and_pending() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let rows = vec![row("here", "computer", "zingzys-mac"), row("p_1", "computer", "spoo")];
        let rows_for = rows.clone();
        rt.script.on("places.list", move |_| -> Result<Vec<PlaceView>, HostError> { Ok(rows_for.clone()) });
        let job = de::<wsp_wire::views::place::PlaceAddJob>(json!({ "addId": "a1", "address": "root@10.0.0.5", "startedAt": "2026-01-01T00:00:00.000Z", "state": "running", "steps": [] }));
        let job_for = job.clone();
        rt.script.on("places.adds", move |_| vec![job_for.clone()]);
        let pending = de::<wsp_wire::views::place::PendingComputer>(json!({
            "id": "a2", "address": "root@10.0.0.6", "step": "choosing", "choices": { "name": "web" }, "startedAt": "2026-01-01T00:00:00.000Z"
        }));
        rt.script.once::<Result<Vec<wsp_wire::views::place::PendingComputer>, HostError>>("places.pending", Ok(vec![pending.clone()]));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 3, "places.list", json!({})).await;
        let expected = json!({ "id": 3, "ok": true, "places": rows, "adds": [job], "pending": [pending] });
        assert_eq!(reply.to_string(), expected.to_string());
        assert_eq!(rt.script.names(), vec!["places.list", "places.adds", "places.pending"]);
        assert_eq!(rt.script.calls_to("places.list")[0].args, json!({ "now": 1_000_000 }));
    })
}

// P4. spawn-guard.test.ts
#[test]
fn places_list_for_a_thread_reads_its_places_and_no_adds() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        script_devices(&rt, vec![("thread-token", held("d_t", Some(thread_scope())))]);
        let rows = vec![row("here", "computer", "zingzys-mac"), row("solari", "provider", "solari")];
        rt.script.on("places.list", move |_| -> Result<Vec<PlaceView>, HostError> { Ok(rows.clone()) });
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorkspaceView, HostError>>("workspaces.get", Err(err("no such workspace")));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = device_socket(&served, "thread-token").await;
        let reply = request(&socket, &sink, 4, "places.list", json!({})).await;
        assert_eq!(reply["ok"], json!(true));
        assert_eq!(reply["adds"], json!([]));
        assert_eq!(reply["pending"], json!([]));
        let places = reply["places"].as_array().unwrap();
        assert_eq!(places.len(), 1, "a thread on a machine reads the providers alone: {reply}");
        assert_eq!(places[0]["id"], json!("solari"));
        assert!(places[0].get("road").is_none());
        assert_eq!(places[0]["label"], json!(place_said(&PlaceRef { id: "solari".into(), name: "solari".into(), kind: Some("provider".into()) })));
        assert_eq!(rt.script.calls_to("workspaces.get")[0].args["id"], json!("ws_1"));
        assert!(rt.script.calls_to("places.adds").is_empty());
        assert!(rt.script.calls_to("places.pending").is_empty());
    })
}

// P4. serve.ts:918-922
#[test]
fn places_list_without_a_place_door_says_so() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 3, "places.list", json!({})).await;
        assert_eq!(reply.to_string(), refused(3, NO_PLACE_DOOR, None));
    })
}

// ---- P5 ------------------------------------------------------------------------------------------------------

// P5. places-remove.test.ts
#[test]
fn places_remove_passes_only_the_flags_sent() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let removed = de::<wsp_wire::views::place_more::PlaceRemoved>(json!({ "removed": true, "swept": [] }));
        for _ in 0..3 {
            rt.script.once::<Result<wsp_wire::views::place_more::PlaceRemoved, HostError>>("places.remove", Ok(removed.clone()));
        }
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let plain = request(&socket, &sink, 1, "places.remove", json!({ "placeId": "p_1" })).await;
        assert_eq!(plain.to_string(), json!({ "id": 1, "ok": true, "removed": true, "swept": [] }).to_string());
        request(&socket, &sink, 2, "places.remove", json!({ "placeId": "p_1", "sudoPassword": "pw", "force": true, "forget": true })).await;
        request(&socket, &sink, 3, "places.remove", json!({ "placeId": "p_1", "force": false, "forget": false })).await;
        let asked = rt.script.calls_to("places.remove");
        assert_eq!(asked[0].args, json!({ "placeId": "p_1", "ask": dbg(&RemoveAsk::default()) }));
        assert_eq!(asked[1].args, json!({ "placeId": "p_1", "ask": dbg(&RemoveAsk { sudo_password: Some("pw".into()), force: Some(true), forget: Some(true) }) }));
        assert_eq!(asked[2].args, json!({ "placeId": "p_1", "ask": dbg(&RemoveAsk::default()) }), "force and forget are passed only when true");
    })
}

// P5. places-remove.test.ts
#[test]
fn places_login_landed_answers_after_the_door() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<(), HostError>>("places.loginLanded", Err(err("the login did not land")));
        rt.script.once::<Result<(), HostError>>("places.loginLanded", Ok(()));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let refused_first = request(&socket, &sink, 1, "places.loginLanded", json!({ "placeId": "p_1", "agent": "claude" })).await;
        assert_eq!(refused_first.to_string(), refused(1, "the login did not land", None), "no ok is sent before the door answers");
        let landed = request(&socket, &sink, 2, "places.loginLanded", json!({ "placeId": "p_1", "agent": "claude" })).await;
        assert_eq!(landed.to_string(), json!({ "id": 2, "ok": true }).to_string());
        assert_eq!(rt.script.calls_to("places.loginLanded")[1].args, json!({ "placeId": "p_1", "agent": "claude" }));
    })
}

// P5. serve.ts:923-975, 1085-1089
#[test]
fn places_asks_pass_their_fields_to_the_door_and_name_the_answer() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let view = row("p_1", "computer", "spoo");
        let update = de::<wsp_wire::views::place_more::PlaceUpdateReply>(json!({ "name": "spoo" }));
        let holds = de::<wsp_wire::views::place_more::PlaceHolds>(json!({ "forks": [], "projects": [], "unsaved": [] }));
        let dial = de::<wsp_wire::views::place_more::PlaceDial>(json!({
            "dialled": { "at": "2026-01-01T00:00:00.000Z", "answered": true }, "line": "spoo answered", "place": view
        }));
        let estimate = de::<wsp_wire::views::place_setup_words::PlaceEstimate>(json!({ "neededBytes": 10, "keptBytes": 5, "unmeasured": 0 }));
        let pending = de::<wsp_wire::views::place::PendingComputer>(json!({
            "id": "a2", "address": "root@10.0.0.6", "step": "choosing", "choices": { "name": "web" }, "startedAt": "2026-01-01T00:00:00.000Z"
        }));
        rt.script.once::<Result<wsp_wire::views::place_more::PlaceUpdateReply, HostError>>("places.update", Ok(update.clone()));
        rt.script.once::<Result<wsp_wire::views::place_more::PlaceHolds, HostError>>("places.holds", Ok(holds.clone()));
        rt.script.once::<Result<wsp_wire::views::place_more::PlaceDial, HostError>>("places.dial", Ok(dial.clone()));
        rt.script.once::<Result<PlaceView, HostError>>("places.skip", Ok(view.clone()));
        rt.script.once::<Result<wsp_wire::views::place_setup_words::PlaceEstimate, HostError>>("places.estimate", Ok(estimate.clone()));
        rt.script.once::<Result<Vec<String>, HostError>>("places.setupLog", Ok(vec!["one".into(), "two".into()]));
        rt.script.once::<Result<wsp_wire::views::place::PendingComputer, HostError>>("places.choose", Ok(pending.clone()));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;

        let r = request(&socket, &sink, 1, "places.update", json!({ "placeId": "p_1", "sudoPassword": "pw" })).await;
        assert_eq!(r.to_string(), json!({ "id": 1, "ok": true, "name": "spoo" }).to_string());
        assert_eq!(rt.script.calls_to("places.update")[0].args, json!({ "placeId": "p_1", "ask": dbg(&RemoveAsk { sudo_password: Some("pw".into()), ..RemoveAsk::default() }) }));
        let r = request(&socket, &sink, 2, "places.holds", json!({ "placeId": "p_1" })).await;
        assert_eq!(r.to_string(), json!({ "id": 2, "ok": true, "forks": [], "projects": [], "unsaved": [] }).to_string());
        let r = request(&socket, &sink, 3, "places.dial", json!({ "placeId": "p_1" })).await;
        assert_eq!(r["line"], json!("spoo answered"));
        assert_eq!(rt.script.calls_to("places.dial")[0].args, json!({ "placeId": "p_1", "now": 1_000_000 }));
        let r = request(&socket, &sink, 4, "places.skip", json!({ "placeId": "p_1", "row": "clis/git" })).await;
        assert_eq!(r.to_string(), json!({ "id": 4, "ok": true, "place": view }).to_string());
        assert_eq!(rt.script.calls_to("places.skip")[0].args, json!({ "placeId": "p_1", "row": "clis/git" }));
        let r = request(&socket, &sink, 5, "places.estimate", json!({ "ref": "spoo", "choices": { "name": "web" } })).await;
        assert_eq!(r.to_string(), json!({ "id": 5, "ok": true, "estimate": estimate }).to_string());
        assert_eq!(rt.script.calls_to("places.estimate")[0].args["reference"], json!("spoo"));
        let r = request(&socket, &sink, 6, "places.setupLog", json!({ "placeId": "p_1", "step": "clis" })).await;
        assert_eq!(r.to_string(), json!({ "id": 6, "ok": true, "lines": ["one", "two"] }).to_string());
        assert_eq!(rt.script.calls_to("places.setupLog")[0].args, json!({ "placeId": "p_1", "step": "clis" }));
        let r = request(&socket, &sink, 7, "places.choose", json!({ "ref": "a2", "choices": { "name": "web" }, "recipe": "web" })).await;
        assert_eq!(r.to_string(), json!({ "id": 7, "ok": true, "pending": pending }).to_string());
        let asked = &rt.script.calls_to("places.choose")[0].args;
        assert_eq!((&asked["reference"], &asked["recipe"]), (&json!("a2"), &json!("web")), "choose passes the recipe word as sent");
    })
}

// ---- P6, P7 --------------------------------------------------------------------------------------------------

// P6. places-settings.test.ts
#[test]
fn places_set_reads_a_recipe_word_as_its_slug_and_none_as_none() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let view = row("p_1", "computer", "spoo");
        for _ in 0..3 {
            rt.script.once::<Result<PlaceView, HostError>>("places.set", Ok(view.clone()));
        }
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.read", Ok(shelved("web-dev", "Web Dev")));
        let served = served_with(&rt, ServeOptions { recipes: Some(Rc::new(FakeShelf { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;

        let named = request(&socket, &sink, 1, "places.set", json!({ "placeId": "p_1", "name": "spoo", "recipe": "Web Dev", "threads": 3, "reset": ["nap"] })).await;
        assert_eq!(named.to_string(), spread(1, &serde_json::to_value(&view).unwrap()), "the door's answer is spread after ok");
        let none = request(&socket, &sink, 2, "places.set", json!({ "placeId": "p_1", "recipe": NO_RECIPE })).await;
        assert_eq!(none["ok"], json!(true));
        let absent = request(&socket, &sink, 3, "places.set", json!({ "placeId": "p_1", "threads": 5 })).await;
        assert_eq!(absent["ok"], json!(true));

        let asked = rt.script.calls_to("places.set");
        assert_eq!(asked[0].args, json!({ "placeId": "p_1", "set": { "threads": 3 }, "reset": ["nap"], "also": { "name": "spoo", "recipe": "web-dev" } }));
        assert_eq!(asked[1].args, json!({ "placeId": "p_1", "set": {}, "reset": [], "also": { "recipe": "none" } }));
        assert_eq!(asked[2].args, json!({ "placeId": "p_1", "set": { "threads": 5 }, "reset": [], "also": {} }));
        assert_eq!(rt.script.calls_to("recipes.read").len(), 1, "none and an absent recipe read no shelf");
        assert_eq!(rt.script.calls_to("recipes.read")[0].args, json!({ "word": "Web Dev" }));
    })
}

// P7. places-settings.test.ts
#[test]
fn places_follow_none_stays_none() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let view = row("p_1", "computer", "spoo");
        rt.script.once::<Result<PlaceView, HostError>>("places.follow", Ok(view.clone()));
        rt.script.once::<Result<PlaceView, HostError>>("places.follow", Ok(view.clone()));
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.read", Ok(shelved("web-dev", "Web Dev")));
        let served = served_with(&rt, ServeOptions { recipes: Some(Rc::new(FakeShelf { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let none = request(&socket, &sink, 1, "places.follow", json!({ "placeId": "p_1", "recipe": NO_RECIPE })).await;
        assert_eq!(none.to_string(), json!({ "id": 1, "ok": true, "place": view }).to_string());
        let named = request(&socket, &sink, 2, "places.follow", json!({ "placeId": "p_1", "recipe": "Web Dev" })).await;
        assert_eq!(named["ok"], json!(true));
        let asked = rt.script.calls_to("places.follow");
        assert_eq!(asked[0].args, json!({ "placeId": "p_1", "recipe": "none" }));
        assert_eq!(asked[1].args, json!({ "placeId": "p_1", "recipe": "web-dev" }));
        assert_eq!(rt.script.calls_to("recipes.read").len(), 1);
    })
}

// ---- P8 ------------------------------------------------------------------------------------------------------

fn doctor_rows(rt: &Rc<FakeRuntime>) -> Vec<PlaceView> {
    let rows = vec![row("here", "computer", "zingzys-mac"), row("solari", "provider", "solari"), row("p_1", "computer", "spoo")];
    let held = rows.clone();
    rt.script.on("places.list", move |_| -> Result<Vec<PlaceView>, HostError> { Ok(held.clone()) });
    rows
}

// P8. serve.test.ts:1296
#[test]
fn doctor_says_each_refusal_without_reaching_the_road() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let rows = doctor_rows(&rt);
        let doctor = FakeDoctor::new(rt.script.clone());
        let served = served_with(&rt, ServeOptions { doctor: Some(doctor.clone()), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let named: Vec<String> = rows.iter().map(table).collect();
        let r = request(&socket, &sink, 1, "places.doctor", json!({ "placeId": "p_nope", "doctorId": "d_1" })).await;
        assert_eq!(r.to_string(), refused(1, &no_such_place_refusal("p_nope", &named), Some("usage")));
        let r = request(&socket, &sink, 2, "places.doctor", json!({ "placeId": "here", "doctorId": "d_1" })).await;
        assert_eq!(r.to_string(), refused(2, &doctor_row_refusal(&table(&rows[0])), Some("usage")));
        let r = request(&socket, &sink, 3, "places.doctor", json!({ "placeId": "solari", "doctorId": "d_1" })).await;
        assert_eq!(r.to_string(), refused(3, &doctor_row_refusal(&table(&rows[1])), Some("usage")));
        assert!(!is_joined_computer("here", "computer") && !is_joined_computer("solari", "provider") && is_joined_computer("p_1", "computer"));
        assert!(rt.script.calls_to("doctor.run").is_empty());
    })
}

// P8. serve.test.ts:1289
#[test]
fn doctor_unserved_says_so() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        doctor_rows(&rt);
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let r = request(&socket, &sink, 1, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_1" })).await;
        assert_eq!(r.to_string(), refused(1, DOCTOR_UNSERVED, None));
        assert!(rt.script.calls_to("places.list").is_empty(), "the unserved word comes before any row is read");
    })
}

// P8. serve.test.ts:1310
#[test]
fn doctor_refuses_a_second_road_on_one_computer_until_the_first_answers() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        doctor_rows(&rt);
        let doctor = FakeDoctor::new(rt.script.clone());
        doctor.hold.set(true);
        let served = served_with(&rt, ServeOptions { doctor: Some(doctor.clone()), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;

        let first_socket = socket.clone();
        let first = tokio::task::spawn_local(async move { send(&first_socket, frame(1, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_1", "project": "spoo-landing" }))).await });
        while rt.script.calls_to("doctor.run").is_empty() {
            tokio::task::yield_now().await;
        }
        let second = request(&socket, &sink, 2, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_2" })).await;
        assert_eq!(second.to_string(), refused(2, &doctor_running_line("spoo"), Some("conflict")));
        assert_eq!(rt.script.calls_to("doctor.run").len(), 1, "the second ask never reached the road");

        doctor.hold.set(false);
        doctor.gate.notify_one();
        first.await.unwrap();
        assert_eq!(sink.reply(1).unwrap().to_string(), json!({ "id": 1, "ok": true, "code": 0 }).to_string());
        assert_eq!(rt.script.calls_to("doctor.run")[0].args, json!({ "placeId": "p_1", "doctorId": "d_1", "project": "spoo-landing" }));

        let again = request(&socket, &sink, 3, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_3" })).await;
        assert_eq!(again.to_string(), json!({ "id": 3, "ok": true, "code": 0 }).to_string());
        assert_eq!(rt.script.calls_to("doctor.run")[1].args, json!({ "placeId": "p_1", "doctorId": "d_3" }));
    })
}

// P8. serve.ts:976-1007 (the id leaves `doctoring` whether the run answered or failed)
#[test]
fn doctor_frees_the_computer_when_the_road_fails() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        doctor_rows(&rt);
        rt.script.once::<Result<i32, HostError>>("doctor.run", Err(err("the road broke")));
        let doctor = FakeDoctor::new(rt.script.clone());
        let told = Rc::new(RefCell::new(Vec::<String>::new()));
        let kept = told.clone();
        let failed: wsp_serve::serve::FailedFn = Rc::new(move |op: &str, e: &HostError| kept.borrow_mut().push(format!("{op}: {}", e.message)));
        let served = served_with(&rt, ServeOptions { doctor: Some(doctor.clone()), failed: Some(failed), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let broke = request(&socket, &sink, 1, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_1" })).await;
        assert_eq!(broke.to_string(), refused(1, "the road broke", None));
        assert_eq!(told.borrow().clone(), vec!["places.doctor: the road broke".to_string()]);
        let again = request(&socket, &sink, 2, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_2" })).await;
        assert_eq!(again.to_string(), json!({ "id": 2, "ok": true, "code": 0 }).to_string(), "a failed road leaves the computer free");
        assert_eq!(rt.script.calls_to("doctor.run").len(), 2);
    })
}

// ---- P9, P10, P11 --------------------------------------------------------------------------------------------

// P9. places-door.test.ts
#[test]
fn places_door_answers_the_view_with_the_host_key_and_no_back_port() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<OpenedDoor, HostError>>("door.open", Ok(opened(Some("https://relay.example/r1"))));
        rt.script.once::<Result<OpenedDoor, HostError>>("door.open", Ok(opened(None)));
        let served = served_with(&rt, ServeOptions { door: Some(Rc::new(FakeDoor { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let with_relay = request(&socket, &sink, 1, "places.door", json!({})).await;
        assert_eq!(
            with_relay.to_string(),
            json!({ "id": 1, "ok": true, "door": { "port": 14621, "addresses": ["http://192.168.1.20:14621", "http://10.0.0.5:14621"], "relay": "https://relay.example/r1", "hostKey": "SHA256:fake-host-key" } }).to_string()
        );
        let plain = request(&socket, &sink, 2, "places.door", json!({})).await;
        assert_eq!(
            plain.to_string(),
            json!({ "id": 2, "ok": true, "door": { "port": 14621, "addresses": ["http://192.168.1.20:14621", "http://10.0.0.5:14621"], "hostKey": "SHA256:fake-host-key" } }).to_string()
        );
        assert!(!with_relay.to_string().contains("backPort") && !with_relay.to_string().contains("ignored"));
    })
}

// P9. serve.ts:1008-1022
#[test]
fn places_door_refuses_off_the_own_road_and_without_a_control() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, ServeOptions { door: Some(Rc::new(FakeDoor { script: rt.script.clone() })), ..base(&rt) });
        let (ticket, tsink) = ticketed(&served, "connect").await;
        let r = request(&ticket, &tsink, 5, "places.door", json!({})).await;
        assert_eq!(r.to_string(), refused(5, PLACE_DOOR_REFUSAL, None));
        assert!(rt.script.calls_to("door.open").is_empty());

        let bare = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&bare).await;
        let r = request(&socket, &sink, 6, "places.door", json!({})).await;
        assert_eq!(r.to_string(), refused(6, PLACE_DOOR_UNSERVED, None));
    })
}

// P10. places-door.test.ts
#[test]
fn places_mint_opens_the_door_before_minting_a_code() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let relay = "https://relay.example/r1";
        rt.script.once::<Result<OpenedDoor, HostError>>("door.open", Ok(opened(Some(relay))));
        rt.script.once::<Result<IssuedCode, HostError>>("devices.issue", Ok(IssuedCode { code: "ABCD2345".into(), expires_at: 1_700_000_000_000 }));
        let served = served_with(&rt, ServeOptions { door: Some(Rc::new(FakeDoor { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "places.mint", json!({})).await;
        let token = join_token("ABCD2345", "SHA256:fake-host-key");
        let addresses = ["http://192.168.1.20:14621", "http://10.0.0.5:14621"];
        let expected = json!({ "id": 1, "ok": true, "joins": serde_json::to_value(join_roads(&token, &addresses, Some(relay))).unwrap(), "expiresAt": "2023-11-14T22:13:20.000Z" });
        assert_eq!(reply.to_string(), expected.to_string());
        assert_eq!(rt.script.names(), vec!["places.hostKey", "door.open", "devices.issue"], "the key, then the door, then the code");
        assert_eq!(rt.script.calls_to("devices.issue")[0].args, json!({ "now": 1_000_000, "ttlMs": 600_000, "here": false }));
    })
}

// P10. places-door.test.ts: a host that cannot answer a join mints no code
#[test]
fn places_mint_with_a_door_that_fails_mints_no_code() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<OpenedDoor, HostError>>("door.open", Err(err("the door would not open")));
        let served = served_with(&rt, ServeOptions { door: Some(Rc::new(FakeDoor { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "places.mint", json!({})).await;
        assert_eq!(reply.to_string(), refused(1, "the door would not open", None));
        assert!(rt.script.calls_to("devices.issue").is_empty());

        let bare = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&bare).await;
        let reply = request(&socket, &sink, 2, "places.mint", json!({})).await;
        assert_eq!(reply.to_string(), refused(2, PLACE_DOOR_UNSERVED, None));
    })
}

// P11. places.test.ts
#[test]
fn places_ssh_hosts_answers_none_without_a_door() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let rows = vec![row("p_1", "computer", "spoo")];
        let held = rows.clone();
        rt.script.on("places.list", move |_| -> Result<Vec<PlaceView>, HostError> { Ok(held.clone()) });
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let none = request(&socket, &sink, 1, "places.sshHosts", json!({})).await;
        assert_eq!(none.to_string(), json!({ "id": 1, "ok": true, "hosts": [] }).to_string());

        let seen = Rc::new(RefCell::new(Vec::<usize>::new()));
        let kept = seen.clone();
        let suggestion = de::<wsp_wire::wire::place_link::SshHostSuggestion>(json!({ "alias": "spoo", "hostName": "10.0.0.5", "from": "config" }));
        let said = suggestion.clone();
        let ssh_hosts: wsp_serve::serve::SshHostsFn = Rc::new(move |rows: Vec<PlaceView>| {
            kept.borrow_mut().push(rows.len());
            let said = said.clone();
            Box::pin(async move { Ok(vec![said]) })
        });
        let served = served_with(&rt, ServeOptions { ssh_hosts: Some(ssh_hosts), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let some = request(&socket, &sink, 2, "places.sshHosts", json!({})).await;
        assert_eq!(some.to_string(), json!({ "id": 2, "ok": true, "hosts": [suggestion] }).to_string());
        assert_eq!(seen.borrow().clone(), vec![1], "the door is handed the place rows");

        let bare = FakeRuntime::new();
        let served = served_with(&bare, base(&bare));
        let (socket, sink) = host_socket(&served).await;
        let no_places = request(&socket, &sink, 3, "places.sshHosts", json!({})).await;
        assert_eq!(no_places.to_string(), json!({ "id": 3, "ok": true, "hosts": [] }).to_string());
    })
}

// ---- P12, P13 ------------------------------------------------------------------------------------------------

fn added(rt_view: &PlaceView) -> PlaceAdded {
    PlaceAdded { add_id: "a1".into(), place: rt_view.clone(), host_key: None, said: Some("spoo joined".into()), pending: None }
}

// P12. places-install.test.ts
#[test]
fn places_add_reads_the_recipe_before_dialling_and_passes_fields_in_order() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let view = row("p_1", "computer", "spoo");
        let relay = "https://relay.example/r1";
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.read", Ok(shelved("web-dev", "Web Dev")));
        rt.script.once::<Result<OpenedDoor, HostError>>("door.open", Ok(opened(Some(relay))));
        rt.script.once::<Result<PlaceAdded, HostError>>("places.add", Ok(added(&view)));
        let served = served_with(
            &rt,
            ServeOptions { door: Some(Rc::new(FakeDoor { script: rt.script.clone() })), recipes: Some(Rc::new(FakeShelf { script: rt.script.clone() })), ..base(&rt) },
        );
        let (socket, sink) = host_socket(&served).await;
        let reply = request(
            &socket,
            &sink,
            1,
            "places.add",
            json!({ "addId": "a1", "address": "root@10.0.0.5", "name": "spoo", "sshPort": 2222, "keyPath": "/k", "hostKey": "SHA256:x", "sudoPassword": "pw", "recipe": "Web Dev" }),
        )
        .await;
        assert_eq!(reply["ok"], json!(true));
        assert_eq!(reply["addId"], json!("a1"));
        assert_eq!(reply["place"], serde_json::to_value(&view).unwrap());
        assert_eq!(rt.script.names(), vec!["recipes.read", "door.open", "places.add"], "the recipe is read before the door opens");
        let want = AddAsk {
            add_id: Some("a1".into()),
            address: "root@10.0.0.5".into(),
            name: Some("spoo".into()),
            ssh_port: Some(2222),
            key_path: Some("/k".into()),
            host_key: Some("SHA256:x".into()),
            sudo_password: Some("pw".into()),
            host_urls: vec!["http://192.168.1.20:14621".into(), "http://10.0.0.5:14621".into(), relay.into()],
            door_port: Some(14622),
            relay: Some(relay.into()),
            choices: Some(file("Web Dev")),
            recipe: Some("web-dev".into()),
        };
        assert_eq!(rt.script.calls_to("places.add")[0].args, json!({ "req": dbg(&want), "now": 1_000_000 }));

        rt.script.once::<Result<OpenedDoor, HostError>>("door.open", Ok(opened(None)));
        rt.script.once::<Result<PlaceAdded, HostError>>("places.add", Ok(added(&view)));
        request(&socket, &sink, 2, "places.add", json!({ "address": "root@10.0.0.6" })).await;
        let bare = AddAsk { address: "root@10.0.0.6".into(), host_urls: vec!["http://192.168.1.20:14621".into(), "http://10.0.0.5:14621".into()], door_port: Some(14622), ..AddAsk::default() };
        assert_eq!(rt.script.calls_to("places.add")[1].args, json!({ "req": dbg(&bare), "now": 1_000_000 }), "absent fields are absent");
    })
}

// P12. places-install.test.ts: an unsaved name dials nothing
#[test]
fn places_add_with_an_unsaved_recipe_dials_nothing() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.read", Err(err("no saved recipe named nope; you have web-dev")));
        let served = served_with(
            &rt,
            ServeOptions { door: Some(Rc::new(FakeDoor { script: rt.script.clone() })), recipes: Some(Rc::new(FakeShelf { script: rt.script.clone() })), ..base(&rt) },
        );
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "places.add", json!({ "address": "root@10.0.0.5", "recipe": "nope" })).await;
        assert_eq!(reply.to_string(), refused(1, "no saved recipe named nope; you have web-dev", None));
        assert!(rt.script.calls_to("door.open").is_empty());
        assert!(rt.script.calls_to("places.add").is_empty());

        let bare = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&bare).await;
        let reply = request(&socket, &sink, 2, "places.add", json!({ "address": "root@10.0.0.5" })).await;
        assert_eq!(reply.to_string(), refused(2, PLACE_DOOR_UNSERVED, None));
    })
}

// P13. places-setup.test.ts
#[test]
fn places_setup_prefers_the_named_recipe_over_choices() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let view = row("p_1", "computer", "spoo");
        for _ in 0..3 {
            rt.script.once::<Result<PlaceSetUp, HostError>>("places.setUp", Ok(PlaceSetUp { add_id: "a1".into(), place: view.clone(), setup: None, said: None }));
        }
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.read", Ok(shelved("web-dev", "Web Dev")));
        let served = served_with(&rt, ServeOptions { recipes: Some(Rc::new(FakeShelf { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;

        let named = request(&socket, &sink, 1, "places.setup", json!({ "ref": "spoo", "recipe": "Web Dev", "choices": { "name": "other" }, "addId": "a1", "row": "clis/git" })).await;
        assert_eq!(named["ok"], json!(true));
        assert_eq!(named["addId"], json!("a1"));
        request(&socket, &sink, 2, "places.setup", json!({ "ref": "spoo", "choices": { "name": "other" } })).await;
        request(&socket, &sink, 3, "places.setup", json!({ "ref": "spoo" })).await;

        let asked = rt.script.calls_to("places.setUp");
        let named_ask = SetUpAsk { choices: Some(file("Web Dev")), recipe: Some("web-dev".into()), add_id: Some("a1".into()), row: Some("clis/git".into()) };
        assert_eq!(asked[0].args, json!({ "reference": "spoo", "o": dbg(&named_ask) }));
        let choices_ask = SetUpAsk { choices: Some(file("other")), ..SetUpAsk::default() };
        assert_eq!(asked[1].args, json!({ "reference": "spoo", "o": dbg(&choices_ask) }));
        assert_eq!(asked[2].args, json!({ "reference": "spoo", "o": dbg(&SetUpAsk::default()) }));
    })
}

// ---- P14 -----------------------------------------------------------------------------------------------------

fn recipe_view(h: &ShelvedRecipe, machines: &[&str]) -> Value {
    let mut v = json!({ "name": h.file.name, "slug": h.slug, "summary": recipe_summary(&h.file), "machines": machines });
    if let Some(at) = &h.saved_at {
        v["savedAt"] = json!(at);
    }
    v["file"] = serde_json::to_value(&h.file).unwrap();
    v
}

fn followers(pairs: &[(&str, &[&str])]) -> IndexMap<String, Vec<String>> {
    pairs.iter().map(|(slug, names)| (slug.to_string(), names.iter().map(|n| n.to_string()).collect())).collect()
}

fn with_shelf(rt: &Rc<FakeRuntime>) -> Rc<Served> {
    served_with(rt, ServeOptions { recipes: Some(Rc::new(FakeShelf { script: rt.script.clone() })), ..base(rt) })
}

// P14. places-settings.test.ts
#[test]
fn recipes_list_get_save_remove_options() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let web = shelved("web-dev", "Web Dev");
        let empty = shelved("empty", "Empty");
        let served = with_shelf(&rt);
        let (socket, sink) = host_socket(&served).await;

        // list: the followers are read first, then every held recipe
        rt.script.once::<Result<IndexMap<String, Vec<String>>, HostError>>("places.followers", Ok(followers(&[("web-dev", &["spoo"])])));
        rt.script.once::<Result<Vec<ShelvedRecipe>, HostError>>("recipes.list", Ok(vec![web.clone(), empty.clone()]));
        let listed = request(&socket, &sink, 1, "recipes.list", json!({})).await;
        assert_eq!(listed.to_string(), json!({ "id": 1, "ok": true, "recipes": [recipe_view(&web, &["spoo"]), recipe_view(&empty, &[])] }).to_string());
        assert_eq!(rt.script.names(), vec!["places.followers", "recipes.list"]);

        // get
        rt.script.once::<Result<(ShelvedRecipe, String), HostError>>("recipes.get", Ok((web.clone(), "h1".into())));
        rt.script.once::<Result<IndexMap<String, Vec<String>>, HostError>>("places.followers", Ok(followers(&[("web-dev", &["spoo"])])));
        let got = request(&socket, &sink, 2, "recipes.get", json!({ "name": "Web Dev" })).await;
        assert_eq!(got.to_string(), json!({ "id": 2, "ok": true, "recipe": recipe_view(&web, &["spoo"]), "hash": "h1" }).to_string());
        assert_eq!(rt.script.calls_to("recipes.get")[0].args, json!({ "word": "Web Dev" }));

        // save a file as given: every follower takes the change, then the view reads the followers
        let before = rt.script.calls().len();
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.save", Ok(web.clone()));
        rt.script.once::<Result<Vec<String>, HostError>>("places.recipeChanged", Ok(vec!["spoo".into()]));
        rt.script.once::<Result<IndexMap<String, Vec<String>>, HostError>>("places.followers", Ok(followers(&[("web-dev", &["spoo"])])));
        let saved = request(&socket, &sink, 3, "recipes.save", json!({ "name": "Web Dev", "file": { "name": "ignored", "copyKeys": true } })).await;
        assert_eq!(saved.to_string(), json!({ "id": 3, "ok": true, "recipe": recipe_view(&web, &["spoo"]) }).to_string());
        let after: Vec<String> = rt.script.names()[before..].to_vec();
        assert_eq!(after, vec!["recipes.save", "places.recipeChanged", "places.followers"]);
        assert_eq!(rt.script.calls_to("recipes.save")[0].args["file"]["name"], json!("Web Dev"), "the name is the frame's name");
        assert_eq!(rt.script.calls_to("recipes.save")[0].args["file"]["copyKeys"], json!(true));
        assert_eq!(rt.script.calls_to("places.recipeChanged")[0].args["slug"], json!("web-dev"));

        // remove: followers before, then the remove, then every follower lets go
        let before = rt.script.calls().len();
        rt.script.once::<Result<IndexMap<String, Vec<String>>, HostError>>("places.followers", Ok(followers(&[("web-dev", &["spoo"])])));
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.remove", Ok(web.clone()));
        rt.script.once::<Result<Vec<String>, HostError>>("places.unfollow", Ok(vec!["spoo".into()]));
        let removed = request(&socket, &sink, 4, "recipes.remove", json!({ "name": "Web Dev" })).await;
        assert_eq!(removed.to_string(), json!({ "id": 4, "ok": true, "recipe": recipe_view(&web, &["spoo"]) }).to_string());
        assert_eq!(rt.script.names()[before..].to_vec(), vec!["places.followers", "recipes.remove", "places.unfollow"]);
        assert_eq!(rt.script.calls_to("places.unfollow")[0].args, json!({ "slug": "web-dev" }));

        // options: the projects on this computer, then the shelf's options
        let mine: wsp_wire::views::workspace::ProjectView = de(json!({
            "id": "pr_1", "name": "lab", "computer": "here", "source": { "kind": "folder", "path": "/home/z/lab" }, "path": "/home/z/lab", "remote": "",
            "defaultBranch": "main", "memoryKey": "k1", "memoryDir": "/m/k1", "createdAt": "2026-01-01T00:00:00.000Z"
        }));
        let theirs: wsp_wire::views::workspace::ProjectView = de(json!({
            "id": "pr_2", "name": "api", "computer": "p_1", "source": { "kind": "folder", "path": "/srv/api" }, "path": "/srv/api", "remote": "",
            "defaultBranch": "main", "memoryKey": "k2", "memoryDir": "/m/k2", "createdAt": "2026-01-01T00:00:00.000Z"
        }));
        rt.script.once::<Result<Vec<wsp_wire::views::workspace::ProjectView>, HostError>>("projects.list", Ok(vec![mine, theirs]));
        rt.script.once::<Result<Value, HostError>>("recipes.options", Ok(json!({ "clis": ["git"] })));
        let options = request(&socket, &sink, 5, "recipes.options", json!({})).await;
        assert_eq!(options.to_string(), json!({ "id": 5, "ok": true, "options": { "clis": ["git"] } }).to_string());
        assert_eq!(rt.script.calls_to("recipes.options")[0].args, json!({ "folders": [["lab", "/home/z/lab"]] }), "only this computer's projects are folders a box can take");
    })
}

// P14. places-settings.test.ts
#[test]
fn recipes_save_from_here_is_refused_usage() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = with_shelf(&rt);
        let (socket, sink) = host_socket(&served).await;
        rt.script.once::<Result<PlaceFor, HostError>>("places.placeFor", Ok(PlaceFor { place_id: None }));
        let from_here = request(&socket, &sink, 1, "recipes.save", json!({ "name": "mine", "from": "here" })).await;
        assert_eq!(from_here.to_string(), refused(1, &recipe_from_here_refusal("here"), Some("usage")));

        rt.script.once::<Result<PlaceFor, HostError>>("places.placeFor", Ok(PlaceFor { place_id: Some("p_1".into()) }));
        rt.script.once::<Result<Option<RecipeFile>, HostError>>("places.picksOf", Ok(None));
        let no_picks = request(&socket, &sink, 2, "recipes.save", json!({ "name": "mine", "from": "spoo" })).await;
        assert_eq!(no_picks.to_string(), refused(2, &no_picks_refusal("spoo"), Some("usage")));
        assert!(rt.script.calls_to("recipes.save").is_empty());
        assert!(rt.script.calls_to("places.follow").is_empty());
    })
}

// P14. places-settings.test.ts: from a computer, its picks are saved under the name and that computer follows
#[test]
fn recipes_save_from_a_computer_saves_its_picks_and_follows() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = with_shelf(&rt);
        let (socket, sink) = host_socket(&served).await;
        let mine = shelved("mine", "mine");
        rt.script.once::<Result<PlaceFor, HostError>>("places.placeFor", Ok(PlaceFor { place_id: Some("p_1".into()) }));
        rt.script.once::<Result<Option<RecipeFile>, HostError>>("places.picksOf", Ok(Some(file("picked"))));
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.save", Ok(mine.clone()));
        rt.script.once::<Result<wsp_wire::views::place_more::PlaceView, HostError>>("places.follow", Ok(row("p_1", "computer", "spoo")));
        rt.script.once::<Result<IndexMap<String, Vec<String>>, HostError>>("places.followers", Ok(followers(&[("mine", &["spoo"])])));
        let reply = request(&socket, &sink, 1, "recipes.save", json!({ "name": "mine", "from": "spoo" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "recipe": recipe_view(&mine, &["spoo"]) }).to_string());
        assert_eq!(rt.script.names(), vec!["places.placeFor", "places.picksOf", "recipes.save", "places.follow", "places.followers"]);
        assert_eq!(rt.script.calls_to("places.placeFor")[0].args, json!({ "word": "spoo" }));
        assert_eq!(rt.script.calls_to("places.picksOf")[0].args, json!({ "placeId": "p_1" }));
        assert_eq!(rt.script.calls_to("recipes.save")[0].args, json!({ "file": { "name": "mine", "agents": {}, "mcp": {}, "clis": {}, "skills": {}, "plugins": {}, "folders": {}, "configs": serde_json::to_value(file("x")).unwrap()["configs"] } }));
        assert_eq!(rt.script.calls_to("places.follow")[0].args, json!({ "placeId": "p_1", "recipe": "mine" }));
    })
}

// P14. places-settings.test.ts
#[test]
fn recipes_remove_check_unfollows_nothing() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = with_shelf(&rt);
        let (socket, sink) = host_socket(&served).await;
        let web = shelved("web-dev", "Web Dev");
        rt.script.once::<Result<IndexMap<String, Vec<String>>, HostError>>("places.followers", Ok(followers(&[("web-dev", &["spoo", "lab"])])));
        rt.script.once::<Result<ShelvedRecipe, HostError>>("recipes.read", Ok(web.clone()));
        let reply = request(&socket, &sink, 1, "recipes.remove", json!({ "name": "Web Dev", "check": true })).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "recipe": recipe_view(&web, &["spoo", "lab"]) }).to_string());
        assert_eq!(rt.script.names(), vec!["places.followers", "recipes.read"]);
        assert!(rt.script.calls_to("recipes.remove").is_empty());
        assert!(rt.script.calls_to("places.unfollow").is_empty());
    })
}

// P14. serve-refusal.test.ts:164
#[test]
fn recipes_are_refused_kind_ticket_off_the_own_road() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = with_shelf(&rt);
        let (socket, sink) = ticketed(&served, "relay").await;
        let ops: Vec<(&str, Value)> = vec![
            ("recipes.list", json!({})),
            ("recipes.get", json!({ "name": "web" })),
            ("recipes.save", json!({ "name": "web", "file": { "name": "web" } })),
            ("recipes.remove", json!({ "name": "web" })),
            ("recipes.options", json!({})),
        ];
        for (i, (op, fields)) in ops.into_iter().enumerate() {
            let id = 10 + i as u64;
            let reply = request(&socket, &sink, id, op, fields).await;
            assert_eq!(reply.to_string(), refused(id, RECIPES_TICKET_REFUSAL, Some("ticket")), "{op}");
        }
        assert!(rt.script.names().iter().all(|n| !n.starts_with("recipes.") && !n.starts_with("places.")), "{:?}", rt.script.names());
    })
}

// P14. serve.ts:1090-1127: neither the host's shelf nor the runtime's
#[test]
fn recipes_without_a_shelf_say_so() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "recipes.list", json!({})).await;
        assert_eq!(reply.to_string(), refused(1, NO_RECIPES, None));
    })
}

// ---- P15, P16, P27 -------------------------------------------------------------------------------------------

// P15. serve-refusal.test.ts:164
#[test]
fn account_get_unserved_and_ticketed() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, base(&rt));
        let (ticket, tsink) = ticketed(&served, "relay").await;
        let r = request(&ticket, &tsink, 5, "account.get", json!({})).await;
        assert_eq!(r.to_string(), refused(5, ACCOUNT_TICKET_REFUSAL, Some("ticket")));
        let (socket, sink) = host_socket(&served).await;
        let r = request(&socket, &sink, 6, "account.get", json!({})).await;
        assert_eq!(r.to_string(), refused(6, ACCOUNT_UNSERVED, None));

        let account: AccountView = de(json!({ "signedIn": true, "login": "zingzy" }));
        rt.script.once::<Result<AccountView, HostError>>("account.read", Ok(account.clone()));
        let served = served_with(&rt, ServeOptions { account: Some(Rc::new(FakeAccount { script: rt.script.clone() })), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let r = request(&socket, &sink, 7, "account.get", json!({})).await;
        assert_eq!(r.to_string(), json!({ "id": 7, "ok": true, "account": { "signedIn": true, "login": "zingzy" } }).to_string());
    })
}

// P16. serve-refusal.test.ts:164
#[test]
fn devices_list_on_a_ticket_is_refused() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, base(&rt));
        let (ticket, tsink) = ticketed(&served, "relay").await;
        let r = request(&ticket, &tsink, 5, "devices.list", json!({})).await;
        assert_eq!(r.to_string(), refused(5, DEVICES_TICKET_REFUSAL, Some("ticket")));
        let r = request(&ticket, &tsink, 6, "devices.revoke", json!({ "deviceId": "d_1" })).await;
        assert_eq!(r.to_string(), refused(6, DEVICES_TICKET_REFUSAL, Some("ticket")));
        assert!(rt.script.calls_to("devices.list").is_empty());
        assert!(rt.script.calls_to("devices.revoke").is_empty());

        let (socket, sink) = host_socket(&served).await;
        let view = held("d_1", None).view;
        rt.script.once::<Result<Vec<wsp_wire::wire::devices::DeviceView>, HostError>>("devices.list", Ok(vec![view.clone()]));
        let r = request(&socket, &sink, 7, "devices.list", json!({})).await;
        assert_eq!(r.to_string(), json!({ "id": 7, "ok": true, "devices": [view] }).to_string());
    })
}

// P27. pairing.test.ts
#[test]
fn devices_revoke_answers_before_cutting_its_own_socket() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        script_devices(&rt, vec![("tok-1", held("d_1", None)), ("tok-2", held("d_2", None))]);
        rt.script.once::<Result<bool, HostError>>("devices.revoke", Ok(true));
        let served = served_with(&rt, base(&rt));
        let (mine, mine_sink) = device_socket(&served, "tok-1").await;
        let (twin, twin_sink) = device_socket(&served, "tok-1").await;
        let (other, other_sink) = device_socket(&served, "tok-2").await;
        let _keep = (&twin, &other);

        let reply = request(&mine, &mine_sink, 9, "devices.revoke", json!({ "deviceId": "d_1" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 9, "ok": true, "revoked": true }).to_string(), "a paired computer may revoke its own token and reads the answer");
        assert_eq!(rt.script.calls_to("devices.revoke")[0].args, json!({ "id": "d_1" }));
        assert_eq!(mine_sink.closed(), Some((4401, UNAUTHORIZED.to_string())));
        assert_eq!(twin_sink.closed(), Some((4401, UNAUTHORIZED.to_string())), "every socket the device held is cut");
        assert_eq!(other_sink.closed(), None, "another device keeps its socket");
    })
}

// ---- P17 -----------------------------------------------------------------------------------------------------

// P17. serve.test.ts:311
#[test]
fn ticket_issue_answers_a_ticket_without_the_token_and_ttl_five_minutes() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let issued = request(&socket, &sink, 1, "ticket.issue", json!({ "purpose": "connect" })).await;
        assert_eq!(issued["ok"], json!(true));
        let ticket = issued["ticket"].as_str().unwrap();
        assert!(!ticket.contains(TOKEN), "the ticket never contains the host token");
        assert_eq!(ticket.len(), 32, "24 random bytes in base64url");
        assert!(ticket.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(issued["expiresAt"], json!(1_000_000 + 300_000));
        assert_eq!(issued.to_string(), json!({ "id": 1, "ok": true, "ticket": ticket, "expiresAt": 1_300_000 }).to_string());
        let kept = served.tickets();
        let held = kept.get(ticket).expect("the ticket is kept");
        assert_eq!((held.expires_at, held.paired), (1_300_000, false));
        assert_eq!(held.purpose, wsp_wire::wire::devices::TicketPurpose::Connect);
    })
}

// P17. serve-refusal.test.ts:164
#[test]
fn a_ticketed_socket_cannot_mint_a_ticket() {
    support::block(async {
        let rt = FakeRuntime::new();
        let (log, lines) = log_lines();
        let served = served_with(&rt, ServeOptions { log: Some(log), ..base(&rt) });
        let (socket, sink) = ticketed(&served, "relay").await;
        let held_before = served.tickets().len();
        let reply = request(&socket, &sink, 5, "ticket.issue", json!({ "purpose": "relay" })).await;
        assert_eq!(reply.to_string(), refused(5, RELAY_TICKET_REFUSAL, Some("ticket")));
        assert_eq!(served.tickets().len(), held_before, "no ticket was minted");
        assert_eq!(lines.borrow().len(), 1);
        assert!(lines.borrow()[0].starts_with("refused ticket.issue kind=ticket: "));
    })
}

// P17. pairing.test.ts
#[test]
fn a_ticket_minted_by_a_paired_socket_carries_the_paired_road() {
    support::block(async {
        let rt = FakeRuntime::new();
        script_devices(&rt, vec![("tok-1", held("d_1", None))]);
        let served = served_with(&rt, base(&rt));
        let (paired, psink) = device_socket(&served, "tok-1").await;
        let from_paired = request(&paired, &psink, 1, "ticket.issue", json!({ "purpose": "connect" })).await;
        let (host, hsink) = host_socket(&served).await;
        let from_host = request(&host, &hsink, 2, "ticket.issue", json!({ "purpose": "relay" })).await;
        let kept = served.tickets();
        assert!(kept.get(from_paired["ticket"].as_str().unwrap()).unwrap().paired);
        assert!(!kept.get(from_host["ticket"].as_str().unwrap()).unwrap().paired);
        assert_eq!(kept.get(from_host["ticket"].as_str().unwrap()).unwrap().purpose, wsp_wire::wire::devices::TicketPurpose::Relay);
    })
}
