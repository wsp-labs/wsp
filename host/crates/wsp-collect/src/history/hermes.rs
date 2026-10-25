// SPDX-License-Identifier: AGPL-3.0-only
//! Hermes keeps every session in one SQLite database, read through the sqlite3 command in read-only mode. Port of
//! packages/collect/src/history/hermes.ts at a3b870a6e; the spec is spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use serde_json::Value;
use wsp_words::exit::HostError;

use crate::history::reader::{Call, HistoryFile, HistoryReader};
use crate::host::Host;

pub const QUERY: &str = "select session_id, tool_calls from messages where tool_calls is not null";
/// The same rows with the folder each session ran in; a database without the sessions table falls back to QUERY.
pub const QUERY_WITH_CWD: &str =
    "select m.session_id, m.tool_calls, s.cwd from messages m left join sessions s on s.id = m.session_id where m.tool_calls is not null";

// history/hermes.ts:13
// export function hermesCall(session: string, name: string, args: unknown, folder?: string): Call {
//   const arg = isRecord(args) ? args : {};
//   const where = folder !== undefined ? { folder } : {};
//   if (name === "terminal" && typeof arg["command"] === "string") return { session, ...where, kind: "shell", line: arg["command"] };
//   return { session, ...where, kind: "other", name };
// }
pub fn hermes_call(session: &str, name: &str, args: Option<&Value>, folder: Option<&str>) -> Call {
    todo!()
}

/// The calls in one row's tool_calls JSON: a JSON string of a list, or the list itself.
// history/hermes.ts:20
// /** The calls in one row's tool_calls JSON. */
// export function hermesCalls(session: string, toolCalls: unknown, folder?: string): Call[] {
//   const list = typeof toolCalls === "string" ? tryJson(toolCalls) : toolCalls;
//   if (!Array.isArray(list)) return [];
//   return list.flatMap(item => {
//     const fn = isRecord(item) && isRecord(item["function"]) ? item["function"] : undefined;
//     if (fn === undefined || typeof fn["name"] !== "string") return [];
//     const args = typeof fn["arguments"] === "string" ? tryJson(fn["arguments"]) : fn["arguments"];
//     return [hermesCall(session, fn["name"], args, folder)];
//   });
// }
pub fn hermes_calls(session: &str, tool_calls: Option<&Value>, folder: Option<&str>) -> Vec<Call> {
    todo!()
}

pub struct HermesReader;

impl HistoryReader for HermesReader {
    // history/hermes.ts:35
    //   async files(host, root) {
    //     const stamped = await stampOf(host, root);
    //     return stamped === undefined ? [] : [stamped];
    //   },
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>> {
        todo!()
    }

    /// The two sqlite3 runs and their errors: `<file> could not be read with sqlite3` when neither answers,
    /// `<file>: sqlite3 returned no JSON` when the answer is not a JSON list; both as one `Err` item.
    // history/hermes.ts:39
    //   async *read(host: Host, _root: string, file: string): AsyncIterable<Call> {
    //     const out = (await host.exec.run("sqlite3", ["-readonly", "-json", file, QUERY_WITH_CWD])) ?? (await host.exec.run("sqlite3", ["-readonly", "-json", file, QUERY]));
    //     if (out === undefined) throw new Error(`${file} could not be read with sqlite3`);
    //     const rows = out.trim() === "" ? [] : tryJson(out);
    //     if (!Array.isArray(rows)) throw new Error(`${file}: sqlite3 returned no JSON`);
    //     for (const row of rows) {
    //       if (!isRecord(row) || typeof row["session_id"] !== "string") continue;
    //       yield* hermesCalls(row["session_id"], row["tool_calls"], typeof row["cwd"] === "string" ? row["cwd"] : undefined);
    //     }
    //   },
    fn read<'a>(&'a self, host: &'a Host, root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>> {
        todo!()
    }

    // history/hermes.ts:34
    //   holder: (_root, file) => file,
    fn holder(&self, root: &str, file: &str) -> String {
        todo!()
    }
}
