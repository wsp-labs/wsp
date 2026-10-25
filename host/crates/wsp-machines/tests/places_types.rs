// SPDX-License-Identifier: AGPL-3.0-only
//! places/types (spec `places_types.md`, rules T1-T17). Expected values from the oracle's `/types`.

mod places_b_common;

use places_b_common::*;
use serde_json::{json, Value};
use wsp_machines::places::types::*;
use wsp_words::exit::{HostError, Kind};

#[test]
fn error_marks_and_kinds() {
    // T3: each class is its mark; the login refusal is kind "login", the provisioning one kind conflict.
    let e = place_login_refused("ssh: Permission denied (publickey)");
    assert_eq!(e.message, "ssh: Permission denied (publickey)");
    assert_eq!(e.kind.as_ref().map(|k| k.as_str().to_string()), Some("login".to_string()));
    assert_eq!(e.fix, None);
    assert!(is_place_login_refused(&e));
    assert!(!is_place_host_key_changed(&e));

    let e = place_provisioning("old-macbook is being set up");
    assert_eq!(e.kind, Some(Kind::Conflict));
    assert!(is_place_provisioning(&e));

    for (e, is) in [
        (place_host_key_changed("key moved"), is_place_host_key_changed as fn(&HostError) -> bool),
        (place_forks_nowhere("no backend"), is_place_forks_nowhere),
        (place_add_taken_back("taken back"), is_place_add_taken_back),
    ] {
        assert_eq!(e.kind, None, "{}", e.message);
        assert!(is(&e), "{}", e.message);
        assert!(!is_place_login_refused(&e));
        assert!(!is_place_provisioning(&e));
    }
}

#[test]
fn predicate_false_for_same_words_unmarked() {
    // T3: a predicate reads the mark, never the words or the kind alone.
    let plain = HostError::with_kind("ssh: Permission denied (publickey)", Kind::parse("login"));
    assert!(!is_place_login_refused(&plain));
    let conflict = HostError::with_kind("old-macbook is being set up", Kind::Conflict);
    assert!(!is_place_provisioning(&conflict));
    assert!(!is_place_add_taken_back(&HostError::new("taken back")));
}

#[test]
fn record_reads_ts_document_and_omits_absent() {
    // T4: the TS-written record (reportedAt before report) reads, and absent fields are never written as null.
    let doc = record_json(json!({ "dialled": null, "setup": null, "sync": null, "backendFacts": null }));
    let held: PlaceRecord = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(held.id, PLACE);
    assert_eq!(held.reported_at.as_deref(), Some("2026-10-02T00:00:00.000Z"));
    assert_eq!(held.road.as_ref().unwrap().key_path.as_deref(), Some("/home/dev/.ssh/id"));
    assert_eq!(held.applied.as_ref().unwrap().rows[2].pick.as_ref().unwrap().from, "/home/dev/site");
    let back = js(&held);
    for k in ["dialled", "setup", "sync", "backendFacts"] {
        assert!(back.get(k).is_none(), "{k} written though absent");
    }
    // Every key the TS wrote reads back with its value.
    for (k, v) in doc.as_object().unwrap() {
        assert_eq!(back.get(k), Some(v), "{k}");
    }
}

#[test]
fn record_round_trip_keeps_values() {
    // T4: a record written and read again is the same record.
    let held = record(json!({ "dialled": { "at": "2026-10-03T00:00:00.000Z", "answered": false, "said": "no" } }));
    let again: PlaceRecord = serde_json::from_value(js(&held)).unwrap();
    assert_eq!(again, held);
    // A key this port does not know still reads.
    let mut doc = record_json(json!({}));
    doc["comesLater"] = json!(true);
    assert!(serde_json::from_value::<PlaceRecord>(doc).is_ok());
}

#[test]
fn road_view_drops_key_path_and_host_key() {
    // T5.
    let held = record(json!({}));
    let view = held.road.as_ref().unwrap().view();
    assert_eq!(js(&view), json!({ "ssh": "maya@10.0.0.5", "from": "10.0.0.5" }));
    let only_key = PlaceRecordRoad { key_path: Some("/k".into()), ..Default::default() };
    assert_eq!(js(&only_key.view()), json!({}));
}

#[test]
fn is_place_record_cases() {
    // T6, each case the oracle ran.
    for case in at("/types/isPlaceRecord").as_array().unwrap() {
        assert_eq!(is_place_record(&case["v"]), case["out"].as_bool().unwrap(), "{}", case["v"]);
    }
    assert!(place_record_of(&record_json(json!({}))).is_some());
    // A record shape whose report does not deserialize is a record by T6 but not a PlaceRecord.
    assert!(place_record_of(&json!({ "id": "a", "publicKey": "k", "name": "n" })).is_none());
    assert!(place_record_of(&json!({ "id": 1 })).is_none());
}

#[test]
fn applied_view_drops_pick_and_created_at() {
    // T7, T9.
    let held = record(json!({}));
    assert_eq!(js(&applied_view(held.applied.as_ref().unwrap())), at("/types/appliedView"));
}

#[test]
fn made_by_setup_only_installed_in_pick_order() {
    // T8: lint installed, fmt present (the computer had it), old never applied.
    let held = record(json!({}));
    assert_eq!(js(&made_by_setup(&held, MadeKind::Plugins)), at("/types/madeBySetup"));
    let bare = record(json!({ "applied": null }));
    assert_eq!(js(&made_by_setup(&bare, MadeKind::Plugins)), at("/types/madeBySetupNoApplied"));
    let no_picks = record(json!({ "picks": null }));
    assert!(made_by_setup(&no_picks, MadeKind::Plugins).is_empty());
}

#[test]
fn projects_made_in_row_order_once() {
    // T10.
    let held = record(json!({}));
    assert_eq!(js(&projects_made(&held).into_iter().collect::<Vec<_>>()), at("/types/projectsMade"));
    let mut doc = record_json(json!({}));
    let rows = doc["applied"]["rows"].as_array_mut().unwrap();
    rows.push(json!({ "id": "folders/b", "label": "FOLDERS/B", "outcome": "installed", "project": { "id": "pr_00000002", "name": "b" } }));
    rows.push(json!({ "id": "folders/c", "label": "FOLDERS/C", "outcome": "present", "project": { "id": "pr_00000001", "name": "site" } }));
    let held: PlaceRecord = serde_json::from_value(doc).unwrap();
    assert_eq!(js(&projects_made(&held).into_iter().collect::<Vec<_>>()), at("/types/projectsMadeTwice"));
    assert!(projects_made(&record(json!({ "applied": null }))).is_empty());
}

#[test]
fn lines_match_oracle() {
    // T11-T16.
    let l = at("/types/lines");
    let s = |k: &str| l[k].as_str().unwrap().to_string();
    assert_eq!(NO_PLACE_DOOR, s("NO_PLACE_DOOR"));
    assert_eq!(NO_PLACE_UPDATER, s("NO_PLACE_UPDATER"));
    assert_eq!(place_update_slow_line("old-macbook", 60), s("placeUpdateSlowLine"));
    assert_eq!(place_swept_over_ssh_line("old-macbook"), s("placeSweptOverSshLine"));
    assert_eq!(place_login_road_line("old-macbook", "maya@10.0.0.5", None), s("placeLoginRoadLine"));
    assert_eq!(place_login_road_line("old-macbook", "maya@10.0.0.5", Some("systemctl failed")), s("placeLoginRoadLineSaid"));
    assert_eq!(place_login_road_line("old-macbook", "maya@10.0.0.5", Some("")), s("placeLoginRoadLineEmpty"));
    assert_eq!(place_swept_over_link_line("old-macbook", "maya@10.0.0.5", Some("systemctl failed")), s("placeSweptOverLinkLine"));
    assert_eq!(place_elsewhere_swept_over_link_line("old-macbook", "maya@10.0.0.5 reaches another machine"), s("placeElsewhereSweptOverLinkLine"));
}

#[test]
fn collection_names() {
    // T17: table names other processes read (spec 02 rule 2.4).
    assert_eq!((PLACES, CAPS, DEFAULT_COLLECTION, DEFAULT_ID), ("places", "caps", "place-default", "default"));
    let _: Value = json!(null);
}
