// SPDX-License-Identifier: AGPL-3.0-only
//! sub-life.ts:1-154: the constants, the result read's command and parse, the session's pack and land commands, the
//! row a sub-machine shows and the day its disk is kept until. Spec: spec/sub_life_reads.md.
use indexmap::IndexMap;

use wsp_frames::True;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::sub_row::{sub_spawning, SubHeld, SubResult, SubRow, SubRowSize, SubState};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_words::js::js_trim;
use wsp_words::js::utf16::slice;
use wsp_words::sub_thread::{sub_session_not_kept_line, sub_session_too_large_line};
use wsp_words::words::units::fmt_bytes;

use super::{BackendFor, SessionFile};
use wsp_core::types::wiring::{SubSession, WorkspaceRecord};

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
    [
        format!("cd {} 2>/dev/null && git rev-parse --is-inside-work-tree >/dev/null 2>&1 || {{ echo norepo; exit 0; }}", shell_quote(folder)),
        "echo \"branch $(git rev-parse --abbrev-ref HEAD 2>/dev/null)\"".to_string(),
        format!("p=$(git rev-parse --verify -q {} || echo none); echo \"pushed $p\"", shell_quote(&format!("refs/remotes/origin/{branch}"))),
        format!("b=$(git rev-parse --verify -q {}); echo \"unpushed $(git rev-list --count --branches --not --remotes $b)\"", shell_quote(&format!("{base_sha}^{{commit}}"))),
        "echo \"changed $(git status --porcelain=v1 | wc -l | tr -d ' ')\"".to_string(),
        format!("[ \"$p\" = none ] || echo \"commits $(git rev-list --count {}..$p 2>/dev/null || echo 0)\"", shell_quote(base_sha)),
    ]
    .join("\n")
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
    let mut said: IndexMap<String, String> = IndexMap::new();
    for line in stdout.split('\n').map(js_trim).filter(|line| !line.is_empty()) {
        match line.find(' ') {
            None => said.insert(line.to_string(), String::new()),
            Some(at) => said.insert(line[..at].to_string(), js_trim(&line[at + 1..]).to_string()),
        };
    }
    let norepo = || SubResult {
        branch: branch.to_string(),
        pushed_sha: None,
        commits: JsInt::from(0),
        unpushed: JsInt::from(0),
        changed: JsInt::from(0),
        on_branch: None,
        norepo: Some(True),
        read_failed: None,
        read_at: read_at.to_string(),
        push_refused: None,
    };
    if said.contains_key("norepo") {
        return Some(norepo());
    }
    let count = |key: &str| -> Option<f64> {
        let value = said.get(key)?;
        let n = js_number(value);
        (!value.is_empty() && n.fract() == 0.0 && n >= 0.0).then_some(n + 0.0)
    };
    let unpushed = count("unpushed")?;
    let changed = count("changed")?;
    let pushed_sha = said.get("pushed").filter(|sha| !sha.is_empty() && sha.as_str() != "none").cloned();
    let on_branch = said.get("branch").filter(|value| !value.is_empty()).cloned();
    Some(SubResult {
        branch: branch.to_string(),
        pushed_sha,
        commits: JsInt(JsNumber(count("commits").unwrap_or(0.0))),
        unpushed: JsInt(JsNumber(unpushed)),
        changed: JsInt(JsNumber(changed)),
        on_branch,
        norepo: None,
        read_failed: None,
        read_at: read_at.to_string(),
        push_refused: None,
    })
}

fn is_decimal_literal(text: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let body = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(at) => (&body[..at], Some(&body[at + 1..])),
        None => (body, None),
    };
    let mantissa_ok = match mantissa.split_once('.') {
        Some((int, frac)) => (int.is_empty() && digits(frac)) || (digits(int) && (frac.is_empty() || digits(frac))),
        None => digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)));
    mantissa_ok && exponent_ok
}

/// JavaScript's `Number(s)` on a string (rule 5): `js_trim`, `""` is 0, `0x`/`0o`/`0b` integers, decimal with an
/// exponent, `Infinity` with a sign, anything else NaN.
pub(crate) fn js_number(s: &str) -> f64 {
    let text = js_trim(s);
    if text.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16_u32), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = text.strip_prefix(prefix) {
            let valid = !digits.is_empty() && digits.chars().all(|c| c.is_digit(radix));
            return if valid { digits.chars().fold(0.0, |acc, c| acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))) } else { f64::NAN };
        }
    }
    match text {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ if is_decimal_literal(text) => text.parse::<f64>().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
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
    [
        format!("f=$(find {} -maxdepth {} -type f -name {} 2>/dev/null | head -n 1)", shell_quote(&file.folder), file.depth + 1, shell_quote(&format!("*{}", file.name))),
        "[ -n \"$f\" ] || { echo nofile; exit 0; }".to_string(),
        "echo \"path $f\"".to_string(),
        format!("gzip -c \"$f\" > {} && chmod 0644 {} && echo \"size $(stat -c %s {})\"", shell_quote(to), shell_quote(to), shell_quote(to)),
    ]
    .join("\n")
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
    let dir = match path.rfind('/') {
        Some(at) => path[..at].to_string(),
        None => slice(path, 0, Some(-1)),
    };
    [
        format!("install -d -m 0700 {}", shell_quote(&dir)),
        format!(
            "find {} -maxdepth {} -type f -name {} ! -path {} -delete 2>/dev/null",
            shell_quote(&file.folder),
            file.depth + 1,
            shell_quote(&format!("*{}", file.name)),
            shell_quote(path)
        ),
        format!("gunzip -c {} > {}; s=$?; rm -f {}; exit $s", shell_quote(from), shell_quote(path), shell_quote(from)),
    ]
    .join("\n")
}

// export function subRowOf(record: WorkspaceRecord): SubRow | undefined { ... sub-life.ts:100-130 }
/// sub-life.ts:100. A sub-machine's record as its thread's row reads it (rules 9-10).
pub fn sub_row_of(record: &WorkspaceRecord) -> Option<SubRow> {
    let sub = record.sub.as_ref()?;
    let state = match record.phase {
        WorkspacePhase::Released | WorkspacePhase::Gone => SubState::Released,
        WorkspacePhase::Queued if record.queue.is_some() => SubState::Waiting,
        WorkspacePhase::Queued => SubState::Starting,
        WorkspacePhase::Running if sub_spawning(sub.spawn.as_ref()) => SubState::Starting,
        WorkspacePhase::Running => SubState::Running,
        _ => SubState::Napping,
    };
    Some(SubRow {
        state,
        on: sub.on.clone(),
        machine: record.name.clone(),
        size: SubRowSize { cpu: JsInt(record.size.cpu.clone()), mem_mb: JsInt(record.size.mem_mb.clone()), name: sub.size_name.clone() },
        rate_usd_per_hour: sub.rate_usd_per_hour.clone(),
        branch: sub.branch.clone(),
        result: sub.result.clone(),
        held: sub.held.as_ref().map(|held| SubHeld { files: held.held.files, commits: held.held.commits }),
        delete_at: sub.delete_at.clone(),
        released_at: sub.released_at.clone(),
        cloud_gone: (sub.cloud_gone == Some(true)).then_some(True),
        session_not_kept: session_not_kept(sub.session.as_ref()),
        spawn: sub.spawn.clone(),
    })
}

/// sub-life.ts:133. What the row says of a session the host holds no copy of (rule 10).
fn session_not_kept(session: Option<&SubSession>) -> Option<String> {
    let session = session?;
    if session.kept.is_some() {
        return None;
    }
    if let Some(bytes) = session.bytes.as_ref().map(|bytes| bytes.0) {
        if bytes > SUB_SESSION_MAX_BYTES as f64 {
            return Some(sub_session_too_large_line(&fmt_bytes(bytes)));
        }
    }
    Some(sub_session_not_kept_line(session.why.as_deref().unwrap_or("no reason was given")))
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
    let backend = ctx.backend_for(record).ok()?;
    backend.lifecycle()?.disk_kept_ms().map(|ms| ms as f64)
}

// export function diskKeptUntil(ctx: Pick<RuntimeContext, "backendFor">, record: WorkspaceRecord, stoppedAt?: number): number | undefined {
//   const from = record.phase === "napping" && record.nappedAt !== undefined ? record.nappedAt : stoppedAt;
//   const ms = from === undefined ? undefined : diskKeptMs(ctx, record);
//   return ms === undefined ? undefined : from! + ms;
// }
/// sub-life.ts:150. The day from which a record's cloud may delete its machine's disk (rule 12).
pub fn disk_kept_until(ctx: &dyn BackendFor, record: &WorkspaceRecord, stopped_at: Option<f64>) -> Option<f64> {
    let napped = record.napped_at.as_ref().map(|at| at.0);
    let from = if record.phase == WorkspacePhase::Napping && napped.is_some() { napped } else { stopped_at };
    let from = from?;
    Some(from + disk_kept_ms(ctx, record)?)
}
