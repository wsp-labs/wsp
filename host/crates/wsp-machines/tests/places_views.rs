// SPDX-License-Identifier: AGPL-3.0-only
//! places/views (spec `places_views.md`, rules V1-V22). Rows compare whole with the oracle's `/rows`; sentences with
//! `tests/fixtures/oracle/places_b_views.json`.

mod places_b_common;

use std::rc::Rc;

use places_b_common::*;
use serde_json::{json, Value};
use wsp_machines::places::link::Joining;
use wsp_machines::places::types::*;
use wsp_store::keys::{new_place_key_pair, sign_place_bytes};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::wire::place_link::PlaceProveRequest;
use wsp_words::exit::HostError;

fn views_oracle(key: &str) -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/places_b_views.json")).unwrap();
    serde_json::from_str::<Value>(&text).unwrap()[key].clone()
}

fn line(key: &str) -> String {
    views_oracle(key).as_str().unwrap().to_string()
}

/// The oracle row with what `with_cap` adds taken off: what `view_of`, `here_row` and `provider_row` answer.
fn bare(row: &Value) -> Value {
    let mut row = row.clone();
    for k in ["cap", "capDefault", "settings", "napMs", "napDefault", "turnLimitMs", "turnLimitDefault", "spawn", "spawnDefault", "running", "size", "sizeDefault", "deleteAfterMs"] {
        row.as_object_mut().unwrap().shift_remove(k);
    }
    row
}

fn prove(signature: &str, report: Value, code: Option<&str>, client: Option<&str>) -> PlaceProveRequest {
    let mut req = json!({ "id": 7, "signature": signature, "report": report });
    if let Some(c) = code {
        req["code"] = json!(c);
    }
    if let Some(c) = client {
        req["client"] = json!({ "name": c });
    }
    serde_json::from_value(req).unwrap()
}

// ---------------------------------------------------------------- V1-V3: pending adds

#[test]
fn pending_records_sorted_and_passthrough() {
    // spec: V1. TS: places-setup.test.ts:1182 lists pending adds oldest first.
    run(async {
        let d = door(|s| {
            s.pending = vec![
                pending_json("a_000000000002", "10.0.0.9", json!({ "step": "wsp", "startedAt": "2026-10-03T00:00:00.000Z", "undo": "rm -rf ~/.wsp", "login": "root@10.0.0.9", "keyPath": "/k", "hostKey": "ssh-ed25519 B" })),
                pending_json("a_000000000001", "10.0.0.5", json!({ "startedAt": "2026-10-02T00:00:00.000Z" })),
                json!({ "id": "a_bad", "address": 5 }),
            ]
        });
        let got = d.views.pending_records().await.unwrap();
        let ids: Vec<&str> = got.iter().map(|p| p.view.id.as_str()).collect();
        assert_eq!(ids, vec!["a_000000000001", "a_000000000002"]);
        assert_eq!(got[1].undo.as_deref(), Some("rm -rf ~/.wsp"));
        assert_eq!(got[1].login.as_deref(), Some("root@10.0.0.9"));
        assert_eq!(got[1].key_path.as_deref(), Some("/k"));
        assert_eq!(got[1].host_key.as_deref(), Some("ssh-ed25519 B"));
    });
}

#[test]
fn put_pending_frame_hides_record_fields() {
    // spec: V2.
    run(async {
        let d = door(|_| {});
        let p = pending_record(pending_json("a_000000000003", "10.0.0.3", json!({ "undo": "rm x", "login": "root@10.0.0.3", "keyPath": "/k" })));
        d.views.put_pending(&p).await.unwrap();
        let stored = d.store.doc("pending-computers", "a_000000000003").unwrap();
        assert_eq!((stored["undo"].clone(), stored["login"].clone(), stored["keyPath"].clone()), (json!("rm x"), json!("root@10.0.0.3"), json!("/k")));
        let frames = d.setups.borrow();
        let SetupEvent::Pending(frame) = &frames[0] else { panic!("a place.pending frame, got {frames:?}") };
        assert_eq!(frame.id, "a_000000000003");
        let view = js(frame.pending.as_ref().unwrap());
        for k in ["undo", "login", "keyPath", "hostKey"] {
            assert!(view.get(k).is_none(), "{k} reached a client");
        }
        assert_eq!(view["address"], json!("10.0.0.3"));
    });
}

#[test]
fn drop_pending_frame_has_no_pending() {
    // spec: V3.
    run(async {
        let d = door(|s| s.pending = vec![pending_json("a_000000000001", "10.0.0.5", json!({}))]);
        d.views.drop_pending("a_000000000001").await.unwrap();
        assert!(d.pending_docs().is_empty());
        let frames = d.setups.borrow();
        let SetupEvent::Pending(frame) = &frames[0] else { panic!("a place.pending frame") };
        assert_eq!((frame.id.as_str(), frame.pending.is_none()), ("a_000000000001", true));
    });
}

// ---------------------------------------------------------------- V4-V6: the floor and the take-back

#[test]
fn floor_while_choosing_no_provisioner_goes_choosing() {
    // spec: V4. TS: places-setup.test.ts:1221 "a computer added with nothing picked waits at its choices".
    run(async {
        let d = door(|s| s.pending = vec![pending_json("a_000000000001", "10.0.0.5", json!({ "step": "join" }))]);
        let p = pending_record(d.pending_docs()[0].clone());
        d.views.floor_while_choosing(&p, PLACE).await.unwrap();
        let stored = d.store.doc("pending-computers", "a_000000000001").unwrap();
        assert_eq!((stored["step"].clone(), stored["placeId"].clone()), (json!("choosing"), json!(PLACE)));
        assert!(stored.get("failed").is_none());
        assert_eq!(d.setups.borrow().len(), 1, "one write: no floor step written");
    });
}

fn flooring(rows: Result<Vec<Value>, &'static str>) -> Door {
    door(move |s| {
        s.pending = vec![pending_json("a_000000000001", "10.0.0.5", json!({ "step": "join" }))];
        *s.setup.link.borrow_mut() = Some(FakeMachineLink::exec_says(""));
        let prov = Rc::new(Provision::default());
        *prov.floor_rows.borrow_mut() = Some(rows.map(|r| r.into_iter().map(|v| serde_json::from_value(v).unwrap()).collect()).map_err(String::from));
        s.wiring = Box::new(move |w| w.provision = Some(prov));
    })
}

#[test]
fn floor_failed_row_says_on_pending() {
    // spec: V4. TS: places-setup.test.ts:1240 "a floor that failed says so on the pending add".
    run(async {
        let d = flooring(Ok(vec![
            json!({ "id": "clis/git", "label": "git", "outcome": "failed", "note": "apt locked" }),
            json!({ "id": "clis/jq", "label": "jq", "outcome": "installed" }),
            json!({ "id": "clis/curl", "label": "curl", "outcome": "failed" }),
        ]));
        let p = pending_record(d.pending_docs()[0].clone());
        d.views.floor_while_choosing(&p, PLACE).await.unwrap();
        let stored = d.store.doc("pending-computers", "a_000000000001").unwrap();
        assert_eq!(stored["step"], json!("choosing"));
        assert_eq!(stored["failed"], json!({ "said": line("floorFailed") }));
        let steps: Vec<Value> = d.setups.borrow().iter().map(|e| match e {
            SetupEvent::Pending(p) => js(p.pending.as_ref().unwrap())["step"].clone(),
            _ => Value::Null,
        }).collect();
        assert_eq!(steps, vec![json!("floor"), json!("choosing")]);
    });
}

#[test]
fn floor_throw_is_one_floor_row() {
    // spec: V4: a throw is one row under the floor's own words, its first line the note.
    run(async {
        let d = flooring(Err("apt-get exited 100\nE: could not get lock"));
        let p = pending_record(d.pending_docs()[0].clone());
        d.views.floor_while_choosing(&p, PLACE).await.unwrap();
        let stored = d.store.doc("pending-computers", "a_000000000001").unwrap();
        assert_eq!(stored["failed"], json!({ "said": format!("the base tools did not install: {}", line("floorWord")) }));
    });
}

#[test]
fn floor_pending_gone_writes_nothing() {
    // spec: V4, V5: a remove or a set up that took the pending add while the floor ran leaves it gone.
    run(async {
        let prov = Rc::new(Provision::default());
        let p2 = prov.clone();
        let d = door(move |s| {
            s.pending = vec![pending_json("a_000000000001", "10.0.0.5", json!({ "step": "join" }))];
            *s.setup.link.borrow_mut() = Some(FakeMachineLink::exec_says(""));
            s.wiring = Box::new(move |w| w.provision = Some(p2));
        });
        let store = d.store.clone();
        *prov.on_floor.borrow_mut() = Some(Box::new(move || {
            store.docs.borrow_mut().get_mut("pending-computers").unwrap().shift_remove("a_000000000001");
        }));
        let p = pending_record(d.pending_docs()[0].clone());
        d.views.floor_once(&p, PLACE).await.unwrap();
        assert!(d.store.doc("pending-computers", "a_000000000001").is_none(), "the floor's end wrote nothing back");
        let steps: Vec<Value> = d.setups.borrow().iter().map(|e| match e {
            SetupEvent::Pending(p) => js(p.pending.as_ref().unwrap())["step"].clone(),
            _ => Value::Null,
        }).collect();
        assert_eq!(steps, vec![json!("floor")], "only the floor step was written");
        assert!(d.views.flooring.borrow().is_empty(), "floor_once took its mark off");
    });
}

#[test]
fn taken_back_without_undo_road() {
    // spec: V6. TS: places-setup.test.ts:1272 "an add the host stopped mid-install is taken back at the next start".
    run(async {
        let d = door(|_| {});
        let p = pending_record(pending_json("a_000000000002", "10.0.0.9", json!({ "step": "wsp", "undo": "rm -rf ~/.wsp", "login": "root@10.0.0.9" })));
        d.views.taken_back(&p).await.unwrap();
        let stored = d.store.doc("pending-computers", "a_000000000002").unwrap();
        assert_eq!(stored["failed"], json!({ "said": line("addNotTakenBack"), "fix": line("addStoppedFix") }));
        assert_eq!(stored["step"], json!("wsp"), "the step stays");
        assert_eq!(stored["undo"], json!("rm -rf ~/.wsp"));
    });
}

#[test]
fn taken_back_undo_ok() {
    // spec: V6: the undo runs over the login with the key file and host key the add kept.
    run(async {
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let s2 = seen.clone();
        let d = door(move |s| s.wiring = Box::new(move |w| w.undo = Some(Rc::new(Undo(Box::new(move |login, script| {
            s2.borrow_mut().push((login.clone(), script.to_string()));
            Ok(())
        }))))));
        let p = pending_record(pending_json("a_000000000002", "10.0.0.9", json!({ "step": "wsp", "undo": "rm -rf ~/.wsp", "login": "root@10.0.0.9", "keyPath": "/k", "hostKey": "ssh-ed25519 B" })));
        d.views.taken_back(&p).await.unwrap();
        assert_eq!(*seen.borrow(), vec![(PlaceLogin { ssh: "root@10.0.0.9".into(), key_path: Some("/k".into()), host_key: Some("ssh-ed25519 B".into()) }, "rm -rf ~/.wsp".to_string())]);
        assert_eq!(d.store.doc("pending-computers", "a_000000000002").unwrap()["failed"], json!({ "said": line("addStopped"), "fix": "Add it again." }));
    });
}

#[test]
fn taken_back_undo_fails_first_line() {
    // spec: V6.
    run(async {
        let d = door(|s| s.wiring = Box::new(|w| w.undo = Some(Rc::new(Undo(Box::new(|_, _| Err(HostError::new("ssh: connect to host 10.0.0.9 port 22: No route to host\nmore"))))))));
        let p = pending_record(pending_json("a_000000000002", "10.0.0.9", json!({ "step": "wsp", "undo": "rm", "login": "root@10.0.0.9" })));
        d.views.taken_back(&p).await.unwrap();
        assert_eq!(
            d.store.doc("pending-computers", "a_000000000002").unwrap()["failed"]["said"],
            json!(format!("{}: ssh: connect to host 10.0.0.9 port 22: No route to host", line("addNotTakenBack")))
        );
    });
}

// ---------------------------------------------------------------- V7-V10: what a remove takes

#[test]
fn unmerged_over_no_home_is_empty() {
    // spec: V7.
    run(async {
        let d = door(|_| {});
        let homeless = record(json!({ "report": report(json!({ "login": { "USER": "maya" } })) }));
        let got = d.views.unmerged_over(PLACE, &homeless).await;
        assert_eq!((got.took.len(), got.kept.len()), (0, 0));
        assert!(d.setup.calls().is_empty(), "no machine was made for it");
    });
}

#[test]
fn store_files_off_failure_is_empty() {
    // spec: V9: a failure takes nothing off and fails nothing.
    run(async {
        let d = door(|s| {
            *s.setup.link.borrow_mut() = Some(FakeMachineLink::new(|_, _| Err("the link went".into())));
            *s.recording.stores.borrow_mut() = Some([("claude".to_string(), "/srv/claude".to_string())].into_iter().collect());
        });
        assert!(d.views.store_files_off(PLACE, &record(json!({}))).await.is_empty());
        let homeless = record(json!({ "report": report(json!({ "login": {} })) }));
        assert!(d.views.store_files_off(PLACE, &homeless).await.is_empty());
    });
}

fn undos() -> Vec<PlaceUndo> {
    vec![
        PlaceUndo { key: "plugins/lint".into(), label: "lint".into(), ids: vec!["plugins/lint".into()], owner: Some("plugins/lint".into()), cmd: Some("claude plugin uninstall lint".into()), ..Default::default() },
        PlaceUndo { key: "plugins/fmt".into(), label: "fmt".into(), ids: vec!["plugins/fmt".into()], note: Some("no command".into()), ..Default::default() },
    ]
}

#[test]
fn plugins_off_only_installed_rows() {
    // spec: V10. TS: places-setup.test.ts:525 "a remove takes off only the plugins wsp put there".
    run(async {
        let prov = Rc::new(Provision::default());
        *prov.undos.borrow_mut() = Some(undos());
        let p2 = prov.clone();
        let d = door(move |s| {
            *s.setup.link.borrow_mut() = Some(FakeMachineLink::exec_says(""));
            s.wiring = Box::new(move |w| w.provision = Some(p2));
        });
        let got = d.views.plugins_off(PLACE, &record(json!({})), true, None, None).await;
        // lint was installed by the setup; fmt was present before wsp and old was never applied.
        assert_eq!(*prov.log.borrow(), vec!["undo /home/maya plugins/lint".to_string()]);
        assert_eq!(got.off, vec![line("pluginOff")]);
        assert!(got.kept.is_empty());
    });
}

#[test]
fn plugins_off_link_then_login_roads() {
    // spec: V10: over the link where linked, else over the login with the password, else nothing runs.
    run(async {
        let prov = Rc::new(Provision::default());
        *prov.undos.borrow_mut() = Some(undos());
        let link = FakeMachineLink::exec_says("");
        let (p2, l2) = (prov.clone(), link.clone());
        let road = Rc::new(RunOverRoad(Box::new(|_, _, timeout, pw| {
            assert_eq!((timeout, pw), (300_000, Some("pw")));
            Ok(RunOver { exit_code: 0, ..Default::default() })
        }), std::cell::RefCell::new(Vec::new())));
        let r2 = road.clone();
        let d = door(move |s| {
            *s.setup.link.borrow_mut() = Some(l2);
            s.wiring = Box::new(move |w| {
                w.provision = Some(p2);
                w.run_over = Some(r2);
            });
        });
        let linked = d.views.plugins_off(PLACE, &record(json!({})), true, Some(&PlaceLogin { ssh: "maya@10.0.0.5".into(), ..Default::default() }), Some("pw")).await;
        assert_eq!(linked.off, vec!["plugin lint"]);
        assert!(link.ops().iter().any(|(op, p)| op == "exec" && p["cmd"] == json!("claude plugin uninstall lint")), "{:?}", link.ops());
        assert!(road.1.borrow().is_empty(), "nothing over the login while linked");
        let over = d.views.plugins_off(PLACE, &record(json!({})), false, Some(&PlaceLogin { ssh: "maya@10.0.0.5".into(), ..Default::default() }), Some("pw")).await;
        assert_eq!(over.off, vec!["plugin lint"]);
        assert_eq!(*road.1.borrow(), vec!["claude plugin uninstall lint".to_string()]);
        let nowhere = d.views.plugins_off(PLACE, &record(json!({})), false, None, None).await;
        assert_eq!((nowhere.off.len(), nowhere.kept.clone()), (0, vec!["lint".to_string()]));
    });
}

#[test]
fn plugins_off_kept_names_and_labels() {
    // spec: V10: a name with no planned command stays by its name; one whose command failed by its label.
    run(async {
        let prov = Rc::new(Provision::default());
        *prov.undos.borrow_mut() = Some(vec![PlaceUndo { key: "plugins/lint".into(), label: "Lint (claude)".into(), ids: vec![], cmd: Some("claude plugin uninstall lint".into()), ..Default::default() }]);
        let p2 = prov.clone();
        let d = door(move |s| {
            *s.setup.link.borrow_mut() = Some(FakeMachineLink::new(|_, _| Ok(obj(json!({ "exitCode": 1, "stdout": "", "stderr": "not installed", "truncated": false })))));
            s.wiring = Box::new(move |w| w.provision = Some(p2));
        });
        let mut doc = record_json(json!({}));
        doc["applied"]["rows"].as_array_mut().unwrap().push(json!({ "id": "plugins/old", "label": "OLD", "outcome": "installed", "step": "plugins" }));
        let held: PlaceRecord = serde_json::from_value(doc).unwrap();
        let got = d.views.plugins_off(PLACE, &held, true, None, None).await;
        assert!(got.off.is_empty());
        assert_eq!(got.kept, vec!["old".to_string(), "Lint (claude)".to_string()]);
        // No undo road at all: nothing, not even kept names.
        let bare = door(|_| {});
        let none = bare.views.plugins_off(PLACE, &held, true, None, None).await;
        assert_eq!((none.off.len(), none.kept.len()), (0, 0));
    });
}

// ---------------------------------------------------------------- V11-V15

#[test]
fn facts_on_answers_held_after_add_facts_ms() {
    // spec: V11. TS: places-facts.test.ts:48 "a computer slower than one round trip joins all the same".
    run(async {
        let d = door(|_| {});
        let held = record(json!({}));
        let (views, h2) = (d.views.clone(), held.clone());
        let read = tokio::task::spawn_local(async move { views.facts_on(PLACE, &h2).await });
        pass(&d.clock, 1_900).await;
        assert!(!read.is_finished(), "waits one round trip");
        pass(&d.clock, 200).await;
        assert_eq!(read.await.unwrap(), held);
        assert_eq!(*d.links.asked.borrow(), vec![PLACE.to_string()]);
    });
}

#[test]
fn backend_from_replaces_kept_backend() {
    // spec: V12.
    run(async {
        let d = door(|s| *s.setup.link.borrow_mut() = Some(FakeMachineLink::exec_says("")));
        d.ctx.backends.borrow_mut().insert(PLACE.into(), StubBackend::new().arc());
        let facts = serde_json::from_value(at("/record/report").get("facts").cloned().unwrap_or(json!({
            "offer": "docker",
            "capabilities": StubBackend::new().capabilities,
            "pricing": { "defaultSize": { "cpu": 2, "memMb": 4096 }, "snapshotStorage": { "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" } },
            "lifecycle": { "budgets": { "wakeAttempts": 1, "daemonAnswersMs": 30000 } }
        })));
        if let Ok(facts) = facts {
            let made = d.views.backend_from(PLACE, facts);
            let kept = d.ctx.backends.borrow().get(PLACE).cloned().unwrap();
            assert!(std::sync::Arc::ptr_eq(&made, &kept), "the backend made is the one kept");
            assert_eq!(d.setup.calls(), vec![format!("linkTo {PLACE}")]);
        } else {
            panic!("the backend facts fixture no longer parses");
        }
    });
}

#[test]
fn tunnelled_other_place_is_false() {
    // spec: V13: only tunnel frames, and only a connection a forward of this place holds.
    run(async {
        let d = door(|_| {});
        let data: wsp_wire::wire::daemon_events::DaemonEvent = serde_json::from_value(json!({ "type": "tunnel.data", "tunnelId": "t1", "data": "aGk=" })).unwrap();
        assert!(!d.views.tunnelled(PLACE, &data));
        let other: wsp_wire::wire::daemon_events::DaemonEvent = serde_json::from_value(json!({ "type": "tunnel.end", "tunnelId": "t1" })).unwrap();
        assert!(!d.views.tunnelled(PLACE, &other));
    });
}

#[test]
fn cut_removes_link_then_closes_1000() {
    // spec: V14. TS: places-link.test.ts "a remove closes the link with its reason".
    run(async {
        let d = door(|_| {});
        let reach = FakeReach::answering(vec![]);
        let (socket, seen_cancelled) = link(&d, PLACE, reach.clone());
        d.views.cut(PLACE, "removed from this host");
        assert!(!d.ctx.live.borrow().contains_key(PLACE));
        assert!(seen_cancelled.get(), "the seen timer was cancelled");
        assert!(reach.closed.get());
        assert_eq!(*socket.closed.borrow(), Some((1000, "removed from this host".to_string())));
        // A second cut is nothing.
        d.views.cut(PLACE, "again");
        assert_eq!(*socket.closed.borrow(), Some((1000, "removed from this host".to_string())));
    });
}

#[test]
fn forks_of_unlinked_none() {
    // spec: V15: unlinked asks nothing; linked with no backend yet asks behind the listing.
    run(async {
        let d = door(|_| {});
        assert_eq!(d.views.forks_of(&record(json!({}))).await, None);
        assert!(d.links.asked.borrow().is_empty());
        link(&d, PLACE, FakeReach::answering(vec![]));
        assert_eq!(d.views.forks_of(&record(json!({}))).await, None);
        settle().await;
        assert_eq!(*d.links.asked.borrow(), vec![PLACE.to_string()]);
    });
}

#[test]
fn forks_of_room_and_running() {
    // spec: V15. TS: places-fork.test.ts:32 a joined computer's row says forks running and room.
    run(async {
        let mut b = StubBackend::new();
        b.offers.capacity = true;
        *b.capacity.lock().unwrap() = Some(Ok(views_oracle("capacity")));
        let backend = b.arc();
        let d = door(move |s| {
            s.links.backends.borrow_mut().insert(PLACE.into(), backend);
        });
        link(&d, PLACE, FakeReach::answering(vec![]));
        let got = d.views.forks_of(&record(json!({}))).await.unwrap();
        assert_eq!(js(&got), json!({ "running": 2, "room": views_oracle("room") }));
        // A backend that offers no capacity says nothing.
        let mut quiet = StubBackend::new();
        quiet.offers.capacity = false;
        let q = quiet.arc();
        let d2 = door(move |s| {
            s.links.backends.borrow_mut().insert(PLACE.into(), q);
        });
        link(&d2, PLACE, FakeReach::answering(vec![]));
        assert_eq!(d2.views.forks_of(&record(json!({}))).await, None);
    });
}

#[test]
fn forks_of_capacity_bound_is_none() {
    // spec: V15: a failure is nothing rather than a guess.
    run(async {
        let mut b = StubBackend::new();
        b.offers.capacity = true;
        *b.capacity.lock().unwrap() = Some(Err("machine.capacity on old-macbook was not answered in 5s".into()));
        let backend = b.arc();
        let d = door(move |s| {
            s.links.backends.borrow_mut().insert(PLACE.into(), backend);
        });
        link(&d, PLACE, FakeReach::answering(vec![]));
        assert_eq!(d.views.forks_of(&record(json!({}))).await, None);
    });
}

// ---------------------------------------------------------------- V16: the row

#[test]
fn view_of_full_row() {
    // spec: V16. TS: places.test.ts:143 a joined computer's row. Oracle /rows/plain/1 less what with_cap adds.
    run(async {
        let d = door(|_| {});
        let row = d.views.view_of(&record(json!({})), Some("here"));
        assert_eq!(js(&row), bare(&at("/rows/plain/1")));
        let defaulted = d.views.view_of(&record(json!({})), Some(PLACE));
        assert_eq!(defaulted.default, true);
        assert_eq!(d.views.view_of(&record(json!({})), None).default, false);
    });
}

#[test]
fn view_of_road_hides_key_path_and_host_key() {
    // spec: V16. Oracle /rows/behind/1: a road holding only a key file is `road: {}`.
    run(async {
        let d = door(|_| {});
        let row = js(&d.views.view_of(&record(json!({ "road": { "keyPath": "/k", "hostKey": "ssh-ed25519 A" } })), Some("here")));
        assert_eq!(row["road"], json!({}));
        let full = js(&d.views.view_of(&record(json!({})), Some("here")));
        assert_eq!(full["road"], json!({ "ssh": "maya@10.0.0.5", "from": "10.0.0.5" }));
        assert!(js(&d.views.view_of(&record(json!({ "road": null })), Some("here"))).get("road").is_none());
    });
}

#[test]
fn view_of_takes_runtime_needs_leave_asks() {
    // spec: V16. Oracle /rows/behind/1: behind, takesRuntime said by a daemon too old to ask, codex signed in.
    run(async {
        let dv = daemon_version();
        let d = door(|_| {});
        let held = record(json!({ "report": report(json!({ "daemonVersion": dv - 1, "takesRuntime": true, "logins": ["codex/auth.json"] })), "road": { "keyPath": "/k" } }));
        assert_eq!(js(&d.views.view_of(&held, Some("here"))), bare(&at("/rows/behind/1")));
        let asks = record(json!({ "report": report(json!({ "takesRuntime": true, "wspDaemonVersion": 999 })) }));
        assert_eq!(d.views.view_of(&asks, Some("here")).takes_runtime, Some(true));
    });
}

#[test]
fn view_of_sign_ins_absent_without_logins() {
    // spec: V16: a daemon listing no logins says nothing, rather than none.
    run(async {
        let d = door(|_| {});
        let old = record(json!({ "report": report(json!({ "logins": null })) }));
        assert!(js(&d.views.view_of(&old, Some("here"))).get("signIns").is_none());
        let blocked = record(json!({ "report": report(json!({ "runsWorkspaces": false, "workspacesBlocked": "docker is not running" })) }));
        assert_eq!(d.views.view_of(&blocked, Some("here")).blocked.as_deref(), Some(line("blocked").as_str()));
        link(&d, PLACE, FakeReach::answering(vec![]));
        assert_eq!(d.views.view_of(&record(json!({})), Some("here")).present, Some(true));
    });
}

// ---------------------------------------------------------------- V17: the join's prove

fn joined_door() -> Door {
    let d = door(|s| s.records = vec![]);
    d.devices.good.borrow_mut().insert("CODE1".into());
    d
}

fn proving(d: &Door, report: Value, code: Option<&str>, client: Option<&str>) -> (Joining, PlaceProveRequest, Vec<u8>) {
    let place = new_place_key_pair();
    let expect = b"the transcript the host's challenge asked the place to sign".to_vec();
    let sig = sign_place_bytes(&place.private_key_pem, &expect).unwrap();
    d.views.joining.borrow_mut().insert(PLACE.into(), Joining { public_key: place.public_key.clone(), at: NOW });
    (Joining { public_key: place.public_key, at: NOW }, prove(&sig, report, code, client), expect)
}

#[test]
fn joined_writes_record_marks_default_emits() {
    // spec: V17. TS: places.test.ts:163 "a join writes the computer's record and makes it the default".
    run(async {
        let d = joined_door();
        let (pending, req, expect) = proving(&d, at("/record/report"), Some("CODE1"), None);
        let proved = d.views.joined(PLACE, pending.clone(), req, &expect, "192.168.1.20", NOW).await.unwrap();
        let Proved::Report { report, device } = proved else { panic!("a report, got {proved:?}") };
        assert_eq!(device, None);
        assert_eq!(js(&report)["login"]["HOME"], json!("/home/maya"));
        let stored = d.stored(PLACE).unwrap();
        assert_eq!(stored["publicKey"], json!(pending.public_key));
        assert_eq!((stored["joinedAt"].clone(), stored["lastSeenAt"].clone(), stored["reportedAt"].clone()), (json!("2023-11-14T22:13:20.000Z"), json!("2023-11-14T22:13:20.000Z"), json!("2023-11-14T22:13:20.000Z")));
        assert_eq!(stored["name"], json!("old-macbook"));
        assert_eq!(d.mark(), Some(json!({ "placeId": PLACE })));
        assert!(!d.views.joining.borrow().contains_key(PLACE));
        assert_eq!(*d.devices.spent.borrow(), vec!["CODE1".to_string()]);
        let events = d.events.borrow();
        let PlaceEvent::Joined(e) = &events[0] else { panic!("place.joined first") };
        assert_eq!((e.from.as_str(), e.place.default, e.place.id.as_str()), ("192.168.1.20", true, PLACE));
    });
}

#[test]
fn joined_bad_signature_spends_no_code() {
    // spec: V17 step 2. TS: places-link.test.ts "a prove signed by another key is refused".
    run(async {
        let d = joined_door();
        let (pending, req, _) = proving(&d, at("/record/report"), Some("CODE1"), None);
        let proved = d.views.joined(PLACE, pending, req, b"other bytes", "192.168.1.20", NOW).await.unwrap();
        assert_eq!(proved, Proved::Refusal { refusal: line("keyRefusal") });
        assert!(d.devices.spent.borrow().is_empty());
        assert!(d.stored(PLACE).is_none() && d.mark().is_none() && d.events.borrow().is_empty());
        assert!(!d.views.joining.borrow().contains_key(PLACE), "the join is forgotten whatever follows");
    });
}

#[test]
fn joined_blocked_spends_no_code() {
    // spec: V17 steps 3 and 4. TS: places.test.ts:176 "a computer that cannot boot the image is not joined".
    run(async {
        let d = joined_door();
        let mut blocked = at("/record/report");
        merge(&mut blocked, json!({ "runsWorkspaces": false, "workspacesBlocked": "docker is not running" }));
        let (pending, req, expect) = proving(&d, blocked, Some("CODE1"), None);
        assert_eq!(d.views.joined(PLACE, pending, req, &expect, "x", NOW).await.unwrap(), Proved::Refusal { refusal: line("blocked") });
        let mut homeless = at("/record/report");
        merge(&mut homeless, json!({ "login": { "HOME": "relative" } }));
        let (pending, req, expect) = proving(&d, homeless, Some("CODE1"), None);
        assert_eq!(d.views.joined(PLACE, pending, req, &expect, "x", NOW).await.unwrap(), Proved::Refusal { refusal: line("homeRefusal") });
        assert!(d.devices.spent.borrow().is_empty());
        assert!(d.stored(PLACE).is_none());
    });
}

#[test]
fn joined_spent_code_refused_no_record() {
    // spec: V17 step 5. TS: places.test.ts:197 "a code spent once is refused".
    run(async {
        let d = joined_door();
        let (pending, req, expect) = proving(&d, at("/record/report"), Some("CODE9"), None);
        assert_eq!(d.views.joined(PLACE, pending, req, &expect, "x", NOW).await.unwrap(), Proved::Refusal { refusal: line("codeRefusal") });
        let (pending, req, expect) = proving(&d, at("/record/report"), None, None);
        assert_eq!(d.views.joined(PLACE, pending, req, &expect, "x", NOW).await.unwrap(), Proved::Refusal { refusal: line("codeRefusal") });
        assert!(d.stored(PLACE).is_none() && d.mark().is_none());
    });
}

#[test]
fn joined_notes_awaiting_install_road() {
    // spec: V17 step 6 with R24: the record's first write carries the road the install came in over.
    run(async {
        let d = joined_door();
        let waiting = Rc::new(std::cell::RefCell::new(wsp_machines::places::records::AwaitingInstall::default()));
        waiting.borrow_mut().login = Some(PlaceLogin { ssh: "maya@10.0.0.5".into(), key_path: Some("/k".into()), host_key: None });
        d.records.awaiting.borrow_mut().insert("CODE1".into(), waiting.clone());
        let (pending, req, expect) = proving(&d, at("/record/report"), Some("CODE1"), None);
        d.views.joined(PLACE, pending, req, &expect, "x", NOW).await.unwrap();
        assert_eq!(waiting.borrow().place_id.as_deref(), Some(PLACE));
        assert_eq!(d.stored(PLACE).unwrap()["road"], json!({ "ssh": "maya@10.0.0.5", "keyPath": "/k" }));
        assert_eq!(d.store.writes_to(PLACES), vec![format!("put places/{PLACE}")], "one write");
    });
}

#[test]
fn joined_admits_client_device() {
    // spec: V17 step 9. TS: places.test.ts:205 "a join from the app's window gets its token in the same act".
    run(async {
        let d = joined_door();
        let (pending, req, expect) = proving(&d, at("/record/report"), Some("CODE1"), Some("old-macbook app"));
        let Proved::Report { device, .. } = d.views.joined(PLACE, pending, req, &expect, "x", NOW).await.unwrap() else { panic!("a report") };
        assert_eq!(device, Some(ProvedDevice { device_id: "d_old-macbook app".into(), device_token: "token-old-macbook app".into() }));
        assert_eq!(*d.devices.admitted.borrow(), vec!["old-macbook app".to_string()]);
    });
}

// ---------------------------------------------------------------- V18: forget

#[test]
fn forget_clears_record_caps_mark_and_emits() {
    // spec: V18. TS: places-remove.test.ts the record, its caps and the default go with a remove.
    run(async {
        let d = door(|s| {
            s.caps = vec![(PLACE.into(), json!({ "threads": 2 }))];
            s.mark = Some(PLACE.into());
        });
        let (socket, _) = link(&d, PLACE, FakeReach::answering(vec![]));
        d.ctx.backends.borrow_mut().insert(PLACE.into(), StubBackend::new().arc());
        d.views.forget(PLACE).await.unwrap();
        assert!(d.stored(PLACE).is_none() && d.caps(PLACE).is_none() && d.mark().is_none());
        assert!(!d.ctx.kept.borrow().contains_key(PLACE));
        assert!(!d.ctx.backends.borrow().contains_key(PLACE));
        assert!(!d.records.settings_held.borrow().contains_key(PLACE));
        assert_eq!(*socket.closed.borrow(), Some((1000, "removed from this host".to_string())));
        assert_eq!(d.setup.calls(), vec![format!("woken {PLACE} false"), format!("closedAt.delete {PLACE}")]);
        assert_eq!(d.event_types(), vec!["place.removed"]);
    });
}

#[test]
fn forget_keeps_mark_on_other_place() {
    // spec: V18: the mark goes only where it names this place; an id with no record still deletes and emits.
    run(async {
        let d = door(|s| s.mark = Some("boat".into()));
        d.views.forget("p_nope").await.unwrap();
        assert_eq!(d.mark(), Some(json!({ "placeId": "boat" })));
        assert_eq!(d.event_types(), vec!["place.removed"]);
        assert!(d.store.writes_to("delete places/p_nope").len() == 1);
    });
}

// ---------------------------------------------------------------- V19-V22: rows

#[test]
fn here_row_fields_and_behind_install() {
    // spec: V19. Oracle /rows/plain/0.
    run(async {
        let d = door(|_| {});
        assert_eq!(js(&d.views.here_row("here")), bare(&at("/rows/plain/0")));
        assert_eq!(d.views.here_row(PLACE).default, false);
        let dv = daemon_version();
        let behind = door(move |s| s.opts = Box::new(move |o| o.here_daemon = Some(Rc::new(Here(Some(dv - 1))))));
        let row = js(&behind.views.here_row("here"));
        assert_eq!(row["daemonVersion"], json!(dv - 1));
        assert_eq!(row["behind"]["fix"], json!("run wsp update here"));
        assert_eq!(row["behind"]["act"], json!("install"));
        let current = door(move |s| s.opts = Box::new(move |o| o.here_daemon = Some(Rc::new(Here(Some(dv))))));
        assert!(js(&current.views.here_row("here")).get("behind").is_none());
    });
}

#[test]
fn rows_of_order_and_one_default() {
    // spec: V20. Oracle /rows/plain and /rows/set (the mark on the joined computer, caps set).
    run(async {
        let d = door(|s| {
            s.recording.running.borrow_mut().insert(PLACE.into(), 2);
        });
        let held = d.records.records().await.unwrap();
        assert_eq!(js(&d.views.rows_of(&held).await.unwrap()), at("/rows/plain"));
        let set = door(|s| {
            s.caps = vec![(PLACE.into(), json!({ "threads": 3, "napMs": null })), ("boat".into(), json!({ "machines": 2, "size": { "cpu": 4, "memMb": 8192 } }))];
            s.mark = Some(PLACE.into());
            s.recording.running.borrow_mut().insert(PLACE.into(), 2);
        });
        let held = set.records.records().await.unwrap();
        let rows = set.views.rows_of(&held).await.unwrap();
        assert_eq!(js(&rows), at("/rows/set"));
        assert_eq!(rows.iter().filter(|r: &&PlaceView| r.default).count(), 1);
    });
}

#[test]
fn rows_of_mark_on_gone_row_falls_to_here() {
    // spec: V20. Oracle /rows/goneMark.
    run(async {
        let d = door(|s| {
            s.mark = Some("p_00000000000000ff".into());
            s.recording.running.borrow_mut().insert(PLACE.into(), 2);
        });
        let held = d.records.records().await.unwrap();
        assert_eq!(js(&d.views.rows_of(&held).await.unwrap()), at("/rows/goneMark"));
    });
}

#[test]
fn provider_row_rate_sizes() {
    // spec: V22. Oracle /rows/plain/2 (no table: rate from the wiring, no sizes).
    run(async {
        let d = door(|_| {});
        assert_eq!(js(&d.views.provider_row("boat", "here")), bare(&at("/rows/plain/2")));
        let mut b = StubBackend::new();
        b.rate = 0.5;
        let backend = b.arc();
        let table = door(move |s| s.providers = Some(Rc::new(Providers { wired: "boat".into(), rows: [("boat".to_string(), StubBackend::new().arc()), ("solari".to_string(), backend)].into_iter().collect() })));
        let row = js(&table.views.provider_row("solari", "solari"));
        assert_eq!((row["rateUsdPerHour"].clone(), row["default"].clone(), row["buildsImages"].clone()), (json!(0.5), json!(true), json!(true)));
        assert_eq!(row["sizes"], json!([{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0 }]));
    });
}

#[test]
fn joined_row_adds_image_facts() {
    // spec: V21.
    run(async {
        let backend = StubBackend::new().arc();
        let d = door(move |s| {
            s.links.backends.borrow_mut().insert(PLACE.into(), backend);
        });
        let row = js(&d.views.joined_row(&record(json!({})), "here"));
        let mut want = bare(&at("/rows/plain/1"));
        want["buildsImages"] = json!(true);
        assert_eq!(row, want);
    });
}
