// SPDX-License-Identifier: AGPL-3.0-only
//! Spec boot_load.md: every scene of `boot_builders.json`, `boot_hydrate.json` and `boot_reread.json`, replayed through
//! the fake runtime of `support/boot.rs` and checked against what the oracle logged, answered and left.
mod common;
#[path = "support/boot.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_core::types::internal::{BuilderLife, BuilderRecord};
use wsp_core::types::pending::Sighting;
use wsp_core::types::wiring::FoundMachine;
use wsp_machines::machines::boot::load::{Asked, LoadReads};
use wsp_core::types::internal::LiveBuilder;
use wsp_core::types::pending::Builder as EngineBuilder;
use wsp_wire::views::golden_image::MachineKind;
use wsp_words::words::thread::GoneSeenBy;

use common::T0;
use support::{asked_err, asked_missing, entry_id, ws, FakeMachine, Opts, Sight, World};

fn builder(id: &str, o: Value) -> Value {
    let mut b = json!({"id": id, "name": format!("b-{id}"), "kind": "sandbox", "baseTemplate": "base", "setupSha": "sha", "createdAt": "2026-09-01T00:00:00.000Z", "firstLife": true});
    for (k, x) in o.as_object().into_iter().flatten() {
        b[k] = x.clone();
    }
    b
}

fn held_builder(record: Value, life: BuilderLife, sealable: bool) -> wsp_machines::images::BuilderRef {
    let mut record = record;
    if record.get("size").is_none() {
        record["size"] = json!({"cpu": 1, "memMb": 1});
    }
    let id = record["id"].as_str().unwrap().to_string();
    let record: BuilderRecord = serde_json::from_value(record).expect("builder record");
    let builder = EngineBuilder {
        machine: FakeMachine::plain(&id),
        kind: MachineKind::Sandbox,
        base_template: record.base_template.clone(),
        setup_sha: record.setup_sha.clone(),
        created_at: record.created_at.clone(),
        first_life: record.first_life,
        size: record.size.clone(),
        import: None,
        parent_snapshot_id: None,
        base: None,
        retired: None,
    };
    std::rc::Rc::new(std::cell::RefCell::new(LiveBuilder { record, builder, sealable, life, reach: None, recipe: None }))
}

/// The builders the scene's oracle left, in map order: id, life, and the sealable, firstLife and size it recorded.
fn assert_builders(scene: &Value, w: &World) {
    let held = w.rt.builders.borrow();
    let want = scene["builders"].as_array().expect("builders");
    let ids: Vec<&str> = held.keys().map(String::as_str).collect();
    let want_ids: Vec<&str> = want.iter().map(|b| b["id"].as_str().unwrap()).collect();
    assert_eq!(ids, want_ids, "scene {:?}: the builders held", scene["name"]);
    for (b, (_, got)) in want.iter().zip(held.iter()) {
        let got = got.borrow();
        let id = &b["id"];
        let life = match got.life {
            BuilderLife::Own => "own",
            BuilderLife::Reusable => "reusable",
            BuilderLife::Stale => "stale",
            BuilderLife::Foreign => "foreign",
            BuilderLife::Held => "held",
        };
        assert_eq!(json!(life), b["life"], "builder {id}: life");
        assert_eq!(json!(got.record.first_life), b["firstLife"], "builder {id}: firstLife");
        if b.get("sealable").is_some() {
            assert_eq!(json!(got.sealable), b["sealable"], "builder {id}: sealable");
        }
        if b.get("size").is_some() {
            assert_eq!(support::to_json(&got.record.size), b["size"], "builder {id}: size");
        }
    }
}

fn assert_live(scene: &Value, w: &World) {
    let live: Vec<String> = w.rt.live.borrow().keys().cloned().collect();
    let want: Vec<String> = scene["live"].as_array().expect("live").iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(live, want, "scene {:?}: the live workspaces", scene["name"]);
}

fn builders(name: &'static str) {
    let fx = common::fixture("boot_builders");
    let scene = common::scene(&fx, name).clone();
    common::run_local(async move {
        let mut o = Opts { store: scene["store"].clone(), answers: scene["answers"].clone(), ..Opts::default() };
        match name {
            "admit: a provider whose copy is the disk as it stands seals any life" => o.snapshots_any_life = true,
            "admit: unreadable places leave the record, a confirmed gone drops it, another error is thrown" => {
                o.no_backend_at = vec!["away"];
                o.gone = vec!["gone", "running"];
            }
            _ => {}
        }
        let w = World::load(o);
        match name {
            "admit: unreadable places leave the record, a confirmed gone drops it, another error is thrown" => {
                if let Err(e) = w.area.refresh_builders().await {
                    w.log.push(json!(["rejected", e.message]));
                }
            }
            "refresh: own builders stay, others re-derive, dropped rows go" => {
                let mut held = w.rt.builders.borrow_mut();
                held.insert("own1".into(), held_builder(builder("own1", json!({})), BuilderLife::Own, false));
                held.insert("gone1".into(), held_builder(builder("gone1", json!({})), BuilderLife::Reusable, false));
                held.insert("a".into(), held_builder(builder("a", json!({"size": {"cpu": 9, "memMb": 9}})), BuilderLife::Reusable, true));
                drop(held);
                w.area.refresh_builders().await.expect("refresh");
            }
            "refresh: two admits of one row share one read" => {
                let (a, b) = (w.area.clone(), w.area.clone());
                let first = tokio::task::spawn_local(async move { a.refresh_builders().await });
                let second = tokio::task::spawn_local(async move { b.refresh_builders().await });
                common::flush().await;
                support::release_one();
                first.await.unwrap().expect("first pass");
                second.await.unwrap().expect("second pass");
            }
            "admit: a host closed while the read was out admits nothing" => {
                let a = w.area.clone();
                let pass = tokio::task::spawn_local(async move { a.refresh_builders().await });
                common::flush().await;
                w.rt.closed.set(true);
                support::release_one();
                pass.await.unwrap().expect("pass");
            }
            _ => w.area.refresh_builders().await.expect("refresh"),
        }
        common::flush().await;
        common::assert_log(&scene, &w.log.take());
        assert_builders(&scene, &w);
        assert_live(&scene, &w);
    });
}

/// The record each hydrate scene's oracle stored.
fn stored_of(name: &str) -> Value {
    if let Some(rest) = name.strip_prefix("phase ") {
        let phase = rest.split(',').next().unwrap();
        return ws("a", if phase == "gone" { json!({"phase": phase, "gone": "kept words"}) } else { json!({"phase": phase}) });
    }
    match name {
        "a queued spawn waits as it was" => ws("a", json!({"phase": "queued"})),
        "a released record stands on no machine" => ws("a", json!({"phase": "released"})),
        "a sub whose cloud left is not read" => ws("a", json!({"sub": {"cloudGone": true}})),
        "a kind with no module is left as it was" => ws("a", json!({"kind": "unwired"})),
        "a desktop golden's stand-in is a desktop" => ws("a", json!({"golden": "desktop-img"})),
        "a folder record names no image and reads none" => ws("a", json!({"golden": ""})),
        "an image that does not read is a sandbox" => ws("a", json!({"golden": "bad"})),
        "a nap the provider read moves nappedAt and writes once" => ws("a", json!({"phase": "napping", "nappedAt": 5})),
        "the same nappedAt writes nothing" => ws("a", json!({"phase": "napping", "nappedAt": T0 - 1000})),
        "a nap clears the disk-gone line a wake left" => ws("a", json!({"phase": "running", "diskGone": "deleted"})),
        "a stream url rides the record" => ws("a", json!({})),
        "a running record that napped or woke is unchecked" => ws("a", json!({"phase": "waking"})),
        "a found machine is read once from the sighting" => ws("a", json!({"phase": "napping"})),
        "the load's deadline holds a late read and loads it when it lands" => ws("a", json!({})),
        other => panic!("no stored record for scene {other:?}"),
    }
}

fn hydrate(name: &'static str) {
    let fx = common::fixture("boot_hydrate");
    let scene = common::scene(&fx, name).clone();
    common::run_local(async move {
        let stored = stored_of(name);
        let w = World::load(Opts { store: scene["store"].clone(), answers: scene["answers"].clone(), ..Opts::default() });
        let mut answered: Option<Option<String>> = None;
        match name {
            "a found machine is read once from the sighting" => {
                let found = FoundMachine { sighting: Sighting::default(), machine: FakeMachine::without_kind("m-a", "running") };
                let got = w.area.hydrate_workspace(stored.clone(), Some(found), None, None).await.expect("hydrate");
                answered = Some(got.as_ref().map(entry_id));
            }
            "the load's deadline holds a late read and loads it when it lands" => {
                let load = Rc::new(LoadReads { until: T0 + 1500, late: RefCell::new(IndexMap::new()) });
                let area = w.area.clone();
                let raw = stored.clone();
                let out = tokio::task::spawn_local(async move { area.hydrate_workspace(raw, None, Some(load), None).await });
                common::flush().await;
                w.clock.advance(1499);
                common::flush().await;
                w.log.push(json!(["mark", "at 1499"]));
                w.clock.advance(1);
                let first = out.await.unwrap().expect("hydrate");
                w.log.push(json!(["answered", if first.is_none() { Value::Null } else { json!("entry") }, "isHeldAway", w.area.is_held_away("a"), "isHeldAway of none", w.area.is_held_away("none")]));
            }
            _ => {
                let got = w.area.hydrate_workspace(stored.clone(), None, None, None).await.expect("hydrate");
                answered = Some(got.as_ref().map(entry_id));
            }
        }
        common::flush().await;
        common::assert_log(&scene, &w.log.take());
        if let Some(got) = answered {
            let want = scene["answer"]["entry"].as_str().map(str::to_string);
            assert_eq!(got, want, "scene {:?}: the entry answered", scene["name"]);
            if let Some(id) = &want {
                let entry = w.rt.live.borrow().get(id).cloned().expect("answered entry is live");
                let unchecked = matches!(stored["phase"].as_str(), Some("napping" | "waking"));
                assert_eq!(entry.borrow().unchecked, unchecked, "scene {:?}: unchecked", scene["name"]);
            }
        }
        assert_live(&scene, &w);
        assert!(w.rt.builders.borrow().is_empty());
    });
}

fn held_read(w: &World, asked: Asked, record: &Value) -> LocalBoxFutureOf {
    let area = w.area.clone();
    let raw = record.clone();
    Box::pin(async move {
        area.hydrate_workspace(raw, None, None, Some(asked)).await.expect("hydrate");
    })
}

type LocalBoxFutureOf = futures::future::LocalBoxFuture<'static, ()>;

fn reread(name: &'static str) {
    let fx = common::fixture("boot_reread");
    let scene = common::scene(&fx, name).clone();
    common::run_local(async move {
        let record = ws("a", json!({}));
        let mut o = Opts { store: scene["store"].clone(), answers: scene["answers"].clone(), sight_word: Some("sightMachine"), sight_first: true, ..Opts::default() };
        match name {
            "a held record the provider finds is loaded onto and synced" => o.sights = vec![Sight::Found { machine: "m-a".into(), state: "running".into() }],
            "a held record read gone settles gone with the load's answer" => o.sights = vec![Sight::Gone],
            "a sighting that fails leaves it held" => o.sights = vec![Sight::Fails("timeout".into())],
            _ => {}
        }
        let w = World::load(o);
        let keep = |w: &World| {
            w.rt.mem.rows.borrow_mut().entry("workspaces".to_string()).or_default().insert("a".to_string(), support::fill_workspace(&record));
        };
        match name {
            "a held record the provider finds is loaded onto and synced" => {
                keep(&w);
                held_read(&w, asked_err("503 busy"), &record).await;
                w.log.push(json!(["mark", "reread"]));
                w.area.reread_held("a", GoneSeenBy::Sweep).await.expect("reread");
            }
            "a held record read gone settles gone with the load's answer" => {
                keep(&w);
                held_read(&w, asked_missing("404 no such machine"), &record).await;
                w.log.push(json!(["mark", "reread"]));
                w.area.reread_held("a", GoneSeenBy::RecordLoad).await.expect("reread");
                w.log.push(json!(["isHeldAway after", w.area.is_held_away("a")]));
            }
            "a sighting that fails leaves it held" => {
                keep(&w);
                held_read(&w, asked_err("503 busy"), &record).await;
                w.area.reread_held("a", GoneSeenBy::Sweep).await.expect("reread");
                w.log.push(json!(["isHeldAway after", w.area.is_held_away("a")]));
            }
            "a record not held, or being reread, is not read" => {
                let entry = w.rt.add_plain_live("a", &record);
                let _ = entry;
                w.area.reread_held("a", GoneSeenBy::Sweep).await.expect("reread");
                w.area.reread_held("none", GoneSeenBy::Sweep).await.expect("reread");
            }
            other => panic!("no act for scene {other:?}"),
        }
        common::flush().await;
        common::assert_log(&scene, &w.log.take());
        assert_live(&scene, &w);
    });
}

#[test]
fn admit_every_life_a_stored_builder_can_read_as() {
    builders("admit: every life a stored builder can read as");
}

#[test]
fn admit_a_provider_whose_copy_is_the_disk_as_it_stands_seals_any_life() {
    builders("admit: a provider whose copy is the disk as it stands seals any life");
}

#[test]
fn admit_unreadable_places_leave_the_record_a_confirmed_gone_drops_it_another_error() {
    builders("admit: unreadable places leave the record, a confirmed gone drops it, another error is thrown");
}

#[test]
fn refresh_own_builders_stay_others_re_derive_dropped_rows_go() {
    builders("refresh: own builders stay, others re-derive, dropped rows go");
}

#[test]
fn refresh_two_admits_of_one_row_share_one_read() {
    builders("refresh: two admits of one row share one read");
}

#[test]
fn admit_a_host_closed_while_the_read_was_out_admits_nothing() {
    builders("admit: a host closed while the read was out admits nothing");
}

#[test]
fn phase_running_provider_running() {
    hydrate("phase running, provider running");
}

#[test]
fn phase_running_provider_paused() {
    hydrate("phase running, provider paused");
}

#[test]
fn phase_running_provider_starting() {
    hydrate("phase running, provider starting");
}

#[test]
fn phase_running_provider_seen_paused() {
    hydrate("phase running, provider seen-paused");
}

#[test]
fn phase_running_provider_404() {
    hydrate("phase running, provider 404");
}

#[test]
fn phase_running_provider_refused() {
    hydrate("phase running, provider refused");
}

#[test]
fn phase_running_provider_state_fails() {
    hydrate("phase running, provider state-fails");
}

#[test]
fn phase_napping_provider_running() {
    hydrate("phase napping, provider running");
}

#[test]
fn phase_napping_provider_paused() {
    hydrate("phase napping, provider paused");
}

#[test]
fn phase_napping_provider_starting() {
    hydrate("phase napping, provider starting");
}

#[test]
fn phase_napping_provider_seen_paused() {
    hydrate("phase napping, provider seen-paused");
}

#[test]
fn phase_napping_provider_404() {
    hydrate("phase napping, provider 404");
}

#[test]
fn phase_napping_provider_refused() {
    hydrate("phase napping, provider refused");
}

#[test]
fn phase_napping_provider_state_fails() {
    hydrate("phase napping, provider state-fails");
}

#[test]
fn phase_pausing_provider_running() {
    hydrate("phase pausing, provider running");
}

#[test]
fn phase_pausing_provider_paused() {
    hydrate("phase pausing, provider paused");
}

#[test]
fn phase_pausing_provider_starting() {
    hydrate("phase pausing, provider starting");
}

#[test]
fn phase_pausing_provider_seen_paused() {
    hydrate("phase pausing, provider seen-paused");
}

#[test]
fn phase_pausing_provider_404() {
    hydrate("phase pausing, provider 404");
}

#[test]
fn phase_pausing_provider_refused() {
    hydrate("phase pausing, provider refused");
}

#[test]
fn phase_pausing_provider_state_fails() {
    hydrate("phase pausing, provider state-fails");
}

#[test]
fn phase_waking_provider_running() {
    hydrate("phase waking, provider running");
}

#[test]
fn phase_waking_provider_paused() {
    hydrate("phase waking, provider paused");
}

#[test]
fn phase_waking_provider_starting() {
    hydrate("phase waking, provider starting");
}

#[test]
fn phase_waking_provider_seen_paused() {
    hydrate("phase waking, provider seen-paused");
}

#[test]
fn phase_waking_provider_404() {
    hydrate("phase waking, provider 404");
}

#[test]
fn phase_waking_provider_refused() {
    hydrate("phase waking, provider refused");
}

#[test]
fn phase_waking_provider_state_fails() {
    hydrate("phase waking, provider state-fails");
}

#[test]
fn phase_gone_provider_running() {
    hydrate("phase gone, provider running");
}

#[test]
fn phase_gone_provider_paused() {
    hydrate("phase gone, provider paused");
}

#[test]
fn phase_gone_provider_starting() {
    hydrate("phase gone, provider starting");
}

#[test]
fn phase_gone_provider_seen_paused() {
    hydrate("phase gone, provider seen-paused");
}

#[test]
fn phase_gone_provider_404() {
    hydrate("phase gone, provider 404");
}

#[test]
fn phase_gone_provider_refused() {
    hydrate("phase gone, provider refused");
}

#[test]
fn phase_gone_provider_state_fails() {
    hydrate("phase gone, provider state-fails");
}

#[test]
fn a_queued_spawn_waits_as_it_was() {
    hydrate("a queued spawn waits as it was");
}

#[test]
fn a_released_record_stands_on_no_machine() {
    hydrate("a released record stands on no machine");
}

#[test]
fn a_sub_whose_cloud_left_is_not_read() {
    hydrate("a sub whose cloud left is not read");
}

#[test]
fn a_kind_with_no_module_is_left_as_it_was() {
    hydrate("a kind with no module is left as it was");
}

#[test]
fn a_desktop_golden_s_stand_in_is_a_desktop() {
    hydrate("a desktop golden's stand-in is a desktop");
}

#[test]
fn a_folder_record_names_no_image_and_reads_none() {
    hydrate("a folder record names no image and reads none");
}

#[test]
fn an_image_that_does_not_read_is_a_sandbox() {
    hydrate("an image that does not read is a sandbox");
}

#[test]
fn a_nap_the_provider_read_moves_nappedat_and_writes_once() {
    hydrate("a nap the provider read moves nappedAt and writes once");
}

#[test]
fn the_same_nappedat_writes_nothing() {
    hydrate("the same nappedAt writes nothing");
}

#[test]
fn a_nap_clears_the_disk_gone_line_a_wake_left() {
    hydrate("a nap clears the disk-gone line a wake left");
}

#[test]
fn a_stream_url_rides_the_record() {
    hydrate("a stream url rides the record");
}

#[test]
fn a_running_record_that_napped_or_woke_is_unchecked() {
    hydrate("a running record that napped or woke is unchecked");
}

#[test]
fn a_found_machine_is_read_once_from_the_sighting() {
    hydrate("a found machine is read once from the sighting");
}

#[test]
fn the_load_s_deadline_holds_a_late_read_and_loads_it_when_it_lands() {
    hydrate("the load's deadline holds a late read and loads it when it lands");
}

#[test]
fn a_held_record_the_provider_finds_is_loaded_onto_and_synced() {
    reread("a held record the provider finds is loaded onto and synced");
}

#[test]
fn a_held_record_read_gone_settles_gone_with_the_load_s_answer() {
    reread("a held record read gone settles gone with the load's answer");
}

#[test]
fn a_sighting_that_fails_leaves_it_held() {
    reread("a sighting that fails leaves it held");
}

#[test]
fn a_record_not_held_or_being_reread_is_not_read() {
    reread("a record not held, or being reread, is not read");
}

