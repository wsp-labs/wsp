// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts "golden pipeline" (:187-254): the scripted pipeline, prepare then seal, and the manifest it appends to.
#![allow(clippy::type_complexity)]
mod golden_roads_common;

use golden_roads_common::*;
use sha2::{Digest, Sha256};
use wsp_engine::golden::{build_golden, rollback};
use wsp_wire::views::golden_image::MachineKind;

// golden.test.ts:188
#[test]
fn seals_no_version_when_the_smoke_fork_fails_and_leaves_no_machine_or_snapshot_behind() {
    let mut exec_results = indexmap::IndexMap::new();
    exec_results.insert("boom --version".to_string(), exec(127, "", "not found"));
    let rec = recording_backend(Opts { exec_results, ..Opts::default() });
    let clock = Manual::new();
    let err = run(build_golden(build_opts(&rec, "true", "boom --version"), &*clock)).expect_err("the build fails");
    assert!(err.message().contains("smoke"), "{}", err.message());
    assert_eq!(rec.state.deleted_snapshots(), vec!["snap_wsp-h1-default-v1"], "the image that failed smoke does not survive");
    assert_eq!(rec.state.killed(), vec!["m1", "m2"], "builder and smoke fork both gone");
}

// golden.test.ts:199
#[test]
fn writes_a_complete_manifest_entry_sandbox_kind_by_default_and_kills_builder_and_fork() {
    let rec = recording();
    let clock = Manual::new();
    let mut opts = build_opts(&rec, "echo setup", "true");
    opts.base_template = Some("base".to_string());
    let built = run(build_golden(opts, &*clock)).expect("the build seals");
    let version = &built.version;
    assert_eq!(version.version.0, 1.0);
    assert_eq!(version.snapshot_id, "snap_wsp-h1-default-v1");
    assert_eq!(version.base_template, "base");
    assert_eq!(version.kind, Some(MachineKind::Sandbox));
    assert_eq!(version.setup_sha, hex::encode(Sha256::digest(b"echo setup")));
    assert_eq!(version.smoke.cmd, "true");
    assert_eq!(version.smoke.exit_code.0, 0.0);
    assert!(parse_ms(&version.created_at).is_some(), "createdAt parses: {}", version.created_at);
    assert_eq!(built.manifest.head.0, 1.0);
    assert_eq!(built.manifest.versions, vec![version.clone()]);
    let created = rec.state.created();
    assert_eq!(created.iter().map(|c| c.kind).collect::<Vec<_>>(), vec![MachineKind::Sandbox, MachineKind::Sandbox]);
    assert_eq!(created[1].from_snapshot.as_deref(), Some("snap_wsp-h1-default-v1"));
    assert_eq!(rec.state.killed(), vec!["m1", "m2"]);
}

// golden.test.ts:218
#[test]
fn stamps_the_builder_as_wsp_builder_and_the_smoke_fork_as_wsp_smoke_with_its_own_created_at() {
    let rec = recording();
    let clock = Manual::new();
    let labels: indexmap::IndexMap<String, String> = [("wsp", "1"), ("wsp-owner", "h_me"), ("createdAt", "2026-09-01T00:00:00.000Z")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut opts = build_opts(&rec, "true", "true");
    opts.size.labels = Some(labels.clone());
    run(build_golden(opts, &*clock)).expect("the build seals");
    let created = rec.state.created();
    let mut want_builder = labels.clone();
    want_builder.insert("wsp-builder".to_string(), "1".to_string());
    assert_eq!(created[0].labels.as_ref(), Some(&want_builder));
    let smoke = created[1].labels.as_ref().expect("the smoke fork's labels");
    assert_eq!(smoke.get("wsp").map(String::as_str), Some("1"));
    assert_eq!(smoke.get("wsp-owner").map(String::as_str), Some("h_me"));
    assert_eq!(smoke.get("wsp-smoke").map(String::as_str), Some("1"));
    assert!(!smoke.contains_key("wsp-builder"));
    let stamped = parse_ms(smoke.get("createdAt").expect("createdAt")).expect("an ISO time");
    assert!(stamped > parse_ms(&labels["createdAt"]).unwrap());
}

// golden.test.ts:228
#[test]
fn appends_versions_and_rollback_only_moves_head() {
    let rec = recording();
    let clock = Manual::new();
    let one = run(build_golden(build_opts(&rec, "a", "true"), &*clock)).unwrap();
    let mut second = build_opts(&rec, "b", "true");
    second.manifest = Some(one.manifest.clone());
    let two = run(build_golden(second, &*clock)).unwrap();
    assert_eq!(two.manifest.versions.iter().map(|v| v.version.0).collect::<Vec<_>>(), vec![1.0, 2.0]);
    assert_eq!(two.manifest.head.0, 2.0);
    let rolled = rollback(&two.manifest, 1).unwrap();
    assert_eq!(rolled.head.0, 1.0);
    assert_eq!(rolled.versions.len(), 2);
    assert_eq!(two.manifest.head.0, 2.0, "input untouched");
}
