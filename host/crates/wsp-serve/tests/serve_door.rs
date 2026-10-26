// SPDX-License-Identifier: AGPL-3.0-only
//! The door every socket passes, and the refusal log: spec/serve_door.md. Each test names its rule and its TS case.
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value};
use support::*;
use wsp_core::devices::{HeldDevice, PairedDevice};
use wsp_machines::places::types::{AuthAnswer, PlaceChallenge, Proved};
use wsp_serve::serve::door::{refused_line, Served};
use wsp_serve::serve::socket::{Incoming, Road};
use wsp_serve::serve::{AdmittedDevices, Authed, Released, ServeOptions, Signer, NO_AUTH_TOKEN};
use wsp_store::keys::{key_fingerprint, make_seal, new_place_key_pair, seal_keys, sign_place_bytes, Seal, Side};
use wsp_wire::types::daemon_contract::{DAEMON_AUTH_DEADLINE_PASSED, DAEMON_PRE_AUTH_BYTES_EXCEEDED, PRE_AUTH_MAX_BYTES};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::devices::{
    device_admission_transcript, AccountDevice, DeviceView, DEVICE_ACCOUNT_UNSERVED, DEVICE_AUTH_REFUSAL, DEVICE_REVOKED_REFUSAL,
    PAIR_CODE_REFUSAL,
};
use wsp_wire::wire::limits::AUTH_DEADLINE_CLOSE;
use wsp_wire::wire::place_link::{PLACE_CODE_REFUSAL, SEAL_CLIENT, SEAL_UNSERVED};
use wsp_wire::wire::runtime::{DEVICE_OPS, THREAD_OPS};
use wsp_words::exit::HostError;
use wsp_words::wire::runtime::thread_op_refusal;
use wsp_words::words::computer::{device_held_refusal, REQUEST_NOT_AN_OBJECT, SCOPED_TOKEN_ROAD_REFUSAL, UNAUTHORIZED};

const UNAUTHORIZED_CLOSE: u16 = 4401;

fn device(id: &str) -> DeviceView {
    serde_json::from_value(
        json!({ "id": id, "name": "laptop", "createdAt": "2026-10-25T00:00:00.000Z", "lastSeenAt": "2026-10-25T00:00:00.000Z" }),
    )
    .unwrap()
}

fn scope(thread: &str, workspace: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": thread })).unwrap()
}

fn held(id: &str, scope: Option<ThreadScope>, road: Option<WorkspaceOrigin>) -> HeldDevice {
    let mut view = device(id);
    view.scope = scope;
    HeldDevice { view, road }
}

/// The options with the fake device door wired, which the door matches a device token through.
fn dopts(rt: &Rc<FakeRuntime>) -> ServeOptions {
    let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
    ServeOptions { devices: Some(devices), ..opts() }
}

/// A report a place's prove carries that the wire's schema takes.
fn report() -> Value {
    json!({
        "name": "box-1", "platform": "linux", "arch": "arm64", "os": "Ubuntu 24.04", "shape": { "cpu": 2, "memMb": 4096 },
        "login": {}, "runsWorkspaces": true, "engine": "none", "daemonVersion": 150, "wsp": ["wsp"], "agents": [], "dialed": "https://host"
    })
}

/// A socket let in with a device token the fake door matches to `device`. The server must be built with `dopts`.
async fn device_socket(
    rt: &Rc<FakeRuntime>,
    served: &Rc<Served>,
    device: HeldDevice,
) -> (Rc<wsp_serve::serve::socket::Socket>, Rc<RecordingSink>) {
    rt.script.once::<Result<Option<HeldDevice>, HostError>>("devices.match", Ok(Some(device)));
    let (socket, sink) = open(served, "");
    let reply = request(&socket, &sink, 0, "auth", json!({ "token": "device-token" })).await;
    assert_eq!(reply["ok"], json!(true), "the device token lets the socket in: {reply}");
    (socket, sink)
}

fn served_now(rt: &Rc<FakeRuntime>, now: Rc<RefCell<i64>>, mut o: ServeOptions) -> Rc<Served> {
    o.now = Some(Rc::new(move || *now.borrow()));
    served_with(rt, o)
}

// ---- Opening ----

#[test]
fn an_empty_auth_token_refuses_to_serve() {
    // D1. serve.ts:385
    block(async {
        let rt = FakeRuntime::new();
        let refused = Served::new(rt.clone(), ServeOptions::default()).err().expect("an empty token is refused");
        assert_eq!(refused.message, NO_AUTH_TOKEN);
    })
}

#[test]
fn a_ticket_lets_one_socket_in_once() {
    // D2. serve.test.ts:311
    block(async {
        let rt = FakeRuntime::new();
        let now = Rc::new(RefCell::new(1_000_000i64));
        let served = served_now(&rt, now.clone(), opts());
        let (host, host_sink) = host_socket(&served).await;
        let issued = request(&host, &host_sink, 1, "ticket.issue", json!({ "purpose": "connect" })).await;
        let ticket = issued["ticket"].as_str().unwrap().to_string();
        assert!(!ticket.contains(TOKEN));
        assert_eq!(issued["expiresAt"], json!(1_000_000 + 300_000));

        let (second, second_sink) = open(&served, &format!("?ticket={ticket}"));
        assert!(second.authed(), "a good ticket lets the socket in at open");
        assert_eq!(second.road(), Road { stamped: Some(WorkspaceOrigin::Here), paired_road: false, me: Some(Authed::Host), by: None });
        assert_eq!(second_sink.closed(), None);
        assert!(served.tickets().is_empty(), "the ticket is spent at its first use");

        let (_third, third_sink) = open(&served, &format!("?ticket={ticket}"));
        assert_eq!(third_sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
    })
}

#[test]
fn a_ticket_past_its_expiry_closes_4401_and_is_spent() {
    // D2. serve.test.ts:332
    block(async {
        let rt = FakeRuntime::new();
        let now = Rc::new(RefCell::new(1_000_000i64));
        let served = served_now(&rt, now.clone(), opts());
        let (host, host_sink) = host_socket(&served).await;
        let issued = request(&host, &host_sink, 1, "ticket.issue", json!({ "purpose": "connect" })).await;
        *now.borrow_mut() += 300_001;
        let (late, late_sink) = open(&served, &format!("?ticket={}", issued["ticket"].as_str().unwrap()));
        assert!(!late.authed());
        assert_eq!(late_sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert!(served.tickets().is_empty(), "an expired ticket is spent too");
    })
}

#[test]
fn a_ticket_at_its_expiry_still_opens() {
    // D2. serve.ts:520 compares with >
    block(async {
        let rt = FakeRuntime::new();
        let now = Rc::new(RefCell::new(5_000i64));
        let served = served_now(&rt, now.clone(), opts());
        let (host, host_sink) = host_socket(&served).await;
        let issued = request(&host, &host_sink, 1, "ticket.issue", json!({ "purpose": "relay" })).await;
        *now.borrow_mut() = issued["expiresAt"].as_i64().unwrap();
        let (socket, sink) = open(&served, &format!("?ticket={}", issued["ticket"].as_str().unwrap()));
        assert!(socket.authed());
        assert_eq!(sink.closed(), None);
    })
}

#[test]
fn a_relay_ticket_stamps_relayed_and_a_connect_ticket_here() {
    // D2, D34. serve.ts:519-526
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (host, host_sink) = host_socket(&served).await;
        let relay = request(&host, &host_sink, 1, "ticket.issue", json!({ "purpose": "relay" })).await;
        let connect = request(&host, &host_sink, 2, "ticket.issue", json!({ "purpose": "connect" })).await;
        let (relayed, _) = open(&served, &format!("?ticket={}", relay["ticket"].as_str().unwrap()));
        let (here, _) = open(&served, &format!("?ticket={}", connect["ticket"].as_str().unwrap()));
        assert_eq!(relayed.road().stamped, Some(WorkspaceOrigin::Relayed));
        assert_eq!(here.road().stamped, Some(WorkspaceOrigin::Here));
        assert!(!relayed.road().own_road() && !here.road().own_road(), "a ticketed socket is never the person's own road");
    })
}

#[test]
fn an_unknown_ticket_closes_and_reads_no_frame() {
    // D2. serve.ts:520-523
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "?ticket=never-minted");
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        send(&socket, json!({ "id": 1, "op": "auth", "token": TOKEN })).await;
        assert!(!socket.authed());
    })
}

#[test]
fn a_silent_socket_closes_4408_with_the_daemons_sentence() {
    // D3. serve.test.ts:169
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, ServeOptions { auth_deadline_ms: Some(60), ..opts() });
        let (_socket, sink) = open(&served, "");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(sink.closed(), Some((AUTH_DEADLINE_CLOSE as u16, DAEMON_AUTH_DEADLINE_PASSED.to_string())));
        assert!(sink.frames().is_empty());
    })
}

#[test]
fn a_socket_let_in_is_held_to_no_deadline() {
    // D3, D5. serve.test.ts:184
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, ServeOptions { auth_deadline_ms: Some(60), ..opts() });
        let (_socket, sink) = host_socket(&served).await;
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(sink.closed(), None);
    })
}

#[test]
fn more_bytes_than_the_cap_close_4401_and_answer_nothing() {
    // D4. serve.test.ts:175
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        socket.bytes_arrived(PRE_AUTH_MAX_BYTES as usize + 1);
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, DAEMON_PRE_AUTH_BYTES_EXCEEDED.to_string())));
        assert!(sink.frames().is_empty());
    })
}

#[test]
fn exactly_the_cap_passes_and_bytes_count_across_chunks() {
    // D4. serve.ts:582 compares with <=
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        socket.bytes_arrived(4_000);
        socket.bytes_arrived(96);
        assert_eq!(sink.closed(), None, "4,096 bytes in two chunks pass");
        socket.bytes_arrived(1);
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, DAEMON_PRE_AUTH_BYTES_EXCEEDED.to_string())));
    })
}

#[test]
fn a_socket_through_the_door_takes_any_size() {
    // D4, D5. serve.test.ts:184
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = host_socket(&served).await;
        socket.bytes_arrived(64 * 1024);
        assert_eq!(sink.closed(), None);
    })
}

// ---- Every frame ----

#[test]
fn invalid_json_before_auth_closes_and_after_auth_stays() {
    // D7. serve.ts:635-641
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (stranger, stranger_sink) = open(&served, "");
        send_raw(&stranger, "{not json").await;
        assert_eq!(stranger_sink.frames(), vec![json!({ "id": null, "ok": false, "error": "invalid json" })]);
        assert_eq!(stranger_sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));

        let (owner, owner_sink) = host_socket(&served).await;
        send_raw(&owner, "{not json").await;
        assert_eq!(owner_sink.frames().last().unwrap(), &json!({ "id": null, "ok": false, "error": "invalid json" }));
        assert_eq!(owner_sink.closed(), None);
        assert_eq!(owner_sink.last_text(), r#"{"id":null,"ok":false,"error":"invalid json"}"#, "R1: id, ok, error in that order");
    })
}

#[test]
fn a_bare_value_before_auth_is_refused_under_a_null_id_and_closes() {
    // D8. serve.test.ts:89
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (stranger, sink) = open(&served, "");
        send_raw(&stranger, "null").await;
        assert_eq!(sink.frames(), vec![json!({ "id": null, "ok": false, "error": REQUEST_NOT_AN_OBJECT })]);
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        let (_owner, owner_sink) = host_socket(&served).await;
        assert_eq!(owner_sink.closed(), None, "the host answers the next socket");
    })
}

#[test]
fn bare_values_on_the_owners_socket_are_refused_and_it_stays() {
    // D8. serve.test.ts:102
    block(async {
        let rt = FakeRuntime::new();
        rt.script.on("workspaces.list", |_| Ok::<Vec<wsp_wire::views::workspace_more::WorkspaceView>, HostError>(vec![]));
        let served = served_with(&rt, opts());
        let (owner, sink) = host_socket(&served).await;
        let before = sink.frames().len();
        for raw in ["null", "7", "\"a string\"", "true", "[]"] {
            send_raw(&owner, raw).await;
        }
        let refused: Vec<Value> = sink.frames()[before..].to_vec();
        assert_eq!(refused, vec![json!({ "id": null, "ok": false, "error": REQUEST_NOT_AN_OBJECT }); 5]);
        let listed = request(&owner, &sink, 9, "workspaces.list", json!({})).await;
        assert_eq!(listed, json!({ "id": 9, "ok": true, "workspaces": [] }));
        assert_eq!(sink.closed(), None);
    })
}

#[test]
fn a_threads_token_is_refused_an_op_outside_its_list_by_name() {
    // D9. spawn-guard.test.ts (the door half), serve.ts:654-659
    block(async {
        let rt = FakeRuntime::new();
        rt.script.on("workspaces.threadPlace", |_| Some("my-box".to_string()));
        let served = served_with(&rt, dopts(&rt));
        let by = scope("t-1", "ws-1");
        let (socket, sink) = device_socket(&rt, &served, held("d-1", Some(by.clone()), Some(WorkspaceOrigin::Relayed))).await;
        assert!(!THREAD_OPS.contains(&"places.remove"));
        let refused = request(&socket, &sink, 3, "places.remove", json!({ "placeId": "p" })).await;
        assert_eq!(refused, json!({ "id": 3, "ok": false, "error": thread_op_refusal("places.remove", "t-1", Some("my-box")) }));
        // Refused by name before its shape is read: a frame with no op names "that frame", under the id it sent.
        send(&socket, json!({ "id": { "odd": true } })).await;
        assert_eq!(
            sink.frames().last().unwrap(),
            &json!({ "id": { "odd": true }, "ok": false, "error": thread_op_refusal("that frame", "t-1", Some("my-box")) })
        );
        assert_eq!(sink.closed(), None);
        assert!(rt.script.calls_to("places.remove").is_empty());
    })
}

#[test]
fn a_paired_device_is_refused_an_op_outside_its_list() {
    // D10. pairing.test.ts (the door half), serve.ts:664-668
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, dopts(&rt));
        let (socket, sink) = device_socket(&rt, &served, held("d-1", None, None)).await;
        assert!(socket.road().paired_road);
        assert!(!DEVICE_OPS.contains(&"agents.key"));
        let refused = request(&socket, &sink, 4, "agents.key", json!({ "agent": "claude", "key": "sk-x" })).await;
        assert_eq!(refused, json!({ "id": 4, "ok": false, "error": device_held_refusal("agents.key") }));
        assert_eq!(sink.closed(), None);
    })
}

#[test]
fn a_paired_device_on_a_relay_ticket_reads_the_relays_gate() {
    // D10, D33. serve.ts:664 (asPaired is false on a relayed stamp)
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, dopts(&rt));
        let (paired, paired_sink) = device_socket(&rt, &served, held("d-1", None, None)).await;
        let relay = request(&paired, &paired_sink, 1, "ticket.issue", json!({ "purpose": "relay" })).await;
        let (relayed, _) = open(&served, &format!("?ticket={}", relay["ticket"].as_str().unwrap()));
        let road = relayed.road();
        assert!(road.paired_road, "the ticket carries the minting socket's paired road");
        assert_eq!(road.stamped, Some(WorkspaceOrigin::Relayed));
        assert!(!road.as_paired(), "a relay ticket's word is stricter and stays");
        assert_eq!(road.origin(Some(WorkspaceOrigin::Here)), Some(Caller::Road(WorkspaceOrigin::Relayed)));
    })
}

#[test]
fn a_malformed_op_on_an_authed_socket_is_refused_and_it_stays() {
    // D11. serve.test.ts:74
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = host_socket(&served).await;
        let bad = request_unchecked(&socket, &sink, 5, "workspaces.get", json!({})).await;
        assert_eq!(bad["ok"], json!(false));
        assert_eq!(bad["id"], json!(5));
        let issue: Value = serde_json::from_str(bad["error"].as_str().unwrap()).expect("the wire carries zod's message, a JSON list");
        assert_eq!(issue[0]["path"], json!(["workspaceId"]));
        assert_eq!(sink.closed(), None);
        assert!(rt.script.calls_to("workspaces.get").is_empty());
    })
}

#[test]
fn a_schema_refusal_before_auth_closes() {
    // D11. serve.test.ts:67
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        send(&socket, json!({ "id": 1, "op": "no.such.op" })).await; // refused by the schema
        assert_eq!(sink.frames()[0]["ok"], json!(false));
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
    })
}

// ---- Before the socket is let in ----

#[test]
fn a_wrong_token_closes_4401() {
    // D19. serve.test.ts:37
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, dopts(&rt));
        let (socket, sink) = open(&served, "");
        let refused = request(&socket, &sink, 1, "auth", json!({ "token": "wrong" })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": UNAUTHORIZED, "kind": "auth" }));
        assert_eq!(sink.last_text(), format!(r#"{{"id":1,"ok":false,"error":"{UNAUTHORIZED}","kind":"auth"}}"#));
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert_eq!(
            rt.script.calls_to("devices.match").iter().map(|c| c.args.clone()).collect::<Vec<_>>(),
            vec![json!({ "token": "wrong" })],
            "a token that is not the host's is asked of the device door once"
        );
    })
}

#[test]
fn the_first_op_not_auth_closes_4401() {
    // D19. serve.test.ts:67
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let refused = request(&socket, &sink, 1, "workspaces.list", json!({})).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": UNAUTHORIZED, "kind": "auth" }));
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert!(rt.script.calls_to("workspaces.list").is_empty());
    })
}

#[test]
fn auth_names_the_release_and_road_only_where_given() {
    // D19, P1. serve.test.ts:54
    block(async {
        let rt = FakeRuntime::new();
        let released = Released { version: Some("0.4.2".into()), road: Some("service".into()) };
        let served = served_with(&rt, ServeOptions { released: Some(released), ..opts() });
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "auth", json!({ "token": TOKEN })).await;
        assert_eq!(sink.last_text(), r#"{"id":1,"ok":true,"version":"0.4.2","road":"service"}"#);
        request(&socket, &sink, 2, "auth", json!({ "token": TOKEN })).await;
        assert_eq!(sink.last_text(), r#"{"id":2,"ok":true,"version":"0.4.2","road":"service"}"#);

        let bare = served_with(&rt, opts());
        let (socket, sink) = open(&bare, "");
        assert_eq!(request(&socket, &sink, 1, "auth", json!({ "token": TOKEN })).await, json!({ "id": 1, "ok": true }));
    })
}

#[test]
fn a_scoped_token_off_the_hosts_road_is_refused_before_seen() {
    // D19. serve.ts:828
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, ServeOptions { own_road: Some(Rc::new(|_| false)), ..dopts(&rt) });
        rt.script.once::<Result<Option<HeldDevice>, HostError>>(
            "devices.match",
            Ok(Some(held("d-9", Some(scope("t-9", "ws-9")), Some(WorkspaceOrigin::Here)))),
        );
        let (socket, sink) = open(&served, "");
        let refused = request(&socket, &sink, 1, "auth", json!({ "token": "scoped" })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": SCOPED_TOKEN_ROAD_REFUSAL, "kind": "auth" }));
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert!(rt.script.calls_to("devices.seen").is_empty(), "a copy refused at the door moves no last seen");
        assert!(!socket.authed());
    })
}

#[test]
fn a_device_auth_moves_its_last_seen_and_the_host_token_does_not() {
    // D19. serve.ts:844
    block(async {
        let rt = FakeRuntime::new();
        let now = Rc::new(RefCell::new(42i64));
        let served = served_now(&rt, now, dopts(&rt));
        let _ = host_socket(&served).await;
        assert!(rt.script.calls_to("devices.seen").is_empty());
        let _ = device_socket(&rt, &served, held("d-1", None, None)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            rt.script.calls_to("devices.seen").iter().map(|c| c.args.clone()).collect::<Vec<_>>(),
            vec![json!({ "id": "d-1", "now": 42 })]
        );
    })
}

/// A device door whose match takes a while, so a first frame is still being decided when a second one arrives.
struct SlowDevices;

#[async_trait::async_trait(?Send)]
impl wsp_core::devices::DeviceDoor for SlowDevices {
    async fn issue(&self, _: i64, _: u64, _: bool) -> Result<wsp_core::devices::IssuedCode, HostError> {
        Err(HostError::new("slow door"))
    }
    async fn redeem(&self, _: &str, _: &str, _: i64) -> Result<Option<PairedDevice>, HostError> {
        Ok(None)
    }
    async fn spend(&self, _: &str, _: i64) -> Result<bool, HostError> {
        Ok(false)
    }
    async fn admit(&self, _: &str, _: i64) -> Result<PairedDevice, HostError> {
        Err(HostError::new("slow door"))
    }
    async fn admit_account(&self, _: &str, _: wsp_wire::wire::devices::DeviceVia, _: i64) -> Result<PairedDevice, HostError> {
        Err(HostError::new("slow door"))
    }
    async fn refuses(&self, _: &str) -> Result<bool, HostError> {
        Ok(false)
    }
    async fn mint(&self, _: &str, _: ThreadScope, _: i64, _: wsp_core::devices::MintMore) -> Result<PairedDevice, HostError> {
        Err(HostError::new("slow door"))
    }
    async fn revoke_asides(&self) -> Result<(), HostError> {
        Ok(())
    }
    async fn r#match(&self, _: &str) -> Result<Option<HeldDevice>, HostError> {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Ok(None)
    }
    async fn seen(&self, _: &str, _: i64) -> Result<Option<HeldDevice>, HostError> {
        Ok(None)
    }
    async fn list(&self) -> Result<Vec<DeviceView>, HostError> {
        Ok(vec![])
    }
    async fn revoke(&self, _: &str) -> Result<bool, HostError> {
        Ok(false)
    }
}

#[test]
fn a_second_frame_while_the_first_is_decided_closes() {
    // D13. serve.ts:709
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, ServeOptions { devices: Some(Rc::new(SlowDevices)), ..opts() });
        let (socket, sink) = open(&served, "");
        let first = socket.on_message(Incoming::Text(json!({ "id": 1, "op": "auth", "token": "slow" }).to_string()));
        let second = socket.on_message(Incoming::Text(json!({ "id": 2, "op": "auth", "token": TOKEN }).to_string()));
        futures::future::join(first, second).await;
        assert_eq!(sink.reply(2), Some(json!({ "id": 2, "ok": false, "error": UNAUTHORIZED, "kind": "auth" })));
        assert!(!socket.authed(), "the host token racing past the door lets nothing in");
    })
}

// ---- seal.open ----

/// The two ends of one agreed key: the host's (handed to the door by the fake place door) and the client's.
fn seal_pair() -> (Seal, Seal) {
    let keys = seal_keys(&[7u8; 32], SEAL_CLIENT);
    (make_seal(&keys, Side::Host), make_seal(&keys, Side::Place))
}

fn challenge(seal: Seal, expect: &[u8]) -> PlaceChallenge {
    PlaceChallenge {
        nonce: "host-nonce".into(),
        host_public_key: "host-public".into(),
        signature: "host-sig".into(),
        ephemeral: "host-half".into(),
        expect: expect.to_vec(),
        seal,
    }
}

async fn send_sealed(socket: &Rc<wsp_serve::serve::socket::Socket>, client: &mut Seal, frame: Value) {
    let bytes = client.seal(&frame.to_string());
    socket.on_message(Incoming::Binary(bytes)).await;
}

fn opened_replies(sink: &RecordingSink, client: &mut Seal) -> Vec<Value> {
    sink.opened_with(client)
}

#[test]
fn seal_open_answers_in_the_clear_then_seals_every_frame() {
    // D14, W10. serve.test.ts:216
    block(async {
        let rt = FakeRuntime::with_places();
        rt.script.on("workspaces.list", |_| Ok::<Vec<wsp_wire::views::workspace_more::WorkspaceView>, HostError>(vec![]));
        let (host_seal, mut client) = seal_pair();
        rt.script.once::<Option<PlaceChallenge>>("places.answerChallenge", Some(challenge(host_seal, b"transcript")));
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let opened = request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        assert_eq!(
            sink.last_text(),
            r#"{"id":1,"ok":true,"nonce":"host-nonce","hostPublicKey":"host-public","signature":"host-sig","ephemeral":"host-half"}"#
        );
        assert_eq!(opened["ok"], json!(true));
        assert_eq!(
            rt.script.calls_to("places.answerChallenge")[0].args,
            json!({ "subject": SEAL_CLIENT, "nonce": b64(32, 1), "ephemeral": b64(32, 2) })
        );

        send_sealed(&socket, &mut client, json!({ "id": 2, "op": "auth", "token": TOKEN })).await;
        send_sealed(&socket, &mut client, json!({ "id": 3, "op": "workspaces.list" })).await;
        let replies = opened_replies(&sink, &mut client);
        assert_eq!(replies, vec![json!({ "id": 2, "ok": true }), json!({ "id": 3, "ok": true, "workspaces": [] })]);
        assert_eq!(sink.texts.borrow().len(), 1, "nothing after the seal is written in the clear");
        // A plaintext frame after the seal is a carrier writing into the link.
        send(&socket, json!({ "id": 4, "op": "workspaces.list" })).await;
        let after = opened_replies(&sink, &mut client);
        assert_eq!(after.last().unwrap(), &json!({ "id": null, "ok": false, "error": "invalid json" }));
    })
}

#[test]
fn seal_open_leaves_both_limits_standing() {
    // D5, D14. serve.ts:711-726 (no throughDoor)
    block(async {
        let rt = FakeRuntime::with_places();
        let (host_seal, _client) = seal_pair();
        rt.script.once::<Option<PlaceChallenge>>("places.answerChallenge", Some(challenge(host_seal, b"t")));
        let served = served_with(&rt, ServeOptions { auth_deadline_ms: Some(60), ..opts() });
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        socket.bytes_arrived(PRE_AUTH_MAX_BYTES as usize + 1);
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, DAEMON_PRE_AUTH_BYTES_EXCEEDED.to_string())));
    })
}

#[test]
fn a_second_seal_open_closes_4401() {
    // D14. serve.test.ts:233
    block(async {
        let rt = FakeRuntime::with_places();
        let (host_seal, mut client) = seal_pair();
        rt.script.once::<Option<PlaceChallenge>>("places.answerChallenge", Some(challenge(host_seal, b"t")));
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        send_sealed(&socket, &mut client, json!({ "id": 2, "op": "seal.open", "nonce": b64(32, 3), "ephemeral": b64(32, 4) })).await;
        assert_eq!(opened_replies(&sink, &mut client), vec![json!({ "id": 2, "ok": false, "error": UNAUTHORIZED, "kind": "auth" })]);
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert_eq!(rt.script.calls_to("places.answerChallenge").len(), 1);
    })
}

#[test]
fn seal_open_after_auth_is_refused_and_the_socket_stays() {
    // D14, P3. serve.test.ts:241
    block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, opts());
        let (socket, sink) = host_socket(&served).await;
        let late = request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        assert_eq!(late, json!({ "id": 1, "ok": false, "error": UNAUTHORIZED }));
        assert_eq!(sink.closed(), None);
    })
}

#[test]
fn seal_open_with_no_place_door_says_seal_unserved() {
    // D14. serve.test.ts:265
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let refused = request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": SEAL_UNSERVED, "kind": "auth" }));
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
    })
}

#[test]
fn a_challenge_the_door_cannot_answer_closes_unauthorized() {
    // D14. serve.ts:718
    block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let refused = request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": UNAUTHORIZED, "kind": "auth" }));
    })
}

// ---- place.join, place.auth, place.prove ----

#[test]
fn a_place_handshake_on_a_sealed_socket_closes() {
    // D15. serve.test.ts:247
    block(async {
        let rt = FakeRuntime::with_places();
        let (host_seal, mut client) = seal_pair();
        rt.script.once::<Option<PlaceChallenge>>("places.answerChallenge", Some(challenge(host_seal, b"t")));
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        send_sealed(
            &socket,
            &mut client,
            json!({ "id": 2, "op": "place.auth", "placeId": "p_1", "nonce": b64(32, 1), "ephemeral": b64(32, 2) }),
        )
        .await;
        assert_eq!(opened_replies(&sink, &mut client), vec![json!({ "id": 2, "ok": false, "error": UNAUTHORIZED, "kind": "auth" })]);
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert!(rt.script.calls_to("places.auth").is_empty(), "the door is not asked");
    })
}

#[test]
fn a_join_code_not_held_says_the_code_refusal() {
    // D15. serve.ts:742
    block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<Option<wsp_machines::places::types::JoinAnswer>, HostError>>("places.join", Ok(None));
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let refused =
            request(&socket, &sink, 1, "place.join", json!({ "publicKey": b64(44, 5), "nonce": b64(32, 1), "ephemeral": b64(32, 2) }))
                .await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": PLACE_CODE_REFUSAL, "kind": "auth" }));
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
    })
}

#[test]
fn a_place_auth_refusal_carries_the_doors_signed_fields() {
    // D15. serve.ts:744
    block(async {
        let rt = FakeRuntime::with_places();
        let signed: wsp_wire::wire::place_link::PlaceAuthRefusal =
            serde_json::from_value(json!({ "hostPublicKey": b64(44, 9), "signature": b64(64, 10) })).unwrap();
        rt.script.once::<Result<AuthAnswer, HostError>>(
            "places.auth",
            Ok(AuthAnswer::Refusal { refusal: "no such place".into(), signed: signed.clone() }),
        );
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "place.auth", json!({ "placeId": "p_1", "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        let mut expected = json!({ "id": 1, "ok": false, "error": "no such place", "kind": "auth" });
        for (k, v) in serde_json::to_value(&signed).unwrap().as_object().unwrap() {
            expected[k] = v.clone();
        }
        assert_eq!(sink.reply(1), Some(expected));
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
    })
}

#[test]
fn a_place_join_with_no_place_door_refuses_in_the_doors_sentence() {
    // D15. serve.ts:736-740 (places() throws NO_PLACE_DOOR inside the try)
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let refused =
            request(&socket, &sink, 1, "place.join", json!({ "publicKey": b64(44, 5), "nonce": b64(32, 1), "ephemeral": b64(32, 2) }))
                .await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": wsp_machines::places::types::NO_PLACE_DOOR, "kind": "auth" }));
    })
}

#[test]
fn a_place_that_proved_is_handed_over_and_reads_nothing_more() {
    // D12, D15. places-link.test.ts (the door half), serve.ts:684-708, 745-754
    block(async {
        let rt = FakeRuntime::with_places();
        let (host_seal, mut client) = seal_pair();
        let reply: wsp_wire::wire::place_link::PlaceAuthReply = serde_json::from_value(
            json!({ "nonce": b64(32, 8), "hostPublicKey": b64(44, 9), "signature": b64(64, 10), "ephemeral": b64(32, 11) }),
        )
        .unwrap();
        rt.script.once::<Result<AuthAnswer, HostError>>(
            "places.auth",
            Ok(AuthAnswer::Reply { reply: reply.clone(), expect: b"to-sign".to_vec(), seal: host_seal }),
        );
        let proved_report: wsp_wire::wire::place_link::PlaceReport = serde_json::from_value(report()).expect("a report the schema takes");
        rt.script.once::<Result<Proved, HostError>>("places.prove", Ok(Proved::Report { report: proved_report, device: None }));
        rt.script.once::<Result<(), HostError>>("places.attach", Ok(()));
        let served = served_with(&rt, ServeOptions { auth_deadline_ms: Some(60), ..opts() });
        let (socket, sink) = open(&served, "");
        let answered =
            request(&socket, &sink, 1, "place.auth", json!({ "placeId": "p_1", "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        let mut expected = json!({ "id": 1, "ok": true });
        for (k, v) in serde_json::to_value(&reply).unwrap().as_object().unwrap() {
            expected[k] = v.clone();
        }
        assert_eq!(answered, expected, "the reply rides in the clear");
        send_sealed(&socket, &mut client, json!({ "id": 2, "op": "place.prove", "signature": b64(64, 6), "report": report() })).await;
        assert_eq!(opened_replies(&sink, &mut client), vec![json!({ "id": 2, "ok": true })]);
        assert!(sink.paused.get(), "the socket stops being read before the link takes it");
        assert!(sink.linked.get(), "the socket is handed to the place door");
        let prove = &rt.script.calls_to("places.prove")[0];
        assert_eq!(prove.args["placeId"], json!("p_1"));
        assert_eq!(prove.args["from"], json!("127.0.0.1"));
        assert_eq!(
            rt.script.names().iter().filter(|n| n.starts_with("places.")).cloned().collect::<Vec<_>>(),
            vec!["places.auth", "places.prove", "places.attach"]
        );
        // Nothing this socket sends after the handover reaches the door, and the deadline is off.
        send_sealed(&socket, &mut client, json!({ "id": 3, "op": "auth", "token": TOKEN })).await;
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        assert_eq!(opened_replies(&sink, &mut client).len(), 1);
        assert_eq!(sink.closed(), None);
    })
}

#[test]
fn a_prove_refused_closes_4401() {
    // D12. serve.ts:690-693
    block(async {
        let rt = FakeRuntime::with_places();
        let (host_seal, mut client) = seal_pair();
        let reply: wsp_wire::wire::place_link::PlaceAuthReply = serde_json::from_value(
            json!({ "nonce": b64(32, 8), "hostPublicKey": b64(44, 9), "signature": b64(64, 10), "ephemeral": b64(32, 11) }),
        )
        .unwrap();
        rt.script
            .once::<Result<AuthAnswer, HostError>>("places.auth", Ok(AuthAnswer::Reply { reply, expect: b"x".to_vec(), seal: host_seal }));
        rt.script.once::<Result<Proved, HostError>>("places.prove", Err(HostError::new("the signature does not verify")));
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "place.auth", json!({ "placeId": "p_1", "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        send_sealed(&socket, &mut client, json!({ "id": 2, "op": "place.prove", "signature": b64(64, 7), "report": report() })).await;
        assert_eq!(
            opened_replies(&sink, &mut client),
            vec![json!({ "id": 2, "ok": false, "error": "the signature does not verify", "kind": "auth" })]
        );
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert!(rt.script.calls_to("places.attach").is_empty());
    })
}

#[test]
fn while_proving_any_other_op_closes() {
    // D12. serve.ts:685
    block(async {
        let rt = FakeRuntime::with_places();
        let (host_seal, mut client) = seal_pair();
        let reply: wsp_wire::wire::place_link::PlaceAuthReply = serde_json::from_value(
            json!({ "nonce": b64(32, 8), "hostPublicKey": b64(44, 9), "signature": b64(64, 10), "ephemeral": b64(32, 11) }),
        )
        .unwrap();
        rt.script
            .once::<Result<AuthAnswer, HostError>>("places.auth", Ok(AuthAnswer::Reply { reply, expect: b"x".to_vec(), seal: host_seal }));
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "place.auth", json!({ "placeId": "p_1", "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
        send_sealed(&socket, &mut client, json!({ "id": 2, "op": "auth", "token": TOKEN })).await;
        assert_eq!(opened_replies(&sink, &mut client), vec![json!({ "id": 2, "ok": false, "error": UNAUTHORIZED, "kind": "auth" })]);
        assert!(!socket.authed(), "a socket asking for a second identity is let in as neither");
    })
}

#[test]
fn a_prove_with_nothing_to_prove_closes() {
    // D16. serve.ts:756
    block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let refused = request(&socket, &sink, 1, "place.prove", json!({ "signature": b64(64, 6), "report": report() })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": UNAUTHORIZED, "kind": "auth" }));
        assert!(rt.script.calls_to("places.prove").is_empty());
    })
}

// ---- pair.redeem ----

fn paired(id: &str) -> PairedDevice {
    PairedDevice { device_id: id.into(), device_token: format!("token-{id}"), device: device(id) }
}

#[test]
fn a_redeem_with_no_device_door_reads_as_a_code_not_held() {
    // D17. serve.ts:757-763
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, sink) = open(&served, "");
        let refused = request(&socket, &sink, 1, "pair.redeem", json!({ "code": "c", "name": "laptop" })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": PAIR_CODE_REFUSAL, "kind": "auth" }));
        // A store that fails reads the same.
        let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
        let with_door = served_with(&rt, ServeOptions { devices: Some(devices), ..opts() });
        rt.script.once::<Result<Option<PairedDevice>, HostError>>("devices.redeem", Err(HostError::new("disk full")));
        let (socket, sink) = open(&with_door, "");
        let refused = request(&socket, &sink, 1, "pair.redeem", json!({ "code": "c", "name": "laptop" })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": PAIR_CODE_REFUSAL, "kind": "auth" }));
    })
}

#[test]
fn a_paid_redeem_lets_in_and_binds_without_release_fields() {
    // D17, D25. serve.ts:764-768
    block(async {
        let rt = FakeRuntime::new();
        let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
        let now = Rc::new(RefCell::new(77i64));
        let released = Released { version: Some("0.4.2".into()), road: None };
        let served = served_now(&rt, now, ServeOptions { devices: Some(devices), released: Some(released), ..opts() });
        rt.script.once::<Result<Option<PairedDevice>, HostError>>("devices.redeem", Ok(Some(paired("d-7"))));
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "pair.redeem", json!({ "code": "c", "name": "laptop" })).await;
        assert_eq!(sink.last_text(), r#"{"id":1,"ok":true,"deviceId":"d-7","deviceToken":"token-d-7"}"#);
        assert_eq!(rt.script.calls_to("devices.redeem")[0].args, json!({ "code": "c", "name": "laptop", "now": 77 }));
        let road = socket.road();
        assert!(road.paired_road);
        assert!(matches!(road.me, Some(Authed::Device(ref d)) if d.view.id == "d-7"));
        // Bound: a revoke of that device cuts this socket.
        rt.script.once::<Result<bool, HostError>>("devices.revoke", Ok(true));
        assert!(served.revoke_device("d-7", None).await.unwrap());
        assert_eq!(sink.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
    })
}

// ---- device.auth ----

struct Account {
    signer: Option<Signer>,
    list: Option<Vec<AccountDevice>>,
    refreshed: RefCell<u32>,
    after_refresh: Option<Vec<AccountDevice>>,
}

#[async_trait::async_trait(?Send)]
impl AdmittedDevices for Account {
    fn signer(&self) -> Option<Signer> {
        self.signer.clone()
    }
    fn list(&self) -> Option<Vec<AccountDevice>> {
        if *self.refreshed.borrow() > 0 && self.after_refresh.is_some() {
            return self.after_refresh.clone();
        }
        self.list.clone()
    }
    async fn refresh(&self) -> Result<(), HostError> {
        *self.refreshed.borrow_mut() += 1;
        Err(HostError::new("the relay is down"))
    }
}

/// A signer key, a device key, the device's row on the account with one admission the signer signed, and the
/// transcript the socket's seal.open will hand the door.
struct DeviceAuthCase {
    signer: Signer,
    device_public: String,
    device_private: String,
    row: AccountDevice,
    expect: Vec<u8>,
}

fn device_auth_case() -> DeviceAuthCase {
    let signer_keys = new_place_key_pair();
    let device_keys = new_place_key_pair();
    let signer = Signer { fingerprint: key_fingerprint(&signer_keys.public_key), public_key: signer_keys.public_key.clone() };
    let fp = key_fingerprint(&device_keys.public_key);
    let issued_at = "2026-10-25T00:00:00.000Z";
    let signature =
        sign_place_bytes(&signer_keys.private_key_pem, &device_admission_transcript(&fp, &signer.fingerprint, issued_at)).unwrap();
    let row: AccountDevice = serde_json::from_value(json!({
        "id": "relay-d-1", "name": "laptop", "fingerprint": fp,
        "admissions": [{ "by": signer.fingerprint, "issuedAt": issued_at, "signature": signature }]
    }))
    .expect("an account row with one admission");
    DeviceAuthCase {
        signer,
        device_public: device_keys.public_key,
        device_private: device_keys.private_key_pem,
        row,
        expect: b"socket-transcript".to_vec(),
    }
}

async fn device_auth(
    rt: &Rc<FakeRuntime>,
    account: Option<Account>,
    case: &DeviceAuthCase,
    sign_this: &[u8],
) -> (Rc<wsp_serve::serve::socket::Socket>, Rc<RecordingSink>, Value) {
    let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
    let admitted = account.map(|a| Rc::new(a) as Rc<dyn AdmittedDevices>);
    let served = served_with(
        rt,
        ServeOptions { devices: Some(devices), admitted, released: Some(Released { version: Some("0.4.2".into()), road: None }), ..opts() },
    );
    let (host_seal, mut client) = seal_pair();
    rt.script.once::<Option<PlaceChallenge>>("places.answerChallenge", Some(challenge(host_seal, &case.expect)));
    let (socket, sink) = open(&served, "");
    request(&socket, &sink, 1, "seal.open", json!({ "nonce": b64(32, 1), "ephemeral": b64(32, 2) })).await;
    let signature = sign_place_bytes(&case.device_private, sign_this).unwrap();
    send_sealed(
        &socket,
        &mut client,
        json!({ "id": 2, "op": "device.auth", "name": "laptop", "publicKey": case.device_public, "signature": signature }),
    )
    .await;
    let reply = opened_replies(&sink, &mut client).pop().expect("device.auth is answered");
    (socket, sink, reply)
}

#[test]
fn device_auth_refuses_each_check_in_order() {
    // D18. pairing.test.ts (the door half), serve.ts:770-813
    block(async {
        let case = device_auth_case();
        let refusal = |e: &str| json!({ "id": 2, "ok": false, "error": e, "kind": "auth" });

        // No account door.
        let rt = FakeRuntime::with_places();
        let (_, _, reply) = device_auth(&rt, None, &case, &case.expect).await;
        assert_eq!(reply, refusal(DEVICE_ACCOUNT_UNSERVED));

        // No signer.
        let rt = FakeRuntime::with_places();
        let (_, _, reply) = device_auth(
            &rt,
            Some(Account { signer: None, list: Some(vec![case.row.clone()]), refreshed: RefCell::new(0), after_refresh: None }),
            &case,
            &case.expect,
        )
        .await;
        assert_eq!(reply, refusal(DEVICE_ACCOUNT_UNSERVED));

        // A key the listing lacks, asked once more and still lacking.
        let rt = FakeRuntime::with_places();
        let account = Account { signer: Some(case.signer.clone()), list: Some(vec![]), refreshed: RefCell::new(0), after_refresh: None };
        let (_, _, reply) = device_auth(&rt, Some(account), &case, &case.expect).await;
        assert_eq!(reply, refusal(DEVICE_AUTH_REFUSAL));

        // A signature over other bytes than this socket's transcript.
        let rt = FakeRuntime::with_places();
        rt.script.on("devices.list", |_| Ok::<Vec<DeviceView>, HostError>(vec![]));
        let account = Account {
            signer: Some(case.signer.clone()),
            list: Some(vec![case.row.clone()]),
            refreshed: RefCell::new(0),
            after_refresh: None,
        };
        let (_, _, reply) = device_auth(&rt, Some(account), &case, b"bytes the device chose").await;
        assert_eq!(reply, refusal(DEVICE_AUTH_REFUSAL));
        assert!(rt.script.calls_to("devices.admitAccount").is_empty());

        // A key this host took away.
        let rt = FakeRuntime::with_places();
        rt.script.on("devices.list", |_| Ok::<Vec<DeviceView>, HostError>(vec![]));
        rt.script.once::<Result<bool, HostError>>("devices.refuses", Ok(true));
        let account = Account {
            signer: Some(case.signer.clone()),
            list: Some(vec![case.row.clone()]),
            refreshed: RefCell::new(0),
            after_refresh: None,
        };
        let (_, _, reply) = device_auth(&rt, Some(account), &case, &case.expect).await;
        assert_eq!(reply, refusal(DEVICE_REVOKED_REFUSAL));
        assert!(rt.script.calls_to("devices.admitAccount").is_empty());
    })
}

#[test]
fn device_auth_with_no_seal_open_is_unauthorized() {
    // D18. serve.ts:780
    block(async {
        let case = device_auth_case();
        let rt = FakeRuntime::with_places();
        let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
        let account: Rc<dyn AdmittedDevices> = Rc::new(Account {
            signer: Some(case.signer.clone()),
            list: Some(vec![case.row.clone()]),
            refreshed: RefCell::new(0),
            after_refresh: None,
        });
        let served = served_with(&rt, ServeOptions { devices: Some(devices), admitted: Some(account), ..opts() });
        let (socket, sink) = open(&served, "");
        let reply = request(
            &socket,
            &sink,
            1,
            "device.auth",
            json!({ "name": "laptop", "publicKey": case.device_public, "signature": b64(64, 12) }),
        )
        .await;
        assert_eq!(reply, json!({ "id": 1, "ok": false, "error": UNAUTHORIZED, "kind": "auth" }));
    })
}

#[test]
fn device_auth_admits_and_names_the_release() {
    // D18, D25. serve.ts:814-822
    block(async {
        let case = device_auth_case();
        let rt = FakeRuntime::with_places();
        rt.script.on("devices.list", |_| Ok::<Vec<DeviceView>, HostError>(vec![]));
        rt.script.once::<Result<PairedDevice, HostError>>("devices.admitAccount", Ok(paired("d-acct")));
        // The listing lacks the key until one more beat, whose own failure is swallowed.
        let account = Account {
            signer: Some(case.signer.clone()),
            list: None,
            refreshed: RefCell::new(0),
            after_refresh: Some(vec![case.row.clone()]),
        };
        let (socket, _sink, reply) = device_auth(&rt, Some(account), &case, &case.expect).await;
        assert_eq!(reply.to_string(), r#"{"id":2,"ok":true,"deviceId":"d-acct","deviceToken":"token-d-acct","version":"0.4.2"}"#);
        let via = &rt.script.calls_to("devices.admitAccount")[0].args["via"];
        assert_eq!(
            via.to_string(),
            json!({ "kind": "account", "relayDeviceId": "relay-d-1", "fingerprint": key_fingerprint(&case.device_public), "publicKey": case.device_public, "admittedBy": case.signer.fingerprint }).to_string()
        );
        assert!(socket.authed());
        assert!(socket.road().paired_road);
    })
}

// ---- The refusal log ----

fn served_logging(rt: &Rc<FakeRuntime>) -> (Rc<Served>, Rc<RefCell<Vec<String>>>) {
    let (log, lines) = log_lines();
    (served_with(rt, ServeOptions { log: Some(log), ..opts() }), lines)
}

#[test]
fn a_refusal_is_logged_once_as_op_kind_and_first_line() {
    // D20, D21. serve-refusal.test.ts:61
    block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorkspaceView, HostError>>(
            "workspaces.get",
            Err(wsp_words::exit::refusal(
                "no workspace nope\nsecond line",
                "Pick one from the list.",
                Some(wsp_words::exit::Kind::NotFound),
            )),
        );
        let (served, lines) = served_logging(&rt);
        let (socket, sink) = host_socket(&served).await;
        request(&socket, &sink, 1, "workspaces.get", json!({ "workspaceId": "nope" })).await;
        assert_eq!(lines.borrow().clone(), vec!["refused workspaces.get kind=not-found: no workspace nope".to_string()]);
    })
}

#[test]
fn a_refused_auth_never_logs_its_token() {
    // D24. serve-refusal.test.ts:80
    block(async {
        let rt = FakeRuntime::new();
        let (served, lines) = served_logging(&rt);
        let (socket, sink) = open(&served, "");
        request(&socket, &sink, 1, "auth", json!({ "token": "wrong-token-sk-ant-x" })).await;
        assert_eq!(lines.borrow().clone(), vec![format!("refused auth kind=auth: {UNAUTHORIZED}")]);
    })
}

#[test]
fn the_sentence_is_cut_at_400_and_says_so() {
    // D21. serve-refusal.test.ts:90
    let payload = json!({ "id": 1, "ok": false, "error": format!("no workspace {}", "w".repeat(3_000)) });
    let frame = json!({ "id": 1, "op": "workspaces.get", "workspaceId": "w".repeat(3_000) });
    let line = refused_line(Some(&frame), payload.as_object().unwrap(), None);
    let (prefix, message) = line.split_once(": ").unwrap();
    assert_eq!(prefix, "refused workspaces.get kind=none");
    let cut = 13 + 3_000 - 400;
    assert_eq!(message, format!("{} (cut {cut} characters)", &format!("no workspace {}", "w".repeat(3_000))[..400]));
}

#[test]
fn a_schema_refusal_logs_its_issues_line_while_the_wire_keeps_zods() {
    // D11, D21. serve-refusal.test.ts:102
    block(async {
        let rt = FakeRuntime::new();
        let (served, lines) = served_logging(&rt);
        let (socket, sink) = host_socket(&served).await;
        let got = request_unchecked(&socket, &sink, 1, "init.keys", json!({ "provider": 42 })).await;
        assert_eq!(got["ok"], json!(false));
        assert!(got["error"].as_str().unwrap().starts_with('['), "the wire keeps zod's message");
        assert_eq!(lines.borrow().clone(), vec!["refused init.keys kind=none: provider: Expected string, received number".to_string()]);
    })
}

#[test]
fn a_padded_key_is_blanked_and_a_repeated_key_never_logs() {
    // D21, D23. serve-refusal.test.ts:70, 110
    const KEY: &str = "sk-ant-x-fake-refusal-key";
    const ROW: &str = "sk-ant-x-fake-row-login";
    let payload =
        json!({ "id": 1, "ok": false, "error": format!("Box refused {KEY} and {ROW}. Check it and paste it again."), "kind": "auth" });
    let frame = json!({ "id": 1, "op": "init.keys", "provider": "box", "key": format!("  {KEY}\n"), "rows": { "logins/claude": ROW } });
    let line = refused_line(Some(&frame), payload.as_object().unwrap(), None);
    assert_eq!(line, "refused init.keys kind=auth: Box refused <redacted> and <redacted>. Check it and paste it again.");
}

#[test]
fn an_undeclared_op_logs_as_frame() {
    // D20. serve-refusal.test.ts:119
    let key = "sk-ant-x-fake-refusal-key";
    let payload = json!({ "id": 1, "ok": false, "error": "bad op" });
    let line = refused_line(Some(&json!({ "id": 1, "op": key })), payload.as_object().unwrap(), Some("op: Invalid discriminator value"));
    assert_eq!(line, "refused frame kind=none: op: Invalid discriminator value");
    assert!(!line.contains(key));
    assert_eq!(refused_line(None, payload.as_object().unwrap(), None), "refused frame kind=none: bad op");
}

#[test]
fn short_secret_values_stay_readable() {
    // D23. serve-refusal.test.ts:128
    let payload = json!({ "id": 1, "ok": false, "error": "Box refused. Check it and paste it again.", "kind": "auth" });
    let line =
        refused_line(Some(&json!({ "id": 1, "op": "init.keys", "provider": "box", "key": " " })), payload.as_object().unwrap(), None);
    assert_eq!(line, "refused init.keys kind=auth: Box refused. Check it and paste it again.");
    let auth = json!({ "id": 1, "ok": false, "error": UNAUTHORIZED, "kind": "auth" });
    assert_eq!(
        refused_line(Some(&json!({ "id": 1, "op": "auth", "token": "u" })), auth.as_object().unwrap(), None),
        "refused auth kind=auth: unauthorized"
    );
}

#[test]
fn more_than_256_secrets_withhold_the_sentence() {
    // D22. serve-refusal.test.ts:141
    let env: serde_json::Map<String, Value> = (0..20_000).map(|i| (format!("V{i}"), json!(format!("value-{i}-sk-ant-x")))).collect();
    let frame = json!({ "id": 1, "op": "workspaces.get", "workspaceId": "w", "env": env });
    let payload = json!({ "id": 1, "ok": false, "error": "anything", "kind": "not-found" });
    let started = std::time::Instant::now();
    let line = refused_line(Some(&frame), payload.as_object().unwrap(), None);
    assert!(started.elapsed() < std::time::Duration::from_millis(1_500));
    assert_eq!(line, "refused workspaces.get kind=not-found: (sentence withheld, 20000 secret values)");
    // Exactly 256 distinct values are still scanned.
    let env: serde_json::Map<String, Value> = (0..256).map(|i| (format!("V{i}"), json!(format!("value-{i}-x")))).collect();
    let line = refused_line(Some(&json!({ "id": 1, "op": "workspaces.get", "env": env })), payload.as_object().unwrap(), None);
    assert_eq!(line, "refused workspaces.get kind=not-found: anything");
}

#[test]
fn a_deeply_nested_secret_is_refused_and_logged_and_the_socket_keeps_answering() {
    // D11, D20. serve-refusal.test.ts:153
    block(async {
        let rt = FakeRuntime::new();
        rt.script.on("workspaces.list", |_| Ok::<Vec<wsp_wire::views::workspace_more::WorkspaceView>, HostError>(vec![]));
        let (served, lines) = served_logging(&rt);
        let (socket, sink) = host_socket(&served).await;
        let raw = format!(r#"{{"id":90,"op":"init.keys","key":{}"x"{}}}"#, "[".repeat(20_000), "]".repeat(20_000));
        send_raw(&socket, &raw).await;
        assert_eq!(sink.reply(90).unwrap()["ok"], json!(false));
        assert_eq!(request(&socket, &sink, 91, "workspaces.list", json!({})).await, json!({ "id": 91, "ok": true, "workspaces": [] }));
        assert_eq!(lines.borrow().len(), 1);
        assert!(lines.borrow()[0].starts_with("refused init.keys kind=none: "));
    })
}

#[test]
fn an_ok_reply_logs_nothing() {
    // R4. serve.ts:625
    block(async {
        let rt = FakeRuntime::new();
        let (served, lines) = served_logging(&rt);
        let _ = host_socket(&served).await;
        assert!(lines.borrow().is_empty());
    })
}

// ---- Origin (invariant 3) ----

#[test]
fn invariant_3_a_threads_frames_are_stamped_with_its_mint_road() {
    // D31, D32. spawn-guard.test.ts; serve.ts:837-841
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, dopts(&rt));
        let by = scope("t-1", "ws-1");
        let (on_machine, _) = device_socket(&rt, &served, held("d-1", Some(by.clone()), Some(WorkspaceOrigin::Relayed))).await;
        assert_eq!(
            on_machine.road().origin(Some(WorkspaceOrigin::Here)),
            Some(Caller::Scoped { origin: WorkspaceOrigin::Relayed, by: by.clone() })
        );
        let (beside, _) = device_socket(&rt, &served, held("d-2", Some(by.clone()), Some(WorkspaceOrigin::Here))).await;
        assert_eq!(
            beside.road().origin(Some(WorkspaceOrigin::Relayed)),
            Some(Caller::Scoped { origin: WorkspaceOrigin::Here, by: by.clone() })
        );
        // A scoped record with no road is an older host's machine token: relayed.
        let (old, _) = device_socket(&rt, &served, held("d-3", Some(by.clone()), None)).await;
        assert_eq!(old.road().origin(None), Some(Caller::Scoped { origin: WorkspaceOrigin::Relayed, by }));
        assert!(!old.road().own_road(), "a thread is never the person's own road");
        assert!(!old.road().paired_road, "a thread's token is no paired computer");
    })
}

#[test]
fn invariant_3_a_threads_origin_reaches_the_verb_whatever_it_sends() {
    // R6, D31. spawn-guard.test.ts (the door half)
    block(async {
        let rt = FakeRuntime::new();
        rt.script.on("workspaces.list", |_| Ok::<Vec<wsp_wire::views::workspace_more::WorkspaceView>, HostError>(vec![]));
        let served = served_with(&rt, dopts(&rt));
        let by = scope("t-1", "ws-1");
        let (socket, sink) = device_socket(&rt, &served, held("d-1", Some(by.clone()), Some(WorkspaceOrigin::Relayed))).await;
        request(&socket, &sink, 1, "workspaces.list", json!({ "origin": "here" })).await;
        assert_eq!(
            rt.script.calls_to("workspaces.list")[0].args["origin"],
            json!({ "origin": "relayed", "by": serde_json::to_value(&by).unwrap() })
        );
    })
}

#[test]
fn invariant_3_a_paired_device_reads_paired_whatever_it_sends() {
    // D30, D33. pairing.test.ts
    block(async {
        let rt = FakeRuntime::new();
        rt.script.on("workspaces.list", |_| Ok::<Vec<wsp_wire::views::workspace_more::WorkspaceView>, HostError>(vec![]));
        let served = served_with(&rt, dopts(&rt));
        let (socket, sink) = device_socket(&rt, &served, held("d-1", None, None)).await;
        request(&socket, &sink, 1, "workspaces.list", json!({ "origin": "here" })).await;
        assert_eq!(rt.script.calls_to("workspaces.list")[0].args["origin"], json!("paired"));
        assert!(socket.road().own_road(), "a paired computer is still one of the person's own for the doors own_road guards");
    })
}

#[test]
fn invariant_3_a_browser_wsp_init_let_in_takes_the_persons_road() {
    // D25. serve.ts:609-610 (a device marked here is no paired computer)
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, dopts(&rt));
        let mut here = held("d-here", None, None);
        here.view.here = Some(wsp_frames::True);
        let (socket, _) = device_socket(&rt, &served, here).await;
        assert!(!socket.road().paired_road);
        assert_eq!(socket.road().origin(Some(WorkspaceOrigin::Here)), Some(Caller::Road(WorkspaceOrigin::Here)));
    })
}

#[test]
fn invariant_3_an_unstamped_socket_carries_its_clients_word() {
    // D30. serve.ts:855
    let road = Road { stamped: None, paired_road: false, me: Some(Authed::Host), by: None };
    assert_eq!(road.origin(Some(WorkspaceOrigin::Relayed)), Some(Caller::Road(WorkspaceOrigin::Relayed)));
    assert_eq!(road.origin(None), None);
    assert!(road.own_road());
    assert!(road.host_only());
}

// ---- Close ----

#[test]
fn a_close_runs_every_detach_once_in_order() {
    // D35. serve.ts:569-573
    block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, opts());
        let (socket, _sink) = host_socket(&served).await;
        let ran = Rc::new(RefCell::new(Vec::new()));
        for n in 0..3 {
            let ran = ran.clone();
            socket.detach(Box::new(move || ran.borrow_mut().push(n)));
        }
        socket.closed();
        socket.closed();
        let mine: Vec<i32> = ran.borrow().clone();
        assert_eq!(mine, vec![0, 1, 2]);
    })
}

#[test]
fn a_revoked_device_loses_every_socket_it_held_and_no_other() {
    // D25, W8. pairing.test.ts
    block(async {
        let rt = FakeRuntime::new();
        let devices: Rc<dyn wsp_core::devices::DeviceDoor> = rt.devices.clone();
        let served = served_with(&rt, ServeOptions { devices: Some(devices), ..opts() });
        let (_a, sink_a) = device_socket(&rt, &served, held("d-1", None, None)).await;
        let (_b, sink_b) = device_socket(&rt, &served, held("d-1", None, None)).await;
        let (_c, sink_c) = device_socket(&rt, &served, held("d-2", None, None)).await;
        let (_h, sink_h) = host_socket(&served).await;
        rt.script.once::<Result<bool, HostError>>("devices.revoke", Ok(true));
        let order = Rc::new(RefCell::new(Vec::new()));
        let seen = order.clone();
        let revoked = served.revoke_device("d-1", Some(Box::new(move |r| seen.borrow_mut().push(format!("answer {r}"))))).await.unwrap();
        assert!(revoked);
        assert_eq!(order.borrow().clone(), vec!["answer true".to_string()]);
        assert_eq!(sink_a.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert_eq!(sink_b.closed(), Some((UNAUTHORIZED_CLOSE, UNAUTHORIZED.to_string())));
        assert_eq!(sink_c.closed(), None);
        assert_eq!(sink_h.closed(), None);
    })
}
