// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/local-backend.test.ts. Spec: spec/local_backend.md.
use std::path::Path;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;

use wsp_engine::errors::Result;
use wsp_engine::land_bytes::{land_bytes, LandBytesOptions};
use wsp_engine::local_backend::{
    local_shape, memory_here, memory_here_on, memory_of_meminfo, memory_of_pressure, LocalBackend, LocalBackendOptions, MemoryHere,
    LOCAL_MACHINE_ID,
};
use wsp_engine::machine::{
    Capabilities, ExecOptions, MachineBackend, MachineKind, MachineLife, MachineListRow, MachineSpec, MachineState, OnLine, RunOptions,
    Size, SnapshotOptions,
};

fn backend_in(root: &Path) -> LocalBackend {
    LocalBackend::new(LocalBackendOptions { root: root.to_path_buf(), env: None })
}

fn err_text<T>(r: Result<T>) -> String {
    match r {
        Ok(_) => panic!("expected a failure, got a success"),
        Err(e) => e.to_string(),
    }
}

fn gib(n: u64) -> u64 {
    n * 1024 * 1024 * 1024
}

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

#[tokio::test]
async fn every_capability_a_provider_fork_has_and_a_computer_does_not_is_false() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    assert_eq!(
        backend.capabilities(),
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
            kept: true,
            copies: true,
            own_network: false,
        }
    );
    assert_eq!(backend.pricing().rate_usd_per_hour(Size { cpu: 2.0, mem_mb: 4096.0 }), 0.0);
}

#[tokio::test]
async fn this_computers_size_is_its_cores_and_its_memory_in_whole_gb() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(local_shape(Some(10), Some(16_654_508_032)), Size { cpu: 10.0, mem_mb: 16_384.0 });
    assert_eq!(local_shape(Some(10), Some(gib(16))), Size { cpu: 10.0, mem_mb: 16_384.0 });
    assert_eq!(local_shape(Some(8), Some(gib(8))), Size { cpu: 8.0, mem_mb: 8_192.0 });
    assert_eq!(backend_in(root.path()).pricing().default_size().mem_mb % 1024.0, 0.0);
}

#[tokio::test]
async fn get_and_list_answer_with_the_one_machine() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    let machine = backend.get(LOCAL_MACHINE_ID).await.unwrap();
    assert_eq!(machine.id(), LOCAL_MACHINE_ID);
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
    let listed = backend.list(None).await.unwrap();
    assert_eq!(
        listed,
        vec![MachineListRow { id: LOCAL_MACHINE_ID.to_string(), state: MachineState::Running, labels: IndexMap::new(), size: None }]
    );
}

/// `^(macOS \d|[A-Z][A-Za-z]+ )`
fn names_a_system(os: &str) -> bool {
    if let Some(rest) = os.strip_prefix("macOS ") {
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    let mut chars = os.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_uppercase()) {
        return false;
    }
    let rest: String = chars.collect();
    let letters = rest.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    letters >= 1 && rest[letters..].starts_with(' ')
}

#[tokio::test(flavor = "multi_thread")]
async fn facts_say_what_this_computer_is() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    let machine = backend.get(LOCAL_MACHINE_ID).await.unwrap();
    assert!(machine.offers().facts);
    let facts = machine.facts().await.unwrap();
    assert!(names_a_system(&facts.os), "{}", facts.os);
    assert_ne!(facts.os, "");
    assert!(facts.uptime_ms.0 > 0.0);
    assert_eq!(facts.folder, root.path().to_str().unwrap());
    assert_eq!(machine.facts().await.unwrap().os, facts.os);
}

#[tokio::test(flavor = "multi_thread")]
async fn exec_runs_a_shell_command_on_this_computer_under_the_workspace_folder() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    let machine = backend.get(LOCAL_MACHINE_ID).await.unwrap();
    let ok = machine.exec("printf hi; pwd", ExecOptions::default()).await.unwrap();
    assert_eq!(ok.exit_code, 0);
    assert_eq!(ok.stdout, format!("hi{}\n", std::fs::canonicalize(root.path()).unwrap().display()));
    let bad = machine.exec("exit 7", ExecOptions::default()).await.unwrap();
    assert_eq!(bad.exit_code, 7);
}

#[tokio::test(flavor = "multi_thread")]
async fn exec_hands_the_bytes_it_is_given_to_the_commands_stdin() {
    let root = tempfile::tempdir().unwrap();
    let machine = backend_in(root.path()).get(LOCAL_MACHINE_ID).await.unwrap();
    let opts = ExecOptions { timeout_ms: Some(5_000), stdin: Some(b"given\n".to_vec()), ..Default::default() };
    let read = machine.exec("cat; printf end", opts).await.unwrap();
    assert_eq!(read.stdout, "given\nend");
}

#[tokio::test(flavor = "multi_thread")]
async fn files_read_and_write_under_the_workspace_folder() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    let machine = backend.get(LOCAL_MACHINE_ID).await.unwrap();
    let wrote = machine.exec("printf pong > note.txt", ExecOptions::default()).await.unwrap();
    assert_eq!(wrote.exit_code, 0);
    assert_eq!(std::fs::read_to_string(root.path().join("note.txt")).unwrap(), "pong");
    let read = machine.exec("cat note.txt", ExecOptions::default()).await.unwrap();
    assert_eq!(read.stdout, "pong");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_streams_each_line() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    let machine = backend.get(LOCAL_MACHINE_ID).await.unwrap();
    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let heard = lines.clone();
    let on_line: OnLine = Arc::new(move |line: &str| heard.lock().unwrap().push(line.to_string()));
    let opts = RunOptions { deadline_ms: 5000, on_line: Some(on_line), ..Default::default() };
    let res = machine.run("printf 'a\\nb\\n'; exit 3", opts).await.unwrap();
    assert_eq!(*lines.lock().unwrap(), ["a", "b"]);
    assert_eq!(res.exit_code, 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_past_its_deadline_is_killed_and_answers_124() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    let machine = backend.get(LOCAL_MACHINE_ID).await.unwrap();
    let res = machine.run("sleep 5", RunOptions { deadline_ms: 150, ..Default::default() }).await.unwrap();
    assert_eq!(res.exit_code, 124);
}

#[tokio::test]
async fn the_moves_only_a_provider_fork_takes_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let backend = backend_in(root.path());
    let machine = backend.get(LOCAL_MACHINE_ID).await.unwrap();
    let life = MachineLife { first_life: true };
    assert_eq!(err_text(machine.snapshot("x", life, SnapshotOptions::default()).await), "this computer cannot be snapshotted");
    assert_eq!(err_text(machine.pause(None).await), "this computer cannot be paused");
    assert_eq!(err_text(machine.resume(None).await), "this computer cannot be resumed");
    assert!(!machine.offers().preview_url);
    assert_eq!(
        err_text(machine.download_url("/x").await),
        "this computer serves no signed download URL; its files are read on it directly"
    );
    assert_eq!(err_text(machine.upload_url("/x").await), "this computer serves no signed upload URL; its files are written on it directly");
    assert_eq!(
        err_text(backend.create(spec(), None, None).await),
        "this computer already exists; it is not created, it is the computer wsp already runs on"
    );
    assert_eq!(err_text(backend.delete_snapshot("snap").await), "this computer holds no snapshots");
    machine.kill().await.unwrap();
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
}

#[tokio::test(flavor = "multi_thread")]
async fn puts_bytes_on_this_computer_by_its_own_road() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let machine = backend_in(root.path()).get(LOCAL_MACHINE_ID).await.unwrap();
    let path = root.path().join("copy").join(".wsp-files").join("req_a").join("report.pdf");
    land_bytes(machine.as_ref(), path.to_str().unwrap(), &[0x25, 0x50, 0x44, 0x46], LandBytesOptions::default()).await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"%PDF");
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[tokio::test]
async fn is_memavailable_over_memtotal_on_linux() {
    let meminfo = "MemTotal:       16303412 kB\nMemFree:          401200 kB\nMemAvailable:    9216000 kB\n";
    assert_eq!(memory_of_meminfo(meminfo), Some(MemoryHere { free_mb: 9_000, total_mb: 15_921 }));
    assert_eq!(memory_of_meminfo("MemTotal: 10 kB\n"), None);
    assert_eq!(memory_of_pressure("54\n", gib(16)), Some(MemoryHere { free_mb: 8_847, total_mb: 16_384 }));
    for odd in ["", "x", "101", "-1", "5.5"] {
        assert_eq!(memory_of_pressure(odd, gib(16)), None, "{odd}");
    }
    assert_eq!(memory_here_on("win32").await, None);
    if cfg!(target_os = "linux") {
        assert!(memory_here().await.unwrap().total_mb > 0);
    }
}
