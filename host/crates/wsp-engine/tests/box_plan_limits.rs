// SPDX-License-Identifier: AGPL-3.0-only
//! The class table, plan sizes and the account's limits read off `GET /limits`: the pure functions of
//! box_backend/plan.rs. Ported from packages/engine/test/box-limits.test.ts (the three pure cases) and the class
//! table case of box-backend.test.ts.
use serde_json::{json, Value};
use wsp_engine::box_backend::{limits_of, place_limits_of, BoxClass, BoxClassType, BoxPricing, BOX_CLASSES};
use wsp_engine::machine::{BackendPricing, Size};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::place_more::{CloudLimits, CloudPlan, CloudStarts, StartsWindow};

/// box-limits.test.ts:8 `OWNER_LIMITS`: GET /limits on the owner's account, 2026-10-11 17:06 UTC, trimmed.
fn owner_limits() -> Value {
    json!({
        "accessTier": "standard",
        "maxActiveSandboxes": 100,
        "activeSandboxes": 11,
        "startLimits": { "perMinute": 12, "perHour": 60, "perDay": 200 },
        "starts": {
            "unlimited": false,
            "minute": { "limit": 12, "used": 0, "remaining": 12 },
            "hour": { "limit": 60, "used": 9, "remaining": 51 },
            "day": { "limit": 200, "used": 187, "remaining": 13 }
        },
        "activeStates": ["provisioned", "cloning", "ready", "idle", "running"],
        "canStart": true,
        "startBlockedReason": null,
        "creditBalanceHours": 81.6,
        "sandboxPlanKey": "box_20"
    })
}

/// box-limits.test.ts:24 `TRIAL_LIMITS`: two machines, 5, 25 and 75 starts.
fn trial_limits() -> Value {
    json!({
        "accessTier": "trial",
        "maxActiveSandboxes": 2,
        "starts": {
            "minute": { "limit": 5, "used": 1, "remaining": 4 },
            "hour": { "limit": 25, "used": 3, "remaining": 22 },
            "day": { "limit": 75, "used": 3, "remaining": 72 }
        }
    })
}

fn window(used: i64, limit: i64) -> StartsWindow {
    StartsWindow { used, limit }
}

fn sizes(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

/// box-backend.test.ts:1462
#[test]
fn the_class_table_is_boats_four_sizes_xlarge_at_0_200_and_named_snapshots_past_10_cost_1_70() {
    let class = |kind, name, cpu, mem_mb, disk_gb, rate_usd_per_hour| BoxClass { kind, name, cpu, mem_mb, disk_gb, rate_usd_per_hour };
    assert_eq!(
        BOX_CLASSES.to_vec(),
        vec![
            class(BoxClassType::Small, "small", 2.0, 4096.0, 12.0, 0.018),
            class(BoxClassType::Default, "default", 4.0, 8192.0, 50.0, 0.036),
            class(BoxClassType::Large, "large", 8.0, 16384.0, 125.0, 0.072),
            class(BoxClassType::Xlarge, "extra large", 16.0, 32768.0, 251.0, 0.2),
        ]
    );
    assert_eq!(BoxPricing.rate_usd_per_hour(Size { cpu: 16.0, mem_mb: 32768.0 }), 0.2);
    let storage = BoxPricing.snapshot_storage();
    assert_eq!(storage.free_named, Some(10));
    assert_eq!(storage.usd_per_named_month, Some(1.7));
    // box-backend.test.ts:1470-1476 also priced a snapshot list through snapshotStorage (snapshot-storage.ts), which
    // is another slice's: 10 named plus a history row costs $0, 13 named plus one costs 3 x 1.7.
}

/// box-limits.test.ts:31
#[test]
fn reads_the_owners_plan_machines_at_once_sizes_sold_and_starts_used_in_each_window() {
    assert_eq!(
        place_limits_of(&limits_of(&owner_limits())),
        Some(CloudLimits {
            plan: CloudPlan { key: Some("box_20".into()), trial: false, machines_at_once: 100, sizes: sizes(&["2x4", "4x8", "8x16"]), running: Some(11) },
            starts: Some(CloudStarts { minute: window(0, 12), hour: window(9, 60), day: window(187, 200) }),
            credit_hours: Some(JsNumber(81.6)),
        })
    );
}

/// box-limits.test.ts:39
#[test]
fn sells_16x32_from_the_100_plan_up_and_two_sizes_on_the_trial() {
    let mut big = owner_limits();
    big["sandboxPlanKey"] = json!("box_100");
    let plan = place_limits_of(&limits_of(&big)).expect("a machine cap was named").plan;
    assert_eq!(plan.sizes, sizes(&["2x4", "4x8", "8x16", "16x32"]));
    assert_eq!(
        place_limits_of(&limits_of(&trial_limits())),
        Some(CloudLimits {
            plan: CloudPlan { key: None, trial: true, machines_at_once: 2, sizes: sizes(&["2x4", "4x8"]), running: None },
            starts: Some(CloudStarts { minute: window(1, 5), hour: window(3, 25), day: window(3, 75) }),
            credit_hours: None,
        })
    );
}

/// box-limits.test.ts:47
#[test]
fn carries_nothing_for_a_read_that_names_no_machine_cap() {
    let mut rest = owner_limits();
    rest.as_object_mut().unwrap().shift_remove("maxActiveSandboxes");
    assert_eq!(place_limits_of(&limits_of(&rest)), None);
}
