// SPDX-License-Identifier: AGPL-3.0-only
//! builders_life: spec/builders_life.md. Every value is read off images/builders.ts:80-189, 283-472 at 65f6b89c7.
mod images_support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::json;

use images_support::*;
use wsp_core::types::internal::{BuilderLife, HeldBy, Sealed};
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{MachineBackend, MachineSpec};
use wsp_engine::orphans::ReapReason;
use wsp_machines::images::builders::life::{CreateStop, ImportMark};
use wsp_machines::images::builders::BuildersArea;
use wsp_machines::images::engine::{GoneReading, ImageVault, SealResult};
use wsp_machines::images::{BuilderRef, ImageError};
use wsp_wire::views::golden_image::GoldenStage;
use wsp_words::exit::HostError;

fn area(w: &Rc<World>) -> Rc<BuildersArea> {
    BuildersArea::new(w.clone())
}

fn sealed_at(b: &BuilderRef, at: &str, version: u32) {
    b.borrow_mut().record.sealed = Some(Sealed { at: at.into(), version });
}

fn spec() -> MachineSpec {
    serde_json::from_value(json!({"kind": "sandbox"})).expect("a spec")
}

/// Runs every task the area spawned, in order, until none is left.
fn drain(w: &Rc<World>) {
    loop {
        let next: Vec<_> = w.spawned.borrow_mut().drain(..).collect();
        if next.is_empty() {
            break;
        }
        for f in next {
            run(f);
        }
    }
}

// L1: builders.ts:82-89.
#[test]
fn arm_grace_schedules_the_rest_of_the_window_and_replaces_an_earlier_timer() {
    let w = World::new();
    let a = area(&w);
    a.arm_grace("b1", "2026-10-25T08:56:00.000Z");
    assert_eq!(w.clock.asked.borrow().clone(), vec![360_000]);
    a.arm_grace("b1", "2026-10-25T08:58:00.000Z");
    assert_eq!(w.clock.asked.borrow().clone(), vec![360_000, 480_000]);
    assert_eq!(w.clock.armed(), 1, "the first timer is cancelled");
    assert_eq!(a.grace_timers.borrow().keys().cloned().collect::<Vec<_>>(), vec!["b1"]);
    a.arm_grace("b2", "2026-10-25T08:40:00.000Z");
    assert_eq!(w.clock.asked.borrow().last(), Some(&0), "a window already over arms at 0");
    a.arm_grace("b3", "not a time");
    assert_eq!(w.clock.asked.borrow().last(), Some(&0), "an unparsable time arms at 0");
}

// L1: the timer drops itself and runs the sweep.
#[test]
fn a_grace_timer_that_fires_runs_the_sweep_and_drops_itself() {
    let w = World::new();
    let a = area(&w);
    a.arm_grace("b1", "2026-10-25T08:56:00.000Z");
    w.clock.advance(360_000);
    assert!(a.grace_timers.borrow().is_empty());
    drain(&w);
    assert!(w.log().contains(&"refresh_builders".to_string()), "{:?}", w.log());
}

// L2: builders.ts:91-94.
#[test]
fn in_window_is_open_until_exactly_the_grace() {
    let w = World::new();
    let a = area(&w);
    assert!(a.in_window("2026-10-25T08:50:00.001Z"));
    assert!(!a.in_window("2026-10-25T08:50:00.000Z"));
    assert!(a.in_window("2026-10-25T09:05:00.000Z"));
    assert!(!a.in_window("yesterday"));
}

// L3: builders.ts:99, one pass at a time.
#[test]
fn expire_grace_joins_a_pass_already_running() {
    let w = World::new();
    let a = area(&w);
    let (one, two) = run(async { futures::join!(a.expire_grace(), a.expire_grace()) });
    assert!(one.is_ok() && two.is_ok());
    assert_eq!(w.lines("refresh_builders"), vec!["refresh_builders"]);
    run(a.expire_grace()).expect("a later pass");
    assert_eq!(w.lines("refresh_builders").len(), 2, "a pass after the first ended starts anew");
}

// L4: builders.ts:100-118.
#[test]
fn expire_grace_stops_and_forgets_builders_past_their_window() {
    let w = World::new();
    let old = live_builder("b1", "default", "boat", BuilderLife::Own);
    sealed_at(&old, "2026-10-25T08:40:00.000Z", 2);
    w.add_builder(old);
    w.store.seed("builders", "b1", json!({"id": "b1"}));
    let a = area(&w);
    a.arm_grace("b1", "2026-10-25T08:40:00.000Z");
    let swept = run(a.expire_grace()).expect("the sweep");
    assert_eq!(swept.reaped.len(), 1);
    let r = &swept.reaped[0];
    assert_eq!((r.id.as_str(), r.builder, r.reason.clone(), r.age_ms), ("b1", true, ReapReason::Grace, Some(1_200_000)));
    assert!(swept.failed.is_empty());
    assert!(!w.builders.borrow().contains_key("b1"));
    assert!(w.store.doc("builders", "b1").is_none());
    assert!(a.grace_timers.borrow().is_empty());
    assert_eq!(w.lines("gone_stop"), vec!["gone_stop b1"]);
}

// L4: a stop that fails keeps the record for the next sweep.
#[test]
fn expire_grace_keeps_a_builder_it_could_not_stop() {
    let w = World::new();
    let b = live_builder("b2", "default", "boat", BuilderLife::Reusable);
    sealed_at(&b, "2026-10-25T08:30:00.000Z", 1);
    w.add_builder(b);
    w.gone_stop_fails.borrow_mut().insert("b2".into(), HostError::new("the provider would not answer"));
    let swept = run(area(&w).expire_grace()).expect("the sweep");
    assert!(swept.reaped.is_empty());
    assert_eq!(swept.failed.len(), 1);
    assert_eq!(swept.failed[0].id.as_deref(), Some("b2"));
    assert_eq!(swept.failed[0].message, "could not stop: the provider would not answer; stays recorded, retried next sweep");
    assert!(w.builders.borrow().contains_key("b2"));
}

// L4: only sealed own or reusable builders past their window.
#[test]
fn expire_grace_skips_unsealed_foreign_held_and_in_window_builders() {
    let w = World::new();
    w.add_builder(live_builder("unsealed", "default", "boat", BuilderLife::Own));
    for (id, life) in [("foreign", BuilderLife::Foreign), ("held", BuilderLife::Held), ("stale", BuilderLife::Stale)] {
        let b = live_builder(id, "default", "boat", life);
        sealed_at(&b, "2026-10-25T08:00:00.000Z", 1);
        w.add_builder(b);
    }
    let fresh = live_builder("fresh", "default", "boat", BuilderLife::Own);
    sealed_at(&fresh, "2026-10-25T08:55:00.000Z", 1);
    w.add_builder(fresh);
    let swept = run(area(&w).expire_grace()).expect("the sweep");
    assert!(swept.reaped.is_empty() && swept.failed.is_empty());
    assert!(w.lines("gone_stop").is_empty());
    assert_eq!(w.builders.borrow().len(), 5);
}

// L5: builders.ts:141-147.
#[test]
fn hold_stamps_and_writes_the_record() {
    let w = World::new();
    let b = live_builder("b1", "default", "boat", BuilderLife::Own);
    w.add_builder(b.clone());
    run(area(&w).hold(&b)).expect("hold");
    let held = b.borrow().record.held_by.clone();
    assert_eq!(held, Some(HeldBy { host: "host-1".into(), pid: 4242, heartbeat: "2026-10-25T09:00:00.000Z".into() }));
    assert_eq!(w.store.doc("builders", "b1"), Some(serde_json::to_value(&b.borrow().record).expect("record")));
    assert!(w.beat.borrow().is_some(), "the heartbeat is armed");
    assert_eq!(w.clock.asked.borrow().clone(), vec![300_000]);
}

// L5: a builder forgotten meanwhile is not written back.
#[test]
fn hold_writes_nothing_for_a_forgotten_builder() {
    let w = World::new();
    let gone = live_builder("b1", "default", "boat", BuilderLife::Own);
    w.add_builder(live_builder("b1", "default", "boat", BuilderLife::Own));
    run(area(&w).hold(&gone)).expect("hold");
    assert!(w.store.calls().is_empty());
    assert!(gone.borrow().record.held_by.is_none());
}

// L5: a closed host writes the record as it is and arms nothing.
#[test]
fn hold_on_a_closed_host_writes_without_a_stamp_or_a_beat() {
    let w = World::new();
    w.closed.set(true);
    let b = live_builder("b1", "default", "boat", BuilderLife::Own);
    let old = HeldBy { host: "host-0".into(), pid: 7, heartbeat: "2026-10-25T08:00:00.000Z".into() };
    b.borrow_mut().record.held_by = Some(old.clone());
    w.add_builder(b.clone());
    run(area(&w).hold(&b)).expect("hold");
    assert_eq!(b.borrow().record.held_by, Some(old));
    assert_eq!(w.store.calls(), vec!["put builders/b1"]);
    assert!(w.beat.borrow().is_none());
    assert!(w.clock.asked.borrow().is_empty());
}

// L6: builders.ts:119-137.
#[test]
fn the_heartbeat_holds_every_own_builder_and_rearms() {
    let w = World::new();
    w.add_builder(live_builder("b1", "default", "boat", BuilderLife::Own));
    w.add_builder(live_builder("b2", "default", "boat", BuilderLife::Reusable));
    w.add_builder(live_builder("b3", "work", "boat", BuilderLife::Own));
    let a = area(&w);
    a.arm();
    assert_eq!(w.clock.asked.borrow().clone(), vec![300_000]);
    w.clock.advance(300_000);
    assert!(w.lines("set_ticking").len() == 1, "the beat hands its run to state.ticking");
    drain(&w);
    assert_eq!(w.store.calls(), vec!["put builders/b1", "put builders/b3"]);
    assert!(w.beat.borrow().is_some(), "armed again after holding");
}

// L6: one failed write costs one beat, never the timer.
#[test]
fn a_failed_heartbeat_write_warns_and_the_beat_goes_on() {
    let w = World::new();
    w.add_builder(live_builder("b1", "default", "boat", BuilderLife::Own));
    w.add_builder(live_builder("b3", "work", "boat", BuilderLife::Own));
    w.store.fail_puts.borrow_mut().push("builders".into());
    run(area(&w).tick());
    assert_eq!(
        w.warned.lines(),
        vec!["heartbeat for builder b1 not written: put builders refused", "heartbeat for builder b3 not written: put builders refused"]
    );
    assert!(w.beat.borrow().is_some());
}

// L6.
#[test]
fn arm_does_nothing_while_a_beat_is_armed() {
    let w = World::new();
    *w.beat.borrow_mut() = Some(Rc::new(|| {}));
    area(&w).arm();
    assert!(w.clock.asked.borrow().is_empty());
    *w.beat.borrow_mut() = None;
    w.closed.set(true);
    area(&w).arm();
    assert!(w.clock.asked.borrow().is_empty(), "a closed host arms nothing");
}

// L7: builders.ts:151-155.
#[test]
fn refuse_untouchable_names_foreign_held_and_building() {
    let w = World::new();
    let a = area(&w);
    let mut foreign_machine = FakeMachine::bare("b1");
    foreign_machine.labels = Some([("wsp-owner".to_string(), "owner-9".to_string())].into_iter().collect());
    let foreign = live_builder("b1", "default", "boat", BuilderLife::Foreign);
    foreign.borrow_mut().builder.machine = Arc::new(foreign_machine);
    assert_eq!(a.refuse_untouchable(&foreign).expect_err("foreign").message, "b1 wears another setup's owner label (owner-9); it is never sealed or reached from here");
    let bare = live_builder("b0", "default", "boat", BuilderLife::Foreign);
    assert_eq!(a.refuse_untouchable(&bare).expect_err("foreign").message, "b0 wears another setup's owner label (undefined); it is never sealed or reached from here");
    let held = live_builder("b2", "default", "boat", BuilderLife::Held);
    held.borrow_mut().record.held_by = Some(HeldBy { host: "h".into(), pid: 99, heartbeat: "x".into() });
    assert_eq!(a.refuse_untouchable(&held).expect_err("held").message, "b2 is in use by another wsp process (pid 99); it is never sealed or reached from here");
    let building = live_builder("b3", "default", "boat", BuilderLife::Own);
    building.borrow_mut().record.building = Some(true);
    let e = a.refuse_untouchable(&building).expect_err("building");
    assert_eq!((e.message.as_str(), e.kind), ("b3 is still being prepared; it is never sealed or reached until its stages finish", None));
    assert!(a.refuse_untouchable(&live_builder("b4", "default", "boat", BuilderLife::Own)).is_ok());
}

// L8: builders.ts:157-160.
#[test]
fn forget_builder_drops_the_entry_and_the_record() {
    let w = World::new();
    w.add_builder(live_builder("b1", "default", "boat", BuilderLife::Own));
    w.store.seed("builders", "b1", json!({"id": "b1"}));
    run(area(&w).forget_builder("b1")).expect("forget");
    assert!(w.builders.borrow().is_empty());
    assert_eq!(w.store.calls(), vec!["delete builders/b1"]);
}

// L9: builders.ts:166-172.
#[test]
fn forget_if_gone_forgets_only_on_gone() {
    let w = World::new();
    let b = live_builder("b1", "default", "boat", BuilderLife::Own);
    w.add_builder(b.clone());
    let a = area(&w);
    *w.roads.gone.borrow_mut() = Ok(GoneReading::Standing("running".into()));
    run(a.forget_if_gone(&b, w.runtime_backend.clone())).expect("standing");
    *w.roads.gone.borrow_mut() = Err(missing("could not read"));
    run(a.forget_if_gone(&b, w.runtime_backend.clone())).expect("unread");
    assert!(w.builders.borrow().contains_key("b1"));
    *w.roads.gone.borrow_mut() = Ok(GoneReading::Gone);
    run(a.forget_if_gone(&b, w.runtime_backend.clone())).expect("gone");
    assert!(!w.builders.borrow().contains_key("b1"));
    assert_eq!(w.lines("road read_gone"), vec!["road read_gone b1"; 3]);
}

// L10: builders.ts:177-189.
#[test]
fn stage_of_sets_and_clears_the_stage_at_and_emits_the_frame() {
    let w = World::new();
    let stage = area(&w).stage_of("default", Some("pl_1"), true);
    stage(GoldenStage::Creating, Some("a builder".into()), None, Some(vec![]));
    {
        let at = w.stage_at.borrow();
        let held = at.get("pl_1/default").expect("the stage is held");
        assert_eq!((held.place.as_str(), held.name.as_str(), held.named), ("pl_1", "default", true));
        assert_eq!(serde_json::to_string(&held.frame).expect("frame"), r#"{"name":"default","stage":"creating","detail":"a builder"}"#);
    }
    stage(GoldenStage::Ready, None, None, Some(vec!["x".into()]));
    assert!(w.stage_at.borrow().is_empty());
    let emitted = w.emitted.borrow();
    assert_eq!(serde_json::to_value(&emitted[0]).expect("event"), json!({"name": "default", "stage": "creating", "detail": "a builder", "place": "pl_1"}));
    assert_eq!(serde_json::to_value(&emitted[1]).expect("event"), json!({"name": "default", "stage": "ready", "left": ["x"], "place": "pl_1"}));
}

// L10: an image's own build names no place on its frames, though its stage is held by computer.
#[test]
fn stage_of_names_the_place_only_for_a_copy() {
    let w = World::new();
    let stage = area(&w).stage_of("default", Some("boat"), false);
    stage(GoldenStage::Snapshotting, None, None, None);
    assert!(w.stage_at.borrow().contains_key("boat/default"));
    assert_eq!(serde_json::to_value(&w.emitted.borrow()[0]).expect("event"), json!({"name": "default", "stage": "snapshotting"}));
    let quiet = area(&w).stage_of("work", None, true);
    quiet(GoldenStage::Failed, Some("why".into()), None, None);
    assert_eq!(w.stage_at.borrow().len(), 1, "no computer, no stage held");
    assert_eq!(serde_json::to_value(&w.emitted.borrow()[1]).expect("event"), json!({"name": "work", "stage": "failed", "detail": "why"}));
}

fn stop_of(signal: Option<wsp_engine::Cancel>, began: Rc<RefCell<Vec<bool>>>, b: Arc<FakeBackend>) -> CreateStop {
    CreateStop { signal, began: Rc::new(move |_creating| began.borrow_mut().push(b.created.borrow().is_empty())) }
}

// L11: builders.ts:285-324.
#[test]
fn recording_creates_holds_a_placeholder_before_answering() {
    let w = World::new();
    let b = Arc::new(FakeBackend::with("pl_1", building_caps()));
    let mut m = FakeMachine::bare("m-new");
    m.labels = Some([("createdAt".to_string(), "2026-10-25T08:59:00.000Z".to_string())].into_iter().collect());
    b.next_machine.borrow_mut().push_back(Arc::new(m));
    let made: Rc<RefCell<Vec<BuilderRef>>> = Rc::new(RefCell::new(vec![]));
    let made2 = made.clone();
    let a = area(&w);
    let road = a.recording_creates(b.clone(), "default", None, Rc::new(move |p| made2.borrow_mut().push(p)), None, "pl_1");
    let machine = run(road.create(spec(), None, None)).expect("created");
    assert_eq!(machine.id(), "m-new");
    let entry = w.builders.borrow().get("m-new").cloned().expect("the placeholder is held");
    assert!(Rc::ptr_eq(&entry, &made.borrow()[0]));
    let rec = serde_json::to_value(&entry.borrow().record).expect("record");
    assert_eq!(rec["createdAt"], json!("2026-10-25T08:59:00.000Z"));
    assert_eq!(rec["size"], json!({"cpu": 2, "memMb": 4096}), "the runtime backend's default size");
    assert_eq!((rec["building"].clone(), rec["firstLife"].clone(), rec["baseTemplate"].clone(), rec["setupSha"].clone()), (json!(true), json!(true), json!(""), json!("")));
    assert_eq!(rec["place"], json!("pl_1"));
    assert_eq!(rec["heldBy"]["pid"], json!(4242));
    assert_eq!(w.store.doc("builders", "m-new"), Some(rec));
    assert_eq!((entry.borrow().life, entry.borrow().sealable), (BuilderLife::Own, true));
    assert!(w.log().contains(&"observed m-new".to_string()));
}

// L11: the placeholder carries the import the prepare asked for.
#[test]
#[ignore = "waits on wsp-core's types::pending::ImportLedger carrying the engine ledger's fields (BuilderRecord.import)"]
fn recording_creates_stamps_the_import_mark() {
    let w = World::new();
    let b = FakeBackend::new("pl_1");
    let road = area(&w).recording_creates(b, "default", Some(ImportMark { recipe_hash: "rh".into(), recipe: None }), Rc::new(|_| {}), None, "pl_1");
    run(road.create(spec(), None, None)).expect("created");
    let rec = serde_json::to_value(&w.builders.borrow()["m-created"].borrow().record).expect("record");
    assert_eq!(rec["import"], json!({"recipeHash": "rh", "applied": [], "smoke": "true"}));
}

// L11: a stop before the create asks nothing.
#[test]
fn recording_creates_refuses_after_a_stop_without_asking() {
    let w = World::new();
    let b = FakeBackend::new("boat");
    let signal = wsp_engine::Cancel::new();
    signal.cancel(None);
    let began = Rc::new(RefCell::new(vec![]));
    let road = area(&w).recording_creates(b.clone(), "default", None, Rc::new(|_| {}), Some(stop_of(Some(signal), began.clone(), b.clone())), "boat");
    let e = run(road.create(spec(), None, None)).err().expect("refused");
    let line = wsp_core::types::internal::PrepareStoppedError::new(None, None).message();
    assert_eq!(e, EngineError::CreateCancelled { machine_id: String::new(), why: "stopped".into(), line });
    assert!(b.created.borrow().is_empty());
    assert!(began.borrow().is_empty());
    assert!(w.builders.borrow().is_empty());
}

// L11: the create in flight is handed to the stop before the provider answers.
#[test]
fn recording_creates_hands_the_create_to_the_stop_before_it_lands() {
    let w = World::new();
    let b = FakeBackend::new("boat");
    let began = Rc::new(RefCell::new(vec![]));
    let road = area(&w).recording_creates(b.clone(), "default", None, Rc::new(|_| {}), Some(stop_of(None, began.clone(), b.clone())), "boat");
    run(road.create(spec(), None, None)).expect("created");
    assert_eq!(began.borrow().clone(), vec![true], "began saw no create asked yet");
    assert_eq!(b.created.borrow().len(), 1);
}

// L11: the machine's own createdAt label wins over the spec's.
#[test]
fn recording_creates_takes_the_created_at_label_of_the_machine() {
    let w = World::new();
    let b = FakeBackend::new("boat");
    let mut s = spec();
    s.labels = Some([("createdAt".to_string(), "2026-10-25T08:00:00.000Z".to_string())].into_iter().collect());
    let road = area(&w).recording_creates(b, "default", None, Rc::new(|_| {}), None, "boat");
    run(road.create(s, None, None)).expect("created");
    let rec = serde_json::to_value(&w.builders.borrow()["m-created"].borrow().record).expect("record");
    assert_eq!(rec["createdAt"], json!("2026-10-25T08:00:00.000Z"), "no machine label: the spec's");
}

// L12: builders.ts:327-350.
#[test]
fn settle_builder_writes_the_finished_record_onto_the_placeholder() {
    let w = World::new();
    let placeholder = live_builder("m1", "default", "pl_1", BuilderLife::Reusable);
    placeholder.borrow_mut().record.created_at = "2026-10-25T08:30:00.000Z".into();
    placeholder.borrow_mut().record.building = Some(true);
    let mut built = engine_builder(FakeMachine::new("m1"));
    built.setup_sha = "sha-9".into();
    built.parent_snapshot_id = Some("snap-4".into());
    let entry = run(area(&w).settle_builder("default", built, Some(placeholder.clone()), "pl_1")).expect("settled");
    assert!(Rc::ptr_eq(&entry, &placeholder));
    assert!(Rc::ptr_eq(&entry, &w.builders.borrow()["m1"]));
    let rec = serde_json::to_value(&entry.borrow().record).expect("record");
    assert_eq!(rec["createdAt"], json!("2026-10-25T08:30:00.000Z"));
    assert_eq!(rec["setupSha"], json!("sha-9"));
    assert_eq!(rec["parentSnapshotId"], json!("snap-4"));
    assert!(rec.get("building").is_none(), "{rec}");
    assert_eq!(entry.borrow().life, BuilderLife::Reusable, "the placeholder's life stays");
    assert_eq!(w.store.doc("builders", "m1"), Some(rec));
}

// L12.
#[test]
fn settle_builder_without_a_placeholder_makes_an_own_entry() {
    let w = World::new();
    let entry = run(area(&w).settle_builder("work", engine_builder(FakeMachine::new("m2")), None, "boat")).expect("settled");
    assert_eq!((entry.borrow().life, entry.borrow().sealable), (BuilderLife::Own, true));
    assert_eq!(entry.borrow().record.created_at, "2026-10-25T10:00:00.000Z", "the builder's own createdAt");
    assert_eq!(entry.borrow().record.place.as_deref(), Some("boat"));
    assert!(w.store.doc("builders", "m2").is_some());
}

fn with_import(b: &BuilderRef, hash: &str) {
    b.borrow_mut().builder.import = Some(ledger(hash));
}

fn vaulted(mut r: SealResult) -> SealResult {
    r.vault = Some(ImageVault { tar: vec![1, 2, 3], sha256: "vs".into(), paths: 2, held: vec!["~/.x".into()] });
    r
}

// L13: builders.ts:357-395.
#[test]
fn an_own_seal_writes_the_record_and_its_vault() {
    let w = World::new();
    w.with_door().computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    let entry = live_builder("b1", "default", "pl_1", BuilderLife::Own);
    with_import(&entry, "rh");
    let stamped = run(area(&w).record_seal("pl_1", "default", &entry, &recipe("setup"), vaulted(seal_result(3, "snap-3", false)), None)).expect("recorded");
    assert_eq!(w.store.calls(), vec!["putBlob image-vaults/default@v3", "put images/default"]);
    assert_eq!(w.store.blob("image-vaults", "default@v3"), Some(vec![1, 2, 3]));
    assert_eq!(
        w.store.doc("images", "default"),
        Some(json!({
            "name": "default", "version": 3, "hash": "img:rh:vs", "recipeHash": "rh", "logins": [], "sealedAt": "2026-10-25T09:00:00.000Z",
            "sealedFrom": "host-1", "vault": {"sha256": "vs", "bytes": 3, "paths": 2, "held": ["~/.x"], "takenAt": "2026-10-25T09:00:00.000Z"},
            "place": "pl_1"
        }))
    );
    assert_eq!(stamped.version.image_hash.as_deref(), Some("img:rh:vs"));
    assert_eq!(stamped.version.no_secrets, None);
    assert_eq!(stamped.manifest.versions[0].image_hash.as_deref(), Some("img:rh:vs"));
}

// L13: the replaced version's vault goes after the record is written.
#[test]
fn an_own_seal_drops_the_replaced_versions_vault() {
    let w = World::new();
    w.with_door().computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    w.put_image_now(&sealed_image("default", 2, "h2", Some("pl_1")));
    w.store.seed_blob("image-vaults", "default@v2", &[9]);
    let entry = live_builder("b1", "default", "pl_1", BuilderLife::Own);
    run(area(&w).record_seal("pl_1", "default", &entry, &recipe("setup"), vaulted(seal_result(3, "snap-3", false)), None)).expect("recorded");
    assert_eq!(w.store.calls(), vec!["putBlob image-vaults/default@v3", "put images/default", "deleteBlob image-vaults/default@v2"]);
    assert_eq!(w.store.blob_ids("image-vaults"), vec!["default@v3"]);
    assert_eq!(w.store.doc("images", "default").expect("record")["hash"], json!("img::vs"), "no builder import: an empty recipe hash");
}

// L13: a copy's seal moves nothing of the record.
#[test]
fn a_copys_seal_writes_no_record_and_stamps_the_copys_hash() {
    let w = World::new();
    w.with_door().computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    let entry = live_builder("b1", "default", "pl_1", BuilderLife::Own);
    let copy = sealed_image("default", 2, "h-record", Some("boat"));
    let stamped = run(area(&w).record_seal("pl_1", "default", &entry, &recipe("setup"), vaulted(seal_result(5, "snap-5", false)), Some(&copy))).expect("recorded");
    assert!(w.store.calls().is_empty());
    assert_eq!(stamped.version.image_hash.as_deref(), Some("h-record"));
    assert_eq!(stamped.version.version, 5.into());
}

// L13: a seal at a provider marks the record and the version.
#[test]
fn a_seal_at_a_provider_marks_no_secrets() {
    let w = World::new();
    let entry = live_builder("b1", "default", "boat", BuilderLife::Own);
    with_import(&entry, "rh");
    let stamped = run(area(&w).record_seal("boat", "default", &entry, &recipe("setup"), seal_result(1, "snap-1", false), None)).expect("recorded");
    let rec = w.store.doc("images", "default").expect("record");
    assert_eq!((rec["noSecrets"].clone(), rec["place"].clone(), rec["hash"].clone()), (json!(true), json!("boat"), json!("img:rh:-")));
    assert!(rec.get("vault").is_none());
    assert_eq!(stamped.version.no_secrets, Some(true));
    assert_eq!(w.store.calls(), vec!["put images/default"]);
}

// L14: builders.ts:402.
#[test]
fn seal_entry_refuses_without_a_recipe_before_reading() {
    let w = World::new();
    let entry = live_builder("b1", "default", "boat", BuilderLife::Own);
    w.add_builder(entry.clone());
    let e = run(area(&w).seal_entry(&entry, true, None, None)).err().expect("refused");
    assert_eq!(e.message(), "this runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run");
    assert!(w.lines("claim").is_empty() && w.lines("road").is_empty());
}

// L14: the seal's options, as builders.ts:414-433 hands them.
#[test]
fn seal_entry_asks_the_seal_with_the_ts_options() {
    let w = World::new();
    w.with_door().computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    let prior = manifest(&[(2, "snap-2", None)]);
    w.put_copy_now("pl_1", "work", &prior);
    let entry = live_builder("b1", "work", "pl_1", BuilderLife::Own);
    let mut r = recipe("setup");
    r.labels = Some([("team".to_string(), "x".to_string())].into_iter().collect());
    r.vault_paths = Some(vec!["~/.claude".into()]);
    entry.borrow_mut().recipe = Some(r);
    w.add_builder(entry.clone());
    w.roads.sealed.borrow_mut().push_back(Ok(seal_result(3, "snap-3", false)));
    run(area(&w).seal_entry(&entry, true, Some(vec![]), None)).expect("sealed");
    let ask = w.roads.seal_asks.borrow()[0].clone();
    assert_eq!(ask.smoke, "true");
    assert_eq!(ask.labels_keys(), vec!["team", "wsp", "wsp-smoke", "wsp-owner", "createdAt"]);
    assert_eq!(ask.size.labels.as_ref().expect("labels")["wsp-owner"], "owner-1");
    assert_eq!(ask.size.labels.as_ref().expect("labels")["createdAt"], "2026-10-25T09:00:00.000Z");
    assert_eq!(ask.manifest, Some(prior));
    assert_eq!(ask.vault_paths, Some(vec!["~/.claude".to_string()]));
    assert_eq!(ask.refuse, None);
    assert!(ask.keep_builder);
    assert_eq!(ask.logins, Some(vec![]));
    assert_eq!((ask.name.as_str(), ask.host_id.as_str()), ("work", "mark-1"));
    assert_eq!(w.lines("claim"), vec!["claim smoke/b1 @pl_1"]);
    assert!(w.log().contains(&"observing".to_string()));
}

/// The keys of a seal's labels, in order.
trait LabelKeys {
    fn labels_keys(&self) -> Vec<String>;
}

impl LabelKeys for wsp_machines::images::engine::SealGoldenAsk {
    fn labels_keys(&self) -> Vec<String> {
        self.size.labels.as_ref().map(|l| l.keys().cloned().collect()).unwrap_or_default()
    }
}

// L14: at a provider the seal refuses the sign-in paths, gh's kept while the broker is not ready, and takes no vault.
#[test]
fn seal_entry_at_a_provider_refuses_paths_and_takes_no_vault() {
    let w = World::new();
    let entry = live_builder("b1", "default", "boat", BuilderLife::Own);
    let mut r = recipe("setup");
    r.vault_paths = Some(vec!["~/.claude".into()]);
    entry.borrow_mut().recipe = Some(r);
    w.add_builder(entry.clone());
    w.roads.sealed.borrow_mut().push_back(Ok(seal_result(1, "snap-1", false)));
    run(area(&w).seal_entry(&entry, false, None, None)).expect("sealed");
    let ask = w.roads.seal_asks.borrow()[0].clone();
    assert_eq!(ask.vault_paths, None);
    let gh: Vec<String> = wsp_catalog::signin_rows::GH_LOGIN_PATHS.clone();
    let expected: Vec<String> = wsp_catalog::signin_rows::NEVER_ON_BROKERED.iter().filter(|p| !gh.contains(p)).cloned().collect();
    assert_eq!(ask.refuse, Some(expected));
}

fn sealing(w: &Rc<World>, place: &str, name: &str, kept: bool) -> BuilderRef {
    let entry = live_builder("b1", name, place, BuilderLife::Reusable);
    entry.borrow_mut().recipe = Some(recipe("setup"));
    w.add_builder(entry.clone());
    w.roads.sealed.borrow_mut().push_back(Ok(seal_result(3, "snap-3", kept)));
    entry
}

// L15: builders.ts:443-450.
#[test]
fn a_kept_seal_stamps_sealed_holds_and_arms_grace() {
    let w = World::new();
    let entry = sealing(&w, "boat", "work", true);
    let a = area(&w);
    let stamped = run(a.seal_entry(&entry, true, None, None)).expect("sealed");
    assert_eq!(stamped.version.version, 3.into());
    let b = entry.borrow();
    assert_eq!(b.record.sealed, Some(Sealed { at: "2026-10-25T09:00:00.000Z".into(), version: 3 }));
    assert_eq!(b.record.parent_snapshot_id.as_deref(), Some("snap-3"));
    assert_eq!(b.builder.parent_snapshot_id.as_deref(), Some("snap-3"));
    assert_eq!(b.life, BuilderLife::Own);
    assert!(w.store.doc("builders", "b1").is_some());
    assert!(a.grace_timers.borrow().contains_key("b1"));
    assert_eq!(w.lines("put_copy"), vec!["put_copy boat work"]);
}

// L15.
#[test]
fn an_unkept_seal_forgets_the_builder() {
    let w = World::new();
    let entry = sealing(&w, "boat", "work", false);
    w.store.seed("builders", "b1", json!({"id": "b1"}));
    run(area(&w).seal_entry(&entry, false, None, None)).expect("sealed");
    assert!(!w.builders.borrow().contains_key("b1"));
    assert!(w.store.doc("builders", "b1").is_none());
    assert_eq!(w.copy_now("boat", "work").map(|m| m.versions.len()), Some(1));
}

// L15: the image's own seal of `default` marks the default place; a failed mark only warns.
#[test]
fn a_default_seal_marks_the_default_place_and_a_failed_mark_only_warns() {
    let w = World::new();
    let door = w.with_door();
    *door.mark_fails.borrow_mut() = Some(HostError::new("the record was busy"));
    let entry = sealing(&w, "boat", "default", false);
    run(area(&w).seal_entry(&entry, false, None, None)).expect("sealed");
    assert_eq!(w.lines("door mark_default_if_none"), vec!["door mark_default_if_none boat"]);
    assert_eq!(w.warned.lines(), vec!["the default place was not marked at the seal: the record was busy"]);
    let copy = sealing(&w, "boat", "default", false);
    run(area(&w).seal_entry(&copy, false, None, Some(sealed_image("default", 1, "h", Some("boat"))))).expect("a copy's seal");
    assert_eq!(w.lines("door mark_default_if_none").len(), 1, "a copy's seal marks nothing");
}

// L16: builders.ts:457.
#[test]
fn a_refused_snapshot_keeps_the_builder_record() {
    let w = World::new();
    let entry = live_builder("b1", "default", "boat", BuilderLife::Own);
    entry.borrow_mut().recipe = Some(recipe("setup"));
    w.add_builder(entry.clone());
    w.roads.sealed.borrow_mut().push_back(Err(ImageError::SnapshotFailed { message: "the provider refused the snapshot".into(), builder_state: "running".into() }));
    let e = run(area(&w).seal_entry(&entry, false, None, None)).err().expect("refused");
    assert!(matches!(e, ImageError::SnapshotFailed { .. }));
    assert!(w.builders.borrow().contains_key("b1"));
    assert!(w.lines("road read_gone").is_empty() && w.lines("road kill_until_gone").is_empty());
}

// L16: a builder that is not first-life is killed, then forgotten where gone.
#[test]
fn a_not_first_life_builder_is_killed_then_forgotten_if_gone() {
    let w = World::new();
    let entry = live_builder("b1", "default", "boat", BuilderLife::Own);
    entry.borrow_mut().recipe = Some(recipe("setup"));
    w.add_builder(entry.clone());
    w.roads.sealed.borrow_mut().push_back(Err(ImageError::Engine(EngineError::NotFirstLife { machine_id: "b1".into(), action: "seal".into() })));
    let e = run(area(&w).seal_entry(&entry, false, None, None)).err().expect("refused");
    assert!(matches!(e, ImageError::Engine(EngineError::NotFirstLife { .. })));
    assert_eq!(w.lines("road "), vec!["road seal_golden b1 keep=false", "road kill_until_gone b1", "road read_gone b1"]);
    assert!(!w.builders.borrow().contains_key("b1"));
}

// L16: a failure after the seal still asks whether the machine is gone.
#[test]
fn a_failure_after_the_seal_still_asks_gone() {
    let w = World::new();
    let entry = sealing(&w, "boat", "work", false);
    w.store.fail_puts.borrow_mut().push("goldens".into());
    *w.roads.gone.borrow_mut() = Ok(GoneReading::Standing("running".into()));
    let e = run(area(&w).seal_entry(&entry, false, None, None)).err().expect("failed");
    assert_eq!(e.message(), "put goldens refused");
    assert_eq!(w.lines("road read_gone"), vec!["road read_gone b1"]);
    assert!(w.builders.borrow().contains_key("b1"), "a machine still standing keeps its record");
}

// L17: builders.ts:469-472.
#[test]
fn drop_image_drops_the_template_before_the_snapshot() {
    let w = World::new();
    let pl = FakeBackend::new("pl_1");
    run(area(&w).drop_image(&version(2, "snap-2", Some("t-2")), Some(pl.clone() as Arc<dyn MachineBackend>))).expect("dropped");
    assert_eq!(w.lines("road drop_template"), vec!["road drop_template t-2"]);
    assert_eq!(pl.deleted.borrow().clone(), vec!["snap-2"]);
    run(area(&w).drop_image(&version(1, "snap-1", None), None)).expect("dropped");
    assert_eq!(w.runtime_backend.deleted.borrow().clone(), vec!["snap-1"], "no backend named: the runtime's");
    *w.roads.drop_template_fails.borrow_mut() = Some(missing("template t-9 refused"));
    let e = run(area(&w).drop_image(&version(9, "snap-9", Some("t-9")), None)).err().expect("refused");
    assert_eq!(e.message(), "template t-9 refused");
    assert_eq!(w.runtime_backend.deleted.borrow().len(), 1, "no snapshot deleted after a failed template drop");
}

