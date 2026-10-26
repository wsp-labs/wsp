// SPDX-License-Identifier: AGPL-3.0-only
//! Red tests for `images/golden/upgrade` (`spec/golden_upgrade.md`): the update on a kept builder or a fork, the
//! builder's reach and listing, and the kill. Ports of the cases of runtime-golden-update.test.ts,
//! runtime-golden-import.test.ts and runtime-builders.test.ts that reach these roads, at the seam of the shared
//! `World`: every command, stage, road and context call lands in `w.log` in order, as one line. The record's import is
//! read through `entry.builder.import` (wsp-core's `BuilderRecord.import` is an empty stand-in).
#![allow(clippy::await_holding_refcell_ref)]

mod images_support;
use images_support::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};

use wsp_core::clock::Cancel;
use wsp_core::types::internal::{BuilderLife, Sealed};
use wsp_core::types::wiring::{GoldenRecipe, GoldenUpgradeResult, UpgradeRoad};
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_engine::orphans::ReapFailure;
use wsp_machines::images::builders::life::Expired;
use wsp_machines::images::engine::{GoldenDelta, SealResult};
use wsp_machines::images::golden::upgrade::UpgradeRequest;
use wsp_machines::images::golden::GoldenArea;
use wsp_machines::images::{BuilderRef, ImageError};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{
    GoldenBaseTool, GoldenLeftBehind, GoldenLogin, GoldenManifest, GoldenMissingTool, GoldenRetired,
};
use wsp_wire::views::workspace::DaemonReachView;
use wsp_wire::wire::machine_link::PreviewReach;

const IN_WINDOW: &str = "2026-10-25T08:55:00.000Z";
const OUT_WINDOW: &str = "2026-10-25T08:40:00.000Z";

fn said(e: &ImageError) -> String {
    match e {
        ImageError::Host(h) => h.message.clone(),
        ImageError::Engine(e) => e.to_string(),
        ImageError::SnapshotFailed { message, .. } => message.clone(),
        ImageError::Stopped(_) => "stopped".into(),
    }
}

fn other(message: &str) -> EngineError {
    EngineError::Other {
        message: message.into(),
        code: None,
    }
}

fn area(w: &Rc<World>) -> Rc<GoldenArea> {
    GoldenArea::new(w.clone())
}

fn retired() -> Vec<GoldenRetired> {
    vec![GoldenRetired {
        id: "shell/bashrc".into(),
        name: "~/.bashrc".into(),
    }]
}

fn ask() -> UpgradeRequest {
    UpgradeRequest {
        recipe: Some(recipe("true")),
        delta: GoldenDelta {
            retired_on_image: retired(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A world with one sealed version of `default` at the wired provider.
fn head_world() -> Rc<World> {
    let w = World::new();
    w.put_copy_now("boat", "default", &manifest(&[(1, "snap_v1", None)]));
    w.put_recipe_now("boat", "default", 1);
    w
}

fn two_versions() -> GoldenManifest {
    manifest(&[(1, "snap_v1", None), (2, "snap_v2", None)])
}

fn sealed_two(builder_kept: bool) -> SealResult {
    SealResult {
        manifest: two_versions(),
        version: version(2, "snap_v2", None),
        builder_kept,
        vault: None,
    }
}

/// What the seal answers, and the fork the update would make where no builder was kept.
fn fork_world() -> Rc<World> {
    let w = head_world();
    w.roads
        .upgraded
        .borrow_mut()
        .push_back(Ok(engine_builder(FakeMachine::new("f1"))));
    w.seals.borrow_mut().push_back(Ok(sealed_two(true)));
    w
}

fn kept_world() -> (Rc<World>, BuilderRef, Arc<FakeMachine>) {
    let w = head_world();
    w.seals.borrow_mut().push_back(Ok(sealed_two(true)));
    let (b, m) = kept(&w, "b1", BuilderLife::Reusable, IN_WINDOW, 1);
    (w, b, m)
}

/// A builder kept since a seal of `version` at `at`, its machine answering exec as the test scripts.
fn kept(
    w: &World,
    id: &str,
    life: BuilderLife,
    at: &str,
    version: u32,
) -> (BuilderRef, Arc<FakeMachine>) {
    let m = FakeMachine::new(id);
    let b = live_builder(id, "default", "boat", life);
    {
        let mut x = b.borrow_mut();
        x.builder = engine_builder(m.clone());
        x.record.sealed = Some(Sealed {
            at: at.into(),
            version,
        });
    }
    w.add_builder(b.clone());
    (b, m)
}

fn timer(w: &World, id: &str) -> Rc<Cell<bool>> {
    let cancelled = Rc::new(Cell::new(false));
    let c = cancelled.clone();
    let cancel: Cancel = Rc::new(move || c.set(true));
    w.grace_timers.borrow_mut().insert(id.into(), cancel);
    cancelled
}

const FORK_FLOW: [&str; 5] = [
    "claim builder/default @boat",
    "recording_creates default boat import=h1 stop=false",
    "road upgrade_builder",
    "settle_builder default f1 boat",
    "seal_entry f1 keep=true logins=0 copy=",
];

const OPENING: [&str; 4] = [
    "ready",
    "stage_of default boat false",
    "expire_grace",
    "refresh_builders",
];

fn flow(w: &World, tail: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = OPENING.iter().map(|s| s.to_string()).collect();
    v.extend(tail.iter().map(|s| s.to_string()));
    let _ = w;
    v
}

// U1: golden.ts:293-302, runtime-golden-update.test.ts:884
#[test]
fn upgrade_refuses_without_a_head() {
    run(async {
        let w = World::new();
        let err = area(&w).upgrade(ask()).await.unwrap_err();
        assert_eq!(said(&err), "no golden named \"default\" to update");
        assert!(matches!(&err, ImageError::Host(h) if h.kind.is_none()));
        // Refused before anything boots: not even the stage listener is made.
        assert_eq!(w.log(), ["ready"]);
        let err = area(&w)
            .upgrade(UpgradeRequest {
                name: Some("other".into()),
                ..ask()
            })
            .await
            .unwrap_err();
        assert_eq!(said(&err), "no golden named \"other\" to update");
        // A copy with no head is none.
        w.store
            .seed("goldens", "boat/empty", json!({"head": 1, "versions": []}));
        let err = area(&w)
            .upgrade(UpgradeRequest {
                name: Some("empty".into()),
                ..ask()
            })
            .await
            .unwrap_err();
        assert_eq!(said(&err), "no golden named \"empty\" to update");
        assert!(w.lines("expire_grace").is_empty() && w.lines("claim").is_empty());
    });
}

// U1: golden.ts:293-294
#[test]
fn upgrade_refuses_without_a_recipe_first() {
    run(async {
        let w = World::new();
        let err = area(&w)
            .upgrade(UpgradeRequest {
                recipe: None,
                ..ask()
            })
            .await
            .unwrap_err();
        assert_eq!(said(&err), "this runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run");
        assert_eq!(w.log(), ["ready"]);
    });
}

// U2: golden.ts:308-310, runtime-golden-update.test.ts:575
#[test]
fn upgrade_sweeps_expired_builders_and_stages_each_failure() {
    run(async {
        let w = fork_world();
        *w.expired.borrow_mut() = Some(Ok(Expired {
            reaped: vec![],
            failed: vec![
                ReapFailure {
                    id: Some("b8".into()),
                    message: "could not stop: 502 Bad Gateway; stays recorded, retried next sweep"
                        .into(),
                },
                ReapFailure {
                    id: Some("b9".into()),
                    message: "boom".into(),
                },
            ],
        }));
        area(&w).upgrade(ask()).await.unwrap();
        let log = w.log();
        assert_eq!(
            log[..6],
            [
                "ready",
                "stage_of default boat false",
                "expire_grace",
                "stage creating an earlier kept builder b8: could not stop: 502 Bad Gateway; stays recorded, retried next sweep",
                "stage creating an earlier kept builder b9: boom",
                "refresh_builders"
            ]
        );
        assert_eq!(log[6], "claim builder/default @boat");
    });
}

// U3: golden.ts:311, runtime-golden-update.test.ts:113, :424
#[test]
fn upgrade_takes_a_builder_kept_at_the_heads_version_in_its_window() {
    run(async {
        let w = head_world();
        w.seals.borrow_mut().push_back(Ok(sealed_two(true)));
        // None of these is the one: another process's, another name's, another place's, another version's, none sealed,
        // one a sweep owns.
        let (_, d_foreign) = kept(&w, "d_foreign", BuilderLife::Foreign, IN_WINDOW, 1);
        let (_, d_held) = kept(&w, "d_held", BuilderLife::Held, IN_WINDOW, 1);
        let (_, d_stale) = kept(&w, "d_stale", BuilderLife::Stale, IN_WINDOW, 1);
        let (n, d_name) = kept(&w, "d_name", BuilderLife::Own, IN_WINDOW, 1);
        n.borrow_mut().record.name = "other".into();
        let (p, d_place) = kept(&w, "d_place", BuilderLife::Own, IN_WINDOW, 1);
        p.borrow_mut().record.place = Some("spoo".into());
        let (_, d_version) = kept(&w, "d_version", BuilderLife::Own, IN_WINDOW, 2);
        let (u, d_unsealed) = kept(&w, "d_unsealed", BuilderLife::Own, IN_WINDOW, 1);
        u.borrow_mut().record.sealed = None;
        // The first in map order that qualifies, a record with no place standing for the wired provider's.
        let (f, first) = kept(&w, "b_first", BuilderLife::Reusable, IN_WINDOW, 1);
        f.borrow_mut().record.place = None;
        let (_, second) = kept(&w, "b_second", BuilderLife::Own, IN_WINDOW, 1);
        let got = area(&w).upgrade(ask()).await.unwrap();
        assert_eq!(got.road, UpgradeRoad::Builder);
        for m in [
            &d_foreign,
            &d_held,
            &d_stale,
            &d_name,
            &d_place,
            &d_version,
            &d_unsealed,
            &second,
        ] {
            assert!(m.execs().is_empty(), "{} was touched", m.id);
        }
        assert_eq!(first.execs(), ["true"]);
        assert_eq!(w.lines("road "), ["road apply_delta b_first"]);
        assert!(w.lines("road kill_until_gone").is_empty());
    });
}

// U3: golden.ts:311, runtime-golden-update.test.ts:560
#[test]
fn upgrade_passes_over_a_builder_out_of_its_window() {
    run(async {
        let w = fork_world();
        let (_, m) = kept(&w, "b1", BuilderLife::Own, OUT_WINDOW, 1);
        let got = area(&w).upgrade(ask()).await.unwrap();
        assert_eq!(got.road, UpgradeRoad::Fork);
        // Not even the no-op runs on it; stopping it is the sweep's.
        assert!(m.execs().is_empty());
        assert_eq!(w.log(), flow(&w, &FORK_FLOW));
    });
}

// U3: golden.ts:311, runtime-golden-update.test.ts:424
#[test]
fn a_builder_another_live_process_holds_is_neither_used_nor_stopped() {
    run(async {
        let w = fork_world();
        let (_, m) = kept(&w, "b1", BuilderLife::Held, IN_WINDOW, 1);
        let got = area(&w).upgrade(ask()).await.unwrap();
        assert_eq!(got.road, UpgradeRoad::Fork);
        assert!(m.execs().is_empty());
        assert!(w.lines("road kill_until_gone").is_empty() && w.lines("forget_builder").is_empty());
        assert!(w.builders.borrow().contains_key("b1"));
    });
}

// U4: golden.ts:313-318, runtime-golden-update.test.ts:465
#[test]
fn a_kept_builder_is_marked_building_before_its_first_exec() {
    run(async {
        let (w, b, m) = kept_world();
        let cancelled = timer(&w, "b1");
        // What the record on the store, the timer and the map hold at the moment the first exec arrives.
        let seen: Rc<RefCell<Option<(Option<Value>, bool, bool)>>> = Rc::new(RefCell::new(None));
        let (store, timers, flag, into) = (
            w.store.clone(),
            w.grace_timers.clone(),
            cancelled.clone(),
            seen.clone(),
        );
        *m.on_exec.borrow_mut() = Some(Rc::new(move |cmd: &str| {
            if cmd == "true" {
                *into.borrow_mut() = Some((
                    store.doc("builders", "b1"),
                    flag.get(),
                    timers.borrow().contains_key("b1"),
                ));
            }
        }));
        area(&w).upgrade(ask()).await.unwrap();
        let (doc, cancelled_then, still_armed) = seen.borrow().clone().expect("the no-op ran");
        let doc = doc.expect("the record was written before the exec");
        assert_eq!(doc["building"], json!(true));
        assert!(doc.get("sealed").is_none());
        assert!(cancelled_then && !still_armed);
        assert!(cancelled.get() && !w.grace_timers.borrow().contains_key("b1"));
        // Settled in memory: no longer mid-setup, and the seal re-arms the window.
        let r = b.borrow();
        assert_eq!(r.record.building, None);
        assert!(r.record.sealed.is_none());
    });
}

// U4: golden.ts:319-324, runtime-golden-update.test.ts:544
#[test]
fn a_kept_builder_that_does_not_answer_is_killed_and_the_update_forks() {
    run(async {
        for answer in [Err(other("502 Bad Gateway")), Ok(exec_exit(1, "", "no"))] {
            let (w, _, m) = kept_world();
            w.roads
                .upgraded
                .borrow_mut()
                .push_back(Ok(engine_builder(FakeMachine::new("f1"))));
            m.answer(answer);
            let got = area(&w).upgrade(ask()).await.unwrap();
            assert_eq!(got.road, UpgradeRoad::Fork);
            let mut tail = vec!["road kill_until_gone b1", "forget_builder b1"];
            tail.extend(FORK_FLOW);
            assert_eq!(w.log(), flow(&w, &tail));
            let held: Vec<String> = w.builders.borrow().keys().cloned().collect();
            assert_eq!(held, ["f1"]);
        }
        // A kill that fails fails the update; the builder is not forgotten and nothing forks.
        let (w, _, m) = kept_world();
        m.answer(Err(other("502 Bad Gateway")));
        *w.roads.kill_fails.borrow_mut() = Some(other("kill refused"));
        let err = area(&w).upgrade(ask()).await.unwrap_err();
        assert_eq!(said(&err), "kill refused");
        assert!(w.lines("forget_builder").is_empty() && w.lines("claim").is_empty());
    });
}

// U4: golden.ts:325-343, runtime-golden-update.test.ts:113, :829, :847
#[test]
fn a_kept_builder_takes_the_delta_and_carries_the_heads_snapshot() {
    run(async {
        let (w, b, _) = kept_world();
        // A head sealed with tools missing from its image and files left behind: the update carries both.
        let missing = json!([{"id": "tools/brew-cask/raycast", "name": "Raycast", "outcome": "skipped", "note": "macOS app, no Linux build"}]);
        let left = json!([{"id": "agents/claude", "path": "~/.claude/settings.json", "note": "hook left behind: /opt/homebrew/bin/terminal-notifier"}]);
        let mut head = version(1, "snap_v1", None);
        head.smoke.cmd = "codex --version".into();
        head.base = Some(vec![GoldenBaseTool {
            name: "node".into(),
            version: "22.23.2".into(),
        }]);
        head.missing_tools = Some(serde_json::from_value(missing.clone()).unwrap());
        head.left_behind = Some(serde_json::from_value(left.clone()).unwrap());
        let mut held = manifest(&[(1, "snap_v1", None)]);
        held.versions = vec![head];
        w.put_copy_now("boat", "default", &held);
        let got = area(&w).upgrade(ask()).await.unwrap();
        assert_eq!(
            got,
            GoldenUpgradeResult {
                manifest: two_versions(),
                version: version(2, "snap_v2", None),
                road: UpgradeRoad::Builder,
                previous_dropped: false,
                builder_kept: true
            }
        );
        assert_eq!(
            w.log(),
            flow(
                &w,
                &[
                    "stage creating your builder from v1, kept since the save",
                    "road apply_delta b1",
                    "hold b1",
                    "stage ready ",
                    "seal_entry b1 keep=true logins=0 copy="
                ]
            )
        );
        let asks = w.roads.delta_asks.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].setup, "true");
        assert_eq!(asks[0].previous_smoke, "codex --version");
        assert_eq!(
            asks[0].previous_base,
            Some(vec![GoldenBaseTool {
                name: "node".into(),
                version: "22.23.2".into()
            }])
        );
        assert_eq!(
            asks[0].previous_missing,
            Some(serde_json::from_value::<Vec<GoldenMissingTool>>(missing).unwrap())
        );
        assert_eq!(
            asks[0].previous_left_behind,
            Some(serde_json::from_value::<Vec<GoldenLeftBehind>>(left).unwrap())
        );
        // The record and the builder both: the setup the delta made, the head's snapshot as the parent, what the image
        // retired; the builder sealed as the head, so its next version descends from the head's snapshot.
        let x = b.borrow();
        assert_eq!(x.record.setup_sha, "sha-1+4");
        assert_eq!(x.record.parent_snapshot_id.as_deref(), Some("snap_v1"));
        assert_eq!(x.record.retired, Some(retired()));
        assert_eq!(x.record.building, None);
        assert!(x.record.import.is_some());
        assert_eq!(x.builder.import, Some(ledger("h-delta")));
        assert_eq!(x.builder.setup_sha, "sha-1+4");
        assert_eq!(x.builder.parent_snapshot_id.as_deref(), Some("snap_v1"));
        assert_eq!(x.builder.retired, Some(retired()));
        assert_eq!(x.life, BuilderLife::Own);
        assert_eq!(
            x.recipe.as_ref().map(|r| r.setup.clone()),
            Some("true".to_string())
        );
        drop(x);

        // A head that carries none of them passes none.
        let (w, _, _) = kept_world();
        area(&w).upgrade(ask()).await.unwrap();
        let asks = w.roads.delta_asks.borrow();
        assert_eq!(
            (
                asks[0].previous_smoke.as_str(),
                asks[0].previous_base.clone(),
                asks[0].previous_missing.clone()
            ),
            ("true", None, None)
        );
        assert_eq!(asks[0].previous_left_behind, None);
    });
}

// U4: golden.ts:344-351, runtime-golden-update.test.ts:450
#[test]
fn a_kept_builder_whose_delta_fails_is_killed_and_staged_failed() {
    run(async {
        let (w, _, _) = kept_world();
        let words = "your files need 3.0 GB, and the builder has 1 KB free";
        *w.roads.delta_applied.borrow_mut() = Some(Err(other(words)));
        let err = area(&w).upgrade(ask()).await.unwrap_err();
        assert_eq!(said(&err), words);
        assert_eq!(
            w.log(),
            flow(
                &w,
                &[
                    "stage creating your builder from v1, kept since the save",
                    "road apply_delta b1",
                    "road kill_until_gone b1",
                    "forget_if_gone b1",
                    &format!("stage failed {words}")
                ]
            )
        );
        assert!(w.lines("seal_entry").is_empty() && w.lines("hold").is_empty());

        // A kill that fails too is told on the same stage line, and the update still fails with the delta's words.
        let (w, _, _) = kept_world();
        *w.roads.delta_applied.borrow_mut() = Some(Err(other(words)));
        *w.roads.kill_fails.borrow_mut() = Some(other("502 Bad Gateway"));
        let err = area(&w).upgrade(ask()).await.unwrap_err();
        assert_eq!(said(&err), words);
        assert_eq!(
            w.lines("stage failed"),
            [format!("stage failed {words}; 502 Bad Gateway")]
        );
        assert_eq!(w.lines("forget_if_gone"), ["forget_if_gone b1"]);
    });
}

fn recipe_with_size() -> GoldenRecipe {
    GoldenRecipe {
        cpu: Some(JsNumber::from(4.0)),
        mem_mb: Some(JsNumber::from(8192.0)),
        envs: Some([("A".to_string(), "1".to_string())].into_iter().collect()),
        labels: Some(
            [("team".to_string(), "x".to_string())]
                .into_iter()
                .collect(),
        ),
        ..recipe("true")
    }
}

// U5: golden.ts:352-379, runtime-golden-update.test.ts:339
#[test]
fn the_fork_road_claims_the_bare_builder_key_and_settles() {
    run(async {
        let w = fork_world();
        let got = area(&w)
            .upgrade(UpgradeRequest {
                recipe: Some(recipe_with_size()),
                ..ask()
            })
            .await
            .unwrap();
        assert_eq!(got.road, UpgradeRoad::Fork);
        assert_eq!(w.log(), flow(&w, &FORK_FLOW));
        let asks = w.roads.upgrade_asks.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].head.snapshot_id, "snap_v1");
        assert_eq!(asks[0].setup, "true");
        assert_eq!(asks[0].delta.retired_on_image, retired());
        assert_eq!(asks[0].size.cpu, Some(JsNumber::from(4.0)));
        assert_eq!(asks[0].size.mem_mb, Some(JsNumber::from(8192.0)));
        assert_eq!(
            asks[0].size.envs,
            Some([("A".to_string(), "1".to_string())].into_iter().collect())
        );
        let labels: Vec<_> = asks[0].size.labels.clone().unwrap().into_iter().collect();
        assert_eq!(
            labels,
            [
                ("team".to_string(), "x".to_string()),
                ("wsp-builder".to_string(), "1".to_string())
            ]
        );
        // The recipe is kept on the entry the fork settled as.
        assert_eq!(
            w.builders
                .borrow()
                .get("f1")
                .unwrap()
                .borrow()
                .recipe
                .as_ref()
                .map(|r| r.setup.clone()),
            Some("true".to_string())
        );
        drop(asks);

        // No size in the recipe asks for none; the claim is the bare name at the place the image stands.
        let w = head_world();
        let spoo = FakeBackend::new("spoo");
        w.with_door().computer("spoo", "Spoo", Some(spoo));
        w.put_image_now(&sealed_image("default", 1, "h", Some("spoo")));
        w.put_copy_now("spoo", "default", &manifest(&[(1, "snap_v1", None)]));
        w.roads
            .upgraded
            .borrow_mut()
            .push_back(Ok(engine_builder(FakeMachine::new("f1"))));
        w.seals.borrow_mut().push_back(Ok(sealed_two(true)));
        area(&w).upgrade(ask()).await.unwrap();
        assert_eq!(
            w.log(),
            [
                "ready",
                "stage_of default spoo false",
                "expire_grace",
                "refresh_builders",
                "claim builder/default @spoo",
                "recording_creates default spoo import=h1 stop=false",
                "road upgrade_builder",
                "settle_builder default f1 spoo",
                "seal_entry f1 keep=true logins=0 copy="
            ]
        );
        let asks = w.roads.upgrade_asks.borrow();
        assert_eq!(
            (
                asks[0].size.cpu,
                asks[0].size.mem_mb,
                asks[0].size.envs.clone()
            ),
            (None, None, None)
        );
    });
}

// U5: golden.ts:366-378
#[test]
fn the_fork_road_forgets_a_gone_placeholder_when_it_fails() {
    run(async {
        let w = head_world();
        *w.placeholder_made.borrow_mut() =
            Some(live_builder("p1", "default", "boat", BuilderLife::Own));
        w.roads
            .upgraded
            .borrow_mut()
            .push_back(Err(ImageError::Host(plain_error("the fork was refused"))));
        let err = area(&w).upgrade(ask()).await.unwrap_err();
        assert_eq!(said(&err), "the fork was refused");
        assert_eq!(
            w.log(),
            flow(
                &w,
                &[
                    "claim builder/default @boat",
                    "recording_creates default boat import=h1 stop=false",
                    "road upgrade_builder",
                    "forget_if_gone p1"
                ]
            )
        );
        // No machine was made, so there is no placeholder to forget, and nothing is sealed.
        let w = head_world();
        w.roads
            .upgraded
            .borrow_mut()
            .push_back(Err(ImageError::Host(plain_error("the fork was refused"))));
        area(&w).upgrade(ask()).await.unwrap_err();
        assert!(
            w.lines("forget_if_gone").is_empty()
                && w.lines("settle_builder").is_empty()
                && w.lines("seal_entry").is_empty()
        );
    });
}

fn logins() -> Vec<GoldenLogin> {
    serde_json::from_value(json!([{"name": "GitHub CLI login", "state": "signed-in"}, {"name": "Codex login", "state": "copied"}])).unwrap()
}

// U6: golden.ts:380-386, runtime-golden-update.test.ts:866
#[test]
fn the_update_seals_keeping_the_builder_with_the_logins() {
    run(async {
        // The kept builder's road and the fork's, with the logins and without.
        for given in [Some(logins()), None] {
            let n = given.as_ref().map(|l| l.len()).unwrap_or(0);
            let (w, b, _) = kept_world();
            area(&w)
                .upgrade(UpgradeRequest {
                    logins: given.clone(),
                    ..ask()
                })
                .await
                .unwrap();
            assert_eq!(
                w.lines("seal_entry"),
                [format!("seal_entry b1 keep=true logins={n} copy=")]
            );
            assert_eq!(
                b.borrow().recipe.as_ref().map(|r| r.smoke.clone()),
                Some("true".to_string())
            );

            let w = fork_world();
            area(&w)
                .upgrade(UpgradeRequest {
                    logins: given,
                    ..ask()
                })
                .await
                .unwrap();
            assert_eq!(
                w.lines("seal_entry"),
                [format!("seal_entry f1 keep=true logins={n} copy=")]
            );
        }
    });
}

// U7: golden.ts:389-392, runtime-golden-update.test.ts:385
#[test]
fn keep_previous_false_keeps_a_version_workspaces_stand_on() {
    run(async {
        let (w, _, _) = kept_world();
        w.add_workspace("w_a", "alpha", "snap_v1");
        w.add_workspace("w_b", "beta", "snap_v1");
        w.add_workspace("w_c", "elsewhere", "snap_v9");
        let got = area(&w)
            .upgrade(UpgradeRequest {
                keep_previous: Some(false),
                ..ask()
            })
            .await
            .unwrap();
        assert_eq!(
            w.warned.lines(),
            ["golden default v1 kept: alpha, beta still on it"]
        );
        assert!(!got.previous_dropped);
        assert_eq!(got.manifest, two_versions());
        assert!(
            w.lines("drop_image").is_empty()
                && w.lines("put_copy").is_empty()
                && w.lines("drop_copy_recipe").is_empty()
        );

        // Only a false keepPrevious drops anything, and only a false one warns.
        for keep in [None, Some(true)] {
            let (w, _, _) = kept_world();
            let got = area(&w)
                .upgrade(UpgradeRequest {
                    keep_previous: keep,
                    ..ask()
                })
                .await
                .unwrap();
            assert!(
                !got.previous_dropped
                    && w.warned.lines().is_empty()
                    && w.lines("drop_image").is_empty()
            );
        }
    });
}

// U7: golden.ts:393-413, runtime-golden-update.test.ts:363
#[test]
fn keep_previous_false_drops_the_head_and_its_recipe() {
    run(async {
        let (w, _, _) = kept_world();
        w.put_recipe_now("boat", "default", 2);
        let got = area(&w)
            .upgrade(UpgradeRequest {
                keep_previous: Some(false),
                ..ask()
            })
            .await
            .unwrap();
        let only_two = manifest(&[(2, "snap_v2", None)]);
        assert_eq!(
            got,
            GoldenUpgradeResult {
                manifest: only_two.clone(),
                version: version(2, "snap_v2", None),
                road: UpgradeRoad::Builder,
                previous_dropped: true,
                builder_kept: true
            }
        );
        // The builder road keeps its builder: nothing is killed.
        let log = w.log();
        let at = log
            .iter()
            .position(|l| l.starts_with("seal_entry"))
            .unwrap();
        assert_eq!(
            log[at + 1..],
            [
                "drop_image snap_v1",
                "put_copy boat default",
                "drop_copy_recipe boat default 1"
            ]
        );
        assert_eq!(w.copy_now("boat", "default"), Some(only_two));
        assert!(
            w.recipe_now("boat", "default", 1).is_none()
                && w.recipe_now("boat", "default", 2).is_some()
        );
        assert!(w.warned.lines().is_empty());
    });
}

// U7: golden.ts:395-403, runtime-golden-update.test.ts:409
#[test]
fn keep_previous_false_on_the_fork_road_kills_the_kept_builder_first() {
    run(async {
        let w = fork_world();
        w.put_recipe_now("boat", "default", 2);
        let cancelled = timer(&w, "f1");
        let got = area(&w)
            .upgrade(UpgradeRequest {
                keep_previous: Some(false),
                ..ask()
            })
            .await
            .unwrap();
        assert_eq!(got.road, UpgradeRoad::Fork);
        assert!(got.previous_dropped);
        // A snapshot with forks under it cannot be deleted: the builder forked from it goes first, window or not.
        assert!(!got.builder_kept);
        assert!(cancelled.get() && !w.grace_timers.borrow().contains_key("f1"));
        let log = w.log();
        let at = log
            .iter()
            .position(|l| l.starts_with("seal_entry"))
            .unwrap();
        assert_eq!(
            log[at + 1..],
            [
                "road kill_until_gone f1",
                "forget_builder f1",
                "drop_image snap_v1",
                "put_copy boat default",
                "drop_copy_recipe boat default 1"
            ]
        );
        assert!(!w.builders.borrow().contains_key("f1"));
        assert_eq!(got.manifest, manifest(&[(2, "snap_v2", None)]));

        // A seal that kept no builder leaves nothing to kill.
        let w = head_world();
        w.roads
            .upgraded
            .borrow_mut()
            .push_back(Ok(engine_builder(FakeMachine::new("f1"))));
        w.seals.borrow_mut().push_back(Ok(sealed_two(false)));
        let got = area(&w)
            .upgrade(UpgradeRequest {
                keep_previous: Some(false),
                ..ask()
            })
            .await
            .unwrap();
        assert!(got.previous_dropped && !got.builder_kept);
        assert!(w.lines("road kill_until_gone").is_empty() && w.lines("forget_builder").is_empty());
    });
}

// U7: golden.ts:404-406, runtime-golden-update.test.ts:409
#[test]
fn keep_previous_false_fails_the_update_where_the_builder_will_not_die() {
    run(async {
        let w = fork_world();
        *w.roads.kill_fails.borrow_mut() = Some(other("502 Bad Gateway"));
        let err = area(&w)
            .upgrade(UpgradeRequest {
                keep_previous: Some(false),
                ..ask()
            })
            .await
            .unwrap_err();
        assert_eq!(said(&err), "502 Bad Gateway");
        assert!(w.lines("drop_image").is_empty() && w.lines("forget_builder").is_empty());
        assert!(w.warned.lines().is_empty());
    });
}

// U7: golden.ts:407-412, runtime-golden-update.test.ts:363
#[test]
fn a_failed_drop_warns_and_keeps_the_copy() {
    run(async {
        let (w, _, _) = kept_world();
        w.put_recipe_now("boat", "default", 2);
        w.drop_image_fails.borrow_mut().insert(
            "snap_v1".into(),
            ImageError::Engine(provider_error(
                ErrorKind::Conflict,
                409,
                "SnapshotHasChildren",
            )),
        );
        let got = area(&w)
            .upgrade(UpgradeRequest {
                keep_previous: Some(false),
                ..ask()
            })
            .await
            .unwrap();
        assert_eq!(
            w.warned.lines(),
            ["golden default v1 kept: its snapshot was not deleted (SnapshotHasChildren)"]
        );
        assert!(!got.previous_dropped);
        // The copy as the seal left it, and nothing written after.
        assert_eq!(got.manifest, two_versions());
        assert!(w.lines("put_copy").is_empty() && w.lines("drop_copy_recipe").is_empty());
        assert!(
            w.recipe_now("boat", "default", 1).is_some()
                && w.recipe_now("boat", "default", 2).is_some()
        );
    });
}

// U8: golden.ts:414
#[test]
fn upgrade_answers_in_the_ts_key_order() {
    run(async {
        let (w, _, _) = kept_world();
        let got = area(&w).upgrade(ask()).await.unwrap();
        let v = serde_json::to_value(&got).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "manifest",
                "version",
                "road",
                "previousDropped",
                "builderKept"
            ]
        );
        assert_eq!(
            (&v["road"], &v["previousDropped"], &v["builderKept"]),
            (&json!("builder"), &json!(false), &json!(true))
        );
        assert_eq!(v["version"]["version"], json!(2));
    });
}

fn reach() -> PreviewReach {
    PreviewReach {
        url: "https://m1-7070.preview.example/?pt_token=edge".into(),
        token: "edge".into(),
        expires_at: JsNumber::from(1_800_000_000_000.0),
    }
}

// U9: golden.ts:418-428, runtime-builders.test.ts:106
#[test]
fn builder_reach_refreshes_the_preview_token_and_names_the_daemon_token() {
    run(async {
        let w = World::new();
        let b = live_builder("b1", "default", "boat", BuilderLife::Own);
        w.add_builder(b.clone());
        *w.roads.reach.borrow_mut() = Some(Ok(reach()));
        let got = area(&w).builder_reach("b1").await.unwrap();
        let want: DaemonReachView =
            serde_json::from_value(json!({"url": "https://m1-7070.preview.example/?pt_token=edge", "expiresAt": 1_800_000_000_000.0, "daemonToken": "token-b1"})).unwrap();
        assert_eq!(got, want);
        assert_eq!(
            w.log(),
            [
                "ready",
                "refresh_builders",
                "refuse_untouchable b1",
                "road refresh_preview_token b1 7070"
            ]
        );
        // Kept on the entry, for the next ask to hold.
        assert_eq!(b.borrow().reach, Some(reach()));

        // A refused refresh fails the call and keeps what the entry held.
        let w = World::new();
        let b = live_builder("b1", "default", "boat", BuilderLife::Own);
        b.borrow_mut().reach = Some(reach());
        w.add_builder(b.clone());
        *w.roads.reach.borrow_mut() = Some(Err(other("preview refused")));
        let err = area(&w).builder_reach("b1").await.unwrap_err();
        assert_eq!(said(&err), "preview refused");
        assert_eq!(b.borrow().reach, Some(reach()));
    });
}

// U9: golden.ts:421-423, runtime-builders.test.ts:106
#[test]
fn builder_reach_refuses_unknown_and_untouchable() {
    run(async {
        let w = World::new();
        let err = area(&w).builder_reach("m_nobody").await.unwrap_err();
        assert_eq!(said(&err), "no such builder: m_nobody");
        assert_eq!(w.log(), ["ready", "refresh_builders"]);

        for life in [BuilderLife::Foreign, BuilderLife::Held] {
            let w = World::new();
            w.add_builder(live_builder("b1", "default", "boat", life));
            *w.roads.reach.borrow_mut() = Some(Ok(reach()));
            let err = area(&w).builder_reach("b1").await.unwrap_err();
            assert_eq!(said(&err), "b1 is untouchable");
            assert_eq!(
                w.log(),
                ["ready", "refresh_builders", "refuse_untouchable b1"]
            );
        }
    });
}

// U10: golden.ts:430-434, runtime-golden-update.test.ts:65
#[test]
fn builders_lists_every_view_in_map_order() {
    run(async {
        let w = World::new();
        for (id, name, life) in [
            ("b3", "default", BuilderLife::Own),
            ("b1", "other", BuilderLife::Reusable),
            ("b2", "default", BuilderLife::Foreign),
        ] {
            w.add_builder(live_builder(id, name, "boat", life));
        }
        let got = area(&w).builders().await.unwrap();
        let listed: Vec<(String, String)> =
            got.iter().map(|v| (v.id.clone(), v.name.clone())).collect();
        assert_eq!(
            listed,
            [
                ("b3".to_string(), "default".to_string()),
                ("b1".to_string(), "other".to_string()),
                ("b2".to_string(), "default".to_string())
            ]
        );
        assert_eq!(w.log(), ["ready", "refresh_builders"]);
        // None held: none listed.
        let w = World::new();
        assert!(area(&w).builders().await.unwrap().is_empty());
    });
}

fn labelled(w: &World, id: &str, labels: &[(&str, &str)]) {
    w.runtime_backend
        .machines
        .borrow_mut()
        .insert(id.into(), Ok(FakeMachine::labelled(id, labels)));
}

// U11: golden.ts:444-454, runtime-golden-import.test.ts:194
#[test]
fn kill_takes_an_unrecorded_machine_only_when_it_wears_this_owners_builder_label() {
    run(async {
        for labels in [
            vec![("wsp-owner", "owner-1"), ("wsp-builder", "1")],
            vec![("wsp-owner", "owner-1"), ("wsp-smoke", "1")],
        ] {
            let w = World::new();
            labelled(&w, "m9", &labels);
            area(&w).kill("m9").await.unwrap();
            assert_eq!(
                w.log(),
                ["ready", "refresh_builders", "road kill_until_gone m9"]
            );
        }
        // Another state's machine, a workspace of this one, a builder label that is not "1", a machine with no labels.
        for labels in [
            vec![("wsp-owner", "h_other"), ("wsp-builder", "1")],
            vec![("wsp-owner", "owner-1")],
            vec![("wsp-owner", "owner-1"), ("wsp-builder", "0")],
            vec![("wsp-builder", "1")],
            vec![],
        ] {
            let w = World::new();
            labelled(&w, "m9", &labels);
            let err = area(&w).kill("m9").await.unwrap_err();
            assert_eq!(said(&err), "no such builder: m9", "labels {labels:?}");
            assert!(w.lines("road kill_until_gone").is_empty());
        }
        let w = World::new();
        w.runtime_backend
            .machines
            .borrow_mut()
            .insert("m9".into(), Ok(FakeMachine::new("m9")));
        assert_eq!(
            said(&area(&w).kill("m9").await.unwrap_err()),
            "no such builder: m9"
        );
    });
}

// U11: golden.ts:446-448, runtime-golden-import.test.ts:206
#[test]
fn kill_refuses_a_missing_machine_as_no_such_builder() {
    run(async {
        let w = World::new();
        let err = area(&w).kill("m_gone").await.unwrap_err();
        assert_eq!(said(&err), "no such builder: m_gone");
        assert_eq!(w.log(), ["ready", "refresh_builders"]);
        let w = World::new();
        w.runtime_backend
            .machines
            .borrow_mut()
            .insert("m_gone".into(), Err(missing("machine m_gone is gone")));
        assert_eq!(
            said(&area(&w).kill("m_gone").await.unwrap_err()),
            "no such builder: m_gone"
        );
        assert!(w.lines("road kill_until_gone").is_empty());
    });
}

// U11: golden.ts:446-449
#[test]
fn kill_fails_with_the_providers_own_read_failure() {
    run(async {
        let w = World::new();
        w.runtime_backend.machines.borrow_mut().insert(
            "m5".into(),
            Err(provider_error(ErrorKind::Transient, 502, "502 Bad Gateway")),
        );
        let err = area(&w).kill("m5").await.unwrap_err();
        assert_eq!(said(&err), "502 Bad Gateway");
        assert!(w.lines("road kill_until_gone").is_empty());
    });
}

// U12: golden.ts:456-458, runtime-golden-import.test.ts:194, runtime-golden-update.test.ts:497
#[test]
fn kill_kills_and_forgets_a_recorded_builder_even_while_building() {
    run(async {
        // Own, mid-setup, stale, reusable: each is stopped as the sweep would stop it.
        for life in [BuilderLife::Own, BuilderLife::Reusable, BuilderLife::Stale] {
            let w = World::new();
            let b = live_builder("b1", "default", "boat", life);
            b.borrow_mut().record.building = Some(true);
            w.add_builder(b);
            area(&w).kill("b1").await.unwrap();
            assert_eq!(
                w.log(),
                [
                    "ready",
                    "refresh_builders",
                    "road kill_until_gone b1",
                    "forget_builder b1"
                ]
            );
            assert!(!w.builders.borrow().contains_key("b1"));
        }
        // A record with no place is the wired provider's; one at a place this host has no backend for cannot be reached.
        let w = World::new();
        let b = live_builder("b1", "default", "boat", BuilderLife::Own);
        b.borrow_mut().record.place = None;
        w.add_builder(b);
        area(&w).kill("b1").await.unwrap();
        assert_eq!(w.lines("road kill_until_gone"), ["road kill_until_gone b1"]);
        let w = World::new();
        w.add_builder(live_builder("b1", "default", "ghost", BuilderLife::Own));
        let err = area(&w).kill("b1").await.unwrap_err();
        assert_eq!(said(&err), "no place ghost");
        assert!(
            w.lines("road kill_until_gone").is_empty() && w.builders.borrow().contains_key("b1")
        );
        // A kill that fails keeps the builder recorded.
        let w = World::new();
        w.add_builder(live_builder("b1", "default", "boat", BuilderLife::Own));
        *w.roads.kill_fails.borrow_mut() = Some(other("502 Bad Gateway"));
        assert_eq!(
            said(&area(&w).kill("b1").await.unwrap_err()),
            "502 Bad Gateway"
        );
        assert!(w.lines("forget_builder").is_empty() && w.builders.borrow().contains_key("b1"));
    });
}

// U12: golden.ts:455, runtime-golden-import.test.ts:210
#[test]
fn kill_refuses_a_foreign_or_held_builder() {
    run(async {
        for life in [BuilderLife::Foreign, BuilderLife::Held] {
            let w = World::new();
            w.add_builder(live_builder("b1", "default", "boat", life));
            let err = area(&w).kill("b1").await.unwrap_err();
            assert_eq!(said(&err), "b1 is untouchable");
            assert_eq!(
                w.log(),
                ["ready", "refresh_builders", "refuse_untouchable b1"]
            );
            assert!(w.builders.borrow().contains_key("b1"));
        }
        // Any other life is not asked about.
        let w = World::new();
        w.add_builder(live_builder("b1", "default", "boat", BuilderLife::Own));
        area(&w).kill("b1").await.unwrap();
        assert!(w.lines("refuse_untouchable").is_empty());
    });
}
