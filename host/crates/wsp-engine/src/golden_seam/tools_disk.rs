// SPDX-License-Identifier: AGPL-3.0-only
//! Unit tools_disk (spec/golden-tools_disk.md): golden-tools.ts:42-185 and 278-285 at a4033dc34. df and the disk the
//! loop keeps free, the guard every install runs under, the reason a failed run is named by, the road a source install
//! reports, the cache sweep and the PATH check.
use indexmap::IndexSet;
use wsp_catalog::road_modules::road_step;
use wsp_catalog::roads::RoadName;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::js::{is_js_whitespace, js_trim, number_string, to_number};
use wsp_words::words::image::DiskUse;
use wsp_words::words::thread::timed_out_line;
use wsp_words::words::units::fmt_bytes;

use super::import_roads::path_line;
use super::{DfColumn, ToolRoad};
use crate::errors::Result;
use crate::exec_detached::INLINE_EXEC_MS;
use crate::machine::{ExecOptions, Machine, RunOptions};

// TS golden-tools.ts:44-50
// /** Where each figure sits on df's line. */
// const DF_COLUMN = { size: 2, used: 3, free: 4 } as const;
// /** df on a disk in kilobytes, the columns asked for on one line in that order, /root's unless a path is named: used is
//  * what a snapshot comes to, free is what an install has left, size is the disk those two are read against; the one
//  * place the df line is spelled. */
// export const dfKbCmd = (columns: readonly (keyof typeof DF_COLUMN)[], path = "/root"): string => `df -Pk ${path} | awk 'NR==2{print ${columns.map(c => `$${DF_COLUMN[c]}`).join(", ")}}'`;
// export const FREE_KB_CMD = dfKbCmd(["free"]);
/// golden-tools.ts:49. `df -Pk <path> | awk 'NR==2{print $a, $b}'`, the columns in the order asked; `path` is written
/// as given, unquoted (a caller hands in `prefix_volume`'s shell word); `None` is `/root`.
pub fn df_kb_cmd(columns: &[DfColumn], path: Option<&str>) -> String {
    let fields: Vec<String> = columns.iter().map(|c| format!("${}", df_field(*c))).collect();
    format!("df -Pk {} | awk 'NR==2{{print {}}}'", path.unwrap_or("/root"), fields.join(", "))
}

fn df_field(column: DfColumn) -> u8 {
    match column {
        DfColumn::Size => 2,
        DfColumn::Used => 3,
        DfColumn::Free => 4,
    }
}

/// golden-tools.ts:50 `FREE_KB_CMD`.
pub fn free_kb_cmd() -> String {
    df_kb_cmd(&[DfColumn::Free], None)
}

/// golden-tools.ts:51 `USED_KB_CMD`.
pub fn used_kb_cmd() -> String {
    df_kb_cmd(&[DfColumn::Used], None)
}

// TS golden-tools.ts:52-55
// /** The folder df reads for a computer somebody owns, as a shell word: wsp's install folder, where every manager there
//  * installs, or where that is not there yet the nearest folder above it, which is where the first install makes it.
//  * The place report and the check before a join read their disk off the same folder, so every floor agrees. */
// export const prefixVolume = (prefix: string): string => `"$(p=${shellQuote(prefix)}; until [ -e "$p" ]; do p=$(dirname "$p"); done; echo "$p")"`;
/// golden-tools.ts:55 `prefixVolume`: the nearest folder at or above `prefix` that exists, as one shell word.
pub fn prefix_volume(prefix: &str) -> String {
    format!("\"$(p={}; until [ -e \"$p\" ]; do p=$(dirname \"$p\"); done; echo \"$p\")\"", shell_quote(prefix))
}

/// golden-tools.ts:112 `FreeDisk`.
#[derive(Debug, Clone, PartialEq)]
pub enum FreeDisk {
    /// JavaScript's number: df's kilobytes as `Number` read them, times 1024.
    Free {
        bytes: f64,
    },
    Unknown {
        reason: String,
    },
}

// TS golden-tools.ts:56-69
// /** How a df that did not answer reads, for dfRead and for the rejected exec diskUse catches. */
// const dfFailed = (res: ExecResult): string => `df failed: ${reasonOf(res, INLINE_EXEC_MS / 1000)}`;
// /** The columns asked for off one df, in bytes, under /root or on the volume a prefix installs onto; a full disk is free
//  * 0 and an empty one used 0, so only a size of zero is a reading no disk has; a rejected exec throws. */
// async function dfRead(machine: Machine, columns: readonly (keyof typeof DF_COLUMN)[], prefix?: string): Promise<{ bytes: number[] } | { reason: string }> {
//   const res = await machine.exec(prefix === undefined ? dfKbCmd(columns) : dfKbCmd(columns, prefixVolume(prefix)), { timeoutMs: INLINE_EXEC_MS });
//   const printed = res.stdout.trim();
//   const kb = printed.split(/\s+/).map(Number);
//   const size = columns.indexOf("size");
//   // The pipe exits as awk does, so a df that failed exits 0 having printed nothing.
//   const answered = res.exitCode === 0 && printed !== "";
//   if (answered && kb.length === columns.length && kb.every(n => Number.isFinite(n) && n >= 0) && (size === -1 || kb[size]! > 0)) return { bytes: kb.map(n => n * 1024) };
//   return { reason: answered ? `df answered ${printed.slice(0, 160)}` : dfFailed(res) };
// }
enum DfRead {
    Bytes(Vec<f64>),
    Reason(String),
}

pub(super) fn inline_s() -> f64 {
    INLINE_EXEC_MS as f64 / 1000.0
}

fn df_failed(res: &ExecResult) -> String {
    format!("df failed: {}", reason_of(res, inline_s()))
}

async fn df_read(machine: &dyn Machine, columns: &[DfColumn], prefix: Option<&str>) -> Result<DfRead> {
    let path = prefix.map(prefix_volume);
    let cmd = df_kb_cmd(columns, path.as_deref());
    let res = machine.exec(&cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    let printed = js_trim(&res.stdout);
    let kb: Vec<f64> = printed.split(is_js_whitespace).filter(|p| !p.is_empty()).map(to_number).collect();
    let size = columns.iter().position(|c| *c == DfColumn::Size);
    let answered = res.exit_code.get() == 0 && !printed.is_empty();
    if answered && kb.len() == columns.len() && kb.iter().all(|n| n.is_finite() && *n >= 0.0) && size.is_none_or(|i| kb[i] > 0.0) {
        return Ok(DfRead::Bytes(kb.iter().map(|n| n * 1024.0).collect()));
    }
    Ok(DfRead::Reason(if answered { format!("df answered {}", cut_units(printed, 160)) } else { df_failed(&res) }))
}

/// The first `n` UTF-16 units of `s`; a cut inside a surrogate pair writes U+FFFD for the half (the coordinator's ruling
/// on #2446 for every UTF-16 cut).
pub(super) fn cut_units(s: &str, n: usize) -> String {
    String::from_utf16_lossy(&s.encode_utf16().take(n).collect::<Vec<u16>>())
}

// TS golden-tools.ts:114-124
// /** One df column read off the machine, in bytes; a df that fails or prints no number is reported, never assumed. */
// async function dfRootBytes(machine: Machine, column: "used" | "free", prefix?: string): Promise<{ bytes: number } | { reason: string }> {
//   const read = await dfRead(machine, [column], prefix);
//   return "bytes" in read ? { bytes: read.bytes[0]! } : read;
// }
//
// /** What df says is free under /root, or on the volume the prefix installs onto where one is named. */
// export async function freeBytes(machine: Machine, prefix?: string): Promise<FreeDisk> {
//   const read = await dfRootBytes(machine, "free", prefix);
//   return "bytes" in read ? { kind: "free", bytes: read.bytes } : { kind: "unknown", reason: read.reason };
// }
/// golden-tools.ts:121 `freeBytes`: df's free column under /root, or on `prefix_volume(prefix)`. A df that fails or
/// prints no usable number is `Unknown` with its reason; an exec that rejects is the `Err`.
pub async fn free_bytes(machine: &dyn Machine, prefix: Option<&str>) -> Result<FreeDisk> {
    Ok(match df_read(machine, &[DfColumn::Free], prefix).await? {
        DfRead::Bytes(bytes) => FreeDisk::Free { bytes: bytes[0] },
        DfRead::Reason(reason) => FreeDisk::Unknown { reason },
    })
}

// TS golden-tools.ts:126-131
// /** What df says is used under /root, or on the volume the prefix installs onto where one is named, and the size of
//  * that disk, off one exec; a df that fails or prints no pair of numbers is reported, never assumed. */
// export async function diskUse(machine: Machine, prefix?: string): Promise<DiskUse> {
//   const read = await dfRead(machine, ["used", "size"], prefix).catch((e: unknown) => ({ reason: dfFailed({ exitCode: 1, stdout: "", stderr: e instanceof Error ? e.message : String(e) }) }));
//   return "bytes" in read ? { kind: "use", usedBytes: read.bytes[0]!, sizeBytes: read.bytes[1]! } : { kind: "unknown", reason: read.reason };
// }
/// golden-tools.ts:128 `diskUse`: used and size off one df. Never fails: a rejected exec reads as `Unknown` with
/// `df failed: <its words>`.
pub async fn disk_use(machine: &dyn Machine, prefix: Option<&str>) -> DiskUse {
    let read = match df_read(machine, &[DfColumn::Used, DfColumn::Size], prefix).await {
        Ok(read) => read,
        Err(e) => DfRead::Reason(df_failed(&ExecResult { exit_code: 1i64.into(), stdout: String::new(), stderr: e.to_string() })),
    };
    match read {
        DfRead::Bytes(bytes) => DiskUse::Use { used_bytes: bytes[0], size_bytes: bytes[1] },
        DfRead::Reason(reason) => DiskUse::Unknown { reason },
    }
}

/// `disk_use`'s size: `Some(size_bytes)` for `DiskUse::Use`, `None` for `Unknown`. What provision hands
/// `owned_floor_bytes`.
pub async fn disk_use_size(machine: &dyn Machine, prefix: Option<&str>) -> Option<u64> {
    match disk_use(machine, prefix).await {
        DiskUse::Use { size_bytes, .. } => Some(size_bytes as u64),
        DiskUse::Unknown { .. } => None,
    }
}

// TS golden-tools.ts:369-372
// /** What a run keeps free on a computer somebody owns: the floor, or a tenth of its disk where that is more, since
//  * the computer runs other things that write to the same disk (a full disk stopped a production queue's saves,
//  * 2026-10-04). */
// export const ownedFloorBytes = (sizeBytes: number | undefined): number => Math.max(TOOLS_DISK_FLOOR, Math.floor((sizeBytes ?? 0) / 10));
/// golden-tools.ts:372 `ownedFloorBytes`: the larger of `TOOLS_DISK_FLOOR` and a tenth of the disk, floored.
pub fn owned_floor_bytes(size_bytes: Option<u64>) -> u64 {
    super::TOOLS_DISK_FLOOR.max(size_bytes.unwrap_or(0) / 10)
}

/// golden-tools.ts:102.
pub const ALREADY_ON_MACHINE: &str = "already on the machine";

/// golden-tools.ts:76 `GUARD_SLACK_S`: two kill graces of 10 s and 40 s of polls.
pub const GUARD_SLACK_S: u64 = 60;

/// golden-tools.ts:78 `guardDeadlineMs`.
pub fn guard_deadline_ms(timeout_s: u64) -> u64 {
    (timeout_s + GUARD_SLACK_S) * 1000
}

// TS golden-tools.ts:90-97
// /** The line that names the failure, for a warning: the last `Error:` line on stderr (Homebrew
//  * follows its error with advice), else the last stderr line, else stdout's; 124 is the guest-side timeout. */
// export function reasonOf(res: ExecResult, timeoutS: number): string {
//   if (res.exitCode === 124) return timedOutLine(timeoutS);
//   const lines = (text: string): string[] => text.split("\n").map(l => l.trim()).filter(l => l !== "");
//   const err = lines(res.stderr);
//   return (err.filter(l => l.startsWith("Error:")).at(-1) ?? err.at(-1) ?? lines(res.stdout).at(-1) ?? `exit ${res.exitCode}`).slice(0, 160);
// }
/// golden-tools.ts:92 `reasonOf`: 124 is `timed_out_line(timeout_s)`; else the last `Error:` line of stderr, else stderr's
/// last line, else stdout's, else `exit <code>`; lines trimmed, blank ones dropped; cut to 160 UTF-16 units.
pub fn reason_of(res: &ExecResult, timeout_s: f64) -> String {
    let code = res.exit_code.as_f64();
    if code == 124.0 {
        return timed_out_line(timeout_s, None);
    }
    let lines = |text: &str| -> Vec<String> { text.split('\n').map(|l| js_trim(l).to_string()).filter(|l| !l.is_empty()).collect() };
    let err = lines(&res.stderr);
    let picked = err
        .iter()
        .rev()
        .find(|l| l.starts_with("Error:"))
        .cloned()
        .or_else(|| err.last().cloned())
        .or_else(|| lines(&res.stdout).last().cloned())
        .unwrap_or_else(|| format!("exit {}", number_string(code)));
    cut_units(&picked, 160)
}

// TS golden-tools.ts:104-110
// /** The last WSP_ROAD line a road install printed, when it printed one. */
// export function roadOf(stdout: string): ToolRoad | undefined {
//   const m = [...stdout.matchAll(/^WSP_ROAD (release|go) (\S+)(?: ([0-9a-f]{64})(?: (\S+)(?: (x86_64|aarch64))?)?)?/gm)].at(-1);
//   return m === undefined
//     ? undefined
//     : { kind: m[1] as ToolRoad["kind"], from: m[2]!, ...(m[3] !== undefined ? { sha256: m[3] } : {}), ...(m[4] !== undefined ? { tag: m[4] } : {}), ...(m[5] !== undefined ? { arch: m[5] as ToolPin["arch"] } : {}) };
// }
/// golden-tools.ts:105 `roadOf`: the last line starting `WSP_ROAD release|go <from>`, with the sha256, tag and arch
/// that follow it where they match.
pub fn road_of(stdout: &str) -> Option<ToolRoad> {
    stdout.split(['\n', '\r', '\u{2028}', '\u{2029}']).filter_map(road_line).next_back()
}

fn road_line(line: &str) -> Option<ToolRoad> {
    let rest = line.strip_prefix("WSP_ROAD ")?;
    let (kind, rest) = match rest.strip_prefix("release ") {
        Some(after) => ("release", after),
        None => ("go", rest.strip_prefix("go ")?),
    };
    let (from, rest) = split_word(rest);
    if from.is_empty() {
        return None;
    }
    let mut road = ToolRoad { kind: kind.to_string(), from: from.to_string(), sha256: None, tag: None, arch: None };
    let Some(after) = rest.strip_prefix(' ') else { return Some(road) };
    let Some(sha) = after.get(..64).filter(|s| s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))) else {
        return Some(road);
    };
    road.sha256 = Some(sha.to_string());
    let Some(tagged) = after[64..].strip_prefix(' ') else { return Some(road) };
    let (tag, rest) = split_word(tagged);
    if tag.is_empty() {
        return Some(road);
    }
    road.tag = Some(tag.to_string());
    road.arch = rest.strip_prefix(' ').and_then(|a| ["x86_64", "aarch64"].into_iter().find(|w| a.starts_with(w))).map(str::to_string);
    Some(road)
}

fn split_word(s: &str) -> (&str, &str) {
    let end = s.find(is_js_whitespace).unwrap_or(s.len());
    s.split_at(end)
}

// TS golden-tools.ts:144-151
// /** The df reading that closes a stage's last line, so the run's log says what each stage left on the disk. */
// export async function freeNote(machine: Machine, prefix?: string): Promise<string | undefined> {
//   const free = await freeBytes(machine, prefix);
//   return free.kind === "free" ? `${fmtBytes(free.bytes)} free` : undefined;
// }
//
// /** A stage's closing line: its parts in order, the empty ones left out. */
// export const closing = (...parts: (string | undefined)[]): string => parts.filter(p => p !== undefined && p !== "").join("; ");
/// golden-tools.ts:145 `freeNote`: `<fmt_bytes(free)> free`, `None` when df did not read; a rejected exec is the `Err`.
pub async fn free_note(machine: &dyn Machine, prefix: Option<&str>) -> Result<Option<String>> {
    Ok(match free_bytes(machine, prefix).await? {
        FreeDisk::Free { bytes } => Some(format!("{} free", fmt_bytes(bytes))),
        FreeDisk::Unknown { .. } => None,
    })
}

/// golden-tools.ts:151 `closing`: the parts joined with `; `, `None` and empty ones left out.
pub fn closing(parts: &[Option<&str>]) -> String {
    parts.iter().flatten().filter(|p| !p.is_empty()).copied().collect::<Vec<&str>>().join("; ")
}

// TS golden-tools.ts:153-172
// const SWEEP_TIMEOUT_S = 300;
// /** The caches the installs leave on the root disk: npm's tarballs, uv's wheels, go's module and build caches,
//  * apt's debs, node-gyp's headers from the daemon's native build. Each is rebuilt on use; together they held about
//  * 2 GB of the 20 GB builder after one recipe. */
// const sweepCachesCmd = (path: string): string => [
//   "set -euo pipefail",
//   `export PATH=${path}`,
//   "if command -v go >/dev/null 2>&1; then go clean -cache -modcache; fi",
//   "rm -rf /root/.npm /root/.cache/uv /root/.cache/go-build /root/.cache/node-gyp",
//   "if command -v apt-get >/dev/null 2>&1; then apt-get clean; fi",
// ].join("\n");
//
// /** After an install stage the caches go, under the guard; the phrase says what came back, or why nothing did. */
// export async function sweepCaches(machine: Machine, path: string = TOOLS_PATH): Promise<string> {
//   const before = await freeBytes(machine);
//   const res = await machine.run(guarded(sweepCachesCmd(path), SWEEP_TIMEOUT_S), { deadlineMs: guardDeadlineMs(SWEEP_TIMEOUT_S) });
//   if (res.exitCode !== 0) return `cache sweep failed (${reasonOf(res, SWEEP_TIMEOUT_S)})`;
//   const after = await freeBytes(machine);
//   return before.kind === "free" && after.kind === "free" && after.bytes > before.bytes ? `caches swept, ${fmtBytes(after.bytes - before.bytes)} back` : "caches swept";
// }
const SWEEP_TIMEOUT_S: u64 = 300;

fn sweep_caches_cmd(path: &str) -> String {
    [
        "set -euo pipefail".to_string(),
        format!("export PATH={path}"),
        "if command -v go >/dev/null 2>&1; then go clean -cache -modcache; fi".to_string(),
        "rm -rf /root/.npm /root/.cache/uv /root/.cache/go-build /root/.cache/node-gyp".to_string(),
        "if command -v apt-get >/dev/null 2>&1; then apt-get clean; fi".to_string(),
    ]
    .join("\n")
}

/// golden-tools.ts:166 `sweepCaches`: the sweep under the guard as one run, df read before and after; the phrase says
/// what came back. A rejected exec or run is the `Err`.
pub async fn sweep_caches(machine: &dyn Machine, path: &str) -> Result<String> {
    let before = free_bytes(machine, None).await?;
    let script = guarded(&sweep_caches_cmd(path), SWEEP_TIMEOUT_S);
    let res = machine.run(&script, RunOptions { deadline_ms: guard_deadline_ms(SWEEP_TIMEOUT_S), ..Default::default() }).await?;
    if res.exit_code.get() != 0 {
        return Ok(format!("cache sweep failed ({})", reason_of(&res, SWEEP_TIMEOUT_S as f64)));
    }
    let after = free_bytes(machine, None).await?;
    Ok(match (before, after) {
        (FreeDisk::Free { bytes: before }, FreeDisk::Free { bytes: after }) if after > before => {
            format!("caches swept, {} back", fmt_bytes(after - before))
        }
        _ => "caches swept".to_string(),
    })
}

// TS golden-tools.ts:186-227
// // Descendants of $1 by parent pid from /proc, widened until no new pid turns up: su -c
// // starts its command in a fresh session, so the session's process group alone misses it.
// const TREE_FN = [
//   "tree() {",
//   '  local want="$1" found="" f s pid ppid more=1',
//   '  while [ -n "$more" ]; do',
//   "    more=",
//   "    for f in /proc/[0-9]*/stat; do",
//   '      [ -r "$f" ] && read -r s < "$f" || continue',
//   '      pid=${f#/proc/}; pid=${pid%/stat}; s=${s##*) }; ppid=${s#* }; ppid=${ppid%% *}',
//   '      case " $want $found " in *" $pid "*) continue;; esac',
//   '      case " $want $found " in *" $ppid "*) found="$found $pid"; more=1;; esac',
//   "    done",
//   "  done",
//   "  echo $found",
//   "}",
// ].join("\n");
//
// /** The script in its own session, watched from outside it: at the timeout the session's process
//  * group and everything descended from it get TERM, then KILL, and the guard returns 124 only once
//  * they are gone (disowned first, or bash reports the kill on stderr). The exit code and output are
//  * the script's own otherwise. */
// export function guarded(script: string, timeoutS: number): string {
//   return [
//     TREE_FN,
//     `setsid bash -c ${shellQuote(script)} &`,
//     "p=$!",
//     "t=0",
//     `while [ $t -lt ${timeoutS} ] && kill -0 $p 2>/dev/null; do sleep 1; t=$((t+1)); done`,
//     "if kill -0 $p 2>/dev/null; then",
//     "  disown $p 2>/dev/null",
//     '  v="$p $(tree $p)"',
//     "  kill -TERM -- -$p $v 2>/dev/null",
//     `  t=0; while [ $t -lt ${KILL_GRACE_S} ] && kill -0 $v 2>/dev/null; do sleep 1; t=$((t+1)); done`,
//     '  v="$p $(tree $p)"',
//     "  kill -KILL -- -$p $v 2>/dev/null",
//     `  t=0; while [ $t -lt ${KILL_GRACE_S} ] && kill -0 $v 2>/dev/null; do sleep 1; t=$((t+1)); done`,
//     "  exit 124",
//     "fi",
//     "wait $p",
//   ].join("\n");
// }
const KILL_GRACE_S: u64 = 10;

const TREE_FN: [&str; 13] = [
    "tree() {",
    r#"  local want="$1" found="" f s pid ppid more=1"#,
    r#"  while [ -n "$more" ]; do"#,
    "    more=",
    "    for f in /proc/[0-9]*/stat; do",
    r#"      [ -r "$f" ] && read -r s < "$f" || continue"#,
    r#"      pid=${f#/proc/}; pid=${pid%/stat}; s=${s##*) }; ppid=${s#* }; ppid=${ppid%% *}"#,
    r#"      case " $want $found " in *" $pid "*) continue;; esac"#,
    r#"      case " $want $found " in *" $ppid "*) found="$found $pid"; more=1;; esac"#,
    "    done",
    "  done",
    "  echo $found",
    "}",
];

/// golden-tools.ts:208 `guarded`: the script in its own session, watched; at the timeout TERM then KILL to the group
/// and every descendant, then exit 124.
pub fn guarded(script: &str, timeout_s: u64) -> String {
    [
        TREE_FN.join("\n"),
        format!("setsid bash -c {} &", shell_quote(script)),
        "p=$!".to_string(),
        "t=0".to_string(),
        format!("while [ $t -lt {timeout_s} ] && kill -0 $p 2>/dev/null; do sleep 1; t=$((t+1)); done"),
        "if kill -0 $p 2>/dev/null; then".to_string(),
        "  disown $p 2>/dev/null".to_string(),
        r#"  v="$p $(tree $p)""#.to_string(),
        "  kill -TERM -- -$p $v 2>/dev/null".to_string(),
        format!("  t=0; while [ $t -lt {KILL_GRACE_S} ] && kill -0 $v 2>/dev/null; do sleep 1; t=$((t+1)); done"),
        r#"  v="$p $(tree $p)""#.to_string(),
        "  kill -KILL -- -$p $v 2>/dev/null".to_string(),
        format!("  t=0; while [ $t -lt {KILL_GRACE_S} ] && kill -0 $v 2>/dev/null; do sleep 1; t=$((t+1)); done"),
        "  exit 124".to_string(),
        "fi".to_string(),
        "wait $p".to_string(),
    ]
    .join("\n")
}

/// golden-tools.ts:230 `roadLimitS`: the road's own limit, off wsp_catalog's `road_step`.
pub fn road_limit_s(road: RoadName) -> u64 {
    road_step(road).limit_s
}

// TS golden-tools.ts:232-235
// /** A road's script under the guard: the road's network clock ahead of the command, the road's limit on the run. */
// export function guardedRoad(road: RoadName, cmd: string): string {
//   return guarded([...ROAD_STEPS[road].env, cmd].join("\n"), roadLimitS(road));
// }
/// golden-tools.ts:233 `guardedRoad`: the road's env lines ahead of `cmd`, under the road's limit.
pub fn guarded_road(road: RoadName, cmd: &str) -> String {
    let step = road_step(road);
    let script: Vec<String> = step.env.iter().cloned().chain([cmd.to_string()]).collect();
    guarded(&script.join("\n"), step.limit_s)
}

/// golden-tools.ts:279's answer. `missing` keeps the order the lines came in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MissingCommands {
    pub missing: IndexSet<String>,
    pub failed: Option<String>,
}

// TS golden-tools.ts:278-285
// /** Which of the commands are not on the machine's tools PATH; `failed` says why the check itself could not run. */
// export async function missingCommands(machine: Machine, bins: readonly string[], path: string = TOOLS_PATH, prefix?: string): Promise<{ missing: Set<string>; failed?: string }> {
//   const cmd = `${pathLine(path, prefix)}\nfor b in ${bins.map(shellQuote).join(" ")}; do command -v "$b" >/dev/null 2>&1 || echo "missing $b"; done`;
//   const res = await machine.exec(cmd, { timeoutMs: INLINE_EXEC_MS });
//   const failed = res.exitCode === 0 ? undefined : reasonOf(res, INLINE_EXEC_MS / 1000);
//   const missing = new Set(res.stdout.split("\n").flatMap(l => (l.startsWith("missing ") ? [l.slice("missing ".length).trim()] : [])));
//   return failed === undefined ? { missing } : { missing, failed };
// }
/// golden-tools.ts:279 `missingCommands`: one exec on `path_line(path, prefix)`; `failed` is `reason_of` at 20 s
/// when it did not exit 0, and `missing` is read off stdout either way. A rejected exec is the `Err`.
pub async fn missing_commands(machine: &dyn Machine, bins: &[String], path: &str, prefix: Option<&str>) -> Result<MissingCommands> {
    let names: Vec<String> = bins.iter().map(|b| shell_quote(b)).collect();
    let cmd = format!(
        "{}\nfor b in {}; do command -v \"$b\" >/dev/null 2>&1 || echo \"missing $b\"; done",
        path_line(path, prefix),
        names.join(" ")
    );
    let res = machine.exec(&cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await?;
    let failed = (res.exit_code.get() != 0).then(|| reason_of(&res, inline_s()));
    let missing = res.stdout.split('\n').filter_map(|l| l.strip_prefix("missing ")).map(|n| js_trim(n).to_string()).collect();
    Ok(MissingCommands { missing, failed })
}
