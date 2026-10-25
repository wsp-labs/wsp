// SPDX-License-Identifier: AGPL-3.0-only
//! Who the kernel takes first on a guest, for memory and for the cpu. Port of the functions of
//! packages/protocol/src/oom.ts at a3b870a6e; its constants are wsp_wire::types::oom. See spec/shell_and_text.md.

use wsp_wire::types::oom::WORK_OOM_SCORE_ADJ;

use crate::js::number_string;

// Who the kernel takes first on a guest, for memory and for the cpu. The
// daemon goes last for the memory killer and first for the scheduler: a build
// or a test run that outgrows the machine loses its own processes, not the link
// the app and the command line reach the machine by, and a load of six on two
// vCPUs still leaves the daemon its heartbeats (one build on a 4 GB workspace
// took the daemon, and the app offered a rebuild of a machine that was fine,
// 2026-09-06). Every command a turn or a terminal runs starts at the work
// scores, so nothing the daemon spawns inherits its standing.
// One above the lowest adjustment Linux takes: the daemon is killed last, never exempt, so a daemon that leaks
// can still be reclaimed and the kernel is never left with nothing to kill. Only a process with CAP_SYS_RESOURCE
// may write below zero (measured: root without it gets EACCES); the guest daemon runs as root.
// oom.ts:14
// export const DAEMON_OOM_SCORE_ADJ = -999;
// in wsp_wire::types::oom: DAEMON_OOM_SCORE_ADJ

// oom.ts:18
// in wsp_wire::types::oom: WORK_OOM_SCORE_ADJ

// The daemon's nice value: ahead of every default-priority process, so a saturated cpu still turns its heartbeats
// around. Only root may go below zero (measured: an unprivileged setPriority(-10) is refused).
// oom.ts:22
// export const DAEMON_NICE = -10;
// in wsp_wire::types::oom: DAEMON_NICE

/// The sh line that puts the running shell, and everything it starts, at the work scores: the memory killer's, and
/// the scheduler's default, which a child of the daemon would otherwise inherit (measured). Both writes are quiet
/// where they cannot apply, so a daemon's tests run on macOS and a shell under a plain-priority daemon is a no-op.
// oom.ts:27
// export function workScoreLine(adj = WORK_OOM_SCORE_ADJ): string {
//   return `{ echo ${adj} > /proc/self/oom_score_adj; } 2>/dev/null; renice 0 $$ >/dev/null 2>&1`;
// }
/// `adj` defaults to WORK_OOM_SCORE_ADJ (500).
pub fn work_score_line(adj: Option<f64>) -> String {
    let adj = number_string(adj.unwrap_or(WORK_OOM_SCORE_ADJ as f64));
    format!("{{ echo {adj} > /proc/self/oom_score_adj; }} 2>/dev/null; renice 0 $$ >/dev/null 2>&1")
}

/// A command as the daemon launches it: behind the work-score line and exec'd into, so the pid stays the command's
/// own and its argv passes through sh untouched.
// oom.ts:33
// export function workArgv(file: string, args: readonly string[]): { file: string; args: string[] } {
//   return { file: "/bin/sh", args: ["-c", `${workScoreLine()}; exec "$0" "$@"`, file, ...args] };
// }
/// A process to spawn: the file and its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argv {
    pub file: String,
    pub args: Vec<String>,
}

pub fn work_argv(file: &str, args: &[impl AsRef<str>]) -> Argv {
    let script = format!("{}; exec \"$0\" \"$@\"", work_score_line(None));
    let mut argv = vec!["-c".to_string(), script, file.to_string()];
    argv.extend(args.iter().map(|a| a.as_ref().to_string()));
    Argv { file: "/bin/sh".to_string(), args: argv }
}

// oom.ts:44
// in wsp_wire::types::oom: DAEMON_MEMORY_MAX_PERCENT
