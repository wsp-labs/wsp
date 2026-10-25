// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/units.ts at a3b870a6e: bytes, sizes, rates, durations, tokens and costs as a
//! person reads them, and the size, Boat and disk lines built on them. Each TS statement stands as a comment above
//! its stub. See spec/units.md.

use wsp_wire::types::init_job::{InitDraft, InitJob, InitScreen, InitScreenId};
use wsp_wire::wire::capabilities::{MachineSizeOffer, PlaceCapacity, WorkspaceSize};

use crate::js::{js_round, js_trim, locale_number, number_string, to_fixed, utf16, Grouping};
use crate::words::base::plural;
use indexmap::IndexSet;
use regex::Regex;
use wsp_wire::types::init_job::InitScreenItemLock;
use wsp_wire::types::js_number::JsNumber;

// units.ts:4
pub const KIB: f64 = 1024.0;
const MIB: f64 = KIB * 1024.0;
const GIB: f64 = MIB * 1024.0;

// units.ts:5
// private in the TS: port it as a private helper
// const MIB = KIB * 1024;

// units.ts:6
// private in the TS: port it as a private helper
// const GIB = MIB * 1024;

// Bytes as a person reads them, in binary units: whole under a gigabyte, since a tenth of a megabyte is noise at that
// scale, and GB with one decimal unless whole.
// units.ts:10
// private in the TS: port it as a private helper
// const COMPACT_UNITS = ["", "K", "M", "G", "T"];

/// rss as a process table column with a three-digit budget fmtBytes does not fit, so its own rule: 900, 12K, 1.5M, 123M, 2.3G.
// units.ts:13
// export function compactBytes(n: number): string {
//   let v = Math.max(0, n);
//   let i = 0;
//   while (v >= 1000 && i < COMPACT_UNITS.length - 1) {
//     v /= 1024;
//     i++;
//   }
//   const text = i === 0 ? String(Math.round(v)) : v < 10 ? v.toFixed(1) : String(Math.round(v));
//   return `${text}${COMPACT_UNITS[i]}`;
// }
pub fn compact_bytes(n: f64) -> String {
    const COMPACT_UNITS: [&str; 5] = ["", "K", "M", "G", "T"];
    let mut v = if n > 0.0 || n.is_nan() { n } else { 0.0 };
    let mut i = 0;
    while v >= 1000.0 && i < COMPACT_UNITS.len() - 1 {
        v /= KIB;
        i += 1;
    }
    let text = if i == 0 || v >= 10.0 { number_string(js_round(v)) } else { to_fixed(v, 1) };
    format!("{text}{}", COMPACT_UNITS[i])
}

// units.ts:24
// export function fmtBytes(n: number): string {
//   return bytesIn(n, KIB);
// }
pub fn fmt_bytes(n: f64) -> String {
    bytes_in(n, KIB)
}

/// Bytes in decimal units, as a provider sells a disk: a 160 GB disk reads 160 GB, where fmtBytes says 149.
// units.ts:29
// export function fmtDiskBytes(n: number): string {
//   return bytesIn(n, 1000);
// }
pub fn fmt_disk_bytes(n: f64) -> String {
    bytes_in(n, 1000.0)
}

// units.ts:33
// private in the TS: port it as a private helper
// function bytesIn(n: number, k: number): string {
//   if (n < k) return `${n} B`;
//   if (n < k ** 2) return `${Math.round(n / k)} KB`;
//   if (n < k ** 3) return `${Math.round(n / k ** 2)} MB`;
//   const gb = n / k ** 3;
//   return `${Number.isInteger(Number(gb.toFixed(1))) ? Math.round(gb) : gb.toFixed(1)} GB`;
// }

/// A size said to be over a cap: past 1 MB with one decimal rounded up, so a file a byte over a cap never reads as
/// the cap itself.
// units.ts:43
// export function fmtBytesOver(n: number): string {
//   if (n < MIB || n >= GIB) return fmtBytes(n);
//   return `${(Math.ceil((n / MIB) * 10) / 10).toFixed(1)} MB`;
// }
pub fn fmt_bytes_over(n: f64) -> String {
    if !(MIB..GIB).contains(&n) {
        return fmt_bytes(n);
    }
    format!("{} MB", to_fixed(((n / MIB) * 10.0).ceil() / 10.0, 1))
}

/// What a row reads where the catalog has measured no size.
// units.ts:49
pub const UNKNOWN_SIZE: &str = "size unknown";

/// How often the agents ran a tool here, the one number a usage row shows.
// units.ts:52
// export const fmtCalls = (n: number): string => `${n.toLocaleString("en-US")} ${n === 1 ? "call" : "calls"}`;
pub fn fmt_calls(n: f64) -> String {
    format!("{} {}", locale_number(n, Grouping::EnUs, 0, 3), if n == 1.0 { "call" } else { "calls" })
}

/// The tally under a screen's card, first half: the count of rows on the image, so a view can colour the estimate after it on its own. The
/// noun is the screen's plural ("agents", "tools"), or a word that does not count ("more") kept as it is.
// units.ts:56
// export const initTallyCount = (count: number, noun: string): string => `${noun.endsWith("s") ? plural(count, noun.replace(/s$/, "")) : `${count} ${noun}`} on the image`;
pub fn init_tally_count(count: f64, noun: &str) -> String {
    let body = match noun.strip_suffix('s') {
        Some(singular) => plural(count, singular),
        None => format!("{} {noun}", number_string(count)),
    };
    format!("{body} on the image")
}

/// Two byte counts against each other, each under the byte rule ("4.9 GB of 20 GB"): the tally's running estimate
/// against the disk and the machine tab's memory and disk rows, each read in the tone the share earns; the first count
/// alone where there is no total.
// units.ts:60
// export const fmtBytesOfTotal = (used: number, total?: number): string => (total === undefined ? fmtBytes(used) : `${fmtBytes(used)} of ${fmtBytes(total)}`);
pub fn fmt_bytes_of_total(used: f64, total: Option<f64>) -> String {
    match total {
        None => fmt_bytes(used),
        Some(total) => format!("{} of {}", fmt_bytes(used), fmt_bytes(total)),
    }
}

/// How many rows of a screen are on the image with these ticks and the bytes they come to: a locked row is on, a row
/// that takes an answer is not counted, and a row nothing measured adds nothing.
// units.ts:64
// export function initTallyOf(screen: Pick<InitScreen, "items">, ticks: ReadonlySet<string>): { count: number; bytes: number } {
//   let count = 0;
//   let bytes = 0;
//   for (const item of screen.items) {
//     if (item.choices !== undefined) continue;
//     if (!(item.lock === "on" || ticks.has(item.id))) continue;
//     count += 1;
//     if (typeof item.size === "number") bytes += item.size;
//   }
//   return { count, bytes };
// }
/// The tally `init_tally_of` answers: rows counted and the bytes they come to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InitTally {
    pub count: f64,
    pub bytes: f64,
}

pub fn init_tally_of(screen: &InitScreen, ticks: &IndexSet<String>) -> InitTally {
    let mut count = 0.0;
    let mut bytes = 0.0;
    for item in &screen.items {
        if item.choices.is_some() {
            continue;
        }
        if !(matches!(item.lock, Some(InitScreenItemLock::On)) || ticks.contains(&item.id)) {
            continue;
        }
        count += 1.0;
        if let Some(Some(size)) = &item.size {
            bytes += size.0;
        }
    }
    InitTally { count, bytes }
}

/// The ticks of a screen as they stand: the draft kept on its step where the person left one, else what the host last
/// answered. The one rule for what a screen opens on and for what the estimate counts.
// units.ts:78
// export const initTicksOf = (screen: Pick<InitScreen, "ticks">, kept?: Pick<InitDraft, "ticks">): ReadonlySet<string> => new Set(kept?.ticks ?? screen.ticks);
pub fn init_ticks_of(screen: &InitScreen, kept: Option<&InitDraft>) -> IndexSet<String> {
    match kept {
        Some(draft) => draft.ticks.iter().cloned().collect(),
        None => screen.ticks.iter().cloned().collect(),
    }
}

/// The running estimate of the image in bytes: what the disk holds before any tick plus every screen's ticked rows as
/// they stand, drafts over answers. The one number every step's tally, the meter and Continue's refusal read. A client
/// passes the draft it holds and the host has not echoed yet, so a tick moves the number before the round trip.
// units.ts:83
// export function initImageBytes(job: Pick<InitJob, "disk" | "screens" | "drafts">, current?: { at: string; ticks: Iterable<string> }): number {
//   const fixed = job.disk?.fixed ?? 0;
//   return job.screens.reduce((sum, s) => sum + initTallyOf(s, current !== undefined && current.at === s.id ? new Set(current.ticks) : initTicksOf(s, job.drafts?.find(d => d.at === s.id))).bytes, fixed);
// }
/// The step a client is on and the ticks it holds that the host has not echoed yet.
#[derive(Clone, Debug, PartialEq)]
pub struct CurrentTicks {
    pub at: String,
    pub ticks: Vec<String>,
}

pub fn init_image_bytes(job: &InitJob, current: Option<&CurrentTicks>) -> f64 {
    let fixed = job.disk.as_ref().map_or(0.0, |disk| disk.fixed.0);
    job.screens.iter().fold(fixed, |sum, screen| {
        let ticks: IndexSet<String> = match current {
            Some(current) if current.at == screen_word(screen.id) => current.ticks.iter().cloned().collect(),
            _ => {
                let draft = job.drafts.as_ref().and_then(|drafts| drafts.iter().find(|d| d.at == screen_word(screen.id)));
                init_ticks_of(screen, draft)
            }
        };
        sum + init_tally_of(screen, &ticks).bytes
    })
}

/// The tone a size is read in, by weight, the one table every size cell reads: a gigabyte and over is the danger
/// tone, 200 MB and over the warning tone, 50 MB and over the yellow tone, anything under muted.
// units.ts:90
// export type SizeTone = "danger" | "warning" | "yellow" | "muted";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SizeTone {
    #[serde(rename = "danger")]
    Danger,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "yellow")]
    Yellow,
    #[serde(rename = "muted")]
    Muted,
}

impl SizeTone {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            SizeTone::Danger => "danger",
            SizeTone::Warning => "warning",
            SizeTone::Yellow => "yellow",
            SizeTone::Muted => "muted",
        }
    }
}

// units.ts:91
// private in the TS: port it as a private helper
// const SIZE_TONES: readonly (readonly [number, SizeTone])[] = [
//   [1024 * MIB, "danger"],
//   [200 * MIB, "warning"],
//   [50 * MIB, "yellow"],
// ];

// units.ts:96
// export function sizeTone(bytes: number): SizeTone {
//   return SIZE_TONES.find(([from]) => bytes >= from)?.[1] ?? "muted";
// }
pub fn size_tone(bytes: f64) -> SizeTone {
    if bytes >= GIB {
        SizeTone::Danger
    } else if bytes >= 200.0 * MIB {
        SizeTone::Warning
    } else if bytes >= 50.0 * MIB {
        SizeTone::Yellow
    } else {
        SizeTone::Muted
    }
}

// The screens where a person is choosing weight, whose size cells wear the weight tone. The agents screen's sizes
// are muted: the agents are what the person came for, not a place to be warned off.
// units.ts:102
// private in the TS: port it as a private helper
// const WEIGHED_SCREENS: ReadonlySet<InitScreenId> = new Set<InitScreenId>(["tools", "also"]);

/// The tone a row's size cell wears on a screen: the weight table where the screen weighs, muted elsewhere and where
/// nothing measured the row.
// units.ts:105
// export const initSizeTone = (screen: Pick<InitScreen, "id">, bytes: number | null): SizeTone => (bytes === null || !WEIGHED_SCREENS.has(screen.id) ? "muted" : sizeTone(bytes));
pub fn init_size_tone(screen_id: InitScreenId, bytes: Option<f64>) -> SizeTone {
    match bytes {
        Some(bytes) if matches!(screen_id, InitScreenId::Tools | InitScreenId::Also) => size_tone(bytes),
        _ => SizeTone::Muted,
    }
}

/// The tone of the disk meter and the tally's estimate by the share the estimate takes of the disk: muted under 70
/// percent, the warning tone from 70, the danger tone from 90 and when over.
// units.ts:109
// export function diskTone(used: number, total: number): "muted" | "warning" | "danger" {
//   const share = total > 0 ? used / total : 1;
//   if (share >= 0.9) return "danger";
//   return share >= 0.7 ? "warning" : "muted";
// }
/// `diskTone`'s answer: never Yellow.
pub fn disk_tone(used: f64, total: f64) -> SizeTone {
    let share = if total > 0.0 { used / total } else { 1.0 };
    if share >= 0.9 {
        SizeTone::Danger
    } else if share >= 0.7 {
        SizeTone::Warning
    } else {
        SizeTone::Muted
    }
}

/// The disk meter's tooltip: what the image holds against the machine's disk, and by how much it is over once it is;
/// the overshoot is said here and in Continue's refusal, nowhere else.
// units.ts:117
// export const initDiskLine = (used: number, total: number): string => `about ${fmtBytes(used)} of ${fmtBytes(total)} on the image${used > total ? `, ${initDiskOverLine(used - total)}` : ""}`;
pub fn init_disk_line(used: f64, total: f64) -> String {
    let over = if used > total { format!(", {}", init_disk_over_line(used - total)) } else { String::new() };
    format!("about {} of {} on the image{over}", fmt_bytes(used), fmt_bytes(total))
}

/// The primary's refusal once the ticks pass the disk, and the tail of the meter's tooltip.
// units.ts:120
// export const initDiskOverLine = (over: number): string => `over by ${fmtBytes(over)}`;
pub fn init_disk_over_line(over: f64) -> String {
    format!("over by {}", fmt_bytes(over))
}

// Memory in GB as the size table names it: whole when whole, else one decimal; a size spec, not a byte count.
// units.ts:123
// private in the TS: port it as a private helper
// const memGb = (memMb: number): number => Number((memMb / 1024).toFixed(1));

/// A size in css pixels, as the settings page states the text and sidebar sizes.
// units.ts:126
// export function fmtPx(px: number): string {
//   return `${px} px`;
// }
pub fn fmt_px(px: f64) -> String {
    format!("{} px", number_string(px))
}

/// A machine size's memory with its unit.
// units.ts:131
// export function fmtMemGb(memMb: number): string {
//   return `${memGb(memMb)} GB`;
// }
pub fn fmt_mem_gb(mem_mb: f64) -> String {
    format!("{} GB", number_string(mem_gb(mem_mb)))
}

/// What one of a machine's cpus is called: a provider's are virtual, this computer's are the cores it has.
// units.ts:136
// export type CpuWord = "vCPU" | "cores";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CpuWord {
    #[serde(rename = "vCPU")]
    VCPU,
    #[serde(rename = "cores")]
    Cores,
}

impl CpuWord {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            CpuWord::VCPU => "vCPU",
            CpuWord::Cores => "cores",
        }
    }
}

/// A count of cpus in the kind's word: "1 core" and "2 cores", while a provider's vCPU reads the same at any count.
// units.ts:139
// export function fmtCpus(n: number, cpu: CpuWord = "vCPU"): string {
//   return `${n} ${cpu === "cores" && n === 1 ? "core" : cpu}`;
// }
pub fn fmt_cpus(n: f64, cpu: Option<CpuWord>) -> String {
    let word = match cpu.unwrap_or(CpuWord::VCPU) {
        CpuWord::Cores if n == 1.0 => "core",
        word => word.as_str(),
    };
    format!("{} {word}", number_string(n))
}

/// A size as the sidebar row and the Machine tab show it: "2 vCPU, 4 GB", or "10 cores, 16 GB"
/// in the word the machine's kind has for a cpu. A size offer is always a provider's, so the provider's word is the default.
/// The size joins its words with no-break spaces, so a sentence carrying it never breaks between a number and its unit.
// units.ts:146
// export function fmtSize(size: WorkspaceSize, cpu: CpuWord = "vCPU"): string {
//   return `${fmtCpus(size.cpu, cpu)},\u00a0${fmtMemGb(size.memMb)}`.replace(/ /g, "\u00a0");
// }
pub fn fmt_size(size: &WorkspaceSize, cpu: Option<CpuWord>) -> String {
    format!("{},\u{a0}{}", fmt_cpus(size.cpu.0, cpu), fmt_mem_gb(size.mem_mb.0)).replace(' ', "\u{a0}")
}

/// What the create says where the machine it got is not the size it was forked at, the one asked for or else the
/// image's own: both sizes, in the row's own words.
// units.ts:152
// export function sizeGotLine(asked: WorkspaceSize, got: WorkspaceSize, named: boolean): string {
//   return `${named ? "asked for" : "the image's size is"} ${fmtSize(asked)}; the machine has ${fmtSize(got)}`;
// }
pub fn size_got_line(asked: &WorkspaceSize, got: &WorkspaceSize, named: bool) -> String {
    let asked_word = if named { "asked for" } else { "the image's size is" };
    format!("{asked_word} {}; the machine has {}", fmt_size(asked, None), fmt_size(got, None))
}

/// What a computer of the person's own is worth saying in one line: the cores and memory it has, and the room left
/// where its threads work. The one reading, so the screen a computer joins on and the row it lands in later cannot
/// describe the same computer differently. A computer that would not say how much room it has leaves that out. The
/// whole line is joined with no-break spaces, fmtSize's own rule carried on: it is one phrase about one computer.
// units.ts:160
// export function placeFactsLine(shape: WorkspaceSize, diskFreeBytes?: number): string {
//   const parts = [fmtSize(shape, "cores"), ...(diskFreeBytes === undefined ? [] : [`${fmtBytes(diskFreeBytes)} free`])];
//   return parts.join(", ").replace(/ /g, "\u00a0");
// }
pub fn place_facts_line(shape: &WorkspaceSize, disk_free_bytes: Option<f64>) -> String {
    let mut parts = vec![fmt_size(shape, Some(CpuWord::Cores))];
    if let Some(free) = disk_free_bytes {
        parts.push(format!("{} free", fmt_bytes(free)));
    }
    parts.join(", ").replace(' ', "\u{a0}")
}

/// The room a box has left, as the doctor reads it back: what the computer has, what the workspaces on it hold
/// right now, and what is left over. One line for the cores and one for the memory, built the same way so neither
/// can say a different thing about the same box; a computer that counts neither says nothing rather than a guess.
// units.ts:168
// export function boxRoomLines(capacity: Pick<PlaceCapacity, "cores" | "memMb" | "cpuTaken" | "memTakenMb">): string[] {
//   const { cpuTaken, memTakenMb } = capacity;
//   if (cpuTaken === undefined || memTakenMb === undefined) return [];
//   return [
//     `${fmtCpus(capacity.cores, "cores")}, ${cpuTaken} in use by forks, ${Math.max(0, capacity.cores - cpuTaken)} free`,
//     `${fmtMemGb(capacity.memMb)}, ${fmtMemGb(memTakenMb)} in use by forks, ${fmtMemGb(Math.max(0, capacity.memMb - memTakenMb))} free`,
//   ];
// }
pub fn box_room_lines(capacity: &PlaceCapacity) -> Vec<String> {
    let (Some(cpu_taken), Some(mem_taken)) = (&capacity.cpu_taken, &capacity.mem_taken_mb) else {
        return Vec::new();
    };
    let (cores, mem_mb) = (capacity.cores.0, capacity.mem_mb.0);
    let (cpu_taken, mem_taken) = (cpu_taken.0, mem_taken.0);
    vec![
        format!(
            "{}, {} in use by forks, {} free",
            fmt_cpus(cores, Some(CpuWord::Cores)),
            number_string(cpu_taken),
            number_string((cores - cpu_taken).max(0.0))
        ),
        format!("{}, {} in use by forks, {} free", fmt_mem_gb(mem_mb), fmt_mem_gb(mem_taken), fmt_mem_gb((mem_mb - mem_taken).max(0.0))),
    ]
}

/// What a row the computer already had says where the command answered from a directory that row's own road never
/// links into: the path it answered from, and the directories the road puts its commands in. The path is a fact the
/// read already has; the road that did answer is not named, since several roads link into the same directory and
/// naming one would be a guess. Read by the computers row, by the app's line for it and by the doctor.
// units.ts:181
// export const presentElsewhereLine = (bin: string, path: string, roadWords: string, bins: readonly string[]): string =>
//   `${bin} answers from ${path}, outside where ${roadWords.replace(/^(?:by|with|as|from) /, "")} puts it (${bins.join(", ")})`;
pub fn present_elsewhere_line(bin: &str, path: &str, road_words: &str, bins: &[impl AsRef<str>]) -> String {
    let road = ["by ", "with ", "as ", "from "].iter().find_map(|prefix| road_words.strip_prefix(prefix)).unwrap_or(road_words);
    let bins: Vec<&str> = bins.iter().map(|bin| bin.as_ref()).collect();
    format!("{bin} answers from {path}, outside where {road} puts it ({})", bins.join(", "))
}

/// What the doctor adds to a tool it found missing inside a workspace that the computer's own row read present:
/// the two readings disagree, which is either a tool that went off the computer since or one that answers from
/// somewhere no workspace can see, and the line that reads the computer again settles it.
// units.ts:187
// export const doctorComputerRowLine = (name: string, finishedAt: string): string =>
//   `the computers row read it present at ${finishedAt}; wsp add ${name} --update reads it again`;
pub fn doctor_computer_row_line(name: &str, finished_at: &str) -> String {
    format!("the computers row read it present at {finished_at}; wsp add {name} --update reads it again")
}

/// Why the folders of a provider are refused: a workspace there is forked fresh, so there is no computer standing
/// to browse, and a project there starts from a repository address or from a folder here, which seeds it.
// units.ts:192
// export const providerFoldersRefusal = (name: string): string =>
//   `${name} keeps no computer to browse, since every workspace there is forked fresh; add the project from a repository address, or from a folder on this computer, which seeds it`;
pub fn provider_folders_refusal(name: &str) -> String {
    format!("{name} keeps no computer to browse, since every workspace there is forked fresh; add the project from a repository address, or from a folder on this computer, which seeds it")
}

/// Why a doctor's computer road was refused for a row that is not a computer somebody joined: the computer the
/// host runs on and a cloud account are proved by the terminal that typed the line, since one reads that computer's
/// own files and the other forks a machine and bills.
// units.ts:198
// export const doctorRowRefusal = (name: string): string =>
//   `wsp doctor proves a computer you added on the host that holds its link; ${name} is proved at the terminal that typed the line`;
pub fn doctor_row_refusal(name: &str) -> String {
    format!("wsp doctor proves a computer you added on the host that holds its link; {name} is proved at the terminal that typed the line")
}

/// Why a second doctor on one computer is refused while the first is still going: two of them would make two
/// workspaces there and read the same tools twice.
// units.ts:203
// export const doctorRunningLine = (name: string): string => `the doctor is already proving ${name}; wait for it`;
pub fn doctor_running_line(name: &str) -> String {
    format!("the doctor is already proving {name}; wait for it")
}

/// What a host that serves the op without wiring the road answers: the road is the host's own, so a runtime served
/// without one has no computer to prove.
// units.ts:207
pub const DOCTOR_UNSERVED: &str = "the host that answered wired no doctor for the computers it holds";

/// Why a copy on the computer the host runs on was refused: the daemon binary staged beside this wsp is older than
/// the one this wsp needs, which is what an update that rebuilt the host and not the binary leaves. Read before the
/// first copy is made, so the binary's own usage text never reaches a person as the refusal.
// units.ts:212
// export const hereDaemonBehindLine = (version: number, host: number, fix: string): string =>
//   `this computer's wsp daemon is version ${version} and this wsp needs ${host}; ${fix} stages the right one`;
pub fn here_daemon_behind_line(version: f64, host: f64, fix: &str) -> String {
    format!(
        "this computer's wsp daemon is version {} and this wsp needs {}; {fix} stages the right one",
        number_string(version),
        number_string(host)
    )
}

/// What a child verb's failure that the verb itself never worded comes to: one sentence naming the verb and where
/// its output is, since a parse error's usage text and a signal say nothing to the person who asked for a copy.
// units.ts:217
// export const copyVerbFailedLine = (exitCode: number): string => `the daemon's copy verb failed (exit ${exitCode}); the host log has its output`;
pub fn copy_verb_failed_line(exit_code: f64) -> String {
    format!("the daemon's copy verb failed (exit {}); the host log has its output", number_string(exit_code))
}

/// What a machine wsp neither forks nor pays for costs, on its row's cost line and the Machine tab's Cost row.
// units.ts:220
pub const FREE_WORD: &str = "free";

/// Whether a place charges nothing an hour: a rate of nothing, and a rate nobody reported, both read free, since
/// neither is a figure to weigh against another. The one reading, so a caption that builds its own clauses and the
/// formatter below cannot disagree about which rows are free.
// units.ts:225
// export const chargesNothing = (usdPerHour: number | undefined): usdPerHour is undefined | 0 => usdPerHour === undefined || usdPerHour === 0;
pub fn charges_nothing(usd_per_hour: Option<f64>) -> bool {
    match usd_per_hour {
        None => true,
        Some(rate) => rate == 0.0,
    }
}

/// A rate a person is being quoted before they pick: a size on offer, a place's own hourly price. A place that
/// charges nothing reads free, since $0.00/hr beside a row a person is choosing between reads as a figure that has
/// not arrived rather than as the fact that nobody is billed. A rate that was metered rather than quoted, which is
/// what a place is burning this minute, keeps its figure and reads fmtRate.
// units.ts:231
// export const fmtPrice = (usdPerHour: number): string => (chargesNothing(usdPerHour) ? FREE_WORD : fmtRate(usdPerHour));
pub fn fmt_price(usd_per_hour: f64) -> String {
    if charges_nothing(Some(usd_per_hour)) {
        FREE_WORD.to_string()
    } else {
        fmt_rate(usd_per_hour)
    }
}

/// What a pane prints in the slot a reading would fill on a kind whose machines serve that reading on no road: the
/// Machine tab's Live rows and the Processes table both read it, in place of a pending that would never settle. The
/// kind table says which kinds serve which reading, so a pane says this within one probe window.
// units.ts:236
pub const NOT_ON_THIS_KIND: &str = "not on this kind";

/// A size as the --size flag and the fork tools spell it: vCPUs, an x, memory in GB ("2x4", "2x0.5").
// units.ts:239
// export function sizeWord(size: WorkspaceSize): string {
//   return `${size.cpu}x${memGb(size.memMb)}`;
// }
pub fn size_word(size: &WorkspaceSize) -> String {
    word_of(size.cpu.0, size.mem_mb.0)
}

/// The size a word names, or nothing when the word is not one.
// units.ts:244
// export function sizeFromWord(word: string): WorkspaceSize | undefined {
//   const m = /^(\d+)x(\d+(?:\.\d+)?)$/.exec(word.trim());
//   if (m === null) return undefined;
//   const cpu = Number(m[1]);
//   const memMb = Math.round(Number(m[2]) * 1024);
//   return cpu > 0 && memMb > 0 ? { cpu, memMb } : undefined;
// }
pub fn size_from_word(word: &str) -> Option<WorkspaceSize> {
    let m = Regex::new(r"^([0-9]+)x([0-9]+(?:\.[0-9]+)?)$").ok()?.captures(js_trim(word))?;
    let cpu: f64 = m[1].parse().ok()?;
    let mem_mb = js_round(m[2].parse::<f64>().ok()? * 1024.0);
    (cpu > 0.0 && mem_mb > 0.0).then_some(WorkspaceSize { cpu: JsNumber(cpu), mem_mb: JsNumber(mem_mb) })
}

/// The row a size names among the ones offered, with whatever that row carries beside the shape, or nothing where
/// the provider offers no such size. The one match, so a picker reading a size's rate and a refusal reading whether
/// it is offered cannot disagree about which row a size is.
// units.ts:255
// export function sizeOffer<T extends WorkspaceSize>(sizes: readonly T[], size: WorkspaceSize): T | undefined {
//   return sizes.find(s => s.cpu === size.cpu && s.memMb === size.memMb);
// }
/// A row that carries a machine's shape (`T extends WorkspaceSize`): a size, or an offer beside its rate.
pub trait Shape {
    fn cpu(&self) -> f64;
    fn mem_mb(&self) -> f64;
}

impl Shape for WorkspaceSize {
    fn cpu(&self) -> f64 {
        self.cpu.0
    }

    fn mem_mb(&self) -> f64 {
        self.mem_mb.0
    }
}

impl Shape for MachineSizeOffer {
    fn cpu(&self) -> f64 {
        self.cpu.0
    }

    fn mem_mb(&self) -> f64 {
        self.mem_mb.0
    }
}

/// The first row whose cpu and memMb both equal the size's.
pub fn size_offer<'a, T: Shape>(sizes: &'a [T], size: &WorkspaceSize) -> Option<&'a T> {
    sizes.iter().find(|offer| offer.cpu() == size.cpu.0 && offer.mem_mb() == size.mem_mb.0)
}

/// Whether a size is one the provider offers; the golden's own size is taken without this check.
// units.ts:260
// export function offeredSize(sizes: readonly WorkspaceSize[], size: WorkspaceSize): boolean {
//   return sizeOffer(sizes, size) !== undefined;
// }
pub fn offered_size<T: Shape>(sizes: &[T], size: &WorkspaceSize) -> bool {
    size_offer(sizes, size).is_some()
}

/// An awake rate in dollars an hour, at the fewest places that do not round the price: cents where cents are the
/// whole of it, three places where they are not, since a workspace at $0.018 an hour reads as $0.02 to the cent and
/// that is a fifth of the price. A third place that says nothing is not added: $0.09 is $0.09, not $0.090.
///
/// Which it is, is read off the rounded thousandth and never off the number itself: a provider computes its rate
/// (Solari charges per vCPU-hour plus per GB-hour), so a real one arrives as 0.09000000000000001, and any test of
/// the float against its own cent form calls that noise a third place. The one rate rule, read by the size refusal,
/// the places table, the size pickers and the provider rows alike.
// units.ts:272
// export function fmtRate(usdPerHour: number): string {
//   return `${fmtRateDollars(usdPerHour)}/hr`;
// }
pub fn fmt_rate(usd_per_hour: f64) -> String {
    format!("{}/hr", fmt_rate_dollars(usd_per_hour))
}

/// That rate's dollars alone, for a line that spells the hour its own way.
// units.ts:277
// export function fmtRateDollars(usdPerHour: number): string {
//   const mils = usdPerHour.toFixed(3);
//   return `$${mils.endsWith("0") ? usdPerHour.toFixed(2) : mils}`;
// }
pub fn fmt_rate_dollars(usd_per_hour: f64) -> String {
    let mils = to_fixed(usd_per_hour, 3);
    if mils.ends_with('0') {
        format!("${}", to_fixed(usd_per_hour, 2))
    } else {
        format!("${mils}")
    }
}

/// The one refusal every road gives a size the provider does not offer, malformed or merely absent: the word as it
/// was given, then every size that is offered with its rate. What happened, alone: each road joins its own fix to it
/// through refusalLine, since what to do about it is the road's (a flag at a terminal, a pick in the app).
// units.ts:285
// export function sizeRefusal(word: string, sizes: readonly MachineSizeOffer[]): string {
//   const offered = sizes.map(s => `${sizeWord(s)} (${fmtRate(s.rateUsdPerHour)})`).join(", ");
//   return `${word} is not a size this provider offers; the sizes are ${offered}`;
// }
pub fn size_refusal(word: &str, sizes: &[MachineSizeOffer]) -> String {
    let offered = sizes
        .iter()
        .map(|s| format!("{} ({})", word_of(s.cpu.0, s.mem_mb.0), fmt_rate(s.rate_usd_per_hour.0)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{word} is not a size this provider offers; the sizes are {offered}")
}

/// What to do about such a size on a road with no flag to name: the app's picker and the wire both ask for one of
/// the sizes the half above just listed. The command line names its own flag instead.
// units.ts:292
pub const SIZE_PICK_FIX: &str = "Ask for one of those instead.";

/// The one refusal a create or a fork gives when the provider is at its machine cap and no builder was left to stop
/// for room: the machines this host knows hold the slots, and the two moves that free one. The provider's own
/// sentence ("Too many concurrent sessions") names nothing a person can act on. A builder is named as one, since
/// the pause it is offered beside is a workspace's move; the runtime stops the builders it may before it asks here.
/// Only the holders are claimed: the plan's cap is nowhere in Capabilities, and a slot held by a machine this host
/// cannot name still counts against it, so no branch but the two-holder line the ticket wrote totals the slots.
// units.ts:300
// export function machineCapRefusal(workspaces: readonly string[], builders: readonly string[] = [], said?: string): string {
//   const holders = [...workspaces, ...builders.map(name => `${name} (builder)`)];
//   // A refusal read as the cap by its status alone, its code one nobody listed, keeps the provider's words.
//   const words = said === undefined || said === "" ? "" : ` The provider said: ${said}`;
//   if (holders.length === 0) return `the provider is at its machine cap and no machine of this computer holds a slot; free one at the provider and try again${words === "" ? "" : `.${words}`}`;
//   const slots = holders.length === 1 ? "a machine slot is" : holders.length === 2 ? "both machine slots are" : "machine slots are";
//   return `${slots} in use: ${holders.join(", ")}. Pause ${holders.length === 1 ? "it" : "one"} or wait for a nap.${words}`;
// }
/// `builders` defaults to none.
pub fn machine_cap_refusal(workspaces: &[impl AsRef<str>], builders: &[impl AsRef<str>], said: Option<&str>) -> String {
    let mut holders: Vec<String> = workspaces.iter().map(|w| w.as_ref().to_string()).collect();
    holders.extend(builders.iter().map(|b| format!("{} (builder)", b.as_ref())));
    let words = match said {
        None | Some("") => String::new(),
        Some(said) => format!(" The provider said: {said}"),
    };
    match holders.len() {
        0 => {
            let period = if words.is_empty() { String::new() } else { format!(".{words}") };
            format!("the provider is at its machine cap and no machine of this computer holds a slot; free one at the provider and try again{period}")
        }
        held => {
            let slots = match held {
                1 => "a machine slot is",
                2 => "both machine slots are",
                _ => "machine slots are",
            };
            let pause = if held == 1 { "it" } else { "one" };
            format!("{slots} in use: {}. Pause {pause} or wait for a nap.{words}", holders.join(", "))
        }
    }
}

/// Boat's three windows of starts: a create, a fork, a deploy from a named snapshot and a resume each count once.
// units.ts:310
// export type StartWindow = "minute" | "hour" | "day";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StartWindow {
    #[serde(rename = "minute")]
    Minute,
    #[serde(rename = "hour")]
    Hour,
    #[serde(rename = "day")]
    Day,
}

impl StartWindow {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            StartWindow::Minute => "minute",
            StartWindow::Hour => "hour",
            StartWindow::Day => "day",
        }
    }
}

/// A start Boat refused because one window's starts are used, with the account's own numbers off `GET /limits`.
/// Boat publishes no reset time and its windows roll, so no line names a clock time. `what` is the machine's name.
// units.ts:314
// export function startsUsedLine(window: StartWindow, used: number, limit: number, plan: string | undefined, what: string | undefined): string {
//   const head = `Boat's starts for ${window === "day" ? "today" : `this ${window}`} are used (${used} of ${limit}${plan === undefined ? "" : ` on ${plan}`}), so ${what === undefined || what === "" ? "the machine" : what} was not started`;
//   if (window === "minute") return `${head}; start it again in a minute.`;
//   if (window === "hour") return `${head}; start it again later in the hour, or move to a plan with more at boat.dev/dashboard.`;
//   return `${head}; Boat counts the day again tomorrow, or a plan with more starts is at boat.dev/dashboard.`;
// }
pub fn starts_used_line(window: StartWindow, used: f64, limit: f64, plan: Option<&str>, what: Option<&str>) -> String {
    let when = if window == StartWindow::Day { "today".to_string() } else { format!("this {}", window.as_str()) };
    let plan = plan.map_or(String::new(), |plan| format!(" on {plan}"));
    let what = match what {
        None | Some("") => "the machine",
        Some(what) => what,
    };
    let head =
        format!("Boat's starts for {when} are used ({} of {}{plan}), so {what} was not started", number_string(used), number_string(limit));
    match window {
        StartWindow::Minute => format!("{head}; start it again in a minute."),
        StartWindow::Hour => format!("{head}; start it again later in the hour, or move to a plan with more at boat.dev/dashboard."),
        StartWindow::Day => format!("{head}; Boat counts the day again tomorrow, or a plan with more starts is at boat.dev/dashboard."),
    }
}

/// The plan a start line names: the trial, or the plan by its monthly price.
// units.ts:322
// export const boatPlanWords = (trial: boolean, usd: number | undefined): string | undefined => (trial ? "the trial" : usd === undefined ? undefined : `the $${usd} plan`);
pub fn boat_plan_words(trial: bool, usd: Option<f64>) -> Option<String> {
    if trial {
        Some("the trial".to_string())
    } else {
        usd.map(|usd| format!("the ${} plan", number_string(usd)))
    }
}

// units.ts:324
pub const BOAT_BALANCE_LINE: &str = "Boat's balance is used up, so no machine starts; add time at boat.dev/dashboard. Boat stops running machines 24 hours after the balance ran out.";

// units.ts:325
pub const BOAT_MEMBER_CAP_LINE: &str = "this Boat organization caps your usage; ask its owner to raise your cap.";

// units.ts:326
pub const BOAT_NO_CAPACITY_LINE: &str = "Boat has no free machines of this size right now; try again in a few minutes, or a smaller size.";

// units.ts:327
pub const BOAT_REQUEST_LIMIT_LINE: &str = "Boat is refusing requests from this computer's address, past its limit of 1,000 a minute, and still was after waiting and asking again; try again in a minute.";

/// The host log's line for a call Boat refused under its request limit, asked again after the wait Boat named.
// units.ts:329
// export const boatRequestLimitWaitLine = (call: string, waitMs: number, tryNumber: number, tries: number): string =>
//   `${call} met Boat's limit of 1,000 requests a minute from one address; asking again in ${Math.ceil(waitMs / 1000)}s, try ${tryNumber} of ${tries}`;
pub fn boat_request_limit_wait_line(call: &str, wait_ms: f64, try_number: f64, tries: f64) -> String {
    format!(
        "{call} met Boat's limit of 1,000 requests a minute from one address; asking again in {}s, try {} of {}",
        number_string((wait_ms / 1000.0).ceil()),
        number_string(try_number),
        number_string(tries)
    )
}

/// A resume Boat refused because the box sat stopped past the 14 days it keeps a disk.
// units.ts:332
pub const BOAT_DISK_GONE_LINE: &str = "Boat deleted this machine's disk once it had been paused for 14 days, so it cannot wake";

/// A sub-thread whose cloud deleted its machine's disk: released, and its next send starts a new machine.
// units.ts:335
// export const diskGoneReleasedLine = (said: string): string => `${said}; the next send starts a new machine from the image`;
pub fn disk_gone_released_line(said: &str) -> String {
    format!("{said}; the next send starts a new machine from the image")
}

/// A workspace whose cloud deleted its machine's disk, moved onto a new machine by the wake that met it.
// units.ts:337
// export const diskGoneReplacedLine = (said: string): string => `${said}; it runs on a new machine from the image now`;
pub fn disk_gone_replaced_line(said: &str) -> String {
    format!("{said}; it runs on a new machine from the image now")
}

/// The status a thread reads once its napping machine is past the days its cloud promises to keep the disk.
// units.ts:339
pub const DISK_EXPIRED_WORD: &str = "Expired";

/// What an expired thread's card says, the date as the surface writes it: Boat measured whole at day 18, so a wake
/// may still find the disk there.
// units.ts:342
// export const diskExpiredLine = (when: string): string => `Its machine's disk was kept until ${when}, so a send may find it gone and start a new machine`;
pub fn disk_expired_line(when: &str) -> String {
    format!("Its machine's disk was kept until {when}, so a send may find it gone and start a new machine")
}

/// Whether a thread reads expired: its napping machine is past the keep of its disk, and no turn of it runs or asks.
// units.ts:344
// export const diskExpired = (t: { diskKeptUntil?: number | undefined; status: string; asking?: string | null | undefined }, now: number): boolean =>
//   t.diskKeptUntil !== undefined && t.diskKeptUntil <= now && t.status !== "running" && (t.asking ?? undefined) === undefined;
/// The three fields of a thread `disk_expired` reads; `asking` None stands for both undefined and null.
#[derive(Clone, Debug, PartialEq)]
pub struct DiskKeep<'a> {
    pub disk_kept_until: Option<f64>,
    pub status: &'a str,
    pub asking: Option<&'a str>,
}

pub fn disk_expired(t: &DiskKeep<'_>, now: f64) -> bool {
    t.disk_kept_until.is_some_and(|kept_until| kept_until <= now) && t.status != "running" && t.asking.is_none()
}

/// A create or a fork Boat could not get a machine for: it removed the box, reported cancelled once, then 404.
// units.ts:348
// export function boatCancelledLine(what: string | undefined, said: string | undefined): string {
//   return `Boat could not get a machine${what === undefined ? "" : ` for ${what}`} and cancelled the create${said === undefined || said === "" ? "" : ` (${said})`}; start it again.`;
// }
pub fn boat_cancelled_line(what: Option<&str>, said: Option<&str>) -> String {
    let for_what = what.map_or(String::new(), |what| format!(" for {what}"));
    let said = match said {
        None | Some("") => String::new(),
        Some(said) => format!(" ({said})"),
    };
    format!("Boat could not get a machine{for_what} and cancelled the create{said}; start it again.")
}

/// A create's line as the box Boat made for it moves through Boat's own states.
// units.ts:353
// export const boatBoxStateLine = (id: string, state: string): string => `Boat's machine ${id} is ${state}`;
pub fn boat_box_state_line(id: &str, state: &str) -> String {
    format!("Boat's machine {id} is {state}")
}

/// The host log's line for a call Boat refused: the status, Boat's code and request id, and the first line of its words.
// units.ts:356
// export function boatRefusedLogLine(call: string, status: number, code: string | undefined, requestId: string | undefined, said: string): string {
//   const end = said.indexOf("\n");
//   return `Boat refused ${call}: ${status} code=${code ?? "none"} requestId=${requestId ?? "none"}: ${(end === -1 ? said : said.slice(0, end)).slice(0, 300)}`;
// }
pub fn boat_refused_log_line(call: &str, status: f64, code: Option<&str>, request_id: Option<&str>, said: &str) -> String {
    let first_line = said.split('\n').next().unwrap_or(said);
    format!(
        "Boat refused {call}: {} code={} requestId={}: {}",
        number_string(status),
        code.unwrap_or("none"),
        request_id.unwrap_or("none"),
        utf16::cut(first_line, 300)
    )
}

/// How a duration reads: short is the chat footer's and the notify line's ("1.5s", "8m 12s"), clock is the cut
/// line's ("15m 00s", "1h 00m 00s").
// units.ts:363
// export type DurationStyle = "short" | "clock";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DurationStyle {
    #[serde(rename = "short")]
    Short,
    #[serde(rename = "clock")]
    Clock,
}

impl DurationStyle {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            DurationStyle::Short => "short",
            DurationStyle::Clock => "clock",
        }
    }
}

// units.ts:365
// private in the TS: port it as a private helper
// const pad2 = (n: number): string => String(n).padStart(2, "0");

// The short style under a minute: ms under a second, tenths under ten, whole seconds after; nothing sensible is "0ms".
// units.ts:368
// private in the TS: port it as a private helper
// function shortSeconds(ms: number): string {
//   if (!Number.isFinite(ms) || ms < 0) return "0ms";
//   if (ms < 1_000) return `${Math.max(1, Math.round(ms))}ms`;
//   if (ms < 10_000) {
//     const tenths = Math.round(ms / 100) / 10;
//     return tenths >= 10 ? "10s" : `${tenths.toFixed(1)}s`;
//   }
//   return `${Math.round(ms / 1_000)}s`;
// }

/// How long a turn ran. Short: ms under a second, tenths under ten, whole seconds under a minute, then minutes and
/// seconds. Clock: minutes and two-digit seconds, whole hours ahead once there are any.
// units.ts:380
// export function fmtDuration(ms: number, style: DurationStyle = "short"): string {
//   if (style === "short" && (ms < 60_000 || !Number.isFinite(ms))) return shortSeconds(ms);
//   const total = Number.isFinite(ms) && ms > 0 ? Math.round(ms / 1_000) : 0;
//   const hours = Math.floor(total / 3_600);
//   const minutes = Math.floor((total % 3_600) / 60);
//   const seconds = total % 60;
//   if (style === "short") return seconds === 0 ? `${hours * 60 + minutes}m` : `${hours * 60 + minutes}m ${seconds}s`;
//   return hours === 0 ? `${minutes}m ${pad2(seconds)}s` : `${hours}h ${pad2(minutes)}m ${pad2(seconds)}s`;
// }
pub fn fmt_duration(ms: f64, style: Option<DurationStyle>) -> String {
    let style = style.unwrap_or(DurationStyle::Short);
    if style == DurationStyle::Short && (ms < 60_000.0 || !ms.is_finite()) {
        return short_seconds(ms);
    }
    let total = if ms.is_finite() && ms > 0.0 { js_round(ms / 1000.0) } else { 0.0 };
    let hours = (total / 3600.0).floor();
    let minutes = ((total % 3600.0) / 60.0).floor();
    let seconds = total % 60.0;
    match style {
        DurationStyle::Short if seconds == 0.0 => format!("{}m", number_string(hours * 60.0 + minutes)),
        DurationStyle::Short => format!("{}m {}s", number_string(hours * 60.0 + minutes), number_string(seconds)),
        DurationStyle::Clock if hours == 0.0 => format!("{}m {:02}s", number_string(minutes), seconds as u64),
        DurationStyle::Clock => format!("{}h {:02}m {:02}s", number_string(hours), minutes as u64, seconds as u64),
    }
}

/// A token count as a reply's footer reads it: whole under a thousand, then thousands and millions to one place,
/// with no trailing zero place.
// units.ts:392
// export function fmtTokens(n: number): string {
//   const count = Number.isFinite(n) && n > 0 ? Math.round(n) : 0;
//   if (count < 1_000) return String(count);
//   // Three figures in the unit, the decimals that leaves and no trailing zero: 4.27k, 22.6M, 7.37B, 687M.
//   const [value, unit] = count < 999_500 ? [count / 1_000, "k"] : count < 999_500_000 ? [count / 1_000_000, "M"] : [count / 1_000_000_000, "B"];
//   return `${Number(value.toFixed(value < 10 ? 2 : value < 100 ? 1 : 0))}${unit}`;
// }
pub fn fmt_tokens(n: f64) -> String {
    let count = if n.is_finite() && n > 0.0 { js_round(n) } else { 0.0 };
    if count < 1_000.0 {
        return number_string(count);
    }
    let (value, unit) = if count < 999_500.0 {
        (count / 1_000.0, "k")
    } else if count < 999_500_000.0 {
        (count / 1_000_000.0, "M")
    } else {
        (count / 1_000_000_000.0, "B")
    };
    let digits = if value < 10.0 {
        2
    } else if value < 100.0 {
        1
    } else {
        0
    };
    format!("{}{unit}", number_string(to_fixed(value, digits).parse().unwrap_or(f64::NAN)))
}

/// How long a machine has been up, as the Machine tab reads it: minutes under an hour, hours and minutes under a
/// day, then days and hours. Coarser than a turn's duration on purpose: the figure is read once, not watched.
// units.ts:402
// export function fmtUptime(ms: number): string {
//   const minutes = Number.isFinite(ms) && ms > 0 ? Math.floor(ms / 60_000) : 0;
//   if (minutes < 60) return `${minutes}m`;
//   const hours = Math.floor(minutes / 60);
//   if (hours < 24) return `${hours}h ${minutes % 60}m`;
//   return `${Math.floor(hours / 24)}d ${hours % 24}h`;
// }
pub fn fmt_uptime(ms: f64) -> String {
    let minutes = if ms.is_finite() && ms > 0.0 { (ms / 60_000.0).floor() } else { 0.0 };
    if minutes < 60.0 {
        return format!("{}m", number_string(minutes));
    }
    let hours = (minutes / 60.0).floor();
    if hours < 24.0 {
        return format!("{}h {}m", number_string(hours), number_string(minutes % 60.0));
    }
    format!("{}d {}h", number_string((hours / 24.0).floor()), number_string(hours % 24.0))
}

/// A running clock on a row redrawn every tick: whole seconds, then the short style's minutes and seconds; tenths
/// would flicker.
// units.ts:412
// export function fmtElapsed(ms: number): string {
//   const seconds = Number.isFinite(ms) && ms > 0 ? Math.floor(ms / 1_000) : 0;
//   return seconds < 60 ? `${seconds}s` : fmtDuration(seconds * 1_000);
// }
pub fn fmt_elapsed(ms: f64) -> String {
    let seconds = if ms.is_finite() && ms > 0.0 { (ms / 1_000.0).floor() } else { 0.0 };
    if seconds < 60.0 {
        format!("{}s", number_string(seconds))
    } else {
        fmt_duration(seconds * 1_000.0, None)
    }
}

/// Every spend figure a person reads, in one shape: cents, whatever the size. Three figures in one thread used to
/// read $0.51, $0.22 and $0.0000, and a reader cannot tell at a glance that the third is the smallest of them. A
/// turn under a cent reads $0.00, which is what it costs to the cent.
// units.ts:420
// export function fmtCost(usd: number): string {
//   return `$${usd.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
// }
pub fn fmt_cost(usd: f64) -> String {
    format!("${}", locale_number(usd, Grouping::EnUs, 2, 2))
}

/// Rupees as India writes them, lakh grouping and the ₹ sign: ₹1,44,740, paise only where the value has them.
// units.ts:425
// export function fmtInr(inr: number): string {
//   const paise = Math.round(inr * 100) % 100 !== 0;
//   return inr.toLocaleString("en-IN", { style: "currency", currency: "INR", minimumFractionDigits: paise ? 2 : 0, maximumFractionDigits: paise ? 2 : 0 });
// }
pub fn fmt_inr(inr: f64) -> String {
    let digits = if js_round(inr * 100.0) % 100.0 != 0.0 { 2 } else { 0 };
    let sign = if inr.is_sign_negative() { "-" } else { "" };
    format!("{sign}\u{20b9}{}", locale_number(inr.abs(), Grouping::EnIn, digits, digits))
}

/// The word beside a figure nobody is billed for: what the agent's own table lists for the tokens a turn spent, on
/// a computer whose turns run on the person's own sign-in.
// units.ts:432
pub const LIST_PRICE_WORD: &str = "list price";

/// Two byte counts against each other with the unit said once when they share it: "3.6 of 3.9 GB", "900.0 MB of 3.9 GB".
// units.ts:435
// export function fmtBytesOf(used: number, total: number, fmt: (n: number) => string = fmtBytes): string {
//   const t = fmt(total);
//   const u = fmt(used);
//   const unit = t.slice(t.lastIndexOf(" "));
//   return `${u.endsWith(unit) ? u.slice(0, -unit.length) : u} of ${t}`;
// }
pub fn fmt_bytes_of(used: f64, total: f64, fmt: Option<fn(f64) -> String>) -> String {
    let fmt = fmt.unwrap_or(fmt_bytes);
    let t = fmt(total);
    let u = fmt(used);
    let unit = &t[t.rfind(' ').unwrap_or(t.len() - 1)..];
    let u = u.strip_suffix(unit).unwrap_or(u.as_str());
    format!("{u} of {t}")
}

fn bytes_in(n: f64, k: f64) -> String {
    if n < k {
        return format!("{} B", number_string(n));
    }
    if n < k * k {
        return format!("{} KB", number_string(js_round(n / k)));
    }
    if n < k * k * k {
        return format!("{} MB", number_string(js_round(n / (k * k))));
    }
    let gb = n / (k * k * k);
    let one_place = to_fixed(gb, 1);
    if one_place.parse::<f64>().is_ok_and(|x| x.fract() == 0.0) {
        format!("{} GB", number_string(js_round(gb)))
    } else {
        format!("{one_place} GB")
    }
}

/// `Number((memMb / 1024).toFixed(1))`: the figure the size table names, one decimal at most.
fn mem_gb(mem_mb: f64) -> f64 {
    to_fixed(mem_mb / 1024.0, 1).parse().unwrap_or(f64::NAN)
}

/// `sizeWord`'s text for a cpu count and a memory figure in MB.
fn word_of(cpu: f64, mem_mb: f64) -> String {
    format!("{}x{}", number_string(cpu), number_string(mem_gb(mem_mb)))
}

/// The id a screen's `id` field carries on the wire, which a draft's `at` names.
fn screen_word(id: InitScreenId) -> &'static str {
    match id {
        InitScreenId::Agents => "agents",
        InitScreenId::Tools => "tools",
        InitScreenId::Also => "also",
        InitScreenId::Logins => "logins",
        InitScreenId::Wsp => "wsp",
    }
}

/// The short style under a minute: ms under a second, tenths under ten, whole seconds after; nothing sensible is "0ms".
fn short_seconds(ms: f64) -> String {
    if !ms.is_finite() || ms < 0.0 {
        return "0ms".to_string();
    }
    if ms < 1_000.0 {
        return format!("{}ms", number_string(1f64.max(js_round(ms))));
    }
    if ms < 10_000.0 {
        let tenths = js_round(ms / 100.0) / 10.0;
        return if tenths >= 10.0 { "10s".to_string() } else { format!("{}s", to_fixed(tenths, 1)) };
    }
    format!("{}s", number_string(js_round(ms / 1_000.0)))
}
