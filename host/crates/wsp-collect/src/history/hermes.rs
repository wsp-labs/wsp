// SPDX-License-Identifier: AGPL-3.0-only
//! Hermes keeps every session in one SQLite database, read through the sqlite3 command in read-only mode. Port of
//! packages/collect/src/history/hermes.ts at a3b870a6e; the spec is spec/history_readers.md.
use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use serde_json::Value;
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;

use crate::history::reader::{is_record, stamp_of, try_json, Call, CallKind, HistoryFile, HistoryReader};
use crate::host::{Host, RunOptions};

pub const QUERY: &str = "select session_id, tool_calls from messages where tool_calls is not null";
/// The same rows with the folder each session ran in; a database without the sessions table falls back to QUERY.
pub const QUERY_WITH_CWD: &str =
    "select m.session_id, m.tool_calls, s.cwd from messages m left join sessions s on s.id = m.session_id where m.tool_calls is not null";

pub fn hermes_call(session: &str, name: &str, args: Option<&Value>, folder: Option<&str>) -> Call {
    let command = args.filter(|v| is_record(v)).and_then(|v| v.get("command")).and_then(Value::as_str);
    let kind = match command {
        Some(line) if name == "terminal" => CallKind::Shell { line: line.to_string() },
        _ => CallKind::Other { name: name.to_string() },
    };
    Call { session: session.to_string(), folder: folder.map(str::to_string), at: None, kind }
}

/// The calls in one row's tool_calls JSON: a JSON string of a list, or the list itself.
pub fn hermes_calls(session: &str, tool_calls: Option<&Value>, folder: Option<&str>) -> Vec<Call> {
    let list = match tool_calls {
        Some(Value::String(text)) => try_json(text),
        other => other.cloned(),
    };
    let Some(items) = list.as_ref().and_then(Value::as_array) else { return Vec::new() };
    items
        .iter()
        .filter_map(|item| {
            let function = item.get("function").filter(|f| is_record(f))?;
            let name = function.get("name")?.as_str()?;
            let args = match function.get("arguments") {
                Some(Value::String(text)) => try_json(text),
                other => other.cloned(),
            };
            Some(hermes_call(session, name, args.as_ref(), folder))
        })
        .collect()
}

async fn sqlite_json(host: &Host, file: &str, query: &str) -> Option<String> {
    let args = ["-readonly".to_string(), "-json".to_string(), file.to_string(), query.to_string()];
    host.exec.run("sqlite3", &args, RunOptions::default()).await
}

/// The calls in every row of the database: the joined query first, the plain one only when the first run answers
/// nothing at all.
async fn database_calls(host: &Host, file: &str) -> Result<Vec<Call>, HostError> {
    let out = match sqlite_json(host, file, QUERY_WITH_CWD).await {
        Some(out) => out,
        None => sqlite_json(host, file, QUERY)
            .await
            .ok_or_else(|| HostError::new(format!("{file} could not be read with sqlite3")))?,
    };
    let rows = if js_trim(&out).is_empty() {
        Vec::new()
    } else {
        match try_json(&out) {
            Some(Value::Array(rows)) => rows,
            _ => return Err(HostError::new(format!("{file}: sqlite3 returned no JSON"))),
        }
    };
    Ok(rows
        .iter()
        .filter_map(|row| {
            let session = row.get("session_id").and_then(Value::as_str)?;
            Some(hermes_calls(session, row.get("tool_calls"), row.get("cwd").and_then(Value::as_str)))
        })
        .flatten()
        .collect())
}

pub struct HermesReader;

impl HistoryReader for HermesReader {
    fn files<'a>(&'a self, host: &'a Host, root: &'a str) -> LocalBoxFuture<'a, Vec<HistoryFile>> {
        async move { stamp_of(host, root).await.into_iter().collect() }.boxed_local()
    }

    /// The two sqlite3 runs and their errors: `<file> could not be read with sqlite3` when neither answers,
    /// `<file>: sqlite3 returned no JSON` when the answer is not a JSON list; both as one `Err` item.
    fn read<'a>(&'a self, host: &'a Host, _root: &'a str, file: &'a str) -> LocalBoxStream<'a, Result<Call, HostError>> {
        futures::stream::once(database_calls(host, file))
            .map(|result| match result {
                Ok(calls) => calls.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            })
            .flat_map(futures::stream::iter)
            .boxed_local()
    }

    fn holder(&self, _root: &str, file: &str) -> String {
        file.to_string()
    }
}
