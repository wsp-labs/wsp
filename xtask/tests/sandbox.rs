// SPDX-License-Identifier: AGPL-3.0-only
//! The sandbox every replay runs in, held to what it promises from inside it: nothing a replayed command does reaches
//! what is installed on the computer. Each case runs the sandbox's own script with a probe as the command, as root on
//! Linux, where a replayed `wsp leave` took /usr/local/bin/claude off a box (#2464); elsewhere it says it skipped.

use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::Command;
use xtask::sandbox::{flags, SCRIPT};

/// What the probe printed inside a fresh sandbox, or none where there is no sandbox to run it in.
fn inside(probe: &str, env: &[(&str, &str)]) -> Option<(bool, String)> {
    // SAFETY: geteuid reads the process's own id and cannot fail.
    if !cfg!(target_os = "linux") || unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: the sandbox's walls are checked as root on Linux");
        return None;
    }
    let out = Command::new("unshare")
        .args(flags(true))
        .args(["--", "/bin/sh", "-c", SCRIPT, "/bin/sh", "-c", probe])
        .envs(env.iter().copied())
        .output()
        .expect("unshare");
    Some((out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))))
}

fn outside() -> tempfile::TempDir {
    // Under /var, which the sandbox holds read-only, as it does the checkout and the home.
    let dir = tempfile::Builder::new().prefix("wsp-sandbox-").tempdir_in("/var/tmp").expect("temp");
    std::fs::create_dir_all(dir.path().join("parity")).unwrap();
    dir
}

fn parity(dir: &Path) -> String {
    dir.join("parity").display().to_string()
}

#[test]
fn a_command_inside_can_neither_add_nor_take_an_entry_at_the_top_of_the_disk_nor_reach_the_computers_tmp() {
    let dir = outside();
    let mark = tempfile::Builder::new().prefix("wsp-sandbox-mark-").tempfile_in("/tmp").expect("mark");
    let probe = r#"test -w / && echo "/ takes writes: rm /bin would take every program off the computer"
[ -e "$MARK" ] && echo "/tmp is the computer's own"
true"#;
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path())), ("MARK", &mark.path().display().to_string())]) else { return };
    assert!(ran, "{said}");
    assert_eq!(said, "");
}

#[test]
fn a_command_inside_cannot_rename_the_computer() {
    let dir = outside();
    let uts = std::fs::read_link("/proc/self/ns/uts").unwrap().display().to_string();
    let probe = r#"[ "$(readlink /proc/self/ns/uts)" = "$UTS" ] && echo "shares the computer's name: hostname x renames it"
true"#;
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path())), ("UTS", &uts)]) else { return };
    assert!(ran, "{said}");
    assert_eq!(said, "");
}

#[test]
fn a_command_inside_reaches_no_service_of_the_computer_over_a_socket() {
    if Command::new("python3").arg("-c").arg("1").output().is_err() {
        eprintln!("skipped: no python3 to dial a socket with");
        return;
    }
    let dir = outside();
    // As the box's daemon binds /root/.wsp/daemon.sock, where the file is the gate and no token rides.
    let door = dir.path().join("daemon.sock");
    let _listening = UnixListener::bind(&door).unwrap();
    let probe = r#"python3 -I -c 'import socket, sys; s = socket.socket(socket.AF_UNIX); s.settimeout(2); s.connect(sys.argv[1])' "$DOOR" 2>/dev/null && echo "dialled a socket outside on a read-only folder"
true"#;
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path())), ("DOOR", &door.display().to_string())]) else { return };
    assert!(ran, "{said}");
    assert_eq!(said, "");
}

#[test]
fn a_check_runs_with_the_goldens_read_only() {
    let dir = outside();
    let probe = r#"test -w "$PARITY" && echo "the host under test can write the goldens it is judged by"
true"#;
    // A check writes nothing, so it names no folder to make writable; only record does.
    let Some((ran, said)) = inside(probe, &[("PARITY", &parity(dir.path()))]) else { return };
    assert!(ran, "the sandbox ran no command when no folder is to be writable: {said}");
    assert_eq!(said, "");
}

#[test]
fn the_variable_the_copy_inside_is_given_does_not_turn_the_sandbox_off_for_a_caller_that_sets_it() {
    // SAFETY: geteuid reads the process's own id and cannot fail.
    if !cfg!(target_os = "linux") || unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: the sandbox's walls are checked as root on Linux");
        return;
    }
    let dir = outside();
    let mnt = std::fs::read_link("/proc/self/ns/mnt").unwrap().display().to_string();
    // A stand-in host that will not start in this computer's own mounts, and otherwise says where it listens.
    let host = dir.path().join("host.sh");
    std::fs::write(
        &host,
        format!("#!/bin/sh\n[ \"$1\" = --version ] && {{ echo 'wsp 9.9.9'; exit 0; }}\n[ \"$(readlink /proc/self/ns/mnt)\" = '{mnt}' ] && exit 3\necho f > \"$WSP_HOME/host-token\"\necho 'listening on ws://127.0.0.1:1/'\nsleep 30\n"),
    )
    .unwrap();
    Command::new("chmod").arg("755").arg(&host).status().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["parity", "--only", "hand/fresh-host", "--jobs", "1", "--host"])
        .arg(&host)
        .env("WSP_PARITY_SANDBOX", "1")
        .output()
        .unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(!said.contains("never said where it listens"), "the tapes ran in this computer's own mounts: {said}");
}
