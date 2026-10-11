// SPDX-License-Identifier: AGPL-3.0-only
//! A Mac's fork tracker: every process a thread's roots fork, followed through kqueue as it forks and kept in a ledger
//! by the kernel's unique id, which is never reused. Claude Code runs each Bash call in a zsh that lives about 24 ms
//! before its command and exits once a server is backgrounded, so by the sampler's next tick the server is launchd's
//! orphan in a group whose leader nobody saw, and an Apple binary hides its environment from every caller. On macOS
//! 26.4, 2 s polls kept 9 of 16 test processes and a tracker like this one 15. It follows the roots the host names
//! whether or not anyone watches; it reads no numbers and wakes only when a process it follows forks or exits.
//!
//! On a fork the parent's children are listed and each new one followed, its own children with it. On the exit of a
//! process that forked, launchd's children are read once for orphans whose parent unique id, first parent's pid
//! version or group names a process of the ledger: a `( sleep & )` whose sleep exec'd after its parent was gone keeps
//! only the version. A process handed to launchd (`launchctl submit`) is launchd's own child and is never followed.

use std::collections::{HashMap, VecDeque};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use libc::{c_int, c_void};
use wsp_frames::{ProcKind, ProcScope};

use crate::libproc;
use crate::proc_owner::Owned;

/// How long a process that forked is kept after its exit, so an orphan it left is still matched to it.
const DEAD_KEEP: Duration = Duration::from_secs(10);
/// The most processes the ledger holds; a thread forking past it keeps what it had and the rungs below name the rest.
pub(crate) const LEDGER_MAX: usize = 8192;
const LAUNCHD: u32 = 1;
const STOP: usize = 0;

struct Entry {
    pid: u32,
    owner: Arc<Owned>,
    /// Every pid version it has held: an exec moves it, and a child forked before keeps the old one.
    versions: Vec<i32>,
    /// The group it led when first seen, if it led one.
    led: Option<u32>,
    forked: bool,
    gone: bool,
}

#[derive(Default)]
struct Ledger {
    entries: HashMap<u64, Entry>,
    /// Each followed process's pid while it lives.
    live: HashMap<u32, u64>,
    versions: HashMap<i32, u64>,
    leaders: HashMap<u32, u64>,
    /// The processes kept past their exit, oldest first.
    dead: VecDeque<(Instant, u64)>,
}

impl Ledger {
    fn owner_of(&self, u: &libproc::Uniq) -> Option<Arc<Owned>> {
        let hit = self.entries.get(&u.unique).or_else(|| self.entries.get(&u.parent)).or_else(|| {
            let parent = self.versions.get(&u.parent_version)?;
            self.entries.get(parent)
        })?;
        Some(Arc::clone(&hit.owner))
    }

    /// Follows `pid` and everything under it for `owner`, when it is the child of `parent` or no parent is asked. Says
    /// whether one of them was gone before it was followed, which leaves orphans to sweep for.
    fn follow(&mut self, kq: c_int, pid: u32, owner: &Arc<Owned>, parent: Option<u64>) -> bool {
        let mut gone_early = false;
        let mut work = vec![(pid, parent)];
        while let Some((pid, parent)) = work.pop() {
            let Some(u) = libproc::uniq(pid) else { continue };
            if parent.is_some_and(|p| p != u.parent) {
                continue;
            }
            if self.entries.contains_key(&u.unique) {
                continue;
            }
            self.prune();
            if self.entries.len() >= LEDGER_MAX {
                return gone_early;
            }
            // Read microseconds before the attach, as the child of the parent checked above: macOS hands pids out in
            // order up to 99,999, so the number cannot come round to another process in between.
            let attached = attach(kq, pid, u.unique);
            let led = libproc::lineage(pid).and_then(|(_, pgid)| (pgid == pid).then_some(pgid));
            let children = libproc::children(pid);
            // One gone before it was followed may have forked, its children already launchd's.
            let forked = !attached || !children.is_empty();
            let entry = Entry { pid, owner: Arc::clone(owner), versions: vec![u.version], led, forked, gone: false };
            self.entries.insert(u.unique, entry);
            self.versions.insert(u.version, u.unique);
            if let Some(g) = led {
                self.leaders.insert(g, u.unique);
            }
            if attached {
                self.live.insert(pid, u.unique);
            } else {
                gone_early = true;
                self.exited(u.unique);
            }
            work.extend(children.into_iter().map(|c| (c, Some(u.unique))));
        }
        gone_early
    }

    /// Lists the parent's children, and takes its pid version as it stands, which a child copies at its fork and an
    /// exec since then has moved.
    fn forked(&mut self, kq: c_int, pid: u32, unique: u64) -> bool {
        let Some(entry) = self.entries.get_mut(&unique) else { return false };
        entry.forked = true;
        if let Some(u) = libproc::uniq(pid).filter(|u| u.unique == unique && !entry.versions.contains(&u.version)) {
            entry.versions.push(u.version);
            self.versions.insert(u.version, unique);
        }
        let owner = Arc::clone(&entry.owner);
        let mut gone_early = false;
        for child in libproc::children(pid) {
            gone_early |= self.follow(kq, child, &owner, Some(unique));
        }
        gone_early
    }

    /// A process that never forked left nobody behind and is dropped at once; one that forked is kept a while for its
    /// orphans. Says whether it forked.
    fn exited(&mut self, unique: u64) -> bool {
        let Some(entry) = self.entries.get_mut(&unique) else { return false };
        if self.live.get(&entry.pid) == Some(&unique) {
            self.live.remove(&entry.pid);
        }
        if !entry.forked {
            self.drop_entry(unique);
            return false;
        }
        if !entry.gone {
            entry.gone = true;
            self.dead.push_back((Instant::now(), unique));
        }
        true
    }

    /// launchd's children read once for orphans of the ledger's processes.
    fn sweep(&mut self, kq: c_int) {
        for orphan in libproc::children(LAUNCHD) {
            if self.live.contains_key(&orphan) {
                continue;
            }
            let Some(u) = libproc::uniq(orphan) else { continue };
            let owner = self.owner_of(&u).or_else(|| {
                let (_, pgid) = libproc::lineage(orphan)?;
                let leader = self.entries.get(self.leaders.get(&pgid)?)?;
                Some(Arc::clone(&leader.owner))
            });
            if let Some(owner) = owner {
                self.follow(kq, orphan, &owner, None);
            }
        }
    }

    fn prune(&mut self) {
        while let Some((at, unique)) = self.dead.front().copied() {
            if at.elapsed() < DEAD_KEEP {
                break;
            }
            self.dead.pop_front();
            self.drop_entry(unique);
        }
    }

    fn drop_entry(&mut self, unique: u64) {
        let Some(entry) = self.entries.remove(&unique) else { return };
        for v in entry.versions {
            if self.versions.get(&v) == Some(&unique) {
                self.versions.remove(&v);
            }
        }
        if let Some(g) = entry.led.filter(|g| self.leaders.get(g) == Some(&unique)) {
            self.leaders.remove(&g);
        }
    }
}

fn change(kq: c_int, ident: usize, filter: i16, flags: u16, fflags: u32, udata: u64) -> bool {
    let ev = libc::kevent { ident, filter, flags, fflags, data: 0, udata: udata as usize as *mut c_void };
    unsafe { libc::kevent(kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) == 0 }
}

/// Watches one pid's forks and exit, its unique id riding on each event. False for a process already gone.
fn attach(kq: c_int, pid: u32, unique: u64) -> bool {
    change(kq, pid as usize, libc::EVFILT_PROC, libc::EV_ADD, libc::NOTE_FORK | libc::NOTE_EXIT, unique)
}

struct Shared {
    kq: OwnedFd,
    ledger: Mutex<Ledger>,
}

impl Shared {
    fn ledger(&self) -> std::sync::MutexGuard<'_, Ledger> {
        self.ledger.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn run(&self) {
        let kq = self.kq.as_raw_fd();
        let mut events: Vec<libc::kevent> =
            (0..64).map(|_| libc::kevent { ident: 0, filter: 0, flags: 0, fflags: 0, data: 0, udata: std::ptr::null_mut() }).collect();
        loop {
            let n = unsafe { libc::kevent(kq, std::ptr::null(), 0, events.as_mut_ptr(), events.len() as c_int, std::ptr::null()) };
            if n < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return;
            }
            let mut ledger = self.ledger();
            let mut sweep = false;
            for ev in &events[..n as usize] {
                if ev.filter == libc::EVFILT_USER {
                    return;
                }
                let (pid, unique) = (ev.ident as u32, ev.udata as usize as u64);
                if ev.fflags & libc::NOTE_FORK != 0 {
                    sweep |= ledger.forked(kq, pid, unique);
                }
                if ev.fflags & libc::NOTE_EXIT != 0 {
                    sweep |= ledger.exited(unique);
                }
            }
            if sweep {
                ledger.sweep(kq);
            }
            ledger.prune();
        }
    }
}

/// The tracker the daemon keeps for this computer: its kqueue and its thread start with the first root the host names.
#[derive(Default)]
pub(crate) struct Tracker {
    started: OnceLock<Option<Arc<Shared>>>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Tracker {
    fn shared(&self) -> Option<&Arc<Shared>> {
        self.started
            .get_or_init(|| {
                let kq = unsafe { libc::kqueue() };
                if kq < 0 {
                    return None;
                }
                let kq = unsafe { OwnedFd::from_raw_fd(kq) };
                if !change(kq.as_raw_fd(), STOP, libc::EVFILT_USER, libc::EV_ADD | libc::EV_CLEAR, 0, 0) {
                    return None;
                }
                let shared = Arc::new(Shared { kq, ledger: Mutex::default() });
                let runs = Arc::clone(&shared);
                let thread = std::thread::Builder::new().name("wsp-forks".into()).spawn(move || runs.run()).ok()?;
                *self.thread.lock().unwrap_or_else(|e| e.into_inner()) = Some(thread);
                Some(shared)
            })
            .as_ref()
    }

    /// Follows every root the threads name: their agents, what the host started for them, and every process in one of
    /// their turns' groups, each with what it has forked so far.
    pub(crate) fn follow(&self, scopes: &[ProcScope]) {
        let named = scopes.iter().any(|s| !s.agents.is_empty() || !s.pids.is_empty() || !s.groups.is_empty());
        if !named && self.started.get().is_none() {
            return;
        }
        let Some(shared) = self.shared() else { return };
        let kq = shared.kq.as_raw_fd();
        let mut ledger = shared.ledger();
        let mut sweep = false;
        let mut groups: HashMap<u32, Arc<Owned>> = HashMap::new();
        for s in scopes {
            let owner = |kind: Option<ProcKind>| Arc::new(Owned { thread: s.thread.clone(), workspace: s.workspace.clone(), kind });
            let agent = owner(None);
            for pid in &s.agents {
                sweep |= ledger.follow(kq, *pid, &agent, None);
            }
            for held in &s.pids {
                sweep |= ledger.follow(kq, held.pid, &owner(Some(held.kind)), None);
            }
            for g in &s.groups {
                groups.entry(*g).or_insert_with(|| Arc::clone(&agent));
            }
        }
        if !groups.is_empty() {
            for pid in libproc::pids() {
                if ledger.live.contains_key(&pid) {
                    continue;
                }
                if let Some(owner) = libproc::lineage(pid).and_then(|(_, pgid)| groups.get(&pgid)) {
                    sweep |= ledger.follow(kq, pid, owner, None);
                }
            }
        }
        if sweep {
            ledger.sweep(kq);
        }
    }

    /// The thread the ledger followed this process from, by its own unique id, its parent's or its first parent's
    /// pid version.
    pub(crate) fn owner_of(&self, pid: u32) -> Option<Arc<Owned>> {
        let shared = self.started.get()?.as_ref()?;
        let u = libproc::uniq(pid)?;
        shared.ledger().owner_of(&u)
    }

    #[cfg(test)]
    pub(crate) fn held(&self) -> usize {
        self.started.get().and_then(|s| s.as_ref()).map_or(0, |s| s.ledger().entries.len())
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        if let Some(Some(shared)) = self.started.get() {
            change(shared.kq.as_raw_fd(), STOP, libc::EVFILT_USER, 0, libc::NOTE_TRIGGER, 0);
        }
        if let Some(thread) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn scope(thread: &str, agents: Vec<u32>) -> ProcScope {
        ProcScope { thread: thread.into(), workspace: None, agents, groups: vec![], folder: None, cgroup: None, warm: false, pids: vec![] }
    }

    /// The pids a case started, ended however it ends.
    struct Ends(Vec<u32>);

    impl Drop for Ends {
        fn drop(&mut self) {
            for pid in &self.0 {
                let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(*pid as i32), nix::sys::signal::Signal::SIGKILL);
            }
        }
    }

    fn settle(what: &str, ready: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ready() {
            assert!(Instant::now() < deadline, "{what} never happened in 20 s");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn starts_with_the_first_root_and_holds_only_what_may_still_leave_an_orphan() {
        let tracker = Tracker::default();
        tracker.follow(&[scope("t_none", vec![])]);
        assert!(tracker.started.get().is_none(), "nothing named, nothing started");
        // A shell that runs fifty short commands, then a sleep it keeps, then waits on its input.
        let mut root = Command::new("/bin/sh")
            .args(["-c", "read _; i=0; while [ $i -lt 50 ]; do /usr/bin/true; i=$((i+1)); done; /bin/sleep 30 & read _"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stranger = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let mut ends = Ends(vec![root.id(), stranger.id()]);
        tracker.follow(&[scope("t_loop", vec![root.id()])]);
        let mut input = root.stdin.take().unwrap();
        writeln!(input).unwrap();
        let sleep_of = || libproc::children(root.id()).into_iter().find(|c| libproc::argv(*c, 100).as_deref() == Some("/bin/sleep 30"));
        settle("the sleep is followed", || sleep_of().is_some_and(|c| tracker.owner_of(c).is_some_and(|o| o.thread == "t_loop")));
        // Each true exited without forking and was dropped: the shell and its sleep are what is held.
        settle("the trues are dropped", || tracker.held() == 2);
        assert_eq!(tracker.owner_of(root.id()).map(|o| o.thread.clone()).as_deref(), Some("t_loop"));
        assert_eq!(tracker.owner_of(stranger.id()), None);
        let sleep = sleep_of().unwrap();
        ends.0.push(sleep);
        // The shell exits; its sleep is launchd's now and still the thread's, by its own unique id.
        writeln!(input).unwrap();
        root.wait().unwrap();
        settle("the sleep is an orphan", || libproc::lineage(sleep).is_some_and(|(ppid, _)| ppid == LAUNCHD));
        assert_eq!(tracker.owner_of(sleep).map(|o| o.thread.clone()).as_deref(), Some("t_loop"));
        drop(ends);
        let _ = stranger.wait();
    }

    /// This process's own cpu and its reaped children's, in seconds.
    fn cpu() -> (f64, f64) {
        let read = |who| {
            let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
            assert_eq!(unsafe { libc::getrusage(who, usage.as_mut_ptr()) }, 0);
            let usage = unsafe { usage.assume_init() };
            let seconds = |t: libc::timeval| t.tv_sec as f64 + f64::from(t.tv_usec) / 1e6;
            seconds(usage.ru_utime) + seconds(usage.ru_stime)
        };
        (read(libc::RUSAGE_SELF), read(libc::RUSAGE_CHILDREN))
    }

    /// The tracker's cost, run alone: `cargo test --lib proc_track::tests::cost -- --ignored --nocapture`. The test's
    /// own thread only waits, so this process's cpu is the tracker's.
    #[test]
    #[ignore = "a measurement that takes 70 s: run it alone with --ignored --nocapture"]
    fn cost() {
        let tracker = Tracker::default();
        let mut idle = Command::new("/bin/sleep").arg("70").spawn().unwrap();
        let _ends = Ends(vec![idle.id()]);
        tracker.follow(&[scope("t_idle", vec![idle.id()])]);
        let (before, _) = cpu();
        std::thread::sleep(Duration::from_secs(60));
        let (after, _) = cpu();
        eprintln!("idle 60 s, one root: {:.6} s of cpu", after - before);
        let _ = idle.kill();
        let _ = idle.wait();
        let storm = "i=0; while [ $i -lt 3000 ]; do /usr/bin/true; i=$((i+1)); done";
        for followed in [false, true] {
            let (self0, kids0) = cpu();
            let started = Instant::now();
            let mut root = Command::new("/bin/sh").args(["-c", &format!("read _; {storm}")]).stdin(Stdio::piped()).spawn().unwrap();
            if followed {
                tracker.follow(&[scope("t_storm", vec![root.id()])]);
            }
            writeln!(root.stdin.take().unwrap()).unwrap();
            root.wait().unwrap();
            let wall = started.elapsed();
            let (self1, kids1) = cpu();
            eprintln!(
                "3000 processes, followed {followed}: the run {:.3} s of cpu over {:.2} s, the tracker {:.4} s ({:.2}%), held after {}",
                kids1 - kids0,
                wall.as_secs_f64(),
                self1 - self0,
                (self1 - self0) / (kids1 - kids0) * 100.0,
                tracker.held()
            );
        }
    }
}
