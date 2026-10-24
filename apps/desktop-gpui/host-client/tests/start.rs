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

/// Quit and stop, then the next launch: the stop keeps the unit and its login switch, so the next login still starts
/// wsp, and the open after it loads that unit again and attaches to the host it started.
fn stops_and_opens_again(manager: Manager, stops: impl Fn(&Stubbed) -> String) {
    let stubbed = Stubbed::new(Runs::Host);
    let road = stubbed.start(manager, start::SERVICE_WAIT);
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
    assert!(stubbed.starts_at_next_login(manager), "Quit and stop left wsp off at the next login: {:?}", stubbed.ran());
    assert!(stubbed.ran().contains(&stops(&stubbed)), "{:?}", stubbed.ran());
    assert_eq!(std::fs::read_to_string(manager.unit(&stubbed.home(), &stubbed.state())).unwrap(), UNIT);
    open();
    let again = stubbed.lock_pid().expect("the next host holds the lock");
    assert_ne!(again, first);
    assert_eq!(Some(again), stubbed.pid().map(i64::from), "the host the stub started again holds the lock");
}

#[test]
fn systemd_quit_and_stop_keeps_the_unit_and_the_next_open_starts_it_again() {
    stops_and_opens_again(Manager::Systemd, |stubbed| format!("systemctl --user stop {}", Manager::Systemd.name(&stubbed.state())));
}

#[test]
fn launchd_quit_and_stop_keeps_the_unit_and_the_next_open_starts_it_again() {
    // SAFETY: getuid reads this process's user and cannot fail.
    let uid = unsafe { libc::getuid() };
    stops_and_opens_again(Manager::Launchd, |stubbed| format!("launchctl bootout gui/{uid}/{}", Manager::Launchd.name(&stubbed.state())));
}

fn lock_port(state: &Path) -> Option<u16> {
    let lock = std::fs::read_to_string(state.with_file_name("host.lock")).ok()?;
    serde_json::from_str::<serde_json::Value>(&lock).ok()?["port"].as_u64()?.try_into().ok()
}

/// The port the systemd unit's own words name.
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
