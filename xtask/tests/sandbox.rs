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
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path())), ("MARK", &mark.path().display().to_string())])
    else {
        return;
    };
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
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path())), ("DOOR", &door.display().to_string())]) else {
        return;
    };
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

#[test]
fn root_inside_cannot_make_the_root_of_the_disk_writable_again() {
    let dir = outside();
    // --options-source=disable: mount reads no fstab, where a LABEL= root would stop it before the kernel does.
    let probe = r#"mount --options-source=disable -o remount,bind,rw / 2>/dev/null; test -w / && echo "remount,bind,rw / took: / takes writes again"
true"#;
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path()))]) else { return };
    assert!(ran, "{said}");
    assert_eq!(said, "");
}

#[test]
fn root_inside_cannot_take_its_own_proc_away_and_write_the_computers_disk_through_pid_1() {
    let dir = outside();
    // Under the run's own /proc sits the computer's, whose PID 1 roots in the computer's mounts, all writable there.
    let probe = r#"umount /proc 2>/dev/null; test -w /proc/1/root/var/tmp && echo "the computer's /var/tmp takes writes through /proc/1/root"
true"#;
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path()))]) else { return };
    assert!(ran, "{said}");
    assert_eq!(said, "");
}

#[test]
fn root_inside_cannot_make_a_node_of_the_computers_disk_and_read_it() {
    use std::os::unix::fs::MetadataExt;
    let dir = outside();
    let disk = Command::new("findmnt").args(["-no", "SOURCE", "/"]).output().unwrap();
    let disk = String::from_utf8_lossy(&disk.stdout).trim().to_string();
    let Ok(meta) = std::fs::metadata(&disk) else {
        eprintln!("skipped: the root of the disk is no device here ({disk})");
        return;
    };
    let (major, minor) = (libc::major(meta.rdev()), libc::minor(meta.rdev()));
    let probe = format!(
        r#"mknod /tmp/disk b {major} {minor} 2>/dev/null && [ "$(dd if=/tmp/disk bs=512 count=1 2>/dev/null | wc -c)" = 512 ] && echo "read the computer's disk through a node made in /tmp"
true"#
    );
    let Some((ran, said)) = inside(&probe, &[("WSP_PARITY_WRITABLE", &parity(dir.path()))]) else { return };
    assert!(ran, "{said}");
    assert_eq!(said, "");
}

#[test]
fn a_socket_the_computer_binds_after_the_sandbox_is_up_is_out_of_reach_too() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    // SAFETY: geteuid reads the process's own id and cannot fail.
    if !cfg!(target_os = "linux") || unsafe { libc::geteuid() } != 0 || Command::new("python3").arg("-c").arg("1").output().is_err() {
        eprintln!("skipped: the sandbox's walls are checked as root on Linux, with python3 to dial");
        return;
    }
    let dir = outside();
    // A run lasts minutes; a host or an ssh master that starts meanwhile binds where the scan at the start never looked.
    let door = dir.path().join("late.sock");
    let probe = r#"echo up; sleep 2; python3 -I -c 'import socket, sys; s = socket.socket(socket.AF_UNIX); s.settimeout(2); s.connect(sys.argv[1])' "$DOOR" 2>/dev/null && echo "dialled a socket bound after the scan"
true"#;
    let mut child = Command::new("unshare")
        .args(flags(true))
        .args(["--", "/bin/sh", "-c", SCRIPT, "/bin/sh", "-c", probe])
        .env("WSP_PARITY_WRITABLE", parity(dir.path()))
        .env("DOOR", &door)
        .stdout(Stdio::piped())
        .spawn()
        .expect("unshare");
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    assert_eq!(lines.next().and_then(Result::ok).as_deref(), Some("up"));
    let _listening = UnixListener::bind(&door).unwrap();
    let said: Vec<String> = lines.map_while(Result::ok).collect();
    assert!(child.wait().unwrap().success());
    assert_eq!(said, Vec::<String>::new());
}

#[test]
fn a_collection_writes_its_checkout_but_no_file_outside_it_through_a_hard_link() {
    let dir = outside();
    // pnpm links every file of node_modules to its one store, so a write there lands in every checkout on the computer.
    let (store, checkout) = (dir.path().join("store"), dir.path().join("checkout"));
    std::fs::create_dir_all(&store).unwrap();
    std::fs::create_dir_all(checkout.join("node_modules")).unwrap();
    std::fs::write(store.join("index.js"), "kept\n").unwrap();
    std::fs::hard_link(store.join("index.js"), checkout.join("node_modules/index.js")).unwrap();
    let probe = r#"echo written > "$CHECKOUT/node_modules/index.js" 2>/dev/null
true"#;
    let checkout = checkout.display().to_string();
    let Some((ran, said)) = inside(probe, &[("WSP_PARITY_WRITABLE", &checkout), ("CHECKOUT", &checkout)]) else { return };
    assert!(ran, "{said}");
    assert_eq!(std::fs::read_to_string(store.join("index.js")).unwrap(), "kept\n", "a write inside reached the store outside the checkout");
}

#[test]
fn a_command_inside_the_checkout_starts_in_the_sandbox() {
    // cargo xtask builds xtask under xtask/target in the checkout, and a --host line can name a host built there too:
    // the checkout goes back read-only before the command does, and the command cannot be put back inside it.
    // SAFETY: geteuid reads the process's own id and cannot fail.
    if !cfg!(target_os = "linux") || unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: the sandbox's walls are checked as root on Linux");
        return;
    }
    let dir = outside();
    let command = dir.path().join("target/debug/xtask");
    std::fs::create_dir_all(command.parent().unwrap()).unwrap();
    std::fs::write(&command, "#!/bin/sh\necho started\n").unwrap();
    std::fs::set_permissions(&command, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let out = Command::new("unshare")
        .args(flags(true))
        .args(["--", "/bin/sh", "-c", SCRIPT])
        .arg(&command)
        .env("WSP_PARITY_ROOT", dir.path())
        .env("WSP_PARITY_CWD", dir.path())
        .output()
        .expect("unshare");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert_eq!(said, "started\n", "the sandbox never ran a command that sits inside the checkout");
}
