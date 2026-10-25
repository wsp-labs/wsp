// SPDX-License-Identifier: AGPL-3.0-only
//! This computer as the local machine, against the TS code's own answers (tests/fixtures/roads/local_backend.json).
//! Spec: spec/local_backend.md.
mod roads_common;

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use roads_common::*;
use serde_json::json;
use wsp_engine::local_backend::*;
use wsp_engine::machine::{MachineBackend, Size};

#[test]
fn shapes_match_the_ts() {
    let want = fixture("local_backend.json")["shape"].clone();
    let inputs: [(u32, u64); 6] = [(10, 16_654_508_032), (10, 16 << 30), (8, 8 << 30), (1, 512 << 20), (4, 1536 << 20), (2, 0)];
    for ((cores, bytes), want) in inputs.iter().zip(want.as_array().unwrap()) {
        let got = local_shape(Some(*cores), Some(*bytes));
        assert_eq!(
            json!({ "cpu": got.cpu, "memMb": got.mem_mb }),
            json!({ "cpu": want["cpu"].as_f64(), "memMb": want["memMb"].as_f64() }),
            "{cores} {bytes}"
        );
    }
}

#[test]
fn meminfo_and_pressure_match_the_ts() {
    let d = fixture("local_backend.json");
    let meminfo = [
        "MemTotal:       16303412 kB\nMemFree: 1 kB\nMemAvailable:    9216000 kB\n",
        "MemTotal: 10 kB\n",
        "MemAvailable: 2047 kB\nMemTotal: 2048 kB\n",
        " MemTotal: 2048 kB\nMemAvailable: 1024 kB\n",
        "MemTotal:2048kB\nMemAvailable:\t1024 kB",
    ];
    let as_json = |m: Option<MemoryHere>| m.map(|m| json!({ "freeMb": m.free_mb, "totalMb": m.total_mb })).unwrap_or(json!(null));
    for (text, want) in meminfo.iter().zip(d["meminfo"].as_array().unwrap()) {
        assert_eq!(as_json(memory_of_meminfo(text)), *want, "{text:?}");
    }
    for case in d["pressure"].as_array().unwrap() {
        let level = case[0].as_str().unwrap();
        assert_eq!(as_json(memory_of_pressure(level, 16 << 30)), case[1], "{level:?}");
    }
}

#[tokio::test]
async fn building_the_backend_makes_no_folder_and_get_makes_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("not").join("yet");
    let backend = LocalBackend::new(LocalBackendOptions { root: root.clone(), env: None });
    assert!(!root.exists());
    assert_eq!(backend.folder, root);
    let first = backend.get("local").await.unwrap();
    assert!(root.is_dir());
    // One computer, one machine: every get answers the same one.
    assert!(Arc::ptr_eq(&first, &backend.get("anything").await.unwrap()));
    assert_eq!(backend.work_folder().unwrap(), root.as_path());
}

#[tokio::test]
async fn bytes_over_a_file_already_there_keep_its_mode() {
    let dir = tempfile::tempdir().unwrap();
    let machine = LocalBackend::new(LocalBackendOptions { root: dir.path().to_path_buf(), env: None }).get("local").await.unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, "old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(machine.put_bytes(path.to_str().unwrap(), b"A", None).await.unwrap(), None);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "A");
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o644);
}

#[tokio::test]
async fn commands_run_under_the_env_the_backend_was_given() {
    let dir = tempfile::tempdir().unwrap();
    let env =
        indexmap::IndexMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string()), ("WSP_ROADS_PROBE".to_string(), "given".to_string())]);
    let machine = LocalBackend::new(LocalBackendOptions { root: dir.path().to_path_buf(), env: Some(env) }).get("local").await.unwrap();
    let said = machine.exec("printf %s \"$WSP_ROADS_PROBE:$HOME\"", Default::default()).await.unwrap();
    // Only what was given: no HOME of this process's leaks in.
    assert_eq!((said.exit_code, said.stdout.as_str()), (0.into(), "given:"));
}

#[test]
fn pricing_and_capabilities_key_order_match_the_ts() {
    let backend = LocalBackend::new(LocalBackendOptions { root: "/nowhere/wsp".into(), env: None });
    assert_eq!(
        serde_json::to_string(&backend.capabilities()).unwrap(),
        r#"{"liveCloneForks":false,"replacesMachine":false,"previewUrls":false,"signedUrls":false,"callbackRelay":false,"diskSnapshots":false,"images":false,"snapshotsAnyLife":false,"snapshotListing":false,"templates":false,"sizes":[],"kept":true,"copies":true,"ownNetwork":false}"#
    );
    let pricing = backend.pricing();
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 64.0, mem_mb: 1.0 }), 0.0);
    assert_eq!(
        serde_json::to_value(&pricing.snapshot_storage().wire).unwrap(),
        json!({ "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" })
    );
    assert!(backend.lifecycle().is_none());
    assert_eq!(backend.offers(), Default::default());
}

#[tokio::test]
async fn memory_here_reads_this_kernel_and_nothing_elsewhere() {
    assert_eq!(memory_here_on("win32").await, None);
    assert_eq!(memory_here_on("freebsd").await, None);
    let here = memory_here_on("linux").await.unwrap();
    let total_kb: u64 = std::fs::read_to_string("/proc/meminfo")
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .unwrap()
        .trim()
        .trim_end_matches(" kB")
        .trim()
        .parse()
        .unwrap();
    assert_eq!(here.total_mb, total_kb / 1024);
    assert!(here.free_mb <= here.total_mb);
    let _ = res(0, "", "");
}
