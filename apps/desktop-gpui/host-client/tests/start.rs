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

/// What a stub runs as the unit's process: the throwaway host on the test's state, or `script` in its place.
enum Runs {
    Host,
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
case "$1" in
  print-disabled) echo 'disabled services = {{'; echo '}}' ;;
  print) [ -f "$loaded" ] || {{ echo "Could not find service" >&2; exit 113; }}
    if alive; then printf '\tstate = running\n\tpid = %s\n' "$pid"; else printf '\tstate = not running\n'; fi ;;
  bootstrap) touch "$loaded"; start ;;
  bootout) rm -f "$loaded"; alive && kill "$pid"; rm -f {pid} ;;
esac
"#,
            loaded = q(&home.join("loaded")),
            pid = q(&stubbed.pid_file())
        );
        // Enabled once enable ran, and still enabled after a stop, as systemd keeps it.
        let systemctl = format!(
            r#"{head}enabled={enabled}
if [ -f "$enabled" ]; then files=enabled; else files=disabled; fi
case "$2" in
  is-enabled) echo enabled ;;
  show) if alive; then printf 'ActiveState=active\nUnitFileState=%s\nMainPID=%s\n' "$files" "$pid"
    else printf 'ActiveState=inactive\nUnitFileState=%s\nMainPID=0\n' "$files"; fi ;;
  enable) touch "$enabled" ;;
  restart) start ;;
  stop) alive && kill "$pid"; rm -f {pid} ;;
esac
"#,
            enabled = q(&home.join("enabled")),
            pid = q(&stubbed.pid_file())
        );
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

/// Quit and stop, then the next launch: the stop keeps the unit, and the open after it loads that unit again and
/// attaches to the host it started.
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
    block(start::stop(&road, &runner)).expect("Quit and stop stops the host");
    assert_eq!(stubbed.lock_pid(), None, "the host the unit ran let the lock go");
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
