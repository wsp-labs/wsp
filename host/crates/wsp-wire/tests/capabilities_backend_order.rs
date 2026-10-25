// SPDX-License-Identifier: AGPL-3.0-only
//! Each backend's capabilities write their keys in the order its TS builds the object (engine/src/*-backend.ts). Every expected
//! text is `JSON.stringify` of an object literal copied from that backend, run under node 22.23.4.

use wsp_wire::types::JsNumber;
use wsp_wire::wire::capabilities::{Capabilities, MachineSizeOffer, PauseMode};

fn none(kept: bool, copies: bool, sizes_last: bool) -> Capabilities {
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
        kept,
        copies,
        own_network: false,
        sizes: Vec::new(),
        sizes_last,
    }
}

fn offer(name: Option<&str>, cpu: f64, mem_mb: f64, rate: f64) -> MachineSizeOffer {
    MachineSizeOffer { name: name.map(str::to_string), cpu: JsNumber(cpu), mem_mb: JsNumber(mem_mb), rate_usd_per_hour: JsNumber(rate) }
}

fn written(capabilities: &Capabilities) -> String {
    serde_json::to_string(capabilities).unwrap()
}

const OFF: &str = r#""liveCloneForks":false,"replacesMachine":false,"previewUrls":false,"signedUrls":false,"callbackRelay":false,"diskSnapshots":false,"images":false,"snapshotsAnyLife":false,"snapshotListing":false,"templates":false"#;

#[test]
fn the_local_backend_writes_sizes_before_kept() {
    let want = format!(r#"{{{OFF},"sizes":[],"kept":true,"copies":true,"ownNetwork":false}}"#);
    assert_eq!(written(&none(true, true, false)), want);
}

#[test]
fn the_place_folder_backend_writes_sizes_before_kept() {
    let want = format!(r#"{{{OFF},"sizes":[],"kept":true,"copies":false,"ownNetwork":false}}"#);
    assert_eq!(written(&none(true, false, false)), want);
}

#[test]
fn the_no_provider_backend_writes_sizes_before_kept() {
    let want = format!(r#"{{{OFF},"sizes":[],"kept":false,"copies":false,"ownNetwork":false}}"#);
    assert_eq!(written(&none(false, false, false)), want);
}

#[test]
fn the_fake_backend_writes_sizes_before_kept() {
    let fake = Capabilities {
        live_clone_forks: true,
        pause_mode: Some(PauseMode::Memory),
        replaces_machine: true,
        preview_urls: true,
        signed_urls: false,
        callback_relay: false,
        disk_snapshots: true,
        images: true,
        snapshots_any_life: true,
        snapshot_listing: true,
        templates: false,
        kept: false,
        copies: true,
        own_network: true,
        sizes: vec![offer(None, 2.0, 4096.0, 0.08), offer(None, 4.0, 8192.0, 0.16), offer(None, 8.0, 16384.0, 0.32)],
        sizes_last: false,
    };
    let want = r#"{"liveCloneForks":true,"pauseMode":"memory","replacesMachine":true,"previewUrls":true,"signedUrls":false,"callbackRelay":false,"diskSnapshots":true,"images":true,"snapshotsAnyLife":true,"snapshotListing":true,"templates":false,"sizes":[{"cpu":2,"memMb":4096,"rateUsdPerHour":0.08},{"cpu":4,"memMb":8192,"rateUsdPerHour":0.16},{"cpu":8,"memMb":16384,"rateUsdPerHour":0.32}],"kept":false,"copies":true,"ownNetwork":true}"#;
    assert_eq!(written(&fake), want);
}

#[test]
fn solari_writes_sizes_last() {
    let solari = Capabilities {
        live_clone_forks: true,
        pause_mode: Some(PauseMode::Memory),
        replaces_machine: true,
        preview_urls: true,
        signed_urls: true,
        callback_relay: true,
        disk_snapshots: true,
        images: true,
        snapshots_any_life: false,
        snapshot_listing: true,
        templates: true,
        kept: false,
        copies: true,
        own_network: true,
        sizes: vec![offer(None, 2.0, 4096.0, 2.0 * 0.035 + 4.0 * 0.01)],
        sizes_last: true,
    };
    let want = r#"{"liveCloneForks":true,"pauseMode":"memory","replacesMachine":true,"previewUrls":true,"signedUrls":true,"callbackRelay":true,"diskSnapshots":true,"images":true,"snapshotsAnyLife":false,"snapshotListing":true,"templates":true,"kept":false,"copies":true,"ownNetwork":true,"sizes":[{"cpu":2,"memMb":4096,"rateUsdPerHour":0.11000000000000001}]}"#;
    assert_eq!(written(&solari), want);
}

#[test]
fn a_box_writes_sizes_last_and_each_size_name_first() {
    let boxed = Capabilities {
        live_clone_forks: false,
        pause_mode: Some(PauseMode::Disk),
        replaces_machine: true,
        preview_urls: true,
        signed_urls: false,
        callback_relay: true,
        disk_snapshots: true,
        images: true,
        snapshots_any_life: true,
        snapshot_listing: true,
        templates: true,
        kept: false,
        copies: true,
        own_network: true,
        sizes: vec![offer(Some("small"), 2.0, 4096.0, 0.018), offer(Some("default"), 4.0, 8192.0, 0.036)],
        sizes_last: true,
    };
    let want = r#"{"liveCloneForks":false,"pauseMode":"disk","replacesMachine":true,"previewUrls":true,"signedUrls":false,"callbackRelay":true,"diskSnapshots":true,"images":true,"snapshotsAnyLife":true,"snapshotListing":true,"templates":true,"kept":false,"copies":true,"ownNetwork":true,"sizes":[{"name":"small","cpu":2,"memMb":4096,"rateUsdPerHour":0.018},{"name":"default","cpu":4,"memMb":8192,"rateUsdPerHour":0.036}]}"#;
    assert_eq!(written(&boxed), want);
}

#[test]
fn a_frame_read_keeps_the_order_it_came_in() {
    for want in [
        format!(r#"{{{OFF},"sizes":[],"kept":true,"copies":true,"ownNetwork":false}}"#),
        format!(r#"{{{OFF},"kept":true,"copies":true,"ownNetwork":false,"sizes":[]}}"#),
    ] {
        let read: Capabilities = serde_json::from_str(&want).unwrap();
        assert_eq!(written(&read), want);
    }
}
