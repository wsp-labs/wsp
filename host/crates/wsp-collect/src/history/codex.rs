// SPDX-License-Identifier: AGPL-3.0-only
//! Codex keeps one rollout jsonl per thread under sessions/YYYY/MM/DD/. A response_item line whose payload is a
//! function_call is a call. Port of packages/collect/src/history/codex.ts at a3b870a6e; the spec is
//! spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use serde_json::Value;
use wsp_words::exit::HostError;

use crate::history::reader::{Call, HistoryFile, HistoryReader};
use crate::host::Host;

/// The function names that carry a shell command (codex.ts:8).
pub const SHELLS: [&str; 4] = ["exec_command", "shell", "container.exec", "local_shell"];

// history/codex.ts:10
// export function codexCall(session: string, name: string, args: unknown, folder?: string, at?: number): Call {
//   const arg = isRecord(args) ? args : {};
//   const where = { ...(folder !== undefined ? { folder } : {}), ...(at !== undefined ? { at } : {}) };
//   if (SHELLS.has(name)) {
//     const cmd = arg["cmd"] ?? arg["command"];
//     if (typeof cmd === "string") return { session, ...where, kind: "shell", line: cmd };
//     if (Array.isArray(cmd) && cmd.every(w => typeof w === "string")) return { session, ...where, kind: "shell", line: cmd.join(" ") };
//   }
//   return { session, ...where, kind: "other", name };
// }
pub fn codex_call(session: &str, name: &str, args: Option<&Value>, folder: Option<&str>, at: Option<f64>) -> Call {
    todo!()
}

pub struct CodexReader;

impl HistoryReader for CodexReader {
    // history/codex.ts:22
    //   files: (host, root) => jsonlFiles(host, root, stem),
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>> {
        todo!()
    }

    // history/codex.ts:24
    //   async *read(host: Host, _root: string, file: string): AsyncIterable<Call> {
    //     const session = stem(file);
    //     // The rollout opens with a session_meta whose payload names the folder; every call after it is that folder's.
    //     let folder: string | undefined;
    //     for await (const line of host.fs.lines(file)) {
    //       if (folder === undefined && line.includes('"session_meta"')) {
    //         const meta = tryJson(line);
    //         const payload = isRecord(meta) && isRecord(meta["payload"]) ? meta["payload"] : undefined;
    //         if (payload !== undefined && typeof payload["cwd"] === "string") folder = payload["cwd"];
    //       }
    //       if (!line.includes('"function_call"')) continue;
    //       const row = tryJson(line);
    //       if (!isRecord(row) || row["type"] !== "response_item" || !isRecord(row["payload"])) continue;
    //       const payload = row["payload"];
    //       if (payload["type"] !== "function_call" || typeof payload["name"] !== "string") continue;
    //       const args = typeof payload["arguments"] === "string" ? tryJson(payload["arguments"]) : payload["arguments"];
    //       yield codexCall(session, payload["name"], args, folder, timeOf(row["timestamp"]));
    //     }
    //   },
    fn read<'a>(&'a self, host: &'a Host, root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>> {
        todo!()
    }

    // history/codex.ts:23
    //   holder: (_root, file) => file,
    fn holder(&self, root: &str, file: &str) -> String {
        todo!()
    }
}
