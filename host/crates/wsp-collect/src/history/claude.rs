// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code keeps one jsonl per session under projects/<project key>/, with sub-agent transcripts nested in a
//! directory of the session's id and cleared sessions moved under _cleared_sessions. Port of
//! packages/collect/src/history/claude.ts at a3b870a6e; the spec is spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use serde_json::Value;
use wsp_words::exit::HostError;

use crate::history::reader::{is_record, jsonl_files, stem, time_of, try_json, Call, CallKind, HistoryFile, HistoryReader};
use crate::host::Host;

pub const CLEARED: &str = "_cleared_sessions";

/// The top-level session a transcript belongs to, from its path under the projects root.
pub fn claude_session(rel: &str) -> Option<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    let second = parts.get(1)?;
    if *second == CLEARED {
        return parts.get(2).map(|p| stem(p));
    }
    Some(if second.ends_with(".jsonl") { stem(second) } else { second.to_string() })
}

pub fn claude_call(session: &str, name: &str, input: Option<&Value>, folder: Option<&str>, at: Option<f64>) -> Call {
    let command = input.filter(|v| is_record(v)).and_then(|v| v.get("command")).and_then(Value::as_str);
    let kind = match command {
        Some(line) if name == "Bash" => CallKind::Shell { line: line.to_string() },
        _ => CallKind::Other { name: name.to_string() },
    };
    Call { session: session.to_string(), folder: folder.map(str::to_string), at, kind }
}

/// The part of a walked file's path under the projects root.
fn under_root<'s>(file: &'s str, root: &str) -> &'s str {
    file.get(root.len() + 1..).unwrap_or("")
}

/// The calls one transcript line holds: only an assistant line's tool_use blocks.
fn calls_of_line(session: &str, line: &str) -> Vec<Call> {
    if !line.contains("\"tool_use\"") || !line.contains("\"assistant\"") {
        return Vec::new();
    }
    let Some(row) = try_json(line) else { return Vec::new() };
    if row.get("type").and_then(Value::as_str) != Some("assistant") {
        return Vec::new();
    }
    let Some(content) = row.get("message").filter(|m| is_record(m)).and_then(|m| m.get("content")).and_then(Value::as_array) else {
        return Vec::new();
    };
    let folder = row.get("cwd").and_then(Value::as_str);
    let at = time_of(row.get("timestamp"));
    content
        .iter()
        .filter(|block| is_record(block) && block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|block| Some((block.get("name")?.as_str()?, block.get("input"))))
        .map(|(name, input)| claude_call(session, name, input, folder, at))
        .collect()
}

pub struct ClaudeReader;

impl HistoryReader for ClaudeReader {
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>> {
        async move { jsonl_files(host, root, &|file: &str| claude_session(under_root(file, root))).await }.boxed_local()
    }

    fn read<'a>(&'a self, host: &'a Host, root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>> {
        let Some(session) = claude_session(under_root(file, root)) else {
            return futures::stream::empty().boxed_local();
        };
        host.fs
            .lines(file, Some("\"tool_use\""))
            .map(move |item| match item {
                Ok(line) => calls_of_line(&session, &line).into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(HostError::new(e.to_string()))],
            })
            .flat_map(futures::stream::iter)
            .boxed_local()
    }

    fn holder(&self, root: &str, file: &str) -> String {
        under_root(file, root).split('/').next().unwrap_or_default().to_string()
    }
}
