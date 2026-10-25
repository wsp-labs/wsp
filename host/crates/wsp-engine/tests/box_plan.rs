// SPDX-License-Identifier: AGPL-3.0-only
//! The "BoxBackend declarations" describe of packages/engine/test/box-backend.test.ts: what a box declares about
//! itself before any call goes out. Nothing here dials the API.
mod common;

use indexmap::IndexMap;
use wsp_engine::box_backend::{
    box_budgets, box_class_for, box_labels, box_name, box_snapshot_name, BoxBudgetsAsk, BoxClassType, BoxPricing, SizeAsk, BOX_BASE_TEMPLATE,
    BOX_CLASSES, BOX_INLINE_MAX_MS, BOX_NAME_MAX, SNAPSHOT_NAME_MAX,
};
use wsp_engine::machine::{BackendPricing, MachineBackend, MachineKind, Size, SnapshotStoragePricing, WireSnapshotStoragePricing};
use wsp_engine::machine::{Capabilities, LifecycleBudgets};
use wsp_wire::types::JsNumber;
use wsp_wire::wire::capabilities::{MachineSizeOffer, PauseMode};

use common::fake_box::{backend_on, FakeBox};

// exec-detached.ts:18
const INLINE_EXEC_MS: u64 = 20_000;
// labels.ts
const WSP_LABEL: &str = "wsp";
const OWNER_LABEL: &str = "wsp-owner";
const WORKSPACE_LABEL: &str = "wsp-workspace";
const NAME_LABEL: &str = "wsp-name";
const GOLDEN_LABEL: &str = "wsp-golden";
const CREATED_AT_LABEL: &str = "createdAt";
const BUILDER_LABEL: &str = "wsp-builder";

fn n(v: f64) -> JsNumber {
    JsNumber(v)
}

fn labels(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn offer(name: &str, cpu: f64, mem_mb: f64, rate: f64) -> MachineSizeOffer {
    MachineSizeOffer { cpu: n(cpu), mem_mb: n(mem_mb), rate_usd_per_hour: n(rate), name: Some(name.to_string()) }
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// box-backend.test.ts:181
#[tokio::test]
async fn says_what_a_box_can_and_cannot_do() {
    let on = backend_on(&FakeBox::new(), "trial", BoxBudgetsAsk::default());
    let expected = Capabilities {
        live_clone_forks: false,
        pause_mode: Some(PauseMode::Disk),
        // A fresh box deployed from the snapshot stands in for one that is gone, which is what the rebuild gate reads.
        replaces_machine: true,
        preview_urls: true,
        signed_urls: false,
        callback_relay: true,
        disk_snapshots: true,
        images: true,
        // A box's named snapshot is the disk as it stands, so one that stopped and booted again saves the same.
        snapshots_any_life: true,
        snapshot_listing: true,
        templates: true,
        kept: false,
        // The project's checkout is copied into the fork by the box's own runtime, and each workspace there gets a
        // network and published ports of its own.
        copies: true,
        own_network: true,
        sizes: vec![
            offer("small", 2.0, 4096.0, 0.018),
            offer("default", 4.0, 8192.0, 0.036),
            offer("large", 8.0, 16384.0, 0.072),
            offer("extra large", 16.0, 32768.0, 0.2),
        ],
    };
    assert_eq!(on.backend.capabilities(), expected);
    let templates = on.backend.base_templates().expect("a box boots from a base template");
    assert_eq!(templates.sandbox, BOX_BASE_TEMPLATE);
    assert_eq!(templates.desktop, BOX_BASE_TEMPLATE);
}

/// box-backend.test.ts:212
#[test]
fn prices_by_class_small_to_xlarge_a_stopped_box_nothing_named_snapshots_past_the_free_ten() {
    let pricing = BoxPricing;
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 2.0, mem_mb: 4096.0 }), 0.018);
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 4.0, mem_mb: 8192.0 }), 0.036);
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 8.0, mem_mb: 16384.0 }), 0.072);
    assert_eq!(pricing.default_size(), Size { cpu: 2.0, mem_mb: 4096.0 });
    assert_eq!(pricing.default_size(), Size { cpu: BOX_CLASSES[0].cpu, mem_mb: BOX_CLASSES[0].mem_mb });
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 16.0, mem_mb: 32768.0 }), 0.2);
    assert_eq!(
        pricing.snapshot_storage(),
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing { free_gb: n(0.0), usd_per_gb_month: n(0.0), billed_from: String::new() },
            free_named: Some(10),
            usd_per_named_month: Some(1.7),
        }
    );
    assert_eq!(pricing.builder_disk_gb(), None);
    for c in BOX_CLASSES {
        assert_eq!(pricing.rate_usd_per_hour(Size { cpu: c.cpu, mem_mb: c.mem_mb }), c.rate_usd_per_hour);
    }
}

/// box-backend.test.ts:224
#[test]
fn reads_a_size_onto_the_class_table_the_match_else_the_smallest_that_holds_it_else_the_largest() {
    let ask = |cpu: Option<f64>, mem_mb: Option<f64>| box_class_for(SizeAsk { cpu, mem_mb }).kind;
    assert_eq!(ask(Some(2.0), Some(4096.0)), BoxClassType::Small);
    assert_eq!(ask(Some(4.0), Some(8192.0)), BoxClassType::Default);
    assert_eq!(ask(Some(2.0), Some(8192.0)), BoxClassType::Default);
    assert_eq!(ask(Some(3.0), Some(1024.0)), BoxClassType::Default);
    assert_eq!(ask(Some(16.0), Some(16384.0)), BoxClassType::Xlarge);
    assert_eq!(ask(Some(32.0), Some(65536.0)), BoxClassType::Xlarge);
    assert_eq!(ask(None, None), BoxClassType::Small);
}

/// box-backend.test.ts:234
#[test]
fn an_inline_command_is_bounded_under_the_edges_cut_and_above_the_detached_roads_own_execs() {
    assert!(BOX_INLINE_MAX_MS > INLINE_EXEC_MS);
    assert!(BOX_INLINE_MAX_MS < 70_000);
}

/// box-backend.test.ts:239
#[tokio::test]
async fn declares_its_lifecycle_one_wake_attempt_fifteen_minutes_no_asking_again_a_backstop() {
    let on = backend_on(&FakeBox::new(), "trial", BoxBudgetsAsk::default());
    let lifecycle = on.backend.lifecycle().expect("a box declares a lifecycle");
    let expected = LifecycleBudgets { wake_attempts: 1, daemon_answers_ms: n(900_000.0), resume_asks: None };
    assert_eq!(lifecycle.budgets(), expected);
    assert_eq!(box_budgets(), expected);
    assert!(lifecycle.has_backstop());
    assert_eq!(on.backend.capabilities().pause_mode, Some(PauseMode::Disk));
}

/// box-backend.test.ts:248
#[test]
fn a_machines_labels_ride_in_the_box_name_shortened_and_read_back_whole() {
    let all = labels(&[
        (WSP_LABEL, "1"),
        (OWNER_LABEL, "h_1a2b3c4d"),
        (WORKSPACE_LABEL, "ws_0f9e8d7c"),
        (NAME_LABEL, "dev;two=three%"),
        (GOLDEN_LABEL, "default"),
        (CREATED_AT_LABEL, "2026-09-11T12:00:00.000Z"),
        ("recipe-tag", "x"),
    ]);
    let name = box_name(&all);
    assert_eq!(name, "wsp;o=h_1a2b3c4d;w=ws_0f9e8d7c;t=2026-09-11T12:00:00.000Z;g=default;n=dev%3btwo%3dthree%25;recipe-tag=x");
    assert!(utf16_len(&name) <= BOX_NAME_MAX);
    assert_eq!(box_labels(Some(&name)), Some(all));
    // Boxes wsp did not name carry no labels, and a name with the mark alone is wsp's with the one label.
    assert_eq!(box_labels(Some("Box 2026-09-11 06:28")), None);
    assert_eq!(box_labels(None), None);
    assert_eq!(box_labels(Some("wsp")), Some(labels(&[(WSP_LABEL, "1")])));
}

/// box-backend.test.ts:268
#[test]
fn a_name_past_the_apis_cap_keeps_the_pairs_in_rank_order_up_to_the_first_that_does_not_fit() {
    let all = labels(&[
        (OWNER_LABEL, "h_1a2b3c4d"),
        (WORKSPACE_LABEL, "ws_0f9e8d7c"),
        (CREATED_AT_LABEL, "2026-09-11T12:00:00.000Z"),
        (BUILDER_LABEL, "1"),
        (GOLDEN_LABEL, &"g".repeat(70)),
        (NAME_LABEL, "n"),
    ]);
    let name = box_name(&all);
    assert!(utf16_len(&name) <= BOX_NAME_MAX);
    let read = box_labels(Some(&name)).expect("the name carries wsp's mark");
    assert_eq!(
        read,
        labels(&[
            (WSP_LABEL, "1"),
            (OWNER_LABEL, "h_1a2b3c4d"),
            (WORKSPACE_LABEL, "ws_0f9e8d7c"),
            (CREATED_AT_LABEL, "2026-09-11T12:00:00.000Z"),
            (BUILDER_LABEL, "1"),
        ])
    );
    // The golden did not fit, so the one-letter name after it is left off too, though it would have: a reader who
    // finds no golden knows the name is missing as well rather than guessing which pairs went.
    assert_eq!(read.get(NAME_LABEL), None);
}

/// box-backend.test.ts:279
#[test]
fn a_snapshot_name_is_lowered_onto_the_providers_pattern_and_a_long_one_ends_in_a_hash() {
    // snapshot-names.ts:23 goldenName(host, golden, version) is `wsp-<host>-<golden>-v<version>`.
    let golden = |host: &str, golden: &str, version: u32| format!("wsp-{host}-{golden}-v{version}");
    assert_eq!(box_snapshot_name(&golden("mac", "default", 3)).unwrap(), "wsp-mac-default-v3");
    assert_eq!(box_snapshot_name("wsp-mac-Default_v3").unwrap(), "wsp-mac-default-v3");
    let project = format!("project-{}", "x".repeat(80));
    let a = box_snapshot_name(&golden("mac", &project, 12)).unwrap();
    let b = box_snapshot_name(&golden("mac", &project, 13)).unwrap();
    assert!(utf16_len(&a) <= SNAPSHOT_NAME_MAX);
    let mut chars = a.chars();
    let first = chars.next().expect("a name");
    assert!(first.is_ascii_lowercase() || first.is_ascii_digit(), "{a}");
    assert!(a.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{a}");
    assert_ne!(a, b);
    // snapshot-names.ts:29 nameOwner reads the host segment after the `wsp-` mark.
    assert!(a.starts_with("wsp-mac-"), "{a}");
    let e = box_snapshot_name("---").unwrap_err();
    assert!(e.to_string().contains("nothing a named snapshot"), "{e}");
}
