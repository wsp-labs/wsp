// SPDX-License-Identifier: AGPL-3.0-only
//! Codex keeps one rollout jsonl per thread under sessions/YYYY/MM/DD/. A response_item line whose payload is a
//! function_call is a call. Port of packages/collect/src/history/codex.ts at a3b870a6e; the spec is
//! spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use serde_json::Value;
use wsp_words::exit::HostError;

use crate::history::reader::{is_record, jsonl_files, stem, time_of, try_json, Call, CallKind, HistoryFile, HistoryReader};
use crate::host::Host;

/// The function names that carry a shell command (codex.ts:8).
pub const SHELLS: [&str; 4] = ["exec_command", "shell", "container.exec", "local_shell"];

pub fn codex_call(session: &str, name: &str, args: Option<&Value>, folder: Option<&str>, at: Option<f64>) -> Call {
    let arg = args.filter(|v| is_record(v));
    if SHELLS.contains(&name) {
        let cmd = arg.and_then(|a| a.get("cmd").filter(|v| !v.is_null()).or_else(|| a.get("command")));
        let line = match cmd {
            Some(Value::String(text)) => Some(text.clone()),
            Some(Value::Array(words)) if words.iter().all(Value::is_string) => {
                Some(words.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
            }
            _ => None,
        };
        if let Some(line) = line {
            return Call { session: session.to_string(), folder: folder.map(str::to_string), at, kind: CallKind::Shell { line } };
        }
    }
    Call { session: session.to_string(), folder: folder.map(str::to_string), at, kind: CallKind::Other { name: name.to_string() } }
}

/// The calls one rollout line holds. The folder is set by the first `session_meta` line whose payload names one, and
/// a later line never changes it.
fn calls_of_line(session: &str, folder: &mut Option<String>, line: &str) -> Vec<Call> {
    if folder.is_none() && line.contains("\"session_meta\"") {
        let meta = try_json(line);
        if let Some(cwd) = meta.as_ref().and_then(|m| m.get("payload")).filter(|p| is_record(p)).and_then(|p| p.get("cwd")).and_then(Value::as_str) {
            *folder = Some(cwd.to_string());
        }
    }
    if !line.contains("\"function_call\"") {
        return Vec::new();
    }
    let Some(row) = try_json(line) else { return Vec::new() };
    let Some(payload) = row.get("payload").filter(|p| is_record(p)) else { return Vec::new() };
    if row.get("type").and_then(Value::as_str) != Some("response_item")
        || payload.get("type").and_then(Value::as_str) != Some("function_call")
    {
        return Vec::new();
    }
    let Some(name) = payload.get("name").and_then(Value::as_str) else { return Vec::new() };
    let args = match payload.get("arguments") {
        Some(Value::String(text)) => try_json(text),
        other => other.cloned(),
    };
    vec![codex_call(session, name, args.as_ref(), folder.as_deref(), time_of(row.get("timestamp")))]
}

pub struct CodexReader;

impl HistoryReader for CodexReader {
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>> {
        async move { jsonl_files(host, root, &|file: &str| Some(stem(file))).await }.boxed_local()
    }

    fn read<'a>(&'a self, host: &'a Host, _root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>> {
        let session = stem(file);
        let mut folder: Option<String> = None;
        host.fs
            .lines(file, None)
            .map(move |item| match item {
                Ok(line) => calls_of_line(&session, &mut folder, &line).into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(HostError::new(e.to_string()))],
            })
            .flat_map(futures::stream::iter)
            .boxed_local()
    }

    fn holder(&self, _root: &str, file: &str) -> String {
        file.to_string()
    }
}
