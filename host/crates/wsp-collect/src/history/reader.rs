// SPDX-License-Identifier: AGPL-3.0-only
//! One reader per session store format. A reader names its agent's own session files, each with the stamp that says
//! whether it changed, and turns one of them into tool calls and nothing else. No line, argument or result an agent saw
//! is kept or returned. Port of packages/collect/src/history/reader.ts at a3b870a6e; the spec is spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use serde_json::Value;
use wsp_words::exit::HostError;

use crate::host::Host;

/// What one call came to (reader.ts:18): a shell line, or any other tool by name.
#[derive(Debug, Clone, PartialEq)]
pub enum CallKind {
    Shell { line: String },
    Other { name: String },
}

/// reader.ts:9-18.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub session: String,
    /// The folder the session ran in, as its own store records it.
    pub folder: Option<String>,
    /// When the store says the call was made, ms epoch.
    pub at: Option<f64>,
    pub kind: CallKind,
}

/// reader.ts:20.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryFile {
    /// Absolute path of one file the reader can read calls from.
    pub path: String,
    /// What says the file has not changed since it was last read: `<mtimeMs>:<bytes>`.
    pub stamp: String,
}

/// reader.ts:27. Dyn-safe so the registry can hand out one reader per format.
pub trait HistoryReader {
    /// The files under root that hold sessions; empty when the root is not there.
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>>;
    /// Every tool call in one of those files. An `Err` item is the TS throw: the file is there and cannot be read.
    fn read<'a>(&'a self, host: &'a Host, root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>>;
    /// Where a file's sessions sit: Claude Code's project folder, else the file itself.
    fn holder(&self, root: &str, file: &str) -> String;
}

/// reader.ts:42. Bump it when commands.rs, tally.rs or a reader changes what a bucket comes to.
pub const PARSE_VERSION: u32 = 2;

/// The file with its stamp: `${mtimeMs}:${bytes}`, the mtime written as JavaScript writes a number (`1000`, `1.5`).
/// `None` when the path is not a file.
// history/reader.ts:44
// /** The file with the stamp a cache compares: its modification time and its size, so a rewrite that keeps either one
//  * is still read again. Nothing when the path is not a file. */
// export async function stampOf(host: Host, path: string): Promise<HistoryFile | undefined> {
//   const st = await host.fs.stat(path);
//   return st === undefined || st.kind !== "file" ? undefined : { path, stamp: `${st.mtimeMs}:${st.bytes}` };
// }
pub async fn stamp_of(host: &Host, path: &str) -> Option<HistoryFile> {
    todo!()
}

/// Every `.jsonl` file under root the session rule has an answer for, stamped, in walk order.
// history/reader.ts:51
// /** Every jsonl file under root the reader keeps a session for, stamped, in the order the walk found them. The one
//  * walk both jsonl formats take: a file the session rule has no answer for is not a transcript and is left out. */
// export async function jsonlFiles(host: Host, root: string, session: (file: string) => string | undefined): Promise<HistoryFile[]> {
//   const out: HistoryFile[] = [];
//   for (const file of await host.fs.walk(root)) {
//     if (!file.endsWith(".jsonl") || session(file) === undefined) continue;
//     const stamped = await stampOf(host, file);
//     if (stamped !== undefined) out.push(stamped);
//   }
//   return out;
// }
pub async fn jsonl_files(host: &Host, root: &str, session: &dyn Fn(&str) -> Option<String>) -> Vec<HistoryFile> {
    todo!()
}

/// A JSON object (not an array, not null).
pub fn is_record(v: &Value) -> bool {
    todo!()
}

/// `JSON.parse(text)`, or `None` where it throws.
pub fn try_json(text: &str) -> Option<Value> {
    todo!()
}

/// A store's timestamp as ms epoch: a JSON string that is an RFC 3339 instant; `None` for anything else.
// history/reader.ts:76
// /** A store's timestamp as ms epoch, or nothing where it is not one. */
// export const timeOf = (v: unknown): number | undefined => {
//   const at = typeof v === "string" ? Date.parse(v) : NaN;
//   return Number.isFinite(at) ? at : undefined;
// };
pub fn time_of(v: Option<&Value>) -> Option<f64> {
    todo!()
}

/// The file's name without its directory or its last extension; a leading dot is not an extension.
// history/reader.ts:82
// export function stem(path: string): string {
//   const base = path.slice(path.lastIndexOf("/") + 1);
//   const dot = base.lastIndexOf(".");
//   return dot > 0 ? base.slice(0, dot) : base;
// }
pub fn stem(path: &str) -> String {
    todo!()
}
