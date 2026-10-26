// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/places.test.ts (joining, dialling back, the proved socket), places-link.test.ts:193 and :245,
//! places-facts.test.ts:15, :48 and :78, places-door.test.ts:32 and :98, and places-setup.test.ts's resume and load
//! cases at the link door's seam. Spec: spec/places_link_door.md.
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use wsp_engine::provision::EngineStep;
use wsp_machines::places::helpers::{PendingRecord, LinkSocket, JOIN_PROVE_MS, PLACE_BAD_KEY_REFUSAL, REPLACED};
use wsp_machines::places::link::{Joining, LinkDoor};
use wsp_machines::places::types::{AuthAnswer, PlaceChallenge, PlaceEvent, Proved};
use wsp_store::keys::{make_seal, new_place_key_pair, seal_keys, sign_place_bytes, Side};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::place_link::{PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport, PLACE_KEY_REFUSAL, PLACE_UNKNOWN_REFUSAL, PLACE_UNSEALED_JOIN_REFUSAL};

use common::places::*;

const KEY: &str = "MCowBQYDK2VwAyEAGb9ECWmEzf6FQbrBZ9w7lshQhqowtrbLDFw4rXAxZuE=";
/// 32 bytes, as a nonce and an ephemeral key ride the wire.
fn b32(fill: u8) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode([fill; 32])
}

/// An X25519 public key as SPKI DER: the right length for the wire, and no key a signature verifies against.
fn x25519_key() -> String {
    use base64::Engine;
    let mut der = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00];
    der.extend([7u8; 32]);
    base64::engine::general_purpose::STANDARD.encode(der)
}

/// The host's signature over one subject's transcript, as the fake challenge makes it: 64 bytes naming the subject.
fn host_signature(subject: &str) -> String {
    use base64::Engine;
    let mut bytes = format!("host-signed:{subject}").into_bytes();
    bytes.resize(64, b'.');
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn nonce() -> String {
    b32(1)
}

/// 64 bytes, a signature's length, signed by nobody.
fn no_signature() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode([9u8; 64])
}

fn challenging(scene: &Scene) {
    *scene.records.challenge.borrow_mut() = Some(Rc::new(|subject: &str| PlaceChallenge {
        nonce: b32(3),
        host_public_key: KEY.into(),
        signature: host_signature(subject),
        ephemeral: b32(4),
        expect: format!("expect:{subject}").into_bytes(),
        seal: make_seal(&seal_keys(b"secret", subject), Side::Host),
    }));
}

fn join_req(key: &str) -> PlaceJoinRequest {
    serde_json::from_value(json!({ "id": 1, "publicKey": key, "nonce": nonce(), "ephemeral": b32(2) })).expect("a join request")
}

fn auth_req(place: &str) -> PlaceAuthRequest {
    serde_json::from_value(json!({ "id": 1, "placeId": place, "nonce": nonce(), "ephemeral": b32(2) })).expect("an auth request")
}

fn prove_req(signature: &str, report: &PlaceReport) -> PlaceProveRequest {
    serde_json::from_value(json!({ "id": 2, "signature": signature, "report": report })).expect("a prove request")
}

fn pending(id: &str, step: &str, place: Option<&str>, login: Option<&str>) -> PendingRecord {
    serde_json::from_value(json!({ "id": id, "address": "root@10.0.0.9", "step": step, "choices": { "name": "laptop" }, "startedAt": iso(T0), "placeId": place, "login": login }).as_object().unwrap().iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), v.clone())).collect::<Map<String, Value>>().into())
        .expect("a pending add")
}

fn door() -> (Scene, Rc<LinkDoor>) {
    let scene = Scene::new(SceneOpts::default());
    let link = scene.link();
    (scene, link)
}

fn current(name: &str) -> PlaceReport {
    report(name, json!({ "daemonVersion": 200, "login": { "HOME": "/root", "PATH": "/usr/bin" } }))
}

fn kept_with_key(scene: &Scene, id: &str, public_key: &str) {
    let mut r = record(id, "spoo", current("spoo"));
    r.public_key = public_key.into();
    scene.records.keep(r);
}

#[test]
fn a_join_with_a_key_this_host_cannot_verify_holds_nothing() {
    block(async {
        let (scene, link) = door();
        challenging(&scene);
        let e = link.join(join_req(&x25519_key()), "10.0.0.9", T0 as u64).await.err().expect("refused");
        assert_eq!(e.message, PLACE_BAD_KEY_REFUSAL);
        assert!(scene.views.joining.borrow().is_empty());
        assert!(scene.records.map.borrow().is_empty());
    });
}

#[test]
fn a_join_answers_the_challenge_and_writes_nothing() {
    block(async {
        let (scene, link) = door();
        challenging(&scene);
        let answer = link.join(join_req(KEY), "10.0.0.9", T0 as u64).await.expect("answered");
        let reply = serde_json::to_value(&answer.reply).unwrap();
        let id = reply["placeId"].as_str().unwrap().to_string();
        assert!(id.len() == 18 && id.starts_with("p_") && id[2..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "eight random bytes in hex: {id}");
        assert_eq!(reply["hostName"], json!("zingzys-mac"));
        assert_eq!(reply["hostPublicKey"], json!(KEY));
        assert_eq!(reply["signature"], json!(host_signature(&id)));
        assert_eq!(answer.expect, format!("expect:{id}").into_bytes());
        assert_eq!(scene.views.joining.borrow().get(&id), Some(&Joining { public_key: KEY.into(), at: T0 as u64 }));
        assert!(scene.records.map.borrow().is_empty(), "nothing is written before the prove");
        assert!(scene.records.writes.borrow().is_empty());
    });
}

#[test]
fn a_join_this_host_cannot_seal_is_refused() {
    block(async {
        let (scene, link) = door();
        let e = link.join(join_req(KEY), "10.0.0.9", T0 as u64).await.err().expect("refused");
        assert_eq!(e.message, PLACE_UNSEALED_JOIN_REFUSAL);
        assert!(scene.views.joining.borrow().is_empty());
    });
}

#[test]
fn a_join_forgets_joins_whose_prove_never_came() {
    block(async {
        let (scene, link) = door();
        challenging(&scene);
        let start = T0 as u64;
        scene.views.joining.borrow_mut().insert("p_old".into(), Joining { public_key: KEY.into(), at: start });
        scene.views.joining.borrow_mut().insert("p_edge".into(), Joining { public_key: KEY.into(), at: start + 1 });
        link.join(join_req(KEY), "10.0.0.9", start + 1 + JOIN_PROVE_MS).await.unwrap();
        let held: Vec<String> = scene.views.joining.borrow().keys().cloned().collect();
        assert!(!held.contains(&"p_old".to_string()), "older than two minutes goes");
        assert!(held.contains(&"p_edge".to_string()), "exactly two minutes stays");
        assert_eq!(held.len(), 2);
    });
}

#[test]
fn auth_refuses_an_unknown_id_with_a_signed_sentence() {
    block(async {
        let (scene, link) = door();
        challenging(&scene);
        match link.auth(auth_req("p_nowhere")).await.expect("answered") {
            AuthAnswer::Refusal { refusal, signed } => {
                assert_eq!(refusal, PLACE_UNKNOWN_REFUSAL);
                assert_eq!(serde_json::to_value(signed).unwrap()["signature"], json!(common::places::signature_of(&format!("signed:p_nowhere:{}:{PLACE_UNKNOWN_REFUSAL}", nonce()))));
            }
            AuthAnswer::Reply { .. } => panic!("an unknown id is refused"),
        }
    });
}

#[test]
fn auth_refuses_a_daemon_older_than_the_seal_in_its_rows_words() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", report("spoo", json!({ "daemonVersion": 17 }))));
        let e = link.auth(auth_req("p_spoo")).await.err().expect("refused");
        let word = wsp_wire::wire::daemon_version::place_daemon_behind(Some(17)).unwrap_or_default();
        assert_eq!(e.message, wsp_wire::wire::daemon_version::place_behind_line("spoo", &word));
    });
}

#[test]
fn auth_answers_a_known_place() {
    block(async {
        let (scene, link) = door();
        challenging(&scene);
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        match link.auth(auth_req("p_spoo")).await.expect("answered") {
            AuthAnswer::Reply { reply, expect, .. } => {
                assert_eq!(serde_json::to_value(reply).unwrap(), json!({ "nonce": b32(3), "hostPublicKey": KEY, "signature": host_signature("p_spoo"), "ephemeral": b32(4) }));
                assert_eq!(expect, b"expect:p_spoo".to_vec());
            }
            AuthAnswer::Refusal { .. } => panic!("a known place is answered"),
        }
    });
}

#[test]
fn prove_refuses_a_wrong_signature_and_writes_nothing() {
    block(async {
        let (scene, link) = door();
        let pair = new_place_key_pair();
        kept_with_key(&scene, "p_spoo", &pair.public_key);
        let signed = sign_place_bytes(&pair.private_key_pem, b"another transcript").unwrap();
        let proved = link.prove("p_spoo", prove_req(&signed, &current("spoo")), b"the transcript", "10.0.0.9", T0 as u64).await.unwrap();
        assert_eq!(proved, Proved::Refusal { refusal: PLACE_KEY_REFUSAL.into() });
        assert!(scene.records.writes.borrow().is_empty());
        let unknown = link.prove("p_nowhere", prove_req(&signed, &current("spoo")), b"the transcript", "10.0.0.9", T0 as u64).await.unwrap();
        assert_eq!(unknown, Proved::Refusal { refusal: PLACE_KEY_REFUSAL.into() });
    });
}

#[test]
fn prove_refuses_a_report_with_no_plain_home() {
    block(async {
        let (scene, link) = door();
        let pair = new_place_key_pair();
        kept_with_key(&scene, "p_spoo", &pair.public_key);
        let signed = sign_place_bytes(&pair.private_key_pem, b"the transcript").unwrap();
        let bad = report("spoo", json!({ "login": { "HOME": "/root/../etc", "PATH": "/usr/bin" } }));
        let proved = link.prove("p_spoo", prove_req(&signed, &bad), b"the transcript", "10.0.0.9", T0 as u64).await.unwrap();
        assert_eq!(proved, Proved::Refusal { refusal: wsp_machines::places::helpers::place_home_refusal(Some("/root/../etc")) });
        assert_eq!(scene.records.get("p_spoo").report.login.get("HOME").map(String::as_str), Some("/root"), "the workspace keeps the login it had");
    });
}

#[test]
fn prove_hands_a_pending_join_to_the_views() {
    block(async {
        let (scene, link) = door();
        scene.views.joining.borrow_mut().insert("p_new".into(), Joining { public_key: KEY.into(), at: T0 as u64 });
        let proved = link.prove("p_new", prove_req(&no_signature(), &current("spoo")), b"t", "10.0.0.9", T0 as u64).await.unwrap();
        assert_eq!(*scene.views.joined_calls.borrow(), vec!["p_new".to_string()]);
        assert!(matches!(proved, Proved::Report { .. }));
    });
}

#[test]
fn prove_answers_the_taken_report() {
    block(async {
        let (scene, link) = door();
        let pair = new_place_key_pair();
        kept_with_key(&scene, "p_spoo", &pair.public_key);
        let signed = sign_place_bytes(&pair.private_key_pem, b"the transcript").unwrap();
        let given = report("spoo", json!({ "login": { "HOME": "/root", "PATH": "/usr/bin:relative:/opt/bin", "CODEX_HOME": "bad;dir" } }));
        let proved = link.prove("p_spoo", prove_req(&signed, &given), b"the transcript", "10.0.0.9", T0 as u64).await.unwrap();
        let Proved::Report { report: taken, device } = proved else { panic!("proved") };
        assert_eq!(device, None);
        assert_eq!(serde_json::to_value(&taken.login).unwrap(), json!({ "HOME": "/root", "PATH": "/usr/bin:/opt/bin" }));
        assert!(scene.records.writes.borrow().is_empty(), "the prove writes nothing; the attach does");
    });
}

async fn attach(scene: &Scene, link: &Rc<LinkDoor>, report: PlaceReport, from: &str, script: Script) -> Rc<FakeSocket> {
    let socket = FakeSocket::new(script);
    link.attach("p_spoo", Rc::clone(&socket) as Rc<dyn LinkSocket>, report, from, scene_now(scene), None).await.expect("attached");
    settle().await;
    socket
}

fn scene_now(scene: &Scene) -> u64 {
    use wsp_core::types::pending::Clock;
    scene.clock.now() as u64
}

fn facts_script() -> Script {
    Rc::new(|op, _| {
        if op == "machine.backend" {
            Reply::Ok(common::backend::said_facts(Some("/root/.wsp/logins")))
        } else {
            Reply::Ok(obj(json!({ "ok": true })))
        }
    })
}

#[test]
fn attach_cuts_the_older_link_first() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let first = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let second = attach(&scene, &link, current("spoo"), "10.0.0.10", answers_ok()).await;
        assert_eq!(scene.views.cuts.borrow().last(), Some(&("p_spoo".to_string(), REPLACED.to_string())));
        assert_eq!(*first.closed_with.borrow(), Some((1000, REPLACED.to_string())));
        assert!(second.open.get());
        assert!(Rc::ptr_eq(&scene.ctx.live.borrow().get("p_spoo").unwrap().reach, &(Rc::clone(&second.reach) as Rc<dyn wsp_core::types::pending::DaemonReach>)));
    });
}

#[test]
fn attach_to_a_place_no_longer_held_closes_the_socket() {
    block(async {
        let (scene, link) = door();
        let socket = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        assert_eq!(*socket.closed_with.borrow(), Some((1000, "this host no longer holds that place".to_string())));
        assert!(scene.ctx.live.borrow().is_empty());
        assert!(scene.events.borrow().is_empty());
    });
}

#[test]
fn attach_keeps_the_joined_name_whatever_the_report_says() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        attach(&scene, &link, current("someone-else"), "10.0.0.9", answers_ok()).await;
        let r = scene.records.get("p_spoo");
        assert_eq!(r.name, "spoo");
        assert_eq!(r.report.name, "someone-else");
    });
}

#[test]
fn attach_keeps_where_the_link_came_from_and_drops_the_last_dial() {
    block(async {
        let (scene, link) = door();
        let mut r = record("p_spoo", "spoo", current("spoo"));
        r.road = Some(serde_json::from_value(json!({ "ssh": "root@10.0.0.9", "keyPath": "/k" })).unwrap());
        r.dialled = Some(serde_json::from_value(json!({ "at": iso(T0), "answered": false, "said": "connection refused" })).unwrap());
        scene.records.keep(r);
        scene.clock.pass(5_000).await;
        attach(&scene, &link, current("spoo"), "192.168.1.20", answers_ok()).await;
        let now = scene.records.get("p_spoo");
        assert_eq!(now.dialled, None);
        let road = serde_json::to_value(now.road.unwrap()).unwrap();
        assert_eq!(road, json!({ "ssh": "root@10.0.0.9", "from": "192.168.1.20", "keyPath": "/k" }));
        assert_eq!(now.last_seen_at, iso(T0 + 5_000));
        assert_eq!(now.reported_at.as_deref(), Some(iso(T0 + 5_000).as_str()));
    });
}

#[test]
fn attach_on_a_new_daemon_asks_the_facts_again() {
    block(async {
        let (scene, link) = door();
        *scene.views.backend.borrow_mut() = Some(Rc::new(|_p, _f| null_backend()));
        let mut r = record("p_spoo", "spoo", current("spoo"));
        r.backend_facts = Some(serde_json::from_value(common::backend::facts_json(None)).expect("facts"));
        scene.records.keep(r);
        let first = attach(&scene, &link, current("spoo"), "10.0.0.9", facts_script()).await;
        assert_eq!(first.reach.asked("machine.backend"), 0, "the same daemon's facts stand");
        let newer = report("spoo", json!({ "daemonVersion": 201, "login": { "HOME": "/root", "PATH": "/usr/bin" } }));
        let second = attach(&scene, &link, newer, "10.0.0.9", facts_script()).await;
        assert_eq!(second.reach.asked("machine.backend"), 1, "a daemon on another version is asked again");
        let facts = serde_json::to_value(scene.records.get("p_spoo").backend_facts).unwrap();
        assert_eq!(facts["logins"], json!("/root/.wsp/logins"), "a field the version before never carried lands");
    });
}

#[test]
fn attach_keeps_a_field_written_while_it_awaited() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        *scene.records.meanwhile.borrow_mut() = Some(Box::new(|r| r.picks = Some(recipe(json!({ "name": "written meanwhile" })))));
        attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        assert_eq!(scene.records.get("p_spoo").picks.map(|p| p.name), Some("written meanwhile".to_string()));
    });
}

#[test]
fn an_attached_link_routes_each_event_to_its_readers() {
    block(async {
        let seen: Rc<RefCell<Vec<(String, DaemonEvent)>>> = Rc::new(RefCell::new(Vec::new()));
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let _ = &seen;
        let socket = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let heard: Rc<RefCell<Vec<Map<String, Value>>>> = Rc::new(RefCell::new(Vec::new()));
        let h = Rc::clone(&heard);
        let channel = link.channel("p_spoo", Rc::new(move |e: &Map<String, Value>| h.borrow_mut().push(e.clone()))).expect("a channel on a live link");
        scene.views.tunnels.borrow_mut().insert("p1".into());
        let pty: DaemonEvent = serde_json::from_value(json!({ "type": "pty.data", "ptyId": "t1", "data": "aGk=" })).unwrap();
        let tunnel: DaemonEvent = serde_json::from_value(json!({ "type": "tunnel.data", "tunnelId": "p1", "data": "aGk=" })).unwrap();
        socket.push(&pty);
        socket.push(&tunnel);
        assert_eq!(heard.borrow().len(), 1, "a tunnel's bytes are the forward's alone");
        assert_eq!(Value::Object(heard.borrow()[0].clone()), serde_json::to_value(&pty).unwrap());
        assert_eq!(*scene.views.tunnel_events.borrow(), vec![tunnel]);
        channel.close();
    });
}

#[test]
fn the_link_closing_marks_the_place_absent() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let socket = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        scene.clock.pass(60_000).await;
        assert_eq!(scene.records.seen.borrow().len(), 1, "the last seen goes every minute");
        socket.drop_link(1006, "gone");
        settle().await;
        assert!(scene.ctx.live.borrow().get("p_spoo").is_none());
        assert_eq!(scene.setup.closed_at.borrow().get("p_spoo"), Some(&((T0 + 60_000) as u64)));
        assert_eq!(scene.records.seen.borrow().len(), 2, "and once more as it goes");
        assert!(socket.reach.closed.get());
        let kinds: Vec<Value> = scene.events.borrow().iter().map(|e| match e {
            PlaceEvent::Absent(a) => json!({ "absent": serde_json::to_value(a).unwrap() }),
            PlaceEvent::Joined(_) => json!("joined"),
            PlaceEvent::Changed(_) => json!("changed"),
            PlaceEvent::Removed(_) => json!("removed"),
        }).collect();
        assert_eq!(kinds.last(), Some(&json!({ "absent": { "placeId": "p_spoo" } })));
        scene.clock.pass(120_000).await;
        assert_eq!(scene.records.seen.borrow().len(), 2, "the timer went with the link");
    });
}

#[test]
fn a_replaced_sockets_close_does_nothing() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let first = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let second = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let absent_before = scene.events.borrow().iter().filter(|e| matches!(e, PlaceEvent::Absent(_))).count();
        first.drop_link(1006, "late close");
        settle().await;
        assert!(scene.ctx.live.borrow().get("p_spoo").is_some());
        assert!(!second.reach.closed.get());
        assert_eq!(scene.events.borrow().iter().filter(|e| matches!(e, PlaceEvent::Absent(_))).count(), absent_before);
    });
}

#[test]
fn a_held_frame_goes_again_before_the_attach_asks_anything() {
    block(async {
        let scene = Scene::new(SceneOpts { relink_wait_ms: Some(10_000), ..SceneOpts::default() });
        let link = scene.link();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let first = attach(&scene, &link, current("spoo"), "10.0.0.9", Rc::new(|_, _| Reply::Hang)).await;
        let machine_link = scene.setup.link_to("p_spoo");
        let asked = tokio::task::spawn_local(async move { machine_link.request("machine.create", Default::default(), wsp_engine::link_backend::LinkAsk { timeout_ms: None, idempotency_key: Some("k1".into()) }).await });
        until("the create went up", || first.reach.asked("machine.create") == 1).await;
        first.drop_link(1006, "gone");
        settle().await;
        let second = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let ops: Vec<String> = second.reach.frames.borrow().iter().map(|(op, _)| op.clone()).collect();
        assert_eq!(ops.first().map(String::as_str), Some("machine.create"), "the held frame goes first: {ops:?}");
        assert!(asked.await.unwrap().is_ok());
    });
}

#[test]
fn the_facts_are_read_behind_the_attach() {
    block(async {
        let (scene, link) = door();
        *scene.views.backend.borrow_mut() = Some(Rc::new(|_p, _f| null_backend()));
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let socket = attach(&scene, &link, current("spoo"), "10.0.0.9", facts_script()).await;
        assert_eq!(socket.reach.asked("machine.backend"), 1);
        let facts = serde_json::to_value(scene.records.get("p_spoo").backend_facts).unwrap();
        assert_eq!(facts["logins"], json!("/root/.wsp/logins"));
        let mut busy = scene.records.get("p_spoo");
        busy.backend_facts = None;
        busy.setup = Some(serde_json::from_value(json!({ "state": "running", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] })).unwrap());
        scene.records.keep(busy);
        scene.ctx.backends.borrow_mut().clear();
        scene.setup.setting.borrow_mut().insert("p_spoo".into());
        let quiet = attach(&scene, &link, report("spoo", json!({ "daemonVersion": 202, "login": { "HOME": "/root", "PATH": "/usr/bin" } })), "10.0.0.9", facts_script()).await;
        assert_eq!(quiet.reach.asked("machine.backend"), 0, "a computer whose setup runs is not asked, and says nothing");
    });
}

#[test]
fn a_link_resumes_a_running_setup() {
    block(async {
        let (scene, link) = door();
        let mut r = record("p_spoo", "spoo", current("spoo"));
        r.picks = Some(recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } } })));
        r.setup = Some(serde_json::from_value(json!({ "state": "running", "addId": "a_old", "startedAt": iso(T0), "steps": [{ "step": "floor", "state": "done", "ms": 1 }], "waiting": [] })).unwrap());
        scene.records.keep(r);
        scene.provisioner.hold(EngineStep::Agents);
        attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        until("the setup resumed", || scene.provisioner.ran_words().contains(&"agents")).await;
        assert!(!scene.provisioner.ran_words().contains(&"floor"));
        assert_eq!(scene.records.get("p_spoo").setup.unwrap().add_id, "a_old");
        scene.provisioner.let_go(EngineStep::Agents);
    });
}

#[test]
fn a_link_catches_a_following_computer_up() {
    block(async {
        let shelf = Rc::new(Shelf { now: RefCell::new(wsp_machines::places::types::Resolved { file: recipe(json!({ "name": "laptop" })), items: Default::default(), hash: "h1".into() }), resolves: std::cell::Cell::new(0) });
        let scene = Scene::new(SceneOpts { shelf: Some(Rc::clone(&shelf)), ..SceneOpts::default() });
        let link = scene.link();
        let mut r = record("p_spoo", "spoo", current("spoo"));
        r.recipe = Some("laptop".into());
        r.picks = Some(recipe(json!({ "name": "laptop" })));
        r.setup = Some(serde_json::from_value(json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] })).unwrap());
        r.applied = Some(serde_json::from_value(json!({ "hash": "h1", "at": iso(T0), "rows": [] })).unwrap());
        scene.records.keep(r);
        attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        scene.clock.pass(0).await;
        assert_eq!(shelf.resolves.get(), 1, "one sync at the dial-back");
    });
}

#[test]
fn a_link_floors_a_pending_add_once() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        scene.views.pending.borrow_mut().push(pending("a1", "floor", Some("p_spoo"), None));
        scene.views.pending.borrow_mut().push(pending("a2", "floor", Some("p_other"), None));
        scene.views.pending.borrow_mut().push(pending("a3", "floor", Some("p_spoo"), None));
        scene.views.flooring.borrow_mut().insert("a3".into());
        attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        assert_eq!(*scene.views.floored.borrow(), vec![("a1".to_string(), "p_spoo".to_string())]);
    });
}

#[test]
fn attach_says_present_and_wakes_what_waits_on_it() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let woke: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let w = Rc::clone(&woke);
        scene.records.awaiting.borrow_mut().push(("p_spoo".into(), Rc::new(move |p: &str| w.borrow_mut().push(p.to_string()))));
        attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        assert_eq!(*woke.borrow(), vec!["p_spoo".to_string()]);
        assert_eq!(scene.records.awaiting.borrow().len(), 1, "nothing is taken off the list");
        let present = scene.setup_frames();
        let _ = present;
        assert!(scene.ctx.live.borrow().contains_key("p_spoo"));
    });
}

#[test]
fn a_channel_rides_the_link_it_opened_on() {
    block(async {
        let (scene, link) = door();
        assert!(link.channel("p_spoo", Rc::new(|_e: &Map<String, Value>| {})).is_none(), "no link, no channel");
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let socket = attach(&scene, &link, current("spoo"), "10.0.0.9", Rc::new(|op, p| Reply::Ok(obj(json!({ "ok": true, "op": op, "params": p }))))).await;
        let channel = link.channel("p_spoo", Rc::new(|_e: &Map<String, Value>| {})).unwrap();
        let answer = channel.send(obj(json!({ "op": "fs.list", "path": "/root" }))).await.unwrap();
        assert_eq!(serde_json::to_value(answer).unwrap()["params"], json!({ "path": "/root" }));
        assert_eq!(socket.reach.frames.borrow().last().unwrap().0, "fs.list");
        assert_eq!(scene.setup.channels.borrow().get("p_spoo").map(|c| c.len()), Some(1));
        let listening = socket.listening();
        socket.drop_link(1006, "gone away");
        let gone = channel.closed().await;
        assert_eq!((gone.code, gone.reason.as_str()), (1006, "gone away"));
        assert!(scene.setup.channels.borrow().get("p_spoo").is_none());
        assert!(listening >= 1);
    });
}

#[test]
fn a_channels_watch_is_a_hold_on_the_shared_watch() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        let socket = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let channel = link.channel("p_spoo", Rc::new(|_e: &Map<String, Value>| {})).unwrap();
        let first = channel.send(obj(json!({ "op": "sys.watch" }))).await.unwrap();
        let again = channel.send(obj(json!({ "op": "sys.watch" }))).await.unwrap();
        assert_eq!(serde_json::to_value(&first).unwrap(), json!({ "id": null, "ok": true }));
        assert_eq!(serde_json::to_value(&again).unwrap(), json!({ "id": null, "ok": true }));
        assert_eq!(socket.reach.asked("sys.watch"), 1);
        let off = channel.send(obj(json!({ "op": "sys.unwatch" }))).await.unwrap();
        assert_eq!(serde_json::to_value(&off).unwrap(), json!({ "id": null, "ok": true }));
        settle().await;
        assert_eq!(socket.reach.asked("sys.unwatch"), 1, "the last reader's hold let go stops the shared watch, the channel's unwatch itself never goes up");
    });
}

#[test]
fn a_channel_on_a_link_that_went_refuses_its_frames() {
    block(async {
        let (scene, link) = door();
        scene.records.keep(record("p_spoo", "spoo", current("spoo")));
        attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let channel = link.channel("p_spoo", Rc::new(|_e: &Map<String, Value>| {})).unwrap();
        let newer = attach(&scene, &link, current("spoo"), "10.0.0.9", answers_ok()).await;
        let e = channel.send(obj(json!({ "op": "fs.list" }))).await.err().expect("refused");
        assert_eq!(e.message, wsp_words::workspace_state::absent_computer("spoo", None).sentence);
        assert_eq!(newer.reach.asked("fs.list"), 0);
    });
}

#[test]
fn load_reads_every_record_back() {
    block(async {
        let (scene, link) = door();
        *scene.views.backend.borrow_mut() = Some(Rc::new(|_p, _f| null_backend()));
        scene.store.docs.borrow_mut().entry("caps".into()).or_default().insert("p_a".into(), json!({}));
        scene.records.stored_settings.borrow_mut().insert("p_a".into(), serde_json::from_value(json!({ "threads": 3 })).unwrap_or_else(|_| serde_json::from_value(json!({})).unwrap()));
        let mut a = record("p_a", "a", current("a"));
        a.backend_facts = Some(serde_json::from_value(common::backend::facts_json(None)).expect("facts"));
        let mut b = record("p_b", "b", current("b"));
        b.joined_at = "2026-10-26T00:00:00.000Z".into();
        b.sync = Some(serde_json::from_value(json!({ "state": "running", "changes": ["clis/jq"], "since": iso(T0) })).unwrap());
        scene.records.map.borrow_mut().insert("p_b".into(), b);
        scene.records.map.borrow_mut().insert("p_a".into(), a);
        link.load().await.unwrap();
        assert_eq!(scene.ctx.kept.borrow().keys().cloned().collect::<Vec<_>>(), ["p_a", "p_b"]);
        assert_eq!(scene.views.built.borrow().iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(), ["p_a"]);
        assert_eq!(*scene.records.held_back.borrow(), ["p_a", "p_b"]);
        assert!(scene.records.settings.borrow().contains_key("p_a"));
        assert_eq!(serde_json::to_value(scene.records.get("p_b").sync.unwrap()).unwrap()["state"], json!("behind"));
    });
}

#[test]
fn load_expires_a_wait_an_ended_setup_left() {
    block(async {
        let (scene, link) = door();
        let mut r = record("p_spoo", "spoo", current("spoo"));
        r.setup = Some(serde_json::from_value(json!({ "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [{ "row": "signins/codex", "label": "Codex", "expiresAt": iso(T0), "state": "waiting", "code": "C" }] })).unwrap());
        scene.records.keep(r);
        let mut running = record("p_run", "run", current("run"));
        running.setup = Some(serde_json::from_value(json!({ "state": "running", "addId": "a2", "startedAt": iso(T0), "steps": [], "waiting": [{ "row": "signins/codex", "label": "Codex", "expiresAt": iso(T0), "state": "waiting" }] })).unwrap());
        scene.records.keep(running);
        link.load().await.unwrap();
        let w = serde_json::to_value(&scene.records.get("p_spoo").setup.unwrap().waiting[0]).unwrap();
        assert_eq!(w, json!({ "row": "signins/codex", "label": "Codex", "code": "C", "expiresAt": iso(T0), "state": "expired" }));
        let kept = serde_json::to_value(&scene.records.get("p_run").setup.unwrap().waiting[0]).unwrap();
        assert_eq!(kept["state"], json!("waiting"), "a running setup's waits run again as it resumes");
    });
}

#[test]
fn load_finishes_an_add_the_host_stopped_in() {
    block(async {
        let (scene, link) = door();
        let mut r = record("p_spoo", "spoo", current("spoo"));
        r.road = Some(serde_json::from_value(json!({ "ssh": "root@10.0.0.9" })).unwrap());
        scene.records.keep(r);
        scene.views.pending.borrow_mut().push(pending("a_joined", "wsp", None, Some("root@10.0.0.9")));
        scene.views.pending.borrow_mut().push(pending("a_lost", "wsp", None, Some("root@10.0.0.77")));
        scene.views.pending.borrow_mut().push(pending("a_floor", "floor", Some("p_spoo"), None));
        link.load().await.unwrap();
        settle().await;
        let joined = scene.views.pending.borrow().iter().find(|p| p.view.id == "a_joined").cloned().unwrap();
        assert_eq!(serde_json::to_value(&joined.view).unwrap()["step"], json!("floor"));
        assert_eq!(joined.view.place_id.as_deref(), Some("p_spoo"));
        assert_eq!(*scene.views.taken_back.borrow(), vec!["a_lost".to_string()]);
    });
}

/// A backend nothing in these tests calls.
fn null_backend() -> Arc<dyn wsp_engine::machine::MachineBackend> {
    common::backend::tagged("built")
}
