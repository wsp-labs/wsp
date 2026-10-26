// SPDX-License-Identifier: AGPL-3.0-only
//! The computer table's rules, after `components/procs/tree.ts` and `kill.ts`: a tree by parent with siblings sorted by
//! the chosen column, a filter that keeps a match's parents above it, the rows that are a turn's agent, the harness
//! pieces a row is labelled as, and the two presses a kill takes. Pure, so the pane draws and the tests read one rule.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::screens::settings::pages::activity::model::Thread;
use crate::screens::settings::pages::activity::place::{
    agent_bin, agent_by_bin, app_line, is_run_script, place_procs, PlaceInput, PlaceThread,
};
use crate::screens::settings::pages::activity::wire::{AppPart, Pid, ProcEntry};

/// The pane's words (`ProcessesSurface.tsx`).
pub mod words {
    pub const FILTER: &str = "filter";
    pub const PENDING: &str = "pending";
    pub const UNAVAILABLE: &str = "unavailable";
    pub const AGENT_UNNAMED: &str = "a thread's agent: stop its thread to end it";
    pub const KEPT_FREED: &str = "freed; the thread's next send starts a new one";
    pub const ENDING: &str = "ending";
    pub const ENDED: &str = "ended";
    pub const KILL_SENT: &str = "KILL sent";
    pub const LEFT: &str = "still running after a forced end";
    /// `endGraceWord` after `KILL_GRACE_MS`.
    pub const FORCED: &str = "ended by KILL after TERM's 2 s";
    pub const STOPPING: &str = "stopping";
    pub const STOPPED: &str = "stopped";
    pub const NO_TURN: &str = "no turn is running";
    pub const KILL: &str = "kill";
    pub const KILL_FORCE: &str = "kill -9";
    pub const END: &str = "end";
    pub const STOP_TURN: &str = "stop turn";
    pub const STOP_SENTENCE: &str = "This process is the thread's agent. Its turn ends as interrupted; what it started keeps running.";
    pub const CANCEL: &str = "Cancel";
    pub const STOP: &str = "Stop";
    pub const UNREADABLE: &str = "unreadable";
    pub const NONE: &str = "none";
    /// The thread pane's (`ACTIVITY_WORDS`).
    pub const NOTHING: &str = "Nothing running for this thread";
    pub const OPEN_IN_ACTIVITY: &str = "Open in Activity";

    /// The count over the table: the rows on screen, never the machine's total.
    pub fn count(shown: usize, total: Option<i64>) -> String {
        match total {
            Some(total) if (shown as i64) < total => format!("{shown} of {total}"),
            _ => format!("{shown} processes"),
        }
    }

    pub fn confirm(word: &str) -> String {
        format!("confirm {word}")
    }

    pub fn stop_title(title: &str) -> String {
        format!("Stop {title}?")
    }

    /// `killTooOldLine`.
    pub fn too_old(name: &str) -> String {
        format!("TERM went to this process alone: {name} runs a daemon too old to end what it started; wsp add {name} --update updates it")
    }
}

/// Which column orders siblings (`ProcSort`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Cpu,
    Mem,
}

/// `byProcColumn`: heaviest first on the column, ties by pid.
fn by_column(sort: Sort, a: &ProcEntry, b: &ProcEntry) -> std::cmp::Ordering {
    let (x, y) = match sort {
        Sort::Cpu => (a.cpu, b.cpu),
        Sort::Mem => (a.rss, b.rss),
    };
    y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal).then(a.pid.cmp(&b.pid))
}

/// One drawn row and how deep it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Row<'a> {
    pub proc: &'a ProcEntry,
    pub depth: usize,
}

fn hit(p: &ProcEntry, needle: &str) -> bool {
    [p.pid.to_string().as_str(), &p.comm, &p.cmdline, &p.user].iter().any(|s| s.to_lowercase().contains(needle))
}

/// `procTable`: the snapshot nested by parent; under a filter a process stays when it or something under it matches.
pub fn table<'a>(procs: &'a [ProcEntry], sort: Sort, filter: &str) -> Vec<Row<'a>> {
    let needle = filter.trim().to_lowercase();
    let pids: HashSet<Pid> = procs.iter().map(|p| p.pid).collect();
    let mut kids: HashMap<Pid, Vec<&ProcEntry>> = HashMap::new();
    let mut roots = Vec::new();
    for p in procs {
        if p.ppid == p.pid || !pids.contains(&p.ppid) {
            roots.push(p);
        } else {
            kids.entry(p.ppid).or_default().push(p);
        }
    }
    fn mark(p: &ProcEntry, kids: &HashMap<Pid, Vec<&ProcEntry>>, needle: &str, keep: &mut HashSet<Pid>) -> bool {
        // Every child is read, so each that stays is marked for the walk.
        let mut under = false;
        for k in kids.get(&p.pid).into_iter().flatten() {
            under |= mark(k, kids, needle, keep);
        }
        let stays = under || needle.is_empty() || hit(p, needle);
        if stays {
            keep.insert(p.pid);
        }
        stays
    }
    let mut keep = HashSet::new();
    for root in &roots {
        mark(root, &kids, &needle, &mut keep);
    }
    fn walk<'a>(
        list: &[&'a ProcEntry],
        depth: usize,
        sort: Sort,
        kids: &HashMap<Pid, Vec<&'a ProcEntry>>,
        keep: &HashSet<Pid>,
        out: &mut Vec<Row<'a>>,
    ) {
        let mut list = list.to_vec();
        list.sort_by(|a, b| by_column(sort, a, b));
        for proc in list {
            if !keep.contains(&proc.pid) {
                continue;
            }
            out.push(Row { proc, depth });
            walk(kids.get(&proc.pid).map(Vec::as_slice).unwrap_or(&[]), depth + 1, sort, kids, keep, out);
        }
    }
    let mut out = Vec::new();
    walk(&roots, 0, sort, &kids, &keep, &mut out);
    out
}

/// A thread whose agent a row may be, as `ProcThread` names it.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentOf {
    pub title: String,
    /// The running turn's, which its stop names; none while no turn runs.
    pub session: Option<String>,
}

/// `agentRows`: each row that is a running turn's agent, as `placeProcs` reads it, with its thread. A signal to any of
/// them ends the turn as failed, so the pane stops the thread instead.
pub fn agent_rows(procs: &[ProcEntry], threads: &[Thread]) -> HashMap<Pid, AgentOf> {
    let running: Vec<&Thread> = threads.iter().filter(|t| t.working && t.pid.is_some()).collect();
    let placing: Vec<PlaceThread> = running
        .iter()
        .map(|t| PlaceThread {
            id: t.id.clone(),
            roots: t.pid.into_iter().collect(),
            cgroup: None,
            bin: agent_bin(&t.harness).map(str::to_owned),
            agents: t.agent_pid.into_iter().collect(),
        })
        .collect();
    let placed = place_procs(PlaceInput { procs, daemon: -1, host: None, threads: &placing, listens: &|_| false, app: HashMap::new() });
    let of =
        |id: &str| running.iter().find(|t| t.id == id).map(|t| AgentOf { title: t.title.clone(), session: Some(t.session_id.clone()) });
    procs
        .iter()
        .filter_map(|p| {
            let id = placed.agent_of(p.pid).or_else(|| placed.folds_into(p.pid).and_then(|at| placed.agent_of(at)))?;
            Some((p.pid, of(id)?))
        })
        .collect()
}

/// `runAgentPids`: every process from a turn's run script down to the first agent under it, whatever thread it is.
pub fn run_agent_pids(procs: &[ProcEntry]) -> HashSet<Pid> {
    let by_pid: HashMap<Pid, &ProcEntry> = procs.iter().map(|p| (p.pid, p)).collect();
    let is_agent = |p: &ProcEntry| agent_by_bin(p).is_some();
    let mut pids = HashSet::new();
    for p in procs.iter().filter(|p| is_agent(p)) {
        let mut path: Vec<Pid> = Vec::new();
        let mut at = by_pid.get(&p.ppid).copied();
        while let Some(up) = at.filter(|u| !path.contains(&u.pid)) {
            // An agent under another agent below the run is that agent's own child, killable as any tool is.
            if is_agent(up) {
                break;
            }
            path.push(up.pid);
            // The run's shell is a fork of the script and reads as it, so the run is every such process up the chain.
            let parent_runs = by_pid.get(&up.ppid).is_some_and(|q| is_run_script(&q.cmdline));
            if is_run_script(&up.cmdline) && !parent_runs {
                pids.insert(p.pid);
                pids.extend(path.iter().copied());
                break;
            }
            at = by_pid.get(&up.ppid).copied();
        }
    }
    pids
}

/// `procLabel`: the harness pieces a row is named as: the daemon, the app's parts, the shells behind the daemon's
/// ptys by their tab's title, and an agent by the command its catalog entry puts on PATH.
pub fn label(p: &ProcEntry, daemon: Pid, titles: &HashMap<String, String>, app: &HashMap<Pid, AppPart>) -> Option<String> {
    if p.pid == daemon {
        return Some("wsp".into());
    }
    if let Some(part) = app.get(&p.pid) {
        return Some(app_line(part));
    }
    if let Some(pty) = &p.pty {
        return Some(titles.get(pty).map_or_else(|| "terminal".into(), |t| format!("terminal {t}")));
    }
    agent_by_bin(p).map(|_| "agent".into())
}

/// `compactBytes`: rss in a three-digit budget, 900, 12K, 1.5M, 123M, 2.3G.
pub fn compact_bytes(n: f64) -> String {
    const UNITS: [&str; 5] = ["", "K", "M", "G", "T"];
    let (mut v, mut i) = (n.max(0.), 0);
    while v >= 1000. && i < UNITS.len() - 1 {
        v /= 1024.;
        i += 1;
    }
    let text = if i == 0 {
        format!("{}", v.round())
    } else if v < 10. {
        format!("{v:.1}")
    } else {
        format!("{}", v.round())
    };
    format!("{text}{}", UNITS[i])
}

/// `ARM_MS`: a second press within it confirms.
pub const ARM: Duration = Duration::from_millis(2000);
/// `ESCALATE_MS`: how long TERM has on a too-old daemon before KILL is offered.
pub const ESCALATE: Duration = Duration::from_millis(5000);

/// What the confirming press sends: the end the daemon carries out, or KILL to a daemon too old for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Send {
    End,
    Force,
}

/// `KillState`; `Ripe` is a too-old daemon's TERM whose wait is over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kill {
    #[default]
    Idle,
    Armed {
        at: Instant,
        send: Send,
    },
    Sent,
    Termed {
        at: Instant,
    },
    Ripe,
}

/// Whether the button shows, what its press leads to, and whether the next press confirms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub send: Send,
    pub confirm: bool,
}

impl Kill {
    pub fn offered(self, now: Instant) -> Option<Offer> {
        match self {
            Kill::Idle => Some(Offer { send: Send::End, confirm: false }),
            Kill::Armed { at, send } => Some(Offer { send, confirm: now.saturating_duration_since(at) < ARM }),
            Kill::Sent => None,
            Kill::Termed { at } => (now.saturating_duration_since(at) >= ESCALATE).then_some(Offer { send: Send::Force, confirm: false }),
            Kill::Ripe => Some(Offer { send: Send::Force, confirm: false }),
        }
    }

    /// A press: the next state and, on the confirming one, what to send.
    pub fn press(self, now: Instant) -> (Kill, Option<Send>) {
        match self.offered(now) {
            None => (self, None),
            Some(Offer { send, confirm: true }) => (Kill::Sent, Some(send)),
            Some(Offer { send, confirm: false }) => (Kill::Armed { at: now, send }, None),
        }
    }

    /// Time passing: an unconfirmed arm lapses to where it was armed from.
    pub fn settle(self, now: Instant) -> Kill {
        match self {
            Kill::Armed { at, send } if now.saturating_duration_since(at) >= ARM => match send {
                Send::End => Kill::Idle,
                Send::Force => Kill::Ripe,
            },
            Kill::Termed { at } if now.saturating_duration_since(at) >= ESCALATE => Kill::Ripe,
            state => state,
        }
    }

    /// Whether time can move it, so the pane ticks while it stands.
    pub fn ticks(self) -> bool {
        matches!(self, Kill::Armed { .. } | Kill::Termed { .. })
    }
}

/// `killedWord`: what a daemon's answer to a tree kill says on the row.
pub fn killed_word(reply: &serde_json::Value) -> &'static str {
    let some = |key: &str| reply.get(key).and_then(serde_json::Value::as_array).is_some_and(|a| !a.is_empty());
    if some("left") {
        words::LEFT
    } else if some("forced") {
        words::FORCED
    } else {
        words::ENDED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: Pid, ppid: Pid, cmd: &str, cpu: f64, rss: f64) -> ProcEntry {
        ProcEntry { cpu, rss, ..ProcEntry::new(pid, ppid, cmd.split(' ').next().unwrap_or_default(), cmd) }
    }

    #[test]
    fn bytes_read_in_three_digits() {
        assert_eq!(compact_bytes(900.), "900");
        assert_eq!(compact_bytes(12. * 1024.), "12K");
        assert_eq!(compact_bytes(1.5 * 1024. * 1024.), "1.5M");
        assert_eq!(compact_bytes(2.3 * 1024. * 1024. * 1024.), "2.3G");
    }

    #[test]
    fn an_agent_under_its_run_script_is_a_run_agent_and_a_tool_under_the_agent_is_not() {
        let procs = [
            p(10, 1, "bash /tmp/runs/0123456789ab.sh", 0., 0.),
            p(11, 10, "bash /tmp/runs/0123456789ab.sh", 0., 0.),
            p(12, 11, "claude -p", 0., 0.),
            p(13, 12, "node mcp.js", 0., 0.),
            p(20, 1, "claude", 0., 0.),
        ];
        let pids = run_agent_pids(&procs);
        assert_eq!(pids, [10, 11, 12].into_iter().collect(), "the run and its shell fork, the agent, never its tool or a loose agent");
    }

    #[test]
    fn a_kill_arms_lapses_and_confirms_within_two_seconds() {
        let t = Instant::now();
        let (armed, sent) = Kill::Idle.press(t);
        assert_eq!(sent, None);
        assert_eq!(armed.offered(t), Some(Offer { send: Send::End, confirm: true }));
        assert_eq!(armed.settle(t + ARM), Kill::Idle, "an unconfirmed arm lapses");
        assert_eq!(armed.press(t + ARM / 2), (Kill::Sent, Some(Send::End)));
        assert_eq!(Kill::Sent.offered(t), None);
        let termed = Kill::Termed { at: t };
        assert_eq!(termed.offered(t + ESCALATE / 2), None);
        assert_eq!(termed.offered(t + ESCALATE), Some(Offer { send: Send::Force, confirm: false }));
        let (force, _) = termed.press(t + ESCALATE);
        assert_eq!(force.settle(t + ESCALATE + ARM), Kill::Ripe, "a lapsed kill -9 stays on offer");
    }
}
