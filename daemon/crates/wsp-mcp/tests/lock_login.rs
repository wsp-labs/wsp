// SPDX-License-Identifier: AGPL-3.0-only
//! A lock naming another login's live process is refused before the token beside the state is read or its port
//! asked, as a dial run by another login than the lock's process sees it. Alone in its file: the dial runs in a forked
//! child, and no other test's threads are running when it forks. Root alone can become another login, so elsewhere
//! this proves nothing and passes.

use std::fs::{File, Permissions};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;

use serde_json::json;
use wsp_mcp::host::Host;
use wsp_mcp::{Args, Env, ReleaseRule, LOCK_REFUSED};

/// Catches: the dial, a dial presenting a paired device token, or a pairing code going on to the page past a lock
/// whose process is another login's; or the refusal said in the desktop app's words to the command line, or in the
/// command line's to the desktop.
#[test]
fn a_lock_naming_another_logins_process_is_refused_as_not_this_logins() {
    // SAFETY: geteuid reads this process's user and cannot fail.
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), Permissions::from_mode(0o755)).unwrap();
    // Nothing listens on the port, so a dial past the login check would say no host answers.
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    // Pid 1 is root's and always alive.
    let lock = json!({ "pid": 1, "port": port, "address": "127.0.0.1", "startedAt": "2026-10-19T00:00:00.000Z" });
    std::fs::write(dir.path().join("host.lock"), lock.to_string()).unwrap();
    std::fs::write(dir.path().join("host-token"), "the-owners-token\n").unwrap();
    for name in ["host.lock", "host-token"] {
        std::fs::set_permissions(dir.path().join(name), Permissions::from_mode(0o644)).unwrap();
    }
    let state = dir.path().join("state.json");
    let env: Env = [("WSP_HOME".to_owned(), dir.path().join("home").to_string_lossy().into_owned())].into_iter().collect();
    let host = || Host::new(&Args { state: state.clone(), ..Args::default() }, &env, None);

    let mut ends = [0; 2];
    // SAFETY: pipe fills the two descriptors it is handed; fork's child runs the dial on a runtime of its own and
    // leaves by _exit, so nothing of the harness's runs twice.
    let child = unsafe {
        assert_eq!(libc::pipe(ends.as_mut_ptr()), 0);
        let child = libc::fork();
        if child == 0 {
            libc::close(ends[0]);
            let said = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if libc::setuid(65534) != 0 {
                    return "could not become another login".to_owned();
                }
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                let paired = host();
                paired.hold_token("the-paired-device-token".to_owned());
                let desktop = host().with_release_rule(ReleaseRule::OwnOrNewer);
                let dials = runtime.block_on(async {
                    [
                        host().client().await.map(|_| ()),
                        paired.client().await.map(|_| ()),
                        host().redeem("424242", "laptop").await,
                        desktop.client().await.map(|_| ()),
                    ]
                });
                let said = dials.map(|dialled| match dialled {
                    Ok(()) => "attached".to_owned(),
                    Err(refused) => format!("{}\n{}", refused.kind.unwrap_or_default(), refused.message),
                });
                said.join("\n\n")
            }));
            let mut out = File::from_raw_fd(ends[1]);
            let _ = out.write_all(said.unwrap_or_else(|_| "the dial panicked".to_owned()).as_bytes());
            drop(out);
            libc::_exit(0);
        }
        libc::close(ends[1]);
        child
    };
    let mut said = String::new();
    // SAFETY: the read end is this process's alone once the write end is closed here.
    unsafe { File::from_raw_fd(ends[0]) }.read_to_string(&mut said).unwrap();
    let mut status = 0;
    // SAFETY: reaps the child forked above.
    unsafe { libc::waitpid(child, &mut status, 0) };
    let refused = |again: &str| {
        format!(
            "{LOCK_REFUSED}\na host (pid 1) holds {} on port {port} but that process is not this login's: stop that process or run wsp down, then {again}",
            dir.path().join("host.lock").display()
        )
    };
    let (line, app) = (refused("run this line again"), refused("open wsp again"));
    assert_eq!(said, [line.as_str(), &line, &line, &app].join("\n\n"));
}
