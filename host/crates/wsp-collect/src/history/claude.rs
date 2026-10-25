// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code keeps one jsonl per session under projects/<project key>/, with sub-agent transcripts nested in a
//! directory of the session's id and cleared sessions moved under _cleared_sessions. Port of
//! packages/collect/src/history/claude.ts at a3b870a6e; the spec is spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use serde_json::Value;
use wsp_words::exit::HostError;

use crate::history::reader::{Call, HistoryFile, HistoryReader};
use crate::host::Host;

pub const CLEARED: &str = "_cleared_sessions";

/// The top-level session a transcript belongs to, from its path under the projects root.
// history/claude.ts:11
// /** The top-level session a transcript belongs to, from its path under the projects root. */
// export function claudeSession(rel: string): string | undefined {
//   const parts = rel.split("/");
//   const second = parts[1];
//   if (second === undefined) return undefined;
//   if (second === CLEARED) return parts[2] === undefined ? undefined : stem(parts[2]);
//   return second.endsWith(".jsonl") ? stem(second) : second;
// }
pub fn claude_session(rel: &str) -> Option<String> {
    todo!()
}

// history/claude.ts:20
// export function claudeCall(session: string, name: string, input: unknown, folder?: string, at?: number): Call {
//   const arg = isRecord(input) ? input : {};
//   const where = { ...(folder !== undefined ? { folder } : {}), ...(at !== undefined ? { at } : {}) };
//   if (name === "Bash" && typeof arg["command"] === "string") return { session, ...where, kind: "shell", line: arg["command"] };
//   return { session, ...where, kind: "other", name };
// }
pub fn claude_call(session: &str, name: &str, input: Option<&Value>, folder: Option<&str>, at: Option<f64>) -> Call {
    todo!()
}

pub struct ClaudeReader;

impl HistoryReader for ClaudeReader {
    // history/claude.ts:28
    //   files: (host, root) => jsonlFiles(host, root, file => claudeSession(file.slice(root.length + 1))),
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>> {
        todo!()
    }

    // history/claude.ts:30
    //   async *read(host: Host, root: string, file: string): AsyncIterable<Call> {
    //     const session = claudeSession(file.slice(root.length + 1));
    //     if (session === undefined) return;
    //     for await (const line of host.fs.lines(file, '"tool_use"')) {
    //       // Only an assistant line holds a tool_use block; the two checks keep the large tool-result lines unparsed.
    //       if (!line.includes('"tool_use"') || !line.includes('"assistant"')) continue;
    //       const row = tryJson(line);
    //       if (!isRecord(row) || row["type"] !== "assistant" || !isRecord(row["message"])) continue;
    //       const content = row["message"]["content"];
    //       if (!Array.isArray(content)) continue;
    //       // Every line of a transcript carries the folder the session ran in, this one included.
    //       const folder = typeof row["cwd"] === "string" ? row["cwd"] : undefined;
    //       const at = timeOf(row["timestamp"]);
    //       for (const block of content) {
    //         if (!isRecord(block) || block["type"] !== "tool_use" || typeof block["name"] !== "string") continue;
    //         yield claudeCall(session, block["name"], block["input"], folder, at);
    //       }
    //     }
    //   },
    fn read<'a>(&'a self, host: &'a Host, root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>> {
        todo!()
    }

    // history/claude.ts:29
    //   holder: (root, file) => file.slice(root.length + 1).split("/")[0]!,
    fn holder(&self, root: &str, file: &str) -> String {
        todo!()
    }
}
