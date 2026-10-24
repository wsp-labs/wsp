// SPDX-License-Identifier: AGPL-3.0-only
//! The host a launch opens on (`desktop:host-lifecycle.ts` `openHostReady`): the one the lock names, else the one
//! this computer's own service manager brings up from its unit, waited on until it answers. A unit that stands is
//! loaded as it stands and never removed here. Where this app carries its own host and owns the wsp command
//! (`Start::wsp`), a missing unit is written by that command's own `wsp up --service`, run in the wsp home, a service
//! host on files that went is restarted onto it before the open (`Start::restart`), and a move onto it (`follow`)
//! writes again a unit that runs another program. Quit and stop wsp stops the host and keeps the unit, login switch
//! and all, for the next launch and the next login; today's removes it, and with it the start at login.
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use wsp_mcp::lock::{alive as pid_alive, answers as probe, page_release, proved_token, serving, vanished, Lock};
use wsp_mcp::ReleaseRule;

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
const LOG_FILE: &str = "host.log";

/// The names a manager's command keeps from the app's environment: where the person's units sit, where the manager
/// is found, and for `systemctl --user` the login's bus.
const MANAGER_ENV: [&str; 5] = ["HOME", "PATH", "USER", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"];

/// The names the wsp command keeps from the app's environment: the manager's, the home it serves and the login
/// shell it reads its PATH from, the locale its lines are written in, and where it keeps temporary files.
const WSP_ENV: [&str; 11] = [
    "HOME",
    "PATH",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "TMPDIR",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "WSP_HOME",
    "WSP_STATE_STORE",
];
/// What a line of the wsp command gets: `wsp up --service` alone waits `SERVICE_WAIT_MS` for its host, after reading
/// the login shell, and a restart waits that long for the old host to go and again for the new one to answer.
const WSP_WAIT: Duration = Duration::from_secs(90);

/// What a start with no unit to load says, in a build that carries no host: today's app writes the unit the first
/// time it opens.
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

    /// How a person names the manager's units in a line (`ServiceManager.words`).
    pub fn words(self) -> &'static str {
        match self {
            Manager::Launchd => "launchd agent",
            Manager::Systemd => "systemd unit",
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

    /// Run in order to stop the host the unit runs, leaving the unit's file and its login switch as they are
    /// (`launchdUnload`, `systemdStop`): the next load or the next login starts it again.
    pub fn stop(self, start: &Start) -> Vec<Vec<String>> {
        match self {
            Manager::Launchd => vec![words(&["launchctl", "bootout", &self.target(start)])],
            Manager::Systemd => vec![words(&["systemctl", "--user", "stop", &self.name(&start.state)])],
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

    /// The words a unit's file runs (`ServiceManager.argv`): systemd's `ExecStart=` line as sh reads it, launchd's
    /// `ProgramArguments`; none where the file holds neither.
    pub fn argv(self, text: &str) -> Option<Vec<String>> {
        match self {
            Manager::Systemd => shell_words(text.lines().find_map(|line| line.strip_prefix("ExecStart="))?),
            Manager::Launchd => {
                let (_, rest) = text.split_once("<key>ProgramArguments</key>")?;
                let (array, _) = rest.split_once("</array>")?;
                let unescape = |word: &str| {
                    [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&apos;", "'"), ("&amp;", "&")]
                        .iter()
                        .fold(word.to_owned(), |word, (from, to)| word.replace(from, to))
                };
                Some(
                    array
                        .split("<string>")
                        .skip(1)
                        .filter_map(|part| part.split_once("</string>"))
                        .map(|(word, _)| unescape(word))
                        .collect(),
                )
            }
        }
    }

    /// Whether a unit's file runs `program` as its first word (`ServiceManager.runs`), read off its words as the
    /// manager reads them, so a path launchd's plist writes with `&amp;` is still the path.
    pub fn runs(self, text: &str, program: &Path) -> bool {
        self.argv(text).is_some_and(|argv| argv.first().is_some_and(|first| Path::new(first) == program))
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

/// A line's words as `/bin/sh` reads `shellQuote`'s output: single quoted runs, `\` escapes and bare words.
fn shell_words(line: &str) -> Option<Vec<String>> {
    let (mut words, mut word, mut open) = (Vec::new(), String::new(), false);
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                word.extend(chars.by_ref().take_while(|c| *c != '\''));
                open = true;
            }
            '\\' => {
                word.push(chars.next()?);
                open = true;
            }
            c if c.is_whitespace() => {
                if open {
                    words.push(std::mem::take(&mut word));
                    open = false;
                }
            }
            c => {
                word.push(c);
                open = true;
            }
        }
    }
    if open {
        words.push(word);
    }
    Some(words)
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

/// Runs a manager's command, as `ServiceDeps.run`, and a line of the wsp command; the real one is [`System`].
pub trait Runner: Sync {
    fn run(&self, argv: &[String]) -> impl Future<Output = Ran> + Send;

    /// A line of the wsp command this app installed, run in `cwd`, which may wait on a host for longer than a
    /// manager answers in.
    fn wsp(&self, argv: &[String], cwd: &Path) -> impl Future<Output = Ran> + Send {
        let _ = cwd;
        self.run(argv)
    }
}

/// `systemRunner`: the manager by its bare name off PATH, 15 s to answer, and 127 where it is not there or never
/// answered, with nothing of the app's environment but what a manager needs.
pub struct System {
    env: Vec<(String, String)>,
    wsp_env: Vec<(String, String)>,
}

impl System {
    pub fn new(env: &Env) -> System {
        let kept = |names: &[&str]| names.iter().filter_map(|name| Some(((*name).to_owned(), env.get(*name)?.clone()))).collect();
        System { env: kept(&MANAGER_ENV), wsp_env: kept(&WSP_ENV) }
    }
}

/// Runs `argv` on nothing of the app's environment but `env`, for at most `wait`, in `cwd` where one is named.
async fn run_for(argv: &[String], env: &[(String, String)], wait: Duration, cwd: Option<&Path>) -> Ran {
    let Some((program, args)) = argv.split_first() else { return Ran { code: 127, output: String::new() } };
    let mut command = tokio::process::Command::new(program);
    command.args(args).env_clear().envs(env.iter().cloned()).stdin(Stdio::null()).kill_on_drop(true);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    match tokio::time::timeout(wait, command.output()).await {
        Ok(Ok(out)) => {
            let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)).trim().to_owned();
            Ran { code: if out.status.success() { 0 } else { out.status.code().unwrap_or(127) }, output: said }
        }
        Ok(Err(e)) => Ran { code: 127, output: format!("{program}: {e}") },
        Err(_) => Ran { code: 127, output: format!("{program} did not answer within {}", fmt_wait(wait)) },
    }
}

impl Runner for System {
    async fn run(&self, argv: &[String]) -> Ran {
        run_for(argv, &self.env, MANAGER_WAIT, None).await
    }

    async fn wsp(&self, argv: &[String], cwd: &Path) -> Ran {
        run_for(argv, &self.wsp_env, WSP_WAIT, Some(cwd)).await
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
    /// The wsp command this app installed and owns, which runs the host this app carries: where there is one, a unit
    /// that is missing is written by its `wsp up --service`, so the unit runs it as today's app's runs its shim.
    pub wsp: Option<PathBuf>,
    /// Whether the host serving is to be restarted onto that command before the open: it is of the bundle's release
    /// and runs files the command named before and that went (`staleShim`).
    pub restart: bool,
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
            wsp: None,
            restart: false,
            wait: SERVICE_WAIT,
            say: Arc::new(|line| eprintln!("desktop-gpui: {line}")),
        }
    }

    fn log(&self) -> PathBuf {
        self.state.with_file_name(LOG_FILE)
    }

    /// The wsp home, the state file's folder: the wsp command runs there, as today's app runs its service
    /// (`servicePlan`'s cwd), so the unit `wsp up --service` writes reads `<home>/.env` and never a `.env` in `$HOME`.
    fn wsp_home(&self) -> &Path {
        self.state.parent().unwrap_or(Path::new("/"))
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

/// The last thing a line of the wsp command said, which is its refusal; else the line, its code and its silence.
fn wsp_failure(argv: &[String], ran: &Ran) -> String {
    ran.output.lines().map(str::trim).rfind(|said| !said.is_empty()).map(str::to_owned).unwrap_or_else(|| run_failure_line(argv, ran))
}

/// `ensureService` with a unit that is never rewritten: a unit that stands, loaded as it stands where the manager has
/// let it go, holds it with nothing running it and nothing serving, or runs a host whose program has gone. A unit set
/// not to start at login is loaded all the same and set back off once loaded. A missing one is written and loaded by
/// the wsp command this app owns, through the host's own `wsp up --service`, which answers once its host serves.
pub async fn ensure<R: Runner>(start: &Start, runner: &R) -> Result<(), Failed> {
    let Some(manager) = start.manager else { return Err(line(no_manager_line(std::env::consts::OS))) };
    if !manager.unit(&start.home, &start.state).exists() {
        let Some(wsp) = &start.wsp else { return Err(line(NOT_SET_UP)) };
        let argv =
            [wsp.to_string_lossy().as_ref(), "up", "--service", "--state", start.state.to_string_lossy().as_ref()].map(str::to_owned);
        let ran = runner.wsp(&argv, start.wsp_home()).await;
        if ran.code != 0 {
            for said in ran.output.lines().filter(|said| !said.trim().is_empty()) {
                (start.say)(&format!("wsp up --service: {said}"));
            }
            // A unit it wrote whose host is slower than its wait is waited on as any other start is.
            if !manager.unit(&start.home, &start.state).exists() {
                return Err(Failed { failure: Box::new(Failure::new(wsp_failure(&argv, &ran))), log: Some(start.log()) });
            }
        }
        (start.say)(&format!("{} wrote the {} {}", wsp.display(), manager.words(), manager.name(&start.state)));
        return Ok(());
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

/// `untilLock` for the lock to go, or where `pid` is named for it to name another: the host still serving once
/// `wait` has passed, else none.
async fn until_gone(state: &Path, wait: Duration, pid: Option<i32>) -> Option<Lock> {
    let until = Instant::now() + wait;
    loop {
        let lock = serving(state).filter(|lock| pid.is_none_or(|pid| lock.pid == pid))?;
        if Instant::now() >= until {
            return Some(lock);
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Quit and stop wsp's stop (`stopWsp` less the unit's removal): the manager stops the host it runs and keeps the unit
/// today's app wrote as it stands, and a host a line started is asked to end by the pid its lock records, as `wsp down`
/// does. The answer is the refusal, or the host still serving once the start's wait has
/// passed.
pub async fn stop<R: Runner>(start: &Start, runner: &R) -> Result<(), String> {
    let lock = serving(&start.state);
    let mut asked = false;
    if let Some(manager) = start.manager {
        if manager.holding(&runner.run(&manager.holds(start)).await) {
            if let Some((argv, ran)) = run_all(&manager.stop(start), runner).await {
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
    let left = until_gone(&start.state, if asked { start.wait } else { Duration::ZERO }, None).await;
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

/// `restartService` through the wsp command this app owns: the host serving the state file is asked to restart on
/// the road it came up on (`wsp restart`), which for the service is its manager starting the unit's line again, now
/// on the command as it stands. Nothing serving is nothing to restart, and the next open starts the unit. The answer
/// is the refusal, in the command's own words.
pub async fn restart<R: Runner>(start: &Start, runner: &R) -> Result<(), String> {
    let Some(wsp) = &start.wsp else { return Err("this app carries no wsp of its own to restart the host with".to_owned()) };
    let Some(lock) = serving(&start.state) else { return Ok(()) };
    let argv = [wsp.to_string_lossy().as_ref(), "restart", "--state", start.state.to_string_lossy().as_ref()].map(str::to_owned);
    (start.say)(&format!("wsp (pid {}) serves {}; restarting it on {}", lock.pid, start.state.display(), wsp.display()));
    let ran = runner.wsp(&argv, start.wsp_home()).await;
    if ran.code != 0 {
        return Err(wsp_failure(&argv, &ran));
    }
    Ok(())
}

/// Whether the launch asked for the host to be restarted and the service's host serves while its unit runs the wsp
/// command, so a restart brings it back on the command as it stands; a unit running another wsp, npm's, is left.
fn restarts_onto_wsp(start: &Start) -> bool {
    let (Some(wsp), Some(manager)) = (&start.wsp, start.manager) else { return false };
    let started_by_service = serving(&start.state).is_some_and(|lock| lock.started_by.as_deref() == Some("service"));
    let unit = std::fs::read_to_string(manager.unit(&start.home, &start.state)).unwrap_or_default();
    start.restart && started_by_service && manager.runs(&unit, wsp)
}

/// `restart` where the launch asked for one and the service's host runs the wsp command; nothing otherwise.
pub async fn restart_service<R: Runner>(start: &Start, runner: &R) -> Result<(), String> {
    if !restarts_onto_wsp(start) {
        return Ok(());
    }
    restart(start, runner).await
}

/// What a restart onto the wsp command came to, for whoever asked for it: only `Restarted`, `Already` and
/// `NotServing` leave the host serving on this bundle's files, or about to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Followed {
    /// The host serving was restarted onto the wsp command, by its own restart or stopped and started by the service.
    Restarted,
    /// It already serves this bundle's release on the files the wsp command names.
    Already,
    /// Nothing served: the next start runs the wsp command, or a unit that ran another program was written again to
    /// run it, which started its host.
    NotServing,
    /// A host serves and was left as it is, for the reason given in words a person reads.
    Left(String),
}

/// `ensureService`'s rewrite of a unit that runs another program, npm's wsp: the host it runs is stopped and the unit
/// written again by the command's `wsp up --service`, with the words it was given kept, which loads it and waits on
/// its host. The load sets it to start at login, so a unit that was set not to is set back.
async fn rewrite<R: Runner>(start: &Start, runner: &R, manager: Manager, wsp: &Path, unit: &str) -> Result<(), String> {
    let state = start.state.display();
    let given = manager.argv(unit).and_then(|argv| Some(argv[argv.iter().position(|word| word == "up")? + 1..].to_vec()));
    let given = given.unwrap_or_else(|| vec!["--state".to_owned(), start.state.to_string_lossy().into_owned()]);
    let off_at_login = manager.starts_at_login(&runner.run(&manager.login_read(start)).await, &start.state) == Some(false);
    (start.say)(&format!(
        "the {} {} runs another wsp; it is stopped and written again to run {}",
        manager.words(),
        manager.name(&start.state),
        wsp.display()
    ));
    // A host serving is stopped whatever the manager said of the unit, so a manager that could not answer refuses here.
    if serving(&start.state).is_some() || manager.holding(&runner.run(&manager.holds(start)).await) {
        if let Some((argv, ran)) = run_all(&manager.stop(start), runner).await {
            return Err(run_failure_line(&argv, &ran));
        }
    }
    if let Some(left) = until_gone(&start.state, start.wait, None).await {
        return Err(format!("wsp (pid {}) was stopped and still serves {state} after {}", left.pid, fmt_wait(start.wait)));
    }
    let mut argv = vec![wsp.to_string_lossy().into_owned(), "up".to_owned(), "--service".to_owned()];
    argv.extend(given);
    let ran = runner.wsp(&argv, start.wsp_home()).await;
    if off_at_login && manager.unit(&start.home, &start.state).exists() {
        if let Some((argv, ran)) = run_all(&[manager.off_at_login(start)], runner).await {
            (start.say)(&run_failure_line(&argv, &ran));
        }
    }
    if ran.code != 0 {
        // A unit it wrote whose host is slower than its wait is waited on as any other start is.
        let written = std::fs::read_to_string(manager.unit(&start.home, &start.state)).is_ok_and(|unit| manager.runs(&unit, wsp));
        if !written {
            return Err(wsp_failure(&argv, &ran));
        }
        for said in ran.output.lines().filter(|said| !said.trim().is_empty()) {
            (start.say)(&format!("wsp up --service: {said}"));
        }
    }
    Ok(())
}

/// The unit, where there is one and it runs another program than the wsp command, written again to run it.
async fn unit_onto<R: Runner>(start: &Start, runner: &R, manager: Manager, wsp: &Path) -> Result<bool, String> {
    match std::fs::read_to_string(manager.unit(&start.home, &start.state)) {
        Ok(unit) if !manager.runs(&unit, wsp) => rewrite(start, runner, manager, wsp, &unit).await.map(|()| true),
        _ => Ok(false),
    }
}

/// Where nothing serves: the unit written again to run the wsp command where it runs another program, so the start
/// that follows runs the command.
pub async fn idle_onto<R: Runner>(start: &Start, runner: &R) -> Result<Followed, String> {
    if let (Some(wsp), Some(manager)) = (&start.wsp, start.manager) {
        unit_onto(start, runner, manager, wsp).await?;
    }
    Ok(Followed::NotServing)
}

/// `lockedHost`'s proof the lock is this home's host before anything reaches its pid: the pid is this login's, and
/// the page on its port carries the digest of the token beside the state. A pid a dead host left, reused by another
/// login or beside another home's host on that port, is refused in words.
async fn own_host(start: &Start, lock: &Lock) -> Result<(), String> {
    proved_token(&start.state, lock, PROBE, ReleaseRule::OwnOrNewer).await.map(drop).map_err(|refused| refused.message)
}

/// `replaceHost` onto the wsp command, by the road the lock names, once the lock is proved this home's host: a
/// service whose unit runs the command restarts through its own `wsp restart`; one whose unit runs another program,
/// npm's wsp, is stopped and its unit written again by the command's `wsp up --service` with the words it was given
/// kept; a host a terminal's `wsp up` or a verb started is stopped by its pid and the service started after it, from
/// a unit written again in the same way where it runs another program. A host no road of wsp's started, or one not
/// proved this home's, is left, said in words. The refusal is in the command's or the manager's own words.
pub async fn follow<R: Runner>(start: &Start, runner: &R) -> Result<Followed, String> {
    let Some(wsp) = &start.wsp else { return Err("this app carries no wsp of its own to restart the host with".to_owned()) };
    let Some(lock) = serving(&start.state) else { return idle_onto(start, runner).await };
    let state = start.state.display();
    let Some(by) = lock.started_by.as_deref() else {
        return Ok(Followed::Left(format!(
            "wsp (pid {}) serves {state} from a process wsp did not start: stop it, then open wsp again",
            lock.pid
        )));
    };
    if let Err(refused) = own_host(start, &lock).await {
        return Ok(Followed::Left(refused));
    }
    let manager = start.manager.ok_or_else(|| no_manager_line(std::env::consts::OS))?;
    if by == "service" {
        let unit = std::fs::read_to_string(manager.unit(&start.home, &start.state)).unwrap_or_default();
        if manager.runs(&unit, wsp) {
            restart(start, runner).await?;
        } else {
            rewrite(start, runner, manager, wsp, &unit).await?;
        }
        return Ok(Followed::Restarted);
    }
    (start.say)(&format!(
        "wsp (pid {}, started by {by}) serves {state}; it is stopped and the service started on {}",
        lock.pid,
        wsp.display()
    ));
    if serving(&start.state).is_some_and(|now| now.pid == lock.pid) {
        // SAFETY: a signal to the pid the host wrote in its own lock, proved this home's host and read again just before.
        unsafe { libc::kill(lock.pid, libc::SIGTERM) };
    }
    // A unit with nothing to stop it, npm's among them, may take the lock as soon as this pid lets it go.
    if let Some(left) = until_gone(&start.state, start.wait, Some(lock.pid)).await {
        return Err(format!("wsp (pid {}) was stopped and still serves {state} after {}", left.pid, fmt_wait(start.wait)));
    }
    if !unit_onto(start, runner, manager, wsp).await? {
        ensure(start, runner).await.map_err(|failed| failed.failure.message)?;
    }
    Ok(Followed::Restarted)
}

/// The release the host serving `state` names on its page (`lockedHost`); none where nothing serves or answers.
pub async fn serving_release(state: &Path) -> Option<String> {
    page_release(&serving(state)?, PROBE).await
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
        if probe(&now, PROBE).await {
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
        if probe(&now, PROBE).await {
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
    if let Err(refused) = restart_service(start, runner).await {
        (start.say)(&format!("the host was not restarted onto the wsp command: {refused}"));
    }
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
                    Some(lock) if probe(&lock, PROBE).await => return Err(refused.into()),
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
                if probe(&now, PROBE).await {
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
mod tests;
