// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/base.ts and words/boat.ts at a3b870a6e: counts with their noun, names in a
//! list, a line cut at a word, a text's last line, and Boat's plan line. Each TS statement stands as a comment
//! above its stub. See spec/base.md.

use crate::exit::HostError;

/// A count with its noun, the noun pluralised by an s: the one rule every line that counts rows, sessions, calls,
/// threads or a plan's files reads, so none of them says "1 sessions". A noun that does not take an s is spelled by
/// its caller.
// base.ts:5
// export function plural(n: number, noun: string): string {
//   return `${n} ${noun}${n === 1 ? "" : "s"}`;
// }
pub fn plural(n: f64, noun: &str) -> String {
    todo!()
}

/// A sentence opening read mid-line: its first letter lowered, the rest as written.
// base.ts:10
// export const lowerFirst = (words: string): string => `${words.charAt(0).toLowerCase()}${words.slice(1)}`;
pub fn lower_first(words: &str) -> String {
    todo!()
}

/// One name inside a comma-joined list of names: quoted when the name carries that comma itself, so a free-text
/// label an agent wrote reads as one entry and not as two nameless ones.
// base.ts:14
// export function listedName(name: string): string {
//   return name.includes(",") ? JSON.stringify(name) : name;
// }
pub fn listed_name(name: &str) -> String {
    todo!()
}

/// A list of names as every tally that names its rows prints it, each name by the rule above.
// base.ts:19
// export function nameList(names: readonly string[]): string {
//   return names.map(listedName).join(", ");
// }
pub fn name_list(names: &[impl AsRef<str>]) -> String {
    todo!()
}

/// A thread count with its noun, as the sidebar's counts and the verbs' lines say it.
// base.ts:24
// export function fmtThreads(n: number): string {
//   return plural(n, "thread");
// }
pub fn fmt_threads(n: f64) -> String {
    todo!()
}

// base.ts:28
pub const ELLIPSIS: &str = "\u{2026}";

/// Text cut to at most room characters, at a word boundary where one fits, with the ellipsis counted inside the
/// room and drawn only where something was taken off. One rule for every line a surface cuts itself: a thread's
/// title from its opening turn, a sidebar row's third line. `room` counts UTF-16 units (rule 8.4).
// base.ts:33
// export function cutLine(text: string, room: number): string {
//   if (text.length <= room) return text;
//   const head = room - ELLIPSIS.length;
//   return `${wordsWithin(text, head) ?? wholeCharacters(text, head).replace(SEPARATOR_TAIL, "")}${ELLIPSIS}`;
// }
pub fn cut_line(text: &str, room: usize) -> String {
    todo!()
}

// The text's first units up to room, with a pair the edge would split left out whole: half of one is no character,
// and a JSON reader such as the Rust tool server's refuses the escape it is written as.
// base.ts:41
// private in the TS: port it as a private helper
// function wholeCharacters(text: string, room: number): string {
//   const head = text.slice(0, room);
//   return /[\uD800-\uDBFF]$/.test(head) ? head.slice(0, -1) : head;
// }

// What a cut leaves dangling at its edge: the space it broke on and the punctuation that hung off the word before.
// base.ts:47
// private in the TS: `\s` is JavaScript's whitespace set (rule 8.6)
// const SEPARATOR_TAIL = /[\s,;:]+$/;

/// The whole words of a line that fit in the room, the separator they ended on taken off; nothing when the line's
/// first word alone overruns it. A word that ends exactly at the room's edge is kept whole. `room` counts UTF-16
/// units.
// base.ts:51
// export function wordsWithin(line: string, room: number): string | undefined {
//   const head = line.slice(0, room + 1);
//   const boundary = head.lastIndexOf(" ");
//   return boundary > 0 ? head.slice(0, boundary).replace(SEPARATOR_TAIL, "") : undefined;
// }
pub fn words_within(line: &str, room: usize) -> Option<String> {
    todo!()
}

/// Text cut to its last line: the last non-empty line with the whitespace collapsed, or nothing when the text has
/// none. The notify line ends with it and a switcher card shows it under the thread's title, so both read one rule.
// base.ts:59
// export function lastLine(text: string): string | undefined {
//   const lines = text.split(/\r?\n/).filter(line => line.trim().length > 0);
//   const last = lines[lines.length - 1];
//   return last === undefined ? undefined : last.replace(/\s+/g, " ").trim();
// }
pub fn last_line(text: &str) -> Option<String> {
    todo!()
}

/// Whether an error carries the boolean mark `mark` (`HostError::marks`).
// base.ts:65
// export const marked = (e: unknown, mark: string): e is Error => e instanceof Error && (e as unknown as Record<string, unknown>)[mark] === true;
pub fn marked(e: &HostError, mark: &str) -> bool {
    todo!()
}

/// One window of Boat starts: its limit.
#[derive(Clone, Debug, PartialEq)]
pub struct StartsLimit {
    pub limit: f64,
}

/// Boat's three windows of starts, all three present or none (`Record<"minute" | "hour" | "day", { limit }>`).
#[derive(Clone, Debug, PartialEq)]
pub struct BoatStarts {
    pub minute: StartsLimit,
    pub hour: StartsLimit,
    pub day: StartsLimit,
}

/// What a key holds, as the engine reads it off one /limits answer (`limitsOf`): the plan, its machine cap, its
/// three windows of starts (absent where the account has none) and the balance. A field the answer left out is left
/// out of the line, so nothing is said that the account did not say.
// boat.ts:7
// export interface BoatLimitsRead {
//   trial: boolean;
//   planUsd?: number;
//   machinesAtOnce?: number;
//   starts?: Record<"minute" | "hour" | "day", { limit: number }>;
//   creditBalanceHours?: number;
// }
#[derive(Clone, Debug, PartialEq)]
pub struct BoatLimitsRead {
    pub trial: bool,
    pub plan_usd: Option<f64>,
    pub machines_at_once: Option<f64>,
    pub starts: Option<BoatStarts>,
    pub credit_balance_hours: Option<f64>,
}

// A count of hours as the plan line says it: whole where it is whole, else to one decimal.
// boat.ts:16
// private in the TS: port it as a private helper
// const hoursWord = (hours: number): string => `${Number.isInteger(hours) ? hours : hours.toFixed(1)} h`;

// boat.ts:18
// private in the TS: port it as a private helper
// const startsWords = (starts: BoatLimitsRead["starts"]): string[] =>
//   starts === undefined ? [] : [`${starts.minute.limit} starts a minute`, `${starts.hour.limit} an hour`, `${starts.day.limit} a day`];

/// The size a new machine gets on Boat until the person picks another, as the plan line names it.
// boat.ts:22
// export interface BoatSizeWords {
//   type: string;
//   cpu: number;
//   memMb: number;
//   rateUsdPerHour: number;
// }
#[derive(Clone, Debug, PartialEq)]
pub struct BoatSizeWords {
    pub r#type: String,
    pub cpu: f64,
    pub mem_mb: f64,
    pub rate_usd_per_hour: f64,
}

/// What wsp add boat prints once the key is saved, off the account's own /limits answer: the plan, the machines it
/// runs at once and its starts, then the size a machine gets. A trial says its hours and the sizes it is sold.
// boat.ts:31
// export function boatAddedLine(limits: BoatLimitsRead | undefined, size: BoatSizeWords): string {
//   if (limits === undefined) return `Boat added; ${sizeWords(size)}.`;
//   const machines = limits.machinesAtOnce === undefined ? [] : [`${limits.machinesAtOnce} machine${limits.machinesAtOnce === 1 ? "" : "s"} at once`];
//   if (limits.trial) {
//     const held = [...(limits.creditBalanceHours === undefined ? [] : [hoursWord(limits.creditBalanceHours)]), ...machines, ...startsWords(limits.starts)];
//     return `Boat added: trial${held.length === 0 ? "" : ` (${held.join(", ")})`}; small and default sizes.`;
//   }
//   const held = [limits.planUsd === undefined ? "a paid plan" : `$${limits.planUsd} plan`, ...machines, ...startsWords(limits.starts)];
//   return `Boat added: ${held.join(", ")}; ${sizeWords(size)}.`;
// }
pub fn boat_added_line(limits: Option<&BoatLimitsRead>, size: &BoatSizeWords) -> String {
    todo!()
}

// boat.ts:42
// private in the TS: port it as a private helper
// const sizeWords = (size: BoatSizeWords): string => `${size.type} ${fmtCpus(size.cpu)} ${fmtMemGb(size.memMb)} at ${fmtRateDollars(size.rateUsdPerHour)}/h until you pick another size`;

/// What wsp init says of the first machine's flags on a build on Boat, which forks no first machine.
// boat.ts:45
// export const boatForksNoFirstLine = (flags: readonly string[]): string => `Boat forks no first machine, so ${flags.join(" and ")} ${flags.length === 1 ? "does" : "do"} nothing here.`;
pub fn boat_forks_no_first_line(flags: &[impl AsRef<str>]) -> String {
    todo!()
}

/// What a seal on Boat ends on in place of a first machine: nothing is forked there, a lead's sub-thread is.
// boat.ts:48
pub const BOAT_READY_LINE: &str = "Boat is ready for sub-threads: a thread starts one with run and on boat.";

/// The GitHub CLI login row's label: the detector writes it and a seal records the login under it.
// boat.ts:51
pub const GITHUB_LOGIN_NAME: &str = "GitHub CLI login";
