// SPDX-License-Identifier: AGPL-3.0-only
//! One thread's processes on a joined computer stand in a cgroup of their own. Port of the sentence and script
//! functions of packages/protocol/src/thread-cgroup.ts at the cut (383708c78) (cgroupMemory and inCgroup are
//! wsp_wire::types::thread_cgroup). See spec/shell_and_text.md.

use wsp_wire::types::daemon_contract::{CGROUP_MOUNT, THREAD_CGROUPS};
use wsp_wire::types::shell_quote::shell_quote;

use crate::js::utf16;

/// The cgroup a thread's processes stand in, as /proc/[pid]/cgroup names it and a process row carries it.
// thread-cgroup.ts:12
// export function threadCgroup(threadId: string): string {
//   return `${THREADS}/${threadId.replace(/[^A-Za-z0-9_-]/g, "_")}`;
// }
pub fn thread_cgroup(thread_id: &str) -> String {
    let mut name = String::with_capacity(thread_id.len());
    for c in thread_id.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            name.push(c);
        } else {
            name.extend(std::iter::repeat_n('_', c.len_utf16()));
        }
    }
    format!("{THREAD_CGROUPS}/{name}")
}

// A box thread's memory: its cgroup's anon and kernel bytes as the daemon read them, which count what its processes
// hold once however many share it and leave out the page cache they read, where its rows' resident figures would count
// each shared library again in every process. Undefined where the daemon read none (a daemon a version behind, a cgroup
// made before its folder turned the memory controller on), and the rows' own figures stand.
// thread-cgroup.ts:20
// export function cgroupMemory(cgroups: readonly CgroupMemory[] | undefined, cgroup: string | undefined): number | undefined {
//   return cgroup === undefined ? undefined : cgroups?.find(c => c.cgroup === cgroup)?.memory;
// }
// cgroupMemory: in wsp_wire::types::thread_cgroup

// Whether a process row's cgroup is this one or one under it.
// thread-cgroup.ts:25
// export function inCgroup(at: string | undefined, cgroup: string): boolean {
//   return at !== undefined && (at === cgroup || at.startsWith(`${cgroup}/`));
// }
// inCgroup: in wsp_wire::types::thread_cgroup

/// The line a launch runs as root before it hands its command to the login: the shell stands itself in the thread's
/// cgroup, so what it starts stands there too. The folder above it turns on the memory and pids controllers for its
/// children, which is what gives each thread's cgroup `memory.stat` and `pids.current`; that folder holds no process
/// of its own, as the kernel asks of one that does. A computer with no cgroup v2, or one that will not take the folder
/// or the controllers, launches the same command where it stands, since the cgroup tracks and never gates.
// thread-cgroup.ts:34
// export function cgroupJoinLine(cgroup: string): string {
//   const at = shellQuote(`${CGROUP_MOUNT}${cgroup}`);
//   const above = shellQuote(`${CGROUP_MOUNT}${cgroup.slice(0, cgroup.lastIndexOf("/"))}`);
//   return `[ -f ${CGROUP_MOUNT}/cgroup.controllers ] && mkdir -p ${at} 2>/dev/null && { echo '+memory +pids' > ${above}/cgroup.subtree_control; echo $$ > ${at}/cgroup.procs; } 2>/dev/null; true`;
// }
pub fn cgroup_join_line(cgroup: &str) -> String {
    let at = shell_quote(&format!("{CGROUP_MOUNT}{cgroup}"));
    let slash = utf16::last_index_of(cgroup, "/").map_or(-1, |i| i as i64);
    let above = shell_quote(&format!("{CGROUP_MOUNT}{}", utf16::slice(cgroup, 0, Some(slash))));
    format!("[ -f {CGROUP_MOUNT}/cgroup.controllers ] && mkdir -p {at} 2>/dev/null && {{ echo '+memory +pids' > {above}/cgroup.subtree_control; echo $$ > {at}/cgroup.procs; }} 2>/dev/null; true")
}

// The shell function that ends one thread's cgroup, `wsp_end <cgroup> <remove>`, as root: what stands in it and every
// cgroup under it when it starts, by pid, TERM, then KILL what is still there after the grace. A process that joins
// meanwhile is the next turn's and is left be. With remove 1 the thread is going, so the cgroups are emptied whole and
// taken away, deepest first. Returns non-zero naming what survived; a cgroup that is not there is nothing to end.
// thread-cgroup.ts:48
// private in the TS: its value, copied by script from the TS:
const END_FUNCTION: &str = "wsp_end() {\n  C=$1; at=\"/sys/fs/cgroup$1\"\n  [ -d \"$at\" ] || return 0\n  mine() { g=$(sed -n 's/^0:://p' /proc/$1/cgroup 2>/dev/null); case \"$g\" in \"$C\"|\"$C\"/*) ;; *) return 1 ;; esac; s=$(sed 's/.*) //' /proc/$1/stat 2>/dev/null); [ -n \"$s\" ] && [ \"${s%% *}\" != Z ]; }\n  left() { L=; for p in $P; do mine \"$p\" && L=\"$L $p\"; done; P=$L; }\n  P=$(find \"$at\" -name cgroup.procs -exec cat {} + 2>/dev/null)\n  [ -n \"$P\" ] && kill -TERM $P 2>/dev/null\n  i=0; left; while [ $i -lt 30 ] && [ -n \"$P\" ]; do sleep 0.1; i=$((i+1)); left; done\n  [ -n \"$P\" ] && kill -KILL $P 2>/dev/null\n  i=0; left; while [ $i -lt 10 ] && [ -n \"$P\" ]; do sleep 0.1; i=$((i+1)); left; done\n  [ -z \"$P\" ] || { echo \"processes$P of $C did not end\" >&2; return 1; }\n  [ \"$2\" = 1 ] || return 0\n  echo 1 > \"$at/cgroup.kill\" 2>/dev/null || { Q=$(find \"$at\" -name cgroup.procs -exec cat {} + 2>/dev/null); [ -n \"$Q\" ] && kill -KILL $Q 2>/dev/null; }\n  i=0; while [ $i -lt 30 ] && [ -d \"$at\" ]; do find \"$at\" -depth -type d -exec rmdir {} + 2>/dev/null; [ -d \"$at\" ] && sleep 0.1; i=$((i+1)); done\n  [ -d \"$at\" ] && { echo \"$C still holds processes\" >&2; return 1; }\n  return 0\n}";
// const END_FUNCTION = [
//   "wsp_end() {",
//   `  C=$1; at="${CGROUP_MOUNT}$1"`,
//   '  [ -d "$at" ] || return 0',
//   // Still running and still the thread's: a pid the kernel handed to another process since is neither.
//   `  mine() { g=$(sed -n 's/^0:://p' /proc/$1/cgroup 2>/dev/null); case "$g" in "$C"|"$C"/*) ;; *) return 1 ;; esac; s=$(sed 's/.*) //' /proc/$1/stat 2>/dev/null); [ -n "$s" ] && [ "\${s%% *}" != Z ]; }`,
//   '  left() { L=; for p in $P; do mine "$p" && L="$L $p"; done; P=$L; }',
//   '  P=$(find "$at" -name cgroup.procs -exec cat {} + 2>/dev/null)',
//   '  [ -n "$P" ] && kill -TERM $P 2>/dev/null',
//   `  i=0; left; while [ $i -lt ${GRACE_TENTHS} ] && [ -n "$P" ]; do sleep 0.1; i=$((i+1)); left; done`,
//   '  [ -n "$P" ] && kill -KILL $P 2>/dev/null',
//   `  i=0; left; while [ $i -lt ${KILLED_TENTHS} ] && [ -n "$P" ]; do sleep 0.1; i=$((i+1)); left; done`,
//   '  [ -z "$P" ] || { echo "processes$P of $C did not end" >&2; return 1; }',
//   '  [ "$2" = 1 ] || return 0',
//   '  echo 1 > "$at/cgroup.kill" 2>/dev/null || { Q=$(find "$at" -name cgroup.procs -exec cat {} + 2>/dev/null); [ -n "$Q" ] && kill -KILL $Q 2>/dev/null; }',
//   `  i=0; while [ $i -lt ${GRACE_TENTHS} ] && [ -d "$at" ]; do find "$at" -depth -type d -exec rmdir {} + 2>/dev/null; [ -d "$at" ] && sleep 0.1; i=$((i+1)); done`,
//   '  [ -d "$at" ] && { echo "$C still holds processes" >&2; return 1; }',
//   "  return 0",
//   "}",
// ].join("\n");

/// Ends one thread's cgroup as root, and with `remove` takes it away (END_FUNCTION).
// thread-cgroup.ts:70
// export function cgroupEndScript(cgroup: string, o: { remove?: boolean } = {}): string {
//   return `${END_FUNCTION}\nwsp_end ${shellQuote(cgroup)} ${o.remove === true ? 1 : 0}`;
// }
/// `remove` is the TS `o.remove === true`.
pub fn cgroup_end_script(cgroup: &str, remove: bool) -> String {
    format!("{END_FUNCTION}\nwsp_end {} {}", shell_quote(cgroup), u8::from(remove))
}

/// Ends and takes away every thread's cgroup on the computer, as a leave does, printing each one it took. `under` is
/// the folder they sit in, which only a test names.
// thread-cgroup.ts:76
// export function threadCgroupsEndScript(under: string = THREADS): string {
//   const at = shellQuote(`${CGROUP_MOUNT}${under}`);
//   return [
//     END_FUNCTION,
//     `[ -d ${at} ] || exit 0`,
//     `for d in ${at}/*/; do [ -d "$d" ] || continue; t=$(basename "$d"); wsp_end ${shellQuote(under)}/"$t" 1 && echo ${shellQuote(`${CGROUP_MOUNT}${under}`)}/"$t"; done`,
//     `rmdir ${at} 2>/dev/null`,
//     "exit 0",
//   ].join("\n");
// }
/// `under` defaults to THREAD_CGROUPS.
pub fn thread_cgroups_end_script(under: Option<&str>) -> String {
    let under = under.unwrap_or(THREAD_CGROUPS);
    let at = shell_quote(&format!("{CGROUP_MOUNT}{under}"));
    let quoted_under = shell_quote(under);
    [
        END_FUNCTION.to_string(),
        format!("[ -d {at} ] || exit 0"),
        format!("for d in {at}/*/; do [ -d \"$d\" ] || continue; t=$(basename \"$d\"); wsp_end {quoted_under}/\"$t\" 1 && echo {at}/\"$t\"; done"),
        format!("rmdir {at} 2>/dev/null"),
        "exit 0".to_string(),
    ]
    .join("\n")
}
