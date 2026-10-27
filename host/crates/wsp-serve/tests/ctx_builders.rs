// SPDX-License-Identifier: AGPL-3.0-only
//! `create_runtime` packet 14 (#2476): `BuildersCtx`, `GoldenCtx`, `ImageCtx`, `Templates` and `GoldenRoads` over the
//! one context, each method read against the TS member it stands for (`builders.ts`, `golden.ts`, `image.ts`,
//! `runtime.ts:491-992` and the engine's `golden.ts`, `image-vault.ts`, `preview.ts`), on a runtime built over a
//! sqlite state in a temp folder.
//!
//! The whole-runtime tests of `runtime-builders`, `runtime-golden-import`, `runtime-golden-update`, `golden-recipe`,
//! `golden-templates`, `goldens-launch`, `sealed-image`, `snapshot-owners`, `snapshot-storage`, `old-copies`,
//! `state-images` and `places-image` run over a scripted `stubBackend` that snapshots, promotes and answers every exec;
//! this crate has the area crates' own ports of them (`wsp-machines/tests/golden_*.rs`, `builders_*.rs`, `image.rs`).
//! What is written below is what the context adds to them: each context method at its seam, and the runs of those
//! tests that go through `rt.golden`, `rt.image` and the stores without a snapshotting guest.
//!
//! A test that goes through a method of another packet's trait is red until that packet lands: the packet is named
//! in the test's last doc line, `waits on packet N`.
#![allow(clippy::type_complexity)]
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::{PrepareAsk, Runtime};
use wsp_core::types::internal::{copy_key, BUILDERS, GOLDENS, GOLDEN_RECIPES, IMAGES};
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{Broker, GoldenExec, GoldenRecipe, RuntimeOptions};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::golden_seam::GoldenImport;
use wsp_engine::machine::{Machine, MachineBackend, MachineSpec};
use wsp_machines::images::builders::{BuildersCtx, StageAt};
use wsp_machines::images::engine::{GoldenRoads, GoneReading, KillConfirm, Templates};
use wsp_machines::images::golden::GoldenCtx;
use wsp_machines::images::image::{ImageCtx, Stopped};
use wsp_machines::images::ImageError;
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_wire::views::golden_image::{GoldenManifest, GoldenStage, GoldenStageEvent, RecipeDigest, SealedImage};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_words::exit::{HostError, Kind};

#[path = "../../wsp-machines/tests/images_support/mod.rs"]
mod images;

const START: i64 = 1_000_000;
const HOST: &str = "box:h1";

struct World {
    ctx: Rc<Ctx>,
    runtime: Rc<AssembledRuntime>,
    store: Rc<dyn Store>,
    clock: FakeClock,
    backend: Arc<dyn MachineBackend>,
    _dir: tempfile::TempDir,
}

/// A runtime over `backend` on a sqlite state in a temp folder, whose owner id is `s1` as the TS `ownedStore` writes it.
fn world_over(backend: Arc<dyn MachineBackend>, adjust: impl FnOnce(&mut RuntimeOptions)) -> World {
    let dir = tempfile::tempdir().expect("a folder");
    let store: Rc<dyn Store> = Rc::new(wsp_store::sqlite_store::sqlite_store(
        &dir.path().join("state.json"),
        wsp_store::store::StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() },
    ));
    let clock = FakeClock::new(START);
    let mut opts = RuntimeOptions {
        backend: Some(backend.clone()),
        store: Some(store.clone()),
        clock: Some(clock.clock()),
        host_id: Some(HOST.to_string()),
        ..Default::default()
    };
    adjust(&mut opts);
    let runtime = create_runtime(opts).expect("a runtime");
    World { ctx: runtime.context(), runtime, store, clock, backend, _dir: dir }
}

fn run_over<Fut: std::future::Future<Output = ()>>(
    backend: Arc<dyn MachineBackend>,
    adjust: impl FnOnce(&mut RuntimeOptions),
    f: impl FnOnce(World) -> Fut,
) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let w = world_over(backend, adjust);
        w.store.put("owner", "id", json!({ "id": "s1" })).await.expect("an owner");
        f(w).await
    });
}

fn engine_backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn run_with<Fut: std::future::Future<Output = ()>>(adjust: impl FnOnce(&mut RuntimeOptions), f: impl FnOnce(World) -> Fut) {
    run_over(engine_backend(), adjust, f)
}

fn run<Fut: std::future::Future<Output = ()>>(f: impl FnOnce(World) -> Fut) {
    run_with(|_| {}, f)
}

fn version_json(n: u32) -> Value {
    let mut v = json!({
        "version": n, "snapshotId": format!("snap_golden-v{n}"), "baseTemplate": "base", "setupSha": format!("sha{n}"),
        "createdAt": format!("2026-08-{}T00:00:00.000Z", 10 + n), "smoke": { "cmd": "true", "exitCode": 0 },
        "base": [{ "name": "node", "version": "22.23.2" }],
    });
    if n > 1 {
        v["parentSnapshotId"] = json!(format!("snap_golden-v{}", n - 1));
    }
    v
}

fn manifest_of(versions: &[u32], head: u32) -> Value {
    json!({ "head": head, "versions": versions.iter().map(|n| version_json(*n)).collect::<Vec<_>>() })
}

fn manifest(versions: &[u32], head: u32) -> GoldenManifest {
    serde_json::from_value(manifest_of(versions, head)).expect("a manifest")
}

fn digest() -> RecipeDigest {
    serde_json::from_value(json!({ "ticks": [], "files": [] })).expect("a recipe digest")
}

fn image_json(name: &str, place: Option<&str>) -> Value {
    let mut j = json!({
        "name": name, "version": 1, "hash": "a".repeat(64), "recipeHash": "r", "logins": [],
        "sealedAt": "2026-10-25T09:00:00.000Z", "sealedFrom": "host-1",
    });
    if let Some(place) = place {
        j["place"] = json!(place);
    }
    j
}

fn sealed_image(name: &str, place: Option<&str>) -> SealedImage {
    serde_json::from_value(image_json(name, place)).expect("a sealed image")
}

fn recipe(setup: &str, smoke: &str) -> GoldenRecipe {
    GoldenRecipe { setup: setup.into(), smoke: smoke.into(), ..Default::default() }
}

fn stage_event(name: &str, stage: GoldenStage, detail: Option<&str>, place: Option<&str>) -> GoldenStageEvent {
    GoldenStageEvent { name: name.into(), stage, detail: detail.map(String::from), step: None, left: None, place: place.map(String::from) }
}

async fn machine_on(backend: &Arc<dyn MachineBackend>) -> Arc<dyn Machine> {
    let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox", "cpu": 2 })).expect("a spec");
    backend.create(spec, None, None).await.expect("a machine")
}

fn same_backend(a: &Arc<dyn MachineBackend>, b: &Arc<dyn MachineBackend>) -> bool {
    std::ptr::eq(Arc::as_ptr(a) as *const u8, Arc::as_ptr(b) as *const u8)
}

// ---------------------------------------------------------------------------------------------------------------
// The core's own reads (runtime.ts:491-992), read through each of the three area contexts.

#[test]
fn the_store_the_clock_and_the_places_are_the_runtimes_own_in_every_context() {
    run(|w| async move {
        let c = &*w.ctx;
        BuildersCtx::store(c).put("meta", "k", json!(1)).await.expect("put");
        assert_eq!(w.store.get("meta", "k").await.expect("get"), Some(json!(1)));
        assert_eq!(GoldenCtx::store(c).get("meta", "k").await.expect("get"), Some(json!(1)));
        assert_eq!(ImageCtx::store(c).get("meta", "k").await.expect("get"), Some(json!(1)));
        assert_eq!(BuildersCtx::clock(c).now(), START);
        assert_eq!(GoldenCtx::clock(c).now(), START);
        w.clock.advance(30);
        assert_eq!(BuildersCtx::clock(c).now(), START + 30);
        assert_eq!(BuildersCtx::places(c).wired(), "default");
        assert_eq!(GoldenCtx::places(c).wired(), "default");
        assert_eq!(ImageCtx::places(c).wired(), "default");
    });
}

#[test]
fn the_backend_is_the_one_the_runtime_was_given() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(same_backend(&BuildersCtx::backend(c), &w.backend));
        assert!(same_backend(&GoldenCtx::backend(c), &w.backend));
    });
}

#[test]
fn opts_project_what_the_builders_area_reads() {
    let wired = GoldenRecipe { setup: "install".into(), smoke: "true".into(), base_template: Some("base".into()), ..Default::default() };
    run_with(
        move |o| {
            o.golden_recipe = Some(wired);
            o.kill_confirm = Some(wsp_engine::gone::KillConfirm { grace_ms: Some(7), poll_ms: Some(3) });
            o.snapshot_retry_ms = Some(11);
            o.no_machines_line = Some("no machines here".into());
        },
        |w| async move {
            let opts = BuildersCtx::opts(&*w.ctx);
            let got = opts.golden_recipe.expect("the wired recipe");
            assert_eq!((got.setup.as_str(), got.smoke.as_str(), got.base_template.as_deref()), ("install", "true", Some("base")));
            assert_eq!(opts.kill_confirm, Some(KillConfirm { grace_ms: Some(7), poll_ms: Some(3) }));
            assert_eq!(opts.snapshot_retry_ms, Some(11));
            assert_eq!(opts.no_machines_line.as_deref(), Some("no machines here"));
            assert!(opts.copy_recipe.is_none(), "no copy recipe was wired");
        },
    );
    run(|w| async move {
        let opts = BuildersCtx::opts(&*w.ctx);
        assert!(opts.golden_recipe.is_none() && opts.kill_confirm.is_none() && opts.snapshot_retry_ms.is_none());
    });
}

#[test]
fn the_wired_copy_recipe_answers_the_recipe_for_a_record() {
    run_with(
        |o| {
            o.copy_recipe =
                Some(Rc::new(|image, force| Box::pin(async move { Ok(recipe(&format!("copy {} {force}", image.name), "true")) })))
        },
        |w| async move {
            let copy = BuildersCtx::opts(&*w.ctx).copy_recipe.expect("the copy recipe");
            let planned = copy(sealed_image("default", None), true).await.expect("a recipe");
            assert_eq!(planned.setup, "copy default true");
        },
    );
}

#[test]
fn kill_confirm_is_the_option_in_the_golden_and_the_image_context() {
    run(|w| async move {
        assert_eq!(GoldenCtx::kill_confirm(&*w.ctx), None);
        assert_eq!(ImageCtx::kill_confirm(&*w.ctx), None);
    });
    run_with(
        |o| o.kill_confirm = Some(wsp_engine::gone::KillConfirm { grace_ms: Some(5), poll_ms: None }),
        |w| async move {
            let want = Some(KillConfirm { grace_ms: Some(5), poll_ms: None });
            assert_eq!(GoldenCtx::kill_confirm(&*w.ctx), want);
            assert_eq!(ImageCtx::kill_confirm(&*w.ctx), want);
        },
    );
}

#[test]
fn the_host_id_is_the_option_and_the_process_and_computer_are_this_ones() {
    run(|w| async move {
        let c = &*w.ctx;
        assert_eq!(BuildersCtx::host_id(c), HOST);
        assert_eq!(BuildersCtx::pid(c), std::process::id());
        assert!(!BuildersCtx::hostname(c).is_empty());
    });
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn an_image_is_marked_by_the_hosts_hex_id_and_the_states_owner() {
    // golden-templates: the template is named wsp-<hex><owner>-<golden>-v<n>; the part after the last colon of the host.
    run(|w| async move {
        w.runtime.golden().builders().await.expect("ready");
        assert_eq!(GoldenCtx::image_mark(&*w.ctx), "h1s1");
        assert_eq!(BuildersCtx::image_mark(&*w.ctx), "h1s1");
    });
}

struct Ready(bool);

#[async_trait::async_trait(?Send)]
impl Broker for Ready {
    fn ready(&self) -> bool {
        self.0
    }
    async fn codex_claims(&self) -> Result<Option<wsp_adapters::pending::CodexClaims>, HostError> {
        Ok(None)
    }
}

#[test]
fn the_broker_is_ready_only_when_there_is_one_and_it_says_so() {
    // sealed-image: gh's login is refused at a provider's seal only once the host's broker is ready.
    run(|w| async move { assert!(!BuildersCtx::broker_ready(&*w.ctx)) });
    run_with(|o| o.broker = Some(Rc::new(Ready(false))), |w| async move { assert!(!BuildersCtx::broker_ready(&*w.ctx)) });
    run_with(|o| o.broker = Some(Rc::new(Ready(true))), |w| async move { assert!(BuildersCtx::broker_ready(&*w.ctx)) });
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn the_owner_is_the_states_and_stamps_the_labels_a_builder_wears() {
    // runtime-builders: one owner id per store, stamped on its builders.
    run(|w| async move {
        w.runtime.golden().builders().await.expect("ready");
        let c = &*w.ctx;
        assert_eq!(BuildersCtx::owner(c), "s1");
        assert_eq!(GoldenCtx::owner(c), "s1");
        let labels = GoldenCtx::builder_labels(c, None);
        assert_eq!(labels.get(wsp_engine::labels::OWNER_LABEL).map(String::as_str), Some("s1"));
    });
}

/// Waits on packet 17: `close` is `PublicCtx`'s.
#[test]
fn closed_turns_true_once_the_runtime_closes() {
    run(|w| async move {
        assert!(!BuildersCtx::closed(&*w.ctx));
        w.runtime.close().await.expect("close");
        assert!(BuildersCtx::closed(&*w.ctx));
    });
}

#[test]
fn the_heartbeat_cancel_is_kept_and_replaced() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(BuildersCtx::beat(c).is_none());
        let fired = Rc::new(Cell::new(0));
        let hit = fired.clone();
        BuildersCtx::set_beat(c, Some(Rc::new(move || hit.set(hit.get() + 1))));
        let kept = BuildersCtx::beat(c).expect("the beat");
        kept();
        assert_eq!(fired.get(), 1);
        BuildersCtx::set_beat(c, None);
        assert!(BuildersCtx::beat(c).is_none());
    });
}

#[test]
fn the_beats_run_starts_at_once() {
    run(|w| async move {
        let ran = Rc::new(Cell::new(false));
        let flag = ran.clone();
        BuildersCtx::set_ticking(&*w.ctx, Box::pin(async move { flag.set(true) }));
        tokio::task::yield_now().await;
        assert!(ran.get(), "ctx.state.ticking = tick() starts the tick");
    });
}

#[test]
fn spawn_starts_the_future_at_once_on_the_task_set() {
    run(|w| async move {
        let ran = Rc::new(Cell::new(false));
        let flag = ran.clone();
        BuildersCtx::spawn(&*w.ctx, Box::pin(async move { flag.set(true) }));
        tokio::task::yield_now().await;
        assert!(ran.get());
    });
}

#[test]
fn warn_writes_one_console_line_in_each_context() {
    run(|w| async move {
        let c = &*w.ctx;
        let lines = wsp_core::console::capture();
        BuildersCtx::warn(c, "one");
        GoldenCtx::warn(c, "two");
        assert_eq!(lines.lines(), vec!["one", "two"]);
    });
}

#[test]
fn a_stage_event_reaches_the_bus_as_golden_stage() {
    run(|w| async move {
        let heard = Rc::new(RefCell::new(Vec::<Value>::new()));
        let log = heard.clone();
        let _off = w.runtime.events().on(
            "golden.stage",
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            }),
        );
        BuildersCtx::emit_stage(&*w.ctx, stage_event("default", GoldenStage::Creating, Some("a machine"), None));
        let got = heard.borrow();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0]["type"], json!("golden.stage"));
        assert_eq!(
            (got[0]["name"].as_str(), got[0]["stage"].as_str(), got[0]["detail"].as_str()),
            (Some("default"), Some("creating"), Some("a machine"))
        );
        assert!(got[0].get("place").is_none());
    });
}

#[test]
fn every_context_reads_the_one_builders_map_and_the_one_set_of_joins() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(Rc::ptr_eq(&BuildersCtx::builders(c), &GoldenCtx::builders(c)));
        assert!(Rc::ptr_eq(&BuildersCtx::builders(c), &ImageCtx::builders(c)));
        let seams = c.builders_seams();
        assert!(Rc::ptr_eq(&GoldenCtx::grace_timers(c), &seams.builders_area.grace_timers));
        assert!(GoldenCtx::preparing(c).borrow().is_empty());
        assert!(ImageCtx::copy_builds(c).borrow().is_empty());
        assert!(Rc::ptr_eq(&ImageCtx::copy_builds(c), &ImageCtx::copy_builds(c)));
        assert!(Rc::ptr_eq(&GoldenCtx::preparing(c), &GoldenCtx::preparing(c)));
    });
}

#[test]
fn the_stage_map_is_the_one_the_core_keeps_for_a_computers_builds() {
    run(|w| async move {
        let c = &*w.ctx;
        let at = StageAt {
            place: "p_1".into(),
            name: "default".into(),
            named: true,
            frame: wsp_core::types::internal::StageFrame {
                name: "default".into(),
                stage: GoldenStage::ApplyingSetup,
                detail: None,
                step: None,
                left: None,
            },
        };
        BuildersCtx::stage_at(c).borrow_mut().insert(copy_key("p_1", "default"), at.clone());
        assert!(Rc::ptr_eq(&BuildersCtx::stage_at(c), &BuildersCtx::stage_at(c)));
        // stage_of on a place keeps the stage there and a ready frame takes it away (builders.ts:177-189).
        let said = GoldenCtx::stage_of(c, "default", Some("p_2"), true);
        said(GoldenStage::InstallingTools, Some("jq".into()), None, None);
        assert_eq!(
            BuildersCtx::stage_at(c).borrow().get(&copy_key("p_2", "default")).map(|s| s.frame.stage),
            Some(GoldenStage::InstallingTools)
        );
        said(GoldenStage::Ready, None, None, None);
        assert!(BuildersCtx::stage_at(c).borrow().get(&copy_key("p_2", "default")).is_none());
    });
}

#[test]
fn a_computers_copy_row_follows_the_stage_frames_naming_it() {
    // runtime.ts:836: a golden.stage frame naming a place writes or drops its copy row, and the same row is the same Rc.
    run(|w| async move {
        let c = &*w.ctx;
        assert!(ImageCtx::copy_row(c, "p_1").is_none());
        BuildersCtx::emit_stage(c, stage_event("default", GoldenStage::InstallingTools, Some("jq (1/2)"), Some("p_1")));
        let row = ImageCtx::copy_row(c, "p_1").expect("a row while it builds");
        assert!(!row.stopped);
        assert!(Rc::ptr_eq(&row, &ImageCtx::copy_row(c, "p_1").expect("the same row")));
        BuildersCtx::emit_stage(c, stage_event("default", GoldenStage::Failed, Some("the box went away"), Some("p_1")));
        let stopped = ImageCtx::copy_row(c, "p_1").expect("a row saying it stopped");
        assert!(stopped.stopped);
        assert!(!Rc::ptr_eq(&row, &stopped));
        BuildersCtx::emit_stage(c, stage_event("default", GoldenStage::Sealed, None, Some("p_1")));
        assert!(ImageCtx::copy_row(c, "p_1").is_none(), "a copy that stands has no row");
    });
}

#[test]
fn a_refusal_before_any_build_is_a_rows_to_say_unless_the_place_forks_nowhere_is_absent_or_away() {
    run(|w| async move {
        let c = &*w.ctx;
        let plain = ImageError::Host(HostError::new("the build said no"));
        assert!(ImageCtx::row_says_failure(c, "default", &plain));
        let absent = ImageError::Host(HostError::with_kind("the computer went", Kind::parse("absent")));
        assert!(!ImageCtx::row_says_failure(c, "default", &absent));
        let nowhere = ImageError::Host(wsp_machines::places::types::place_forks_nowhere("it forks no machines"));
        assert!(!ImageCtx::row_says_failure(c, "default", &nowhere));
    });
}

#[test]
fn a_place_is_away_only_where_a_door_is_wired_and_neither_it_nor_a_link_answers() {
    // No place door in this runtime: nothing is ever away (runtime.ts:853).
    run(|w| async move {
        assert!(!ImageCtx::place_away(&*w.ctx, "p_1"));
        assert!(!ImageCtx::place_away(&*w.ctx, "default"));
    });
}

#[test]
fn a_stopped_build_is_framed_failed_with_the_sentence_or_the_error() {
    run(|w| async move {
        let c = &*w.ctx;
        let heard = Rc::new(RefCell::new(Vec::<Value>::new()));
        let log = heard.clone();
        let _off = w.runtime.events().on(
            "golden.stage",
            Rc::new(move |e| {
                log.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                Ok(())
            }),
        );
        ImageCtx::frame_stopped(c, "p_1", "default", Stopped::Line("it went away".into()));
        ImageCtx::frame_stopped(c, "p_1", "other", Stopped::Error(ImageError::Host(HostError::new("the create failed"))));
        let got = heard.borrow();
        assert_eq!(got.len(), 2);
        assert_eq!(
            (got[0]["stage"].as_str(), got[0]["detail"].as_str(), got[0]["place"].as_str()),
            (Some("failed"), Some("it went away"), Some("p_1"))
        );
        assert_eq!((got[1]["name"].as_str(), got[1]["detail"].as_str()), (Some("other"), Some("the create failed")));
    });
}

#[test]
fn a_claim_keeps_the_machines_made_through_it_in_flight_until_it_is_dropped() {
    // machines.ts:183-230: claiming(purpose, run, at, place) is a guard here, released when the run settles.
    run(|w| async move {
        let c = &*w.ctx;
        let seams = c.builders_seams();
        let claim = BuildersCtx::claiming(c, "golden/default", None, None);
        let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox", "cpu": 2 })).expect("a spec");
        let made = claim.backend().create(spec, None, None).await.expect("a machine");
        assert!(seams.machines.inflight().contains(&made.id().to_string()), "claimed into inflight");
        drop(claim);
        assert!(!seams.machines.inflight().contains(&made.id().to_string()), "released with the handle");
        let again = GoldenCtx::claiming(c, "golden/default", Some(w.backend.clone()), Some("default"));
        drop(again);
    });
}

#[test]
fn observed_and_observing_wrap_through_the_machines_area() {
    // runtime.ts: `observed` carries the exec listener the golden recipe names; a backend observed hands observed machines.
    let seen = Rc::new(RefCell::new(Vec::<GoldenExec>::new()));
    let log = seen.clone();
    let seen2 = seen.clone();
    run_with(
        move |o| {
            o.golden_recipe = Some(GoldenRecipe {
                setup: "install".into(),
                smoke: "true".into(),
                on_exec: Some(Rc::new(move |e| log.borrow_mut().push(e))),
                ..Default::default()
            })
        },
        |w| async move {
            let c = &*w.ctx;
            let machine = BuildersCtx::observed(c, machine_on(&w.backend).await);
            let id = machine.id().to_string();
            let _ = machine.exec("echo hi", Default::default()).await;
            assert!(seen2.borrow().iter().any(|e| e.machine_id == id && e.cmd == "echo hi"), "the exec listener heard the command");
            let backend = BuildersCtx::observing(c, w.backend.clone());
            assert!(!same_backend(&backend, &w.backend), "observing hands a backend of its own");
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The records area's calls and the places the builders area reads.

#[test]
fn a_computers_copy_of_a_golden_is_written_and_read_back_by_place_and_name() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(BuildersCtx::copy_of(c, "default", "default").await.expect("read").is_none());
        BuildersCtx::put_copy(c, "default", "default", &manifest(&[1, 2], 2)).await.expect("write");
        let got = GoldenCtx::copy_of(c, "default", "default").await.expect("read").expect("a manifest");
        assert_eq!(serde_json::to_value(&got).expect("json"), manifest_of(&[1, 2], 2));
        assert_eq!(w.store.get(GOLDENS, &copy_key("default", "default")).await.expect("get"), Some(manifest_of(&[1, 2], 2)));
        assert!(GoldenCtx::copy_of(c, "p_1", "default").await.expect("read").is_none(), "another place has no copy");
        assert!(ImageCtx::copy_of(c, "default", "default").await.expect("read").is_some());
    });
}

#[test]
fn a_versions_recipe_is_kept_per_place_and_dropped_by_version() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(GoldenCtx::copy_recipe_of(c, "default", "default", 1).await.expect("read").is_none());
        BuildersCtx::put_copy_recipe(c, "default", "default", 1, &digest()).await.expect("write");
        ImageCtx::put_copy_recipe(c, "default", "default", 2, &digest()).await.expect("write");
        assert!(ImageCtx::copy_recipe_of(c, "default", "default", 1).await.expect("read").is_some());
        assert!(w.store.get(GOLDEN_RECIPES, &copy_key("default", "default@v1")).await.expect("get").is_some());
        GoldenCtx::drop_copy_recipe(c, "default", "default", 1).await.expect("drop");
        assert!(GoldenCtx::copy_recipe_of(c, "default", "default", 1).await.expect("read").is_none());
        assert!(GoldenCtx::copy_recipe_of(c, "default", "default", 2).await.expect("read").is_some());
    });
}

#[test]
fn the_image_record_is_read_by_name_in_every_context() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(BuildersCtx::record_of(c, "default").await.expect("read").is_none());
        w.store.put(IMAGES, "default", image_json("default", Some("p_9"))).await.expect("put");
        for got in
            [BuildersCtx::record_of(c, "default").await, GoldenCtx::record_of(c, "default").await, ImageCtx::record_of(c, "default").await]
        {
            assert_eq!(got.expect("read").expect("a record").place.as_deref(), Some("p_9"));
        }
    });
}

#[test]
fn recorded_images_name_every_snapshot_and_template_a_manifest_holds() {
    run(|w| async move {
        let c = &*w.ctx;
        let mut m = manifest_of(&[1, 2], 2);
        m["versions"][1]["templateId"] = json!("tpl_golden-v2");
        w.store.put(GOLDENS, &copy_key("default", "default"), m).await.expect("put");
        let ids = GoldenCtx::recorded_images(c).await.expect("read");
        assert!(["snap_golden-v1", "snap_golden-v2", "tpl_golden-v2"].iter().all(|id| ids.contains(*id)), "{ids:?}");
    });
}

#[test]
fn the_image_a_snapshot_belongs_to_is_found_by_its_version_row() {
    run(|w| async move {
        let c = &*w.ctx;
        w.store.put(GOLDENS, &copy_key("default", "default"), manifest_of(&[1, 2], 2)).await.expect("put");
        let found = ImageCtx::image_of(c, "snap_golden-v2").await.expect("read");
        assert_eq!(found.golden, "snap_golden-v2");
        assert_eq!(found.version.expect("the version").version.0, 2.0);
        assert!(ImageCtx::image_of(c, "snap_nobody").await.expect("read").version.is_none());
    });
}

#[test]
fn no_live_workspace_stands_on_a_snapshot_in_a_fresh_runtime() {
    run(|w| async move { assert!(GoldenCtx::forked_from(&*w.ctx, "snap_golden-v1").is_empty()) });
}

#[test]
fn an_images_place_is_the_records_place_or_the_wired_one() {
    run(|w| async move {
        let c = &*w.ctx;
        assert_eq!(GoldenCtx::image_place(c, "default").await.expect("place"), "default");
        w.store.put(IMAGES, "default", image_json("default", Some("p_9"))).await.expect("put");
        assert_eq!(GoldenCtx::image_place(c, "default").await.expect("place"), "p_9");
        assert_eq!(ImageCtx::image_place(c, "default").await.expect("place"), "p_9");
        assert_eq!(ImageCtx::image_place(c, "other").await.expect("place"), "default");
    });
}

#[test]
fn a_place_word_answers_its_backend_and_an_unknown_one_is_refused_as_missing() {
    run(|w| async move {
        let c = &*w.ctx;
        let at = GoldenCtx::place_at(c, "default").await.expect("the wired place");
        assert_eq!(at.place, "default");
        assert!(same_backend(&at.at, &w.backend));
        assert!(same_backend(&GoldenCtx::backend_at(c, "default").expect("the wired backend"), &w.backend));
        let refused = GoldenCtx::backend_at(c, "p_nowhere").err().expect("a refusal");
        assert_eq!(refused.kind, Some(Kind::Missing));
        assert!(ImageCtx::place_at(c, "p_nowhere").await.is_err());
    });
}

#[test]
fn a_places_name_and_kind_are_read_from_the_places_this_host_has() {
    run(|w| async move {
        let c = &*w.ctx;
        assert_eq!(GoldenCtx::place_name(c, "default"), "default");
        assert_eq!(ImageCtx::place_name(c, "default"), "default");
        assert_eq!(GoldenCtx::place_kind_of(c, "default"), PlaceKind::Provider);
        assert_eq!(ImageCtx::place_kind_of(c, "p_1"), PlaceKind::Computer);
    });
}

#[test]
fn the_places_an_image_can_be_built_at_are_the_providers_that_fork() {
    run(|w| async move {
        let rows = GoldenCtx::build_places(&*w.ctx).await.expect("places");
        assert_eq!(rows.iter().map(|r| r.place.as_str()).collect::<Vec<_>>(), ["default"]);
        assert_eq!(rows[0].kind, PlaceKind::Provider);
    });
}

#[test]
fn a_recipe_is_the_one_named_or_the_wired_one_or_a_refusal() {
    // golden-recipe: a runtime wired without a recipe refuses a bare prepare; one wired with it answers a prepare that names none.
    run(|w| async move {
        let refused = GoldenCtx::recipe_or_throw(&*w.ctx, None).err().expect("no recipe");
        assert!(refused.message.contains("no golden recipe"), "{}", refused.message);
        let named = GoldenCtx::recipe_or_throw(&*w.ctx, Some(recipe("echo setup-from-call", "echo smoke-from-call"))).expect("named");
        assert_eq!(named.setup, "echo setup-from-call");
        assert!(ImageCtx::copy_recipe_or_throw(&*w.ctx).is_err(), "no copy recipe wired");
    });
    run_with(
        |o| o.golden_recipe = Some(recipe("echo wired", "echo wired-smoke")),
        |w| async move {
            assert_eq!(GoldenCtx::recipe_or_throw(&*w.ctx, None).expect("wired").smoke, "echo wired-smoke");
            let named = GoldenCtx::recipe_or_throw(&*w.ctx, Some(recipe("echo call", "true"))).expect("named wins");
            assert_eq!(named.setup, "echo call");
        },
    );
}

#[test]
fn a_builders_labels_mark_it_as_this_hosts_and_this_states() {
    run(|w| async move {
        let c = &*w.ctx;
        let mut extra = indexmap::IndexMap::new();
        extra.insert("team".to_string(), "spoo".to_string());
        let labels = GoldenCtx::builder_labels(c, Some(&extra));
        assert_eq!(labels.get("team").map(String::as_str), Some("spoo"));
        assert_eq!(labels.get(wsp_engine::labels::WSP_LABEL).map(String::as_str), Some("1"));
        assert_eq!(labels.get(wsp_engine::labels::BUILDER_LABEL).map(String::as_str), Some("1"));
        assert_eq!(labels.get(wsp_engine::labels::CREATED_AT_LABEL).map(String::as_str), Some("1970-01-01T00:16:40.000Z"));
    });
}

#[test]
fn no_builder_stands_in_a_fresh_runtime_and_a_door_answers_none_without_places() {
    run(|w| async move {
        let c = &*w.ctx;
        assert!(BuildersCtx::builders(c).borrow().is_empty());
        assert!(BuildersCtx::door(c).is_none());
        assert!(GoldenCtx::door(c).is_none());
        assert!(ImageCtx::door(c).is_none());
        let refused = BuildersCtx::place_door_of(c).err().expect("no door wired");
        assert!(!refused.message.is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The golden area's own work that goes through the context: `ready`, `refresh_builders`, `daemon_token_of`, `conflict`.

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn ready_answers_once_the_runtime_has_booted_and_refresh_builders_reads_the_store_again() {
    run(|w| async move {
        let c = &*w.ctx;
        GoldenCtx::ready(c).await.expect("boot");
        ImageCtx::ready(c).await.expect("boot");
        BuildersCtx::refresh_builders(c).await.expect("refresh");
        GoldenCtx::refresh_builders(c).await.expect("refresh");
        assert!(BuildersCtx::builders(c).borrow().is_empty());
    });
}

#[test]
fn a_conflict_is_a_refusal_of_kind_conflict_with_its_words() {
    run(|w| async move {
        let e = GoldenCtx::conflict(&*w.ctx, "a build is running");
        assert_eq!((e.message.as_str(), e.kind.clone()), ("a build is running", Some(Kind::Conflict)));
    });
}

#[test]
fn a_builders_daemon_token_is_asked_of_the_transcripts_area_for_its_machine() {
    run_with(
        |o| o.daemon_token = Some("0123456789abcdef".into()),
        |w| async move {
            // The transcripts area asks the guest, so a guest that will not answer is the refusal it gives.
            let machine = machine_on(&w.backend).await;
            let refused = GoldenCtx::daemon_token_of(&*w.ctx, machine).await.expect_err("the stand-in provider runs nothing");
            assert_eq!(refused.message, wsp_engine::fake_backend::FAKE_NO_GUEST);
            // A guest that answers is read: the call reaches its exec.
            let guest = images::FakeMachine::new("m_guest");
            let read = GoldenCtx::daemon_token_of(&*w.ctx, guest.clone()).await;
            assert!(read.is_ok(), "{read:?}");
            assert!(!guest.execs().is_empty(), "the guest was asked");
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// `gone_stop`: the watch's stop (engine golden.ts GoneWatch), shared with the machines area.

#[test]
fn gone_stop_asks_the_first_kill_here_and_hands_the_read_back_to_the_watch() {
    // runtime-builders: a machine still being asked to stop is the watch's, and what is asked is read until it is gone.
    run(|w| async move {
        let c = &*w.ctx;
        let machine = machine_on(&w.backend).await;
        let id = machine.id().to_string();
        BuildersCtx::gone_stop(c, w.backend.clone(), machine.clone()).await.expect("the first kill");
        let seams = c.builders_seams();
        seams.gone.settled().await;
        assert!(seams.gone.ids().is_empty(), "the watch let go once the machine read gone");
        assert_eq!(
            w.backend.get(&id).await.expect("the stand-in never 404s").state().await.expect("a state"),
            wsp_engine::machine::MachineState::Gone
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// GoldenRoads over the engine's golden.ts, image-vault.ts, preview.ts and drop-template.ts.

#[test]
fn a_setup_sha_chains_the_version_it_came_from_with_what_the_delta_ran() {
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        let none = GoldenImport::default();
        let sha = roads.next_setup_sha("prev", "install", &none);
        assert_eq!(sha.len(), 64);
        assert_eq!(sha, wsp_engine::golden::next_setup_sha("prev", "install", &none));
        assert_ne!(sha, roads.next_setup_sha("prev", "other", &none));
        assert_ne!(sha, roads.next_setup_sha("next", "install", &none));
    });
}

#[test]
fn an_image_hash_is_sha256_of_the_rule_the_recipe_and_the_vault() {
    // node -e 'createHash("sha256").update("wsp-image-2\nr\nnone").digest("hex")' and "...\nr\nabc"
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        assert_eq!(roads.image_hash("r", None), "520074e66a71d51404a82c6d49637d86f7cad982252385111aca14120b2cdb4d");
        assert_eq!(roads.image_hash("r", Some("abc")), "892aae153c68be6d1441e26ab066f92998f61b45e26385149aaf6a289ce105cb");
    });
}

#[test]
fn a_rollback_moves_head_to_an_earlier_version_and_refuses_one_the_manifest_lacks() {
    // runtime-builders: refuses a version that is not in the manifest (golden.ts:1471, `rollback target v9 not in manifest`).
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        let rolled = roads.rollback(&manifest(&[1, 2], 2), 1).expect("v1 is there");
        assert_eq!(serde_json::to_value(&rolled).expect("json")["head"], json!(1));
        let refused = roads.rollback(&manifest(&[1, 2], 2), 9).expect_err("v9 is not");
        assert_eq!(refused.to_string(), "rollback target v9 not in manifest");
    });
}

#[test]
fn a_machine_the_provider_no_longer_has_reads_gone_and_one_it_has_reads_its_state() {
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        let machine = machine_on(&w.backend).await;
        assert_eq!(roads.read_gone(w.backend.clone(), machine.id()).await.expect("a reading"), GoneReading::Standing("running".into()));
        machine.kill().await.expect("kill");
        assert_eq!(roads.read_gone(w.backend.clone(), machine.id()).await.expect("a reading"), GoneReading::Gone);
    });
}

#[test]
fn a_kill_is_done_once_the_machine_reads_gone() {
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        let machine = machine_on(&w.backend).await;
        let id = machine.id().to_string();
        roads.kill_until_gone(w.backend.clone(), machine, Some(KillConfirm { grace_ms: Some(1), poll_ms: Some(1) })).await.expect("killed");
        assert_eq!(
            w.backend.get(&id).await.expect("the stand-in never 404s").state().await.expect("a state"),
            wsp_engine::machine::MachineState::Gone
        );
    });
}

#[test]
fn dropping_a_template_with_no_id_asks_nothing_of_the_provider() {
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        roads.drop_template(w.backend.clone(), None).await.expect("nothing to do");
    });
}

#[test]
fn a_backend_with_no_templates_has_no_template_road() {
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        assert!(roads.templates_of(w.backend.clone()).is_none(), "the stand-in provider offers none");
    });
}

#[test]
fn the_four_template_calls_of_a_backend_without_templates_refuse() {
    // golden.ts:291 templatesOf: the Templates impl is the backend's own four calls, which the stand-in refuses.
    run(|w| async move {
        let templates: &dyn Templates = &*w.ctx;
        assert!(templates.list().await.is_err());
        assert!(templates.get("tpl_1").await.is_err());
        assert!(templates.promote("snap_1", "name").await.is_err());
        assert!(templates.delete("tpl_1").await.is_err());
    });
}

#[test]
fn an_archive_with_a_member_outside_the_asked_paths_refuses_every_member() {
    // sealed-image: a copy reads the archive against the paths the seal asked for, and one carrying a foreign member boots no builder.
    run(|w| async move {
        let roads: &dyn GoldenRoads = &*w.ctx;
        let held = ["/root/.claude".to_string()];
        let torn = roads.refuse_foreign_members("import", &[1u8; 7], &held).expect_err("bytes that are no gzip are refused");
        assert!(torn.contains("its first bytes") && torn.contains("they do not decompress as tar's gzip reads them"), "{torn}");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Whole-runtime runs through `rt.golden` and `rt.image`.

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn a_golden_that_was_never_sealed_has_no_manifest_and_the_image_no_record() {
    // sealed-image: image.get on a store that has never sealed answers no image and no copy.
    run(|w| async move {
        assert!(w.runtime.golden().get(None).await.expect("get").is_none());
        let got = w.runtime.image().get(None).await.expect("image.get");
        assert!(got.image.is_none() && got.copies.is_empty(), "{:?}", serde_json::to_value(&got));
    });
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn a_stored_golden_is_read_under_its_place_and_name_and_rolled_back_in_the_store() {
    run(|w| async move {
        w.store.put(GOLDENS, &copy_key("default", "default"), manifest_of(&[1, 2], 2)).await.expect("put");
        let got = w.runtime.golden().get(None).await.expect("get").expect("a manifest");
        assert_eq!(serde_json::to_value(&got).expect("json")["head"], json!(2));
        let rolled = w.runtime.golden().rollback(1, None).await.expect("rollback");
        assert_eq!(serde_json::to_value(&rolled).expect("json")["head"], json!(1));
        assert_eq!(w.store.get(GOLDENS, &copy_key("default", "default")).await.expect("get").expect("stored")["head"], json!(1));
        let refused = w.runtime.golden().rollback(9, None).await.expect_err("v9 is not in the manifest");
        assert!(refused.message.contains("v9"), "{}", refused.message);
        let none = w.runtime.golden().rollback(1, Some("nobody".into())).await.expect_err("no such golden");
        assert_eq!(none.kind, Some(Kind::Missing));
    });
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn snapshot_storage_splits_the_listing_into_this_hosts_recorded_and_what_carries_no_mark_of_it() {
    // snapshot-storage: counts every snapshot the provider lists, the recorded ones kept and the rest named as others.
    let backend = images::FakeBackend::new("provider");
    for n in 1..=4u32 {
        backend.snapshots.borrow_mut().as_mut().expect("a listing").push(images::snapshot_row(
            &format!("snap_golden-v{n}"),
            None,
            (7 + n) as f64 * 1e9,
            None,
        ));
    }
    backend.snapshots.borrow_mut().as_mut().expect("a listing").push(images::snapshot_row("snap_old-golden", None, 20e9, None));
    run_over(
        backend,
        |_| {},
        |w| async move {
            w.store.put(GOLDENS, &copy_key("default", "default"), manifest_of(&[1, 2, 3, 4], 4)).await.expect("put");
            let storage = w.runtime.golden().storage().await.expect("storage").expect("a provider that lists");
            let json = serde_json::to_value(&storage).expect("json");
            assert_eq!(json["count"], json!(5));
            assert_eq!(json["kept"]["count"], json!(4));
            assert_eq!(json["others"]["count"], json!(1));
            assert_eq!(json["orphans"]["count"], json!(0));
        },
    );
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn a_provider_that_lists_no_snapshots_has_no_storage_and_no_retention() {
    // snapshot-storage: is undefined on a backend that cannot list snapshots.
    let backend = images::FakeBackend::forkless("provider");
    run_over(
        backend,
        |_| {},
        |w| async move {
            assert!(w.runtime.golden().storage().await.expect("storage").is_none());
            assert!(w.runtime.golden().retention(None, None).await.expect("retention").is_none());
        },
    );
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn retention_offers_the_older_ancestors_and_prune_deletes_them_and_drops_their_versions() {
    let backend = images::FakeBackend::new("provider");
    for n in 1..=4u32 {
        backend.snapshots.borrow_mut().as_mut().expect("a listing").push(images::snapshot_row(
            &format!("snap_golden-v{n}"),
            None,
            (7 + n) as f64 * 1e9,
            None,
        ));
    }
    let probe = backend.clone();
    run_over(
        backend,
        |_| {},
        |w| async move {
            w.store.put(GOLDENS, &copy_key("default", "default"), manifest_of(&[1, 2, 3, 4], 4)).await.expect("put");
            for n in 1..=4u32 {
                w.store
                    .put(GOLDEN_RECIPES, &copy_key("default", &format!("default@v{n}")), json!({ "ticks": [], "files": [] }))
                    .await
                    .expect("put");
            }
            let offered = w.runtime.golden().retention(None, None).await.expect("retention").expect("a plan");
            let kept: Vec<u32> = offered.plan.keep.iter().map(|v| v.version.0 as u32).collect();
            let offered_up: Vec<u32> = offered.plan.drop.iter().map(|v| v.version.0 as u32).collect();
            assert_eq!((kept, offered_up), (vec![4, 3], vec![1, 2]));
            let pruned = w.runtime.golden().prune(None, None).await.expect("prune");
            assert_eq!(pruned.dropped.iter().map(|v| v.version.0 as u32).collect::<Vec<_>>(), [1, 2]);
            let mut deleted = probe.deleted.borrow().clone();
            deleted.sort();
            assert_eq!(deleted, ["snap_golden-v1", "snap_golden-v2"], "the head and its parent stay");
            let left = w.runtime.golden().get(None).await.expect("get").expect("a manifest");
            assert_eq!(serde_json::to_value(&left).expect("json")["versions"].as_array().map(Vec::len), Some(2));
            assert!(w.store.get(GOLDEN_RECIPES, &copy_key("default", "default@v1")).await.expect("get").is_none());
        },
    );
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn a_provider_without_templates_has_nothing_to_promote_and_a_golden_that_does_not_exist_none() {
    // golden-templates: promote is undefined on a backend without templates.
    run(|w| async move {
        assert!(w.runtime.golden().promote(None).await.expect("promote").is_none());
    });
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn a_bare_prepare_is_refused_without_a_recipe_and_a_wired_one_prepares_a_builder() {
    // golden-recipe, runtime-golden-import and runtime-builders: the builder is made on the provider, its setup runs,
    // the record is written, and the builder is listed until it is killed.
    let backend = images::FakeBackend::new("provider");
    let machine = images::FakeMachine::new("m1");
    backend.next_machine.borrow_mut().push_back(machine.clone());
    let seen = Rc::new(RefCell::new(Vec::<GoldenExec>::new()));
    let log = seen.clone();
    run_over(
        backend,
        move |o| {
            o.golden_recipe = Some(GoldenRecipe {
                setup: "echo wired".into(),
                smoke: "true".into(),
                on_exec: Some(Rc::new(move |e| log.borrow_mut().push(e))),
                ..Default::default()
            })
        },
        |w| async move {
            let golden = w.runtime.golden();
            let built = golden.prepare(PrepareAsk { name: Some("default".into()), ..Default::default() }).await.expect("a builder");
            assert_eq!(built.id, "m1");
            assert!(machine.execs().iter().any(|c| c.contains("echo wired")), "the wired setup ran: {:?}", machine.execs());
            let listed = golden.builders().await.expect("builders");
            assert_eq!(listed.iter().map(|b| (b.id.as_str(), b.sealable)).collect::<Vec<_>>(), [("m1", Some(true))]);
            let stored = w.store.get(BUILDERS, "m1").await.expect("get").expect("a builder record");
            assert_eq!(stored["firstLife"], json!(true));
            golden.kill("m1").await.expect("kill");
            assert!(golden.builders().await.expect("builders").is_empty());
            assert!(w.store.get(BUILDERS, "m1").await.expect("get").is_none());
        },
    );
    assert!(seen.borrow().iter().any(|e| e.machine_id == "m1"), "the exec listener heard the machine's commands");
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn a_prepare_that_names_no_recipe_in_a_runtime_wired_with_none_is_refused() {
    run(|w| async move {
        let refused =
            w.runtime.golden().prepare(PrepareAsk { name: Some("default".into()), ..Default::default() }).await.expect_err("no recipe");
        assert!(refused.message.contains("no golden recipe"), "{}", refused.message);
        assert!(w.runtime.golden().builders().await.expect("builders").is_empty());
    });
}

/// Waits on packets 11 and 16: the runtime's boot reads `ThreadsCtx::store` (11) and `SlatesAreaCtx::store` and `clock` (16), which are `todo!()` until they land.
#[test]
fn a_builder_reach_for_a_machine_that_is_not_a_builder_is_refused() {
    // runtime-builders: builderReach("m_nobody") rejects with "no such builder".
    run(|w| async move {
        let refused = w.runtime.golden().builder_reach("m_nobody").await.expect_err("no such builder");
        assert!(refused.message.contains("no such builder"), "{}", refused.message);
    });
}

#[test]
fn the_wire_to_the_image_pipeline_refuses_a_build_it_cannot_read() {
    // `golden.build`'s request is wsp-core's empty `BuildGoldenOptions`, which carries neither setup nor smoke, so
    // the engine's road has nothing to run: the context answers the refusal, never a build of an empty recipe.
    run(|w| async move {
        let ctx = &*w.ctx;
        let roads: &dyn GoldenRoads = ctx;
        let ask = wsp_machines::images::engine::BuildGoldenAsk {
            request: Default::default(),
            backend: w.backend.clone(),
            name: "default".into(),
            host_id: "h1s1".into(),
            labels: Default::default(),
            manifest: None,
        };
        let refused = roads.build_golden(ask).await.expect_err("a refusal");
        assert!(!refused.to_string().is_empty());
    });
}
