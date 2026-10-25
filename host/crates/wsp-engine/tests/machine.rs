// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/no-provider-backend.test.ts, and the "box" and "no provider" rows of
//! copies-flags.test.ts (the other rows read backends of other slices).
use wsp_engine::box_backend::{BoxBackend, BoxBackendOptions};
use wsp_engine::machine::{Capabilities, MachineBackend, MachineKind, MachineSpec, Size};
use wsp_engine::no_provider_backend::NoProviderBackend;
use wsp_words::words::thread::NO_PROVIDER_LINE;

fn spec() -> MachineSpec {
    MachineSpec {
        kind: MachineKind::Sandbox,
        template: None,
        from_snapshot: None,
        cpu: None,
        mem_mb: None,
        disk_gb: None,
        envs: None,
        labels: None,
        on_idle: None,
        idle_timeout_ms: None,
        idempotency_key: None,
        engine: None,
        copy: None,
        shares: None,
        binds: None,
    }
}

/// `Capabilities.parse(row.capabilities)`: what the schema reads back from the backend's own answer.
fn parsed(capabilities: Capabilities) -> Capabilities {
    serde_json::from_value(serde_json::to_value(capabilities).unwrap()).unwrap()
}

/// no-provider-backend.test.ts:9
#[test]
fn every_capability_is_false_and_it_offers_no_sizes() {
    assert_eq!(
        NoProviderBackend::new(None).capabilities(),
        Capabilities {
            live_clone_forks: false,
            pause_mode: None,
            replaces_machine: false,
            preview_urls: false,
            signed_urls: false,
            callback_relay: false,
            disk_snapshots: false,
            images: false,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            sizes: vec![],
            // Nothing here is a machine of the person's; there is no machine at all.
            kept: false,
            // Nothing forks and nothing copies: there is nothing to copy onto.
            copies: false,
            own_network: false,
        }
    );
}

/// no-provider-backend.test.ts:30
#[tokio::test]
async fn it_holds_nothing_so_the_listing_is_empty_rather_than_a_refusal() {
    assert_eq!(NoProviderBackend::new(None).list(None).await.unwrap(), vec![]);
}

/// no-provider-backend.test.ts:34
#[tokio::test]
async fn every_road_that_would_touch_a_machine_says_how_to_get_one() {
    let backend = NoProviderBackend::new(None);
    let e = backend.create(spec(), None, None).await.err().expect("create refuses");
    assert!(e.to_string().contains(NO_PROVIDER_LINE), "{e}");
    let e = backend.get("m1").await.err().expect("get refuses");
    assert!(e.to_string().contains(NO_PROVIDER_LINE), "{e}");
    let e = backend.delete_snapshot("snap_x").await.unwrap_err();
    assert!(e.to_string().contains(NO_PROVIDER_LINE), "{e}");
    // The road out is the verb off the front page, never a shell variable: this is the first refusal a person who
    // typed wsp new on a fresh home reads, and one provider's variable is not the answer for the others.
    assert!(NO_PROVIDER_LINE.contains("wsp add <provider>"));
    assert!(!NO_PROVIDER_LINE.contains("SOLARI_API_KEY"));
    assert!(NO_PROVIDER_LINE.contains("wsp add"));
}

/// no-provider-backend.test.ts:46
#[test]
fn nothing_bills_on_it_and_it_names_no_size_to_bill_for() {
    let pricing = NoProviderBackend::new(None).pricing();
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 4.0, mem_mb: 8192.0 }), 0.0);
    assert_eq!(pricing.default_size(), Size { cpu: 0.0, mem_mb: 0.0 });
}

/// copies-flags.test.ts:20 (the "box" row of `ROWS`, read by the case at :27).
#[test]
fn copies_flags_box_copies_and_its_copy_has_its_own_network() {
    let backend = BoxBackend::new(BoxBackendOptions { api_key: "sk-ant-x".into(), ..Default::default() });
    let capabilities = parsed(backend.capabilities());
    assert!(capabilities.copies, "box copies");
    assert!(capabilities.own_network, "box own_network");
}

/// copies-flags.test.ts:22 (the "no provider" row of `ROWS`, read by the case at :27).
#[test]
fn copies_flags_no_provider_copies_nothing_and_has_no_network_of_its_own() {
    let capabilities = parsed(NoProviderBackend::new(None).capabilities());
    assert!(!capabilities.copies, "no provider copies");
    assert!(!capabilities.own_network, "no provider own_network");
}
