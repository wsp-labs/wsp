// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/thread.ts:1-445 at a3b870a6e: a cut turn, a harness that ended early, the host's
//! validator refusal, machine, disk and resize lines, the words every door quotes about turns, and the two-halves
//! refusal line with redaction. Each TS statement stands as a comment above its stub. See spec/thread_lines.md.

use wsp_wire::wire::capabilities::WorkspaceSize;
use crate::js::{is_js_whitespace, js_collapse_whitespace, js_round, js_trim, js_trim_end, number_string, utf16};
use crate::words::base::plural;
use crate::words::units::{fmt_bytes_of, fmt_disk_bytes, fmt_duration, fmt_mem_gb, fmt_uptime, size_word, DurationStyle};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::shell_quote::shell_line;


/// A limit as one unit: whole hours when it is hours, else whole minutes.
// thread.ts:9
// export function fmtLimit(ms: number): string {
//   return ms >= 3_600_000 && ms % 3_600_000 === 0 ? `${ms / 3_600_000}h` : `${Math.round(ms / 60_000)}m`;
// }
pub fn fmt_limit(ms: f64) -> String {
    if ms >= 3_600_000.0 && ms % 3_600_000.0 == 0.0 {
        format!("{}h", number_string(ms / 3_600_000.0))
    } else {
        format!("{}m", number_string(js_round(ms / 60_000.0)))
    }
}

/// Which rule ended a turn: idle is the turn doing nothing at all for the limit, TURN_IDLE_MS's own rule, and wall
/// is the turn limit set on the computer it ran on.
// thread.ts:15
// export type TurnCutRule = "idle" | "wall";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TurnCutRule {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "wall")]
    Wall,
}

impl TurnCutRule {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            TurnCutRule::Idle => "idle",
            TurnCutRule::Wall => "wall",
        }
    }
}

/// The one line every client shows for a turn the runtime cut: which rule, how long the turn ran, the limit.
// thread.ts:18
// export function turnCutLine(rule: TurnCutRule, elapsedMs: number, limitMs: number): string {
//   return rule === "idle"
//     ? `stopped after ${fmtDuration(elapsedMs, "clock")} with no output for ${fmtLimit(limitMs)}`
//     : `stopped after ${fmtDuration(elapsedMs, "clock")} at the ${fmtLimit(limitMs)} turn limit; send to continue where it stopped, or change the limit in Settings > Computers`;
// }
pub fn turn_cut_line(rule: TurnCutRule, elapsed_ms: f64, limit_ms: f64) -> String {
    let elapsed = fmt_duration(elapsed_ms, Some(DurationStyle::Clock));
    let limit = fmt_limit(limit_ms);
    match rule {
        TurnCutRule::Idle => format!("stopped after {elapsed} with no output for {limit}"),
        TurnCutRule::Wall => format!(
            "stopped after {elapsed} at the {limit} turn limit; send to continue where it stopped, or change the limit in Settings > Computers"
        ),
    }
}

/// An install step the guard ended at its road's limit: the seconds, and that it was the second time when it was.
// thread.ts:25
// export function timedOutLine(limitS: number, times = 1): string {
//   return `timed out after ${limitS}s${times === 2 ? ", twice" : ""}`;
// }
/// `times` defaults to 1.
pub fn timed_out_line(limit_s: f64, times: Option<f64>) -> String {
    let twice = if times == Some(2.0) { ", twice" } else { "" };
    format!("timed out after {}s{twice}", number_string(limit_s))
}

/// The step's line when its road's limit ended it and the step is run once more: a download that ran the clock out
/// was a dead read, and the words say so before the second run starts.
// thread.ts:31
// export function stepRetryLine(limitS: number): string {
//   return `${timedOutLine(limitS)}; trying once more`;
// }
pub fn step_retry_line(limit_s: f64) -> String {
    format!("{}; trying once more", timed_out_line(limit_s, None))
}

/// The turn's error when the harness process ended before any result, in the words of what actually ended it rather
/// than in a number nobody can act on. Exit 127 is the shell saying the binary was not on PATH, so the line names the
/// binary and the PATH the launch exported, or that it exported none. A signal is the process being killed, named.
/// A code the run has is the code. No code at all is a run whose leader is gone without leaving one, which is a kill
/// the road could not name; where nothing of the agent ever came back, the launch never reached it and says so.
// thread.ts:40
// export function harnessExitLine(bin: string, exitCode: number | null, path: string | undefined, ended: HarnessEnded = {}): string {
//   if (exitCode === 127) {
//     const searched = path === undefined ? "the launch exported no PATH, the machine's own was searched" : `PATH searched: ${path}`;
//     return `${bin} was not found on PATH (exit 127); ${searched}`;
//   }
//   if (ended.signal !== undefined) return `${bin} was killed (${ended.signal}) before it answered`;
//   if (exitCode !== null) return `${bin} exited with code ${String(exitCode)} before emitting a result`;
//   if (ended.reached === false) return `the launch never reached ${bin}: its run ended before the agent said a word`;
//   return `${bin} was killed before it answered`;
// }
/// `exitCode` None is null; `ended` defaults to `{}`.
pub fn harness_exit_line(bin: &str, exit_code: Option<i32>, path: Option<&str>, ended: Option<&HarnessEnded>) -> String {
    let none = HarnessEnded::default();
    let ended = ended.unwrap_or(&none);
    if exit_code == Some(127) {
        let searched = match path {
            None => "the launch exported no PATH, the machine's own was searched".to_string(),
            Some(path) => format!("PATH searched: {path}"),
        };
        return format!("{bin} was not found on PATH (exit 127); {searched}");
    }
    if let Some(signal) = &ended.signal {
        return format!("{bin} was killed ({signal}) before it answered");
    }
    if let Some(code) = exit_code {
        return format!("{bin} exited with code {code} before emitting a result");
    }
    if ended.reached == Some(false) {
        return format!("the launch never reached {bin}: its run ended before the agent said a word");
    }
    format!("{bin} was killed before it answered")
}

/// What the road that watched the run knows about how it ended, beyond the code: the signal that ended it where it
/// saw one, and whether the agent ever announced itself. A road that knows neither leaves both out and the code
/// stands on its own.
// thread.ts:54
// export interface HarnessEnded {
//   /** The signal's own name, as the computer running the process spells it. */
//   readonly signal?: string;
//   /** False says nothing of the agent ever came back, so the run ended before the launch became a turn. */
//   readonly reached?: boolean;
// }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HarnessEnded {
    /// The signal's own name, as the computer running the process spells it.
    pub signal: Option<String>,
    /// False says nothing of the agent ever came back.
    pub reached: Option<bool>,
}

// What a person's line calls each field a request carries, for the refusal that has to name one. The wire's own
// names are never printed: a person types flags and words, not fields. A field with no row here is one no line
// names on its own, and the refusal says the line instead.
// thread.ts:64
// private in the TS: port it as a private helper
// const REQUEST_WORDS: Readonly<Record<string, string>> = {
//   threadId: "the thread",
//   sessionId: "the thread",
//   argv: "the command",
//   prompt: "the message",
//   cwd: "--cwd",
//   model: "--model",
//   effort: "--effort",
//   permissionMode: "--access",
// };

/// What the host would not read, out of the refusal its own validator answers a request with. The list carries the
/// wire's field names and every op the host serves, so none of it reaches a person: what comes back is the one thing
/// they can act on, which of their arguments the host refused, or that this wsp and the host are different builds.
/// Nothing when the refusal is a sentence, which is every refusal wsp writes itself.
// thread.ts:79
// export function validatorRefusal(error: string): string | undefined {
//   let issues: unknown;
//   try {
//     issues = JSON.parse(error);
//   } catch {
//     return undefined;
//   }
//   if (!Array.isArray(issues) || issues.length === 0) return undefined;
//   const rows = issues.map(issue => (typeof issue === "object" && issue !== null ? (issue as { code?: unknown; path?: unknown }) : {}));
//   if (!rows.every(row => typeof row.code === "string" && Array.isArray(row.path))) return undefined;
//   const fields = rows.map(row => (row.path as unknown[])[0]).filter((field): field is string => typeof field === "string");
//   // A discriminator the host does not know is this wsp asking for an op the host does not serve, which no argument
//   // of the line can fix: the two builds differ.
//   if (rows.some(row => row.code === "invalid_union_discriminator") && fields.every(field => field === "op")) {
//     return "the host does not serve this line; it runs another version of wsp, restart it with wsp restart";
//   }
//   const named = [...new Set(fields.map(field => REQUEST_WORDS[field]).filter((word): word is string => word !== undefined))];
//   return named.length === 0 ? "the host would not read this line" : `the host would not read ${named.join(" and ")} on this line`;
// }
pub fn validator_refusal(error: &str) -> Option<String> {
    let issues: serde_json::Value = serde_json::from_str(error).ok()?;
    let issues = issues.as_array().filter(|rows| !rows.is_empty())?;
    let rows: Vec<(&str, &Vec<serde_json::Value>)> = issues
        .iter()
        .map(|row| Some((row.get("code")?.as_str()?, row.get("path")?.as_array()?)))
        .collect::<Option<_>>()?;
    let fields: Vec<&str> = rows.iter().filter_map(|(_, path)| path.first()?.as_str()).collect();
    if rows.iter().any(|(code, _)| *code == "invalid_union_discriminator") && fields.iter().all(|field| *field == "op") {
        return Some("the host does not serve this line; it runs another version of wsp, restart it with wsp restart".to_string());
    }
    let mut named: Vec<&str> = Vec::new();
    for word in fields.iter().filter_map(|field| request_word(field)) {
        if !named.contains(&word) {
            named.push(word);
        }
    }
    if named.is_empty() {
        Some("the host would not read this line".to_string())
    } else {
        Some(format!("the host would not read {} on this line", named.join(" and ")))
    }
}

/// The host's one line when it asked the person's login shell for their PATH and got none back: why, and that the
/// PATH the launch handed it stands. A host started from Finder or the Dock has only launchd's four system folders,
/// so this line is what says why no agent of theirs was found afterwards.
// thread.ts:102
// export function loginPathLine(why: string): string {
//   return `login shell: no PATH read (${why}); this host keeps the PATH it was started with`;
// }
pub fn login_path_line(why: &str) -> String {
    format!("login shell: no PATH read ({why}); this host keeps the PATH it was started with")
}

/// The turn's error when a host that came back looked for the turn's run on the machine and the machine no longer
/// holds it: the run's files were swept, so nothing the agent did while the host was away can be read back.
// thread.ts:108
pub const RUN_GONE_LINE: &str = "the machine no longer holds this turn's run, so nothing of it can be read back";

/// The end of a turn a stop settled itself, where no agent was there to say it stopped: a turn a restart left
/// running with no road to its process, or one on a computer that did not answer.
// thread.ts:112
pub const TURN_STOPPED_LINE: &str = "stopped";

/// What a command waiting on a turn is told when the host it asked stops: the run is the machine's, not the host's,
/// so it goes on and its reply lands in the thread whether or not this command is still there to see it.
// thread.ts:116
pub const HOST_STOPPING_LINE: &str = "the host is restarting; the turn goes on and its reply lands in the thread";

/// What a send is told when the host stopped before it took the message and no host took it after: no turn runs and
/// nothing waits in the thread, so the person has to send it again.
// thread.ts:120
pub const NOT_DELIVERED_LINE: &str = "the host stopped before it took the message, so it was not delivered; send it again";

/// What a machine is called when the provider reports it running and the road every command takes is dead: whose
/// machine it is, which one, and the guest's own words, so the failure reads as the provider's and not as wsp's.
// thread.ts:124
// export function guestUnusableLine(provider: string, machineId: string, detail: string): string {
//   return `${provider} left ${machineId} running but nothing on it can run: ${detail}`;
// }
pub fn guest_unusable_line(provider: &str, machine_id: &str, detail: &str) -> String {
    format!("{provider} left {machine_id} running but nothing on it can run: {detail}")
}

/// Why nothing runs on a box whose work folder is gone: the provider starts every command there, the wake's own
/// check included, so only a fresh machine gets past it.
// thread.ts:130
// export function noWorkFolderLine(provider: string, folder: string, said: string): string {
//   return `${folder} is missing, and ${provider} runs every command there (it said: ${said}); a wake cannot make it again, so a rebuild is the way out, and work not pushed is lost with the old disk`;
// }
pub fn no_work_folder_line(provider: &str, folder: &str, said: &str) -> String {
    format!(
        "{folder} is missing, and {provider} runs every command there (it said: {said}); a wake cannot make it again, so a rebuild is the way out, and work not pushed is lost with the old disk"
    )
}

/// What a machine is called when the provider answers that it cannot reach it: the provider's own words, then the two
/// roads open on a running machine. A wake of a running machine changes nothing and a rebuild is refused while the
/// machine is only not answering, so neither is named; the machine's id is nothing a person acts on.
// thread.ts:137
// export function machineUnreachableLine(said: string): string {
//   return `the provider cannot reach the machine (${said}); pause and wake the workspace, or delete it`;
// }
pub fn machine_unreachable_line(said: &str) -> String {
    format!("the provider cannot reach the machine ({said}); pause and wake the workspace, or delete it")
}

/// The daemon's link came back while the provider's exec stayed down (2026-09-23), and its retryable 502 is documented for pause, not exec, so wait comes first.
// thread.ts:142
// export function execFailedLine(said: string): string {
//   return `the provider cannot run commands on the machine (${said}) while the machine and its threads may still be running; wait, or pause and wake the workspace, or delete it`;
// }
pub fn exec_failed_line(said: &str) -> String {
    format!(
        "the provider cannot run commands on the machine ({said}) while the machine and its threads may still be running; wait, or pause and wake the workspace, or delete it"
    )
}

/// A pause the provider refuses outright: its words, the cause measured on Solari (about 10 GB of memory and disk together), the road left.
// thread.ts:147
// export function napRefusedLine(said: string): string {
//   return `the provider does not pause this machine (${said}): its memory and disk together are over what it pauses; it runs until you delete it`;
// }
pub fn nap_refused_line(said: &str) -> String {
    format!(
        "the provider does not pause this machine ({said}): its memory and disk together are over what it pauses; it runs until you delete it"
    )
}

/// A stop the provider would not take while the snapshot of the machine's disk fails, which it does rather than lose
/// what was written since the last one (Boat): its own reading, the cause seen (a full disk), and the road left. A Boat
/// machine whose stops kept failing was dropped with its disk on 2026-10-01, so pushing comes first.
// thread.ts:154
// export function stopRefusedLine(said: string): string {
//   return `the provider will not stop this machine while the snapshot of its disk fails (${said}): push its work, then free space on its disk, since a full disk fails the snapshot and a machine left like this has been dropped with its disk`;
// }
pub fn stop_refused_line(said: &str) -> String {
    format!(
        "the provider will not stop this machine while the snapshot of its disk fails ({said}): push its work, then free space on its disk, since a full disk fails the snapshot and a machine left like this has been dropped with its disk"
    )
}

/// How full a disk may be before its row says so, said only past it: past this a build's output can fill the rest
/// between two turns, and a stop that snapshots a full disk fails.
// thread.ts:160
pub const DISK_FULL_PCT: f64 = 90.0;

/// A machine whose stop snapshots its disk, past DISK_FULL_PCT: said before a stop can fail. The share is cut to a
/// tenth and never rounded up, so a disk just past the line never reads as standing on it.
// thread.ts:164
// export function diskFullLine(pct: number): string {
//   return `its disk is ${Math.floor(pct * 10) / 10}% full: a stop snapshots the disk and fails once it is full, so free space on it (build output, caches) before it naps`;
// }
pub fn disk_full_line(pct: f64) -> String {
    format!(
        "its disk is {}% full: a stop snapshots the disk and fails once it is full, so free space on it (build output, caches) before it naps",
        number_string((pct * 10.0).floor() / 10.0)
    )
}

/// How far under DISK_FULL_PCT a disk falls before the next crossing is said again: a build refilling its caches at the
/// line would otherwise be swept under and announced every other beat.
// thread.ts:170
pub const DISK_REARM_PCT: f64 = 85.0;

/// The size a full disk can move to: the first one up whose measured root disk grows the one the machine reads, and the
/// thread whose line it is, which wsp resize takes.
// thread.ts:174
// export interface DiskMove {
//   thread: string;
//   name: string;
//   cpu: number;
//   memMb: number;
//   diskGb: number;
// }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiskMove {
    pub thread: String,
    pub name: String,
    pub cpu: f64,
    pub mem_mb: f64,
    pub disk_gb: f64,
}

/// What a thread on a machine whose stop snapshots its disk is told once the disk passes DISK_FULL_PCT, once per
/// crossing: how full by df, what wsp cleared of its own there, what the person can do, and a move only to a size whose
/// root disk is measured bigger. A stop that cannot save a full disk is refused, and Boat dropped a machine whose stops
/// kept failing with its disk (2026-10-01).
// thread.ts:186
// export function diskMarkLine(o: { machine: string; on: string; pct: number; usedBytes: number; sizeBytes: number; freedBytes: number; machineGoes: boolean; move?: DiskMove }): string {
//   const full = `The disk of ${o.machine} is ${Math.floor(o.pct * 10) / 10}% used (${fmtBytesOf(o.usedBytes, o.sizeBytes, fmtDiskBytes)}) and still filling. ${o.on} cannot save a stop of a full disk, and deletes one whose stops keep failing with its disk.`;
//   const cleared = o.freedBytes > 0 ? ` wsp cleared ${fmtDiskBytes(o.freedBytes)} of its own staging and build caches there.` : " wsp found none of its own staging or build caches there to clear.";
//   const free = ` Free space on its disk${o.machineGoes ? ", or let the thread finish: its machine is deleted once it is done." : "."}`;
//   const m = o.move;
//   const move = m === undefined ? "" : ` Or move it to ${m.name} (${m.cpu} vCPU, ${fmtMemGb(m.memMb)}, ${m.diskGb} GB disk): ${shellLine(["wsp", "resize", m.thread, "--size", sizeWord(m)])} stops it and resumes it at that size, with its files, which ends a turn running there.`;
//   return `${full}${cleared}${free}${move}`;
// }
/// What `disk_mark_line` says about one machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiskMark {
    pub machine: String,
    pub on: String,
    pub pct: f64,
    pub used_bytes: f64,
    pub size_bytes: f64,
    pub freed_bytes: f64,
    pub machine_goes: bool,
    pub r#move: Option<DiskMove>,
}

pub fn disk_mark_line(o: &DiskMark) -> String {
    let full = format!(
        "The disk of {} is {}% used ({}) and still filling. {} cannot save a stop of a full disk, and deletes one whose stops keep failing with its disk.",
        o.machine,
        number_string((o.pct * 10.0).floor() / 10.0),
        fmt_bytes_of(o.used_bytes, o.size_bytes, Some(fmt_disk_bytes)),
        o.on
    );
    let cleared = if o.freed_bytes > 0.0 {
        format!(" wsp cleared {} of its own staging and build caches there.", fmt_disk_bytes(o.freed_bytes))
    } else {
        " wsp found none of its own staging or build caches there to clear.".to_string()
    };
    let free = if o.machine_goes {
        " Free space on its disk, or let the thread finish: its machine is deleted once it is done."
    } else {
        " Free space on its disk."
    };
    let move_clause = match &o.r#move {
        None => String::new(),
        Some(m) => {
            let size = WorkspaceSize { cpu: JsNumber(m.cpu), mem_mb: JsNumber(m.mem_mb) };
            let word = size_word(&size);
            format!(
                " Or move it to {} ({} vCPU, {}, {} GB disk): {} stops it and resumes it at that size, with its files, which ends a turn running there.",
                m.name,
                number_string(m.cpu),
                fmt_mem_gb(m.mem_mb),
                number_string(m.disk_gb),
                shell_line(&["wsp", "resize", m.thread.as_str(), "--size", word.as_str()])
            )
        }
    };
    format!("{full}{cleared}{free}{move_clause}")
}

/// A move to another size asked of a provider that changes no machine's size.
// thread.ts:196
// export const resizeNoMoveLine = (on: string): string => `${on} changes no machine's size once it is made`;
pub fn resize_no_move_line(on: &str) -> String {
    format!("{on} changes no machine's size once it is made")
}

// thread.ts:197
pub const RESIZE_NO_MOVE_FIX: &str = "Start a new thread at the size you want instead.";

/// A move to another size while a turn runs there: the stop it takes ends every process on the machine.
// thread.ts:200
// export const resizeBusyLine = (name: string): string => `${name} has a turn running, and the stop a move to another size takes would end it`;
pub fn resize_busy_line(name: &str) -> String {
    format!("{name} has a turn running, and the stop a move to another size takes would end it")
}

// thread.ts:201
pub const RESIZE_BUSY_FIX: &str = "Stop the turn or wait for it to end, then move it.";

/// A move whose machine came back at another size than asked: a plain resume someone else sent was the one it joined.
// thread.ts:204
// export const resizeLandedLine = (name: string, got: WorkspaceSize, asked: WorkspaceSize): string => `${name} came back at ${sizeWord(got)}, not the ${sizeWord(asked)} asked for`;
pub fn resize_landed_line(name: &str, got: &WorkspaceSize, asked: &WorkspaceSize) -> String {
    format!("{name} came back at {}, not the {} asked for", size_word(got), size_word(asked))
}

// thread.ts:205
pub const RESIZE_AGAIN_FIX: &str = "Run wsp resize again.";

/// What wsp resize prints once the machine runs again at the size it was moved to.
// thread.ts:208
// export const resizedLine = (name: string, size: WorkspaceSize): string => `${name} runs at ${sizeWord(size)} now, on a machine of that size`;
pub fn resized_line(name: &str, size: &WorkspaceSize) -> String {
    format!("{name} runs at {} now, on a machine of that size", size_word(size))
}

/// A --size word that names no size at all, on the command line and in the tool alike.
// thread.ts:211
// export const resizeWordLine = (word: string): string => `${JSON.stringify(word)} names no size`;
pub fn resize_word_line(word: &str) -> String {
    format!("{} names no size", serde_json::to_string(word).unwrap_or_default())
}

// thread.ts:212
pub const RESIZE_WORD_FIX: &str = "A size is <cpu>x<memGb> with neither figure 0, like 4x8.";

/// The turn's error when nothing on the machine answered a launch from this computer for the whole reach window:
/// how many times it was tried and over how long. The fetch's own words name a Node error and the machine id,
/// neither of which a person can act on.
// thread.ts:217
// export function machineUnreachedLine(attempts: number, elapsedMs: number): string {
//   return `the machine could not be reached from this computer after ${plural(attempts, "attempt")} over ${fmtDuration(elapsedMs)}`;
// }
pub fn machine_unreached_line(attempts: f64, elapsed_ms: f64) -> String {
    format!(
        "the machine could not be reached from this computer after {} over {}",
        plural(attempts, "attempt"),
        fmt_duration(elapsed_ms, None)
    )
}

/// The line an MCP install ends on: the command every agent's config now runs, as one shell line.
// thread.ts:222
// export function mcpServerCommandLine(command: string, args: readonly string[]): string {
//   return `The server command is ${shellLine([command, ...args])}`;
// }
pub fn mcp_server_command_line(command: &str, args: &[impl AsRef<str>]) -> String {
    let mut words: Vec<&str> = vec![command];
    words.extend(args.iter().map(|arg| arg.as_ref()));
    format!("The server command is {}", shell_line(&words))
}

/// The very last line an MCP install prints: the thing to do next, which is inside the agent it just gave the tools
/// to. `open` is the agent's own command and `first` what to type at its prompt, its slash form where it has one.
// thread.ts:228
// export function nextInsideAgentLine(open: string, first: string): string {
//   return `Next: run ${shellLine([open])} in this folder and say: ${first}`;
// }
pub fn next_inside_agent_line(open: &str, first: &str) -> String {
    format!("Next: run {} in this folder and say: {first}", shell_line(&[open]))
}

/// One level of this computer's own folders in words, the same on the command line and in the app's folder browser:
/// how many folders the level holds, where it sits, and how many of it are hidden, whether or not those are listed.
// thread.ts:234
// export function folderLevelLine(listing: { dir: string; folders: readonly unknown[]; hidden: number }): string {
//   const held = listing.hidden === 0 ? "" : `, ${listing.hidden} hidden`;
//   return listing.folders.length === 0 ? `No folders in ${listing.dir}${held}.` : `${plural(listing.folders.length, "folder")} in ${listing.dir}${held}.`;
// }
/// `folders` is the count of the listing's folders; nothing else of them is read.
pub fn folder_level_line(dir: &str, folders: usize, hidden: f64) -> String {
    let held = if hidden == 0.0 { String::new() } else { format!(", {} hidden", number_string(hidden)) };
    if folders == 0 {
        format!("No folders in {dir}{held}.")
    } else {
        format!("{} in {dir}{held}.", plural(folders as f64, "folder"))
    }
}

/// What the folder browser's state slot says for a level this computer would not let the host read, and the words the
/// command line's own refusal line carries: the host's reason, which names the folder itself, on one line whatever the
/// host said. A refused level leaves the list on the level it was already on, so this slot is where it is read.
// thread.ts:242
// export function folderRefusalLine(reason: string): string {
//   return `No folders read. ${reason.replace(/\s+/g, " ").trim()}`;
// }
pub fn folder_refusal_line(reason: &str) -> String {
    format!("No folders read. {}", js_trim(&js_collapse_whitespace(reason)))
}

/// The one stderr line the command line shows under a command that exited non-zero, naming the folder it ran in:
/// the host chose it when none was named, so the person did not see it go by. The caller passes the folder the host
/// answered with rather than the one it asked for; absent, the workspace's kind named none and the machine's own home
/// is where the command ran.
// thread.ts:250
// export function execFolderLine(cwd: string | undefined): string {
//   return `ran in ${cwd ?? "the home folder"}`;
// }
pub fn exec_folder_line(cwd: Option<&str>) -> String {
    format!("ran in {}", cwd.unwrap_or("the home folder"))
}

/// The last line of a reply whose process ended while the agent's own background tasks were still running: the turn
/// reads done, since the work left running is not its failure, and this names that work.
// thread.ts:256
// export function backgroundTasksLine(running: number): string {
//   return `ended with ${plural(running, "background task")} running`;
// }
pub fn background_tasks_line(running: f64) -> String {
    format!("ended with {} running", plural(running, "background task"))
}

/// The last line of the line a lead is sent at a reply given over the agent's own running background tasks: the turn
/// is still open, and a lead that read the reply alone took the child for finished and waiting on nothing.
// thread.ts:262
// export function stillRunningLine(running: number): string {
//   return `${plural(running, "background task")} still running; another line comes when this turn ends`;
// }
pub fn still_running_line(running: f64) -> String {
    format!("{} still running; another line comes when this turn ends", plural(running, "background task"))
}

/// One line under a held reply for a background command that finished after the agent's words without waking it: the
/// command as the harness described it, how it ended and how long after the reply, so the thread's last message says
/// what the turn stayed open for and what came of it. Where the harness wakes the agent instead, its own next reply
/// is the report and no line of ours is added.
// thread.ts:270
// export function taskFinishedLine(description: string, status: string, msAfterReply: number): string {
//   return `\`${description}\` ${status}, ${fmtDuration(msAfterReply)} after the reply`;
// }
pub fn task_finished_line(description: &str, status: &str, ms_after_reply: f64) -> String {
    format!("`{description}` {status}, {} after the reply", fmt_duration(ms_after_reply, None))
}

/// What a launch that woke a machine and then died before its agent said a word answers with. The wake was this
/// launch's and nothing else ran there, so the machine goes back where the launch found it rather than billing out
/// an idle window for work that never happened.
// thread.ts:277
// export const workspaceAsleepAgainLine = (name: string): string => `${name} is asleep again`;
pub fn workspace_asleep_again_line(name: &str) -> String {
    format!("{name} is asleep again")
}

/// The same launch on a machine it did not wake, or one another thread is working on: nothing is touched, so the
/// line says what that costs by naming when the idle window takes it. Whole minutes, since nothing turns on the
/// seconds and a countdown a person reads once need not move.
// thread.ts:282
// export const workspaceStaysAwakeLine = (name: string, napsInMs?: number): string =>
//   `${name} stays awake${napsInMs === undefined ? "" : `, naps in ${fmtUptime(napsInMs)}`}`;
pub fn workspace_stays_awake_line(name: &str, naps_in_ms: Option<f64>) -> String {
    let naps = naps_in_ms.map_or(String::new(), |ms| format!(", naps in {}", fmt_uptime(ms)));
    format!("{name} stays awake{naps}")
}

/// The reason a status carries when the runtime's idle policy napped a workspace, with the one reading of it back
/// beside it: the runtime stamps the nap through `of` and the app's paused line takes the window out through
/// `windowIn`, so the words and their parse are one thing and a rewording moves both at once.
// thread.ts:288
// export const IDLE_REASON = {
//   of: (windowMs: number): string => `idle ${Math.round(windowMs / 60_000)} min`,
//   /** The window inside that reason, as its own words (`20 min`); nothing for a reason of any other shape, which is
//    * every reason a nap the person asked for or a wake wrote. */
//   windowIn: (reason: string | undefined): string | undefined => (reason === undefined ? undefined : (/^idle (\d+ min)$/.exec(reason)?.[1] ?? undefined)),
// } as const;
pub mod idle_reason {
    /// `idle <js_round(window_ms / 60000)> min`.
    pub fn of(window_ms: f64) -> String {
        format!("idle {} min", crate::js::number_string(crate::js::js_round(window_ms / 60_000.0)))
    }

    /// The window inside that reason (`20 min`) where the whole reason is `idle <digits> min`; None for None and
    /// for any other shape.
    pub fn window_in(reason: Option<&str>) -> Option<String> {
        let reason = reason?;
        regex::Regex::new(r"^idle ([0-9]+ min)$").ok()?.captures(reason).map(|caps| caps[1].to_string())
    }
}

/// What a window on another computer says while the wsp it shows has gone quiet: the computer that host runs on is
/// asleep or off, and the workspaces on every other computer keep working. It reads as a fact in the sidebar's own
/// prose line and as the send's reason, never as an alert: nothing is broken and nothing is lost.
// thread.ts:298
pub const HOST_ASLEEP_LINE: &str = "your Mac is asleep; threads on your other computers keep running";

// thread.ts:299
pub const HOST_ASLEEP_SEND: &str = "your Mac is asleep; new turns start when it wakes";

/// One line per retry of a provider call that never left this computer: which call, the system error the road gave,
/// and which try of how many is about to go, so a run that still fails carries the whole flap in its log.
// thread.ts:303
// export function providerRoadRetryLine(call: string, code: string, tryNumber: number, tries: number): string {
//   return `${call} did not leave this computer (${code}); try ${tryNumber} of ${tries}`;
// }
pub fn provider_road_retry_line(call: &str, code: &str, try_number: f64, tries: f64) -> String {
    format!("{call} did not leave this computer ({code}); try {} of {}", number_string(try_number), number_string(tries))
}

/// A create whose answer never arrived, asked again under its key until the provider named the machine it made.
// thread.ts:308
// export function createAnswerLostLine(provider: string, said: string, machineId: string): string {
//   return `the answer to ${provider}'s create was lost (${said}); wsp asked again and found ${machineId}, which this create goes on with.`;
// }
pub fn create_answer_lost_line(provider: &str, said: &str, machine_id: &str) -> String {
    format!("the answer to {provider}'s create was lost ({said}); wsp asked again and found {machine_id}, which this create goes on with.")
}

/// A fork the provider refused as "Snapshot not found" while its own listing holds that snapshot, asked again.
// thread.ts:313
// export function snapshotListedWaitLine(snapshotId: string, waitMs: number, waitedMs: number): string {
//   return `the provider lists snapshot ${snapshotId} yet answered "Snapshot not found"; asking again in ${fmtDuration(waitMs)} (${fmtDuration(waitedMs)} so far)`;
// }
pub fn snapshot_listed_wait_line(snapshot_id: &str, wait_ms: f64, waited_ms: f64) -> String {
    format!(
        "the provider lists snapshot {snapshot_id} yet answered \"Snapshot not found\"; asking again in {} ({} so far)",
        fmt_duration(wait_ms, None),
        fmt_duration(waited_ms, None)
    )
}

/// The same wait as a line of the create a person follows, one per ask: the lines' own pace is how long it has been.
// thread.ts:318
// export function imageServedWaitLine(provider: string, waitMs: number): string {
//   return `waiting for ${provider} to serve the image; asking again in ${fmtDuration(waitMs)}`;
// }
pub fn image_served_wait_line(provider: &str, wait_ms: f64) -> String {
    format!("waiting for {provider} to serve the image; asking again in {}", fmt_duration(wait_ms, None))
}

/// The same refusal once the wait for it has run out: the snapshot is listed, so it is not gone.
// thread.ts:323
// export function snapshotListedRefusedLine(snapshotId: string, waitedMs: number): string {
//   return `the provider lists snapshot ${snapshotId} yet answered "Snapshot not found" for ${fmtDuration(waitedMs)}; it is not gone, so ask again in a minute`;
// }
pub fn snapshot_listed_refused_line(snapshot_id: &str, waited_ms: f64) -> String {
    format!(
        "the provider lists snapshot {snapshot_id} yet answered \"Snapshot not found\" for {}; it is not gone, so ask again in a minute",
        fmt_duration(waited_ms, None)
    )
}

/// When a turn is over, in the one sentence every door the agent reads quotes whole: the skill, the tool
/// descriptions, the command line's help and the machine's own context. The reply comes back at once from a follow;
/// the row settles only at the process exit, since a harness can keep working after it answers.
// thread.ts:330
pub const TURN_END_WORDS: &str = "A turn ends when the agent process exits, not at its reply, and the thread reads running until then";

/// What another agent is, quoted whole by every door an agent reads before it picks a tool: the launch context, the
/// tool server's instructions, the run tool and the skill. An agent's own subagent tool answers inside its turn and
/// never reaches the sidebar, so the person asking for a second model or harness would see nothing of it.
// thread.ts:335
pub const ANOTHER_AGENT_WORDS: &str = "When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps";

/// How an agent starts work it wants to hear back from, quoted whole by the skill's rules and the machine's context:
/// the harness tracks its own background road and wakes the agent when it ends, and nothing tracks a shell `&`. The
/// machine context names the road by the agent's catalog entry where it carries one; the skill, read by every agent,
/// names none.
// thread.ts:342
// export const backgroundWorkWords = (road?: string): string =>
//   `Start long work with ${road ?? "your harness's own background road"}, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends`;
pub fn background_work_words(road: Option<&str>) -> String {
    format!(
        "Start long work with {}, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends",
        road.unwrap_or("your harness's own background road")
    )
}

// thread.ts:344
// export const BACKGROUND_WORK_WORDS = backgroundWorkWords();
pub const BACKGROUND_WORK_WORDS: &str = "Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends";

/// When the notify line goes, quoted the same way: with the reply, once, never again at the exit. A reply the agent
/// gave while work it started was still running goes at once, saying so, and the turn's end sends what came after it.
// thread.ts:348
pub const NOTIFY_WORDS: &str = "The notify line goes once, at the reply; a reply given with background tasks still running says so, and a second line goes at the turn's end";

/// Which road a caller takes to its children's ends, in the two sentences every door quotes whole: the skill's rules,
/// the tool descriptions and the command line. Which one holds is decided by whether the caller is a thread, which its
/// launch environment says; nothing else decides it, and a blocking wait is neither road.
// thread.ts:353
pub const NOTIFY_CALLER: &str = "A wsp thread starts every child with --notify me and ends its turn, and each child's finished line wakes it with that child's whole report";

// thread.ts:354
pub const COORDINATOR_HANDOFF: &str = "A caller that is not a wsp thread cannot be woken at all, so it takes the reply of one turn as the call returns, and hands work of more than one turn to a single coordinator thread in the project's folder on this computer";

/// The refusal of a start whose turn token no turn on this host carries: the host minted every token it knows into a
/// turn's own launch, so one it does not know is a caller naming a turn it is not, and reading it as the person would
/// put a builder's report in front of nobody.
// thread.ts:360
pub const NO_SUCH_TURN: &str = "no turn on this host carries that token; only wsp running inside a turn has one, and a turn that ended has none";

/// What a send meets when its thread's last turn has replied but its agent process is still running (a child it did
/// not wait for, a lingering task): the row still reads running and is not free for a new turn, so the message waits
/// for that process rather than starting a second agent in the same worktree. Not a refusal: it says where the
/// message went, and every door shows it in these words. The thread is named by its title where the caller holds
/// one; a caller without one says nothing, since an id names no thread to the person reading the line.
// thread.ts:367
// export function stillWorkingLine(title?: string): string {
//   const named = title === undefined || title.trim() === "" ? "This thread" : title.trim();
//   return `${named} replied, still working; the message runs as its next turn once that process exits`;
// }
pub fn still_working_line(title: Option<&str>) -> String {
    let named = match title.map(js_trim) {
        None | Some("") => "This thread",
        Some(title) => title,
    };
    format!("{named} replied, still working; the message runs as its next turn once that process exits")
}

/// The send key's label while the thread's turn runs: Enter queues, nothing sends.
// thread.ts:373
pub const TURN_IN_FLIGHT: &str = "Turn in flight";

/// The composer's line for a stop click the runtime refused or could not place, over the runtime's own words.
// thread.ts:376
// export function stopFailedLine(error: string): string {
//   return `Could not stop: ${error}`;
// }
pub fn stop_failed_line(error: &str) -> String {
    format!("Could not stop: {error}")
}

/// The composer's line for a send-now into a running turn that did not go, over the runtime's own words.
// thread.ts:381
// export function sendNowFailedLine(error: string): string {
//   return `Could not send now: ${error}`;
// }
pub fn send_now_failed_line(error: &str) -> String {
    format!("Could not send now: {error}")
}

/// The one line every client shows on a start whose thread's previous turn was cut, before the new turn's output.
// thread.ts:386
pub const AFTER_CUT_LINE: &str = "previous turn was cut; resuming";

/// Every refusal a terminal reads is two halves, what happened and then what to do about it, in one or two lines:
/// the law the app's first-run slot draws in its two inks, in the one sentence stderr gets. A refusal that only
/// names the fault leaves the person to guess the fix, which is the whole complaint. The first half is closed here
/// when it closes itself with nothing, since many of these sentences are written for the app as well, where they
/// stand alone and nothing follows them.
// thread.ts:393
// export function refusalLine(happened: string, fix: string): string {
//   const said = happened.trimEnd();
//   return `${said}${/[.!?:]$/.test(said) ? "" : "."} ${fix}`;
// }
pub fn refusal_line(happened: &str, fix: &str) -> String {
    let said = js_trim_end(happened);
    let stop = if said.ends_with(['.', '!', '?', ':']) { "" } else { "." };
    format!("{said}{stop} {fix}")
}

/// Text with every hidden value blanked wherever it appears, as written and trimmed: a door that trims what it read
/// before echoing it would otherwise slip the padded value past. Longest first, so a value that is another's prefix
/// leaves no tail; a value under four characters once trimmed is no key, token or code and would blank the sentence
/// letter by letter. The one rule every log that must not hold a secret reads.
// thread.ts:402
// export function redacted(text: string, hidden: readonly string[]): string {
//   const values = [...new Set(hidden.flatMap(h => (h.trim().length < 4 ? [] : [h, h.trim()])))].sort((a, b) => b.length - a.length);
//   let out = text;
//   for (const v of values) out = out.split(v).join("<redacted>");
//   return out;
// }
pub fn redacted(text: &str, hidden: &[impl AsRef<str>]) -> String {
    let mut values: Vec<&str> = Vec::new();
    for hidden in hidden {
        let raw = hidden.as_ref();
        let trimmed = js_trim(raw);
        if utf16::len(trimmed) < 4 {
            continue;
        }
        for value in [raw, trimmed] {
            if !values.contains(&value) {
                values.push(value);
            }
        }
    }
    values.sort_by_key(|value| std::cmp::Reverse(utf16::len(value)));
    values.iter().fold(text.to_string(), |out, value| out.replace(value, "<redacted>"))
}

/// A schema's issues on one line, each as its path and its message, for a log or a refusal that names what was wrong
/// rather than dumping the issue list.
// thread.ts:411
// export function issuesLine(issues: readonly { path: readonly (string | number)[]; message: string }[]): string {
//   return issues.map(i => `${i.path.join(".")}: ${i.message}`).join("; ");
// }
/// One step of a schema issue's path: a key or an array index.
#[derive(Clone, Debug, PartialEq)]
pub enum PathPart {
    Key(String),
    Index(f64),
}

/// A schema issue as `issues_line` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub path: Vec<PathPart>,
    pub message: String,
}

pub fn issues_line(issues: &[Issue]) -> String {
    issues
        .iter()
        .map(|issue| {
            let path: Vec<String> = issue
                .path
                .iter()
                .map(|part| match part {
                    PathPart::Key(key) => key.clone(),
                    PathPart::Index(index) => number_string(*index),
                })
                .collect();
            format!("{}: {}", path.join("."), issue.message)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The command's name, said once. A line refused inside a verb is printed behind the verb's name, so a sentence
/// that opens with that same name would say it twice; the prefix is the one home for it, and the sentence may open
/// with it or not without either of them knowing about the other.
// thread.ts:418
// export function sayOnce(prefix: string, message: string): string {
//   const name = prefix.replace(/:\s*$/, "");
//   return name !== "" && (message === name || message.startsWith(`${name} `)) ? message : `${prefix}${message}`;
// }
pub fn say_once(prefix: &str, message: &str) -> String {
    let name = match prefix.rfind(':') {
        Some(colon) if prefix[colon + 1..].chars().all(is_js_whitespace) => &prefix[..colon],
        _ => prefix,
    };
    if !name.is_empty() && (message == name || message.starts_with(&format!("{name} "))) {
        message.to_string()
    } else {
        format!("{prefix}{message}")
    }
}

/// A word no command answers to, named: the first half of that refusal, wherever the word was typed.
// thread.ts:424
// export const unknownWordLine = (word: string): string => `unknown command: ${word}.`;
pub fn unknown_word_line(word: &str) -> String {
    format!("unknown command: {word}.")
}

/// Where the list of words is: the second half of an unknown word's refusal. The help runs to hundreds of lines,
/// so a typo is pointed at it and never handed it.
// thread.ts:428
// export const runForTheList = (list: string): string => `Run ${list} for the list.`;
pub fn run_for_the_list(list: &str) -> String {
    format!("Run {list} for the list.")
}

/// The refusal of a flag another verb reads: the verbs it belongs to, then the one that does not read it, so the
/// caller is told where the flag lives rather than left with the parser's bare unknown-option line.
// thread.ts:432
// export function foreignFlagLine(flag: string, readers: readonly string[], here: string): string {
//   const owners = readers.length > 1 ? `${readers.slice(0, -1).join(", ")} and ${readers.at(-1)}` : readers[0];
//   return `${flag} belongs to ${owners}; ${here} does not read it`;
// }
pub fn foreign_flag_line(flag: &str, readers: &[impl AsRef<str>], here: &str) -> String {
    let readers: Vec<&str> = readers.iter().map(|reader| reader.as_ref()).collect();
    let owners = match readers.as_slice() {
        [] => "undefined".to_string(),
        [only] => only.to_string(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    };
    format!("{flag} belongs to {owners}; {here} does not read it")
}

/// An agent id no catalog entry carries, named beside the ids the catalog does know.
// thread.ts:438
// export function unknownAgentLine(id: string, known: readonly string[]): string {
//   return `no agent called ${id}; the catalog knows ${known.join(", ")}`;
// }
pub fn unknown_agent_line(id: &str, known: &[impl AsRef<str>]) -> String {
    let known: Vec<&str> = known.iter().map(|id| id.as_ref()).collect();
    format!("no agent called {id}; the catalog knows {}", known.join(", "))
}

/// The refusal of a start naming an agent the host has no adapter for, listing the ones it has.
// thread.ts:443
// export function noAdapterLine(harness: string, agents: readonly string[]): string {
//   return `no adapter registered for harness "${harness}"; agents on this host: ${agents.join(", ") || "none"}`;
// }
pub fn no_adapter_line(harness: &str, agents: &[impl AsRef<str>]) -> String {
    let agents: Vec<&str> = agents.iter().map(|agent| agent.as_ref()).collect();
    let on_host = if agents.is_empty() { "none".to_string() } else { agents.join(", ") };
    format!("no adapter registered for harness \"{harness}\"; agents on this host: {on_host}")
}

/// The words a request field is read as in a refusal, or nothing for a field no line names on its own.
fn request_word(field: &str) -> Option<&'static str> {
    match field {
        "threadId" | "sessionId" => Some("the thread"),
        "argv" => Some("the command"),
        "prompt" => Some("the message"),
        "cwd" => Some("--cwd"),
        "model" => Some("--model"),
        "effort" => Some("--effort"),
        "permissionMode" => Some("--access"),
        _ => None,
    }
}
