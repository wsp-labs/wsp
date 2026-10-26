// SPDX-License-Identifier: AGPL-3.0-only
//! The rules both turn roads share: the cut, the idle clock, when a tree is worth reading, the guest's awk programs
//! and the log's line cutter. Port of packages/runtime/src/machine-exec.ts:95-298 at 65f6b89c7. Spec:
//! spec/machine_exec_rules.md.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use indexmap::IndexMap;
use wsp_adapters::port::{ExecFailure, ExecFailureKind};
use wsp_wire::wire::limits::{EXEC_LINE_MAX_BYTES, TURN_WORK_TICKS_PER_S};
use wsp_words::words::thread::{turn_cut_line, TurnCutRule};

const LINE_MAX: usize = EXEC_LINE_MAX_BYTES as usize;

/// The two limits a turn runs under on every kind of machine. `f64::INFINITY` is no limit (the exec verb's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnLimits {
    pub idle_ms: f64,
    pub deadline_ms: f64,
}

/// The one rule that cuts a turn: the wall since it started, else the idle stretch; `None` while neither passed. The
/// failure's message is `turn_cut_line`, its kind `Other`.
// TS machine-exec.ts:103-108
// /** The one rule that cuts a turn, whatever launched it: the error carrying turnCutLine when a limit has passed, else
//  * nothing. The cloud road and the local child both read it, so a hung agent ends with the same words on either. */
// export function turnCut(limits: TurnLimits, elapsedMs: number, quietMs: number): Error | undefined {
//   const rule: TurnCutRule | undefined = elapsedMs >= limits.deadlineMs ? "wall" : quietMs >= limits.idleMs ? "idle" : undefined;
//   return rule === undefined ? undefined : new Error(turnCutLine(rule, elapsedMs, rule === "wall" ? limits.deadlineMs : limits.idleMs));
// }
pub fn turn_cut(limits: TurnLimits, elapsed_ms: i64, quiet_ms: i64) -> Option<ExecFailure> {
    let elapsed = elapsed_ms as f64;
    let (rule, limit) = if elapsed >= limits.deadline_ms {
        (TurnCutRule::Wall, limits.deadline_ms)
    } else if quiet_ms as f64 >= limits.idle_ms {
        (TurnCutRule::Idle, limits.idle_ms)
    } else {
        return None;
    };
    Some(ExecFailure { kind: ExecFailureKind::Other, message: turn_cut_line(rule, elapsed, limit) })
}

/// One reading of a turn's tree and the clock it was taken on; only the difference between two means anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkReading {
    pub ticks: i64,
    pub at: i64,
}

/// The clock the idle limit is read against: bytes and messages move it, and so does tree work past
/// `TURN_WORK_TICKS_PER_S` since the clock last moved.
// TS machine-exec.ts:117-132
// /** The clock the idle limit is read against, one shape for every road: the harness's bytes and the person's messages
//  * move it, and so does work in the process tree the turn started once that work clears TURN_WORK_TICKS_PER_S since
//  * the clock last moved. A harness sitting in a long tool call prints nothing while its children burn a core, and
//  * reading bytes alone ended two builds that were still working (43 and 38 minutes, 2026-09-08). */
// export interface TurnActivity {
//   /** How long the turn has done nothing: what the idle limit is compared with. */
//   quietMs(nowMs: number): number;
//   /** A byte from the harness, or a message the person sent into the turn. */
//   touch(atMs: number): void;
//   /** A stretch nothing could be read through: the road to the machine was dark, so it is no part of the turn's
//    * quiet and the clock goes on from where it stood. */
//   hold(ms: number): void;
//   /** One reading of the turn's tree. The first, and any that undercuts the one before it, is only what the next is
//    * measured from: a tree whose members exited carries less work than it did and is no baseline. */
//   read(ticks: number, atMs: number): void;
// }
#[derive(Clone, Debug, PartialEq)]
pub struct TurnActivity {
    active_at: i64,
    base: Option<WorkReading>,
}

impl TurnActivity {
    /// How long the turn has done nothing: what the idle limit is compared with.
    pub fn quiet_ms(&self, now_ms: i64) -> i64 {
        now_ms - self.active_at
    }
    /// A byte from the harness or a message the person sent; sets the clock to `at_ms` even when earlier.
    pub fn touch(&mut self, at_ms: i64) {
        self.active_at = at_ms;
    }
    /// A stretch the road was dark: no part of the quiet, the clock goes on from where it stood.
    pub fn hold(&mut self, ms: i64) {
        self.active_at += ms;
    }
    /// One reading of the tree; the first, and any below the base, only becomes the base.
    pub fn read(&mut self, ticks: i64, at_ms: i64) {
        let base = match self.base {
            Some(base) if ticks >= base.ticks => base,
            _ => {
                self.base = Some(WorkReading { ticks, at: at_ms });
                return;
            }
        };
        let floor = (TURN_WORK_TICKS_PER_S as f64 * (at_ms - base.at) as f64 / 1_000.0).max(1.0);
        if ((ticks - base.ticks) as f64) < floor {
            return;
        }
        self.active_at = at_ms;
        self.base = Some(WorkReading { ticks, at: at_ms });
    }
}

// TS machine-exec.ts:134-155
// export function turnActivity(startedAt: number): TurnActivity {
//   let activeAt = startedAt;
//   let base: WorkReading | undefined;
//   return {
//     quietMs: nowMs => nowMs - activeAt,
//     touch: atMs => {
//       activeAt = atMs;
//     },
//     hold: ms => {
//       activeAt += ms;
//     },
//     read: (ticks, atMs) => {
//       if (base === undefined || ticks < base.ticks) {
//         base = { ticks, at: atMs };
//         return;
//       }
//       if (ticks - base.ticks < Math.max(1, (TURN_WORK_TICKS_PER_S * (atMs - base.at)) / 1_000)) return;
//       activeAt = atMs;
//       base = { ticks, at: atMs };
//     },
//   };
// }
pub fn turn_activity(started_at: i64) -> TurnActivity {
    TurnActivity { active_at: started_at, base: None }
}

/// Whether the tree is worth reading now: a finite idle limit and half of it quiet.
// TS machine-exec.ts:157-163
// /** Whether the turn's tree is worth reading at this moment: there is an idle limit for a reading to hold open, and
//  * the stream has been quiet long enough that the cut is in sight. A turn that is printing is already alive on its
//  * bytes, and a stream run with no idle limit (the exec verb hands both limits infinite on purpose) would pay for a
//  * reading every second for the life of a build and could not be cut by any answer it got. */
// export function readsWork(idleMs: number, quietMs: number): boolean {
//   return Number.isFinite(idleMs) && quietMs >= idleMs / 2;
// }
pub fn reads_work(idle_ms: f64, quiet_ms: i64) -> bool {
    idle_ms.is_finite() && quiet_ms as f64 >= idle_ms / 2.0
}

/// The guest's reading of a run's tree, given the run's leader in `p`; the TS text byte for byte.
// TS machine-exec.ts:165-181
// /** What the guest counts a run's work by, given the pid of the run's script in `p`, which also leads the run's
//  * process group: every process whose parent chain reaches that pid or a member of that group, each counted once
//  * for the CPU ticks it and the children it has reaped have burned and the megabytes it has read or written,
//  * summed into one number the next poll is compared with. The chain matters because Claude Code starts every Bash
//  * tool command, foreground or background, in a session and group of its own, so a build an agent runs is never in
//  * the run's group; the group keeps the members whose parent went, the run's input pump among them. The reaped
//  * children's ticks (cutime, cstime) keep the sum from falling each time a compiler exits, since a reading below the
//  * last is only a new baseline; the kernel folds a reaped child's bytes into its parent's io already. /proc is the
//  * only road to it, since the guests have no pgrep, and the comm field is stripped by its last bracket because a
//  * process may be named `(sh (2))`. Each file is opened by getline rather than handed to awk as input: every awk
//  * aborts on an input file it cannot open, so one process exiting between the shell's glob and the read would void
//  * the whole reading. A guest with no /proc reads 0 on every poll, and one with no awk reads nothing; either leaves
//  * the turn's idle clock on bytes alone. */
// export const TREE_WORK_AWK =
//   "BEGIN{for(k=1;k<ARGC;k++){if((getline s<ARGV[k])>0){n=s+0;sub(/^[0-9]+ \\(.*\\) /,\"\",s);split(s,f,\" \");up[n]=f[2];g[n]=f[3];c[n]=f[12]+f[13]+f[14]+f[15];at[n]=ARGV[k]}close(ARGV[k])}" +
//   "for(n in up){q=n;for(d=0;d<64;d++){if(q==p||((q in g)&&g[q]==p)){t+=c[n];i=at[n];sub(/stat$/,\"io\",i);while((getline l<i)>0)if(l~/^[rw]char:/){split(l,h,\" \");t+=int(h[2]/1048576)}close(i);break}if(!(q in up))break;q=up[q]}}" +
//   "print t+0}";
pub const TREE_WORK_AWK: &str = r#"BEGIN{for(k=1;k<ARGC;k++){if((getline s<ARGV[k])>0){n=s+0;sub(/^[0-9]+ \(.*\) /,"",s);split(s,f," ");up[n]=f[2];g[n]=f[3];c[n]=f[12]+f[13]+f[14]+f[15];at[n]=ARGV[k]}close(ARGV[k])}for(n in up){q=n;for(d=0;d<64;d++){if(q==p||((q in g)&&g[q]==p)){t+=c[n];i=at[n];sub(/stat$/,"io",i);while((getline l<i)>0)if(l~/^[rw]char:/){split(l,h," ");t+=int(h[2]/1048576)}close(i);break}if(!(q in up))break;q=up[q]}}print t+0}"#;

/// What a kept turn left in its run's group, off `/proc/<pid>/stat` lines on stdin, given the group in `p` and the
/// input pump's pid in `t`; the TS text byte for byte.
// TS machine-exec.ts:183-189
// /** What a turn on a kept process left running in its run's group, off one /proc/[pid]/stat line per process on stdin,
//  * given the group in `p` and the input pump's pid in `t`: every process whose parent is outside the group, the leader
//  * and the pump aside, and everything under each. The agent and the servers it started have their parents in the
//  * group, so they stay. The lines come from the shell, since gawk ends at a stat file whose process went after the glob. */
// export const LEFTOVERS_AWK =
//   '{s=$0;sub(/^[0-9]+ \\(.*\\) /,"",s);split(s,f," ");if(f[3]==p)up[$1]=f[2]}' +
//   'END{for(x in up)if(!(up[x] in up)&&x!=p&&x!=t)l[x]=1;do{g=0;for(x in up)if((up[x] in l)&&!(x in l)){l[x]=1;g=1}}while(g);for(x in l)printf "%s ",x}';
pub const LEFTOVERS_AWK: &str = r#"{s=$0;sub(/^[0-9]+ \(.*\) /,"",s);split(s,f," ");if(f[3]==p)up[$1]=f[2]}END{for(x in up)if(!(up[x] in up)&&x!=p&&x!=t)l[x]=1;do{g=0;for(x in up)if((up[x] in l)&&!(x in l)){l[x]=1;g=1}}while(g);for(x in l)printf "%s ",x}"#;

/// Whether the run is stopped on a question only a person can answer; the idle clock does not run while it is.
pub type TurnWaiting = Arc<dyn Fn() -> bool>;

/// A run's log as the lines it ends, both roads' reader: a line past `EXEC_LINE_MAX_BYTES` is handed on in pieces
/// that each end where a character starts.
// TS machine-exec.ts:266-298
// /** A run's log as the lines it ends, both roads' reader: a line past EXEC_LINE_MAX_BYTES is handed on in pieces of
//  * it, so what waits for a newline is at most the cap between chunks. A piece ends where a character starts. */
// export class LineCut {
//   private pending: Buffer = Buffer.alloc(0);
//
//   *take(chunk: Buffer): Generator<string> {
//     let seen = this.pending.length;
//     this.pending = seen === 0 ? chunk : Buffer.concat([this.pending, chunk]);
//     while (true) {
//       const nl = this.pending.indexOf(0x0a, seen);
//       const line = this.pending;
//       if ((nl === -1 ? line.length : nl) > EXEC_LINE_MAX_BYTES) {
//         let at = EXEC_LINE_MAX_BYTES;
//         for (let back = 0; back < 3 && (line[at]! & 0xc0) === 0x80; back++) at--;
//         seen = (nl === -1 ? line.length : nl) - at;
//         this.pending = line.subarray(at);
//         yield line.subarray(0, at).toString("utf8");
//       } else if (nl !== -1) {
//         // An empty view would keep the whole buffer the line was joined in while the run is quiet.
//         this.pending = nl + 1 === line.length ? Buffer.alloc(0) : line.subarray(nl + 1);
//         seen = 0;
//         yield line.subarray(0, nl).toString("utf8");
//       } else return;
//     }
//   }
//
//   /** The tail no newline ended, let go of as it is handed over. */
//   rest(): string | undefined {
//     const last = this.pending;
//     this.pending = Buffer.alloc(0);
//     return last.length > 0 ? last.toString("utf8") : undefined;
//   }
// }
#[derive(Debug, Default)]
pub struct LineCut {
    pending: Vec<u8>,
}

impl LineCut {
    pub fn new() -> Self {
        LineCut { pending: Vec::new() }
    }
    /// Every complete line the buffered bytes now hold, decoded lossily as node's `Buffer.toString("utf8")` does.
    pub fn take(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut seen = self.pending.len();
        self.pending.extend_from_slice(chunk);
        let mut lines = Vec::new();
        let mut from = 0;
        loop {
            let nl = self.pending[seen..].iter().position(|&b| b == b'\n').map(|i| seen + i);
            let end = nl.unwrap_or(self.pending.len());
            if end - from > LINE_MAX {
                let mut at = from + LINE_MAX;
                for _ in 0..3 {
                    if self.pending[at] & 0xc0 != 0x80 {
                        break;
                    }
                    at -= 1;
                }
                lines.push(String::from_utf8_lossy(&self.pending[from..at]).into_owned());
                from = at;
                seen = end;
            } else if let Some(nl) = nl {
                lines.push(String::from_utf8_lossy(&self.pending[from..nl]).into_owned());
                from = nl + 1;
                seen = from;
            } else {
                break;
            }
        }
        self.pending.drain(..from);
        lines
    }
    /// The tail no newline ended, let go of as it is handed over; `None` when nothing is pending.
    pub fn rest(&mut self) -> Option<String> {
        let last = std::mem::take(&mut self.pending);
        (!last.is_empty()).then(|| String::from_utf8_lossy(&last).into_owned())
    }
}

/// Every run a process is reading, each as the call that stops reading it (the TS `reading: Set<() => void>`). The
/// wiring that made the factory owns it and lets go of every run when it closes.
#[derive(Default)]
pub struct ReadingSet {
    drops: Mutex<IndexMap<u64, Arc<dyn Fn()>>>,
    next: AtomicU64,
}

impl ReadingSet {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    fn locked(&self) -> MutexGuard<'_, IndexMap<u64, Arc<dyn Fn()>>> {
        self.drops.lock().unwrap_or_else(PoisonError::into_inner)
    }
    /// Registers one reader's let-go; answers the key `remove` takes.
    pub fn add(&self, drop: Arc<dyn Fn()>) -> u64 {
        let key = self.next.fetch_add(1, Ordering::Relaxed);
        self.locked().insert(key, drop);
        key
    }
    pub fn remove(&self, key: u64) {
        self.locked().shift_remove(&key);
    }
    pub fn len(&self) -> usize {
        self.locked().len()
    }
    pub fn is_empty(&self) -> bool {
        self.locked().is_empty()
    }
    /// Calls every registered let-go, in the order they were added, off a copy of the set taken first (the TS
    /// `for (const drop of [...reading]) drop()`); the lock is never held while one runs.
    pub fn let_go_all(&self) {
        let drops: Vec<Arc<dyn Fn()>> = self.locked().values().cloned().collect();
        for drop in drops {
            drop();
        }
    }
}
