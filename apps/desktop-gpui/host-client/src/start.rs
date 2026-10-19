// SPDX-License-Identifier: AGPL-3.0-only
//! The host a launch opens on (`desktop:host-lifecycle.ts` `openHostReady`): the one the lock names, else the one
//! this computer's own service manager brings up from the unit today's app wrote, waited on until it answers. The unit
//! is loaded as it stands and never written, rewritten or removed here: it runs today's app's shim, and this app
//! carries no node to run a unit of its own. Quit and stop wsp stops the host and keeps the unit for the next launch.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{Env, Failure, HostClient};

/// `SERVICE_WAIT_MS`: how long a start goes with no process running the unit before it is read as failed.
pub const SERVICE_WAIT: Duration = Duration::from_secs(20);
/// `STARTING_WAITS`: how many of those waits a host may run with nothing moving before it is read as stuck.
const STARTING_WAITS: u32 = 6;
const POLL: Duration = Duration::from_millis(200);
/// `MANAGER_ASK_MS`: how often the manager is asked for the process it runs while no lock names one.
const MANAGER_ASK: Duration = Duration::from_secs(1);
/// `EXITS_FAILED`: the host on its way down under a restart is one exit, and a host that fails as it starts is lost
/// again as the manager runs it the next time.
const EXITS_FAILED: u32 = 2;
/// `MANAGER_WAIT_MS`: what a manager's command gets to answer in.
const MANAGER_WAIT: Duration = Duration::from_secs(15);
/// `PROBE_MS`.
const PROBE: Duration = Duration::from_secs(2);
const TAIL_LINES: usize = 20;
const LOCK_FILE: &str = "host.lock";
const LOG_FILE: &str = "host.log";

/// The names a manager's command keeps from the app's environment: where the person's units sit, where the manager
/// is found, and for `systemctl --user` the login's bus.
const MANAGER_ENV: [&str; 5] = ["HOME", "PATH", "USER", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"];

/// What a start with no unit to load says: today's app writes the unit the first time it opens.
pub const NOT_SET_UP: &str = "wsp is not set up on this computer yet: open the wsp app once, then open this one";

/// The two managers `host:service.ts` writes units for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manager {
    Launchd,
    Systemd,
}

/// The ActiveStates of a unit that has a process or is about to (`SYSTEMD_RUNNING`).
const SYSTEMD_RUNNING: [&str; 5] = ["active", "activating", "deactivating", "reloading", "refreshing"];
/// The UnitFileStates of a unit systemd starts at login (`SYSTEMD_ENABLED`).
const SYSTEMD_ENABLED: [&str; 4] = ["enabled", "enabled-runtime", "linked", "linked-runtime"];

impl Manager {
    /// This computer's own manager, where it has one wsp writes units for.
    pub fn here() -> Option<Manager> {
        if cfg!(target_os = "macos") {
            Some(Manager::Launchd)
        } else if cfg!(target_os = "linux") {
            Some(Manager::Systemd)
        } else {
            None
        }
    }

    /// What the manager calls the host's unit for this state file.
    pub fn name(self, state: &Path) -> String {
        match self {
            Manager::Launchd => format!("com.wsp.host.{}", tag(state)),
            Manager::Systemd => format!("wsp-host-{}.service", tag(state)),
        }
    }

    /// The file the manager reads that unit from, under the person's home.
    pub fn unit(self, home: &Path, state: &Path) -> PathBuf {
        match self {
            Manager::Launchd => home.join("Library/LaunchAgents").join(format!("{}.plist", self.name(state))),
            Manager::Systemd => home.join(".config/systemd/user").join(self.name(state)),
        }
    }

    fn domain(uid: u32) -> String {
        format!("gui/{uid}")
    }

    fn target(self, start: &Start) -> String {
        format!("{}/{}", Manager::domain(start.uid), self.name(&start.state))
    }

    /// Run in order to have the unit serve now and again at login.
    pub fn load(self, start: &Start) -> Vec<Vec<String>> {
        match self {
            Manager::Launchd => vec![
                words(&["launchctl", "enable", &self.target(start)]),
                words(&["launchctl", "bootstrap", &Manager::domain(start.uid), &self.unit(&start.home, &start.state).to_string_lossy()]),
            ],
            Manager::Systemd => {
                let name = self.name(&start.state);
                vec![
                    words(&["systemctl", "--user", "daemon-reload"]),
                    words(&["systemctl", "--user", "enable", &name]),
                    words(&["systemctl", "--user", "restart", &name]),
                ]
            }
        }
    }

    /// Stops the host the unit runs and leaves the unit's file and its login switch as they are (`launchdUnload`,
    /// `systemdStop`): launchd lets the label go and the next load bootstraps it, systemd keeps it enabled.
    pub fn stop(self, start: &Start) -> Vec<String> {
        match self {
            Manager::Launchd => words(&["launchctl", "bootout", &self.target(start)]),
            Manager::Systemd => words(&["systemctl", "--user", "stop", &self.name(&start.state)]),
        }
    }

    /// Sets a loaded unit to start only when asked, leaving it running.
    pub fn off_at_login(self, start: &Start) -> Vec<String> {
        match self {
            Manager::Launchd => words(&["launchctl", "disable", &self.target(start)]),
            Manager::Systemd => words(&["systemctl", "--user", "disable", &self.name(&start.state)]),
        }
    }

    /// The command whose answer says whether the unit starts at login.
    pub fn login_read(self, start: &Start) -> Vec<String> {
        match self {
            Manager::Launchd => words(&["launchctl", "print-disabled", &Manager::domain(start.uid)]),
            Manager::Systemd => words(&["systemctl", "--user", "is-enabled", &self.name(&start.state)]),
        }
    }

    /// That answer read; nothing where the manager said neither.
    fn starts_at_login(self, ran: &Ran, state: &Path) -> Option<bool> {
        match self {
            // A disabled label reads `"label" => disabled`, or `=> true` before macOS 13; a label never switched is absent.
            Manager::Launchd => {
                let quoted = format!("\"{}\" => ", self.name(state));
                (ran.code == 0).then(|| !["disabled", "true"].iter().any(|said| ran.output.contains(&format!("{quoted}{said}"))))
            }
            Manager::Systemd => match ran.output.trim().lines().last().map(str::trim) {
                Some("enabled") => Some(true),
                Some("disabled") => Some(false),
                _ => None,
            },
        }
    }

    /// The command whose answer says whether the manager holds the unit, and the process it runs.
    pub fn holds(self, start: &Start) -> Vec<String> {
        match self {
            Manager::Launchd => words(&["launchctl", "print", &self.target(start)]),
            Manager::Systemd => {
                words(&["systemctl", "--user", "show", &self.name(&start.state), "--property=ActiveState,UnitFileState,MainPID"])
            }
        }
    }

    fn holding(self, ran: &Ran) -> bool {
        match self {
            Manager::Launchd => ran.code == 0,
            Manager::Systemd => {
                let said = |name: &str| property(&ran.output, name).unwrap_or_default();
                ran.code == 0 && (SYSTEMD_RUNNING.contains(&said("ActiveState")) || SYSTEMD_ENABLED.contains(&said("UnitFileState")))
            }
        }
    }

    /// A unit the manager holds with no process and none on its way, as systemd keeps one enabled that was stopped;
    /// launchd lets a stopped label go, so a label it holds is always on its way.
    fn idle(self, ran: &Ran) -> bool {
        match self {
            Manager::Launchd => false,
            Manager::Systemd => ran.code == 0 && !SYSTEMD_RUNNING.contains(&property(&ran.output, "ActiveState").unwrap_or_default()),
        }
    }

    /// The process the unit runs now; launchctl answers 113 for a label it does not have, and 0 is no process.
    fn pid_of(self, ran: &Ran) -> Option<i32> {
        if ran.code != 0 {
            return None;
        }
        // The service's own pid sits one tab in; a nested block a tab deeper carries pid lines that are not the service's.
        let pid = match self {
            Manager::Launchd => ran.output.lines().find_map(|line| line.strip_prefix("\tpid = ")?.parse::<i32>().ok()),
            Manager::Systemd => property(&ran.output, "MainPID").and_then(|pid| pid.parse::<i32>().ok()),
        };
        pid.filter(|pid| *pid > 0)
    }
}

fn property<'a>(output: &'a str, name: &str) -> Option<&'a str> {
    output.lines().find_map(|line| line.strip_prefix(name)?.strip_prefix('=')).map(str::trim)
}

fn words(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|word| (*word).to_owned()).collect()
}

/// `serviceTag`: the first eight hex of the state path's sha256, which every manager's name for its unit carries.
pub fn tag(state: &Path) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, state.to_string_lossy().as_bytes());
    digest.as_ref()[..4].iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `noManagerLine`: a computer wsp writes no unit for, in node's name for its platform.
pub fn no_manager_line(os: &str) -> String {
    let platform = match os {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    format!(
        "wsp writes no service on {platform}; it writes a launchd agent on darwin and a systemd unit on linux. Run wsp up in a terminal that stays open instead."
    )
}

/// What a manager's command answered: its exit code, and whatever it said on either stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    pub code: i32,
    pub output: String,
}

/// Runs a manager's command, as `ServiceDeps.run`; the real one is [`System`].
pub trait Runner: Sync {
    fn run(&self, argv: &[String]) -> impl Future<Output = Ran> + Send;
}

/// `systemRunner`: the manager by its bare name off PATH, 15 s to answer, and 127 where it is not there or never
/// answered, with nothing of the app's environment but what a manager needs.
pub struct System {
    env: Vec<(String, String)>,
}

impl System {
    pub fn new(env: &Env) -> System {
        System { env: MANAGER_ENV.iter().filter_map(|name| Some(((*name).to_owned(), env.get(*name)?.clone()))).collect() }
    }
}

impl Runner for System {
    async fn run(&self, argv: &[String]) -> Ran {
        let Some((program, args)) = argv.split_first() else { return Ran { code: 127, output: String::new() } };
        let mut command = tokio::process::Command::new(program);
        command.args(args).env_clear().envs(self.env.iter().cloned()).stdin(Stdio::null()).kill_on_drop(true);
        match tokio::time::timeout(MANAGER_WAIT, command.output()).await {
            Ok(Ok(out)) => {
                let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_owned();
                Ran { code: if out.status.success() { 0 } else { out.status.code().unwrap_or(127) }, output: said }
            }
            Ok(Err(e)) => Ran { code: 127, output: format!("{program}: {e}") },
            Err(_) => Ran { code: 127, output: format!("{program} did not answer within {}", fmt_wait(MANAGER_WAIT)) },
        }
    }
}

/// `runFailureLine`: what was run, its code and what it said, on one line.
pub fn run_failure_line(argv: &[String], ran: &Ran) -> String {
    let said = ran.output.split_whitespace().collect::<Vec<_>>().join(" ");
    let said = if said.is_empty() { "and said nothing".to_owned() } else { format!("and said: {said}") };
    format!("{} exited {} {said}", argv.join(" "), ran.code)
}

/// `runAll`: in order, stopping at the first that refuses, which is the answer.
async fn run_all<R: Runner>(commands: &[Vec<String>], runner: &R) -> Option<(Vec<String>, Ran)> {
    for argv in commands {
        let ran = runner.run(argv).await;
        if ran.code != 0 {
            return Some((argv.clone(), ran));
        }
    }
    None
}

/// One start: the state file, the app's environment the dial reads, and this computer's manager.
#[derive(Clone)]
pub struct Start {
    pub state: PathBuf,
    pub env: Env,
    /// The person's home folder, where the manager reads unit files from.
    pub home: PathBuf,
    /// The user launchd names its domain by.
    pub uid: u32,
    pub manager: Option<Manager>,
    /// `SERVICE_WAIT_MS`, shorter in a test.
    pub wait: Duration,
    /// Where the lines a person reads in the app's log go.
    pub say: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Start {
    /// A start on this computer: its own manager, the HOME the app was launched with and this login's uid.
    pub fn here(state: &Path, env: &Env) -> Start {
        let home = env.get("HOME").filter(|h| !h.is_empty()).map(PathBuf::from).or_else(std::env::home_dir).unwrap_or_default();
        Start {
            state: state.to_owned(),
            env: env.clone(),
            home,
            // SAFETY: getuid reads this process's user and cannot fail.
            uid: unsafe { libc::getuid() },
            manager: Manager::here(),
            wait: SERVICE_WAIT,
            say: Arc::new(|line| eprintln!("desktop-gpui: {line}")),
        }
    }

    fn log(&self) -> PathBuf {
        self.state.with_file_name(LOG_FILE)
    }
}

/// `StartFailed`, and every other refusal an open ends on: the failure, and the log a failed start points to.
#[derive(Debug)]
pub struct Failed {
    pub failure: Box<Failure>,
    /// The host's log where the start itself failed; none for a refusal, whose box opens whichever log there is.
    pub log: Option<PathBuf>,
}

impl From<Failure> for Failed {
    fn from(failure: Failure) -> Failed {
        Failed { failure: Box::new(failure), log: None }
    }
}

fn line(message: impl Into<String>) -> Failed {
    Failure::new(message).into()
}

/// The host's lock beside the state file.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Lock {
    pid: i32,
    port: u16,
    #[serde(default)]
    address: Option<String>,
    /// Which road brought the host up: `service`, or a line's (`up`, `verb`); none for a host started by hand.
    #[serde(default)]
    started_by: Option<String>,
    /// Read so a lock without it is no lock, as wsp's reader holds it.
    #[allow(dead_code)]
    started_at: String,
}

fn read_lock(state: &Path) -> Option<Lock> {
    serde_json::from_str(&std::fs::read_to_string(state.with_file_name(LOCK_FILE)).ok()?).ok()
}

/// `servingHost`: the lock, while its process is alive and its program is still there.
fn serving(state: &Path) -> Option<Lock> {
    read_lock(state).filter(|lock| pid_alive(lock.pid) && !runs_vanished(lock.pid))
}

/// `vanishedHost`: a lock whose process still runs a program that is gone, which to its manager is up.
fn vanished(state: &Path) -> bool {
    read_lock(state).is_some_and(|lock| pid_alive(lock.pid) && runs_vanished(lock.pid))
}

/// `pidAlive`: a live process, whoever owns it.
fn pid_alive(pid: i32) -> bool {
    // SAFETY: signal 0 checks the pid and delivers nothing.
    pid > 0 && unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

/// `runsVanished`: read off /proc, so a computer without one never says so; an update in place is not gone.
fn runs_vanished(pid: i32) -> bool {
    let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe")) else { return false };
    let exe = exe.to_string_lossy();
    !Path::new(exe.strip_suffix(" (deleted)").unwrap_or(&exe)).exists()
}

/// `httpProbe`: whether anything answers one GET where the lock says the host is.
async fn answers(lock: &Lock) -> bool {
    let address = match lock.address.as_deref() {
        None | Some("0.0.0.0") | Some("::") => "127.0.0.1",
        Some(address) => address,
    };
    let at = if address.contains(':') { format!("[{address}]:{}", lock.port) } else { format!("{address}:{}", lock.port) };
    let asked = async {
        let mut tcp = TcpStream::connect(&at).await.ok()?;
        tcp.write_all(format!("GET / HTTP/1.1\r\nHost: {at}\r\nConnection: close\r\n\r\n").as_bytes()).await.ok()?;
        let mut first = [0u8; 1];
        (tcp.read(&mut first).await.ok()? == 1).then_some(())
    };
    matches!(tokio::time::timeout(PROBE, asked).await, Ok(Some(())))
}

fn log_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// `logSince`: the log's non-empty lines past a mark.
fn log_since(path: &Path, from: u64) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    let from = usize::try_from(from).unwrap_or(usize::MAX).min(bytes.len());
    String::from_utf8_lossy(&bytes[from..]).lines().filter(|line| !line.trim().is_empty()).map(str::to_owned).collect()
}

/// `stoppedLine`: a host that exited as it started, in its own last line.
pub fn stopped_line(last: Option<&str>) -> String {
    match last {
        None => "wsp stopped as it started and wrote nothing to its log".to_owned(),
        Some(last) => format!("wsp stopped as it started: {}", last.trim()),
    }
}

/// `fmtDuration` in its short style.
pub fn fmt_wait(wait: Duration) -> String {
    let ms = wait.as_secs_f64() * 1000.0;
    if ms < 1_000.0 {
        return format!("{}ms", ms.round().max(1.0));
    }
    if ms < 10_000.0 {
        let tenths = (ms / 100.0).round() / 10.0;
        return if tenths >= 10.0 { "10s".to_owned() } else { format!("{tenths:.1}s") };
    }
    if ms < 60_000.0 {
        return format!("{}s", (ms / 1_000.0).round());
    }
    let total = (ms / 1_000.0).round() as u64;
    let (minutes, seconds) = (total / 60, total % 60);
    if seconds == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

/// `ensureService` with a unit that is never written: the unit today's app wrote, loaded as it stands where the
/// manager has let it go, holds it with nothing running it and nothing serving, or runs a host whose program has gone.
/// A unit set not to start at login is loaded all the same and set back off once loaded.
pub async fn ensure<R: Runner>(start: &Start, runner: &R) -> Result<(), Failed> {
    let Some(manager) = start.manager else { return Err(line(no_manager_line(std::env::consts::OS))) };
    if !manager.unit(&start.home, &start.state).exists() {
        return Err(line(NOT_SET_UP));
    }
    let off_at_login = manager.starts_at_login(&runner.run(&manager.login_read(start)).await, &start.state) == Some(false);
    let holds = manager.holds(start);
    let held = runner.run(&holds).await;
    if !manager.holding(&held) || (manager.idle(&held) && serving(&start.state).is_none()) || vanished(&start.state) {
        // A load refused because the manager holds the unit is another launch that loaded it a moment before.
        if let Some((argv, ran)) = run_all(&manager.load(start), runner).await {
            let held = runner.run(&holds).await;
            if !manager.holding(&held) || manager.idle(&held) {
                return Err(line(run_failure_line(&argv, &ran)));
            }
        }
    }
    if off_at_login {
        if let Some((argv, ran)) = run_all(&[manager.off_at_login(start)], runner).await {
            (start.say)(&run_failure_line(&argv, &ran));
        }
    }
    Ok(())
}

/// `untilLock` for the lock to go: the host still serving once `wait` has passed, else none.
async fn until_gone(state: &Path, wait: Duration) -> Option<Lock> {
    let until = Instant::now() + wait;
    loop {
        let lock = serving(state)?;
        if Instant::now() >= until {
            return Some(lock);
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Quit and stop wsp's stop (`stopWsp` less the unit's removal): the manager stops the host it runs and keeps the unit
/// today's app wrote, and a host a line started is asked to end by the pid its lock records, as `wsp down` does. The
/// answer is the refusal, or the host still serving once the start's wait has passed.
pub async fn stop<R: Runner>(start: &Start, runner: &R) -> Result<(), String> {
    let lock = serving(&start.state);
    let mut asked = false;
    if let Some(manager) = start.manager {
        if manager.holding(&runner.run(&manager.holds(start)).await) {
            let argv = manager.stop(start);
            let ran = runner.run(&argv).await;
            if ran.code != 0 {
                return Err(run_failure_line(&argv, &ran));
            }
            asked = true;
        }
    }
    let Some(lock) = lock else { return Ok(()) };
    let by = lock.started_by.as_deref();
    if by.is_some_and(|by| by != "service") {
        // SAFETY: a signal to the pid the host wrote in its own lock, which `serving` read as alive.
        unsafe { libc::kill(lock.pid, libc::SIGTERM) };
        asked = true;
    }
    let left = until_gone(&start.state, if asked { start.wait } else { Duration::ZERO }).await;
    let Some(left) = left else { return Ok(()) };
    let host = match by {
        Some("up") => "the host wsp up started",
        Some("verb") => "the host a verb started",
        _ => "the host",
    };
    Err(format!("{host} (pid {}) is still serving {}", left.pid, start.state.display()))
}

/// `untilAttachable`: waits for a host the dial attaches to, for as long as a process runs the service and something
/// moves (the lock taken, a line in its log). The process is the lock's, else the one the manager says it runs. It
/// fails once no process has run it for the start's wait, once two have exited, or once one has run the longer wait
/// with nothing moving.
pub async fn until_attachable<R: Runner>(start: &Start, runner: &R) -> Result<HostClient, Failed> {
    let log = start.log();
    let from = Instant::now();
    let mark = log_size(&log);
    let (mut pid, mut exits, mut asked) = (None::<i32>, 0, None::<Instant>);
    let (mut alive_at, mut moved_at, mut moved) = (from, from, String::new());
    let failed = |message: String| {
        let said = log_since(&log, mark);
        for line in &said[said.len().saturating_sub(TAIL_LINES)..] {
            (start.say)(&format!("host: {line}"));
        }
        Failed { failure: Box::new(Failure::new(message)), log: Some(log.clone()) }
    };
    loop {
        let lock = serving(&start.state);
        if lock.is_some() {
            if let Ok(client) = HostClient::connect(&start.state, &start.env).await {
                return Ok(client);
            }
        }
        let now = Instant::now();
        if pid.is_some_and(|pid| !pid_alive(pid)) {
            exits += 1;
            pid = None;
        }
        if let Some(lock) = &lock {
            pid = Some(lock.pid);
        } else if let (None, Some(manager)) = (pid, start.manager) {
            if asked.is_none_or(|at| now - at >= MANAGER_ASK) {
                asked = Some(now);
                pid = manager.pid_of(&runner.run(&manager.holds(start)).await).filter(|pid| pid_alive(*pid));
            }
        }
        if pid.is_some() {
            alive_at = now;
        }
        let state = format!("{:?}:{:?}:{}", lock.as_ref().map(|l| (l.pid, l.port)), pid, log_size(&log));
        if state != moved {
            moved = state;
            moved_at = now;
        }
        if exits >= EXITS_FAILED || now - alive_at >= start.wait {
            // A host that exits quicker than the manager is asked is never seen running, and its lines are how it is known.
            let said = log_since(&log, mark).pop();
            if exits > 0 || said.is_some() {
                return Err(failed(stopped_line(said.as_deref())));
            }
            return Err(failed(format!("wsp did not start within {}: nothing ran its service", fmt_wait(now - from))));
        }
        if now - moved_at >= start.wait * STARTING_WAITS {
            return Err(failed(format!("wsp did not start within {}: it neither answered nor wrote to its log", fmt_wait(now - from))));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// How an open ended short of a host: a start that failed, or a refusal of a lock that stands.
enum Short {
    Failed(Failed),
    Refused(Box<Failure>),
}

/// `openHost` with no replace: the host the dial attaches to, else the unit loaded and waited on. A lock naming a
/// host whose program has gone is passed over for the service, which starts it again.
async fn open_host<R: Runner>(start: &Start, runner: &R) -> Result<HostClient, Short> {
    if !vanished(&start.state) {
        match HostClient::connect(&start.state, &start.env).await {
            Ok(client) => return Ok(client),
            Err(refused) if serving(&start.state).is_some() => return Err(Short::Refused(Box::new(refused))),
            Err(_) => {}
        }
    }
    ensure(start, runner).await.map_err(Short::Failed)?;
    until_attachable(start, runner).await.map_err(Short::Failed)
}

/// When the host's log last grew, kept across every wait of one open.
struct Moved {
    size: u64,
    at: Instant,
}

/// `untilAnswers`: waits on a host that holds the lock and has not answered, until something answers on its port,
/// its process lets the lock go or another takes it, or its log has not moved for the longer wait or the open's
/// `until` has passed (false). A port that starts an answer it never ends answers every probe and fails every dial.
async fn until_answers(start: &Start, lock: &Lock, moved: &mut Moved, until: Instant) -> bool {
    let log = start.log();
    loop {
        tokio::time::sleep(POLL).await;
        let Some(now) = serving(&start.state).filter(|now| now.pid == lock.pid && now.port == lock.port) else { return true };
        let size = log_size(&log);
        if size != moved.size {
            *moved = Moved { size, at: Instant::now() };
        }
        if moved.at.elapsed() >= start.wait * STARTING_WAITS || Instant::now() >= until {
            return false;
        }
        if answers(&now).await {
            return true;
        }
    }
}

/// `settles`: waits, until `until`, for the lock to be gone or to answer as this app's own host; a lock whose port
/// answers as anything else is a host that is up, and its refusal stands.
async fn settles(start: &Start, until: Instant) -> bool {
    loop {
        if Instant::now() >= until {
            return false;
        }
        let Some(now) = serving(&start.state) else { return true };
        if HostClient::connect(&start.state, &start.env).await.is_ok() {
            return true;
        }
        if answers(&now).await {
            return false;
        }
        tokio::time::sleep(POLL).await;
    }
}

/// `openHostReady`: the host the window opens on and what `first` reads of it (the first launch's question), read
/// across a restart. A launch that meets the host on its way down finds its lock and page up a moment, then reads
/// nothing, or finds the lock with nothing behind it; the host after it holds the lock a while before it answers. The
/// launch waits for the lock to settle and attaches again, within one longer wait; a host that answers keeps its
/// refusal, and a start that failed is the answer at once.
pub async fn ready<R, N, F, Fut>(start: &Start, runner: &R, first: F) -> Result<(Arc<HostClient>, N), Failed>
where
    R: Runner,
    F: Fn(Arc<HostClient>) -> Fut,
    Fut: Future<Output = Result<N, Failure>>,
{
    let from = Instant::now();
    let until = from + start.wait * STARTING_WAITS;
    let mut moved = Moved { size: log_size(&start.log()), at: from };
    let waited = |refused: Failure| -> Failed {
        let message = format!("after {} of waiting, {}", fmt_wait(from.elapsed()), refused.message);
        Failure { message, ..refused }.into()
    };
    let gave_up = |refused: Failure| if Instant::now() < until { refused.into() } else { waited(refused) };
    loop {
        let client = match open_host(start, runner).await {
            Ok(client) => Arc::new(client),
            Err(Short::Failed(failed)) => return Err(failed),
            Err(Short::Refused(refused)) => {
                let refused = *refused;
                let lock = serving(&start.state);
                if refused.kind.as_deref() == Some(wsp_mcp::NOT_ANSWERING) {
                    match lock {
                        Some(lock) if !until_answers(start, &lock, &mut moved, until).await => return Err(waited(refused)),
                        _ => continue,
                    }
                }
                // A lock whose port answers is a host that is up and refused this app; one with nothing behind it is a
                // host closing or still binding.
                match lock {
                    None => return Err(refused.into()),
                    Some(lock) if answers(&lock).await => return Err(refused.into()),
                    Some(_) => {}
                }
                if !settles(start, until).await {
                    return Err(gave_up(refused));
                }
                continue;
            }
        };
        let answered = serving(&start.state);
        let refused = match first(client.clone()).await {
            Ok(next) => return Ok((client, next)),
            Err(refused) => refused,
        };
        drop(client);
        // The host that answered and still serves refused the read, or went quiet after its page, which is waited on
        // as one that has not answered yet. Any other failure is a host closing or the one after it still binding.
        if let (Some(now), Some(answered)) = (serving(&start.state), answered) {
            if now.pid == answered.pid {
                if refused.kind.as_deref() == Some("unreachable") {
                    if until_answers(start, &now, &mut moved, until).await {
                        continue;
                    }
                    return Err(waited(refused));
                }
                if answers(&now).await {
                    return Err(refused.into());
                }
            }
        }
        if !settles(start, until).await {
            return Err(gave_up(refused));
        }
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            Manager::Launchd.unit(Path::new("/Users/a"), state),
            Path::new("/Users/a/Library/LaunchAgents/com.wsp.host.2f71d6c9.plist")
        );
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
        let fake =
            Fake::new(|argv| if argv[2] == "show" { ok("ActiveState=active\nUnitFileState=enabled\nMainPID=7") } else { ok("enabled") });
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

    #[test]
    fn the_stop_boots_out_or_stops_the_unit_and_keeps_its_file() {
        let f = fixture(Manager::Launchd, SERVICE_WAIT);
        let fake = Fake::new(|_| ok("\tstate = running\n"));
        block(stop(&f.start, &fake)).unwrap();
        let label = format!("com.wsp.host.{}", f.tag());
        assert_eq!(fake.ran(), [format!("launchctl print gui/501/{label}"), format!("launchctl bootout gui/501/{label}")]);
        assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), "the unit today's app wrote\n");

        let f = fixture(Manager::Systemd, SERVICE_WAIT);
        let fake = Fake::new(|_| ok("ActiveState=active\nUnitFileState=enabled\nMainPID=7"));
        block(stop(&f.start, &fake)).unwrap();
        let name = format!("wsp-host-{}.service", f.tag());
        assert_eq!(
            fake.ran(),
            [format!("systemctl --user show {name} --property=ActiveState,UnitFileState,MainPID"), format!("systemctl --user stop {name}")]
        );
        assert_eq!(std::fs::read_to_string(&f.unit).unwrap(), "the unit today's app wrote\n");

        let fake =
            Fake::new(
                |argv| if argv[2] == "stop" { Ran { code: 5, output: "Failed to stop".to_owned() } } else { ok("ActiveState=active") },
            );
        assert_eq!(block(stop(&f.start, &fake)).unwrap_err(), format!("systemctl --user stop {name} exited 5 and said: Failed to stop"));
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

    /// Catches: a systemd unit the stop left enabled read as held at the next launch, which then waited on a process
    /// nothing would start.
    #[test]
    fn a_systemd_unit_the_stop_left_enabled_is_loaded_again() {
        let f = fixture(Manager::Systemd, SERVICE_WAIT);
        let stopped = Arc::new(Mutex::new(false));
        let fake = Fake::new(move |argv| match argv[2].as_str() {
            "stop" => {
                *stopped.lock().unwrap() = true;
                ok("")
            }
            "show" if *stopped.lock().unwrap() => ok("ActiveState=inactive\nUnitFileState=enabled\nMainPID=0"),
            "show" => ok("ActiveState=active\nUnitFileState=enabled\nMainPID=7"),
            "is-enabled" => ok("enabled"),
            _ => ok(""),
        });
        block(stop(&f.start, &fake)).unwrap();
        block(ensure(&f.start, &fake)).unwrap();
        let name = format!("wsp-host-{}.service", f.tag());
        assert_eq!(
            fake.ran()[4..],
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
        assert_eq!(
            run_failure_line(&argv, &Ran { code: 1, output: " \n".to_owned() }),
            "launchctl enable gui/501/x exited 1 and said nothing"
        );
        assert_eq!(
            run_failure_line(&argv, &Ran { code: 1, output: "a\n  b".to_owned() }),
            "launchctl enable gui/501/x exited 1 and said: a b"
        );
    }

    #[test]
    fn says_a_wait_as_fmt_duration_does() {
        for (ms, said) in
            [(0, "1ms"), (400, "400ms"), (1_840, "1.8s"), (9_990, "10s"), (20_400, "20s"), (120_000, "2m"), (125_000, "2m 5s")]
        {
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
}
