// SPDX-License-Identifier: AGPL-3.0-only
//! golden_prepare: spec/golden_prepare.md. Every value is read off images/golden.ts:64-290 at 65f6b89c7.
mod images_support;

use std::rc::Rc;

use futures::channel::oneshot;
use futures::FutureExt;
use serde_json::json;

use images_support::*;
use wsp_core::types::api::PrepareAsk;
use wsp_core::types::internal::BuilderLife;
use wsp_core::types::pending::BuildGoldenOptions;
use wsp_core::types::wiring::GoldenBuildRequest;
use wsp_engine::golden_seam::GoldenImport;
use wsp_engine::Cancel;
use wsp_machines::images::engine::BuiltGolden;
use wsp_machines::images::golden::{GoldenArea, Preparing};
use wsp_machines::images::ImageError;
use wsp_wire::views::golden_image::ALREADY_APPLIED;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_words::exit::{HostError, Kind};

/// Re-raises the panic of a spawned task that already ended, so a red run names the unit's own `todo!()` rather than
/// a later assert of the test.
macro_rules! still_running {
    ($t:expr) => {
        if $t.is_finished() {
            match (&mut $t).await {
                Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
                _ => panic!("the task ended before the test acted"),
            }
        }
    };
}

fn area(w: &Rc<World>) -> Rc<GoldenArea> {
    GoldenArea::new(w.clone())
}

/// A recipe that carries an import (hash `h1` through `import_mark`).
fn importing() -> wsp_core::types::wiring::GoldenRecipe {
    let mut r = recipe("setup");
    r.import = Some(GoldenImport::default());
    r
}

fn ask(r: Option<wsp_core::types::wiring::GoldenRecipe>) -> PrepareAsk {
    PrepareAsk { recipe: r, ..Default::default() }
}

fn reusable(w: &Rc<World>, id: &str, hash: &str) -> wsp_machines::images::BuilderRef {
    let b = live_builder(id, "default", "boat", BuilderLife::Reusable);
    b.borrow_mut().builder.import = Some(ledger(hash));
    w.add_builder(b.clone());
    b
}

/// Lets the tasks spawned on this thread run until they wait on something.
async fn settle() {
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
}

// G1: golden.ts:80-97.
#[test]
fn build_claims_the_golden_key_and_files_the_wired_copy() {
    let w = World::new();
    let built = manifest(&[(1, "snap-1", None)]);
    *w.roads.built.borrow_mut() = Some(Ok(BuiltGolden { manifest: built.clone(), version: version(1, "snap-1", None) }));
    let sealed = run(area(&w).build(GoldenBuildRequest { options: BuildGoldenOptions::default(), name: None })).expect("built");
    assert_eq!(sealed.manifest, built);
    assert_eq!(sealed.version, version(1, "snap-1", None));
    assert_eq!(w.copy_now("boat", "default"), Some(built));
    assert_eq!(w.log()[0], "ready");
    assert_eq!(w.lines("claim"), vec!["claim golden/default @-"]);
    assert_eq!(w.lines("road build_golden"), vec!["road build_golden default"]);
}

// G1: the build carries the prior manifest and the owner's labels.
#[test]
fn build_hands_the_prior_manifest_and_the_owner_labels() {
    let w = World::new();
    let prior = manifest(&[(1, "snap-1", None)]);
    w.put_copy_now("boat", "work", &prior);
    *w.roads.built.borrow_mut() = Some(Ok(BuiltGolden { manifest: manifest(&[(1, "snap-1", None), (2, "snap-2", None)]), version: version(2, "snap-2", None) }));
    run(area(&w).build(GoldenBuildRequest { options: BuildGoldenOptions::default(), name: Some("work".into()) })).expect("built");
    let ask = w.roads.build_asks.borrow()[0].clone();
    assert_eq!(ask.manifest, Some(prior));
    assert_eq!((ask.name.as_str(), ask.host_id.as_str()), ("work", "mark-1"));
    let labels: Vec<(String, String)> = ask.labels.into_iter().collect();
    assert_eq!(labels, vec![("wsp".into(), "1".into()), ("wsp-owner".into(), "owner-1".into()), ("createdAt".into(), "2026-10-25T09:00:00.000Z".into())]);
}

// G2: golden.ts:98-102.
#[test]
fn get_reads_the_copy_at_the_image_place() {
    let w = World::new();
    w.put_image_now(&sealed_image("default", 2, "h", Some("pl_1")));
    w.put_copy_now("pl_1", "default", &manifest(&[(2, "snap-at-pl1", None)]));
    w.put_copy_now("boat", "default", &manifest(&[(1, "snap-at-boat", None)]));
    assert_eq!(run(area(&w).get(None)).expect("get"), Some(manifest(&[(2, "snap-at-pl1", None)])));
    assert_eq!(run(area(&w).get(Some("other"))).expect("get"), None);
}

// G3: golden.ts:111-116.
#[test]
fn build_place_goes_to_the_records_place_whatever_was_asked() {
    let w = World::new();
    w.with_door().computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    w.put_image_now(&sealed_image("default", 1, "h", Some("pl_1")));
    let place = run(area(&w).build_place(Some("boat"))).expect("place");
    assert_eq!((place.place.as_str(), place.name.as_str(), place.kind), ("pl_1", "spoo", PlaceKind::Computer));
    assert_eq!(w.lines("place_at"), vec!["place_at pl_1"]);
}

// G3: a target that forks nothing is refused, with no fallback.
#[test]
fn build_place_refuses_a_target_that_forks_nothing() {
    let w = World::new();
    *w.runtime_backend.caps.borrow_mut() = forkless_caps();
    let e = run(area(&w).build_place(Some("boat"))).err().expect("refused").into_host();
    assert_eq!(e.message, "boat forks no machines, so your image cannot be built there");
    assert_eq!(e.kind, Some(Kind::Conflict));
    assert!(w.lines("build_places").is_empty());
}

// G3: the marked place.
#[test]
fn build_place_takes_the_marked_place() {
    let w = World::new();
    let door = w.with_door();
    door.computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    *door.marked.borrow_mut() = Ok(Some("pl_1".into()));
    let place = run(area(&w).build_place(None)).expect("place");
    assert_eq!((place.place.as_str(), place.name.as_str()), ("pl_1", "spoo"));
    assert_eq!(w.lines("door "), vec!["door default_place", "door forking_backend pl_1"]);
}

// G3: only a place that forks nothing is passed over.
#[test]
fn build_place_falls_through_a_wired_provider_that_forks_nothing() {
    let w = World::new();
    *w.runtime_backend.caps.borrow_mut() = forkless_caps();
    w.with_door().computer("pl_2", "hetzner", Some(FakeBackend::new("pl_2")));
    let place = run(area(&w).build_place(None)).expect("place");
    assert_eq!((place.place.as_str(), place.name.as_str(), place.kind), ("pl_2", "hetzner", PlaceKind::Computer));
    assert_eq!(w.lines("build_places"), vec!["build_places"]);
}

// G3: a marked place's own failure is the person's to read.
#[test]
fn build_place_keeps_a_marked_places_own_failure() {
    let w = World::new();
    let door = w.with_door();
    door.computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    *door.marked.borrow_mut() = Ok(Some("pl_1".into()));
    door.forking_fails.borrow_mut().insert("pl_1".into(), HostError::new("spoo is not connected"));
    let e = run(area(&w).build_place(None)).err().expect("failed").into_host();
    assert_eq!(e.message, "spoo is not connected");
    assert!(w.lines("build_places").is_empty());
}

// G3: none, and more than one, are each their own refusal.
#[test]
fn build_place_refuses_none_and_asks_among_many() {
    let w = World::new();
    *w.runtime_backend.caps.borrow_mut() = forkless_caps();
    let e = run(area(&w).build_place(None)).err().expect("none").into_host();
    assert_eq!((e.message.as_str(), e.kind), (wsp_words::words::thread::NO_BUILD_PLACE_LINE, Some(Kind::Conflict)));
    w.places.rows.borrow_mut().insert("solari".into(), FakeBackend::new("solari"));
    w.places.rows.borrow_mut().insert("e2b".into(), FakeBackend::new("e2b"));
    let e = run(area(&w).build_place(None)).err().expect("many").into_host();
    let said: Vec<String> = ["solari", "e2b"]
        .iter()
        .map(|id| wsp_words::place_word::place_said(&wsp_words::place_word::PlaceRef { id: id.to_string(), name: id.to_string(), kind: Some("provider".into()) }))
        .collect();
    assert_eq!(e.message, wsp_words::words::thread::build_place_ask_line(&said));
    assert_eq!(e.kind, Some(Kind::Conflict));
}

// G4: golden.ts:133-137, 75-78.
#[test]
fn prepare_at_names_the_records_place_for_an_image_with_a_record() {
    let w = World::new();
    w.with_door().computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    w.put_image_now(&sealed_image("default", 1, "h", Some("pl_1")));
    let at = run(area(&w).prepare_at(None, Some("boat"))).expect("at");
    assert_eq!((at.place.as_str(), at.name.as_str()), ("pl_1", "spoo"));
}

// G4.
#[test]
fn prepare_at_takes_the_word_or_the_wired_provider() {
    let w = World::new();
    w.with_door().computer("pl_2", "hetzner", Some(FakeBackend::new("pl_2")));
    let at = run(area(&w).prepare_at(Some("work"), Some("pl_2"))).expect("at");
    assert_eq!((at.place.as_str(), at.name.as_str()), ("pl_2", "hetzner"));
    let wired = run(area(&w).prepare_at(Some("work"), None)).expect("at");
    assert_eq!((wired.place.as_str(), wired.name.as_str()), ("boat", "boat"));
    assert_eq!(w.lines("place_at"), vec!["place_at pl_2"], "the wired provider is read through backend_at, not place_at");
}

// G5: golden.ts:141 before anything else.
#[test]
fn prepare_refuses_without_a_recipe_before_any_place_is_read() {
    let w = World::new();
    let e = run(area(&w).prepare(ask(None))).err().expect("refused");
    assert_eq!(e.message(), "this runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run");
    assert_eq!(w.log(), vec!["ready"]);
}

fn in_flight(w: &Rc<World>, key: &str, hash: Option<&str>, id: &str) {
    let view = view_of(&live_builder(id, "default", "boat", BuilderLife::Own));
    let run = async move { Ok(view) }.boxed_local().shared();
    w.preparing.borrow_mut().insert(key.into(), Preparing { hash: hash.map(String::from), run });
}

// G5: a prepare in flight with the same hash is joined.
#[test]
fn prepare_joins_one_in_flight_with_the_same_hash() {
    let w = World::new();
    in_flight(&w, "boat/default", Some("h1"), "b-joined");
    let view = run(area(&w).prepare(ask(Some(importing())))).expect("joined");
    assert_eq!(view.id, "b-joined");
    assert!(w.lines("claim").is_empty() && w.lines("road").is_empty());
    let bare = World::new();
    in_flight(&bare, "boat/default", None, "b-bare");
    assert_eq!(run(area(&bare).prepare(ask(Some(recipe("other setup"))))).expect("joined").id, "b-bare", "two prepares with no import share one");
}

// G5: another hash is refused.
#[test]
fn prepare_refuses_one_in_flight_with_another_hash() {
    let w = World::new();
    in_flight(&w, "boat/default", Some("h0"), "b-other");
    let e = run(area(&w).prepare(ask(Some(importing())))).err().expect("refused").into_host();
    assert_eq!(e.message, "a builder named default is still being prepared for a different recipe; wait for it to finish, then run again");
    assert_eq!(e.kind, None);
}

// G6: golden.ts:149, 156, 268.
#[test]
fn prepare_claims_the_builder_key_per_place() {
    let w = World::new();
    w.with_door().computer("pl_1", "spoo", Some(FakeBackend::new("pl_1")));
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m1"))));
    let view = run(area(&w).prepare(PrepareAsk { recipe: Some(recipe("setup")), place: Some("pl_1".into()), copy: Some(true), ..Default::default() })).expect("prepared");
    assert_eq!(view.id, "m1");
    assert_eq!(w.lines("claim"), vec!["claim builder/pl_1/default @pl_1"]);
    assert_eq!(w.lines("stage_of"), vec!["stage_of default pl_1 true"]);
}

// G6, G7: a reusable builder with the same import is attached to.
#[test]
fn prepare_attaches_to_a_reusable_builder_with_the_same_import() {
    let w = World::new();
    reusable(&w, "b-other", "h0");
    reusable(&w, "b1", "h1");
    let view = run(area(&w).prepare(ask(Some(importing())))).expect("attached");
    assert_eq!(view.id, "b1");
    assert_eq!(w.lines("road "), vec!["road apply_golden_import b1"]);
}

// G6.
#[test]
fn prepare_never_attaches_without_an_import() {
    let w = World::new();
    reusable(&w, "b1", "h1");
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m2"))));
    let view = run(area(&w).prepare(ask(Some(recipe("setup"))))).expect("prepared");
    assert_eq!(view.id, "m2");
    assert!(w.lines("road apply_golden_import").is_empty());
}

// G6: in `preparing` while it runs, out once it settled.
#[test]
fn prepare_leaves_preparing_once_settled() {
    let w = World::new();
    let (tx, rx) = oneshot::channel::<()>();
    *w.roads.prepare_gate.borrow_mut() = Some(rx.map(|_| ()).boxed_local().shared());
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m1"))));
    run(async {
        let a = area(&w);
        let mut task = tokio::task::spawn_local(async move { a.prepare(ask(Some(recipe("setup")))).await.map(|v| v.id) });
        settle().await;
        still_running!(task);
        assert_eq!(w.preparing.borrow().keys().cloned().collect::<Vec<_>>(), vec!["boat/default"]);
        let _ = tx.send(());
        assert_eq!(task.await.expect("joined").expect("prepared"), "m1");
    });
    assert!(w.preparing.borrow().is_empty());
}

// G7: golden.ts:207-235.
#[test]
fn attach_stages_already_applied_and_holds_the_builder() {
    let w = World::new();
    let b = reusable(&w, "b1", "h1");
    run(area(&w).prepare(ask(Some(importing())))).expect("attached");
    let stages = w.lines("stage ");
    assert_eq!(stages, vec![format!("stage creating {ALREADY_APPLIED}"), format!("stage deploying-daemon {ALREADY_APPLIED}"), "stage ready ".to_string()]);
    assert_eq!(b.borrow().builder.import, Some(ledger("h-applied")));
    assert_eq!(b.borrow().life, BuilderLife::Own);
    assert!(b.borrow().recipe.is_some());
    assert_eq!(w.lines("hold"), vec!["hold b1"]);
    let machine = b.borrow().builder.machine.clone();
    let fake = machine.as_ref() as *const dyn wsp_engine::machine::Machine as *const FakeMachine;
    assert_eq!(unsafe { (*fake).execs() }, vec!["true"]);
}

// G7: a builder that does not answer a no-op goes.
#[test]
fn attach_kills_and_fails_a_builder_that_does_not_answer() {
    let w = World::new();
    let b = live_builder("b1", "default", "boat", BuilderLife::Reusable);
    let m = FakeMachine::new("b1");
    m.answer(Ok(exec_exit(1, "", "")));
    b.borrow_mut().builder.machine = m;
    b.borrow_mut().builder.import = Some(ledger("h1"));
    w.add_builder(b);
    let e = run(area(&w).prepare(ask(Some(importing())))).err().expect("failed");
    assert_eq!(e.message(), "the builder answered exit 1 to a no-op; it is not serving");
    let tail: Vec<String> = w.log().into_iter().filter(|l| l.starts_with("road kill") || l.starts_with("forget_if_gone") || l.starts_with("stage failed")).collect();
    assert_eq!(tail, vec!["road kill_until_gone b1", "forget_if_gone b1", "stage failed the builder answered exit 1 to a no-op; it is not serving"]);
    assert!(w.lines("hold").is_empty());
}

// G7: a failed kill is appended to the failed stage's detail.
#[test]
fn attach_appends_a_failed_kill_to_the_failed_stage() {
    let w = World::new();
    reusable(&w, "b1", "h1");
    *w.roads.import_applied.borrow_mut() = Some(Err(missing("the apply broke")));
    *w.roads.kill_fails.borrow_mut() = Some(missing("the kill was refused"));
    let e = run(area(&w).prepare(ask(Some(importing())))).err().expect("failed");
    assert_eq!(e.message(), "the apply broke");
    assert_eq!(w.lines("stage failed"), vec!["stage failed the apply broke; the kill was refused"]);
}

// G8: golden.ts:236-258.
#[test]
fn fresh_prepares_on_the_recording_backend_and_settles() {
    let w = World::new();
    let mut r = importing();
    r.labels = Some([("team".to_string(), "x".to_string())].into_iter().collect());
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m1"))));
    let view = run(area(&w).prepare(ask(Some(r)))).expect("prepared");
    assert_eq!(view.id, "m1");
    assert_eq!(w.lines("recording_creates"), vec!["recording_creates default boat import=h1 stop=true"]);
    assert_eq!(w.lines("settle_builder"), vec!["settle_builder default m1 boat"]);
    let ask = w.roads.prepare_asks.borrow()[0].clone();
    assert_eq!(ask.setup, "setup");
    assert!(ask.import.is_some());
    assert_eq!(ask.size.labels, None, "the recipe's labels go through builder_labels, not the size");
    assert!(w.builders.borrow()["m1"].borrow().recipe.is_some());
}

// G8: a prepare that fails forgets the placeholder where its machine is gone.
#[test]
fn fresh_forgets_a_gone_placeholder_when_the_prepare_fails() {
    let w = World::new();
    let (tx, rx) = oneshot::channel::<()>();
    *w.roads.prepare_gate.borrow_mut() = Some(rx.map(|_| ()).boxed_local().shared());
    w.roads.prepared.borrow_mut().push_back(Err(ImageError::Host(HostError::new("the stages failed"))));
    run(async {
        let a = area(&w);
        let mut task = tokio::task::spawn_local(async move { a.prepare(ask(Some(recipe("setup")))).await.map(|v| v.id) });
        settle().await;
        still_running!(task);
        let made = w.made.borrow().clone().expect("recording_creates was handed made");
        made(live_builder("m-ph", "default", "boat", BuilderLife::Own));
        let _ = tx.send(());
        let e = task.await.expect("joined").err().expect("failed");
        assert_eq!(e.message(), "the stages failed");
    });
    assert_eq!(w.lines("forget_if_gone"), vec!["forget_if_gone m-ph"]);
}

// G9: a stop before any machine.
#[test]
fn a_stop_before_any_machine_answers_stopped_with_no_id() {
    let w = World::new();
    let signal = Cancel::new();
    signal.cancel(None);
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m1"))));
    let e = run(area(&w).prepare(PrepareAsk { recipe: Some(recipe("setup")), signal: Some(signal), ..Default::default() })).err().expect("stopped");
    match e {
        ImageError::Stopped(s) => assert_eq!((s.builder_id, s.kept, s.left), (None, false, None)),
        other => panic!("not a stop: {other:?}"),
    }
    assert!(w.lines("settle_builder").is_empty());
}

// G9: the stop waits for the machine the create returns, then kills it.
#[test]
fn a_stop_waits_for_the_create_in_flight_then_kills_it() {
    let w = World::new();
    let (tx, rx) = oneshot::channel::<()>();
    *w.roads.prepare_gate.borrow_mut() = Some(rx.map(|_| ()).boxed_local().shared());
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m-made"))));
    let signal = Cancel::new();
    run(async {
        let a = area(&w);
        let s = signal.clone();
        let mut task = tokio::task::spawn_local(async move { a.prepare(PrepareAsk { recipe: Some(recipe("setup")), signal: Some(s), ..Default::default() }).await });
        settle().await;
        still_running!(task);
        let made = w.made.borrow().clone().expect("made");
        let ph = live_builder("m-made", "default", "boat", BuilderLife::Own);
        w.add_builder(ph.clone());
        made(ph);
        signal.cancel(None);
        settle().await;
        let _ = tx.send(());
        match task.await.expect("joined") {
            Err(ImageError::Stopped(s)) => assert_eq!((s.builder_id.as_deref(), s.kept, s.left), (Some("m-made"), false, None)),
            other => panic!("not a stop: {:?}", other.map(|v| v.id)),
        }
    });
    assert_eq!(w.lines("road kill_until_gone"), vec!["road kill_until_gone m-made"]);
    assert_eq!(w.lines("forget_builder"), vec!["forget_builder m-made"]);
    assert!(w.lines("settle_builder").is_empty());
}

// G9: an attached builder is released, its record reusable; the stop reads `mine` one turn after it starts.
#[test]
fn a_stop_on_an_attached_builder_releases_it_reusable() {
    let w = World::new();
    let b = reusable(&w, "b1", "h1");
    b.borrow_mut().record.held_by = Some(wsp_core::types::internal::HeldBy { host: "host-1".into(), pid: 4242, heartbeat: "x".into() });
    let signal = Cancel::new();
    signal.cancel(None);
    let e = run(area(&w).prepare(PrepareAsk { recipe: Some(importing()), signal: Some(signal), ..Default::default() })).err().expect("stopped");
    match e {
        ImageError::Stopped(s) => assert_eq!((s.builder_id.as_deref(), s.kept, s.left), (Some("b1"), true, None)),
        other => panic!("not a stop: {other:?}"),
    }
    assert_eq!(b.borrow().life, BuilderLife::Reusable);
    assert!(b.borrow().record.held_by.is_none());
    assert_eq!(w.store.doc("builders", "b1"), Some(serde_json::to_value(&b.borrow().record).expect("record")));
    assert!(w.lines("road kill_until_gone").is_empty() && w.lines("hold").is_empty());
}

// G9: a kill that fails says what was left, and keeps the record.
#[test]
fn a_stop_whose_kill_fails_says_what_was_left() {
    let w = World::new();
    let (tx, rx) = oneshot::channel::<()>();
    *w.roads.prepare_gate.borrow_mut() = Some(rx.map(|_| ()).boxed_local().shared());
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m-made"))));
    *w.roads.kill_fails.borrow_mut() = Some(missing("the provider is down"));
    let signal = Cancel::new();
    run(async {
        let a = area(&w);
        let s = signal.clone();
        let mut task = tokio::task::spawn_local(async move { a.prepare(PrepareAsk { recipe: Some(recipe("setup")), signal: Some(s), ..Default::default() }).await });
        settle().await;
        still_running!(task);
        let made = w.made.borrow().clone().expect("made");
        made(live_builder("m-made", "default", "boat", BuilderLife::Own));
        signal.cancel(None);
        settle().await;
        let _ = tx.send(());
        match task.await.expect("joined") {
            Err(ImageError::Stopped(s)) => assert_eq!((s.builder_id.as_deref(), s.kept, s.left.as_deref()), (Some("m-made"), false, Some("the provider is down"))),
            other => panic!("not a stop: {:?}", other.map(|v| v.id)),
        }
    });
    assert!(w.lines("forget_builder").is_empty());
}

// G9: once a stop began, the work's own answer never reaches the caller.
#[test]
fn a_stop_answers_whatever_the_work_does_after() {
    let w = World::new();
    let (tx, rx) = oneshot::channel::<()>();
    *w.roads.prepare_gate.borrow_mut() = Some(rx.map(|_| ()).boxed_local().shared());
    w.roads.prepared.borrow_mut().push_back(Ok(engine_builder(FakeMachine::new("m-late"))));
    let signal = Cancel::new();
    run(async {
        let a = area(&w);
        let s = signal.clone();
        let mut task = tokio::task::spawn_local(async move { a.prepare(PrepareAsk { recipe: Some(recipe("setup")), signal: Some(s), ..Default::default() }).await });
        settle().await;
        still_running!(task);
        signal.cancel(None);
        settle().await;
        let _ = tx.send(());
        assert!(matches!(task.await.expect("joined"), Err(ImageError::Stopped(_))));
    });
    assert!(w.lines("settle_builder").is_empty(), "the late builder is not settled");
}

// G10: golden.ts:273-281.
#[test]
fn seal_refuses_an_unknown_builder() {
    let w = World::new();
    let e = run(area(&w).seal("zz", None, None)).err().expect("refused").into_host();
    assert_eq!((e.message.as_str(), e.kind), ("no such builder: zz", None));
    assert_eq!(w.log(), vec!["ready", "refresh_builders"]);
}

// G10.
#[test]
fn seal_refuses_an_untouchable_builder() {
    let w = World::new();
    w.add_builder(live_builder("b1", "default", "boat", BuilderLife::Foreign));
    let e = run(area(&w).seal("b1", None, None)).err().expect("refused");
    assert_eq!(e.message(), "b1 is untouchable");
    assert!(w.lines("seal_entry").is_empty());
}

// G10: keep only a builder built from a recipe, and never when asked not to.
#[test]
fn seal_keeps_only_a_builder_from_a_recipe() {
    let w = World::new();
    let b = live_builder("b1", "default", "boat", BuilderLife::Own);
    let mut l = ledger("h1");
    l.recipe = Some(serde_json::from_value(json!({"ticks": [], "files": []})).unwrap_or_else(|e| panic!("a digest: {e}")));
    b.borrow_mut().builder.import = Some(l);
    w.add_builder(b);
    w.add_builder(live_builder("b2", "default", "boat", BuilderLife::Own));
    for _ in 0..3 {
        w.seals.borrow_mut().push_back(Ok(seal_result(1, "snap-1", false)));
    }
    run(area(&w).seal("b1", Some(vec![]), None)).expect("sealed");
    run(area(&w).seal("b1", None, Some(false))).expect("sealed");
    run(area(&w).seal("b2", None, None)).expect("sealed");
    assert_eq!(w.lines("seal_entry"), vec!["seal_entry b1 keep=true logins=0 copy=", "seal_entry b1 keep=false logins=0 copy=", "seal_entry b2 keep=false logins=0 copy="]);
}

// G11: golden.ts:283-290.
#[test]
fn recipe_reads_the_digest_of_the_copy_head() {
    let w = World::new();
    w.put_image_now(&sealed_image("default", 2, "h", Some("pl_1")));
    w.put_copy_now("pl_1", "default", &manifest(&[(1, "snap-1", None), (2, "snap-2", None)]));
    let digest = json!({"ticks": [], "files": []});
    w.store.seed("golden-recipes", "pl_1/default@v2", digest.clone());
    w.store.seed("golden-recipes", "pl_1/default@v1", json!({"ticks": [{"id": "x"}], "files": []}));
    let got = run(area(&w).recipe(None)).expect("recipe");
    assert_eq!(got.map(|d| serde_json::to_value(d).expect("json")), Some(digest));
    assert_eq!(run(area(&w).recipe(Some("none"))).expect("recipe"), None);
}
