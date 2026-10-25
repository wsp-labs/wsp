// SPDX-License-Identifier: AGPL-3.0-only
//! The registry: one reader per session store format the catalog names, and the reads over every agent's store. Port
//! of packages/collect/src/history/index.ts at a3b870a6e; the spec is spec/history_store.md.
pub mod cache;
pub mod claude;
pub mod codex;
pub mod commands;
pub mod hermes;
pub mod reader;
pub mod tally;

use wsp_catalog::catalog::{AgentEntry, HistoryFormat};
use wsp_wire::views::golden_image::RecipeHistoryState;
use wsp_words::exit::HostError;

use crate::history::cache::HistoryCache;
use crate::history::reader::HistoryReader;
use crate::history::tally::Usage;
use crate::host::Host;

/// How many of a store's files are read at once (index.ts:39).
pub const FILES_AT_ONCE: usize = 4;

/// `HISTORY_READERS[format]` (index.ts:16).
pub fn history_reader(format: HistoryFormat) -> &'static dyn HistoryReader {
    todo!()
}

/// index.ts:22: the recipe's history line for one agent, with the counts it kept.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentHistory {
    pub agent: String,
    pub state: RecipeHistoryState,
    pub sessions: u64,
    pub calls: u64,
    pub usage: Usage,
}

/// index.ts:28.
#[derive(Default)]
pub struct StoreOptions<'a> {
    /// Absolute folders the counts are weighed against.
    pub folders: Option<&'a [String]>,
    pub cache: Option<&'a dyn HistoryCache>,
    /// Told `(read, files)` as each of the store's files lands.
    pub on_file: Option<&'a dyn Fn(usize, usize)>,
}

/// What one agent's store came to: each file from the cache when its stamp matches, else read, FILES_AT_ONCE at a
/// time; the first file that cannot be read ends the store with its error once the reads already open settle.
// history/index.ts:37
// /** How many of a store's files are read at once: a first read of a busy computer's sessions spent a quarter of its time
//  * waiting on the disk one file at a time, and another file's lines are parsed while one waits. */
// const FILES_AT_ONCE = 4;
//
// /** What one agent's store came to. Every session file is either taken from the cache, when its stamp says it has
//  * not changed, or read; the folders are weighed after the merge, so one cache serves a run for any project. */
// export async function readStore(host: Host, reader: HistoryReader, root: string, opts: StoreOptions = {}): Promise<Usage> {
//   const files = await reader.files(host, root);
//   const held: { holder: string; buckets: readonly HistoryBucket[] }[] = new Array(files.length);
//   let next = 0;
//   let read = 0;
//   const reading = async (): Promise<void> => {
//     for (let i = next++; i < files.length; i = next++) {
//       const f = files[i]!;
//       let buckets = opts.cache?.get(f.path, f.stamp);
//       if (buckets === undefined) {
//         buckets = await reduceCalls(reader.read(host, root, f.path));
//         opts.cache?.set(f.path, f.stamp, buckets);
//       }
//       held[i] = { holder: reader.holder(root, f.path), buckets };
//       opts.onFile?.(++read, files.length);
//     }
//   };
//   // A file that cannot be read ends the store: no file is started after it, and the throw waits for the reads already
//   // open, so nothing reports on a store after it was said unreadable.
//   const ran = await Promise.allSettled(Array.from({ length: Math.min(FILES_AT_ONCE, files.length) }, () => reading().catch((e: unknown) => {
//     next = files.length;
//     throw e;
//   })));
//   const failed = ran.find((r): r is PromiseRejectedResult => r.status === "rejected");
//   if (failed !== undefined) throw failed.reason;
//   return usageOf(oneHolder(held), opts.folders);
// }
pub async fn read_store(host: &Host, reader: &dyn HistoryReader, root: &str, opts: StoreOptions<'_>) -> Result<Usage, HostError> {
    todo!()
}

/// index.ts:72.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryProgress {
    pub agent: String,
    pub read: usize,
    pub files: usize,
}

/// index.ts:80.
#[derive(Default)]
pub struct HistoryOptions<'a> {
    pub on_agent: Option<&'a dyn Fn(&AgentHistory)>,
    pub folders: Option<&'a [String]>,
    pub cache: Option<&'a dyn HistoryCache>,
    pub on_progress: Option<&'a dyn Fn(HistoryProgress)>,
}

/// What each agent's session store says it used, in the agents' order: read, empty, unreadable, or no reader. The
/// cache is saved once after the last agent; its write error is the only error.
// history/index.ts:91
// /** What each agent's session store on this computer says it used, read-only. A store that is there but cannot be
//  * read is said so and counts nothing; a format with no reader is said so too. */
// export async function readHistories(host: Host, agents: readonly AgentEntry[] = CATALOG_AGENTS, opts: HistoryOptions = {}): Promise<AgentHistory[]> {
//   const { onAgent, folders, cache, onProgress } = opts;
//   const out: AgentHistory[] = [];
//   for (const a of agents) {
//     let h: AgentHistory;
//     if (a.history === undefined) h = { agent: a.id, state: "no-reader", sessions: 0, calls: 0, usage: EMPTY };
//     else {
//       try {
//         const usage = await readStore(host, HISTORY_READERS[a.history.format], expand(host, a.history.root), {
//           ...(folders !== undefined ? { folders } : {}),
//           ...(cache !== undefined ? { cache } : {}),
//           ...(onProgress !== undefined ? { onFile: (read, files) => onProgress({ agent: a.id, read, files }) } : {}),
//         });
//         h = { agent: a.id, state: usage.sessions === 0 ? "empty" : "read", sessions: usage.sessions, calls: usage.calls, usage };
//       } catch {
//         h = { agent: a.id, state: "unreadable", sessions: 0, calls: 0, usage: EMPTY };
//       }
//     }
//     onAgent?.(h);
//     out.push(h);
//   }
//   cache?.save();
//   return out;
// }
pub async fn read_histories(host: &Host, agents: &[&AgentEntry], opts: HistoryOptions<'_>) -> Result<Vec<AgentHistory>, HostError> {
    todo!()
}
