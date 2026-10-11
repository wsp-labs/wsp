// SPDX-License-Identifier: AGPL-3.0-only
//! proc.kill: what a kill takes, what it never touches, and TERM then KILL. The targets are the pid alone, the members
//! of its process group (a one-process tool: a group TERM took 6 of 16 of a thread's processes on a Mac, so no
//! thread's end goes by one), or the pid and everything under it, each read once with what makes it that process and
//! not another, and signalled through it: a Mac's audit token, whose pid version the kernel matches at the signal, or a
//! Linux pidfd, whose start is checked once it is open. A pid that holds another process by then is never signalled.
//! Before any signal the pid must still be the process the row was read from, by its identity and its start time, and
//! no target may be init, a kernel thread, the daemon, the daemon's parent, a pid the host names as its own or the
//! app's, or a thread's agent: the turn's root, the agent its run recorded and every process between, since a signal
//! there reads as a failed turn and an agent is ended by stopping its thread. On a box whose daemon is root, a target
//! must also stand in a thread's cgroup or under the daemon, so a page never ends the box's own services. TERM goes
//! first; whatever is still there after KILL_GRACE_MS gets KILL, and the reply names what stayed.

use std::collections::HashSet;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::Signal;
use wsp_frames::{numbers, DaemonErrorCode, KillMode, ProcKillReply, ProcSignal};

use crate::paths::OpError;
use crate::proc::no_process;

/// How often the targets are read again while TERM's grace runs, and after KILL.
const POLL_MS: u64 = 50;
/// How long the targets are given to go after KILL before the reply names them as left.
const AFTER_KILL_MS: u64 = 500;

/// One process as a kill reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Seen {
    pub(crate) pid: u32,
    pub(crate) ppid: u32,
    pub(crate) pgid: u32,
    pub(crate) started_at: i64,
    pub(crate) zombie: bool,
    pub(crate) comm: String,
    /// What says the pid still holds this process: its unique id on a Mac, its start in clock ticks on Linux.
    pub(crate) id: u64,
    /// The pid version a Mac's audit token carries; nothing on Linux.
    pub(crate) version: i32,
}

impl Seen {
    fn same(&self, other: &Seen) -> bool {
        self.pid == other.pid && self.id == other.id
    }
}

/// What no kill signals: init, the daemon and its parent, and the host's lists. An app's main process takes every
/// process under it along, Electron's helpers and tabs, short of anything under the daemon or its parent.
#[derive(Debug, Clone, Default)]
pub(crate) struct Guard {
    pub(crate) this: u32,
    pub(crate) parent: u32,
    pub(crate) protect: HashSet<u32>,
    pub(crate) apps: HashSet<u32>,
    /// Each running turn's root, the process the host launched it as, and the agent its run recorded.
    pub(crate) agents: HashSet<u32>,
    /// The names an agent's binary is run by, for a turn whose run recorded no agent.
    pub(crate) agent_bins: HashSet<String>,
    /// A root daemon on a box: what it may end stands in a thread's cgroup or under the daemon.
    pub(crate) boxed: bool,
}

/// One proc.kill frame past its pid: what it asks, and the host's lists.
#[derive(Debug, Clone)]
pub(crate) struct Frame {
    pub(crate) signal: ProcSignal,
    pub(crate) mode: Option<KillMode>,
    pub(crate) started_at: Option<i64>,
    pub(crate) identity: Option<String>,
    pub(crate) protect: Option<Vec<u32>>,
    pub(crate) apps: Option<Vec<u32>>,
    pub(crate) agents: Option<Vec<u32>>,
    pub(crate) agent_bins: Option<Vec<String>>,
}

impl Guard {
    pub(crate) fn here(frame: Frame, boxed: bool) -> Guard {
        let set = |l: Option<Vec<u32>>| l.unwrap_or_default().into_iter().collect();
        Guard {
            this: std::process::id(),
            parent: std::os::unix::process::parent_id(),
            protect: set(frame.protect),
            apps: set(frame.apps),
            agents: set(frame.agents),
            agent_bins: frame.agent_bins.unwrap_or_default().into_iter().collect(),
            boxed,
        }
    }

    /// Every pid the host listed, and every process on the way from one down to the agent under it: another pid it
    /// listed, the agent the turn's run recorded, or where none stands under it each process run as an agent's binary
    /// by its accounting name or argv[0]'s basename, since a native Claude Code runs as its version and cursor-agent
    /// as node. The wrapper, its shell and the agent itself; a tool server or a dev server under the agent is none.
    fn agent_chains(&self, t: &dyn Table, table: &[Seen]) -> HashSet<u32> {
        let mut chains = self.agents.clone();
        let named = |s: &Seen| self.runs_as_agent(t, s);
        let listed: Vec<(&Seen, Vec<Seen>)> =
            table.iter().filter(|s| self.agents.contains(&s.pid)).map(|s| (s, with_descendants(table, vec![s.clone()]))).collect();
        // A recorded agent is listed beside its turn's root and stands under it; a root is a listed pid under no other.
        let inner: HashSet<u32> = listed.iter().flat_map(|(_, under)| under.iter().skip(1).map(|s| s.pid)).collect();
        for (root, under) in listed.iter().filter(|(s, _)| !inner.contains(&s.pid)) {
            let recorded: Vec<&Seen> = under.iter().skip(1).filter(|s| self.agents.contains(&s.pid)).collect();
            let mut agents: Vec<Seen> = if recorded.is_empty() {
                under.iter().skip(1).filter(|s| named(s)).cloned().collect()
            } else {
                recorded.iter().map(|s| (*s).clone()).collect()
            };
            // A recorded process that runs as no agent's name is a launcher, npm's codex.js under node: the agent's
            // own binary it starts, and what stands between them, are the agent too.
            for launcher in recorded.iter().filter(|s| !named(s)) {
                agents.extend(with_descendants(table, vec![(*launcher).clone()]).into_iter().skip(1).filter(|s| named(s)));
            }
            for agent in &agents {
                let mut at = Some(agent);
                while let Some(s) = at.filter(|s| s.pid != root.pid) {
                    chains.insert(s.pid);
                    at = table.iter().find(|p| p.pid == s.ppid && p.pid != s.pid);
                }
            }
        }
        chains
    }

    /// Whether the process runs as an agent's binary, by its accounting name or argv[0]'s basename.
    fn runs_as_agent(&self, t: &dyn Table, s: &Seen) -> bool {
        self.agent_bins.contains(&s.comm)
            || t.arg0(s.pid).is_some_and(|arg0| self.agent_bins.contains(arg0.rsplit('/').next().unwrap_or(&arg0)))
    }

    /// Whether a process in a thread's cgroup is that thread's agent or stands above it there: a box turn's run script
    /// and its shell, whose run records no agent and whose root no host lists.
    fn holds_agent(&self, t: &dyn Table, table: &[Seen], s: &Seen) -> bool {
        let Some(cgroup) = t.cgroup(s.pid).filter(|c| c.starts_with(&format!("{THREAD_CGROUPS}/"))) else { return false };
        with_descendants(table, vec![s.clone()])
            .iter()
            .any(|d| t.cgroup(d.pid).as_deref() == Some(cgroup.as_str()) && self.runs_as_agent(t, d))
    }

    /// Whether an app's main process stands at or above the pid before the daemon, its parent or init does.
    fn under_app(&self, table: &[Seen], pid: u32) -> bool {
        let mut at = pid;
        for _ in 0..table.len().max(1) {
            if self.apps.contains(&at) {
                return true;
            }
            if at <= 1 || at == self.this || at == self.parent {
                return false;
            }
            match table.iter().find(|s| s.pid == at) {
                Some(s) if s.ppid != at => at = s.ppid,
                _ => return false,
            }
        }
        false
    }

    /// Whether a box's root daemon may end the process: one in a thread's cgroup, or one under the daemon itself.
    fn inside(&self, t: &dyn Table, table: &[Seen], s: &Seen) -> bool {
        if t.cgroup(s.pid).is_some_and(|c| c.starts_with(&format!("{THREAD_CGROUPS}/"))) {
            return true;
        }
        let mut at = s.ppid;
        for _ in 0..table.len() {
            if at == self.this {
                return true;
            }
            match table.iter().find(|p| p.pid == at) {
                Some(p) if p.ppid != at && at > 1 => at = p.ppid,
                _ => return false,
            }
        }
        false
    }

    /// The first refusal among the targets, out of one reading of the table.
    fn check(&self, t: &dyn Table, table: &[Seen], ask: Ask, targets: &[Seen]) -> Result<(), OpError> {
        let agents = if self.agents.is_empty() { HashSet::new() } else { self.agent_chains(t, table) };
        for s in targets {
            if let Some(e) = self.refusal(table, &agents, ask.pid, ask.mode, s.pid) {
                return Err(e);
            }
            if cfg!(target_os = "linux") && s.ppid == KTHREADD {
                return Err(forbidden(ask, s.pid, "it is part of wsp or the system"));
            }
            if !self.agent_bins.is_empty() && self.holds_agent(t, table, s) {
                return Err(forbidden(ask, s.pid, AGENT_WHY));
            }
            if self.boxed && !self.inside(t, table, s) {
                return Err(forbidden(ask, s.pid, "it is outside every thread wsp runs on this computer"));
            }
        }
        Ok(())
    }

    fn refusal(&self, table: &[Seen], agents: &HashSet<u32>, asked: u32, mode: KillMode, target: u32) -> Option<OpError> {
        let forbidden = |why: &str| Some(refused(asked, mode, target, why));
        if target == 1
            || (cfg!(target_os = "linux") && target == KTHREADD)
            || target == self.this
            || target == self.parent
            || self.protect.contains(&target)
            || self.under_app(table, target)
        {
            return forbidden("it is part of wsp or the system");
        }
        if agents.contains(&target) {
            return forbidden(AGENT_WHY);
        }
        None
    }
}

/// Why an agent, and what stands between its turn's root and it, is never signalled.
const AGENT_WHY: &str = "it is a thread's agent; stop the thread instead";

/// The parent of every Linux kernel thread.
const KTHREADD: u32 = 2;

/// Where a box's turns stand, one cgroup per thread (`packages/protocol/src/thread-cgroup.ts`).
const THREAD_CGROUPS: &str = "/wsp-threads";

fn refused(asked: u32, mode: KillMode, target: u32, why: &str) -> OpError {
    let whose =
        if target == asked { format!("pid {asked}") } else { format!("pid {target}, which {} takes with pid {asked}", mode_word(mode)) };
    OpError::coded(DaemonErrorCode::Forbidden, format!("refusing to signal {whose}: {why}"))
}

fn not_listed(pid: u32) -> OpError {
    OpError::coded(DaemonErrorCode::NotFound, format!("pid {pid} is no longer the process that was listed"))
}

fn forbidden(ask: Ask, target: u32, why: &str) -> OpError {
    refused(ask.pid, ask.mode, target, why)
}

fn mode_word(mode: KillMode) -> &'static str {
    match mode {
        KillMode::Pid => "the kill",
        KillMode::Group => "its process group",
        KillMode::Tree => "the kill of everything under it",
    }
}

fn alive(s: &Seen) -> bool {
    !s.zombie
}

/// The live processes a kill of `root` in this mode takes, out of one reading of the table.
fn targets(table: &[Seen], root: &Seen, mode: KillMode) -> Vec<Seen> {
    match mode {
        KillMode::Pid => vec![root.clone()],
        KillMode::Group => table.iter().filter(|s| s.pgid == root.pgid && alive(s)).cloned().collect(),
        KillMode::Tree => with_descendants(table, vec![root.clone()]),
    }
}

/// The processes given and every live process under any of them.
fn with_descendants(table: &[Seen], mut taken: Vec<Seen>) -> Vec<Seen> {
    let mut i = 0;
    while i < taken.len() {
        let parent = taken[i].pid;
        let kids: Vec<Seen> = table
            .iter()
            .filter(|s| s.ppid == parent && s.pid != parent && alive(s) && !taken.iter().any(|t| t.pid == s.pid))
            .cloned()
            .collect();
        taken.extend(kids);
        i += 1;
    }
    taken
}

fn pids(seen: &[Seen]) -> Vec<u32> {
    seen.iter().map(|s| s.pid).collect()
}

/// Reads one process, and every process, and signals one, for the platform the daemon runs on.
pub(crate) trait Table: Send + Sync {
    fn one(&self, pid: u32) -> Option<Seen>;
    fn all(&self) -> Vec<Seen>;
    /// Signals the process `seen` was read as and never another: ESRCH where its pid holds another process or none.
    fn signal(&self, seen: &Seen, signal: Signal) -> nix::Result<()>;
    /// The name a process was run by, argv[0], read for the few processes under a turn's root.
    fn arg0(&self, pid: u32) -> Option<String>;
    /// The cgroup v2 path a process stands in, Linux only.
    fn cgroup(&self, pid: u32) -> Option<String>;
}

/// /proc, read as the processes pane reads it; the start time off the same boot clock the pane's rows carry.
#[cfg(target_os = "linux")]
pub(crate) struct ProcTable {
    btime: i64,
}

#[cfg(target_os = "linux")]
impl ProcTable {
    pub(crate) fn new() -> Option<ProcTable> {
        let stat = std::fs::read_to_string("/proc/stat").ok()?;
        Some(ProcTable { btime: crate::proc::parse_btime(&stat).ok()? })
    }
}

#[cfg(target_os = "linux")]
impl Table for ProcTable {
    fn one(&self, pid: u32) -> Option<Seen> {
        let st = crate::proc::parse_proc_pid_stat(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?).ok()?;
        Some(Seen {
            pid: st.pid,
            ppid: st.ppid,
            pgid: st.pgrp,
            started_at: crate::proc::started_ms(self.btime, st.starttime),
            zombie: st.state == "Z" || st.state == "X",
            comm: st.comm,
            id: st.starttime,
            version: 0,
        })
    }

    fn all(&self) -> Vec<Seen> {
        let Ok(dirs) = std::fs::read_dir("/proc") else { return Vec::new() };
        dirs.flatten().filter_map(|d| d.file_name().to_str()?.parse::<u32>().ok()).filter_map(|pid| self.one(pid)).collect()
    }

    fn signal(&self, seen: &Seen, signal: Signal) -> nix::Result<()> {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        let still = || self.one(seen.pid).is_some_and(|now| now.same(seen));
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, seen.pid as libc::pid_t, 0) };
        if fd < 0 {
            return match Errno::last() {
                // A kernel before 5.3 has no pidfd: the start is checked and the pid signalled by number.
                Errno::ENOSYS if still() => nix::sys::signal::kill(nix::unistd::Pid::from_raw(seen.pid as i32), signal),
                Errno::ENOSYS => Err(Errno::ESRCH),
                e => Err(e),
            };
        }
        // The pidfd holds whichever process had the pid at the open, so a start read after it names that process.
        let fd = unsafe { OwnedFd::from_raw_fd(fd as std::os::fd::RawFd) };
        if !still() {
            return Err(Errno::ESRCH);
        }
        let sent = unsafe {
            libc::syscall(libc::SYS_pidfd_send_signal, fd.as_raw_fd(), signal as libc::c_int, std::ptr::null::<libc::siginfo_t>(), 0)
        };
        if sent < 0 {
            Err(Errno::last())
        } else {
            Ok(())
        }
    }

    fn arg0(&self, pid: u32) -> Option<String> {
        let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        let first = cmdline.split(|b| *b == 0).next().filter(|a| !a.is_empty())?;
        Some(String::from_utf8_lossy(first).into_owned())
    }

    fn cgroup(&self, pid: u32) -> Option<String> {
        crate::proc::read_cgroup(std::path::Path::new(&format!("/proc/{pid}")))
    }
}

/// The kernel through libproc: the person's own processes, which are the only ones this daemon may signal.
#[cfg(target_os = "macos")]
pub(crate) struct ProcTable;

#[cfg(target_os = "macos")]
impl ProcTable {
    pub(crate) fn new() -> Option<ProcTable> {
        Some(ProcTable)
    }
}

#[cfg(target_os = "macos")]
impl Table for ProcTable {
    fn one(&self, pid: u32) -> Option<Seen> {
        let id = crate::libproc::identity(pid)?;
        let unique = crate::libproc::uniq(pid)?;
        Some(Seen {
            pid: id.pid,
            ppid: id.ppid,
            pgid: id.pgid,
            started_at: id.started_ms,
            zombie: id.state == 'Z',
            comm: id.comm,
            id: unique.unique,
            version: unique.version,
        })
    }

    fn all(&self) -> Vec<Seen> {
        crate::libproc::pids().into_iter().filter_map(|pid| self.one(pid)).collect()
    }

    fn signal(&self, seen: &Seen, signal: Signal) -> nix::Result<()> {
        match crate::libproc::signal_exact(seen.pid, seen.version, signal as libc::c_int) {
            // An exec since the reading moved the pid version and kept the unique id: the same process, signalled again.
            Err(Errno::ESRCH) => match crate::libproc::uniq(seen.pid) {
                Some(now) if now.unique == seen.id && now.version != seen.version => {
                    crate::libproc::signal_exact(seen.pid, now.version, signal as libc::c_int)
                }
                _ => Err(Errno::ESRCH),
            },
            sent => sent,
        }
    }

    fn arg0(&self, pid: u32) -> Option<String> {
        crate::libproc::arg0(pid)
    }

    fn cgroup(&self, _pid: u32) -> Option<String> {
        None
    }
}

/// The table for this daemon's platform, or the refusal a platform with none answers.
pub(crate) fn table() -> Result<Box<dyn Table>, OpError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Some(t) = ProcTable::new() {
        return Ok(Box::new(t));
    }
    Err(OpError::coded(DaemonErrorCode::Unsupported, "this computer's processes cannot be read to signal one".to_owned()))
}

/// What one proc.kill asks.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ask {
    pub(crate) pid: u32,
    pub(crate) signal: ProcSignal,
    pub(crate) mode: KillMode,
    pub(crate) started_at: Option<i64>,
    /// The row's id, which the pid must still hold: its unique id on a Mac, its start ticks on Linux.
    pub(crate) identity: Option<u64>,
    /// How far the row's start time may sit from the one read here: none where both read the kernel's own clock.
    pub(crate) start_slack_ms: i64,
}

/// Checks, signals and waits, as the module's head says. The grace is a parameter so a test can name it.
pub(crate) async fn kill_processes(t: &dyn Table, ask: Ask, guard: &Guard, grace: Duration) -> Result<ProcKillReply, OpError> {
    // Before the read, which on a Mac answers nothing for launchd or another account's process.
    if let Some(e) = guard.refusal(&[], &guard.agents, ask.pid, ask.mode, ask.pid) {
        return Err(e);
    }
    let root = t.one(ask.pid).filter(alive).ok_or_else(|| no_process(ask.pid))?;
    // An identity pins the process alone; a start time read off a boot clock the reader cached can drift after a step.
    if let (Some(at), None) = (ask.started_at, ask.identity) {
        if (root.started_at - at).abs() > ask.start_slack_ms {
            return Err(not_listed(ask.pid));
        }
    }
    if ask.identity.is_some_and(|id| id != root.id) {
        return Err(not_listed(ask.pid));
    }
    let alone =
        ask.mode == KillMode::Pid && guard.apps.is_empty() && guard.agents.is_empty() && guard.agent_bins.is_empty() && !guard.boxed;
    let table = if alone { vec![root.clone()] } else { t.all() };
    let taken = targets(&table, &root, ask.mode);
    // A group or a tree holding an agent is refused whole, the agent named, even where the agent is not the pid asked for.
    guard.check(t, &table, ask, &taken)?;
    let first = match ask.signal {
        ProcSignal::Term => Signal::SIGTERM,
        ProcSignal::Kill => Signal::SIGKILL,
    };
    send(t, ask, &taken, first)?;
    let mut forced = Vec::new();
    let mut here = still_here(t, &taken, &root, ask.mode);
    if first == Signal::SIGTERM && !here.is_empty() {
        here = wait_for(t, &taken, &root, ask.mode, grace).await;
        if !here.is_empty() {
            // A process that joined the group or the tree during the grace is taken too, behind the same guard.
            guard.check(t, &t.all(), ask, &here)?;
            send(t, ask, &here, Signal::SIGKILL)?;
            forced = pids(&here);
        }
    }
    let left = if here.is_empty() { here } else { wait_for(t, &taken, &root, ask.mode, Duration::from_millis(AFTER_KILL_MS)).await };
    Ok(ProcKillReply { signalled: pids(&taken), forced, left: pids(&left) })
}

/// Each target through the identity it was read with.
fn send(t: &dyn Table, ask: Ask, targets: &[Seen], signal: Signal) -> Result<(), OpError> {
    let mut outcome = Ok(());
    for s in targets {
        let sent = signal_result(s.pid, t.signal(s, signal));
        // One that went on its own between the reading and the signal is the outcome wanted; the asked pid's refusal is the answer.
        if s.pid == ask.pid && !matches!(&sent, Err(e) if e.code == Some(DaemonErrorCode::NotFound)) {
            outcome = sent;
        }
    }
    outcome
}

fn signal_result(pid: u32, sent: nix::Result<()>) -> Result<(), OpError> {
    match sent {
        Ok(()) => Ok(()),
        Err(Errno::ESRCH) => Err(no_process(pid)),
        Err(Errno::EPERM) => Err(OpError::coded(DaemonErrorCode::Forbidden, format!("no permission to signal pid {pid}"))),
        Err(e) => Err(OpError::plain(e.to_string())),
    }
}

/// The targets still there: each one signalled that is the same process and not a zombie, and in a group or a tree
/// what has joined it since.
fn still_here(t: &dyn Table, signalled: &[Seen], root: &Seen, mode: KillMode) -> Vec<Seen> {
    match mode {
        KillMode::Pid => signalled.iter().filter_map(|s| t.one(s.pid).filter(|now| now.same(s) && alive(now))).collect(),
        KillMode::Group => t.all().into_iter().filter(|s| s.pgid == root.pgid && alive(s)).collect(),
        KillMode::Tree => {
            let table = t.all();
            let here = table.iter().filter(|now| alive(now) && signalled.iter().any(|s| now.same(s))).cloned().collect();
            with_descendants(&table, here)
        }
    }
}

async fn wait_for(t: &dyn Table, signalled: &[Seen], root: &Seen, mode: KillMode, span: Duration) -> Vec<Seen> {
    let until = tokio::time::Instant::now() + span;
    loop {
        let here = still_here(t, signalled, root, mode);
        if here.is_empty() || tokio::time::Instant::now() >= until {
            return here;
        }
        tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
    }
}

/// Whether a daemon of these readings ends only what stands in a thread's cgroup or under it: a box's, only as root,
/// since a daemon that is not root can signal no one else's anyway, and only where cgroup v2 is mounted, since a box
/// without it launches its turns where they stand (`cgroupJoinLine`). A fork's turns stand in no such cgroup.
fn boxed(readings: &crate::readings::KindReadings, root: bool, cgroup_v2: bool) -> bool {
    readings.threads_in_cgroups && root && cgroup_v2
}

/// What says cgroup v2 is mounted, the file the box's join line asks for before it joins a thread's cgroup.
const CGROUP_V2: &str = "/sys/fs/cgroup/cgroup.controllers";

/// The longest pid file a run writes.
const PID_FILE_BYTES: u64 = 32;

/// The pids the turns this computer runs for a host recorded in its run folder: for each run still held (its claim
/// folder there and no exit code yet), the run's root (`<run>.pid`) and the agent agentWord started (`<run>.agent`).
/// A turn the host launched here through its machine road carries no pid the host knows, so this computer's daemon
/// reads them itself. The folder is the login's; a file that is a link, or holds no pid, names nothing.
pub(crate) fn recorded_runs(dir: &std::path::Path) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let held = |id: &str| dir.join(format!("{id}.d")).is_dir() && !dir.join(format!("{id}.exit")).exists();
    entries
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".agent").map(str::to_owned))
        .filter(|id| held(id))
        .flat_map(|id| [read_pid(&dir.join(format!("{id}.pid"))), read_pid(&dir.join(format!("{id}.agent")))])
        .flatten()
        .collect()
}

fn read_pid(path: &std::path::Path) -> Option<u32> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let mut text = String::new();
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?
        .take(PID_FILE_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    text.trim().parse::<u32>().ok().filter(|pid| *pid > 1)
}

/// One proc.kill frame on this daemon's platform, with the start slack the kind's process reader needs, the box rule
/// where the kind's turns stand in cgroups and the daemon is root, the turns its run folder records (`runs`, none on
/// the host's own computer, whose host names them), and the grace every daemon gives TERM.
pub(crate) async fn proc_kill(kind: &str, pid: u32, frame: Frame, runs: Option<&std::path::Path>) -> Result<ProcKillReply, OpError> {
    let readings = crate::readings::readings_for(kind, std::env::consts::OS)?;
    // An identity no process could hold names none, and the pid is not the one listed.
    let identity = frame.identity.as_deref().map(|id| id.parse::<u64>().map_err(|_| not_listed(pid))).transpose()?;
    let mode = frame.mode.unwrap_or(KillMode::Pid);
    let ask = Ask { pid, signal: frame.signal, mode, started_at: frame.started_at, identity, start_slack_ms: readings.start_slack_ms };
    let mut guard = Guard::here(frame, boxed(readings, nix::unistd::geteuid().is_root(), std::path::Path::new(CGROUP_V2).exists()));
    guard.agents.extend(runs.map(recorded_runs).unwrap_or_default());
    kill_processes(table()?.as_ref(), ask, &guard, Duration::from_millis(numbers::KILL_GRACE_MS)).await
}

#[cfg(test)]
#[path = "../tests/kill/mod.rs"]
mod tests;
