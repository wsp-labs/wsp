// SPDX-License-Identifier: AGPL-3.0-only
//! Red tests for `images/golden/storage` (`spec/golden_storage.md`): storage, orphans, delete_orphans, retention_places,
//! retention, prune, rollback and promote. Ports of the cases of snapshot-storage.test.ts, snapshot-owners.test.ts,
//! golden-templates.test.ts, sealed-image.test.ts (retention places, prune by place) and runtime-builders.test.ts
//! (rollback) that reach these roads, at the seam of the shared `World`: every number below was read off the TS
//! `snapshot-storage.ts` run under node over the same rows, none off the Rust helpers.
mod images_support;
use images_support::*;

use std::rc::Rc;

use serde_json::{json, Value};

use wsp_core::types::api::{OrphanFailed, OrphansDeleted};
use wsp_core::types::wiring::{GoldenPromotion, PromotionDone};
use wsp_engine::errors::EngineError;
use wsp_machines::images::engine::Promoted;
use wsp_machines::images::golden::GoldenArea;
use wsp_machines::images::ImageError;
use wsp_wire::views::golden_image::{GoldenManifest, GoldenVersion};
use wsp_wire::views::snapshot_lineage::SnapshotStorage;
use wsp_wire::wire::machine_link::{SnapshotRow, TemplateRow};
use wsp_words::exit::{HostError, Kind};

const OLD: &str = "2026-10-25T08:00:00.000Z";
const GB: f64 = 1e9;

fn said(e: &ImageError) -> String {
    match e {
        ImageError::Host(h) => h.message.clone(),
        ImageError::Engine(e) => e.to_string(),
        ImageError::SnapshotFailed { message, .. } => message.clone(),
        ImageError::Stopped(_) => "stopped".into(),
    }
}

fn storage_of(v: Value) -> SnapshotStorage {
    serde_json::from_value(v).expect("a storage line")
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} is not {b}");
}

fn nums(vs: &[GoldenVersion]) -> Vec<u32> {
    vs.iter().map(|v| v.version.0 as u32).collect()
}

fn ids(rows: &[SnapshotRow]) -> Vec<String> {
    rows.iter().map(|r| r.id.clone()).collect()
}

fn conflict(message: &str) -> EngineError {
    provider_error(wsp_engine::errors::ErrorKind::Conflict, 409, message)
}

fn other(message: &str) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

/// Version `n` as the TS storage tests write it: parent recorded, base recorded, snapshot `snap_<tag>-v<n>`.
fn gv(tag: &str, n: u32) -> Value {
    let mut v = json!({
        "version": n, "snapshotId": format!("snap_{tag}-v{n}"), "baseTemplate": "base", "setupSha": format!("sha{n}"),
        "createdAt": format!("2026-08-{}T00:00:00.000Z", 10 + n), "smoke": {"cmd": "true", "exitCode": 0},
        "base": [{"name": "node", "version": "22.23.2"}]
    });
    if n > 1 {
        v["parentSnapshotId"] = json!(format!("snap_{tag}-v{}", n - 1));
    }
    v
}

fn chain(tag: &str, versions: &[u32], head: u32) -> GoldenManifest {
    serde_json::from_value(json!({"head": head, "versions": versions.iter().map(|n| gv(tag, *n)).collect::<Vec<_>>()})).expect("a manifest")
}

fn golden_rows(tag: &str, n: u32) -> Vec<SnapshotRow> {
    (1..=n).map(|i| snapshot_row(&format!("snap_{tag}-v{i}"), None, f64::from(7 + i) * GB, None)).collect()
}

/// Four sealed versions of `default` on the wired place, a snapshot and a recipe for each, the listing answering by ids.
fn four_versions() -> Rc<World> {
    let w = World::on(FakeBackend::priced("boat", gb_pricing()));
    *w.mark.borrow_mut() = "h1s1".into();
    w.put_copy_now("boat", "default", &chain("golden", &[1, 2, 3, 4], 4));
    *w.runtime_backend.snapshots.borrow_mut() = Ok(golden_rows("golden", 4));
    w.runtime_backend.filter_ids.set(true);
    for n in 1..=4 {
        w.put_recipe_now("boat", "default", n);
    }
    w.trace_backend();
    w
}

fn account_rows() -> Vec<SnapshotRow> {
    vec![
        snapshot_at("snap_wsp-h1s1-default-v1", Some("wsp-h1s1-default-v1"), 12.0 * GB, OLD),
        snapshot_at("snap_orphan", Some("wsp-h1s1-default-v9"), 20.0 * GB, OLD),
        snapshot_at("snap_before", Some("golden-v1"), 21.0 * GB, OLD),
        snapshot_at("snap_other", Some("wsp-zz9-default-v1"), 7.0 * GB, OLD),
    ]
}

fn account_templates() -> Vec<TemplateRow> {
    vec![
        template_row("tpl_wsp-h1s1-default-v1", "wsp-h1s1-default-v1", Some(OLD)),
        template_row("tpl_wsp-h1s1-old-v1", "wsp-h1s1-old-v1", Some(OLD)),
        template_row("base", "base", Some(OLD)),
    ]
}

/// One sealed version this host records, one snapshot and one template it made that nothing records, one snapshot from
/// before the mark existed and one another host sealed (snapshot-owners.test.ts `account`).
fn account(pricing: wsp_engine::machine::SnapshotStoragePricing) -> Rc<World> {
    let w = World::on(FakeBackend::priced("boat", pricing));
    *w.mark.borrow_mut() = "h1s1".into();
    *w.recorded.borrow_mut() = ["snap_wsp-h1s1-default-v1", "tpl_wsp-h1s1-default-v1"].map(String::from).into_iter().collect();
    *w.runtime_backend.snapshots.borrow_mut() = Ok(account_rows());
    w.with_templates(account_templates());
    w.trace_backend();
    w
}

fn area(w: &Rc<World>) -> Rc<GoldenArea> {
    GoldenArea::new(w.clone())
}

// S1: golden.ts:68, snapshot-storage.test.ts:44
#[test]
fn storage_reads_named_rows_alone_where_snapshots_bill_by_count() {
    run(async {
        let w = World::on(FakeBackend::priced("boat", boat_pricing()));
        *w.mark.borrow_mut() = "h1s1".into();
        *w.recorded.borrow_mut() = ["r1"].map(String::from).into_iter().collect();
        *w.runtime_backend.snapshots.borrow_mut() = Ok(vec![
            snapshot_at("r1", Some("wsp-h1s1-default-v1"), 5.0 * GB, OLD),
            snapshot_at("r2", Some("golden-v1"), 3.0 * GB, OLD),
            snapshot_at("history", None, 9.0 * GB, OLD),
        ]);
        w.runtime_backend.filter_ids.set(true);
        let got = area(&w).storage().await.unwrap();
        let want = storage_of(json!({
            "count": 2, "totalBytes": 8e9, "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "", "monthlyUsd": 0,
            "named": {"count": 2, "free": 10}, "kept": {"count": 1, "bytes": 5e9}, "orphans": {"count": 0, "bytes": 0}, "others": {"count": 1, "bytes": 3e9}
        }));
        assert_eq!(got, Some(want));
        let _ = area(&w).orphans().await.unwrap();
        assert_eq!(*w.runtime_backend.listed.borrow(), vec![Some(vec![]), Some(vec![])]);

        // Billed by the GB, the whole account is read.
        let w = account(gb_pricing());
        area(&w).storage().await.unwrap();
        area(&w).orphans().await.unwrap();
        assert_eq!(*w.runtime_backend.listed.borrow(), vec![None, None]);
    });
}

// S2: golden.ts:69-72, snapshot-storage.test.ts:55
#[test]
fn storage_is_none_without_a_listing() {
    run(async {
        let w = World::on(FakeBackend::forkless("boat"));
        *w.mark.borrow_mut() = "h1s1".into();
        assert_eq!(area(&w).storage().await.unwrap(), None);
        assert_eq!(area(&w).retention(None, None).await.unwrap(), None);
        assert!(w.runtime_backend.listed.borrow().is_empty());
        assert_eq!(w.lines("ready").len(), 2);
    });
}

// S2: golden.ts:69-72, snapshot-owners.test.ts:49, :129
#[test]
fn storage_reads_the_owner_mark_recorded_images_and_the_clock() {
    run(async {
        let w = World::on(FakeBackend::priced("boat", gb_pricing()));
        *w.mark.borrow_mut() = "h1s1".into();
        w.recorded.borrow_mut().clear();
        *w.runtime_backend.snapshots.borrow_mut() = Ok(vec![
            snapshot_at("snap_young", Some("wsp-h1s1-default-v2"), 9.0 * GB, "2026-10-25T08:59:55.000Z"),
            snapshot_at("snap_edge", Some("wsp-h1s1-default-v3"), 1.0 * GB, "2026-10-25T08:59:00.001Z"),
            snapshot_at("snap_orphan", Some("wsp-h1s1-default-v9"), 20.0 * GB, OLD),
            snapshot_at("snap_before", Some("golden-v1"), 21.0 * GB, OLD),
        ]);
        let a = area(&w);
        let line = |kept: (u32, f64), orphans: (u32, f64)| {
            storage_of(json!({
                "count": 4, "totalBytes": 51e9, "freeGb": 10, "usdPerGbMonth": 0.05, "billedFrom": "2026-10-01", "monthlyUsd": 2.0500000000000003,
                "kept": {"count": kept.0, "bytes": kept.1}, "orphans": {"count": orphans.0, "bytes": orphans.1}, "others": {"count": 1, "bytes": 21e9}
            }))
        };
        // Marked this host's moments ago and recorded by nothing: still its own and kept, inside the grace.
        assert_eq!(a.storage().await.unwrap(), Some(line((2, 10e9), (1, 20e9))));
        assert!(w.log().contains(&"recorded_images".to_string()));
        // One millisecond on, the edge row is past the grace and an orphan; the clock is the context's.
        w.clock.advance(1);
        assert_eq!(a.storage().await.unwrap(), Some(line((1, 9e9), (2, 21e9))));
        w.clock.advance(59_999);
        assert_eq!(a.storage().await.unwrap(), Some(line((0, 0.0), (3, 30e9))));
        // The mark is the context's: a host of another mark owns none of them.
        *w.mark.borrow_mut() = "zz9".into();
        let got = a.storage().await.unwrap().unwrap();
        assert_eq!((got.kept.count.0, got.orphans.count.0, got.others.count.0), (0.0, 0.0, 4.0));
        // Recorded ids are kept whoever made them.
        *w.mark.borrow_mut() = "h1s1".into();
        *w.recorded.borrow_mut() = ["snap_orphan"].map(String::from).into_iter().collect();
        let got = a.storage().await.unwrap().unwrap();
        assert_eq!((got.kept.count.0, got.orphans.count.0, got.others.count.0), (1.0, 2.0, 1.0));
    });
}

// S3: golden.ts:466-486, snapshot-owners.test.ts:57, :69
#[test]
fn orphans_split_snapshots_and_templates_and_price_what_deleting_saves() {
    run(async {
        let w = account(gb_pricing());
        let plan = area(&w).orphans().await.unwrap().unwrap();
        let rows = account_rows();
        assert_eq!(plan.snapshots, vec![rows[1].clone()]);
        assert_eq!(plan.templates, vec![account_templates()[1].clone()]);
        assert_eq!(plan.others.snapshots, vec![rows[2].clone(), rows[3].clone()]);
        assert_eq!(plan.others.templates, vec![account_templates()[2].clone()]);
        assert_eq!(plan.freed_bytes, 20e9);
        // 60 GB on the account, 40 GB once the orphan goes, both past the free 10 GB.
        near(plan.saves_usd_per_month, 1.0);
        // The owner read comes before the listing, the templates after it.
        assert_eq!(w.log(), ["ready", "recorded_images", "backend list_snapshots *", "templates list"]);

        // Boat bills named snapshots by count: 14 named, 1 once the 13 orphans go, so the 4 past the free 10 stop costing.
        let w = World::on(FakeBackend::priced("boat", boat_pricing()));
        *w.mark.borrow_mut() = "h1s1".into();
        *w.recorded.borrow_mut() = ["wsp-h1s1-default-v1"].map(String::from).into_iter().collect();
        *w.runtime_backend.snapshots.borrow_mut() =
            Ok((1..=14).map(|n| snapshot_at(&format!("wsp-h1s1-default-v{n}"), Some(&format!("wsp-h1s1-default-v{n}")), 0.0, OLD)).collect());
        let plan = area(&w).orphans().await.unwrap().unwrap();
        assert_eq!(plan.snapshots.len(), 13);
        assert_eq!(plan.templates, vec![]);
        near(plan.saves_usd_per_month, 6.8);
    });
}

// S3: golden.ts:466-467, snapshot-owners.test.ts:158
#[test]
fn orphans_is_none_without_a_listing() {
    run(async {
        let w = World::on(FakeBackend::forkless("boat"));
        *w.mark.borrow_mut() = "h1s1".into();
        assert_eq!(area(&w).orphans().await.unwrap(), None);
        assert_eq!(area(&w).delete_orphans().await.unwrap(), None);
        assert!(w.runtime_backend.listed.borrow().is_empty());
        assert!(w.lines("recorded_images").is_empty());
    });
}

// S3: golden.ts:466-486, snapshot-owners.test.ts:106
#[test]
fn a_snapshot_a_live_workspace_forks_from_is_recorded_so_never_an_orphan() {
    run(async {
        let w = account(gb_pricing());
        let mut rows = account_rows();
        rows.push(snapshot_at("snap_forked", Some("wsp-h1s1-default-v8"), 5.0 * GB, OLD));
        *w.runtime_backend.snapshots.borrow_mut() = Ok(rows.clone());
        w.recorded.borrow_mut().insert("snap_forked".into());
        let plan = area(&w).orphans().await.unwrap().unwrap();
        assert_eq!(ids(&plan.snapshots), ["snap_orphan"]);
        area(&w).delete_orphans().await.unwrap();
        assert_eq!(*w.runtime_backend.deleted.borrow(), ["snap_orphan"]);
    });
}

// S3: golden.ts:466-486, snapshot-owners.test.ts:93
#[test]
fn a_name_marked_by_the_host_alone_is_foreign_unless_recorded() {
    run(async {
        let w = account(gb_pricing());
        let mut rows = account_rows();
        rows.push(snapshot_at("wsp-h1-default-v7", Some("wsp-h1-default-v7"), 3.0 * GB, OLD));
        rows.push(snapshot_at("wsp-h1-default-v2", Some("wsp-h1-default-v2"), 3.0 * GB, OLD));
        *w.runtime_backend.snapshots.borrow_mut() = Ok(rows);
        w.recorded.borrow_mut().insert("wsp-h1-default-v2".into());
        let plan = area(&w).orphans().await.unwrap().unwrap();
        assert_eq!(ids(&plan.snapshots), ["snap_orphan"]);
        let names: Vec<_> = plan.others.snapshots.iter().filter_map(|r| r.name.clone()).collect();
        assert_eq!(names, ["golden-v1", "wsp-zz9-default-v1", "wsp-h1-default-v7"]);
        area(&w).delete_orphans().await.unwrap();
        assert_eq!(*w.runtime_backend.deleted.borrow(), ["snap_orphan"]);
    });
}

// S3: golden.ts:466-486, snapshot-owners.test.ts:120
#[test]
fn a_snapshot_marked_inside_the_grace_is_kept_and_counted_as_this_hosts() {
    run(async {
        let w = account(gb_pricing());
        let mut rows = account_rows();
        rows.push(snapshot_at("snap_sealing", Some("wsp-h1s1-default-v2"), 9.0 * GB, "2026-10-25T08:59:55.000Z"));
        rows.push(snapshot_at("snap_sealing_edge", Some("wsp-h1s1-default-v3"), 1.0 * GB, "2026-10-25T08:59:00.001Z"));
        *w.runtime_backend.snapshots.borrow_mut() = Ok(rows);
        let mut templates = account_templates();
        templates.push(template_row("tpl_promoting", "wsp-h1s1-default-v2", Some("2026-10-25T08:59:55.000Z")));
        w.with_templates(templates);
        let a = area(&w);
        let plan = a.orphans().await.unwrap().unwrap();
        assert_eq!(ids(&plan.snapshots), ["snap_orphan"]);
        assert_eq!(plan.templates.iter().map(|t| t.id.clone()).collect::<Vec<_>>(), ["tpl_wsp-h1s1-old-v1"]);
        // Counted as this host's and kept, not as another host's.
        let want = storage_of(json!({
            "count": 6, "totalBytes": 70e9, "freeGb": 10, "usdPerGbMonth": 0.05, "billedFrom": "2026-10-01", "monthlyUsd": 3,
            "kept": {"count": 3, "bytes": 22e9}, "orphans": {"count": 1, "bytes": 20e9}, "others": {"count": 2, "bytes": 28e9}
        }));
        assert_eq!(a.storage().await.unwrap(), Some(want));
        a.delete_orphans().await.unwrap();
        assert_eq!(*w.runtime_backend.deleted.borrow(), ["snap_orphan"]);
        assert_eq!(w.lines("templates delete"), ["templates delete tpl_wsp-h1s1-old-v1"]);
    });
}

// S4: golden.ts:488-519, snapshot-owners.test.ts:79, :88
#[test]
fn delete_orphans_deletes_templates_then_snapshots_and_counts_a_missing_one_deleted() {
    run(async {
        let w = account(gb_pricing());
        let mut rows = account_rows();
        rows.push(snapshot_at("snap_gone", Some("wsp-h1s1-default-v8"), 1.0 * GB, OLD));
        *w.runtime_backend.snapshots.borrow_mut() = Ok(rows.clone());
        // The provider already lost this one: gone is what was asked for.
        w.runtime_backend.delete_fails.borrow_mut().insert("snap_gone".into(), missing("Not found"));
        let done = area(&w).delete_orphans().await.unwrap().unwrap();
        let want = OrphansDeleted { snapshots: vec![rows[1].clone(), rows[4].clone()], templates: vec![account_templates()[1].clone()], failed: vec![] };
        assert_eq!(done, want);
        // The template first, which the provider will not let the snapshot under it go before; one account listing.
        assert_eq!(
            w.log(),
            [
                "ready", "recorded_images", "backend list_snapshots *", "templates list", "templates delete tpl_wsp-h1s1-old-v1",
                "backend delete_snapshot snap_orphan", "backend delete_snapshot snap_gone"
            ]
        );
    });
}

// S4: golden.ts:488-519, snapshot-owners.test.ts:136
#[test]
fn delete_orphans_reports_each_failure_and_goes_on() {
    run(async {
        let w = account(gb_pricing());
        let mut rows = account_rows();
        rows.push(snapshot_at("snap_second", Some("wsp-h1s1-default-v8"), 1.0 * GB, OLD));
        *w.runtime_backend.snapshots.borrow_mut() = Ok(rows.clone());
        let t = w.with_templates(account_templates());
        t.delete_fails.borrow_mut().insert("tpl_wsp-h1s1-old-v1".into(), conflict("template in use"));
        w.runtime_backend.delete_fails.borrow_mut().insert("snap_orphan".into(), conflict("SnapshotHasChildren"));
        let done = area(&w).delete_orphans().await.unwrap().unwrap();
        let want = OrphansDeleted {
            snapshots: vec![rows[4].clone()],
            templates: vec![],
            failed: vec![
                OrphanFailed { id: "tpl_wsp-h1s1-old-v1".into(), name: Some("wsp-h1s1-old-v1".into()), message: "template in use".into() },
                OrphanFailed { id: "snap_orphan".into(), name: Some("wsp-h1s1-default-v9".into()), message: "SnapshotHasChildren".into() },
            ],
        };
        assert_eq!(done, want);
        assert_eq!(*w.runtime_backend.deleted.borrow(), ["snap_orphan", "snap_second"]);
    });
}

// S5: golden.ts:521-533, sealed-image.test.ts:599
#[test]
fn retention_places_lists_own_then_wired_then_sorted_others_with_a_backend() {
    run(async {
        let held = || manifest(&[(1, "snap_v1", None)]);
        let world = |image_at: Option<&str>, copies: &[&str]| {
            let w = World::new();
            let door = w.with_door();
            for p in ["Zed", "alpha", "zeta", "zzz"] {
                door.computer(p, p, Some(FakeBackend::new(p)));
            }
            w.places.rows.borrow_mut().insert("spoo".into(), FakeBackend::new("spoo"));
            if let Some(p) = image_at {
                w.put_image_now(&sealed_image("default", 1, "h", Some(p)));
            }
            for p in copies {
                w.put_copy_now(p, "default", &held());
            }
            // Another name's copy at a place with a backend is no copy of this one.
            w.put_copy_now("zzz", "other", &held());
            w
        };
        // The image's own place first, the wired provider next, then the rest by plain code-unit order: a capital sorts
        // before every lower case letter, and a place this host no longer has is left out.
        let w = world(Some("spoo"), &["boat", "spoo", "zeta", "Zed", "alpha", "gone"]);
        let got = area(&w).retention_places(None).await.unwrap();
        assert_eq!(got, ["spoo", "boat", "Zed", "alpha", "zeta"]);
        assert_eq!(w.log(), ["ready"]);
        // The image at the wired provider is listed once.
        let w = world(None, &["boat", "alpha"]);
        assert_eq!(area(&w).retention_places(None).await.unwrap(), ["boat", "alpha"]);
        // No copy at the wired provider: it is not offered.
        let w = world(Some("spoo"), &["spoo", "alpha"]);
        assert_eq!(area(&w).retention_places(None).await.unwrap(), ["spoo", "alpha"]);
        // A name asked for is the name read.
        let w = world(None, &["boat"]);
        w.put_copy_now("alpha", "other", &held());
        assert_eq!(area(&w).retention_places(Some("other")).await.unwrap(), ["boat", "alpha", "zzz"]);
    });
}

// S6: golden.ts:535-547, snapshot-storage.test.ts:79
#[test]
fn retention_plans_the_asked_place_and_says_whether_it_is_a_copy() {
    run(async {
        let w = four_versions();
        let got = area(&w).retention(None, None).await.unwrap().unwrap();
        assert_eq!(nums(&got.plan.keep), [4, 3]);
        assert_eq!(nums(&got.plan.drop), [1, 2]);
        assert!(got.plan.abandoned.is_empty() && got.plan.guarded.is_empty());
        assert!(!got.plan.parent_assumed);
        assert_eq!(got.plan.freed_bytes, 17e9);
        near(got.plan.saves_usd_per_month, 0.85);
        assert_eq!(got.plan.pricing, gb_pricing());
        assert_eq!((got.place.as_str(), got.place_name.as_str(), got.copy), ("boat", "boat", false));
        // The listing is asked for the versions' own ids, not the account's history.
        let ids_asked: Vec<String> = (1..=4).map(|n| format!("snap_golden-v{n}")).collect();
        assert_eq!(*w.runtime_backend.listed.borrow(), vec![Some(ids_asked)]);

        // A place that is not the image's own is a copy, listed on its own backend and named as the door names it.
        let spoo = FakeBackend::new("spoo");
        *spoo.snapshots.borrow_mut() = Ok(golden_rows("golden", 4));
        w.with_door().computer("spoo", "Spoo", Some(spoo.clone()));
        w.put_copy_now("spoo", "default", &chain("golden", &[1, 2, 3, 4], 4));
        let got = area(&w).retention(None, Some("spoo")).await.unwrap().unwrap();
        assert_eq!((got.place.as_str(), got.place_name.as_str(), got.copy), ("spoo", "Spoo", true));
        assert_eq!(nums(&got.plan.drop), [1, 2]);
        assert_eq!(spoo.listed.borrow().len(), 1);
        assert_eq!(w.runtime_backend.listed.borrow().len(), 1);
    });
}

// S6: golden.ts:535-547, snapshot-storage.test.ts:104, :135
#[test]
fn retention_never_offers_a_snapshot_a_workspace_was_forked_from() {
    run(async {
        let w = four_versions();
        w.add_workspace("w_a", "alpha", "snap_golden-v1");
        w.add_workspace("w_b", "beta", "snap_golden-v1");
        let got = area(&w).retention(None, None).await.unwrap().unwrap();
        assert_eq!(nums(&got.plan.drop), [2]);
        assert_eq!(got.plan.guarded.len(), 1);
        assert_eq!(got.plan.guarded[0].version.version.0, 1.0);
        assert_eq!(got.plan.guarded[0].workspaces, ["alpha", "beta"]);
        assert_eq!(got.plan.freed_bytes, 9e9);
    });
}

// S6: golden.ts:535-547, snapshot-storage.test.ts:162
#[test]
fn a_rolled_back_head_keeps_the_versions_ahead_of_it() {
    run(async {
        let w = four_versions();
        w.put_copy_now("boat", "default", &chain("golden", &[1, 2, 3, 4], 2));
        let got = area(&w).retention(None, None).await.unwrap().unwrap();
        assert_eq!(nums(&got.plan.keep), [2, 1]);
        assert!(got.plan.drop.is_empty() && got.plan.abandoned.is_empty());
        assert_eq!(got.plan.freed_bytes, 0.0);
    });
}

// S6: golden.ts:535-547, snapshot-storage.test.ts:55, :201
#[test]
fn retention_is_none_without_a_listing_or_a_copy() {
    run(async {
        let w = World::on(FakeBackend::forkless("boat"));
        w.put_copy_now("boat", "default", &chain("golden", &[1], 1));
        assert_eq!(area(&w).retention(None, None).await.unwrap(), None);
        assert!(w.runtime_backend.listed.borrow().is_empty());

        // A listing but no golden of that name, and no copy at the place asked: nothing to plan, nothing listed.
        let w = four_versions();
        w.with_door().computer("spoo", "Spoo", Some(FakeBackend::new("spoo")));
        assert_eq!(area(&w).retention(Some("nope"), None).await.unwrap(), None);
        assert_eq!(area(&w).retention(None, Some("spoo")).await.unwrap(), None);
        assert!(w.runtime_backend.listed.borrow().is_empty());
    });
}

// S7: golden.ts:549-574, snapshot-storage.test.ts:79
#[test]
fn prune_drops_each_planned_version_and_its_recipe() {
    run(async {
        let w = four_versions();
        // What the provider lists once the drops landed: the kept versions, which this host records.
        *w.recorded.borrow_mut() = ["snap_golden-v3", "snap_golden-v4"].map(String::from).into_iter().collect();
        let got = area(&w).prune(None, None).await.unwrap();
        assert_eq!(nums(&got.dropped), [1, 2]);
        assert!(got.failed.is_empty());
        assert_eq!(
            w.log(),
            [
                "ready", "backend list_snapshots snap_golden-v1,snap_golden-v2,snap_golden-v3,snap_golden-v4",
                "drop_image snap_golden-v1", "put_copy boat default", "drop_copy_recipe boat default 1",
                "drop_image snap_golden-v2", "put_copy boat default", "drop_copy_recipe boat default 2",
                "backend list_snapshots snap_golden-v4,snap_golden-v3", "recorded_images"
            ]
        );
        assert_eq!(w.copy_now("boat", "default"), Some(chain("golden", &[3, 4], 4)));
        assert_eq!((w.recipe_now("boat", "default", 1), w.recipe_now("boat", "default", 2)), (None, None));
        assert!(w.recipe_now("boat", "default", 3).is_some() && w.recipe_now("boat", "default", 4).is_some());
        // The storage line is read over the kept versions alone.
        let want = storage_of(json!({
            "count": 2, "totalBytes": 21e9, "freeGb": 10, "usdPerGbMonth": 0.05, "billedFrom": "2026-10-01", "monthlyUsd": 0.55,
            "kept": {"count": 2, "bytes": 21e9}, "orphans": {"count": 0, "bytes": 0}, "others": {"count": 0, "bytes": 0}
        }));
        assert_eq!(got.storage, Some(want));
        // What is left has nothing more to offer.
        let again = area(&w).retention(None, None).await.unwrap().unwrap();
        assert!(again.plan.drop.is_empty());
    });
}

// S7: golden.ts:549-574, snapshot-storage.test.ts:135
#[test]
fn prune_leaves_a_version_a_workspace_stands_on() {
    run(async {
        let w = four_versions();
        w.add_workspace("w_a", "alpha", "snap_golden-v1");
        let got = area(&w).prune(None, None).await.unwrap();
        assert_eq!(nums(&got.dropped), [2]);
        assert_eq!(w.lines("drop_image"), ["drop_image snap_golden-v2"]);
        assert_eq!(w.copy_now("boat", "default"), Some(chain("golden", &[1, 3, 4], 4)));
        assert!(w.recipe_now("boat", "default", 1).is_some() && w.recipe_now("boat", "default", 2).is_none());
    });
}

// S7: golden.ts:559-565, snapshot-storage.test.ts:122
#[test]
fn prune_counts_a_missing_snapshot_as_dropped() {
    run(async {
        let w = four_versions();
        // The provider no longer has v1's snapshot: gone is what a drop asks, so its version goes with it.
        w.drop_image_fails.borrow_mut().insert("snap_golden-v1".into(), ImageError::Engine(missing("Not found")));
        let got = area(&w).prune(None, None).await.unwrap();
        assert_eq!(nums(&got.dropped), [1, 2]);
        assert!(got.failed.is_empty());
        assert_eq!(w.copy_now("boat", "default"), Some(chain("golden", &[3, 4], 4)));
        assert!(w.recipe_now("boat", "default", 1).is_none());
    });
}

// S7: golden.ts:559-565, snapshot-storage.test.ts:112
#[test]
fn prune_keeps_a_version_whose_delete_failed() {
    run(async {
        let w = four_versions();
        w.drop_image_fails.borrow_mut().insert("snap_golden-v1".into(), ImageError::Engine(conflict("SnapshotHasChildren")));
        let got = area(&w).prune(None, None).await.unwrap();
        assert_eq!(nums(&got.dropped), [2]);
        assert_eq!(got.failed, vec![(1, "SnapshotHasChildren".to_string())]);
        assert_eq!(w.copy_now("boat", "default"), Some(chain("golden", &[1, 3, 4], 4)));
        // The refused version keeps its recipe; the one dropped after it loses its own.
        assert!(w.recipe_now("boat", "default", 1).is_some() && w.recipe_now("boat", "default", 2).is_none());
        assert_eq!(w.lines("drop_copy_recipe"), ["drop_copy_recipe boat default 2"]);
    });
}

// S7: golden.ts:549-574, snapshot-storage.test.ts:171
#[test]
fn prune_without_a_plan_drops_nothing() {
    run(async {
        let w = World::on(FakeBackend::priced("boat", gb_pricing()));
        let got = area(&w).prune(None, None).await.unwrap();
        assert!(got.dropped.is_empty() && got.failed.is_empty());
        assert_eq!(got.storage, None);
        assert_eq!(w.log(), ["ready"]);

        let w = World::on(FakeBackend::forkless("boat"));
        w.put_copy_now("boat", "default", &chain("golden", &[1, 2, 3, 4], 4));
        let got = area(&w).prune(None, None).await.unwrap();
        assert!(got.dropped.is_empty() && got.failed.is_empty() && got.storage.is_none());
        assert!(w.lines("drop_image").is_empty());
    });
}

// S7: golden.ts:572-573
#[test]
fn prune_leaves_out_storage_a_failed_listing_did_not_read() {
    run(async {
        let w = four_versions();
        // The plan's listing answers; the storage line's, after the deletes, is refused.
        w.runtime_backend.list_queue.borrow_mut().push_back(Ok(golden_rows("golden", 4)));
        *w.runtime_backend.snapshots.borrow_mut() = Err(provider_error(wsp_engine::errors::ErrorKind::Transient, 502, "upstream unavailable"));
        let got = area(&w).prune(None, None).await.unwrap();
        assert_eq!(nums(&got.dropped), [1, 2]);
        assert!(got.failed.is_empty());
        assert_eq!(got.storage, None);
        assert_eq!(w.copy_now("boat", "default"), Some(chain("golden", &[3, 4], 4)));
    });
}

// S7: golden.ts:549-574, sealed-image.test.ts:560
#[test]
fn prune_reaches_the_copy_at_the_place_asked_by_that_places_own_version_numbers() {
    run(async {
        // The image was sealed at spoo and stands at v2; the wired provider holds four copies of its own, numbered apart.
        let w = World::on(FakeBackend::priced("boat", gb_pricing()));
        *w.mark.borrow_mut() = "h1s1".into();
        let spoo = FakeBackend::priced("spoo", gb_pricing());
        w.with_door().computer("spoo", "Spoo", Some(spoo.clone()));
        w.put_image_now(&sealed_image("default", 2, "h", Some("spoo")));
        w.put_copy_now("boat", "default", &chain("copy", &[1, 2, 3, 4], 4));
        w.put_copy_now("spoo", "default", &chain("golden", &[1, 2], 2));
        *w.runtime_backend.snapshots.borrow_mut() = Ok(golden_rows("copy", 4));
        *spoo.snapshots.borrow_mut() = Ok(golden_rows("golden", 2));
        for n in 1..=4 {
            w.put_recipe_now("boat", "default", n);
        }
        for n in 1..=2 {
            w.put_recipe_now("spoo", "default", n);
        }
        let a = area(&w);
        let asked = a.retention(None, Some("boat")).await.unwrap().unwrap();
        assert_eq!((asked.place.as_str(), asked.copy), ("boat", true));
        assert_eq!(nums(&asked.plan.drop), [1, 2]);
        // No place asked is the image's own, which has nothing past its two kept versions.
        assert!(a.prune(None, None).await.unwrap().dropped.is_empty());
        assert!(w.lines("drop_image").is_empty());
        let got = a.prune(None, Some("boat")).await.unwrap();
        assert_eq!(nums(&got.dropped), [1, 2]);
        assert_eq!(w.lines("drop_image"), ["drop_image snap_copy-v1", "drop_image snap_copy-v2"]);
        assert_eq!(w.copy_now("boat", "default"), Some(chain("copy", &[3, 4], 4)));
        assert_eq!(w.copy_now("spoo", "default"), Some(chain("golden", &[1, 2], 2)));
        assert!(w.recipe_now("boat", "default", 1).is_none() && w.recipe_now("boat", "default", 3).is_some());
        assert!(w.recipe_now("spoo", "default", 1).is_some() && w.recipe_now("spoo", "default", 2).is_some());
        assert_eq!(w.lines("drop_copy_recipe"), ["drop_copy_recipe boat default 1", "drop_copy_recipe boat default 2"]);
    });
}

fn head_at(n: u32, versions: &[u32], tag: &str) -> GoldenManifest {
    chain(tag, versions, n)
}

// S8: golden.ts:576-589, runtime-builders.test.ts:559
#[test]
fn rollback_refuses_missing_and_writes_the_rolled_back_wired_copy() {
    run(async {
        let w = World::new();
        w.put_copy_now("boat", "default", &chain("golden", &[1, 2], 2));
        let rolled = head_at(1, &[1, 2], "golden");
        *w.roads.rolled_back.borrow_mut() = Some(Ok(rolled.clone()));
        let got = area(&w).rollback(1, None).await.unwrap();
        assert_eq!(got, rolled);
        assert_eq!(w.copy_now("boat", "default"), Some(rolled));
        // It does not wait for the host to be ready.
        assert_eq!(w.log(), ["road rollback 1", "put_copy boat default"]);

        // No wired copy of that name.
        let before = w.log().len();
        let err = area(&w).rollback(1, Some("nope")).await.unwrap_err();
        assert_eq!(err, HostError { kind: Some(Kind::Missing), ..HostError::new("no golden named \"nope\"") });
        assert_eq!(w.log().len(), before);
    });
}

// S8: golden.ts:580-584, runtime-builders.test.ts:571
#[test]
fn rollback_marks_the_engines_refusal_missing() {
    run(async {
        let w = World::new();
        let held = chain("golden", &[1], 1);
        w.put_copy_now("boat", "default", &held);
        *w.roads.rolled_back.borrow_mut() = Some(Err(other("rollback target v7 not in manifest")));
        let err = area(&w).rollback(7, None).await.unwrap_err();
        assert_eq!(err, HostError { kind: Some(Kind::Missing), ..HostError::new("rollback target v7 not in manifest") });
        // Untouched.
        assert_eq!(w.copy_now("boat", "default"), Some(held));
        assert!(w.lines("put_copy").is_empty());
    });
}

fn promoted(id: &str, sharing: Option<u32>) -> Result<Promoted, EngineError> {
    Ok(Promoted { template_id: id.into(), sharing })
}

fn row(version: u32, done: PromotionDone) -> GoldenPromotion {
    GoldenPromotion { golden: "default".into(), version, done }
}

fn done(id: &str, sharing: Option<u32>) -> PromotionDone {
    PromotionDone::Promoted { template_id: id.into(), sharing }
}

fn templates_of(m: &GoldenManifest) -> Vec<Option<String>> {
    m.versions.iter().map(|v| v.template_id.clone()).collect()
}

fn promote_world(versions: &[(u32, Option<&str>)]) -> Rc<World> {
    let w = World::new();
    *w.mark.borrow_mut() = "h1s1".into();
    w.with_templates(vec![]);
    let vs: Vec<_> = versions.iter().map(|(n, t)| (*n, format!("snap_golden-v{n}"), *t)).collect();
    let refs: Vec<(u32, &str, Option<&str>)> = vs.iter().map(|(n, s, t)| (*n, s.as_str(), *t)).collect();
    w.put_copy_now("boat", "default", &manifest(&refs));
    w
}

// S9: golden.ts:591-611, golden-templates.test.ts:79
#[test]
fn promote_templates_each_version_without_one() {
    run(async {
        let w = promote_world(&[(1, None), (2, None), (3, Some("tpl_three"))]);
        w.roads.promoted.borrow_mut().push_back(promoted("tpl_wsp-h1s1-default-v1", Some(1)));
        w.roads.promoted.borrow_mut().push_back(promoted("tpl_wsp-h1s1-default-v2", Some(0)));
        let got = area(&w).promote(None).await.unwrap();
        assert_eq!(got, Some(vec![row(1, done("tpl_wsp-h1s1-default-v1", Some(1))), row(2, done("tpl_wsp-h1s1-default-v2", Some(0)))]));
        // Named wsp-<host mark>-<golden>-v<n>, one write per version.
        assert_eq!(
            w.log(),
            [
                "ready", "road promote_version snap_golden-v1 wsp-h1s1-default-v1", "put_copy boat default",
                "road promote_version snap_golden-v2 wsp-h1s1-default-v2", "put_copy boat default"
            ]
        );
        let stored = w.copy_now("boat", "default").unwrap();
        assert_eq!(templates_of(&stored), [Some("tpl_wsp-h1s1-default-v1".into()), Some("tpl_wsp-h1s1-default-v2".into()), Some("tpl_three".into())]);
        // A second run finds nothing to do.
        assert_eq!(area(&w).promote(None).await.unwrap(), Some(vec![]));
        assert_eq!(w.lines("road promote_version").len(), 2);
    });
}

// S9: golden.ts:591-611, golden-templates.test.ts:31
#[test]
fn promote_names_the_template_after_the_hosts_mark_and_owner() {
    run(async {
        let w = World::new();
        *w.mark.borrow_mut() = "9f3a1c2bs1".into();
        w.with_templates(vec![]);
        w.put_copy_now("boat", "default", &manifest(&[(1, "snap_wsp-9f3a1c2bs1-default-v1", None)]));
        w.roads.promoted.borrow_mut().push_back(promoted("tpl_wsp-9f3a1c2bs1-default-v1", Some(1)));
        let got = area(&w).promote(None).await.unwrap();
        assert_eq!(got, Some(vec![row(1, done("tpl_wsp-9f3a1c2bs1-default-v1", Some(1)))]));
        assert_eq!(w.lines("road promote_version"), ["road promote_version snap_wsp-9f3a1c2bs1-default-v1 wsp-9f3a1c2bs1-default-v1"]);
    });
}

// S9: golden.ts:604-609, golden-templates.test.ts:101
#[test]
fn promote_writes_a_failed_row_and_goes_on() {
    run(async {
        let w = promote_world(&[(1, None), (2, None)]);
        w.roads.promoted.borrow_mut().push_back(Err(other("its snapshot is gone at the provider")));
        w.roads.promoted.borrow_mut().push_back(promoted("tpl_wsp-h1s1-default-v2", Some(0)));
        let got = area(&w).promote(None).await.unwrap();
        let failed = PromotionDone::Failed { error: "its snapshot is gone at the provider".into() };
        assert_eq!(got, Some(vec![row(1, failed), row(2, done("tpl_wsp-h1s1-default-v2", Some(0)))]));
        let stored = w.copy_now("boat", "default").unwrap();
        assert_eq!(templates_of(&stored), [None, Some("tpl_wsp-h1s1-default-v2".into())]);
        assert_eq!(w.lines("put_copy").len(), 1);
    });
}

// S9: golden.ts:604-609, golden-templates.test.ts:114
#[test]
fn promote_leaves_sharing_out_where_the_count_was_unread() {
    run(async {
        let w = promote_world(&[(1, None)]);
        w.roads.promoted.borrow_mut().push_back(promoted("tpl_wsp-h1s1-default-v1", None));
        let got = area(&w).promote(None).await.unwrap().unwrap();
        assert_eq!(got, vec![row(1, done("tpl_wsp-h1s1-default-v1", None))]);
        assert!(serde_json::to_value(&got[0]).unwrap().get("sharing").is_none());
    });
}

// S9: golden.ts:598-603
#[test]
fn promote_writes_a_failed_row_where_the_name_cannot_be_made() {
    run(async {
        let w = promote_world(&[(1, None)]);
        *w.mark.borrow_mut() = String::new();
        let got = area(&w).promote(None).await.unwrap().unwrap();
        let error = "a snapshot name needs a host of lowercase letters and digits, got \"\"";
        assert_eq!(got, vec![row(1, PromotionDone::Failed { error: error.into() })]);
        assert!(w.lines("road promote_version").is_empty());
    });
}

// S9: golden.ts:591-611, golden-templates.test.ts:122
#[test]
fn promote_is_none_without_templates() {
    run(async {
        let w = World::new();
        w.put_copy_now("boat", "default", &manifest(&[(1, "snap_golden-v1", None)]));
        assert_eq!(area(&w).promote(None).await.unwrap(), None);
        assert_eq!(w.log(), ["ready"]);
        // With templates, a golden that does not exist has nothing to promote.
        let w = promote_world(&[(1, None)]);
        assert_eq!(area(&w).promote(Some("other")).await.unwrap(), Some(vec![]));
        assert!(w.lines("road promote_version").is_empty());
    });
}

// S9: golden.ts:605-607
#[test]
fn promote_fails_a_version_whose_golden_was_dropped_meanwhile() {
    run(async {
        let w = promote_world(&[(1, None), (2, None)]);
        w.roads.promoted.borrow_mut().push_back(promoted("tpl_wsp-h1s1-default-v1", Some(1)));
        w.roads.promoted.borrow_mut().push_back(promoted("tpl_wsp-h1s1-default-v2", Some(0)));
        // The golden is deleted while the template is being made.
        let store = w.store.clone();
        *w.roads.on_promote.borrow_mut() = Some(Rc::new(move || {
            if let Some(c) = store.docs.borrow_mut().get_mut("goldens") {
                c.shift_remove("boat/default");
            }
        }));
        let got = area(&w).promote(None).await.unwrap().unwrap();
        let gone = || PromotionDone::Failed { error: "golden default was dropped while its versions were being promoted".into() };
        assert_eq!(got, vec![row(1, gone()), row(2, gone())]);
        assert!(w.lines("put_copy").is_empty());
        assert!(w.copy_now("boat", "default").is_none());
    });
}
