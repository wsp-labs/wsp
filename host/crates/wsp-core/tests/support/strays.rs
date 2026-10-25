// SPDX-License-Identifier: AGPL-3.0-only
//! What a run leaves running while a test reads it, and how a test reads whether it is still there
//! (runtime/test/strays.ts). Every pid is one the command under test wrote to a file of its own, never one found by
//! name; `sweep` ends each recorded pid's group after the case.
use std::sync::Mutex;

static STRAYS: Mutex<Vec<(i32, Option<i32>)>> = Mutex::new(Vec::new());

/// A pid's process group as ps reads it while it runs.
pub fn group_of(pid: i32) -> Option<i32> {
    let out = std::process::Command::new("ps").args(["-o", "pgid=", "-p", &pid.to_string()]).output().ok()?;
    let g: i32 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    (g > 0).then_some(g)
}

/// The pid a command wrote to `file`, once the file holds one; recorded for the sweep with its group.
pub async fn grandchild(file: &str) -> i32 {
    super::wait_for(&format!("a pid in {file}"), 5_000, || !super::read(file).trim().is_empty()).await;
    let pid: i32 = super::read(file).trim().parse().expect("a pid");
    assert!(pid > 0);
    STRAYS.lock().unwrap().push((pid, group_of(pid)));
    pid
}

/// Starts `cmd` as the leader of a group of its own and reaps it on a thread when it exits, as node reaps the
/// children it spawns: an exited child nobody waits on stays a zombie, which `kill(pid, 0)` still reads as there.
/// Recorded for the sweep.
pub fn spawn_reaped(mut cmd: std::process::Command) -> i32 {
    use std::os::unix::process::CommandExt;
    let mut child = cmd.process_group(0).stdin(std::process::Stdio::null()).spawn().expect("the child");
    let pid = child.id() as i32;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    record(pid);
    pid
}

/// Records a pid this test started itself, for the sweep.
pub fn record(pid: i32) {
    STRAYS.lock().unwrap().push((pid, group_of(pid)));
}

/// Whether the pid is still there (signal 0, any error read as gone, as the TS helper reads it).
pub fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 sends nothing.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Waits until the pid is gone.
pub async fn gone(pid: i32) {
    super::wait_for(&format!("pid {pid} to go"), 5_000, || !alive(pid)).await;
}

/// The parent of a pid, as ps reads it.
pub fn ppid_of(pid: i32) -> i32 {
    let out = std::process::Command::new("ps").args(["-o", "ppid=", "-p", &pid.to_string()]).output().expect("ps");
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

/// Kills a group, ignoring every error; never this process's own group.
pub fn kill_group(pid: i32) {
    if pid > 1 && group_of(std::process::id() as i32) != Some(pid) {
        // SAFETY: a negative pid names a group this test started.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
}

/// Ends every recorded pid's whole group, never the test's own.
pub fn sweep() {
    let own = group_of(std::process::id() as i32);
    for (pid, group) in STRAYS.lock().unwrap().drain(..) {
        let targets = match group {
            Some(g) if Some(g) != own => vec![-g, pid],
            _ => vec![pid],
        };
        for t in targets {
            // SAFETY: each target is a pid or a group this test recorded.
            unsafe {
                libc::kill(t, libc::SIGKILL);
            }
        }
    }
}
