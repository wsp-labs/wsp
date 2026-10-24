// SPDX-License-Identifier: AGPL-3.0-only
use super::*;
use std::process::{Child, Command};
use std::sync::Mutex;

type Answer = Box<dyn Fn(&[String]) -> Ran + Send + Sync>;

/// A manager that answers each command by `answer` and keeps every argv it was handed.
struct Fake {
    ran: Mutex<Vec<Vec<String>>>,
    answer: Answer,
}

impl Fake {
    fn new(answer: impl Fn(&[String]) -> Ran + Send + Sync + 'static) -> Fake {
        Fake { ran: Mutex::new(Vec::new()), answer: Box::new(answer) }
    }

    fn ran(&self) -> Vec<String> {
        self.ran.lock().unwrap().iter().map(|argv| argv.join(" ")).collect()
    }
}

impl Runner for Fake {
    async fn run(&self, argv: &[String]) -> Ran {
        self.ran.lock().unwrap().push(argv.to_vec());
        (self.answer)(argv)
    }
}

fn ok(output: &str) -> Ran {
    Ran { code: 0, output: output.to_owned() }
}

const ABSENT: Ran = Ran { code: 113, output: String::new() };
const INACTIVE: &str = "ActiveState=inactive\nUnitFileState=disabled\nMainPID=0";

/// A start on a home of its own, holding the unit today's app wrote; what it says in the app's log is kept.
struct Fixture {
    _dir: tempfile::TempDir,
    start: Start,
    said: Arc<Mutex<Vec<String>>>,
    unit: PathBuf,
    kids: Vec<Child>,
}

fn fixture(manager: Manager, wait: Duration) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_owned();
    let state = home.join(".wsp/state.json");
    std::fs::create_dir_all(state.parent().unwrap()).unwrap();
    let unit = manager.unit(&home, &state);
    std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
    std::fs::write(&unit, "the unit today's app wrote\n").unwrap();
    let said = Arc::new(Mutex::new(Vec::new()));
    let kept = said.clone();
    let vars: Env = [("HOME".to_owned(), home.to_string_lossy().into_owned())].into_iter().collect();
    let start = Start {
        state,
        env: vars,
        home,
        uid: 501,
        manager: Some(manager),
        wsp: None,
        restart: false,
        port: free_port_here(),
        wait,
        say: Arc::new(move |line| kept.lock().unwrap().push(line.to_owned())),
    };
    Fixture { _dir: dir, start, said, unit, kids: Vec::new() }
}

impl Fixture {
    fn tag(&self) -> String {
        tag(&self.start.state)
    }

    fn said(&self) -> Vec<String> {
        self.said.lock().unwrap().clone()
    }

    fn log(&self) -> PathBuf {
        self.start.state.with_file_name("host.log")
    }

    /// Starts `program` as a child held unreaped until this goes, so its pid is never another process's.
    fn kid(&mut self, program: &Path, args: &[&str]) -> i32 {
        let child = Command::new(program).args(args).env_clear().env("PATH", "/usr/bin:/bin").spawn().unwrap();
        let pid = child.id() as i32;
        self.kids.push(child);
        pid
    }

    fn write_lock(&self, pid: i32, port: u16, by: &str) -> PathBuf {
        let path = self.start.state.with_file_name("host.lock");
        let lock = format!(r#"{{"pid":{pid},"port":{port},"startedAt":"2026-10-19T00:00:00.000Z","startedBy":"{by}"}}"#);
        std::fs::write(&path, lock).unwrap();
        std::fs::write(self.start.state.with_file_name("host-token"), "token\n").unwrap();
        path
    }
}

/// A port nothing listens on as the test starts, so no host of the machine's on `DEFAULT_PORT` moves what a test reads.
fn free_port_here() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// A wsp page on loopback carrying the digest of `token`, as a host serves its own; its port.
fn page(token: &str) -> u16 {
    use std::io::{Read as _, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let digest: String =
        ring::digest::digest(&ring::digest::SHA256, token.as_bytes()).as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { return };
            let _ = socket.read(&mut [0u8; 1024]);
            let page =
                format!(r#"<html><script>window.__WSP__ = {{"wsPath":"/ws","version":"0.4.0","tokenHash":"{digest}"}};</script></html>"#);
            let _ = write!(socket, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{page}", page.len());
        }
    });
    port
}

/// The unit npm's `wsp up --service` writes for the fixture's state, with a port it was given.
fn npm_unit(f: &Fixture) -> String {
    let (state, port) = (f.start.state.display(), f.start.port);
    format!("ExecStart='/usr/bin/node' '/usr/lib/node_modules/@wsp-labs/wsp/dist/bin.js' 'up' '--state' '{state}' '--port' '{port}'\n")
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for kid in &mut self.kids {
            let _ = kid.kill();
            let _ = kid.wait();
        }
    }
}

fn block<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

/// A child this test reaps as soon as it exits, so its pid reads dead the moment it is.
fn reaped(mut child: Child) -> i32 {
    let pid = child.id() as i32;
    std::thread::spawn(move || child.wait());
    pid
}

fn sh(script: &str) -> i32 {
    reaped(Command::new("/bin/sh").args(["-c", script]).env_clear().env("PATH", "/usr/bin:/bin").spawn().unwrap())
}

#[test]
fn names_the_unit_as_service_ts_does() {
    // node: createHash("sha256").update(path).digest("hex").slice(0, 8)
    assert_eq!(tag(Path::new("/Users/a/.wsp/state.json")), "2f71d6c9");
    assert_eq!(tag(Path::new("/home/a/.wsp/state.json")), "3d4b8f28");
    let state = Path::new("/Users/a/.wsp/state.json");
    assert_eq!(Manager::Launchd.unit(Path::new("/Users/a"), state), Path::new("/Users/a/Library/LaunchAgents/com.wsp.host.2f71d6c9.plist"));
    let state = Path::new("/home/a/.wsp/state.json");
    assert_eq!(Manager::Systemd.unit(Path::new("/home/a"), state), Path::new("/home/a/.config/systemd/user/wsp-host-3d4b8f28.service"));
}

#[test]
fn launchd_loads_a_unit_it_let_go_with_enable_then_bootstrap_and_leaves_the_file() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let fake = Fake::new(|argv| if argv[1] == "print" { ABSENT } else { ok("") });
    block(ensure(&f.start, &fake)).unwrap();
    let (label, plist) = (format!("com.wsp.host.{}", f.tag()), f.unit.display());
    assert_eq!(
        fake.ran(),
        [
            "launchctl print-disabled gui/501".to_owned(),
            format!("launchctl print gui/501/{label}"),
            format!("launchctl enable gui/501/{label}"),
            format!("launchctl bootstrap gui/501 {plist}"),
        ]
    );
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), "the unit today's app wrote\n");
}

#[test]
fn systemd_loads_a_unit_with_daemon_reload_enable_and_restart() {
    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "is-enabled" => ok("enabled"),
        "show" => ok(INACTIVE),
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    let name = format!("wsp-host-{}.service", f.tag());
    assert_eq!(
        fake.ran(),
        [
            format!("systemctl --user is-enabled {name}"),
            format!("systemctl --user show {name} --property=ActiveState,UnitFileState,MainPID"),
            "systemctl --user daemon-reload".to_owned(),
            format!("systemctl --user enable {name}"),
            format!("systemctl --user restart {name}"),
        ]
    );
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), "the unit today's app wrote\n");
}

#[test]
fn a_unit_the_manager_holds_is_not_loaded_again() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let fake = Fake::new(|_| ok("\tstate = running\n"));
    block(ensure(&f.start, &fake)).unwrap();
    assert_eq!(fake.ran().len(), 2, "{:?}", fake.ran());
    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let fake = Fake::new(|argv| if argv[2] == "show" { ok("ActiveState=active\nUnitFileState=enabled\nMainPID=7") } else { ok("enabled") });
    block(ensure(&f.start, &fake)).unwrap();
    assert_eq!(fake.ran().len(), 2, "{:?}", fake.ran());
}

#[test]
fn a_load_refused_as_another_launch_loaded_it_a_moment_before_is_waited_on() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let loaded = Arc::new(Mutex::new(false));
    let fake = Fake::new(move |argv| match argv[1].as_str() {
        "print" if *loaded.lock().unwrap() => ok(""),
        "print" => ABSENT,
        "bootstrap" => {
            *loaded.lock().unwrap() = true;
            Ran { code: 5, output: "Bootstrap failed: 5: Input/output error".to_owned() }
        }
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    assert!(fake.ran().last().unwrap().starts_with("launchctl print gui/501/"), "{:?}", fake.ran());
}

#[test]
fn a_load_the_manager_refuses_is_its_own_line() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let fake = Fake::new(|argv| match argv[1].as_str() {
        "print" => ABSENT,
        "bootstrap" => Ran { code: 5, output: "Bootstrap failed: 5:\n  Input/output error\n".to_owned() },
        _ => ok(""),
    });
    let failed = block(ensure(&f.start, &fake)).unwrap_err();
    assert_eq!(
        failed.failure.message,
        format!("launchctl bootstrap gui/501 {} exited 5 and said: Bootstrap failed: 5: Input/output error", f.unit.display())
    );
    assert_eq!(failed.log, None);
    assert!(f.unit.exists());

    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "show" | "is-enabled" => Ran { code: 1, output: "Failed to connect to bus: No medium found".to_owned() },
        "daemon-reload" => Ran { code: 1, output: String::new() },
        _ => ok(""),
    });
    let failed = block(ensure(&f.start, &fake)).unwrap_err();
    assert_eq!(failed.failure.message, "systemctl --user daemon-reload exited 1 and said nothing");
    assert!(!fake.ran().iter().any(|argv| argv.contains("restart")), "{:?}", fake.ran());
    assert!(f.unit.exists());
}

#[test]
fn a_unit_set_off_at_login_is_loaded_and_set_back_off() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let label = format!("com.wsp.host.{}", f.tag());
    let disabled = format!("disabled services = {{\n\t\t\"{label}\" => disabled\n\t}}");
    let fake = Fake::new(move |argv| match argv[1].as_str() {
        "print-disabled" => ok(&disabled),
        "print" => ABSENT,
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    assert_eq!(
        fake.ran()[2..],
        [
            format!("launchctl enable gui/501/{label}"),
            format!("launchctl bootstrap gui/501 {}", f.unit.display()),
            format!("launchctl disable gui/501/{label}")
        ]
    );

    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "is-enabled" => ok("disabled"),
        "show" => ok(INACTIVE),
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    assert_eq!(fake.ran().last().unwrap(), &format!("systemctl --user disable wsp-host-{}.service", f.tag()));
}

#[test]
fn setting_the_unit_back_off_refused_goes_to_the_app_log_and_the_open_goes_on() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let label = format!("com.wsp.host.{}", f.tag());
    let disabled = format!("\"{label}\" => true");
    let fake = Fake::new(move |argv| match argv[1].as_str() {
        "print-disabled" => ok(&disabled),
        "print" => ABSENT,
        "disable" => Ran { code: 1, output: "Bad request.".to_owned() },
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    assert_eq!(f.said(), [format!("launchctl disable gui/501/{label} exited 1 and said: Bad request.")]);
}

#[test]
fn no_unit_says_to_open_wsp_once_and_no_manager_says_what_wsp_writes() {
    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    std::fs::remove_file(&f.unit).unwrap();
    let fake = Fake::new(|_| ok(""));
    assert_eq!(block(ensure(&f.start, &fake)).unwrap_err().failure.message, NOT_SET_UP);
    assert!(fake.ran().is_empty());
    let mut f = fixture(Manager::Systemd, SERVICE_WAIT);
    f.start.manager = None;
    assert_eq!(block(ensure(&f.start, &fake)).unwrap_err().failure.message, no_manager_line(std::env::consts::OS));
    assert_eq!(
        no_manager_line("windows"),
        "wsp writes no service on win32; it writes a launchd agent on darwin and a systemd unit on linux. Run wsp up in a terminal that stays open instead."
    );
}

/// Read off /proc, which a Mac has none of; there such a host keeps serving.
#[cfg(target_os = "linux")]
#[test]
fn a_host_whose_program_is_gone_has_its_manager_start_it_again() {
    let mut f = fixture(Manager::Systemd, SERVICE_WAIT);
    let copy = f.start.home.join("sleep");
    // Copied by a child: a write handle of this process's, inherited by another test's fork, makes the exec ETXTBSY.
    assert!(Command::new("/bin/cp").arg("/bin/sleep").arg(&copy).env_clear().status().unwrap().success());
    let pid = f.kid(&copy, &["30"]);
    std::fs::remove_file(&copy).unwrap();
    let lock = format!(r#"{{"pid":{pid},"port":1,"startedAt":"2026-10-19T00:00:00.000Z","startedBy":"service"}}"#);
    std::fs::write(f.start.state.with_file_name("host.lock"), lock).unwrap();
    let fake = Fake::new(move |argv| match argv[2].as_str() {
        "show" => ok(&format!("ActiveState=active\nUnitFileState=enabled\nMainPID={pid}")),
        _ => ok("enabled"),
    });
    block(ensure(&f.start, &fake)).unwrap();
    assert!(fake.ran().last().unwrap().contains(" restart "), "{:?}", fake.ran());
}

/// Catches: Quit and stop wsp booting the host out and keeping its unit, so the next login started it again, where
/// today's goes `wsp down`'s road and removes the unit (`desktop:host-lifecycle.ts:675`).
#[test]
fn the_stop_unloads_the_unit_and_removes_its_file_as_wsp_down_does() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let fake = Fake::new(|_| ok("\tstate = running\n"));
    block(stop(&f.start, &fake)).unwrap();
    let label = format!("com.wsp.host.{}", f.tag());
    assert_eq!(fake.ran(), [format!("launchctl print gui/501/{label}"), format!("launchctl bootout gui/501/{label}")]);
    assert!(!f.unit.exists(), "the plist went, so the next login loads nothing");

    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let fake = Fake::new(|_| ok("ActiveState=active\nUnitFileState=enabled\nMainPID=7"));
    block(stop(&f.start, &fake)).unwrap();
    let name = format!("wsp-host-{}.service", f.tag());
    assert_eq!(
        fake.ran(),
        [
            format!("systemctl --user show {name} --property=ActiveState,UnitFileState,MainPID"),
            format!("systemctl --user disable --now {name}"),
            "systemctl --user daemon-reload".to_owned(),
        ]
    );
    assert!(!f.unit.exists(), "the unit file went, so the next login starts nothing");
}

#[test]
fn a_unit_the_manager_no_longer_holds_has_its_file_removed_and_nothing_unloaded() {
    let f = fixture(Manager::Launchd, SERVICE_WAIT);
    let fake = Fake::new(|_| ABSENT);
    block(stop(&f.start, &fake)).unwrap();
    assert_eq!(fake.ran().len(), 1, "{:?}", fake.ran());
    assert!(!f.unit.exists());

    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let fake = Fake::new(|_| ok(INACTIVE));
    block(stop(&f.start, &fake)).unwrap();
    assert_eq!(fake.ran().len(), 1, "{:?}", fake.ran());
    assert!(!f.unit.exists());
}

#[test]
fn an_unload_the_manager_refuses_is_its_line_and_keeps_the_unit() {
    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let name = format!("wsp-host-{}.service", f.tag());
    let fake =
        Fake::new(
            |argv| if argv[2] == "disable" { Ran { code: 5, output: "Failed to disable".to_owned() } } else { ok("ActiveState=active") },
        );
    assert_eq!(
        block(stop(&f.start, &fake)).unwrap_err(),
        format!("systemctl --user disable --now {name} exited 5 and said: Failed to disable")
    );
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), "the unit today's app wrote\n");
}

/// Catches: a manager that answers neither "held" nor "absent", as `systemctl --user` with no bus, read as absent and
/// its unit removed while the manager may still hold it.
#[test]
fn a_manager_that_cannot_answer_leaves_the_unit_and_refuses_only_while_a_host_serves() {
    let mut f = fixture(Manager::Launchd, Duration::from_millis(200));
    let fake = Fake::new(|_| Ran { code: 5, output: "Bootstrap failed".to_owned() });
    block(stop(&f.start, &fake)).unwrap();
    assert_eq!(fake.ran().len(), 1, "{:?}", fake.ran());
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), "the unit today's app wrote\n");

    let pid = f.kid(Path::new("/bin/sleep"), &["30"]);
    f.write_lock(pid, 1, "service");
    let refused = block(stop(&f.start, &fake)).unwrap_err();
    assert!(refused.contains("so wsp cannot tell whether the launchd agent"), "{refused}");
    assert!(pid_alive(pid));
    assert!(f.unit.exists());
}

#[test]
fn a_host_a_line_started_is_ended_by_the_pid_its_lock_records_and_no_other() {
    let f = fixture(Manager::Launchd, Duration::from_secs(5));
    let pid = sh("exec sleep 30");
    f.write_lock(pid, 1, "up");
    let fake = Fake::new(|_| ABSENT);
    block(stop(&f.start, &fake)).unwrap();
    assert!(!pid_alive(pid), "the host wsp up started is gone");
    assert_eq!(fake.ran().len(), 1, "a manager that holds nothing is asked to stop nothing: {:?}", fake.ran());

    let mut f = fixture(Manager::Launchd, Duration::from_secs(5));
    let pid = f.kid(Path::new("/bin/sleep"), &["30"]);
    f.write_lock(pid, 1, "service");
    let refused = block(stop(&f.start, &fake)).unwrap_err();
    assert_eq!(refused, format!("the host (pid {pid}) is still serving {}", f.start.state.display()));
    assert!(pid_alive(pid), "a host the line did not start is no line's to signal");
}

/// Catches: a systemd unit stopped by hand (`systemctl --user stop`) and left enabled read as held at the next
/// launch, which then waited on a process nothing would start.
#[test]
fn a_systemd_unit_stopped_and_left_enabled_is_loaded_again() {
    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "show" => ok("ActiveState=inactive\nUnitFileState=enabled\nMainPID=0"),
        "is-enabled" => ok("enabled"),
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    let name = format!("wsp-host-{}.service", f.tag());
    assert_eq!(
        fake.ran()[2..],
        [
            "systemctl --user daemon-reload".to_owned(),
            format!("systemctl --user enable {name}"),
            format!("systemctl --user restart {name}")
        ]
    );
}

#[test]
fn reads_the_managers_answers_as_service_ts_does() {
    let (launchd, systemd) = (Manager::Launchd, Manager::Systemd);
    assert_eq!(launchd.pid_of(&ok("\tstate = running\n\t\tpid = 7\n\tpid = 42\n")), Some(42));
    assert_eq!(launchd.pid_of(&ok("\tstate = not running\n")), None);
    assert_eq!(launchd.pid_of(&ABSENT), None);
    assert_eq!(systemd.pid_of(&ok("ActiveState=active\nMainPID=42\n")), Some(42));
    assert_eq!(systemd.pid_of(&ok(INACTIVE)), None);
    assert!(!systemd.holding(&ok(INACTIVE)));
    assert!(systemd.holding(&ok("ActiveState=inactive\nUnitFileState=enabled\nMainPID=0")));
    assert!(systemd.holding(&ok("ActiveState=activating\nUnitFileState=disabled\nMainPID=0")));
    assert!(!systemd.holding(&Ran { code: 1, output: "ActiveState=active".to_owned() }));
}

#[test]
fn says_a_refusal_on_one_line() {
    let argv = words(&["launchctl", "enable", "gui/501/x"]);
    assert_eq!(run_failure_line(&argv, &Ran { code: 1, output: " \n".to_owned() }), "launchctl enable gui/501/x exited 1 and said nothing");
    assert_eq!(run_failure_line(&argv, &Ran { code: 1, output: "a\n  b".to_owned() }), "launchctl enable gui/501/x exited 1 and said: a b");
}

#[test]
fn says_a_wait_as_fmt_duration_does() {
    for (ms, said) in [(0, "1ms"), (400, "400ms"), (1_840, "1.8s"), (9_990, "10s"), (20_400, "20s"), (120_000, "2m"), (125_000, "2m 5s")] {
        assert_eq!(fmt_wait(Duration::from_millis(ms)), said);
    }
}

#[test]
fn the_real_runner_answers_127_for_a_manager_not_on_path_and_hands_on_only_what_a_manager_needs() {
    let names = ["HOME", "PATH", "USER", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "WSP_HOME", "ANTHROPIC_API_KEY"];
    let vars: Env = names.iter().map(|name| ((*name).to_owned(), "/usr/bin:/bin".to_owned())).collect();
    let system = System::new(&vars);
    let missing = block(system.run(&words(&["wsp-no-such-manager", "print"])));
    assert_eq!(missing.code, 127, "{missing:?}");
    let script = "for n in HOME PATH USER XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS WSP_HOME ANTHROPIC_API_KEY LANG; do eval \"[ -n \\\"\\${$n+x}\\\" ]\" && echo $n; done; exit 3";
    let named = block(system.run(&words(&["sh", "-c", script])));
    assert_eq!(named, Ran { code: 3, output: "HOME\nPATH\nUSER\nXDG_RUNTIME_DIR\nDBUS_SESSION_BUS_ADDRESS".to_owned() });
}

#[test]
fn a_service_nothing_runs_is_one_line_and_the_manager_is_asked_once_a_second() {
    let f = fixture(Manager::Launchd, Duration::from_millis(2_500));
    let fake = Fake::new(|_| ABSENT);
    let failed = block(until_attachable(&f.start, &fake)).map(|_| ()).unwrap_err();
    let message = &failed.failure.message;
    assert!(message.starts_with("wsp did not start within 2.") && message.ends_with("s: nothing ran its service"), "{message}");
    assert_eq!(failed.log, Some(f.log()));
    assert_eq!(fake.ran().len(), 3, "{:?}", fake.ran());
}

/// A manager that keeps the unit alive: each `print` that finds the last process gone starts `script` again.
fn kept_alive(script: String) -> Fake {
    let kids = Mutex::new(Vec::<i32>::new());
    Fake::new(move |_| {
        let mut kids = kids.lock().unwrap();
        if kids.last().is_none_or(|pid| !pid_alive(*pid)) {
            kids.push(sh(&script));
        }
        ok(&format!("\tstate = running\n\tpid = {}\n", kids.last().unwrap()))
    })
}

#[test]
fn a_host_that_exits_as_it_starts_is_its_last_line_with_its_log_and_the_lines_in_the_app_log() {
    let f = fixture(Manager::Launchd, Duration::from_secs(5));
    let log = f.log().display().to_string();
    let script = format!(
        "sleep 0.3; echo 'listening on 127.0.0.1:4400' >> '{log}'; echo 'wsp: port 4400 is held by another program' >> '{log}'; exit 1"
    );
    let fake = kept_alive(script);
    let from = Instant::now();
    let failed = block(until_attachable(&f.start, &fake)).map(|_| ()).unwrap_err();
    assert_eq!(failed.failure.message, "wsp stopped as it started: wsp: port 4400 is held by another program");
    assert_eq!(failed.log, Some(f.log()));
    assert!(f.said().contains(&"host: wsp: port 4400 is held by another program".to_owned()), "{:?}", f.said());
    // Two exits, so the wait did not run out the whole start's wait for another.
    assert!(from.elapsed() < Duration::from_secs(5), "{:?}", from.elapsed());
}

#[test]
fn a_host_that_exits_saying_nothing_says_so() {
    let f = fixture(Manager::Launchd, Duration::from_secs(5));
    let failed = block(until_attachable(&f.start, &kept_alive("exit 1".to_owned()))).map(|_| ()).unwrap_err();
    assert_eq!(failed.failure.message, "wsp stopped as it started and wrote nothing to its log");
}

#[test]
fn a_host_that_runs_and_neither_answers_nor_writes_is_given_up_on_after_the_longer_wait() {
    let mut f = fixture(Manager::Systemd, Duration::from_millis(300));
    let pid = f.kid(Path::new("/bin/sleep"), &["30"]);
    let fake = Fake::new(move |_| ok(&format!("ActiveState=active\nUnitFileState=enabled\nMainPID={pid}")));
    let failed = block(until_attachable(&f.start, &fake)).map(|_| ()).unwrap_err();
    let message = &failed.failure.message;
    // Six of the 300 ms waits with nothing moving.
    assert!(
        message.starts_with("wsp did not start within 1.") && message.ends_with("s: it neither answered nor wrote to its log"),
        "{message}"
    );
}

#[test]
fn a_lock_whose_port_never_answers_is_given_up_on_after_the_longer_wait() {
    let mut f = fixture(Manager::Systemd, Duration::from_millis(300));
    let pid = f.kid(Path::new("/bin/sleep"), &["30"]);
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let lock_path = f.write_lock(pid, port, "service");
    let fake = Fake::new(|_| ok(""));
    let failed = block(ready(&f.start, &fake, |_| async { Ok(()) })).map(|_| ()).unwrap_err();
    let message = &failed.failure.message;
    let holds = format!("a host (pid {pid}) holds {} on port {port} but no wsp host answers there", lock_path.display());
    assert!(message.starts_with("after 1.") && message.contains(&format!("s of waiting, {holds}")), "{message}");
    assert!(fake.ran().is_empty(), "{:?}", fake.ran());
}

/// Catches: a port that sends a first byte and never closes read as answering at every probe and refused at every
/// dial, which with a log still moving kept the open from ever ending.
#[test]
fn a_port_that_starts_an_answer_and_never_ends_it_is_given_up_on_after_the_longer_wait() {
    let mut f = fixture(Manager::Systemd, Duration::from_millis(300));
    let grows = format!("while :; do echo moving >> '{}'; sleep 0.1; done", f.log().display());
    let pid = f.kid(Path::new("/bin/sh"), &["-c", &grows]);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for mut tcp in listener.incoming().flatten() {
            let _ = std::io::Write::write_all(&mut tcp, b"H");
            held.push(tcp);
        }
    });
    let lock_path = f.write_lock(pid, port, "service");
    let fake = Fake::new(|_| ok(""));
    let opened = block(async { tokio::time::timeout(Duration::from_secs(20), ready(&f.start, &fake, |_| async { Ok(()) })).await });
    let failed = opened.expect("the open never ended").map(|_| ()).unwrap_err();
    let message = &failed.failure.message;
    let holds = format!("a host (pid {pid}) holds {} on port {port} but no wsp host answers there", lock_path.display());
    assert!(message.starts_with("after ") && message.contains(&format!("s of waiting, {holds}")), "{message}");
}

/// The wsp command this app owns, in the fixture's home as the app writes it.
fn owned(f: &mut Fixture) -> PathBuf {
    let wsp = f.start.home.join(".wsp/bin/wsp");
    f.start.wsp = Some(wsp.clone());
    wsp
}

/// Catches: a computer with no unit yet, where only this app is installed, told to open today's app first instead
/// of having the host's own `wsp up --service` write the unit to run this app's command.
#[test]
fn a_missing_unit_is_written_by_the_wsp_command_this_app_owns() {
    for manager in [Manager::Launchd, Manager::Systemd] {
        let mut f = fixture(manager, SERVICE_WAIT);
        std::fs::remove_file(&f.unit).unwrap();
        let wsp = owned(&mut f);
        let fake = Fake::new(|_| ok("systemd unit wsp-host-x.service is loaded; it serves again at every login"));
        block(ensure(&f.start, &fake)).unwrap();
        assert_eq!(fake.ran(), [format!("{} up --service --state {} --port {}", wsp.display(), f.start.state.display(), f.start.port)]);
        assert_eq!(f.said(), [format!("{} wrote the {} {}", wsp.display(), manager.words(), manager.name(&f.start.state))]);
    }
}

#[test]
fn an_install_the_command_refused_is_its_last_line_with_the_host_log() {
    let mut f = fixture(Manager::Systemd, SERVICE_WAIT);
    std::fs::remove_file(&f.unit).unwrap();
    owned(&mut f);
    let refusal = "wsp up --service: a service starts without your shell, so it reads its provider key from a file.";
    let fake = Fake::new(move |_| Ran { code: 1, output: format!("adopted the login shell's PATH\n{refusal}\n") });
    let failed = block(ensure(&f.start, &fake)).unwrap_err();
    assert_eq!(failed.failure.message, refusal);
    assert_eq!(failed.log, Some(f.log()));
    assert_eq!(f.said().last().unwrap(), &format!("wsp up --service: {refusal}"));

    // The unit written and its host slower than the command waited: the open goes on to wait on it.
    let unit = f.unit.clone();
    let slow = Fake::new(move |_| {
        std::fs::write(&unit, "the unit wsp up --service wrote\n").unwrap();
        Ran { code: 1, output: "the systemd unit loaded, but nothing answered on port 4400 within 20s".to_owned() }
    });
    block(ensure(&f.start, &slow)).unwrap();
}

/// Catches: a unit someone else wrote (today's app, a terminal's `wsp up --service`) written over by this app.
#[test]
fn a_unit_that_stands_is_loaded_as_it_stands_whoever_owns_the_command() {
    let mut f = fixture(Manager::Systemd, SERVICE_WAIT);
    owned(&mut f);
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "is-enabled" => ok("enabled"),
        "show" => ok(INACTIVE),
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    assert!(!fake.ran().iter().any(|line| line.contains("--service")), "{:?}", fake.ran());
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), "the unit today's app wrote\n");
}

#[test]
fn only_a_service_host_whose_unit_runs_the_command_is_restarted_onto_it() {
    let mut f = fixture(Manager::Systemd, SERVICE_WAIT);
    let wsp = owned(&mut f);
    let pid = f.kid(Path::new("/bin/sleep"), &["30"]);
    f.write_lock(pid, 1, "service");
    std::fs::write(&f.unit, format!("ExecStart={} up --state x\n", wsp.display())).unwrap();
    assert!(!restarts_onto_wsp(&f.start), "a launch that asked for no restart restarts nothing");
    f.start.restart = true;
    assert!(restarts_onto_wsp(&f.start));
    std::fs::write(&f.unit, "ExecStart=/usr/bin/node /usr/lib/node_modules/@wsp-labs/wsp/dist/bin.js up\n").unwrap();
    assert!(!restarts_onto_wsp(&f.start), "a unit running another wsp is not this command's");
    std::fs::write(&f.unit, format!("ExecStart={} up --state x\n", wsp.display())).unwrap();
    f.write_lock(pid, 1, "up");
    assert!(!restarts_onto_wsp(&f.start), "a host a terminal holds is that terminal's");
}

/// Catches: a launch that asked for a restart (an older host, or files that went) opening on that host as it was.
#[test]
fn a_launch_that_asks_for_a_restart_restarts_the_service_host_before_it_opens() {
    let mut f = fixture(Manager::Systemd, Duration::from_millis(300));
    let wsp = owned(&mut f);
    f.start.restart = true;
    std::fs::write(&f.unit, format!("ExecStart={} up --state x\n", wsp.display())).unwrap();
    let host = sh("exec sleep 30");
    let lock = f.write_lock(host, 1, "service");
    let fake = Fake::new(move |argv| {
        if argv[1] == "restart" {
            // SAFETY: the pid this test started and recorded, which the restart ends as a host's manager would.
            unsafe { libc::kill(host, libc::SIGTERM) };
            std::fs::remove_file(&lock).unwrap();
            return ok("the host restarted and serves again; no thread is running on it");
        }
        ok("ActiveState=inactive\nUnitFileState=disabled\nMainPID=0")
    });
    let _ = block(ready(&f.start, &fake, |_| async { Ok(()) }));
    assert_eq!(fake.ran()[0], format!("{} restart --state {}", wsp.display(), f.start.state.display()));
}

#[test]
fn a_restart_with_nothing_serving_runs_nothing_and_a_refused_one_is_its_words() {
    let mut f = fixture(Manager::Launchd, SERVICE_WAIT);
    let fake = Fake::new(|_| Ran {
        code: 1,
        output: "wsp restart: the host wsp up holds in a terminal comes back only from that terminal".to_owned(),
    });
    assert_eq!(block(restart(&f.start, &fake)), Err("this app carries no wsp of its own to restart the host with".to_owned()));
    let wsp = owned(&mut f);
    block(restart(&f.start, &fake)).unwrap();
    assert!(fake.ran().is_empty(), "{:?}", fake.ran());
    let pid = f.kid(Path::new("/bin/sleep"), &["30"]);
    f.write_lock(pid, 1, "up");
    assert_eq!(
        block(restart(&f.start, &fake)),
        Err("wsp restart: the host wsp up holds in a terminal comes back only from that terminal".to_owned())
    );
    assert_eq!(fake.ran(), [format!("{} restart --state {}", wsp.display(), f.start.state.display())]);
}

#[test]
fn the_real_runner_hands_the_wsp_command_its_own_names_and_runs_it_where_it_is_told() {
    let names = ["HOME", "PATH", "SHELL", "WSP_HOME", "WSP_HOST", "WSP_HOST_TOKEN", "ANTHROPIC_API_KEY", "GIT_DIR"];
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    let mut vars: Env = names.iter().map(|name| ((*name).to_owned(), "/usr/bin:/bin".to_owned())).collect();
    vars.insert("HOME".to_owned(), home.to_string_lossy().into_owned());
    let system = System::new(&vars);
    let script = "for n in HOME PATH SHELL WSP_HOME WSP_HOST WSP_HOST_TOKEN ANTHROPIC_API_KEY GIT_DIR; do eval \"[ -n \\\"\\${$n+x}\\\" ]\" && echo $n; done; pwd -P";
    let wsp_home = home.join(".wsp");
    std::fs::create_dir_all(&wsp_home).unwrap();
    let named = block(system.wsp(&words(&["sh", "-c", script]), &wsp_home));
    assert_eq!(named, Ran { code: 0, output: format!("HOME\nPATH\nSHELL\nWSP_HOME\n{}", wsp_home.display()) });
}

/// Catches: a launch that cannot tell the release of the host serving, which every release rule reads first.
#[test]
fn the_release_serving_is_the_one_its_page_names() {
    use std::io::{Read as _, Write as _};
    let f = fixture(Manager::Systemd, SERVICE_WAIT);
    assert_eq!(block(serving_release(&f.start.state)), None, "nothing serves");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for socket in listener.incoming().take(2) {
            let mut socket = socket.unwrap();
            let _ = socket.read(&mut [0u8; 1024]);
            let page = r#"<html><script>window.__WSP__ = {"wsPath":"/ws","version":"0.4.0","tokenHash":"x"};</script></html>"#;
            let _ = write!(socket, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{page}", page.len());
        }
    });
    f.write_lock(std::process::id() as i32, port, "service");
    assert_eq!(block(serving_release(&f.start.state)).as_deref(), Some("0.4.0"));
}

/// Catches: `wsp up --service` run in `$HOME`, which the host writes into the unit as its working folder, so a
/// person's `~/.env` holding a provider key wins over `~/.wsp/.env` at every login (`host:keys.ts`).
#[test]
fn the_wsp_command_writes_the_unit_from_the_wsp_home_and_not_the_persons_home() {
    let mut f = fixture(Manager::Systemd, SERVICE_WAIT);
    std::fs::remove_file(&f.unit).unwrap();
    let wsp = owned(&mut f);
    let home = f.start.home.canonicalize().unwrap();
    let ran_in = home.join("ran-in");
    std::fs::create_dir_all(wsp.parent().unwrap()).unwrap();
    std::fs::write(&wsp, format!("#!/bin/sh\npwd -P > '{}'\n", ran_in.display())).unwrap();
    std::fs::set_permissions(&wsp, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let vars: Env =
        [("HOME", home.to_string_lossy().as_ref()), ("PATH", "/usr/bin:/bin")].map(|(k, v)| (k.to_owned(), v.to_owned())).into();
    block(ensure(&f.start, &System::new(&vars))).unwrap();
    assert_eq!(std::fs::read_to_string(&ran_in).unwrap().trim(), home.join(".wsp").to_string_lossy());
}

/// Catches: a restart that ran nothing read as done, so the update's swap writes up and deletes the old app while
/// its host keeps serving off it, and the open after the ask says nothing of why.
#[test]
fn a_host_no_road_of_wsps_started_is_left_in_words_and_nothing_serving_is_said() {
    let mut f = fixture(Manager::Systemd, SERVICE_WAIT);
    let wsp = owned(&mut f);
    std::fs::write(&f.unit, format!("ExecStart={} up --state x\n", wsp.display())).unwrap();
    let fake = Fake::new(|_| ok(""));
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::NotServing));
    let pid = f.kid(Path::new("/bin/sleep"), &["30"]);
    let lock = format!(r#"{{"pid":{pid},"port":1,"startedAt":"2026-10-19T00:00:00.000Z"}}"#);
    std::fs::write(f.start.state.with_file_name("host.lock"), lock).unwrap();
    let state = f.start.state.display();
    let said = format!("wsp (pid {pid}) serves {state} from a process wsp did not start: stop it, then open wsp again");
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::Left(said)));
    assert!(fake.ran().is_empty(), "{:?}", fake.ran());
}

/// Catches: a host a terminal's `wsp up` or a verb started left on the older files, where today's app stops it by
/// its pid and starts the service after it (`replaceHost`).
#[test]
fn a_host_a_terminal_started_is_stopped_by_its_pid_and_the_service_started_after_it() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    std::fs::write(&f.unit, format!("ExecStart={} up --state x\n", wsp.display())).unwrap();
    let host = sh("exec sleep 30");
    f.write_lock(host, page("token"), "up");
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "is-enabled" => ok("enabled"),
        "show" => ok(INACTIVE),
        _ => ok(""),
    });
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::Restarted));
    assert!(!pid_alive(host), "the terminal's host was stopped");
    let name = Manager::Systemd.name(&f.start.state);
    assert!(fake.ran().contains(&format!("systemctl --user restart {name}")), "{:?}", fake.ran());
}

/// Catches: a service whose unit runs npm's wsp restarted onto npm's older files, or left, where today's app writes
/// the unit again to run its own command with the words it was given (`restartService`); and the rewrite's load
/// turning start at login back on for a unit the person set to start only when asked (`ensureService`'s `offAtLogin`).
#[test]
fn a_service_unit_running_another_wsp_is_written_again_to_run_the_command_with_its_words_and_login_switch() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    let state = f.start.state.display().to_string();
    std::fs::write(&f.unit, npm_unit(&f)).unwrap();
    let host = sh("exec sleep 30");
    let lock = f.write_lock(host, page("token"), "service");
    let fake = Fake::new(move |argv| match argv.get(2).map(String::as_str) {
        Some("is-enabled") => ok("disabled"),
        Some("stop") => {
            // SAFETY: the pid this test started and recorded, which the stop ends as the manager would.
            unsafe { libc::kill(host, libc::SIGTERM) };
            let _ = std::fs::remove_file(&lock);
            ok("")
        }
        _ => ok(""),
    });
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::Restarted));
    let name = Manager::Systemd.name(&f.start.state);
    assert_eq!(
        fake.ran(),
        [
            format!("systemctl --user is-enabled {name}"),
            format!("systemctl --user stop {name}"),
            format!("{} up --service --state {state} --port {}", wsp.display(), f.start.port),
            format!("systemctl --user disable {name}"),
        ]
    );
}

/// Catches: a launchd plist read with its `&amp;` left in, so under a home whose path holds `&` the unit running the
/// wsp command reads as another wsp's: its host is booted out and the unit written over instead of restarted.
#[test]
fn a_launchd_unit_running_the_command_under_a_home_with_an_ampersand_restarts_through_it() {
    let mut f = fixture(Manager::Launchd, Duration::from_secs(5));
    f.start.home = f.start.home.join("a & b");
    f.start.state = f.start.home.join(".wsp/state.json");
    std::fs::create_dir_all(f.start.state.parent().unwrap()).unwrap();
    let wsp = owned(&mut f);
    let unit = Manager::Launchd.unit(&f.start.home, &f.start.state);
    std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
    let xml = |path: &Path| path.to_string_lossy().replace('&', "&amp;");
    let plist = format!(
        "<plist version=\"1.0\">\n<dict>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{}</string>\n    <string>up</string>\n    <string>--state</string>\n    <string>{}</string>\n  </array>\n</dict>\n</plist>\n",
        xml(&wsp),
        xml(&f.start.state)
    );
    std::fs::write(&unit, plist).unwrap();
    let host = sh("exec sleep 30");
    let lock = f.write_lock(host, page("token"), "service");
    let fake = Fake::new(move |argv| {
        if argv.get(1).map(String::as_str) == Some("restart") {
            // SAFETY: the pid this test started and recorded, which the restart ends as a host's manager would.
            unsafe { libc::kill(host, libc::SIGTERM) };
            let _ = std::fs::remove_file(&lock);
        }
        ok("")
    });
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::Restarted));
    assert_eq!(fake.ran(), [format!("{} restart --state {}", wsp.display(), f.start.state.display())]);
}

/// Catches: a host a terminal started stopped and the service loaded from a unit that runs npm's older wsp, so the
/// host comes back on npm's files while the answer says it was restarted onto the command (`replaceHost` ends in
/// `ensureService`, which writes such a unit again).
#[test]
fn a_host_a_terminal_started_comes_back_on_the_command_where_the_unit_ran_another_wsp() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    std::fs::write(&f.unit, npm_unit(&f)).unwrap();
    let host = sh("exec sleep 30");
    f.write_lock(host, page("token"), "up");
    let fake = Fake::new(|argv| match argv.get(2).map(String::as_str) {
        Some("is-enabled") => ok("enabled"),
        Some("show") => ok(INACTIVE),
        _ => ok(""),
    });
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::Restarted));
    assert!(!pid_alive(host), "the terminal's host was stopped");
    let name = Manager::Systemd.name(&f.start.state);
    let up = format!("{} up --service --state {} --port {}", wsp.display(), f.start.state.display(), f.start.port);
    assert!(fake.ran().contains(&up), "{:?}", fake.ran());
    assert!(!fake.ran().contains(&format!("systemctl --user restart {name}")), "npm's unit is not loaded as it stands: {:?}", fake.ran());
}

/// Catches: nothing serving answered as the next start running the wsp command while the unit runs npm's older wsp,
/// which that start loads as it stands.
#[test]
fn nothing_serving_on_a_unit_running_another_wsp_writes_the_unit_again() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    std::fs::write(&f.unit, npm_unit(&f)).unwrap();
    let fake = Fake::new(|argv| match argv.get(2).map(String::as_str) {
        Some("show") => ok(INACTIVE),
        _ => ok(""),
    });
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::NotServing));
    let up = format!("{} up --service --state {} --port {}", wsp.display(), f.start.state.display(), f.start.port);
    assert!(fake.ran().contains(&up), "{:?}", fake.ran());
    std::fs::write(&f.unit, format!("ExecStart={} up --state x\n", wsp.display())).unwrap();
    let fake = Fake::new(|_| ok(""));
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::NotServing));
    assert!(fake.ran().is_empty(), "a unit running the command stays as it is: {:?}", fake.ran());
}

/// Catches: a rewrite read as failed where `wsp up --service` wrote and loaded the unit and only its host was slower
/// than its wait, so an update's swap waiting on the host puts the old app back while the host comes up.
#[test]
fn a_rewrite_whose_host_is_slower_than_its_wait_is_waited_on_and_one_that_wrote_nothing_is_refused() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    std::fs::write(&f.unit, npm_unit(&f)).unwrap();
    let (unit, runs) = (f.unit.clone(), format!("ExecStart={} up --state x\n", wsp.display()));
    let slow = Fake::new(move |argv| match argv.get(1).map(String::as_str) {
        Some("up") => {
            std::fs::write(&unit, &runs).unwrap();
            Ran { code: 1, output: "wsp did not start within 20s".to_owned() }
        }
        Some("--user") if argv[2] == "show" => ok(INACTIVE),
        _ => ok(""),
    });
    assert_eq!(block(follow(&f.start, &slow)), Ok(Followed::NotServing));
    assert!(f.said().contains(&"wsp up --service: wsp did not start within 20s".to_owned()), "{:?}", f.said());
    std::fs::write(&f.unit, npm_unit(&f)).unwrap();
    let refused = Fake::new(|argv| match argv.get(1).map(String::as_str) {
        Some("up") => Ran { code: 1, output: "wsp up --service: the port is taken".to_owned() },
        Some("--user") if argv[2] == "show" => ok(INACTIVE),
        _ => ok(""),
    });
    assert_eq!(block(follow(&f.start, &refused)), Err("wsp up --service: the port is taken".to_owned()));
}

/// Catches: the wait after a terminal's host is stopped reading any lock as that host, so npm's unit taking the lock
/// the moment it is let go is said to be the host that would not stop, and its unit is never written again.
#[test]
fn a_unit_taking_the_lock_once_the_terminals_host_lets_it_go_is_written_again() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    std::fs::write(&f.unit, npm_unit(&f)).unwrap();
    let port = page("token");
    let terminal = sh("exec sleep 30");
    let lock = f.write_lock(terminal, port, "up");
    let npm = sh("exec sleep 30");
    std::thread::spawn({
        let lock = lock.clone();
        move || {
            while pid_alive(terminal) {
                std::thread::sleep(Duration::from_millis(5));
            }
            let held = format!(r#"{{"pid":{npm},"port":{port},"startedAt":"2026-10-19T00:00:00.000Z","startedBy":"service"}}"#);
            std::fs::write(&lock, held).unwrap();
        }
    });
    let fake = Fake::new(move |argv| match argv.get(2).map(String::as_str) {
        Some("show") => ok("ActiveState=active\nUnitFileState=enabled\nMainPID=1"),
        Some("stop") => {
            // SAFETY: the pid this test started and recorded, which the stop ends as the manager would.
            unsafe { libc::kill(npm, libc::SIGTERM) };
            ok("")
        }
        _ => ok(""),
    });
    assert_eq!(block(follow(&f.start, &fake)), Ok(Followed::Restarted));
    let up = format!("{} up --service --state {} --port {}", wsp.display(), f.start.state.display(), f.start.port);
    assert!(fake.ran().contains(&up), "{:?}", fake.ran());
}

/// Catches: a stop that rests on the lock's pid being alive and something on its port naming a release, so after a
/// crash left the lock and the pid went to another process, with another home's host on that port, the update's
/// move signals a process that is not this home's host (`lockedHost` holds the page to the token beside the state).
#[test]
fn a_lock_whose_page_carries_another_tokens_digest_is_left_and_its_pid_never_signalled() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    owned(&mut f);
    let host = sh("exec sleep 30");
    f.write_lock(host, page("another home's token"), "up");
    let fake = Fake::new(|_| ok(""));
    let said = match block(follow(&f.start, &fake)) {
        Ok(Followed::Left(said)) => said,
        other => panic!("the host was not left: {other:?}"),
    };
    assert!(said.contains("carries another token's digest"), "{said}");
    assert!(pid_alive(host), "the process was never signalled");
    assert!(fake.ran().is_empty(), "{:?}", fake.ran());
}

#[test]
fn a_launchd_units_words_are_read_off_its_program_arguments() {
    let plist = "<dict><key>Label</key><string>com.wsp.host.x</string><key>ProgramArguments</key><array>\
                 <string>/Users/a &amp; b/.wsp/bin/wsp</string><string>up</string><string>--port</string><string>4400</string>\
                 </array><key>KeepAlive</key><true/></dict>";
    assert_eq!(Manager::Launchd.argv(plist), Some(words(&["/Users/a & b/.wsp/bin/wsp", "up", "--port", "4400"])));
    assert_eq!(Manager::Systemd.argv("ExecStart='/a b/wsp' 'up' --port 4400\n"), Some(words(&["/a b/wsp", "up", "--port", "4400"])));
}

/// A listener of the test's own holding a port, as another program would; its port.
fn held_port() -> (std::net::TcpListener, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

/// Catches: a fresh install whose port another wsp host holds, serving a state file of its own, handed to `wsp up
/// --service` as the port the unit names, so the service's host refuses it and the first run stops on a dialog. The
/// other host's lock, process and unit are never touched.
#[test]
fn a_fresh_install_whose_port_another_host_holds_writes_its_unit_on_the_next_free_port() {
    for manager in [Manager::Launchd, Manager::Systemd] {
        let mut f = fixture(manager, SERVICE_WAIT);
        std::fs::remove_file(&f.unit).unwrap();
        let wsp = owned(&mut f);
        let other = f.start.home.join("dev/.wsp/state.json");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        let (other_pid, held) = (sh("exec sleep 30"), page("the other home's token"));
        let lock = format!(r#"{{"pid":{other_pid},"port":{held},"startedAt":"2026-10-25T03:01:00.000Z","startedBy":"up"}}"#);
        std::fs::write(other.with_file_name("host.lock"), &lock).unwrap();
        f.start.port = held;
        let fake = Fake::new(|_| ok(""));
        block(ensure(&f.start, &fake)).unwrap();
        let ran = fake.ran();
        let port: u16 = ran[0].rsplit(' ').next().unwrap().parse().unwrap();
        assert!(port > held && port < held + 20, "the next free port after {held}, not {port}");
        assert_eq!(ran, [format!("{} up --service --state {} --port {port}", wsp.display(), f.start.state.display())]);
        let moved = format!(
            "port {held} is in use on this computer by another program, so the {} {} serves on {port}",
            manager.words(),
            manager.name(&f.start.state)
        );
        assert_eq!(f.said()[0], moved);
        assert!(pid_alive(other_pid), "the other host was never signalled");
        assert_eq!(std::fs::read_to_string(other.with_file_name("host.lock")).unwrap(), lock);
        assert!(!ran.iter().any(|line| line.contains(&manager.name(&other))), "{ran:?}");
    }
}

/// Catches: a later launch whose unit names a port another program took since, loaded as it stands, so its host
/// refuses the port at every start; the unit is stopped and written again on a free port, with its other words kept.
#[test]
fn a_unit_whose_port_another_program_holds_is_written_again_on_a_free_one() {
    for manager in [Manager::Launchd, Manager::Systemd] {
        let mut f = fixture(manager, Duration::from_secs(5));
        let wsp = owned(&mut f);
        let (_taken, held) = held_port();
        let state = f.start.state.display().to_string();
        let unit = match manager {
            Manager::Systemd => format!("ExecStart='{}' 'up' '--state' '{state}' '--port' '{held}' '--listen' '127.0.0.1'\n", wsp.display()),
            Manager::Launchd => format!(
                "<key>ProgramArguments</key><array><string>{}</string><string>up</string><string>--state</string><string>{state}</string><string>--port</string><string>{held}</string><string>--listen</string><string>127.0.0.1</string></array>",
                wsp.display()
            ),
        };
        std::fs::write(&f.unit, unit).unwrap();
        let fake = Fake::new(move |argv| match (manager, argv.get(1).map(String::as_str), argv.get(2).map(String::as_str)) {
            (Manager::Launchd, Some("print"), _) => ok("\tstate = waiting\n"),
            (Manager::Systemd, _, Some("show")) => ok("ActiveState=activating\nUnitFileState=enabled\nMainPID=0"),
            (Manager::Systemd, _, Some("is-enabled")) => ok("enabled"),
            _ => ok(""),
        });
        block(ensure(&f.start, &fake)).unwrap();
        let ran = fake.ran();
        let up = ran.iter().find(|line| line.contains(" up --service ")).unwrap_or_else(|| panic!("{ran:?}"));
        let port: u16 = up.split(' ').skip_while(|word| *word != "--port").nth(1).unwrap().parse().unwrap();
        assert_ne!(port, held);
        assert_eq!(*up, format!("{} up --service --state {state} --port {port} --listen 127.0.0.1", wsp.display()));
        let stop = manager.stop(&f.start).remove(0).join(" ");
        assert!(
            ran.iter().position(|line| *line == stop).expect("the unit was stopped") < ran.iter().position(|line| line == up).unwrap(),
            "the held unit is stopped first: {ran:?}"
        );
        assert!(
            f.said().iter().any(|line| line.contains(&format!("serves on port {held}, which another program holds"))),
            "{:?}",
            f.said()
        );
    }
}

/// Catches: a unit moved off a port that is free, or off the one its own host holds, which would leave the host the
/// app and the wsp command already reach.
#[test]
fn a_unit_on_a_free_port_or_its_own_hosts_is_loaded_as_it_stands() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    let (taken, held) = held_port();
    let unit = format!("ExecStart='{}' 'up' '--state' '{}' '--port' '{held}'\n", wsp.display(), f.start.state.display());
    std::fs::write(&f.unit, &unit).unwrap();
    let host = sh("exec sleep 30");
    f.write_lock(host, held, "service");
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "show" => ok(INACTIVE),
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    drop(taken);
    std::fs::remove_file(f.start.state.with_file_name("host.lock")).unwrap();
    block(ensure(&f.start, &fake)).unwrap();
    assert!(!fake.ran().iter().any(|line| line.contains("--service") || line.contains(" stop ")), "{:?}", fake.ran());
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), unit);
}

/// Catches: a rewrite onto a free port that `wsp up --service` refused before writing anything read as done, since the
/// unit already runs the wsp command, so the open waits out its whole wait and says nothing ran the service instead
/// of the command's refusal, at every launch.
#[test]
fn a_rewrite_off_a_held_port_that_the_command_refused_is_its_refusal() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    let (_taken, held) = held_port();
    let unit = format!("ExecStart='{}' 'up' '--state' '{}' '--port' '{held}'\n", wsp.display(), f.start.state.display());
    std::fs::write(&f.unit, &unit).unwrap();
    let refusal = "wsp up --service: a service starts without your shell, so it reads its provider key from a file.";
    let fake = Fake::new(move |argv| match argv.get(1).map(String::as_str) {
        Some("up") => Ran { code: 1, output: refusal.to_owned() },
        Some("--user") if argv[2] == "show" => ok(INACTIVE),
        _ => ok(""),
    });
    let failed = block(ensure(&f.start, &fake)).unwrap_err();
    assert_eq!(failed.failure.message, refusal);
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), unit);
}

/// A unit for the fixture's state running the wsp command on `port`.
fn unit_on(f: &Fixture, wsp: &Path, port: u16) -> String {
    format!("ExecStart='{}' 'up' '--state' '{}' '--port' '{port}'\n", wsp.display(), f.start.state.display())
}

const ACTIVATING: &str = "ActiveState=activating\nUnitFileState=enabled\nMainPID=0";

/// Catches: a recorded port another program held for a moment, as the launch read it, moving the unit for good; the
/// port is read again a moment later and the unit is loaded as it stands.
#[test]
fn a_recorded_port_held_for_a_moment_leaves_the_unit_as_it_stands() {
    let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
    let wsp = owned(&mut f);
    let (taken, held) = held_port();
    std::fs::write(&f.unit, unit_on(&f, &wsp, held)).unwrap();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        drop(taken);
    });
    let fake = Fake::new(|argv| match argv[2].as_str() {
        "show" => ok(ACTIVATING),
        _ => ok(""),
    });
    block(ensure(&f.start, &fake)).unwrap();
    assert!(!fake.ran().iter().any(|line| line.contains("--service") || line.contains(" stop ")), "{:?}", fake.ran());
    assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), unit_on(&f, &wsp, held));
}

/// Catches: a later launch moving the unit off a held port while a computer has joined this host, whose door moves
/// with the port and would no longer answer it; the joined computer read off the database, or off a state file not yet
/// moved into one. The host's own refusal stands, as before.
#[test]
fn a_unit_on_a_held_port_stays_while_a_computer_has_joined() {
    for store in ["state.db", "state.json"] {
        let mut f = fixture(Manager::Systemd, Duration::from_secs(5));
        let wsp = owned(&mut f);
        let (_taken, held) = held_port();
        std::fs::write(&f.unit, unit_on(&f, &wsp, held)).unwrap();
        if store == "state.db" {
            let db = rusqlite::Connection::open(f.start.state.with_file_name(store)).unwrap();
            db.execute_batch(
                "create table collections (collection text, id text, json text, primary key (collection, id));
                 insert into collections values ('places', 'p1', '{\"id\":\"p1\",\"name\":\"box\"}');",
            )
            .unwrap();
        } else {
            std::fs::write(&f.start.state, r#"{"workspaces":{},"places":{"p1":{"id":"p1","name":"box"}}}"#).unwrap();
        }
        let fake = Fake::new(|argv| match argv[2].as_str() {
            "show" => ok(ACTIVATING),
            _ => ok(""),
        });
        block(ensure(&f.start, &fake)).unwrap();
        assert!(!fake.ran().iter().any(|line| line.contains("--service") || line.contains(" stop ")), "{store}: {:?}", fake.ran());
        assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), unit_on(&f, &wsp, held));
        let stays = format!("since the computers joined to this host dial its door at {}", held + 20);
        assert!(f.said().iter().any(|line| line.contains(&stays)), "{store}: {:?}", f.said());
    }
}

/// Catches: a store with no computer in it read as one joined, so a later launch never moves; and a store that is
/// there and cannot be read read as none, so the unit moves out from under a joined computer.
#[test]
fn joined_reads_the_places_the_store_holds() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state.json");
    assert!(!joined(&state), "a home with no store has none");
    std::fs::write(&state, r#"{"places":{}}"#).unwrap();
    assert!(!joined(&state));
    std::fs::write(&state, "not json").unwrap();
    assert!(joined(&state), "a state file that does not read");
    let db = rusqlite::Connection::open(dir.path().join("state.db")).unwrap();
    db.execute_batch("pragma journal_mode = wal; create table collections (collection text, id text, json text); insert into collections values ('workspaces', 'w1', '{}');").unwrap();
    drop(db);
    assert!(!joined(&state), "the database has no places, and the old state file is passed over");
    std::fs::write(dir.path().join("state.db"), "not a database").unwrap();
    assert!(joined(&state), "a database that does not read");
}
