// SPDX-License-Identifier: AGPL-3.0-only
//! Spec boot_load.md: the tests the rules table names beside the scenes, each over the fake runtime of
//! `support/boot.rs`. Expected values come from `boot.ts`, the TS tests the table cites and the scene fixtures.
mod common;
#[path = "support/boot.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_core::types::internal::{absent_machine, BuilderLife, StoredBuilder};
use wsp_engine::machine::MachineKind;
use wsp_machines::machines::boot::load::{late_read_line, LOAD_READS_MS};
use wsp_words::exit::HostError;
use wsp_words::words::thread::GoneSeenBy;

use common::{MemStore, StoreLogs};
use support::{asked_err, asked_missing, live_workspace, workspace_record, ws, GatedStore, Opts, Sight, World};

fn builder(id: &str) -> Value {
    json!({"id": id, "name": format!("b-{id}"), "kind": "sandbox", "baseTemplate": "base", "setupSha": "sha", "createdAt": "2026-09-01T00:00:00.000Z", "firstLife": true})
}

fn load_world(o: Opts) -> World {
    World::load(Opts { sight_word: Some("sightMachine"), sight_first: true, ..o })
}

/// boot.ts:18.
#[test]
fn the_load_waits_one_and_a_half_seconds_for_every_read_together() {
    assert_eq!(LOAD_READS_MS, 1_500);
}

/// boot.ts:27.
#[test]
fn a_read_still_out_at_the_deadline_leaves_the_line_the_provider_has_not_yet_answered() {
    assert_eq!(late_read_line("m-a"), "the provider has not yet answered for m-a; it is read in once it does");
}

/// runtime-golden-import.test.ts:515: a sweep whose last listing is older than a pass that admitted a row neither
/// drops nor kills that row. The first pass lists the builders and is held; a row appears; a second pass lists it and
/// admits it; the first then ends having listed none, and applies nothing: the row it never saw stays held, and the
/// provider is read for it once.
#[test]
fn an_older_pass_neither_drops_nor_admits_after_a_newer_one_listed() {
    common::run_local(async {
        let log = common::Log::default();
        let inner = MemStore::seeded(&json!({}), log, StoreLogs::default());
        let (open, gate) = futures::channel::oneshot::channel::<()>();
        let listed = Rc::new(Cell::new(false));
        let store: Rc<dyn wsp_core::types::pending::Store> = Rc::new(GatedStore { inner: inner.clone(), gate: RefCell::new(Some(gate)), listed: listed.clone() });
        let w = World::load(Opts { store_override: Some(store), ..Opts::default() });

        let first = {
            let area = w.area.clone();
            tokio::task::spawn_local(async move { area.refresh_builders().await })
        };
        common::flush().await;
        assert!(listed.get(), "the first pass listed the builders and waits");

        inner.rows.borrow_mut().entry("builders".to_string()).or_default().insert("b1".into(), builder("b1"));
        w.area.refresh_builders().await.expect("the newer pass");
        assert_eq!(w.rt.builders.borrow().keys().cloned().collect::<Vec<_>>(), vec!["b1".to_string()], "the newer pass admitted the row");

        let _ = open.send(());
        first.await.unwrap().expect("the older pass");
        common::flush().await;
        assert_eq!(w.rt.builders.borrow().keys().cloned().collect::<Vec<_>>(), vec!["b1".to_string()], "the older pass dropped nothing it had not listed");
        let gets: Vec<Value> = w.log.take().into_iter().filter(|e| e[0] == "backend.get").collect();
        assert_eq!(gets, vec![json!(["backend.get", "solari", "b1"])], "the provider was read for the row once");
        assert!(inner.rows.borrow()["builders"].contains_key("b1"), "the row stays in the store");
    });
}

/// boot.ts:111: a second caller of one stored id gets the read that is out, and the key is free once it settles.
#[test]
fn an_admit_of_an_id_that_settled_reads_the_provider_again() {
    common::run_local(async {
        let w = World::load(Opts { store: json!({"builders": {"slow": builder("slow")}}), ..Opts::default() });
        w.area.refresh_builders().await.expect("first pass");
        w.rt.builders.borrow_mut().shift_remove("slow");
        w.area.refresh_builders().await.expect("second pass");
        let gets = w.log.take().into_iter().filter(|e| e[0] == "backend.get").count();
        assert_eq!(gets, 2, "each pass that finds no entry reads the provider once");
        assert!(w.rt.builders.borrow().contains_key("slow"));
    });
}

/// runtime-gone.test.ts:183: a read that lands past the load's deadline and fails holds the record on what the
/// provider said, as a read that fails in time does, and a sighting that fails after it leaves the record held.
#[test]
fn a_late_read_that_fails_holds_the_record_on_the_providers_words() {
    common::run_local(async {
        let w = load_world(Opts { store: json!({"workspaces": {"a": ws("a", json!({}))}}), sights: vec![Sight::Fails("provider: key refused (401)".into())], ..Opts::default() });
        let landed = w.area.load_late("a", asked_err("provider: key refused (401)")).await;
        assert!(landed.is_none(), "a held record answers no entry");
        common::flush().await;
        let rec = ws("a", json!({}));
        common::assert_log(
            &json!({"name": "late read fails", "log": [["imageOf", "img"], ["attach", rec, "m-a", "sandbox"], ["sightMachine", "m-a"]]}),
            &w.log.take(),
        );
        let entry = w.rt.live.borrow().get("a").cloned().expect("the record is live, held");
        let machine = entry.borrow().machine.clone();
        let said = machine.state().await.expect_err("the stand-in refuses");
        assert_eq!(said.to_string(), "provider: key refused (401)");
        assert!(w.area.is_held_away("a"));
        assert_eq!(support::to_json(&entry.borrow().record.phase), json!("running"), "the record keeps the phase it was left at");
        assert!(w.rt.rereading.borrow().is_empty(), "the record is no longer marked being read");
    });
}

/// runtime-gone.test.ts:190: a late read the provider answers gone for settles the record gone with the provider's
/// words, as a load that met the 404 in time does: `(404 gone)` is the word the provider said.
#[test]
fn a_late_gone_answer_settles_gone_with_the_loads_words() {
    common::run_local(async {
        let w = load_world(Opts { store: json!({"workspaces": {"a": ws("a", json!({}))}}), sights: vec![Sight::Gone], ..Opts::default() });
        let landed = w.area.load_late("a", asked_missing("404 gone")).await;
        assert!(landed.is_none());
        common::flush().await;
        let rec = ws("a", json!({}));
        common::assert_log(
            &json!({"name": "late read gone", "log": [
                ["imageOf", "img"],
                ["attach", rec.clone(), "m-a", "sandbox"],
                ["sightMachine", "m-a"],
                ["attach", rec, "m-a", "sandbox"],
                ["markGone", "a", "m-a", "machine m-a is gone at the provider: the record load found it gone at 2026-09-21T14:13:20Z (404 gone)"]
            ]}),
            &w.log.take(),
        );
    });
}

/// boot.ts:200: a record already being read again is not read twice.
#[test]
fn a_record_being_reread_is_not_read_again() {
    common::run_local(async {
        let w = load_world(Opts { sights: vec![Sight::Running], ..Opts::default() });
        support::held_for_test(&w, "503 busy").await;
        w.rt.rereading.borrow_mut().insert("a".into());
        w.log.clear();
        w.area.reread_held("a", GoneSeenBy::Sweep).await.expect("reread");
        assert!(w.log.take().is_empty(), "nothing is asked of the provider");
        assert!(w.area.is_held_away("a"));
    });
}

/// boot.ts:190: an id with no entry reads as held away; a queued spawn's stand-in is none such; a stand-in on any
/// other phase is.
#[test]
fn held_away_is_no_entry_or_an_absent_stand_in_that_is_not_a_queued_spawn() {
    common::run_local(async {
        let w = load_world(Opts::default());
        assert!(w.area.is_held_away("none"));
        let away = |id: &str, phase: &str| {
            let machine = absent_machine(&format!("m-{id}"), MachineKind::Sandbox, Rc::new(|| HostError::new("away")));
            let entry = live_workspace(workspace_record(&ws(id, json!({"phase": phase}))), machine);
            w.rt.live.borrow_mut().insert(id.to_string(), entry);
        };
        away("q", "queued");
        away("r", "running");
        w.rt.add_plain_live("p", &ws("p", json!({})));
        assert!(!w.area.is_held_away("q"), "a queued spawn has no machine yet");
        assert!(w.area.is_held_away("r"));
        assert!(!w.area.is_held_away("p"), "a record on a machine that answered is not held away");
    });
}

/// boot.ts:44: first life seals anywhere; any other life seals only where the backend's copy of a disk is the disk as
/// it stands; a place with no backend seals nothing.
#[test]
fn a_builder_seals_on_first_life_or_where_the_backend_seals_any_life() {
    common::run_local(async {
        let w = load_world(Opts { no_backend_at: vec!["away"], ..Opts::default() });
        assert!(w.area.sealable_at(None, true));
        assert!(w.area.sealable_at(Some("away"), true), "first life is true before any backend is asked");
        assert!(!w.area.sealable_at(None, false));
        assert!(!w.area.sealable_at(Some("away"), false), "no backend reads false");
        let any = load_world(Opts { snapshots_any_life: true, no_backend_at: vec!["away"], ..Opts::default() });
        assert!(any.area.sealable_at(None, false));
        assert!(any.area.sealable_at(Some("solari"), false));
        assert!(!any.area.sealable_at(Some("away"), false));
    });
}

/// boot.ts:49 against the fixture's eleven stored builders, one per road: the life each reads as given the labels the
/// provider answered and whether it may be sealed, which `boot_builders.json` records.
#[test]
fn life_of_reads_each_of_the_eleven_roads_a_stored_builder_can_take() {
    common::run_local(async {
        let fx = common::fixture("boot_builders");
        let scene = common::scene(&fx, "admit: every life a stored builder can read as");
        let w = load_world(Opts::default());
        let stored = scene["store"]["builders"].as_object().expect("stored builders");
        assert_eq!(stored.len(), 11);
        for (id, row) in stored {
            let want = scene["builders"].as_array().unwrap().iter().find(|b| b["id"] == *id).expect("the builder the oracle left");
            let row = common::unscrub_pids(row);
            let stored: StoredBuilder = serde_json::from_value(row).expect("a stored builder");
            let labels: Option<IndexMap<String, String>> = scene["answers"][id.as_str()]["labels"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect());
            let sealable = want["sealable"].as_bool().unwrap();
            let life = w.area.life_of(&stored, labels.as_ref(), sealable);
            let word = match life {
                BuilderLife::Own => "own",
                BuilderLife::Reusable => "reusable",
                BuilderLife::Stale => "stale",
                BuilderLife::Foreign => "foreign",
                BuilderLife::Held => "held",
            };
            assert_eq!(json!(word), want["life"], "builder {id}");
        }
    });
}
