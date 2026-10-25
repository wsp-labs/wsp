// SPDX-License-Identifier: AGPL-3.0-only
//! The look a wake takes at a box's daemon unit, run through a real bash with systemctl, journalctl and sleep stubbed
//! on the PATH: the stubs answer what systemd 255 answered on a box for a daemon that keeps exiting (2026-10-16).
//! Ported from packages/engine/test/daemon-exits.test.ts.
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use wsp_engine::box_backend::{daemon_exits_of, DAEMON_EXITS_CMD};

/// packages/protocol/src/views/session-events.ts:349 `DAEMON_UNIT`.
const DAEMON_UNIT: &str = "wsp-daemon.service";

/// packages/protocol/test/stub-runner.sh: the one executable every stub runs, reading the script kept beside the link
/// it was started through.
const STUB_RUNNER: &str = r##"#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Runs the script write_stub kept beside the link it was started through.
body="${0%/*}/.${0##*/}.stub"
[ -f "$body" ] || { echo "stub-runner: no script at $body" >&2; exit 127; }
IFS= read -r first < "$body"
case $first in "#!"*) exec ${first#??} "$body" "$@" ;; esac
exec /bin/sh "$body" "$@"
"##;

extern "C" fn remove_runner_dir() {
    if let Some(path) = RUNNER.get() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

static RUNNER: OnceLock<PathBuf> = OnceLock::new();

/// packages/protocol/test/stub-script.ts:9 `STUB_RUNNER`: read only, and its first exec paid once here rather than
/// inside a case, so a stub never meets a half-closed writer (ETXTBSY) from another test's fork.
fn stub_runner() -> &'static Path {
    RUNNER.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("wsp-stub-runner-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stub-runner.sh");
        std::fs::write(&path, STUB_RUNNER).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o555)).unwrap();
        for attempt in 0..50 {
            match Command::new(&path).output() {
                Ok(_) => break,
                Err(e) if attempt < 49 && e.raw_os_error() == Some(libc::ETXTBSY) => {
                    std::thread::sleep(std::time::Duration::from_millis(20))
                }
                Err(e) => panic!("the stub runner would not start: {e}"),
            }
        }
        unsafe { libc::atexit(remove_runner_dir) };
        path
    })
}

/// packages/protocol/test/stub-script.ts:17 `writeStub`: the path is a link to the runner and the script sits beside
/// it, read by the interpreter its first line names.
fn write_stub(path: &Path, script: &str) {
    let dir = path.parent().unwrap();
    let name = path.file_name().unwrap().to_string_lossy();
    std::fs::write(dir.join(format!(".{name}.stub")), script).unwrap();
    let _ = std::fs::remove_file(path);
    symlink(stub_runner(), path).unwrap();
}

#[derive(Default)]
struct Look<'a> {
    counts: [&'a str; 2],
    active: Option<&'a str>,
    sub: Option<&'a str>,
    result: Option<&'a str>,
    status: Option<&'a str>,
    journal: Option<&'a str>,
}

/// daemon-exits.test.ts:23 `look`: what the look prints on a unit whose two restart counts, five seconds apart, are
/// `counts`, and what it asked the journal.
fn look(o: Look) -> (String, String) {
    let tmp = tempfile::Builder::new().prefix("wsp-daemon-exits-").tempdir().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join("counts"), format!("{}\n", o.counts.join("\n"))).unwrap();
    std::fs::write(dir.join("journal"), o.journal.unwrap_or("")).unwrap();
    let d = dir.display();
    let systemctl = [
        "#!/bin/sh".to_string(),
        r#"case "$*" in"#.to_string(),
        format!(r#"  "show -p NRestarts --value {DAEMON_UNIT}") head -n 1 "{d}/counts"; sed -i.was 1d "{d}/counts" ;;"#),
        format!(r#"  "show -p ActiveState --value {DAEMON_UNIT}") echo {} ;;"#, o.active.unwrap_or("active")),
        format!(r#"  "show -p SubState --value {DAEMON_UNIT}") echo {} ;;"#, o.sub.unwrap_or("auto-restart")),
        format!(r#"  "show -p Result --value {DAEMON_UNIT}") echo {} ;;"#, o.result.unwrap_or("exit-code")),
        format!(r#"  "show -p ExecMainStatus --value {DAEMON_UNIT}") echo {} ;;"#, o.status.unwrap_or("2")),
        r#"  *) echo "unexpected: $*" >&2; exit 1 ;;"#.to_string(),
        "esac".to_string(),
        String::new(),
    ]
    .join("\n");
    write_stub(&dir.join("systemctl"), &systemctl);
    write_stub(&dir.join("journalctl"), &format!("#!/bin/sh\necho \"$*\" > \"{d}/asked\"\ncat \"{d}/journal\"\n"));
    write_stub(&dir.join("sleep"), "#!/bin/sh\nexit 0\n");
    let run = Command::new("bash")
        .arg("-c")
        .arg(DAEMON_EXITS_CMD)
        .env_clear()
        .env("PATH", format!("{d}:/usr/bin:/bin"))
        .env("HOME", dir)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stderr), "");
    let asked = std::fs::read_to_string(dir.join("asked")).unwrap_or_default().trim().to_string();
    (String::from_utf8_lossy(&run.stdout).into_owned(), asked)
}

/// daemon-exits.test.ts:50 `LOOPED`: three runs of a daemon from before --doors, as journalctl -o cat printed them:
/// no blank lines, and the window of the last lines starting inside a run.
fn looped() -> String {
    [
        "For more information, try '--help'.",
        "error: unexpected argument '--doors' found",
        "Usage: wsp-daemon [OPTIONS]",
        "For more information, try '--help'.",
        "error: unexpected argument '--doors' found",
        "Usage: wsp-daemon [OPTIONS]",
        "For more information, try '--help'.",
        "",
    ]
    .join("\n")
}

const DOORS_SAID: &str = "error: unexpected argument '--doors' found; Usage: wsp-daemon [OPTIONS]; For more information, try '--help'.";

/// daemon-exits.test.ts:54
#[test]
fn reads_a_unit_restarting_under_restart_always_as_a_daemon_that_exits_in_its_own_words() {
    let (stdout, journal) = look(Look { counts: ["4", "6"], journal: Some(&looped()), ..Default::default() });
    assert_eq!(daemon_exits_of(&stdout).as_deref(), Some(DOORS_SAID));
    // The daemon's own output, not systemd's lines about the unit.
    assert_eq!(journal, format!("_SYSTEMD_UNIT={DAEMON_UNIT} -n 20 --no-pager -o cat"));
}

/// daemon-exits.test.ts:61
#[test]
fn reads_a_unit_a_start_limit_left_failed_as_a_daemon_that_exits_however_still_its_count() {
    let (stdout, _) = look(Look { counts: ["26", "26"], active: Some("failed"), journal: Some(&looped()), ..Default::default() });
    assert_eq!(daemon_exits_of(&stdout).as_deref(), Some(DOORS_SAID));
}

/// daemon-exits.test.ts:65
#[test]
fn cuts_each_line_short_so_the_error_before_a_767_character_usage_line_still_shows() {
    let usage = format!("usage: wsp-daemon {}", "[--host <addr>] ".repeat(48)).trim().to_string();
    let journal = format!("error: unexpected argument '--no-such-flag' found\n{usage}\n").repeat(4);
    let (stdout, _) = look(Look { counts: ["20", "22"], journal: Some(&journal), ..Default::default() });
    let said = daemon_exits_of(&stdout);
    assert_eq!(said, Some(format!("error: unexpected argument '--no-such-flag' found; {}…", &usage[..120])));
}

/// daemon-exits.test.ts:71
#[test]
fn reads_a_daemon_that_lives_three_seconds_before_it_exits_once_its_fifth_restart_lands() {
    let (stdout, _) = look(Look { counts: ["4", "5"], journal: Some("panicked at slow start\n"), ..Default::default() });
    assert_eq!(daemon_exits_of(&stdout).as_deref(), Some("panicked at slow start"));
}

/// daemon-exits.test.ts:75
#[test]
fn says_the_exit_status_of_a_daemon_that_exits_printing_nothing() {
    let (stdout, _) = look(Look { counts: ["0", "5"], status: Some("101"), ..Default::default() });
    assert_eq!(daemon_exits_of(&stdout).as_deref(), Some("exit status 101"));
}

/// daemon-exits.test.ts:79
#[test]
fn says_the_signal_that_killed_a_daemon_that_printed_nothing() {
    let (stdout, _) = look(Look { counts: ["4", "6"], result: Some("signal"), status: Some("9"), ..Default::default() });
    assert_eq!(daemon_exits_of(&stdout).as_deref(), Some("killed by signal 9"));
    let (stdout, _) = look(Look { counts: ["4", "6"], result: Some("oom-kill"), status: Some("9"), ..Default::default() });
    assert_eq!(daemon_exits_of(&stdout).as_deref(), Some("killed for want of memory"));
}

/// daemon-exits.test.ts:84
#[test]
fn names_no_status_once_the_next_copy_runs_since_the_unit_then_reads_success_and_0() {
    let (stdout, _) =
        look(Look { counts: ["4", "6"], sub: Some("running"), result: Some("success"), status: Some("0"), ..Default::default() });
    assert_eq!(daemon_exits_of(&stdout).as_deref(), Some("it printed nothing"));
}

/// daemon-exits.test.ts:88 `it.each`: reads nothing off the unit.
fn reads_nothing(counts: [&str; 2], active: Option<&str>) {
    let (stdout, journal) = look(Look { counts, active, journal: Some(&looped()), ..Default::default() });
    assert_eq!(stdout, "");
    assert_eq!(daemon_exits_of(&stdout), None);
    assert_eq!(journal, "");
}

/// daemon-exits.test.ts:89
#[test]
fn reads_nothing_off_a_daemon_that_runs() {
    reads_nothing(["0", "0"], None);
}

/// daemon-exits.test.ts:90
#[test]
fn reads_nothing_off_a_daemon_that_exited_once_on_purpose() {
    reads_nothing(["3", "4"], None);
}

/// daemon-exits.test.ts:91
#[test]
fn reads_nothing_off_a_daemon_that_exited_five_times_and_now_runs() {
    reads_nothing(["7", "7"], None);
}

/// daemon-exits.test.ts:92
#[test]
fn reads_nothing_off_a_unit_not_started_yet() {
    reads_nothing(["0", "0"], Some("inactive"));
}

/// daemon-exits.test.ts:93
#[test]
fn reads_nothing_off_a_unit_the_box_does_not_have() {
    reads_nothing(["", ""], Some("inactive"));
}
