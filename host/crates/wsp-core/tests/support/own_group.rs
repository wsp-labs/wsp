// SPDX-License-Identifier: AGPL-3.0-only
//! A test that starts real processes and signals process groups runs in a child copy of the test binary that leads a
//! process group of its own, so no signal it sends can reach the runner. The child runs that one test; the parent
//! waits for it with a bound, then kills whatever is left in the child's group. Within one test binary the children
//! run one at a time, as vitest runs one file's cases.
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ENV: &str = "WSP_OWN_GROUP_TEST";
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Defines a `#[test]` whose async body runs in its own process group, bounded by `$timeout_ms`.
#[macro_export]
macro_rules! own_group_test {
    ($(#[$m:meta])* $name:ident, $timeout_ms:expr, $body:block) => {
        $(#[$m])*
        #[test]
        fn $name() {
            $crate::support::own_group::run(module_path!(), stringify!($name), $timeout_ms, &[], || async move $body);
        }
    };
    ($(#[$m:meta])* $name:ident, $timeout_ms:expr, env = $env:expr, $body:block) => {
        $(#[$m])*
        #[test]
        fn $name() {
            $crate::support::own_group::run(module_path!(), stringify!($name), $timeout_ms, $env, || async move $body);
        }
    };
}

/// The test's path as libtest filters it: the module path less the crate's own name, then the test's name.
fn test_path(module: &str, name: &str) -> String {
    match module.split_once("::") {
        Some((_, rest)) => format!("{rest}::{name}"),
        None => name.to_string(),
    }
}

/// In the child (the variable names this test), runs `body` on a current-thread runtime inside a `LocalSet`. In the parent, starts the child
/// as the leader of a new process group with `env` added, waits up to `timeout_ms`, kills the child's group, and fails
/// when the child failed or ran out of time.
pub fn run<F, Fut>(module: &str, name: &str, timeout_ms: u64, env: &[(&str, &str)], body: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let path = test_path(module, name);
    if std::env::var(ENV).ok().as_deref() == Some(path.as_str()) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
        tokio::task::LocalSet::new().block_on(&rt, body());
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(std::env::current_exe().expect("the test binary"));
    cmd.args([path.as_str(), "--exact", "--nocapture", "--test-threads", "1"]).env(ENV, &path).stdin(Stdio::null()).process_group(0);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("the child test");
    let group = child.id() as i32;
    let until = Instant::now() + Duration::from_millis(timeout_ms);
    let status = loop {
        if let Some(status) = child.try_wait().expect("the child's status") {
            break Some(status);
        }
        if Instant::now() >= until {
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // SAFETY: a negative pid names the child's own group, which this process started and does not belong to.
    unsafe {
        libc::kill(-group, libc::SIGKILL);
    }
    let _ = child.wait();
    match status {
        Some(s) => assert!(s.success(), "{path} failed in its own process group: {s}"),
        None => panic!("{path} ran past {timeout_ms} ms in its own process group"),
    }
}
