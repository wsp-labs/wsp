// SPDX-License-Identifier: AGPL-3.0-only
//! A file's calls reduced to names and counts, and one agent's buckets added up. Port of
//! packages/collect/src/history/tally.ts at a3b870a6e; the spec is spec/history_store.md.
use futures::stream::LocalBoxStream;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_words::exit::HostError;

use crate::history::commands::Install;
use crate::history::reader::Call;

/// tally.ts:7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    pub sessions: u64,
    pub calls: u64,
}

/// Over this many bytes a row is heavy (tally.ts:13).
pub const HEAVY_BYTES: u64 = 300 * wsp_catalog::roads::MIB;
/// The least use that ticks a tool on its own (tally.ts:15).
pub const USED_FLOOR: Count = Count { sessions: 2, calls: 5 };
/// A heavy row takes more use to earn its place (tally.ts:17).
pub const HEAVY_USED_FLOOR: Count = Count { sessions: 3, calls: 20 };

/// `size > HEAVY_BYTES`; `None` is not heavy.
pub fn is_heavy(size: Option<u64>) -> bool {
    todo!()
}

/// At or over the floor its weight sets, on both counts.
// history/tally.ts:21
// /** Whether this much use ticks a tool of this size: at or over the floor its weight sets. */
// export function meetsUsedFloor(c: Count, size: number | undefined): boolean {
//   const floor = isHeavy(size) ? HEAVY_USED_FLOOR : USED_FLOOR;
//   return c.sessions >= floor.sessions && c.calls >= floor.calls;
// }
pub fn meets_used_floor(c: Count, size: Option<u64>) -> bool {
    todo!()
}

/// tally.ts:28. Each map is ordered most sessions first, then most calls, then by name in UTF-16 order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub sessions: u64,
    pub calls: u64,
    pub commands: IndexMap<String, Count>,
    /// Keyed by install_key (`brew gh`, `npm agent-browser`).
    pub installs: IndexMap<String, Count>,
    /// The catalog tools the commands and installs stand for, by id; a session counts once per tool.
    pub tools: IndexMap<String, Count>,
}

/// `${via} ${name}` (tally.ts:66).
pub fn install_key(i: &Install) -> String {
    todo!()
}

/// The text after the first space of a key; the whole key when it has none (tally.ts:67).
pub fn installed_name(key: &str) -> String {
    todo!()
}

/// What one session came to in one file (tally.ts:71): all a cache stores. Keys in this order on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryBucket {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// The newest call of that session in that file, ms epoch; written as JavaScript writes the number.
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "wsp_words::js::serialize_opt_f64")]
    pub at: Option<f64>,
    pub calls: u64,
    /// Command name to how often it ran, in first-seen order.
    pub commands: IndexMap<String, u64>,
    /// install_key to how often an installer was asked for it, in first-seen order.
    pub installs: IndexMap<String, u64>,
}

/// Whether a bucket's session ran at one of the folders or inside it (`underProject`); a bucket with no folder never.
// history/tally.ts:85
// /** Whether a bucket counts when the recipe is weighed against named folders: its session ran at one of them or
//  * inside it. A bucket whose store records no folder counts for nothing, since nothing says it is this project's. */
// export function inFolders(where: { folder?: string }, folders: readonly string[]): boolean {
//   const folder = where.folder;
//   return folder !== undefined && folders.some(f => underProject(folder, f));
// }
pub fn in_folders(folder: Option<&str>, folders: &[String]) -> bool {
    todo!()
}

/// One file's calls reduced to one bucket per (session, folder), in first-call order. The stream's first `Err` is
/// returned as it is, and nothing after it is read.
// history/tally.ts:92
// /** Reduces one file's calls to names and counts, one bucket per session and folder in it; shell lines are read for
//  * their command words here and dropped. The catalog tools the names stand for are not decided here: they are looked
//  * up when the buckets are merged, so a catalog that grew since a bucket was cached still decides its tools. */
// export async function reduceCalls(calls: AsyncIterable<Call>): Promise<HistoryBucket[]> {
//   const buckets = new Map<string, HistoryBucket>();
//   for await (const c of calls) {
//     const key = `${c.session}\u0000${c.folder ?? ""}`;
//     let b = buckets.get(key);
//     if (b === undefined) {
//       b = { session: c.session, ...(c.folder !== undefined ? { folder: c.folder } : {}), calls: 0, commands: {}, installs: {} };
//       buckets.set(key, b);
//     }
//     b.calls += 1;
//     if (c.at !== undefined && (b.at === undefined || c.at > b.at)) b.at = c.at;
//     switch (c.kind) {
//       case "shell":
//         for (const name of commandNames(c.line)) b.commands[name] = (b.commands[name] ?? 0) + 1;
//         for (const i of installNames(c.line)) {
//           const k = installKey(i);
//           b.installs[k] = (b.installs[k] ?? 0) + 1;
//         }
//         break;
//       case "other":
//         break;
//       default: {
//         const _exhaustive: never = c;
//         return _exhaustive;
//       }
//     }
//   }
//   return [...buckets.values()];
// }
pub async fn reduce_calls(calls: LocalBoxStream<'_, Result<Call, HostError>>) -> Result<Vec<HistoryBucket>, HostError> {
    todo!()
}

/// One file's buckets with the holder its sessions sit under (TS `{ holder, buckets }`).
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub holder: String,
    pub buckets: Vec<HistoryBucket>,
}

/// The buckets with every session counted from one holder: the one whose newest call is latest, the first found on a
/// tie (a bucket with no time reads as minus infinity). Order: held order, then bucket order.
// history/tally.ts:125
// /** The buckets with every session counted from one holder: one session id under two holders is its files copied,
//  * and the holder whose newest call is latest is the one that kept growing. On a tie the first holder found keeps it. */
// export function oneHolder(held: readonly { holder: string; buckets: readonly HistoryBucket[] }[]): HistoryBucket[] {
//   const newest = new Map<string, { holder: string; at: number }>();
//   const seen = new Map<string, Map<string, number>>();
//   for (const { holder, buckets } of held) {
//     for (const b of buckets) {
//       const holders = seen.get(b.session) ?? seen.set(b.session, new Map()).get(b.session)!;
//       holders.set(holder, Math.max(holders.get(holder) ?? -Infinity, b.at ?? -Infinity));
//     }
//   }
//   for (const [session, holders] of seen) {
//     for (const [holder, at] of holders) {
//       const kept = newest.get(session);
//       if (kept === undefined || at > kept.at) newest.set(session, { holder, at });
//     }
//   }
//   return held.flatMap(({ holder, buckets }) => buckets.filter(b => newest.get(b.session)?.holder === holder));
// }
pub fn one_holder(held: &[Held]) -> Vec<HistoryBucket> {
    todo!()
}

/// Every bucket added up; with `folders`, only buckets `in_folders`.
// history/tally.ts:38
// interface Seen {
//   sessions: Set<string>;
//   calls: number;
// }
//
// class Tally {
//   private readonly seen = new Map<string, Seen>();
//
//   add(name: string, session: string, calls: number): void {
//     let s = this.seen.get(name);
//     if (s === undefined) {
//       s = { sessions: new Set(), calls: 0 };
//       this.seen.set(name, s);
//     }
//     s.sessions.add(session);
//     s.calls += calls;
//   }
//
//   /** Most sessions first, then most calls, then by name. */
//   counts(): Map<string, Count> {
//     const rows = [...this.seen].map(([name, s]) => [name, { sessions: s.sessions.size, calls: s.calls }] as const);
//     rows.sort((a, b) => b[1].sessions - a[1].sessions || b[1].calls - a[1].calls || (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
//     return new Map(rows);
//   }
// }
// history/tally.ts:145
// /** Every bucket of one agent's store added up. With `folders`, only the buckets whose session ran at one of them or
//  * inside it are counted. */
// export function usageOf(buckets: readonly HistoryBucket[], folders?: readonly string[]): Usage {
//   const sessions = new Set<string>();
//   let total = 0;
//   const commands = new Tally();
//   const installs = new Tally();
//   const tools = new Tally();
//   const tool = (name: string, session: string, calls: number): void => {
//     const t = catalogToolFor(name);
//     if (t !== undefined) tools.add(t.id, session, calls);
//   };
//   for (const b of buckets) {
//     if (folders !== undefined && !inFolders(b, folders)) continue;
//     sessions.add(b.session);
//     total += b.calls;
//     for (const [name, calls] of Object.entries(b.commands)) {
//       commands.add(name, b.session, calls);
//       tool(name, b.session, calls);
//     }
//     for (const [key, calls] of Object.entries(b.installs)) {
//       installs.add(key, b.session, calls);
//       tool(installedName(key), b.session, calls);
//     }
//   }
//   return { sessions: sessions.size, calls: total, commands: commands.counts(), installs: installs.counts(), tools: tools.counts() };
// }
pub fn usage_of(buckets: &[HistoryBucket], folders: Option<&[String]>) -> Usage {
    todo!()
}
