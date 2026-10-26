// SPDX-License-Identifier: AGPL-3.0-only
//! Deep input audit (wsp-labs/wsp-map#2448): input nested past what a walk can hold must be answered, never abort the host.
//! A stack overflow kills the whole process, so each case runs its body in a child copy of this test binary on a thread
//! with a 1 MiB stack, the size the host's threads would run it on, and fails when the child dies.
//! `DEEP_AUDIT_DEPTH` overrides a case's depth, which is how the depth each one dies at was found.

const CHILD: &str = "DEEP_AUDIT_CHILD";
const DEPTH: &str = "DEEP_AUDIT_DEPTH";
const HOST_THREAD_STACK: usize = 1024 * 1024;

/// Runs `body(depth)` in a child copy of this binary and fails when the child dies, as a stack overflow kills a host.
fn in_child(name: &str, depth: usize, body: fn(usize)) {
    let depth = std::env::var(DEPTH).ok().and_then(|d| d.parse().ok()).unwrap_or(depth);
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new()
            .stack_size(HOST_THREAD_STACK)
            .spawn(move || body(depth))
            .expect("a thread")
            .join()
            .expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().expect("this binary"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .env(DEPTH, depth.to_string())
        .output()
        .expect("the child starts");
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).rev().take(3).collect();
        panic!(
            "{depth} deep: the whole process died ({}) instead of answering: {}",
            out.status,
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        );
    }
}

// ---- cases ----
//
// Each case: a `fn <case>_body(depth: usize)` that builds the input `depth` deep, calls the code under test and asserts
// the answer node gives for the same input (an answer, a refusal, or a failed call, which in Rust is an Err), then a
// #[test] that calls in_child with the test's exact name and a depth past where the code dies. The comment above the
// #[test] names the recursion (file:line), what reaches it from outside the host, the depth it dies at on a 1 MiB stack
// in a debug build, and what node answers at the test's depth.

// ---- local_exec/reap.rs: remove_tree over the folder a run left behind ----
//
// Recursion: `remove_tree` (crates/wsp-core/src/local_exec/reap.rs:641), one level per folder, called by `reap_run` for the
// run's `<base>.d` folder when the run ends. What reaches it: whatever the run's own command (an agent's tool call, or a
// build it started) made under that folder; the only bound is the kernel's path length (4,096 bytes, so about 2,000
// folders of one letter). Node: `rmSync(path, { recursive: true, force: true })` removes the tree down to about 1,400 folders (it throws a RangeError past 1,427, to reapRun's caller).
fn reap_deep_body(depth: usize) {
    let root = tempfile::tempdir().expect("a folder");
    let base = root.path().join("r");
    let mut deep = std::path::PathBuf::from(format!("{}.d", base.display()));
    for _ in 0..depth {
        deep.push("d");
    }
    std::fs::create_dir_all(&deep).expect("a deep tree");
    let done = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(wsp_core::local_exec::reap::reap_run(base.to_str().unwrap(), None, 0));
    assert_eq!(done, Ok(()));
    assert!(!std::path::Path::new(&format!("{}.d", base.display())).exists());
}

// Dies at 673 on a 1 MiB stack in a debug build.
#[test]
fn reap_run_removes_a_folder_tree_1400_deep() {
    in_child("reap_run_removes_a_folder_tree_1400_deep", 1_400, reap_deep_body);
}
