// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of the sweeps: names and ages against the TS's own output (tests/fixtures/oracle/small.json), the
//! storage view's shape, and retention leaving versions above the head. spec/sweeps.md.
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use wsp_engine::labels::is_reserved;
use wsp_engine::machine::{SnapshotRow, SnapshotStoragePricing};
use wsp_engine::orphans::describe_age;
use wsp_engine::snapshot_names::{golden_name, name_owner, project_snapshot_name};
use wsp_engine::snapshot_storage::{retention_plan, snapshot_storage, OwnerRead};

fn oracle() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle/small.json");
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

// spec/sweeps.md W8
#[test]
fn describe_age_matches_the_ts_oracle() {
    for c in oracle()["ages"].as_array().unwrap() {
        assert_eq!(describe_age(c["ms"].as_i64()), c["words"].as_str().unwrap(), "{}", c["ms"]);
    }
}

// spec/sweeps.md W1, W2
#[test]
fn names_match_the_ts_oracle() {
    let o = &oracle()["snapshotNames"];
    for c in o["golden"].as_array().unwrap() {
        let got = golden_name(c["host"].as_str().unwrap(), c["golden"].as_str().unwrap(), c["version"].as_u64().unwrap());
        match c["out"].get("ok") {
            Some(ok) => assert_eq!(got.unwrap(), ok.as_str().unwrap()),
            None => assert_eq!(got.unwrap_err().to_string(), c["out"]["error"].as_str().unwrap()),
        }
    }
    assert_eq!(project_snapshot_name("h1", "my-proj", "2026-10-25T00-00-00Z").unwrap(), o["project"]["ok"].as_str().unwrap());
    for c in o["owners"].as_array().unwrap() {
        assert_eq!(name_owner(c["name"].as_str()), c["owner"].as_str().map(String::from), "{}", c["name"]);
    }
    let labels: indexmap::IndexMap<String, String> = [("poc".to_string(), String::new())].into_iter().collect();
    assert!(is_reserved(&labels), "an empty value is still the key");
    assert!(!is_reserved(&indexmap::IndexMap::new()));
}

fn pricing(v: Value) -> SnapshotStoragePricing {
    SnapshotStoragePricing {
        wire: serde_json::from_value(json!({ "freeGb": v["freeGb"], "usdPerGbMonth": v["usdPerGbMonth"], "billedFrom": "2026-10-01" }))
            .unwrap(),
        free_named: v["freeNamed"].as_u64().map(|n| n as u32),
        usd_per_named_month: v["usdPerNamedMonth"].as_f64(),
    }
}

fn row(id: &str, name: Option<&str>, size: u64, created_at: &str) -> SnapshotRow {
    let mut v = json!({ "id": id, "sizeBytes": size, "createdAt": created_at });
    if let Some(n) = name {
        v["name"] = json!(n);
    }
    serde_json::from_value(v).unwrap()
}

// spec/sweeps.md W9 to W11: named rides only where billed by count, and the keys keep the view's order.
#[test]
fn storage_view_writes_named_only_when_billed_by_count() {
    let now = 1_790_000_000_000_i64;
    let rows = vec![
        row("a", Some("wsp-h1-default-v1"), 6_000_000_000, "2026-01-01T00:00:00Z"),
        row("b", Some("wsp-h2-default-v1"), 5_000_000_000, "2026-01-01T00:00:00Z"),
        row("c", None, 1_000_000_000, "2026-01-01T00:00:00Z"),
        row("d", Some("wsp-h1-fresh-v1"), 1, "2099-01-01T00:00:00Z"),
    ];
    let read = OwnerRead { host_id: "h1".into(), recorded: HashSet::new(), now };
    let by_gb = snapshot_storage(&rows, &pricing(json!({ "freeGb": 10, "usdPerGbMonth": 0.05 })), &read).unwrap();
    let v = serde_json::to_value(&by_gb).unwrap();
    assert_eq!(
        v.as_object().unwrap().keys().cloned().collect::<Vec<_>>(),
        vec!["count", "totalBytes", "freeGb", "usdPerGbMonth", "billedFrom", "monthlyUsd", "kept", "orphans", "others"]
    );
    assert_eq!(v["kept"], json!({ "count": 1, "bytes": 1 }));
    assert_eq!(v["orphans"], json!({ "count": 1, "bytes": 6_000_000_000_u64 }));
    assert_eq!(v["others"], json!({ "count": 2, "bytes": 6_000_000_000_u64 }));
    // 12.000000001 GB less the free 10, at $0.05.
    assert!((v["monthlyUsd"].as_f64().unwrap() - 0.10000000005).abs() < 1e-12, "{}", v["monthlyUsd"]);
    let by_count =
        snapshot_storage(&rows, &pricing(json!({ "freeGb": 0, "usdPerGbMonth": 0, "freeNamed": 1, "usdPerNamedMonth": 0.5 })), &read)
            .unwrap();
    let v = serde_json::to_value(&by_count).unwrap();
    assert_eq!(v["named"], json!({ "count": 3, "free": 1 }));
    assert_eq!(v["monthlyUsd"], json!(1.0));
    let empty = OwnerRead { host_id: String::new(), recorded: HashSet::new(), now };
    assert_eq!(
        snapshot_storage(&rows, &pricing(json!({ "freeGb": 10, "usdPerGbMonth": 0.05 })), &empty).unwrap_err().to_string(),
        "the snapshot split needs this host's mark; without one it could not tell this host's snapshots from another host's"
    );
}

fn version(n: u64, snap: &str, parent: Option<&str>) -> Value {
    let mut v = json!({ "version": n, "snapshotId": snap, "baseTemplate": "base", "setupSha": "s", "createdAt": "2026-01-01T00:00:00Z", "smoke": { "cmd": "true", "exitCode": 0 }, "base": [] });
    if let Some(p) = parent {
        v["parentSnapshotId"] = json!(p);
    }
    v
}

// spec/sweeps.md W12: a rollback leaves versions above the head alone; the guarded one keeps its workspaces.
#[test]
fn retention_keeps_versions_above_the_head() {
    let manifest = serde_json::from_value(json!({
        "head": 3,
        "versions": [version(1, "s1", None), version(2, "s2", Some("s1")), version(3, "s3", Some("s2")), version(4, "s4", Some("s3")), version(5, "s5", Some("s4")), version(6, "s6", Some("s1"))]
    }))
    .unwrap();
    let rows = vec![row("s1", Some("wsp-h1-g-v1"), 3_000_000_000, "x"), row("s2", None, 1, "x"), row("s5", None, 7, "x")];
    let forked = |snap: &str| if snap == "s9" { vec!["ws-1".to_string()] } else { vec![] };
    let plan = retention_plan(&manifest, &rows, &forked, &pricing(json!({ "freeGb": 0, "usdPerGbMonth": 1 })), Some(1));
    let ids = |vs: &Vec<wsp_wire::views::golden_image::GoldenVersion>| vs.iter().map(|v| v.snapshot_id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&plan.keep), vec!["s3"]);
    // Older ancestors oldest first; 4, 5 above the head and 6 above it too are never offered.
    assert_eq!(ids(&plan.drop), vec!["s1", "s2"]);
    assert_eq!(plan.abandoned, vec![]);
    assert!(!plan.parent_assumed);
    assert_eq!(plan.freed_bytes, 3_000_000_001.0);
    assert_eq!(plan.named_freed, 1);
    assert!((plan.saves_usd_per_month - 3.000000001).abs() < 1e-9, "{}", plan.saves_usd_per_month);
}

// ---- K-rules (spec/sweeps.md K6, K7), values from running snapshot-storage.ts at a4033dc34

/// K6: the named charge is `max(0, named - free_named) * price` without underflow (named 1 below free 5 is 0); a zero
/// named price is still a price (`named` written); no named price, no `named`; per-GB billing still adds the named
/// charge to the sum without writing `named`.
#[test]
fn the_named_charge_never_underflows_and_named_shows_only_when_billed_by_count() {
    let rows = vec![row("s", Some("wsp-h1-g-v1"), 1_000_000_000, "2020-01-01T00:00:00Z")];
    let read = OwnerRead { host_id: "h1".into(), recorded: HashSet::new(), now: 1_790_000_000_000 };
    for (p, monthly, named) in [
        (json!({ "freeGb": 0, "usdPerGbMonth": 0, "freeNamed": 5, "usdPerNamedMonth": 0.5 }), 0.0, Some(json!({ "count": 1, "free": 5 }))),
        (json!({ "freeGb": 0, "usdPerGbMonth": 0, "usdPerNamedMonth": 0 }), 0.0, Some(json!({ "count": 1, "free": 0 }))),
        (json!({ "freeGb": 0, "usdPerGbMonth": 0, "freeNamed": 2 }), 0.0, None),
        (json!({ "freeGb": 0, "usdPerGbMonth": 1, "freeNamed": 0, "usdPerNamedMonth": 0.5 }), 1.5, None),
    ] {
        let v = serde_json::to_value(snapshot_storage(&rows, &pricing(p.clone()), &read).unwrap()).unwrap();
        assert_eq!(v["monthlyUsd"].as_f64().unwrap(), monthly, "{p}");
        assert_eq!(v.get("named").cloned(), named, "{p}");
    }
}

/// K7: `keep_count` 0 drops the head too; past the chain's length keeps it all and drops nothing (no panic); the
/// named saving is the charge before less the charge after, each floored at 0.
#[test]
fn retention_keeps_any_count_and_saves_by_the_charge_before_and_after() {
    let manifest = serde_json::from_value(json!({
        "head": 4,
        "versions": [version(1, "s1", None), version(2, "s2", Some("s1")), version(3, "s3", Some("s2")), version(4, "s4", Some("s3"))]
    }))
    .unwrap();
    let rows: Vec<SnapshotRow> = (1..=4).map(|n| row(&format!("s{n}"), Some(&format!("wsp-h1-g-v{n}")), 1_000_000_000, "x")).collect();
    let none = |_: &str| Vec::<String>::new();
    let ns = |vs: &Vec<wsp_wire::views::golden_image::GoldenVersion>| vs.iter().map(|v| v.version.0 as u64).collect::<Vec<u64>>();
    for (keep, kept, dropped, saves) in [
        (0, vec![], vec![1, 2, 3, 4], 4.0),
        (1, vec![4], vec![1, 2, 3], 3.0),
        (3, vec![4, 3, 2], vec![1], 1.0),
        (4, vec![4, 3, 2, 1], vec![], 0.0),
        (9, vec![4, 3, 2, 1], vec![], 0.0),
    ] {
        let plan = retention_plan(&manifest, &rows, &none, &pricing(json!({ "freeGb": 0, "usdPerGbMonth": 1 })), Some(keep));
        assert_eq!((ns(&plan.keep), ns(&plan.drop)), (kept, dropped), "keep {keep}");
        assert!((plan.saves_usd_per_month - saves).abs() < 1e-9, "keep {keep}: {}", plan.saves_usd_per_month);
    }
    for (free, saves) in [(0, 1.0), (1, 1.0), (2, 1.0), (3, 0.5), (4, 0.0)] {
        let plan = retention_plan(
            &manifest,
            &rows,
            &none,
            &pricing(json!({ "freeGb": 0, "usdPerGbMonth": 0, "freeNamed": free, "usdPerNamedMonth": 0.5 })),
            None,
        );
        assert_eq!(plan.named_freed, 2);
        assert!((plan.saves_usd_per_month - saves).abs() < 1e-9, "free {free}: {}", plan.saves_usd_per_month);
    }
}
