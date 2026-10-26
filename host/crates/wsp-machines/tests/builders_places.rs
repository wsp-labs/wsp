// SPDX-License-Identifier: AGPL-3.0-only
//! builders/places (spec `builders_places.md`, rules P1-P11): where an image is built and how a builder is shown.
//! The TS covers these through whole-host cases (sealed-image, copy-on-press, places-image, places-fork); each test
//! here names its rule and the TS line it stands for, and the four named after a TS case stand for the cases of those
//! files that reach this unit's functions and no other unit's.

mod images_support;

use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;

use images_support::*;
use serde_json::{json, Value};

use wsp_core::types::internal::{BuilderLife, BuilderRecord, NO_COPY_RECIPE};
use wsp_core::types::wiring::GoldenRecipe;
use wsp_engine::machine::MachineBackend;
use wsp_machines::images::builders::BuildersArea;
use wsp_machines::images::BuilderRef;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::wire::machine_link::{no_such_place_refusal, place_builds_no_image_line};
use wsp_words::exit::{HostError, Kind};
use wsp_words::words::thread::{place_forks_nothing_pick_line, NO_PROVIDER_LINE};

fn area(w: &Rc<World>) -> Rc<BuildersArea> {
    BuildersArea::new(w.clone())
}

fn same(a: &Arc<dyn MachineBackend>, b: &Arc<FakeBackend>) -> bool {
    std::ptr::eq(Arc::as_ptr(a) as *const (), Arc::as_ptr(b) as *const ())
}

fn err<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected a refusal, got an answer"),
    }
}

/// The provider table a test wants: these rows in this order and this one wired.
fn set_rows(w: &World, wired: &str, rows: &[(&str, &Arc<FakeBackend>)]) {
    *w.places.wired.borrow_mut() = wired.to_string();
    let mut table = w.places.rows.borrow_mut();
    table.clear();
    for (id, b) in rows {
        table.insert(id.to_string(), (*b).clone());
    }
}

fn record(extra: Value) -> BuilderRecord {
    let mut base = json!({
        "id": "b1", "name": "default", "kind": "sandbox", "baseTemplate": "base", "setupSha": "sha-1",
        "createdAt": "2026-10-25T10:00:00.000Z", "size": {"cpu": 2, "memMb": 4096}, "firstLife": true
    });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).expect("a builder record")
}

fn entry_of(r: BuilderRecord, life: BuilderLife) -> BuilderRef {
    let b = live_builder("b1", "default", "boat", life);
    b.borrow_mut().record = r;
    b
}

fn view_json(area: &BuildersArea, b: &BuilderRef) -> Value {
    let record = b.borrow().record.clone();
    let view = area.builder_view(&record, &b.borrow());
    serde_json::to_value(view).unwrap()
}

fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

// ---------------------------------------------------------------------------------------------------------------
// P1: the view of a builder
// ---------------------------------------------------------------------------------------------------------------

// P1: builders.ts:57-78. Every fact a record has, keyed in the TS literal's order.
#[test]
fn builder_view_carries_every_fact_in_the_ts_key_order() {
    let w = World::new();
    let a = area(&w);
    let b = entry_of(
        record(json!({
            "streamUrl": "wss://screen.example/1", "building": true, "place": "boat",
            "import": {"recipeHash": "h1", "recipe": {"ticks": [], "files": []}, "applied": [], "smoke": ""},
            "sealed": {"at": "2026-10-25T10:30:00.000Z", "version": 3}
        })),
        BuilderLife::Own,
    );
    let view = view_json(&a, &b);
    assert_eq!(
        keys(&view),
        [
            "id",
            "name",
            "kind",
            "createdAt",
            "placeId",
            "placeName",
            "size",
            "rateUsdPerHour",
            "screen",
            "sealable",
            "recipeHash",
            "recipe",
            "building",
            "sealed"
        ]
    );
    assert_eq!(
        view,
        json!({
            "id": "b1", "name": "default", "kind": "sandbox", "createdAt": "2026-10-25T10:00:00.000Z", "placeId": "boat", "placeName": "boat",
            "size": {"cpu": 2, "memMb": 4096}, "rateUsdPerHour": 0.1, "screen": {"streamUrl": "wss://screen.example/1"}, "sealable": true,
            "recipeHash": "h1", "recipe": {"ticks": [], "files": []}, "building": true, "sealed": {"at": "2026-10-25T10:30:00.000Z", "version": 3}
        })
    );

    // A record with no place is shown at the wired provider; one with nothing else carries only the bare facts.
    let bare = entry_of(record(json!({})), BuilderLife::Own);
    bare.borrow_mut().sealable = false;
    let view = view_json(&a, &bare);
    assert_eq!(
        keys(&view),
        [
            "id",
            "name",
            "kind",
            "createdAt",
            "placeId",
            "placeName",
            "size",
            "rateUsdPerHour",
            "sealable"
        ]
    );
    assert_eq!(view["placeId"], json!("boat"));
    assert_eq!(view["sealable"], json!(false));
}

// P1: builders.ts:72-73. A foreign owner by life alone, with the machine's `wsp-owner` label or ""; a holder only by life.
#[test]
fn builder_view_names_a_foreign_owner_and_a_holder_only_by_life() {
    let w = World::new();
    let a = area(&w);
    let held_by = json!({"host": "host-2", "pid": 77, "heartbeat": "2026-10-25T10:29:00.000Z"});
    let labelled = |life: BuilderLife, owner: Option<&str>, extra: Value| {
        let b = entry_of(record(extra), life);
        let machine = match owner {
            Some(o) => FakeMachine::labelled("b1", &[("wsp-owner", o)]),
            None => FakeMachine::new("b1"),
        };
        b.borrow_mut().builder.machine = machine;
        view_json(&a, &b)
    };

    let foreign = labelled(
        BuilderLife::Foreign,
        Some("host-2"),
        json!({"heldBy": held_by}),
    );
    assert_eq!(foreign["foreignOwner"], json!("host-2"));
    assert!(
        foreign.get("heldBy").is_none(),
        "heldBy is for the held life alone"
    );
    assert_eq!(
        labelled(BuilderLife::Foreign, None, json!({}))["foreignOwner"],
        json!("")
    );
    assert!(labelled(BuilderLife::Own, Some("host-2"), json!({}))
        .get("foreignOwner")
        .is_none());

    let held = labelled(BuilderLife::Held, None, json!({"heldBy": held_by}));
    assert_eq!(held["heldBy"], held_by);
    assert!(held.get("foreignOwner").is_none());
    assert!(
        labelled(BuilderLife::Held, None, json!({}))
            .get("heldBy")
            .is_none(),
        "a held builder whose record names no holder shows none"
    );
    assert!(
        labelled(BuilderLife::Reusable, None, json!({"heldBy": held_by}))
            .get("heldBy")
            .is_none()
    );
}

// P1: builders.ts:59-60. The rate is the place's backend's, the row first and the door's after; absent with neither.
#[test]
fn builder_view_has_no_rate_without_a_backend() {
    let w = World::new();
    let a = area(&w);
    let at_lab = || entry_of(record(json!({"place": "p_lab"})), BuilderLife::Own);

    let view = view_json(&a, &at_lab());
    assert!(view.get("rateUsdPerHour").is_none());
    assert_eq!(view["placeId"], json!("p_lab"));
    assert_eq!(
        view["placeName"],
        json!("p_lab"),
        "no door names it, so the id"
    );

    let door = w.with_door();
    door.computer("p_lab", "lab", None);
    let view = view_json(&a, &at_lab());
    assert!(
        view.get("rateUsdPerHour").is_none(),
        "a computer the door names but has no backend for"
    );
    assert_eq!(view["placeName"], json!("lab"));

    door.backends
        .borrow_mut()
        .insert("p_lab".into(), FakeBackend::new("lab"));
    let view = view_json(&a, &at_lab());
    assert_eq!(view["rateUsdPerHour"], json!(0.1));
}

// ---------------------------------------------------------------------------------------------------------------
// P2 to P5: places
// ---------------------------------------------------------------------------------------------------------------

// P2: builders.ts:191-195; places-fork.test.ts:109. The row, then the door, else a refusal naming every provider held.
#[test]
fn backend_at_reads_the_row_then_the_door_else_refuses_missing() {
    let w = World::new();
    let (box_, lab) = (FakeBackend::new("box"), FakeBackend::new("lab"));
    let boat = w.runtime_backend.clone();
    set_rows(&w, "boat", &[("boat", &boat), ("box", &box_)]);
    let a = area(&w);
    assert!(same(&a.backend_at("box").expect("the row"), &box_));
    let door = w.with_door();
    door.computer("p_lab", "lab", Some(lab.clone()));
    assert!(same(&a.backend_at("p_lab").expect("the door's"), &lab));
    assert!(same(
        &a.backend_at("boat").expect("the row before the door"),
        &boat
    ));

    let e = err(a.backend_at("nowhere"));
    assert_eq!(e.kind, Some(Kind::Missing));
    assert_eq!(
        e.message,
        no_such_place_refusal("nowhere", &["boat".to_string(), "Boat".to_string()])
    );
    assert_eq!(e.message, "no place named nowhere; you have boat, Boat");
}

// P3: builders.ts:201-202. A provider row named by the word answers as it is, whatever the door says.
#[test]
fn place_at_takes_a_provider_row_as_it_is() {
    let w = World::new();
    let solari = FakeBackend::new("solari");
    w.places
        .rows
        .borrow_mut()
        .insert("solari".into(), solari.clone());
    let a = area(&w);
    run(async {
        let at = a.place_at("solari").await.unwrap();
        assert_eq!(at.place, "solari");
        assert!(same(&at.at, &solari));
        w.with_door();
        let at = a.place_at("solari").await.unwrap();
        assert_eq!(at.place, "solari");
        assert!(
            w.lines("door").is_empty(),
            "the door is not asked for a word a provider row answers to"
        );
    });
}

// P3: builders.ts:203-206; places-fork.test.ts:109, sealed-image.test.ts:689. With no door a cloud is read by the name a
// person types for it, and a word nothing answers to is the missing refusal.
#[test]
fn place_at_without_a_door_reads_the_cloud_word() {
    let w = World::new();
    let (solari, box_) = (FakeBackend::new("solari"), FakeBackend::new("box"));
    set_rows(&w, "solari", &[("solari", &solari), ("box", &box_)]);
    let a = area(&w);
    run(async {
        for word in ["boat", "Boat", "BOAT"] {
            let at = a.place_at(word).await.unwrap();
            assert_eq!(at.place, "box", "{word}");
            assert!(same(&at.at, &box_), "{word}");
        }
        let at = a.place_at("Solari").await.unwrap();
        assert_eq!(at.place, "solari");
        assert!(same(&at.at, &solari));
        let e = err(a.place_at("nowhere").await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Missing));
        assert_eq!(
            e.message,
            no_such_place_refusal("nowhere", &["Solari".to_string(), "Boat".to_string()])
        );
        assert!(e.message.ends_with("; you have Solari, Boat"));
    });
}

// P3: builders.ts:207-221. With a door the word goes to `place_for`, and a place id answers with the computer's forking backend.
#[test]
fn place_at_asks_the_door_and_its_forking_backend() {
    let w = World::new();
    let lab = FakeBackend::new("lab");
    let door = w.with_door();
    door.computer("p_srv", "srv", Some(lab.clone()));
    door.words
        .borrow_mut()
        .insert("srv".into(), Ok(Some("p_srv".into())));
    door.words
        .borrow_mut()
        .insert("p_srv".into(), Ok(Some("p_srv".into())));
    let a = area(&w);
    run(async {
        for word in ["srv", "p_srv"] {
            let at = a.place_at(word).await.unwrap();
            assert_eq!(at.place, "p_srv", "{word}");
            assert!(same(&at.at, &lab), "{word}");
        }
        assert_eq!(
            w.log(),
            [
                "door place_for srv",
                "door forking_backend p_srv",
                "door place_for p_srv",
                "door forking_backend p_srv"
            ]
        );
    });
}

// P3: builders.ts:210-212; places-image.test.ts:57. A refusal from the door is rethrown with its words and the missing kind.
#[test]
fn place_at_marks_a_door_refusal_missing() {
    let w = World::new();
    let door = w.with_door();
    door.words.borrow_mut().insert(
        "srv".into(),
        Err(HostError::new("no place named srv; you have boat")),
    );
    door.words.borrow_mut().insert(
        "busy".into(),
        Err(HostError {
            kind: Some(Kind::Conflict),
            ..HostError::new("busy cannot run wsp workspaces")
        }),
    );
    let a = area(&w);
    run(async {
        let e = err(a.place_at("srv").await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Missing));
        assert_eq!(e.message, "no place named srv; you have boat");
        let e = err(a.place_at("busy").await.map(|_| ()));
        assert_eq!(
            e.kind,
            Some(Kind::Missing),
            "the kind is stamped over whatever the error carried"
        );
        assert_eq!(e.message, "busy cannot run wsp workspaces");
        assert!(w.lines("door forking_backend").is_empty());
    });
}

// P3: builders.ts:213-218; places-fork.test.ts:124. A word the door names no place for may still name the wired provider.
#[test]
fn place_at_reads_the_wired_providers_name_when_the_door_names_no_place() {
    let w = World::new();
    let (solari, box_) = (FakeBackend::new("solari"), FakeBackend::new("box"));
    set_rows(&w, "solari", &[("solari", &solari), ("box", &box_)]);
    w.with_door();
    let a = area(&w);
    run(async {
        for word in ["SOLARI", "solari"] {
            let at = a.place_at(word).await.unwrap();
            assert_eq!(at.place, "solari", "{word}");
            assert!(same(&at.at, &solari), "{word}");
        }
        assert_eq!(
            w.lines("door place_for"),
            ["door place_for SOLARI"],
            "the row answers the exact id before the door is asked"
        );
    });
}

// P3: builders.ts:216; places-image.test.ts:80. A word naming no place that builds, this computer's own name among them,
// is the one conflict sentence.
#[test]
fn place_at_refuses_a_word_naming_nothing_that_builds() {
    let w = World::new();
    w.with_door();
    let a = area(&w);
    run(async {
        let e = err(a.place_at("this-mac").await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, place_builds_no_image_line("this-mac"));
        assert!(e
            .message
            .starts_with("this-mac takes no copy of your image"));

        // The wired provider named, with no backend in the table, builds nothing either.
        set_rows(&w, "ghost", &[]);
        let e = err(a.place_at("ghost").await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, place_builds_no_image_line("ghost"));
    });
}

// P3: no TS line beyond places-image.test.ts:17. A computer by its name or its id, its backend learned from the computer
// itself, a word the door has no place for refused in the door's words.
#[test]
fn a_copy_build_names_the_computer_by_its_name_or_id_learns_its_backend_from_the_computer_itself_and_is_refused_at_the_record_rather_than_at_the_name(
) {
    let w = World::new();
    let lab = FakeBackend::new("srv");
    let door = w.with_door();
    door.computer("p_0a1b", "srv", Some(lab.clone()));
    door.words
        .borrow_mut()
        .insert("srv".into(), Ok(Some("p_0a1b".into())));
    door.words
        .borrow_mut()
        .insert("p_0a1b".into(), Ok(Some("p_0a1b".into())));
    door.words.borrow_mut().insert(
        "nowhere".into(),
        Err(HostError::new("no place named nowhere; you have srv, Boat")),
    );
    let a = area(&w);
    run(async {
        for word in ["srv", "p_0a1b"] {
            let at = a.place_at(word).await.unwrap();
            assert_eq!(
                (at.place.as_str(), same(&at.at, &lab)),
                ("p_0a1b", true),
                "{word}"
            );
        }
        assert_eq!(
            w.lines("door forking_backend"),
            ["door forking_backend p_0a1b", "door forking_backend p_0a1b"]
        );
        let e = err(a.place_at("nowhere").await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Missing));
        assert!(
            e.message.contains("no place named nowhere; you have") && e.message.contains("srv")
        );
        assert!(lab.created.borrow().is_empty(), "nothing was made there");
    });
}

// P3: no TS line beyond places-fork.test.ts:124. A cloud by the name a person types for it, in any case, on the image
// road: Boat is the provider row `box`, and the provider this host forks on stays itself by either word.
#[test]
fn reads_a_cloud_by_the_name_a_person_types_for_it_in_any_case_on_the_fork_road_and_the_image_road_alike(
) {
    let w = World::new();
    let (solari, box_) = (FakeBackend::new("solari"), FakeBackend::new("box"));
    set_rows(&w, "solari", &[("solari", &solari), ("box", &box_)]);
    let door = w.with_door();
    door.backends
        .borrow_mut()
        .insert("box".into(), box_.clone());
    for word in ["boat", "Boat", "BOAT"] {
        door.words
            .borrow_mut()
            .insert(word.into(), Ok(Some("box".into())));
    }
    door.words.borrow_mut().insert(
        "nowhere".into(),
        Err(HostError::new(
            "no place named nowhere; you have Solari, Boat",
        )),
    );
    let a = area(&w);
    run(async {
        for word in ["boat", "Boat", "BOAT", "box"] {
            let at = a.place_at(word).await.unwrap();
            assert_eq!(
                (at.place.as_str(), same(&at.at, &box_)),
                ("box", true),
                "{word}"
            );
        }
        for word in ["solari", "Solari", "SOLARI"] {
            let at = a.place_at(word).await.unwrap();
            assert_eq!(
                (at.place.as_str(), same(&at.at, &solari)),
                ("solari", true),
                "{word}"
            );
        }
        let e = err(a.place_at("nowhere").await.map(|_| ()));
        assert!(e.message.ends_with("; you have Solari, Boat"));
    });
}

// P4: builders.ts:233-239. The computer is asked first and its answer dropped; the backend is then read off the record, and
// one that forks nothing answers none.
#[test]
fn forking_at_asks_the_computer_first_and_drops_a_backend_that_forks_nothing() {
    let w = World::new();
    let (lab, idle) = (FakeBackend::new("lab"), FakeBackend::forkless("idle"));
    let door = w.with_door();
    door.computer("p_lab", "lab", Some(lab.clone()));
    door.computer("p_idle", "idle", Some(idle));
    let a = area(&w);
    run(async {
        let at = a
            .forking_at(Some("p_lab"))
            .await
            .unwrap()
            .expect("a backend that forks");
        assert!(same(&at, &lab));
        assert_eq!(
            w.log(),
            ["door forking_backend p_lab", "backend_of_kind cloud p_lab"]
        );
        assert!(a.forking_at(Some("p_idle")).await.unwrap().is_none());

        w.log.borrow_mut().clear();
        let own = a
            .forking_at(None)
            .await
            .unwrap()
            .expect("the runtime's own backend forks");
        assert!(same(&own, &w.runtime_backend));
        assert_eq!(
            w.log(),
            ["backend_of_kind cloud -"],
            "no place named, no computer asked"
        );
    });
}

// P4: builders.ts:236. The computer's own refusal fails the call before any backend is read.
#[test]
fn forking_at_fails_with_the_computers_refusal() {
    let w = World::new();
    let door = w.with_door();
    door.forking_fails.borrow_mut().insert(
        "p_lab".into(),
        HostError {
            kind: Some(Kind::Conflict),
            ..HostError::new("lab cannot run wsp workspaces: it mounts cgroup v1 at /sys/fs/cgroup")
        },
    );
    let a = area(&w);
    run(async {
        let e = err(a.forking_at(Some("p_lab")).await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(
            e.message,
            "lab cannot run wsp workspaces: it mounts cgroup v1 at /sys/fs/cgroup"
        );
        assert_eq!(w.log(), ["door forking_backend p_lab"]);
    });
}

// P5: builders.ts:226-229; sealed-image.test.ts:700. Nothing runs here: the one no-provider sentence.
#[test]
fn landing_backend_refuses_with_no_provider_line_when_nothing_runs() {
    let w = World::new();
    w.runtime_backend.caps.borrow_mut().sizes.clear();
    let a = area(&w);
    run(async {
        let e = err(a.landing_backend(None).await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, NO_PROVIDER_LINE);
        assert_eq!(w.log(), ["backend_of_kind cloud -"]);
    });
}

// P5: builders.ts:229; sealed-image.test.ts:708 (the fork gated on the place it lands on). A place that forks nothing
// names the places that do, providers by their word and computers by their name; with none left the one sentence.
#[test]
fn landing_backend_names_the_places_that_run() {
    let w = World::new();
    let (none, solari) = (FakeBackend::forkless("none"), FakeBackend::new("solari"));
    set_rows(&w, "none", &[("none", &none), ("solari", &solari)]);
    w.runtime_backend.caps.borrow_mut().sizes.clear();
    let door = w.with_door();
    door.backends
        .borrow_mut()
        .insert("none".into(), none.clone());
    let a = area(&w);
    run(async {
        let want = place_forks_nothing_pick_line("none", &["Solari".to_string()]);
        for place in [Some("none"), None] {
            let e = err(a.landing_backend(place).await.map(|_| ()));
            assert_eq!(e.kind, Some(Kind::Conflict), "{place:?}");
            assert_eq!(e.message, want, "{place:?}");
        }
        assert_eq!(
            want,
            "none forks no machines; name a place that runs workspaces with --on: Solari"
        );

        // A joined computer that forks is named by its own name, after the providers.
        door.computer("p_lab", "lab", Some(FakeBackend::new("lab")));
        let e = err(a.landing_backend(Some("none")).await.map(|_| ()));
        assert_eq!(
            e.message,
            place_forks_nothing_pick_line("none", &["Solari".to_string(), "lab".to_string()])
        );

        // And alone, with no place that runs workspaces, the no-provider sentence.
        set_rows(&w, "none", &[("none", &none)]);
        door.rows.borrow_mut().clear();
        let e = err(a.landing_backend(Some("none")).await.map(|_| ()));
        assert_eq!(e.message, NO_PROVIDER_LINE);
        let landed = a
            .landing_backend(Some("p_lab"))
            .await
            .expect("a place that forks");
        assert!(landed.capabilities().sizes.len() == 2);
    });
}

// P5: builders.ts:229. The host's own sentence replaces the no-provider one where it wired one.
#[test]
fn landing_backend_uses_the_hosts_own_no_machines_line() {
    let w = World::new();
    w.runtime_backend.caps.borrow_mut().sizes.clear();
    w.opts.borrow_mut().no_machines_line = Some("this host starts no machines of its own".into());
    let a = area(&w);
    run(async {
        let e = err(a.landing_backend(None).await.map(|_| ()));
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, "this host starts no machines of its own");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// P6 to P9: names, kinds and the list of places
// ---------------------------------------------------------------------------------------------------------------

// P6: builders.ts:242. The door's name, else this computer's hostname, else the provider's word.
#[test]
fn place_name_reads_the_door_then_hostname_then_provider_word() {
    let w = World::new();
    let a = area(&w);
    assert_eq!(a.place_name("here"), "laptop");
    assert_eq!(a.place_name("box"), "Boat");
    assert_eq!(a.place_name("p_0a1b"), "p_0a1b");
    let door = w.with_door();
    door.computer("p_lab", "lab", None);
    assert_eq!(a.place_name("p_lab"), "lab");
    assert_eq!(
        a.place_name("here"),
        "laptop",
        "a door that names no such place leaves the hostname"
    );
    door.names
        .borrow_mut()
        .insert("here".into(), "zingzy's MacBook Pro".into());
    assert_eq!(
        a.place_name("here"),
        "zingzy's MacBook Pro",
        "the door's name before the hostname"
    );
}

// P7: builders.ts:245. The record's place, else the provider this host forks on.
#[test]
fn image_place_reads_the_record_else_wired() {
    let w = World::new();
    let a = area(&w);
    run(async {
        assert_eq!(a.image_place("default").await.unwrap(), "boat");
        w.put_image_now(&sealed_image("default", 1, "h1", Some("solari")));
        assert_eq!(a.image_place("default").await.unwrap(), "solari");
        w.put_image_now(&sealed_image("older", 1, "h0", None));
        assert_eq!(
            a.image_place("older").await.unwrap(),
            "boat",
            "a record sealed before places"
        );
        assert_eq!(a.image_place("other").await.unwrap(), "boat");
    });
}

// P8: builders.ts:247. A provider row is a provider; any other place an image builds at is a computer.
#[test]
fn place_kind_of_tells_provider_from_computer() {
    let w = World::new();
    let a = area(&w);
    assert!(a.place_kind_of("boat") == PlaceKind::Provider);
    assert!(a.place_kind_of("p_lab") == PlaceKind::Computer);
    assert!(a.place_kind_of("here") == PlaceKind::Computer);
}

fn tuples(rows: &[wsp_machines::images::builders::BuildPlace]) -> Vec<(String, String, bool)> {
    rows.iter()
        .map(|r| {
            (
                r.place.clone(),
                r.name.clone(),
                r.kind == PlaceKind::Provider,
            )
        })
        .collect()
}

// P9: builders.ts:250-263. The provider rows that fork, in table order, then the door's joined computers that fork.
#[test]
fn build_places_lists_forking_providers_then_joined_computers() {
    let w = World::new();
    let (idle, solari) = (FakeBackend::forkless("idle"), FakeBackend::new("solari"));
    let boat = w.runtime_backend.clone();
    set_rows(
        &w,
        "boat",
        &[("boat", &boat), ("idle", &idle), ("solari", &solari)],
    );
    let (a_lab, b_lab) = (FakeBackend::new("a"), FakeBackend::new("b"));
    let door = w.with_door();
    door.computer("here", "this-mac", Some(FakeBackend::new("here")));
    door.computer("p_a", "lab-a", Some(a_lab.clone()));
    door.rows.borrow_mut().push(
        serde_json::from_value(
            json!({"id": "box", "kind": "provider", "name": "box", "default": false}),
        )
        .unwrap(),
    );
    door.computer("p_b", "lab-b", Some(b_lab.clone()));
    let area = area(&w);
    run(async {
        let rows = area.build_places().await.unwrap();
        assert_eq!(
            tuples(&rows),
            vec![
                ("boat".into(), "boat".into(), true),
                ("solari".into(), "solari".into(), true),
                ("p_a".into(), "lab-a".into(), false),
                ("p_b".into(), "lab-b".into(), false)
            ]
        );
        assert!(
            same(&rows[1].backend, &solari)
                && same(&rows[2].backend, &a_lab)
                && same(&rows[3].backend, &b_lab)
        );
        assert_eq!(w.lines("door list"), [format!("door list {NOW}")]);
        assert_eq!(
            w.lines("door forking_backend"),
            ["door forking_backend p_a", "door forking_backend p_b"],
            "this computer and a provider row are never asked"
        );
    });
}

// P9: builders.ts:260. A computer whose backend read fails is left out, silently.
#[test]
fn build_places_skips_a_computer_whose_backend_fails() {
    let w = World::new();
    let door = w.with_door();
    door.computer("p_a", "lab-a", Some(FakeBackend::new("a")));
    door.computer("p_b", "lab-b", Some(FakeBackend::new("b")));
    door.computer("p_c", "lab-c", None);
    door.forking_fails
        .borrow_mut()
        .insert("p_b".into(), HostError::new("lab-b is not answering"));
    let area = area(&w);
    run(async {
        let rows = area.build_places().await.unwrap();
        assert_eq!(
            tuples(&rows),
            vec![
                ("boat".into(), "boat".into(), true),
                ("p_a".into(), "lab-a".into(), false)
            ]
        );
        assert_eq!(
            w.lines("door forking_backend"),
            [
                "door forking_backend p_a",
                "door forking_backend p_b",
                "door forking_backend p_c"
            ]
        );
        assert!(w.warned.lines().is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// P10, P11: recipes and labels
// ---------------------------------------------------------------------------------------------------------------

// P10: builders.ts:267-271. A host that wired no copy recipe says so in the one sentence, with no kind.
#[test]
fn copy_recipe_or_throw_refuses_without_one() {
    let w = World::new();
    let a = area(&w);
    let e = err(a.copy_recipe_or_throw().map(|_| ()));
    assert_eq!(e.message, NO_COPY_RECIPE);
    assert_eq!(e.kind, None);
    assert!(e.fix.is_none());

    let log = w.log.clone();
    w.opts.borrow_mut().copy_recipe = Some(Rc::new(move |image, cloud| {
        log.borrow_mut()
            .push(format!("copy_recipe {} cloud={cloud}", image.hash));
        async { Ok(recipe("echo copy")) }.boxed_local()
    }));
    let compose = a.copy_recipe_or_throw().expect("the wired one");
    run(async {
        let made = compose(sealed_image("default", 1, "h1", None), true)
            .await
            .unwrap();
        assert_eq!(made.setup, "echo copy");
    });
    assert_eq!(w.log(), ["copy_recipe h1 cloud=true"]);
}

fn setup_of(r: Result<GoldenRecipe, HostError>) -> String {
    r.expect("a recipe").setup
}

// P10: builders.ts:275-279. The recipe the call names, else the one the runtime was wired with.
#[test]
fn recipe_or_throw_prefers_the_named_recipe() {
    let w = World::new();
    let a = area(&w);
    w.opts.borrow_mut().golden_recipe = Some(recipe("wired"));
    assert_eq!(setup_of(a.recipe_or_throw(Some(recipe("named")))), "named");
    assert_eq!(setup_of(a.recipe_or_throw(None)), "wired");
    w.opts.borrow_mut().golden_recipe = None;
    assert_eq!(setup_of(a.recipe_or_throw(Some(recipe("named")))), "named");
}

// P10: builders.ts:277. With neither, a plain error in the TS's own words.
#[test]
fn recipe_or_throw_refuses_without_one() {
    let w = World::new();
    let a = area(&w);
    let e = err(a.recipe_or_throw(None));
    assert_eq!(e.message, "this runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run");
    assert_eq!(e.kind, None);
}

fn pairs(m: &indexmap::IndexMap<String, String>) -> Vec<(String, String)> {
    m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn kv(k: &str, v: &str) -> (String, String) {
    (k.to_string(), v.to_string())
}

// P11: builders.ts:281. The extra labels first in their order, then the four marks.
#[test]
fn builder_labels_put_the_four_marks_after_the_extra_ones() {
    let w = World::new();
    let a = area(&w);
    let extra: indexmap::IndexMap<String, String> = [kv("team", "infra"), kv("batch", "7")]
        .into_iter()
        .collect();
    let marks = [
        kv("wsp", "1"),
        kv("wsp-builder", "1"),
        kv("wsp-owner", "owner-1"),
        kv("createdAt", "2026-10-25T09:00:00.000Z"),
    ];
    let got = pairs(&a.builder_labels(Some(&extra)));
    assert_eq!(
        got,
        [vec![kv("team", "infra"), kv("batch", "7")], marks.to_vec()].concat()
    );
    assert_eq!(pairs(&a.builder_labels(None)), marks.to_vec());
    w.clock.advance(61_000);
    assert_eq!(
        a.builder_labels(None).get("createdAt").map(String::as_str),
        Some("2026-10-25T09:01:01.000Z"),
        "the context clock, not the wall's"
    );
}

// P11: builders.ts:281 (the JS spread). An extra label of a mark's name keeps its place and takes the new value.
#[test]
fn builder_labels_keep_an_overridden_keys_place() {
    let w = World::new();
    let a = area(&w);
    let extra: indexmap::IndexMap<String, String> = [
        kv("wsp-owner", "someone-else"),
        kv("wsp", "0"),
        kv("x", "y"),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        pairs(&a.builder_labels(Some(&extra))),
        vec![
            kv("wsp-owner", "owner-1"),
            kv("wsp", "1"),
            kv("x", "y"),
            kv("wsp-builder", "1"),
            kv("createdAt", "2026-10-25T09:00:00.000Z")
        ]
    );
}
