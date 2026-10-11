// SPDX-License-Identifier: AGPL-3.0-only
//! Every process on the machine as proc.snapshot and proc.changes events: a watching socket's first frame is the
//! whole list, every later one what moved since the frame before. The sampler holds the clock, the subscribers and
//! the last scan; what a machine of one kind is read with is its own module behind ProcSource, registered in readings.rs.
//! The module here is the guest's: straight from /proc, one stat file per pid per tick, cmdline only when a pid is
//! new or exec'd, the uid from the pid directory's owner. One sampler per daemon; the first subscriber starts it
//! and the last one leaving stops it. Inspecting one pid is the only path that touches /proc/net.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use tokio::task::AbortHandle;
use tokio::time::MissedTickBehavior;
use wsp_frames::{
    numbers, words, DaemonErrorCode, DaemonEvent, ProcCost, ProcEntry, ProcInspectReply, ProcListReply, ProcScope, ProcSignal,
};

use crate::paths::OpError;
use crate::proc_owner::{folder_in, mark_in, OwnerFacts, Ownership};
use crate::sys::{finite, read_named};
use crate::{frame_text, Outbound, SharedLog};

/// /proc reports times in USER_HZ ticks, 100 a second on every Linux.
const USER_HZ: f64 = 100.0;
const AT_PAGESZ: u64 = 6;
/// The listening state in /proc/net/tcp's st column.
const TCP_LISTEN: &str = "0A";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcStat {
    pub(crate) pid: u32,
    pub(crate) comm: String,
    pub(crate) state: String,
    pub(crate) ppid: u32,
    pub(crate) pgrp: u32,
    /// utime plus stime, in ticks.
    pub(crate) ticks: u64,
    pub(crate) threads: u32,
    /// Ticks after boot.
    pub(crate) starttime: u64,
    pub(crate) rss_pages: u64,
}

/// The comm sits in parens and may hold anything, so the split runs from the last one.
pub(crate) fn parse_proc_pid_stat(text: &str) -> Result<ProcStat, String> {
    let malformed = || "/proc/[pid]/stat is malformed".to_owned();
    let open = text.find('(').ok_or_else(malformed)?;
    let close = text.rfind(')').filter(|close| *close > open).ok_or_else(malformed)?;
    let pid: u32 = text[..open].trim().parse().map_err(|_| malformed())?;
    let f: Vec<&str> = text[close + 1..].split_whitespace().collect();
    let n = |i: usize| -> Result<u64, String> { f.get(i).and_then(|v| v.parse::<u64>().ok()).ok_or_else(malformed) };
    Ok(ProcStat {
        pid,
        comm: text[open + 1..close].to_owned(),
        state: f.first().map_or("?", |s| *s).to_owned(),
        ppid: u32::try_from(n(1)?).map_err(|_| malformed())?,
        pgrp: u32::try_from(n(2)?).map_err(|_| malformed())?,
        ticks: n(11)? + n(12)?,
        threads: u32::try_from(n(17)?).map_err(|_| malformed())?,
        starttime: n(19)?,
        rss_pages: n(21)?,
    })
}

pub(crate) fn parse_btime(proc_stat: &str) -> Result<i64, String> {
    proc_stat
        .lines()
        .find_map(|l| l.strip_prefix("btime").filter(|rest| rest.starts_with(char::is_whitespace)))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| "/proc/stat has no btime".to_owned())
}

/// auxv is an array of (type, value) native words; AT_PAGESZ carries the page size.
pub(crate) fn parse_auxv_page_size(buf: &[u8]) -> Option<u64> {
    let word = |at: usize| u64::from_le_bytes(buf[at..at + 8].try_into().ok()?).into();
    let mut i = 0;
    while i + 16 <= buf.len() {
        let kind: Option<u64> = word(i);
        match kind {
            Some(0) => return None,
            Some(AT_PAGESZ) => return word(i + 8),
            _ => i += 16,
        }
    }
    None
}

pub(crate) fn parse_passwd_users(text: &str) -> HashMap<u32, String> {
    let mut users = HashMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() >= 3 && !f[0].is_empty() {
            if let Ok(uid) = f[2].parse::<u32>() {
                users.insert(uid, f[0].to_owned());
            }
        }
    }
    users
}

/// The listening rows of one /proc/net/tcp text as (port, inode). The ports module carries the whole row; the
/// inspect here needs the two columns that tie a socket inode to its port.
pub(crate) fn listening_rows(text: &str) -> Vec<(u16, u64)> {
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 || f[3] != TCP_LISTEN {
            continue;
        }
        let port = f[1].rsplit(':').next().and_then(|hex| u16::from_str_radix(hex, 16).ok());
        let inode = f[9].parse::<u64>().ok();
        if let (Some(port), Some(inode)) = (port, inode) {
            rows.push((port, inode));
        }
    }
    rows
}

/// The TCP ports one pid listens on: its socket inodes out of its fd table against the listening rows of both
/// address families, sorted, each once.
pub(crate) fn listening_ports_of(proc_root: &Path, pid: u32) -> Vec<u16> {
    let dir = proc_root.join(pid.to_string());
    let mut inodes = HashSet::new();
    if let Ok(fds) = std::fs::read_dir(dir.join("fd")) {
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else { continue };
            let target = target.to_string_lossy();
            if let Some(inode) = target.strip_prefix("socket:[").and_then(|s| s.strip_suffix(']')).and_then(|s| s.parse::<u64>().ok()) {
                inodes.insert(inode);
            }
        }
    }
    let mut ports = Vec::new();
    for file in ["tcp", "tcp6"] {
        let text = std::fs::read_to_string(proc_root.join("net").join(file)).unwrap_or_default();
        ports.extend(listening_rows(&text).into_iter().filter(|(_, inode)| inodes.contains(inode)).map(|(port, _)| port));
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// What one scan read: every process the module shows, and how many there are before the cap.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProcScan {
    pub(crate) total: u64,
    pub(crate) procs: Vec<ProcEntry>,
    /// The cpu milliseconds the processes the scan ran and reaped spent, `ps` on the road that runs it, which the
    /// thread's own clock leaves out.
    pub(crate) child_ms: f64,
}

pub(crate) struct ProcScanInput {
    /// The clock this scan is stamped with, epoch milliseconds.
    pub(crate) at: i64,
    /// Wall milliseconds since the last scan, 0 on the first, where a cpu delta has nothing to run from.
    pub(crate) elapsed_ms: i64,
    /// The daemon pty a pid's shell belongs to, for the rows that carry one.
    pub(crate) pty: HashMap<u32, String>,
    /// Only the processes in these cgroups, and the daemon itself, where a source can list them; every process
    /// without it.
    pub(crate) only: Option<Vec<String>>,
    /// The daemon's own pid, which a scan narrowed to cgroups keeps.
    pub(crate) daemon: u32,
}

/// One kind's road to the processes on its machine, and to one of them in depth, run off the runtime thread. A
/// machine wsp forks reads its own /proc (ProcFsSource); this computer reads its own host with ps. Adding a kind is
/// its module and the row in readings.rs, nothing here.
pub(crate) trait ProcSource: OwnerFacts + Send + Sync {
    fn scan(&self, input: &ProcScanInput) -> Result<ProcScan, String>;
    /// One process in depth. `procs` is the newest scan, which is where the children column comes from.
    fn inspect(&self, pid: u32, procs: &[ProcEntry]) -> Result<ProcInspectReply, OpError>;
}

pub(crate) fn no_process(pid: u32) -> OpError {
    OpError::coded(DaemonErrorCode::NotFound, format!("no process {pid}"))
}

pub(crate) fn children_of(pid: u32, procs: &[ProcEntry]) -> Vec<u32> {
    procs.iter().filter(|p| p.ppid == pid).map(|p| p.pid).collect()
}

struct Known {
    starttime: u64,
    comm: String,
    ticks: u64,
    cmdline: String,
    user: String,
}

#[derive(Default)]
struct FsState {
    /// Per pid across scans: the ticks the cpu delta runs from, and what is only re-read after an exec.
    known: HashMap<u32, Known>,
    btime: Option<i64>,
    page_size: u64,
    users: HashMap<u32, String>,
}

/// The most cgroup folders one narrowed scan walks: every thread's, and those it made under its own.
const CGROUP_WALK_DIRS: usize = 4096;

/// The guest's own processes, read straight from /proc: the module every machine wsp forks is served by.
pub(crate) struct ProcFsSource {
    proc_root: PathBuf,
    passwd_path: PathBuf,
    /// Where the cgroup v2 tree is mounted, whose cgroup.procs a narrowed scan lists its pids from.
    cgroup_root: PathBuf,
    cap: usize,
    state: Mutex<FsState>,
}

/// A kernel thread: kthreadd, or a child of it. Neither counts toward the cap, since a 2 vCPU box runs 84 of them.
fn kernel_thread(p: &ProcEntry) -> bool {
    p.ppid == 2 || (p.ppid == 0 && p.comm == "kthreadd")
}

impl ProcFsSource {
    pub(crate) fn new(proc_root: PathBuf, passwd_path: PathBuf, cap: usize) -> ProcFsSource {
        ProcFsSource { proc_root, passwd_path, cgroup_root: PathBuf::from("/sys/fs/cgroup"), cap, state: Mutex::new(FsState::default()) }
    }

    #[cfg(test)]
    fn with_cgroup_root(self, cgroup_root: PathBuf) -> ProcFsSource {
        ProcFsSource { cgroup_root, ..self }
    }

    /// The pids standing in each cgroup and every cgroup under it, as a thread's end takes them, the daemon's own added:
    /// a cgroup gone or never made lists none. The walk stops at CGROUP_WALK_DIRS folders, deeper than any thread's.
    fn members(&self, cgroups: &[String], daemon: u32) -> Vec<u32> {
        let mut pids = vec![daemon];
        let mut folders: Vec<PathBuf> = cgroups.iter().map(|cg| self.cgroup_root.join(cg.trim_start_matches('/'))).collect();
        let mut walked = 0;
        while let Some(folder) = folders.pop() {
            walked += 1;
            if walked > CGROUP_WALK_DIRS {
                break;
            }
            pids.extend(
                std::fs::read_to_string(folder.join("cgroup.procs"))
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|l| l.trim().parse::<u32>().ok()),
            );
            let Ok(entries) = std::fs::read_dir(&folder) else { continue };
            folders.extend(entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()));
        }
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    fn prime(&self, state: &mut FsState) -> Result<(), String> {
        state.btime = Some(parse_btime(&read_named(&self.proc_root.join("stat"))?)?);
        let auxv = std::fs::read(self.proc_root.join("self/auxv")).unwrap_or_default();
        state.page_size = parse_auxv_page_size(&auxv).filter(|n| *n > 0).unwrap_or(4096);
        state.users = parse_passwd_users(&std::fs::read_to_string(&self.passwd_path).unwrap_or_default());
        Ok(())
    }

    fn read_one(&self, state: &mut FsState, pid: u32, elapsed_ticks: f64, pty: Option<&String>) -> Option<ProcEntry> {
        let dir = self.proc_root.join(pid.to_string());
        let st = parse_proc_pid_stat(&std::fs::read_to_string(dir.join("stat")).ok()?).ok()?;
        let mut cpu = 0.0;
        let same_image = state.known.get(&pid).is_some_and(|k| k.starttime == st.starttime && k.comm == st.comm);
        if let Some(k) = state.known.get(&pid) {
            if k.starttime == st.starttime && elapsed_ticks > 0.0 {
                cpu = finite((st.ticks as f64 - k.ticks as f64) / elapsed_ticks * 100.0).max(0.0);
            }
        }
        if same_image {
            if let Some(k) = state.known.get_mut(&pid) {
                k.ticks = st.ticks;
            }
        } else {
            // A new pid, or the same pid after an exec (the comm moved): the image changed, so cmdline and owner are read again.
            let uid = std::fs::metadata(&dir).ok().map(|m| m.uid());
            let user = uid.and_then(|uid| state.users.get(&uid).cloned()).unwrap_or_else(|| uid.map_or("-1".to_owned(), |u| u.to_string()));
            let cmdline = read_cmdline(&dir);
            state.known.insert(pid, Known { starttime: st.starttime, comm: st.comm.clone(), ticks: st.ticks, cmdline, user });
        }
        let k = state.known.get(&pid)?;
        Some(ProcEntry {
            pid,
            ppid: st.ppid,
            user: k.user.clone(),
            state: st.state,
            comm: st.comm,
            cmdline: k.cmdline.clone(),
            cpu,
            rss: st.rss_pages * state.page_size,
            started_at: state.btime.unwrap_or(0) * 1000 + st.starttime as i64 * numbers::STAT_TICK_MS as i64,
            pty: pty.cloned(),
            cgroup: read_cgroup(&dir),
            energy_mw: None,
            pgid: Some(st.pgrp),
            owner: None,
            kind: None,
        })
    }
}

/// The longest cgroup path a row carries; a deeper one is left off rather than cut, since a cut path names
/// another cgroup.
const CGROUP_PATH_BYTES: u64 = 512;

/// The cgroup v2 path a process stands in, off the `0::` line of /proc/[pid]/cgroup; nothing on a v1-only machine
/// or where the file cannot be read. Read every tick, since a process moves cgroup without changing its image.
pub(crate) fn read_cgroup(dir: &Path) -> Option<String> {
    let mut text = String::new();
    std::fs::File::open(dir.join("cgroup")).ok()?.take(CGROUP_PATH_BYTES + 4).read_to_string(&mut text).ok()?;
    text.lines().find_map(|line| line.strip_prefix("0::")).map(str::to_owned)
}

/// The first CMDLINE_BYTES only, the NULs as spaces; nothing for a pid whose cmdline cannot be read.
fn read_cmdline(dir: &Path) -> String {
    let Ok(mut file) = std::fs::File::open(dir.join("cmdline")) else { return String::new() };
    let mut buf = [0u8; numbers::CMDLINE_BYTES];
    let mut end = 0;
    while end < buf.len() {
        match file.read(&mut buf[end..]) {
            Ok(0) => break,
            Ok(n) => end += n,
            Err(_) => return String::new(),
        }
    }
    while end > 0 && buf[end - 1] == 0 {
        end -= 1;
    }
    String::from_utf8_lossy(&buf[..end]).replace('\0', " ")
}

impl OwnerFacts for ProcFsSource {
    fn thread_mark(&self, pid: u32) -> Option<crate::proc_owner::Mark> {
        mark_in(&self.proc_root, pid)
    }
    fn folder(&self, pid: u32) -> Option<PathBuf> {
        folder_in(&self.proc_root, pid)
    }
}

impl ProcSource for ProcFsSource {
    /// Reads every pid once, or with `only` the pids its cgroups list; cpu is the tick delta against the last scan
    /// over the wall time between them. The count is every process on the machine either way. Reading every pid, the
    /// cap takes the threads' cgroups' members first, then the rest by pid, and kernel threads never count toward it.
    fn scan(&self, input: &ProcScanInput) -> Result<ProcScan, String> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.btime.is_none() {
            self.prime(&mut state)?;
        }
        let elapsed_ticks = input.elapsed_ms as f64 / 1000.0 * USER_HZ;
        let listed = std::fs::read_dir(&self.proc_root).map_err(|e| format!("{}: {e}", self.proc_root.display()))?;
        let mut pids: Vec<u32> = listed.flatten().filter_map(|d| d.file_name().to_str().and_then(|n| n.parse().ok())).collect();
        pids.sort_unstable();
        let read = match &input.only {
            Some(cgroups) => self.members(cgroups, input.daemon),
            None => {
                let threads: HashSet<u32> = self.members(&[numbers::THREAD_CGROUPS.to_owned()], input.daemon).into_iter().collect();
                let (first, rest): (Vec<u32>, Vec<u32>) = pids.iter().partition(|pid| threads.contains(pid));
                first.into_iter().chain(rest).collect()
            }
        };
        let mut procs = Vec::new();
        let mut seen = HashSet::new();
        let mut counted = 0;
        for pid in &read {
            if counted >= self.cap {
                break;
            }
            if let Some(entry) = self.read_one(&mut state, *pid, elapsed_ticks, input.pty.get(pid)) {
                if !kernel_thread(&entry) {
                    counted += 1;
                }
                seen.insert(*pid);
                procs.push(entry);
            }
        }
        procs.sort_unstable_by_key(|p| p.pid);
        state.known.retain(|pid, _| seen.contains(pid));
        Ok(ProcScan { total: pids.len() as u64, procs, child_ms: 0.0 })
    }

    fn inspect(&self, pid: u32, procs: &[ProcEntry]) -> Result<ProcInspectReply, OpError> {
        let dir = self.proc_root.join(pid.to_string());
        let st =
            std::fs::read_to_string(dir.join("stat")).ok().and_then(|t| parse_proc_pid_stat(&t).ok()).ok_or_else(|| no_process(pid))?;
        let cwd = std::fs::read_link(dir.join("cwd")).ok().map(|p| p.to_string_lossy().into_owned());
        Ok(ProcInspectReply {
            pid,
            cwd,
            ports: listening_ports_of(&self.proc_root, pid),
            threads: Some(st.threads),
            children: children_of(pid, procs),
        })
    }
}

/// The daemon's live ptys as (shell pid, pty id), rebuilt per scan: an exited pty's pid can be reused by a stranger.
pub(crate) type PtyPids = Arc<dyn Fn() -> Vec<(u32, String)> + Send + Sync>;
pub(crate) type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

pub(crate) struct ProcSamplerOptions {
    /// The daemon's own pid, named in every snapshot.
    pub(crate) self_pid: u32,
    pub(crate) ptys: PtyPids,
    /// From the cpu baseline to the first snapshot.
    pub(crate) first: Duration,
    /// Between every snapshot after it.
    pub(crate) interval: Duration,
    pub(crate) now: Clock,
    pub(crate) log: SharedLog,
}

#[derive(Default)]
struct Inner {
    subscribers: HashMap<u64, Outbound>,
    /// What each subscriber reads: the cgroups it named, or None for every process.
    scopes: HashMap<u64, Option<Vec<String>>>,
    running: Option<AbortHandle>,
    last_at: Option<i64>,
    last_procs: Vec<ProcEntry>,
    last_total: u64,
    /// Whether the last tick read every process, so a list may ride it; a tick narrowed to cgroups read only theirs.
    last_whole: bool,
    /// What the last tick said, so a watch that arrives mid-stream inherits it; None until one has finished.
    last: Option<Result<(), String>>,
    /// The list every subscriber holds, which the next frame's changes are read against; None until the first
    /// snapshot after a start.
    sent: Option<Sent>,
    /// The seq of the last frame sent, counted for as long as the daemon runs.
    seq: u64,
    /// The subscribers that watched again, each of which is sent a whole snapshot in place of the next changes.
    owed: HashSet<u64>,
    /// The threads the host last named, which every stamped scan reads its rows' owners off.
    threads: Arc<Vec<ProcScope>>,
}

struct Sent {
    seq: u64,
    at: i64,
    total: u64,
    procs: Vec<ProcEntry>,
}

/// The cpu time the calling thread has spent, in milliseconds; zero where the clock cannot be read.
fn thread_cpu_ms() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) } != 0 {
        return 0.0;
    }
    ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1e6
}

/// The memory one list of rows takes: each row and the text it owns.
fn held_bytes(procs: &[ProcEntry]) -> u64 {
    let text = |p: &ProcEntry| {
        p.user.capacity()
            + p.state.capacity()
            + p.comm.capacity()
            + p.cmdline.capacity()
            + p.pty.as_ref().map_or(0, String::capacity)
            + p.cgroup.as_ref().map_or(0, String::capacity)
            + p.owner
                .as_ref()
                .map_or(0, |o| o.thread.as_ref().map_or(0, String::capacity) + o.workspace.as_ref().map_or(0, String::capacity))
    };
    procs.iter().map(|p| (size_of::<ProcEntry>() + text(p)) as u64).sum()
}

/// The rows of `now` that are new or differ from `before`, and the pids of `before` that `now` no longer lists.
fn changes(before: &[ProcEntry], now: &[ProcEntry]) -> (Vec<ProcEntry>, Vec<u32>) {
    let held: HashMap<u32, &ProcEntry> = before.iter().map(|p| (p.pid, p)).collect();
    let listed: HashSet<u32> = now.iter().map(|p| p.pid).collect();
    let moved = now.iter().filter(|p| held.get(&p.pid) != Some(p)).cloned().collect();
    let gone = before.iter().map(|p| p.pid).filter(|pid| !listed.contains(pid)).collect();
    (moved, gone)
}

pub(crate) struct ProcSampler {
    source: Arc<dyn ProcSource>,
    opts: ProcSamplerOptions,
    inner: Mutex<Inner>,
    /// Read and written inside each poll's blocking scan, never beside another.
    ownership: Arc<Mutex<Ownership>>,
}

impl ProcSampler {
    pub(crate) fn new(source: Arc<dyn ProcSource>, opts: ProcSamplerOptions) -> Arc<ProcSampler> {
        let opts = ProcSamplerOptions { first: crate::sys::tick_of(opts.first), interval: crate::sys::tick_of(opts.interval), ..opts };
        Arc::new(ProcSampler { source, opts, inner: Mutex::new(Inner::default()), ownership: Arc::default() })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[cfg(test)]
    pub(crate) fn running(&self) -> bool {
        self.lock().running.is_some()
    }

    /// A socket joining a stream already under way is sent the list the others hold, so the changes after it read
    /// against the same rows for everyone.
    pub(crate) fn subscribe(self: &Arc<Self>, key: u64, out: Outbound, scope: Option<Vec<String>>) {
        let mut inner = self.lock();
        if let Some(sent) = &inner.sent {
            let snapshot = DaemonEvent::ProcSnapshot {
                at: sent.at,
                daemon: self.opts.self_pid,
                total: sent.total,
                procs: sent.procs.clone(),
                seq: Some(sent.seq),
                cost: None,
            };
            out.send_text(&frame_text(&snapshot));
        }
        inner.subscribers.insert(key, out);
        inner.scopes.insert(key, scope);
        if inner.subscribers.len() == 1 {
            self.start(&mut inner);
        }
    }

    /// A subscriber that watches again, naming its set anew: the next tick reads what they all name now, and its next
    /// frame is whole.
    pub(crate) fn rescope(&self, key: u64, scope: Option<Vec<String>>) {
        let mut inner = self.lock();
        if inner.subscribers.contains_key(&key) {
            inner.scopes.insert(key, scope);
            inner.owed.insert(key);
        }
    }

    /// The threads every stamped scan reads owners off, the whole list each time, whose roots a Mac's tracker follows
    /// from now on.
    pub(crate) fn set_scopes(&self, scopes: Vec<ProcScope>) {
        self.source.scopes_moved(&scopes);
        self.lock().threads = Arc::new(scopes);
    }

    pub(crate) fn unsubscribe(&self, key: u64) {
        let mut inner = self.lock();
        inner.scopes.remove(&key);
        if inner.subscribers.remove(&key).is_some() && inner.subscribers.is_empty() {
            self.stop(&mut inner);
        }
    }

    /// One scan before a watch is taken, so a module that cannot read this machine refuses the op instead of leaving
    /// the pane at pending for a stream that never comes. Its reading is thrown away: cpu is a delta, so the first
    /// snapshot still lands `first` after the reply. A sampler already polling scans every interval anyway, so a
    /// second watcher inherits what the last tick said rather than paying for a scan of its own, which would also run
    /// beside that tick and leave the module's per pid state read from two clocks.
    pub(crate) async fn probe(&self) -> Result<(), OpError> {
        let inherited = {
            let inner = self.lock();
            if inner.running.is_some() {
                inner.last.clone()
            } else {
                None
            }
        };
        match inherited {
            Some(said) => said.map_err(OpError::plain),
            None => self.scan(0, (self.opts.now)(), self.only(), false).await.map(|_| ()).map_err(OpError::plain),
        }
    }

    /// What a tick reads for its subscribers together: every process where any of them names no cgroups, else the
    /// cgroups they name between them.
    fn scope(inner: &Inner) -> Option<Vec<String>> {
        let mut union = Vec::new();
        for scope in inner.scopes.values() {
            union.extend(scope.as_ref()?.iter().cloned());
        }
        union.sort();
        union.dedup();
        Some(union)
    }

    /// What the next tick reads: the subscribers' scope, every process where none subscribes.
    fn only(&self) -> Option<Vec<String>> {
        let inner = self.lock();
        if inner.scopes.is_empty() {
            None
        } else {
            Self::scope(&inner)
        }
    }

    /// The scan of `only`, every process without it, and the cpu milliseconds it took on the thread that ran it. A
    /// stamped scan names each row's owner; a probe's or an inspect's is thrown away or read for its tree alone.
    async fn scan(&self, elapsed_ms: i64, at: i64, only: Option<Vec<String>>, stamp: bool) -> Result<(ProcScan, f64), String> {
        let source = Arc::clone(&self.source);
        let pty: HashMap<u32, String> = (self.opts.ptys)().into_iter().collect();
        let daemon = self.opts.self_pid;
        let threads = Arc::clone(&self.lock().threads);
        let ownership = Arc::clone(&self.ownership);
        tokio::task::spawn_blocking(move || {
            let from = thread_cpu_ms();
            let mut scan = source.scan(&ProcScanInput { at, elapsed_ms, pty, only, daemon })?;
            if stamp {
                ownership.lock().unwrap_or_else(|e| e.into_inner()).stamp(&mut scan.procs, &threads, source.as_ref(), daemon);
            }
            Ok((scan, thread_cpu_ms() - from))
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()))
    }

    /// The first poll after a start is the cpu baseline and emits nothing; a failed read of one pid drops that pid for the tick.
    pub(crate) async fn poll(&self) {
        let at = (self.opts.now)();
        let elapsed = self.lock().last_at.map_or(0, |last| at - last);
        let only = self.only();
        let whole = only.is_none();
        let scanned = self.scan(elapsed, at, only, true).await;
        let mut inner = self.lock();
        let framing = thread_cpu_ms();
        let (scan, scan_ms) = match scanned {
            Ok(read) => read,
            Err(e) => {
                inner.last = Some(Err(e));
                return;
            }
        };
        inner.last = Some(Ok(()));
        let first = inner.last_at.is_none();
        inner.last_at = Some(at);
        inner.last_procs.clone_from(&scan.procs);
        inner.last_total = scan.total;
        inner.last_whole = whole;
        if first {
            return;
        }
        let daemon = self.opts.self_pid;
        let seq = inner.seq + 1;
        inner.seq = seq;
        // The rows are held twice between ticks, the last read and the list every watcher holds; the cpu is this tick's
        // read, what it ran, and the changes found, the frames' text and their sends after it left out.
        let moved_rows = inner.sent.as_ref().map(|sent| (changes(&sent.procs, &scan.procs), sent.seq));
        let cost =
            Some(ProcCost { cpu_ms: scan_ms + scan.child_ms + (thread_cpu_ms() - framing).max(0.0), held: 2 * held_bytes(&scan.procs) });
        let whole = frame_text(&DaemonEvent::ProcSnapshot {
            at,
            daemon,
            total: scan.total,
            procs: scan.procs.clone(),
            seq: Some(seq),
            cost: cost.clone(),
        });
        let moved = moved_rows.map(|((procs, gone), base)| {
            frame_text(&DaemonEvent::ProcChanges { at, daemon, total: scan.total, procs, gone, seq, base, cost: cost.clone() })
        });
        let owed = std::mem::take(&mut inner.owed);
        for (key, out) in &inner.subscribers {
            match &moved {
                Some(moved) if !owed.contains(key) => out.send_text(moved),
                _ => out.send_text(&whole),
            };
        }
        inner.sent = Some(Sent { seq, at, total: scan.total, procs: scan.procs });
    }

    pub(crate) async fn inspect(&self, pid: u32) -> Result<ProcInspectReply, OpError> {
        let held = {
            let inner = self.lock();
            inner.last_at.map(|_| inner.last_procs.clone())
        };
        let procs = match held {
            Some(procs) => procs,
            None => self.scan(0, (self.opts.now)(), self.only(), false).await.map_err(OpError::plain)?.0.procs,
        };
        let source = Arc::clone(&self.source);
        tokio::task::spawn_blocking(move || source.inspect(pid, &procs)).await.unwrap_or_else(|e| Err(OpError::plain(e.to_string())))
    }

    /// Every process once, each row's owner named, since settle and delete read their thread's processes off it with
    /// nobody watching. A running sampler's last rows stand where that tick read every process, since a scan of its
    /// own beside the tick would leave the module's per pid state read from two clocks; with none running, or one
    /// narrowed to cgroups, one scan, whose cpu reads 0, and the narrowed watch's next cpu reads low for that tick.
    pub(crate) async fn list(&self) -> Result<ProcListReply, OpError> {
        let held = {
            let inner = self.lock();
            inner.last_at.filter(|_| inner.last_whole).map(|at| (at, inner.last_total, inner.last_procs.clone()))
        };
        let daemon = self.opts.self_pid;
        let (at, total, procs) = match held {
            Some(held) => held,
            None => {
                let at = (self.opts.now)();
                let (scan, _) = self.scan(0, at, None, true).await.map_err(OpError::plain)?;
                (at, scan.total, scan.procs)
            }
        };
        Ok(ProcListReply { at, daemon, total, procs })
    }

    fn start(self: &Arc<Self>, inner: &mut Inner) {
        if inner.running.is_some() {
            return;
        }
        inner.last_at = None;
        inner.sent = None;
        let me = Arc::clone(self);
        let task = tokio::spawn(async move {
            me.poll().await;
            tokio::time::sleep(me.opts.first).await;
            let mut ticker = tokio::time::interval(me.opts.interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                me.poll().await;
            }
        });
        inner.running = Some(task.abort_handle());
        (self.opts.log)(words::PROC_SAMPLER_STARTED);
    }

    fn stop(&self, inner: &mut Inner) {
        if let Some(task) = inner.running.take() {
            task.abort();
            (self.opts.log)(words::PROC_SAMPLER_STOPPED);
        }
        inner.last_at = None;
        inner.last = None;
        inner.sent = None;
        inner.owed.clear();
        self.ownership.lock().unwrap_or_else(|e| e.into_inner()).forget();
    }
}

/// init, the daemon and whatever started the daemon: never signalled, the workspace would go with them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProtectedPids {
    pub(crate) this: u32,
    pub(crate) parent: u32,
}

/// Signals one process.
pub(crate) fn kill_process(pid: u32, signal: ProcSignal, protected: ProtectedPids) -> Result<(), OpError> {
    if pid == 1 || pid == protected.this || pid == protected.parent {
        return Err(OpError::coded(
            DaemonErrorCode::Forbidden,
            format!("refusing to signal pid {pid}: it is init, the daemon or the daemon's parent"),
        ));
    }
    let signal = match signal {
        ProcSignal::Kill => Signal::SIGKILL,
        ProcSignal::Term => Signal::SIGTERM,
    };
    match kill(Pid::from_raw(pid as i32), signal) {
        Ok(()) => Ok(()),
        Err(nix::errno::Errno::ESRCH) => Err(no_process(pid)),
        Err(nix::errno::Errno::EPERM) => Err(OpError::coded(DaemonErrorCode::Forbidden, format!("no permission to signal pid {pid}"))),
        Err(e) => Err(OpError::plain(e.to_string())),
    }
}

#[cfg(test)]
#[path = "../tests/fake_proc/mod.rs"]
pub(crate) mod fake_proc;

#[cfg(test)]
#[path = "../tests/proc/mod.rs"]
mod tests;
