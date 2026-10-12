// SPDX-License-Identifier: AGPL-3.0-only
//! Whose each process is, stamped on proc.watch's rows off the threads the host names in proc.scopes. The first rung
//! that names an owner wins, and a process with none takes its nearest owned parent's before the next rung is read:
//! 1. it is one of a thread's own (its agent, a slate run or tool server the host started, a turn's group) or a
//!    parent of it is;
//! 2. a Mac's fork tracker followed it from one of those, or its parent (proc_track.rs);
//! 3. its group was seen under the thread while the group's leader was the thread's, which keeps an `&` or `nohup`
//!    child once the shell that led the group exits (Claude Code gives every Bash call a group of its own);
//! 4. its environment carries the thread's mark, which `setsid` does not drop; a label only, it grants nothing;
//! 5. it stands in the thread's cgroup, on a box;
//! 6. it runs in a workspace's folder: that workspace, no thread;
//! 7. it is in the daemon's own coalition on a Mac, which is the host's: wsp's, no thread.
//!
//! Groups are remembered only while the sampler runs, so a group whose leader exited while nobody watched is read by
//! the rungs after it. A remembered group and a read mark are each held against the process they were read off, by its
//! start time and, on a Mac, its unique id.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use wsp_frames::{ProcEntry, ProcKind, ProcOwnedBy, ProcOwner, ProcScope};

/// The variable a turn's launch environment carries with its thread's id, as packages/protocol/src/env.ts names it.
pub(crate) const THREAD_MARK: &str = "WSP_THREAD";
/// The variable every slate command and its then carry, and no turn's launch: beside the mark it says the process is a
/// slate's run, on a box where the host is not the one that starts it and so names no pid.
pub(crate) const SLATE_MARK: &str = "SLATE_DIR";
/// How much of an environment is read for the mark: a turn's is a few kilobytes, and Linux caps one at 2 MiB.
const ENVIRON_BYTES: u64 = 256 * 1024;
/// A thread id as the scopes cap it; a longer value is no mark of wsp's.
const MARK_MAX: usize = 64;
/// How far up a parent chain is walked, past which a chain is a loop the kernel was caught moving.
const CHAIN_MAX: usize = 256;

/// What the rungs past the parent chain read off one process, by the road its computer is read with.
pub(crate) trait OwnerFacts {
    /// The value of `THREAD_MARK` in its environment, and whether `SLATE_MARK` stands beside it.
    fn thread_mark(&self, _pid: u32) -> Option<Mark> {
        None
    }
    fn folder(&self, _pid: u32) -> Option<PathBuf> {
        None
    }
    /// Its launchd coalition, a Mac's only.
    fn coalition(&self, _pid: u32) -> Option<u64> {
        None
    }
    /// The kernel's unique id for it, never reused, a Mac's only.
    fn unique_id(&self, _pid: u32) -> Option<u64> {
        None
    }
    /// The thread a Mac's fork tracker followed it from.
    fn forked(&self, _pid: u32) -> Option<Owned> {
        None
    }
    /// The threads the host names moved, which a Mac's fork tracker follows the roots of at once, watched or not.
    fn scopes_moved(&self, _scopes: &[ProcScope]) {}
}

/// A thread's mark read off one process's environment, and whether it is a slate's run.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mark {
    pub(crate) thread: String,
    pub(crate) slate: bool,
}

/// The mark out of an environment's bytes, NUL-separated `NAME=value` entries.
pub(crate) fn mark_of(environ: &[u8]) -> Option<Mark> {
    let key = format!("{THREAD_MARK}=");
    let slate = format!("{SLATE_MARK}=");
    let mut entries = environ.split(|b| *b == 0);
    let value = entries.clone().find_map(|entry| entry.strip_prefix(key.as_bytes()))?;
    let value = std::str::from_utf8(value).ok()?;
    let word = !value.is_empty() && value.len() <= MARK_MAX && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    word.then(|| Mark { thread: value.to_owned(), slate: entries.any(|entry| entry.starts_with(slate.as_bytes())) })
}

/// The mark of one process off /proc/[pid]/environ, which reads for any binary of the same account, or as root.
pub(crate) fn mark_in(proc_root: &Path, pid: u32) -> Option<Mark> {
    let mut bytes = Vec::new();
    std::fs::File::open(proc_root.join(pid.to_string()).join("environ")).ok()?.take(ENVIRON_BYTES).read_to_end(&mut bytes).ok()?;
    mark_of(&bytes)
}

pub(crate) fn folder_in(proc_root: &Path, pid: u32) -> Option<PathBuf> {
    std::fs::read_link(proc_root.join(pid.to_string()).join("cwd")).ok()
}

/// A thread a rung names, and the kind a slate run or tool server passes to what it starts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Owned {
    pub(crate) thread: String,
    pub(crate) workspace: Option<String>,
    pub(crate) kind: Option<ProcKind>,
}

/// Which process a pid was when something was read off it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Who {
    started_at: i64,
    unique: Option<u64>,
}

fn who(p: &ProcEntry, facts: &dyn OwnerFacts) -> Who {
    Who { started_at: p.started_at, unique: facts.unique_id(p.pid) }
}

/// What the ladder keeps from one tick to the next: the groups seen under a thread, for as long as each has a member
/// and its leader is the one seen, and each process's mark, read once per process.
#[derive(Default)]
pub(crate) struct Ownership {
    groups: HashMap<u32, (Who, Owned)>,
    marks: HashMap<u32, (Who, Option<Mark>)>,
}

/// A folder by its device and inode, read without following a link at its last name: a workspace's folder is the
/// agent's to replace, and as root on a box a link swapped in for it must not make every process read as inside it.
fn dir_id(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path).ok()?;
    meta.file_type().is_dir().then(|| (meta.dev(), meta.ino()))
}

impl Ownership {
    pub(crate) fn forget(&mut self) {
        *self = Ownership::default();
    }

    /// Stamps every row's owner and kind; `own` is the daemon's pid, whose coalition is wsp's.
    pub(crate) fn stamp(&mut self, procs: &mut [ProcEntry], scopes: &[ProcScope], facts: &dyn OwnerFacts, own: u32) {
        let mut ladder = Ladder::new(procs, scopes);
        ladder.roots();
        ladder.inherit();
        ladder.rung(ProcOwnedBy::Fork, |p| facts.forked(p.pid));
        self.remember(&ladder, facts);
        let groups = &self.groups;
        ladder.rung(ProcOwnedBy::Group, |p| groups.get(&p.pgid?).map(|(_, owned)| owned.clone()));
        let marks = &mut self.marks;
        ladder.rung(ProcOwnedBy::Env, |p| {
            let now = who(p, facts);
            let mark = match marks.get(&p.pid) {
                Some((held, mark)) if *held == now => mark.clone(),
                _ => {
                    let mark = facts.thread_mark(p.pid);
                    marks.insert(p.pid, (now, mark.clone()));
                    mark
                }
            };
            mark.map(|m| Owned {
                workspace: scopes.iter().find(|s| s.thread == m.thread).and_then(|s| s.workspace.clone()),
                kind: m.slate.then_some(ProcKind::Slate),
                thread: m.thread,
            })
        });
        ladder.rung(ProcOwnedBy::Cgroup, |p| {
            let at = p.cgroup.as_deref()?;
            let under = |c: &str| at == c || at.strip_prefix(c).is_some_and(|rest| rest.starts_with('/'));
            scopes.iter().find(|s| s.cgroup.as_deref().is_some_and(under)).map(|s| Owned {
                thread: s.thread.clone(),
                workspace: s.workspace.clone(),
                kind: None,
            })
        });
        let folders: HashMap<(u64, u64), Option<String>> =
            scopes.iter().filter_map(|s| Some((dir_id(Path::new(s.folder.as_ref()?))?, s.workspace.clone()))).collect();
        if !folders.is_empty() {
            // A process's folder or one above it is a workspace's, each read once a tick whatever many run there.
            let mut under: HashMap<PathBuf, Option<Option<String>>> = HashMap::new();
            ladder.unthreaded(ProcOwnedBy::Folder, |p| {
                let cwd = facts.folder(p.pid)?;
                under
                    .entry(cwd.clone())
                    .or_insert_with(|| cwd.ancestors().find_map(|at| dir_id(at).and_then(|id| folders.get(&id).cloned())))
                    .clone()
            });
        }
        if let Some(wsp) = facts.coalition(own) {
            ladder.unthreaded(ProcOwnedBy::Coalition, |p| (facts.coalition(p.pid) == Some(wsp)).then_some(None));
        }
        let seen: HashSet<u32> = ladder.procs.iter().map(|p| p.pid).collect();
        self.marks.retain(|pid, _| seen.contains(pid));
        ladder.finish();
    }

    /// A group is the thread's while its leader is: a process the first two rungs named, in a group whose leader they
    /// named for the same thread. A group with no member left is forgotten, since its number can be reused, and so is
    /// one whose number now leads a process other than the leader seen.
    fn remember(&mut self, ladder: &Ladder<'_>, facts: &dyn OwnerFacts) {
        let alive: HashSet<u32> = ladder.procs.iter().filter_map(|p| p.pgid).collect();
        self.groups
            .retain(|g, (leader, _)| alive.contains(g) && ladder.index.get(g).is_none_or(|l| who(&ladder.procs[*l], facts) == *leader));
        let first = |by: ProcOwnedBy| matches!(by, ProcOwnedBy::Root | ProcOwnedBy::Fork);
        for (i, p) in ladder.procs.iter().enumerate() {
            let (Some(g), Some(owner)) = (p.pgid, &ladder.owner[i]) else { continue };
            if !first(owner.by) || self.groups.contains_key(&g) {
                continue;
            }
            let Some(&l) = ladder.index.get(&g) else { continue };
            let Some(leader) = ladder.owner[l].as_ref() else { continue };
            if first(leader.by) && leader.thread == owner.thread {
                let thread = owner.thread.clone().unwrap_or_default();
                self.groups.insert(g, (who(&ladder.procs[l], facts), Owned { thread, workspace: owner.workspace.clone(), kind: None }));
            }
        }
    }
}

struct Ladder<'a> {
    procs: &'a mut [ProcEntry],
    scopes: &'a [ProcScope],
    index: HashMap<u32, usize>,
    owner: Vec<Option<ProcOwner>>,
    kind: Vec<Option<ProcKind>>,
}

impl<'a> Ladder<'a> {
    fn new(procs: &'a mut [ProcEntry], scopes: &'a [ProcScope]) -> Ladder<'a> {
        let index = procs.iter().enumerate().map(|(i, p)| (p.pid, i)).collect();
        let n = procs.len();
        Ladder { procs, scopes, index, owner: vec![None; n], kind: vec![None; n] }
    }

    /// The first rung's own processes: each scope's agents and the pids the host started, then any process in one of
    /// its turns' groups.
    fn roots(&mut self) {
        let mut named: HashMap<u32, (usize, ProcKind)> = HashMap::new();
        let mut groups: HashMap<u32, usize> = HashMap::new();
        for (s, scope) in self.scopes.iter().enumerate() {
            for pid in &scope.agents {
                named.entry(*pid).or_insert((s, ProcKind::Agent));
            }
            for held in &scope.pids {
                named.entry(held.pid).or_insert((s, held.kind));
            }
            for g in &scope.groups {
                groups.entry(*g).or_insert(s);
            }
        }
        for i in 0..self.procs.len() {
            let p = &self.procs[i];
            let found = named.get(&p.pid).map(|(s, k)| (*s, Some(*k))).or_else(|| p.pgid.and_then(|g| groups.get(&g)).map(|s| (*s, None)));
            if let Some((s, kind)) = found {
                let scope = &self.scopes[s];
                self.owner[i] = Some(ProcOwner {
                    warm: false,
                    by: ProcOwnedBy::Root,
                    thread: Some(scope.thread.clone()),
                    workspace: scope.workspace.clone(),
                });
                self.kind[i] = kind;
            }
        }
    }

    /// Every process no rung named takes the owner of its nearest parent that one did, and a slate run's or a tool
    /// server's kind with it; an agent's children are not agents.
    fn inherit(&mut self) {
        for i in 0..self.procs.len() {
            if self.owner[i].is_some() {
                continue;
            }
            let mut chain = vec![i];
            let mut at = i;
            let found = loop {
                let ppid = self.procs[at].ppid;
                let Some(&up) = self.index.get(&ppid) else { break None };
                if up == at || chain.len() > CHAIN_MAX {
                    break None;
                }
                if self.owner[up].is_some() {
                    break Some(up);
                }
                chain.push(up);
                at = up;
            };
            let Some(up) = found else { continue };
            let kind = self.kind[up].filter(|k| *k != ProcKind::Agent);
            for c in chain {
                self.owner[c] = self.owner[up].clone();
                self.kind[c] = kind;
            }
        }
    }

    /// A rung that names a thread, read for each process no rung above named.
    fn rung(&mut self, by: ProcOwnedBy, mut read: impl FnMut(&ProcEntry) -> Option<Owned>) {
        for i in 0..self.procs.len() {
            if self.owner[i].is_none() {
                if let Some(o) = read(&self.procs[i]) {
                    self.owner[i] = Some(ProcOwner { warm: false, by, thread: Some(o.thread), workspace: o.workspace });
                    self.kind[i] = o.kind;
                }
            }
        }
        self.inherit();
    }

    /// A rung that names no thread: a workspace, or nothing past wsp's own.
    fn unthreaded(&mut self, by: ProcOwnedBy, mut read: impl FnMut(&ProcEntry) -> Option<Option<String>>) {
        for i in 0..self.procs.len() {
            if self.owner[i].is_none() {
                if let Some(workspace) = read(&self.procs[i]) {
                    self.owner[i] = Some(ProcOwner { warm: false, by, thread: None, workspace });
                }
            }
        }
        self.inherit();
    }

    /// Writes each row's owner and kind, an owner whose thread is a warm agent's read as warm whichever rung named it.
    fn finish(self) {
        let warm: HashSet<&str> = self.scopes.iter().filter(|s| s.warm).map(|s| s.thread.as_str()).collect();
        for ((p, owner), kind) in self.procs.iter_mut().zip(self.owner).zip(self.kind) {
            p.owner = owner.map(|o| ProcOwner { warm: o.thread.as_deref().is_some_and(|t| warm.contains(t)), ..o });
            p.kind = kind;
        }
    }
}

#[cfg(test)]
#[path = "../tests/proc_owner/mod.rs"]
mod tests;
