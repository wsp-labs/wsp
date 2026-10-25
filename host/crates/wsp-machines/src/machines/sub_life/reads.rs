// SPDX-License-Identifier: AGPL-3.0-only
//! sub-life.ts:1-154: the constants, the result read's command and parse, the session's pack and land commands, the
//! row a sub-machine shows and the day its disk is kept until. Spec: spec/sub_life_reads.md.
use wsp_wire::views::sub_row::{SubResult, SubRow};

use super::{BackendFor, SessionFile};
use wsp_core::types::wiring::WorkspaceRecord;

/// sub-life.ts:16. The host blobs a sub-thread's kept session goes under, by thread id, gzipped.
pub const SUB_SESSIONS: &str = "sub-sessions";
/// sub-life.ts:18. A session past this packed is not kept.
pub const SUB_SESSION_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// sub-life.ts:21. The provider's own stop on a done machine whose nap did not land, this long past its delete.
pub const SUB_STOP_AFTER_DELETE_MS: f64 = 600_000.0;
/// sub-life.ts:23. How long a delete the cloud refused waits before it is asked again.
pub const SUB_DELETE_RETRY_MS: f64 = 600_000.0;
/// sub-life.ts:25. The longest a lead's line waits on the done read after its sub-thread's turn.
pub const SUB_LINE_WAIT_MS: f64 = 300_000.0;
/// sub-life.ts:27-28. How far back the done looks for the agent's last push.
pub(crate) const PUSH_READ_EVENTS: usize = 400;
pub(crate) const PUSH_READ_BYTES: usize = 2_000_000;
/// sub-life.ts:30. The row's reason for the nap a done takes.
pub(crate) const DONE_NAP: &str = "its sub-thread is done";

// export function subResultCommand(folder: string, branch: string, baseSha: string): string {
//   return [
//     `cd ${shellQuote(folder)} 2>/dev/null && git rev-parse --is-inside-work-tree >/dev/null 2>&1 || { echo norepo; exit 0; }`,
//     `echo "branch $(git rev-parse --abbrev-ref HEAD 2>/dev/null)"`,
//     `p=$(git rev-parse --verify -q ${shellQuote(`refs/remotes/origin/${branch}`)} || echo none); echo "pushed $p"`,
//     `b=$(git rev-parse --verify -q ${shellQuote(`${baseSha}^{commit}`)}); echo "unpushed $(git rev-list --count --branches --not --remotes $b)"`,
//     `echo "changed $(git status --porcelain=v1 | wc -l | tr -d ' ')"`,
//     `[ "$p" = none ] || echo "commits $(git rev-list --count ${shellQuote(baseSha)}..$p 2>/dev/null || echo 0)"`,
//   ].join("\n");
// }
/// sub-life.ts:36. One command in the sub-thread's folder, off local refs alone (rule 2).
pub fn sub_result_command(folder: &str, branch: &str, base_sha: &str) -> String {
    todo!()
}

// export function subResultOf(stdout: string, branch: string, readAt: string): SubResult | undefined {
//   const said = new Map<string, string>();
//   for (const line of stdout.split("\n").map(l => l.trim())) {
//     if (line === "") continue;
//     const at = line.indexOf(" ");
//     said.set(at < 0 ? line : line.slice(0, at), at < 0 ? "" : line.slice(at + 1).trim());
//   }
//   if (said.has("norepo")) return { branch, pushedSha: null, commits: 0, unpushed: 0, changed: 0, norepo: true, readAt };
//   const count = (key: string): number | undefined => {
//     const n = Number(said.get(key));
//     return said.get(key) !== "" && Number.isInteger(n) && n >= 0 ? n : undefined;
//   };
//   const unpushed = count("unpushed");
//   const changed = count("changed");
//   if (unpushed === undefined || changed === undefined) return undefined;
//   const pushed = said.get("pushed");
//   const onBranch = said.get("branch");
//   return { branch, pushedSha: pushed === undefined || pushed === "" || pushed === "none" ? null : pushed, commits: count("commits") ?? 0, unpushed, changed, ...(onBranch !== undefined && onBranch !== "" ? { onBranch } : {}), readAt };
// }
/// sub-life.ts:49. The result command's answer read into a result; `None` where it carries no counts (rules 3-7).
pub fn sub_result_of(stdout: &str, branch: &str, read_at: &str) -> Option<SubResult> {
    todo!()
}

/// JavaScript's `Number(s)` on a string (rule 5): `js_trim`, `""` is 0, `0x`/`0o`/`0b` integers, decimal with an
/// exponent, `Infinity` with a sign, anything else NaN.
pub(crate) fn js_number(s: &str) -> f64 {
    todo!()
}

// export function subSessionPackCommand(file: { folder: string; name: string; depth: number }, to: string): string {
//   return [
//     `f=$(find ${shellQuote(file.folder)} -maxdepth ${file.depth + 1} -type f -name ${shellQuote(`*${file.name}`)} 2>/dev/null | head -n 1)`,
//     `[ -n "$f" ] || { echo nofile; exit 0; }`,
//     `echo "path $f"`,
//     `gzip -c "$f" > ${shellQuote(to)} && chmod 0644 ${shellQuote(to)} && echo "size $(stat -c %s ${shellQuote(to)})"`,
//   ].join("\n");
// }
/// sub-life.ts:79. Finds a session's file under its folder and packs it where the cloud's file read reaches.
pub fn sub_session_pack_command(file: &SessionFile, to: &str) -> String {
    todo!()
}

// export function subSessionLandCommand(file: { folder: string; name: string; depth: number }, path: string, from: string): string {
//   const dir = path.slice(0, path.lastIndexOf("/"));
//   return [
//     `install -d -m 0700 ${shellQuote(dir)}`,
//     `find ${shellQuote(file.folder)} -maxdepth ${file.depth + 1} -type f -name ${shellQuote(`*${file.name}`)} ! -path ${shellQuote(path)} -delete 2>/dev/null`,
//     `gunzip -c ${shellQuote(from)} > ${shellQuote(path)}; s=$?; rm -f ${shellQuote(from)}; exit $s`,
//   ].join("\n");
// }
/// sub-life.ts:90. Lands a kept session at the path it was kept from; the folder is `path` up to its last `/` (a path
/// with none loses its last UTF-16 unit, as `slice(0, -1)` does).
pub fn sub_session_land_command(file: &SessionFile, path: &str, from: &str) -> String {
    todo!()
}

// export function subRowOf(record: WorkspaceRecord): SubRow | undefined { ... sub-life.ts:100-130 }
/// sub-life.ts:100. A sub-machine's record as its thread's row reads it (rules 9-10).
pub fn sub_row_of(record: &WorkspaceRecord) -> Option<SubRow> {
    todo!()
}

// export function diskKeptMs(ctx: Pick<RuntimeContext, "backendFor">, record: WorkspaceRecord): number | undefined {
//   try {
//     return ctx.backendFor(record).lifecycle?.diskKeptMs;
//   } catch {
//     return undefined;
//   }
// }
/// sub-life.ts:140. How long a record's cloud keeps a paused machine's disk (rule 11).
pub fn disk_kept_ms(ctx: &dyn BackendFor, record: &WorkspaceRecord) -> Option<f64> {
    todo!()
}

// export function diskKeptUntil(ctx: Pick<RuntimeContext, "backendFor">, record: WorkspaceRecord, stoppedAt?: number): number | undefined {
//   const from = record.phase === "napping" && record.nappedAt !== undefined ? record.nappedAt : stoppedAt;
//   const ms = from === undefined ? undefined : diskKeptMs(ctx, record);
//   return ms === undefined ? undefined : from! + ms;
// }
/// sub-life.ts:150. The day from which a record's cloud may delete its machine's disk (rule 12).
pub fn disk_kept_until(ctx: &dyn BackendFor, record: &WorkspaceRecord, stopped_at: Option<f64>) -> Option<f64> {
    todo!()
}
