// SPDX-License-Identifier: AGPL-3.0-only
//! The host start's whole road with no service registered on this machine: stub `launchctl` and `systemctl` first on
//! the manager's PATH, a home of the test's own holding the unit today's app would have written, and the load starting
//! a throwaway host on the test's state, as launchd and systemd would start the unit. Needs node and the packages
//! built, as common's Throwaway does.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wsp_host_client::start::{self, Manager, Start, System};
use wsp_host_client::{Env, Failure};

const UNIT: &str = "the unit today's app wrote\n";

/// A home with a unit in it and the two stubs; the host a stub started is stopped by its pid when this goes.
struct Stubbed {
    dir: tempfile::TempDir,
    said: Arc<Mutex<Vec<String>>>,
}

/// What a stub runs as the unit's process: the throwaway host on the test's state, the line the systemd unit's own
/// `ExecStart=` names, or `script` in its place.
enum Runs {
    Host,
    #[cfg(target_os = "linux")]
    Unit,
    Script(&'static str),
    Nothing,
}

impl Stubbed {
    fn new(runs: Runs) -> Stubbed {
        let dir = tempfile::tempdir().unwrap();
        let stubbed = Stubbed { dir, said: Arc::new(Mutex::new(Vec::new())) };
        let home = stubbed.home();
        std::fs::create_dir_all(stubbed.state().parent().unwrap()).unwrap();
        for manager in [Manager::Launchd, Manager::Systemd] {
            let unit = manager.unit(&home, &stubbed.state());
            std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
            std::fs::write(unit, UNIT).unwrap();
        }
        let q = |path: &Path| format!("'{}'", path.display());
        let (state, log, pid, ran) = (q(&stubbed.state()), q(&stubbed.log()), q(&stubbed.pid_file()), q(&stubbed.ran_file()));
        let run = match runs {
            Runs::Host => {
                let bin = common::bundle_with(&home, None);
                format!(
                    "WSP_UPDATE_CHECK=0 WSP_STARTED_BY=service node {} up --port 0 --no-relay --state {state} </dev/null >>{log} 2>&1 &",
                    q(&bin)
                )
            }
            #[cfg(target_os = "linux")]
            Runs::Unit => {
                let unit = q(&Manager::Systemd.unit(&home, &stubbed.state()));
                format!(
                    r#"line=$(sed -n 's/^ExecStart=//p' {unit}); WSP_UPDATE_CHECK=0 WSP_STARTED_BY=service sh -c "exec $line" </dev/null >>{log} 2>&1 &"#
                )
            }
            Runs::Script(script) => format!("LOG={log} sh -c '{script}' </dev/null >/dev/null 2>&1 &"),
            Runs::Nothing => ":".to_owned(),
        };
        let started = if matches!(runs, Runs::Nothing) { String::new() } else { format!("echo $! > {pid}") };
        // Each stub keeps the unit alive as KeepAlive and Restart=always do: a process that exited is started again.
        let head = format!(
            r#"#!/bin/sh
[ "$1" = wsp-stub ] && {{ echo "stub $(basename "$0")"; exit 0; }}
echo "$(basename "$0") $*" >> {ran}
pid=$(cat {pid} 2>/dev/null)
alive() {{ [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; }}
start() {{ {run}
{started}
pid=$(cat {pid} 2>/dev/null); }}
[ -n "$pid" ] && ! alive && start
"#
        );
        let launchctl = format!(
            r#"{head}loaded={loaded}
disabled={disabled}
case "$1" in
  print-disabled) echo 'disabled services = {{'; [ -f "$disabled" ] && echo '	"{label}" => disabled'; echo '}}' ;;
  enable) rm -f "$disabled" ;;
  disable) touch "$disabled" ;;
  print) [ -f "$loaded" ] || {{ echo "Could not find service" >&2; exit 113; }}
    if alive; then printf '\tstate = running\n\tpid = %s\n' "$pid"; else printf '\tstate = not running\n'; fi ;;
  bootstrap) touch "$loaded"; start ;;
  bootout) rm -f "$loaded"; alive && kill "$pid"; rm -f {pid} ;;
esac
"#,
            loaded = q(&home.join("loaded")),
            disabled = q(&home.join("disabled")),
            label = Manager::Launchd.name(&stubbed.state()),
            pid = q(&stubbed.pid_file())
        );
        // Enabled as today's app leaves it, still enabled after a stop as systemd keeps it, and disabled by disable.
        let systemctl = format!(
            r#"{head}enabled={enabled}
if [ -f "$enabled" ]; then files=enabled; else files=disabled; fi
case "$2" in
  is-enabled) echo "$files" ;;
  show) if alive; then printf 'ActiveState=active\nUnitFileState=%s\nMainPID=%s\n' "$files" "$pid"
    else printf 'ActiveState=inactive\nUnitFileState=%s\nMainPID=0\n' "$files"; fi ;;
  enable) touch "$enabled" ;;
  restart) start ;;
  stop) alive && kill "$pid"; rm -f {pid} ;;
  disable) rm -f "$enabled"; if [ "$3" = --now ]; then alive && kill "$pid"; rm -f {pid}; fi ;;
esac
"#,
            enabled = q(&home.join("enabled")),
            pid = q(&stubbed.pid_file())
        );
        std::fs::write(home.join("enabled"), "").unwrap();
        std::fs::create_dir_all(stubbed.stubs()).unwrap();
        for (name, text) in [("launchctl", launchctl), ("systemctl", systemctl)] {
            let (path, draft) = (stubbed.stubs().join(name), stubbed.home().join(format!("{name}.draft")));
            std::fs::write(&draft, text).unwrap();
            // Copied by a child: a write handle of this process's, inherited by another test's fork, makes the exec ETXTBSY.
            assert!(std::process::Command::new("/bin/cp").arg(&draft).arg(&path).env_clear().status().unwrap().success());
            std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        }
        stubbed
    }

    /// Whether the next login starts the unit: launchd runs a plist at load unless its label is disabled, systemd an
    /// enabled unit.
    fn starts_at_next_login(&self, manager: Manager) -> bool {
        match manager {
            Manager::Launchd => manager.unit(&self.home(), &self.state()).exists() && !self.home().join("disabled").exists(),
            Manager::Systemd => self.home().join("enabled").exists(),
        }
    }

    fn home(&self) -> PathBuf {
        self.dir.path().to_owned()
    }

    fn state(&self) -> PathBuf {
        self.home().join(".wsp/state.json")
    }

    fn log(&self) -> PathBuf {
        self.state().with_file_name("host.log")
    }

    fn stubs(&self) -> PathBuf {
        self.home().join("stubs")
    }

    fn pid_file(&self) -> PathBuf {
        self.home().join("unit.pid")
    }

    fn ran_file(&self) -> PathBuf {
        self.home().join("ran")
    }

    fn ran(&self) -> Vec<String> {
        std::fs::read_to_string(self.ran_file()).unwrap_or_default().lines().map(str::to_owned).collect()
    }

    fn pid(&self) -> Option<i32> {
        std::fs::read_to_string(self.pid_file()).ok()?.trim().parse().ok()
    }

    /// The app's environment as the test hands it on: its own home, and the stubs ahead of everything on PATH.
    fn vars(&self) -> Env {
        let path = format!("{}:{}", self.stubs().display(), std::env::var("PATH").unwrap_or_default());
        [("HOME", self.home().display().to_string()), ("PATH", path), ("USER", "wsp-test".to_owned())]
            .map(|(k, v)| (k.to_owned(), v))
            .into_iter()
            .collect()
    }

    /// The start every test runs, once both managers resolve to the stubs through the real runner on the child's PATH,
    /// so a stub missed fails here and never reaches this machine's own launchctl or systemctl.
    fn start(&self, manager: Manager, wait: Duration) -> Start {
        let runner = System::new(&self.vars());
        for name in ["launchctl", "systemctl"] {
            let ran = block(start::Runner::run(&runner, &[name.to_owned(), "wsp-stub".to_owned()]));
            assert_eq!((ran.code, ran.output.trim()), (0, format!("stub {name}").as_str()), "{name} on the child's PATH is not the stub");
        }
        let said = self.said.clone();
        Start {
            state: self.state(),
            env: self.vars(),
            home: self.home(),
            // SAFETY: getuid reads this process's user and cannot fail.
            uid: unsafe { libc::getuid() },
            manager: Some(manager),
            wsp: None,
            restart: false,
            port: start::DEFAULT_PORT,
            wait,
            say: Arc::new(move |line| said.lock().unwrap().push(line.to_owned())),
        }
    }

    fn lock_pid(&self) -> Option<i64> {
        let lock = std::fs::read_to_string(self.state().with_file_name("host.lock")).ok()?;
        serde_json::from_str::<serde_json::Value>(&lock).ok()?["pid"].as_i64()
    }
}

impl Drop for Stubbed {
    fn drop(&mut self) {
        let Some(pid) = self.pid() else { return };
        // SAFETY: signals to the one process a stub started, by the pid it wrote.
        unsafe { libc::kill(pid, libc::SIGTERM) };
        let until = Instant::now() + Duration::from_secs(10);
        // SAFETY: signal 0 checks the pid and delivers nothing.
        while unsafe { libc::kill(pid, 0) } == 0 {
            if Instant::now() >= until {
                // SAFETY: as above, to a pid still alive, so not yet one the system handed to another process.
                unsafe { libc::kill(pid, libc::SIGKILL) };
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn block<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

/// Opens through the real road and reads the projects off the host it attached to; `loads` is what the manager
/// should have been asked first, in order.
fn attaches(manager: Manager, loads: impl Fn(&Stubbed) -> Vec<String>) {
    let stubbed = Stubbed::new(Runs::Host);
    let road = stubbed.start(manager, start::SERVICE_WAIT);
    let opened =
        block(start::ready(&road, &System::new(&road.env), |client| async move { client.projects().await.map(|_| client.here()) }));
    let (_, here) = opened.unwrap_or_else(|f| {
        let dial = block(wsp_host_client::HostClient::connect(&road.state, &road.env)).err().map(|f| f.message);
        panic!("{}\ndial: {dial:?}\nhost log: {}", f.failure.message, std::fs::read_to_string(stubbed.log()).unwrap_or_default())
    });
    assert!(here);
    assert_eq!(stubbed.lock_pid(), stubbed.pid().map(i64::from), "the host the stub started holds the lock");
    let (ran, loads) = (stubbed.ran(), loads(&stubbed));
    assert_eq!(ran[..loads.len()], loads, "{ran:?}");
    assert_eq!(std::fs::read_to_string(manager.unit(&stubbed.home(), &stubbed.state())).unwrap(), UNIT);
}

#[test]
fn systemd_loads_the_unit_and_the_app_attaches_to_the_host_it_started() {
    attaches(Manager::Systemd, |stubbed| {
        let name = Manager::Systemd.name(&stubbed.state());
        vec![
            format!("systemctl --user is-enabled {name}"),
            format!("systemctl --user show {name} --property=ActiveState,UnitFileState,MainPID"),
            "systemctl --user daemon-reload".to_owned(),
            format!("systemctl --user enable {name}"),
            format!("systemctl --user restart {name}"),
        ]
    });
}

#[test]
fn launchd_loads_the_unit_and_the_app_attaches_to_the_host_it_started() {
    attaches(Manager::Launchd, |stubbed| {
        // SAFETY: getuid reads this process's user and cannot fail.
        let uid = unsafe { libc::getuid() };
        let label = Manager::Launchd.name(&stubbed.state());
        let plist = Manager::Launchd.unit(&stubbed.home(), &stubbed.state());
        vec![
            format!("launchctl print-disabled gui/{uid}"),
            format!("launchctl print gui/{uid}/{label}"),
            format!("launchctl enable gui/{uid}/{label}"),
            format!("launchctl bootstrap gui/{uid} {}", plist.display()),
        ]
    });
}

#[test]
fn a_host_that_exits_as_it_starts_is_its_last_line() {
    let stubbed = Stubbed::new(Runs::Script("sleep 0.3; echo \"wsp: port 4400 is held by another program\" >> \"$LOG\"; exit 1"));
    let road = stubbed.start(Manager::Systemd, start::SERVICE_WAIT);
    let from = Instant::now();
    let failed = block(start::ready(&road, &System::new(&road.env), |_| async { Ok(()) })).map(|_| ()).unwrap_err();
    assert_eq!(failed.failure.message, "wsp stopped as it started: wsp: port 4400 is held by another program");
    assert_eq!(failed.log, Some(stubbed.log()));
    assert!(stubbed.said.lock().unwrap().contains(&"host: wsp: port 4400 is held by another program".to_owned()));
    assert!(from.elapsed() < start::SERVICE_WAIT, "two exits end the wait: {:?}", from.elapsed());
}

#[test]
fn a_service_that_starts_nothing_is_one_line_once_the_wait_runs_out() {
    let stubbed = Stubbed::new(Runs::Nothing);
    let road = stubbed.start(Manager::Launchd, Duration::from_millis(400));
    let failed = block(start::ready(&road, &System::new(&road.env), |_| async { Ok(()) })).map(|_| ()).unwrap_err();
    let message = &failed.failure.message;
    assert!(message.starts_with("wsp did not start within ") && message.ends_with(": nothing ran its service"), "{message}");
    assert!(stubbed.ran().iter().any(|argv| argv.starts_with("launchctl bootstrap ")), "{:?}", stubbed.ran());
}

#[test]
fn a_host_that_answers_and_refuses_the_first_read_is_that_refusal_with_no_second_dial() {
    let stubbed = Stubbed::new(Runs::Host);
    let road = stubbed.start(Manager::Systemd, start::SERVICE_WAIT);
    let asked = Arc::new(Mutex::new(0));
    let first = |_| {
        let asked = asked.clone();
        async move {
            let mut asked = asked.lock().unwrap();
            *asked += 1;
            if *asked > 1 {
                return Ok(());
            }
            Err(Failure::new("golden.get: refused"))
        }
    };
    let failed = block(start::ready(&road, &System::new(&road.env), first)).map(|_| ()).unwrap_err();
    assert_eq!(failed.failure.message, "golden.get: refused");
    assert_eq!(*asked.lock().unwrap(), 1);
}

/// Quit and stop, then the next launch: the stop unloads the unit and removes its file, as `wsp down` does, so the next
/// login starts nothing, and the open after it writes the unit again through the app's `wsp up --service`, stood in for
/// by a line that writes and loads it as that command does, and attaches to the host it started.
fn stops_and_opens_again(manager: Manager, stops: impl Fn(&Stubbed) -> String) {
    let stubbed = Stubbed::new(Runs::Host);
    let mut road = stubbed.start(manager, start::SERVICE_WAIT);
    let unit = manager.unit(&stubbed.home(), &stubbed.state());
    let name = manager.name(&stubbed.state());
    let (launchctl, systemctl) = (stubbed.stubs().join("launchctl"), stubbed.stubs().join("systemctl"));
    let load = match manager {
        Manager::Launchd => format!("'{}' bootstrap gui/{} '{}'", launchctl.display(), road.uid, unit.display()),
        Manager::Systemd => format!("'{0}' --user enable {name} && '{0}' --user restart {name}", systemctl.display()),
    };
    let (wsp, written) = (stubbed.home().join("wsp"), stubbed.home().join("unit.written"));
    std::fs::write(&written, UNIT).unwrap();
    std::fs::write(&wsp, format!("#!/bin/sh\ncp '{}' '{}' && {load}\n", written.display(), unit.display())).unwrap();
    std::fs::set_permissions(&wsp, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    road.wsp = Some(wsp);
    let runner = System::new(&road.env);
    let open = || {
        block(start::ready(&road, &runner, |_| async { Ok(()) })).map(|_| ()).unwrap_or_else(|f| {
            panic!(
                "{}\nran: {:?}\nhost log: {}",
                f.failure.message,
                stubbed.ran(),
                std::fs::read_to_string(stubbed.log()).unwrap_or_default()
            )
        })
    };
    open();
    let first = stubbed.lock_pid().expect("the first host holds the lock");
    assert!(stubbed.starts_at_next_login(manager), "the unit as today's app leaves it starts at login");
    block(start::stop(&road, &runner)).expect("Quit and stop stops the host");
    assert_eq!(stubbed.lock_pid(), None, "the host the unit ran let the lock go");
    assert!(stubbed.ran().contains(&stops(&stubbed)), "{:?}", stubbed.ran());
    assert!(!unit.exists(), "Quit and stop left the unit's file");
    assert!(!stubbed.starts_at_next_login(manager), "Quit and stop left wsp on at the next login: {:?}", stubbed.ran());
    open();
    assert_eq!(std::fs::read_to_string(&unit).unwrap(), UNIT, "the next open wrote the unit again");
    let again = stubbed.lock_pid().expect("the next host holds the lock");
    assert_ne!(again, first);
    assert_eq!(Some(again), stubbed.pid().map(i64::from), "the host the stub started again holds the lock");
}

#[test]
fn systemd_quit_and_stop_removes_the_unit_and_the_next_open_writes_it_again() {
    stops_and_opens_again(Manager::Systemd, |stubbed| {
        format!("systemctl --user disable --now {}", Manager::Systemd.name(&stubbed.state()))
    });
}

#[test]
fn launchd_quit_and_stop_removes_the_unit_and_the_next_open_writes_it_again() {
    // SAFETY: getuid reads this process's user and cannot fail.
    let uid = unsafe { libc::getuid() };
    stops_and_opens_again(Manager::Launchd, |stubbed| format!("launchctl bootout gui/{uid}/{}", Manager::Launchd.name(&stubbed.state())));
}

fn lock_port(state: &Path) -> Option<u16> {
    let lock = std::fs::read_to_string(state.with_file_name("host.lock")).ok()?;
    serde_json::from_str::<serde_json::Value>(&lock).ok()?["port"].as_u64()?.try_into().ok()
}

/// The port the systemd unit's own words name.
#[cfg(target_os = "linux")]
fn unit_port(stubbed: &Stubbed) -> Option<u16> {
    let unit = std::fs::read_to_string(Manager::Systemd.unit(&stubbed.home(), &stubbed.state())).ok()?;
    let argv = Manager::Systemd.argv(&unit)?;
    argv.get(argv.iter().position(|word| word == "--port")? + 1)?.parse().ok()
}

/// Catches: a fresh install whose `wsp up --service` writes `--port 4400` into the unit while another wsp host, on
/// another state file, holds 4400, so the service's host refuses the port it was named and the first run stops on a
/// dialog; and a later launch whose recorded port another program took. The port the host moved to is the unit's,
/// read by every later start, the wsp command and the app; the other host is never touched. Systemd's unit only, as
/// the host's own `wsp up --service` writes the manager of the platform it runs on.
#[cfg(target_os = "linux")]
#[test]
fn a_fresh_install_whose_port_another_host_holds_serves_on_a_free_one_the_unit_keeps() {
    let other = common::Throwaway::start();
    let (held, other_lock) = (
        lock_port(&other.state()).expect("the other host's port"),
        std::fs::read_to_string(other.state().with_file_name("host.lock")).unwrap(),
    );
    let stubbed = Stubbed::new(Runs::Unit);
    for manager in [Manager::Launchd, Manager::Systemd] {
        std::fs::remove_file(manager.unit(&stubbed.home(), &stubbed.state())).unwrap();
    }
    let wsp = stubbed.home().join(".wsp/bin/wsp");
    std::fs::create_dir_all(wsp.parent().unwrap()).unwrap();
    std::fs::write(&wsp, format!("#!/bin/sh\nexec node '{}' \"$@\"\n", common::bundle_with(&stubbed.home(), None).display())).unwrap();
    std::fs::set_permissions(&wsp, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut road = stubbed.start(Manager::Systemd, start::SERVICE_WAIT);
    road.wsp = Some(wsp.clone());
    road.port = held;
    let runner = System::new(&road.env);
    let open = || {
        block(start::ready(&road, &runner, |client| async move { client.projects().await.map(|_| client.here()) })).unwrap_or_else(|f| {
            panic!(
                "{}\nran: {:?}\nhost log: {}",
                f.failure.message,
                stubbed.ran(),
                std::fs::read_to_string(stubbed.log()).unwrap_or_default()
            )
        })
    };

    let (_, here) = open();
    assert!(here);
    let port = lock_port(&stubbed.state()).expect("the host holds the lock");
    assert_ne!(port, held, "the host serves beside the other one");
    assert_eq!(unit_port(&stubbed), Some(port), "the unit records the port its host serves on");
    let moved = format!(
        "port {held} is in use on this computer by another program, so the systemd unit {} serves on {port}",
        Manager::Systemd.name(&stubbed.state())
    );
    assert!(stubbed.said.lock().unwrap().contains(&moved), "{:?}", stubbed.said.lock().unwrap());
    let status = std::process::Command::new(&wsp)
        .args(["status", "--state", &stubbed.state().to_string_lossy()])
        .env_clear()
        .envs(&road.env)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&status.stdout);
    assert!(status.status.success() && said.contains(&format!(":{port}")), "wsp status: {said}{}", String::from_utf8_lossy(&status.stderr));
    assert_eq!(
        std::fs::read_to_string(other.state().with_file_name("host.lock")).unwrap(),
        other_lock,
        "the other host's lock is as it was"
    );
    block(wsp_host_client::HostClient::connect(&other.state(), &other.env())).expect("the other host still answers");

    block(start::stop(&road, &runner)).expect("Quit and stop stops the host");
    let (_, here) = open();
    assert!(here);
    assert_eq!(lock_port(&stubbed.state()), Some(port), "a later launch serves on the recorded port");
    assert_eq!(unit_port(&stubbed), Some(port));

    block(start::stop(&road, &runner)).expect("Quit and stop stops the host");
    let taken = std::net::TcpListener::bind(("127.0.0.1", port)).expect("the test holds the recorded port");
    let (_, here) = open();
    assert!(here);
    let again = lock_port(&stubbed.state()).expect("the host holds the lock");
    assert!(again != port && again != held, "{again}");
    assert_eq!(unit_port(&stubbed), Some(again), "the unit records the port it moved to");
    drop(taken);
    assert_eq!(std::fs::read_to_string(other.state().with_file_name("host.lock")).unwrap(), other_lock);
}

/// launchd and the wsp command as the Mac's first launch met them: `wsp up --service` writes the plist and its host
/// refuses `refused` in the host's own words, whatever this computer's probe reads of that port, and on any other
/// port a real host starts as the agent would start it.
struct MacLaunch {
    home: PathBuf,
    state: PathBuf,
    refused: u16,
    bin: PathBuf,
    host: Mutex<Option<std::process::Child>>,
    ran: Mutex<Vec<String>>,
}

impl MacLaunch {
    fn plist_port(&self) -> Option<u16> {
        let argv = Manager::Launchd.argv(&std::fs::read_to_string(Manager::Launchd.unit(&self.home, &self.state)).ok()?)?;
        argv.get(argv.iter().position(|word| word == "--port")? + 1)?.parse().ok()
    }

    fn host_pid(&self) -> Option<u32> {
        let mut host = self.host.lock().unwrap();
        let alive = host.as_mut().is_some_and(|child| matches!(child.try_wait(), Ok(None)));
        alive.then(|| host.as_ref().unwrap().id())
    }
}

/// The host's own refusal of a held port it was named (`portTakenLine`, `portInsteadLine`).
fn refusal(port: u16) -> String {
    let next = port + 1;
    format!(
        "Port {port} is in use on this computer by Electron (pid 24790).\nNothing was booted. The next free port is {next}: wsp up --port {next}. Or stop what holds the one you named.\n"
    )
}

impl start::Runner for MacLaunch {
    async fn run(&self, argv: &[String]) -> start::Ran {
        self.ran.lock().unwrap().push(argv.join(" "));
        let ok = |output: String| start::Ran { code: 0, output };
        match argv.get(1).map(String::as_str) {
            Some("print") => match self.host_pid() {
                Some(pid) => ok(format!("\tstate = running\n\tpid = {pid}\n")),
                None => {
                    // KeepAlive starts the plist's host again, and it refuses its port again in the log.
                    if self.plist_port() == Some(self.refused) {
                        let mut log =
                            std::fs::OpenOptions::new().create(true).append(true).open(self.state.with_file_name("host.log")).unwrap();
                        std::io::Write::write_all(&mut log, refusal(self.refused).as_bytes()).unwrap();
                    }
                    start::Ran { code: 113, output: "Could not find service".to_owned() }
                }
            },
            Some("bootout") => {
                if let Some(mut child) = self.host.lock().unwrap().take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                ok(String::new())
            }
            _ => ok(String::new()),
        }
    }

    async fn wsp(&self, argv: &[String], _cwd: &Path) -> start::Ran {
        self.ran.lock().unwrap().push(argv.join(" "));
        let port: u16 = argv[argv.iter().position(|word| word == "--port").unwrap() + 1].parse().unwrap();
        let words: String = argv.iter().map(|word| format!("<string>{word}</string>")).collect();
        let plist = Manager::Launchd.unit(&self.home, &self.state);
        std::fs::create_dir_all(plist.parent().unwrap()).unwrap();
        std::fs::write(&plist, format!("<plist><dict><key>ProgramArguments</key><array>{words}</array></dict></plist>\n")).unwrap();
        let log = self.state.with_file_name("host.log");
        if port == self.refused {
            let said = refusal(port);
            std::fs::write(&log, said.repeat(2)).unwrap();
            let loaded = format!(
                "the launchd agent {} loaded, but nothing answered on port {port} for {} within 20s.",
                Manager::Launchd.name(&self.state),
                self.state.display()
            );
            return start::Ran { code: 1, output: format!("{loaded}\n{said}") };
        }
        let out = std::fs::OpenOptions::new().create(true).append(true).open(&log).unwrap();
        let mut vars = common::env_in(&self.home);
        vars.insert("WSP_STARTED_BY".to_owned(), "service".to_owned());
        let child = std::process::Command::new("node")
            .arg(&self.bin)
            .args(["up", "--state", &self.state.to_string_lossy(), "--port", &port.to_string(), "--no-relay"])
            .env_clear()
            .envs(&vars)
            .current_dir(&self.home)
            .stdin(std::process::Stdio::null())
            .stdout(out.try_clone().unwrap())
            .stderr(out)
            .spawn()
            .unwrap();
        *self.host.lock().unwrap() = Some(child);
        let until = Instant::now() + Duration::from_secs(30);
        while lock_port(&self.state) != Some(port) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        start::Ran { code: 0, output: format!("launchd agent {} is loaded", Manager::Launchd.name(&self.state)) }
    }
}

impl Drop for MacLaunch {
    fn drop(&mut self) {
        if let Some(mut child) = self.host.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Catches: the Mac's first launch, where this computer's probe read the port free and the host the plist runs
/// refused it as held, stopping on the dialog with the plist still naming that port. The launch entry the window
/// calls (`start::ready`) moves the plist to the port the host's refusal named and opens on the host there.
#[test]
fn a_launch_whose_host_refuses_the_port_the_probe_read_free_moves_to_the_one_it_named() {
    refused_launch(false, false);
}

/// Catches: a later launch whose plist names a port the probe reads free and its host refuses at every start, as
/// launchd starts it again, loaded as it stands at every launch; once the host has refused it, the plist moves.
#[test]
fn a_later_launch_whose_host_refuses_its_recorded_port_moves_to_the_one_it_named() {
    refused_launch(true, false);
}

/// Catches: the move on the host's refusal taken while a computer has joined this host, whose door moves with the
/// port; the plist stays and the host's own refusal is the answer, as before.
#[test]
fn a_later_launch_whose_host_refuses_its_port_stays_while_a_computer_has_joined() {
    refused_launch(true, true);
}

fn refused_launch(standing: bool, joined: bool) {
    let dir = common::tempdir();
    let home = dir.path().to_owned();
    let state = home.join(".wsp/state.json");
    std::fs::create_dir_all(state.parent().unwrap()).unwrap();
    let refused = loop {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        if port < u16::MAX && std::net::TcpListener::bind(("127.0.0.1", port + 1)).is_ok() {
            break port;
        }
    };
    let wsp = home.join(".wsp/bin/wsp");
    let launch = MacLaunch {
        home: home.clone(),
        state: state.clone(),
        refused,
        bin: common::bundle_with(&home, None),
        host: Mutex::new(None),
        ran: Mutex::new(Vec::new()),
    };
    if standing {
        let words: String = [wsp.to_string_lossy().as_ref(), "up", "--state", &state.to_string_lossy(), "--port", &refused.to_string()]
            .iter()
            .map(|word| format!("<string>{word}</string>"))
            .collect();
        let plist = Manager::Launchd.unit(&home, &state);
        std::fs::create_dir_all(plist.parent().unwrap()).unwrap();
        std::fs::write(&plist, format!("<plist><dict><key>ProgramArguments</key><array>{words}</array></dict></plist>\n")).unwrap();
    }
    if joined {
        let db = rusqlite::Connection::open(state.with_file_name("state.db")).unwrap();
        db.execute_batch(
            "create table collections (collection text, id text, json text); insert into collections values ('places', 'p1', '{}');",
        )
        .unwrap();
    }
    let said = Arc::new(Mutex::new(Vec::new()));
    let kept = said.clone();
    let road = Start {
        state: state.clone(),
        env: common::env_in(&home),
        home: home.clone(),
        uid: 501,
        manager: Some(Manager::Launchd),
        wsp: Some(wsp.clone()),
        restart: false,
        port: refused,
        wait: if standing { Duration::from_secs(2) } else { start::SERVICE_WAIT },
        say: Arc::new(move |line| kept.lock().unwrap().push(line.to_owned())),
    };
    let opened = block(start::ready(&road, &launch, |client| async move { client.projects().await.map(|_| client.here()) }));
    if joined {
        let failed = opened.map(|_| ()).unwrap_err();
        assert!(failed.failure.message.contains(&format!("The next free port is {}", refused + 1)), "{}", failed.failure.message);
        assert_eq!(launch.plist_port(), Some(refused), "the plist stays on its port");
        assert!(!launch.ran.lock().unwrap().iter().any(|line| line.contains(" up --service ")), "{:?}", launch.ran.lock().unwrap());
        return;
    }
    let (_, here) =
        opened.unwrap_or_else(|f| panic!("{}\nran: {:?}\nsaid: {:?}", f.failure.message, launch.ran.lock().unwrap(), said.lock().unwrap()));
    assert!(here);
    let next = refused + 1;
    let plist = std::fs::read_to_string(Manager::Launchd.unit(&home, &state)).unwrap();
    let argv = Manager::Launchd.argv(&plist).unwrap();
    let recorded: u16 = argv[argv.iter().position(|word| word == "--port").unwrap() + 1].parse().unwrap();
    // Another test may take the named port first, and the probe steps past it.
    assert!(recorded >= next && recorded < refused + 20, "{plist}");
    assert_eq!(lock_port(&state), Some(recorded), "the host serves on the port the plist records");
    let moved = format!("which its host refused as in use, so it moves to {next}");
    assert!(said.lock().unwrap().iter().any(|line| line.contains(&moved)), "{:?}", said.lock().unwrap());
}
