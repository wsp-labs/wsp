// SPDX-License-Identifier: AGPL-3.0-only
//! A Claude Code session copied through one message to a new session beside it. TS:
//! packages/adapter-claude/src/chain.ts at a3b870a6e.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde_json::{json, Value};

use crate::claude::landmines::{is_uuid, new_session_id};
use crate::pending::{fmt_duration, shell_quote};
use crate::port::{end_run, AdapterError, Env, ExecOptions, ExecStreamFactory};
use wsp_wire::text::json_stringify;
use wsp_words::js::{number_string, to_number, utf16};
use wsp_words::v8_json::parse_json_lax;

/// chain.ts:23.
pub const COPY_WALL_MS: u64 = 90_000;

const READ_STRING: u32 = 200;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// chain.ts:30.
pub fn no_conversation_line(session: &str) -> String {
    format!("No conversation found with session ID: {session}")
}

fn uuid(what: &str, id: &str) -> Result<String, AdapterError> {
    if is_uuid(id) {
        Ok(id.to_string())
    } else {
        Err(AdapterError(format!("{what} must be a UUID, got \"{id}\"")))
    }
}

/// chain.ts:39. `Err("session identifier must be a UUID, got \"<id>\"")`.
pub fn session_file_line(config_dir: &str, session: &str) -> Result<String, AdapterError> {
    let projects = shell_quote(&format!("{config_dir}/projects"));
    let session = uuid("session identifier", session)?;
    let missing = shell_quote(&no_conversation_line(&session));
    Ok(format!("src=$(ls {projects}/*/{session}.jsonl 2>/dev/null | head -n 1); [ -n \"$src\" ] || {{ echo {missing} >&2; exit 1; }}"))
}

/// chain.ts:46.
pub fn chain_read_command(session: &str, config_dir: &str) -> Result<String, AdapterError> {
    let blank = shell_quote(&format!(r#"s/([,:{{[])"([^"\\]|\\.){{{READ_STRING},}}"/\1""/g"#));
    Ok(format!("{}; n=$(wc -l < \"$src\"); echo $n; head -n \"$n\" \"$src\" | sed -E {blank} | grep -n -F '\"uuid\":\"'", session_file_line(config_dir, session)?))
}

/// One call an assistant line holds.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainCall {
    pub id: String,
    pub name: String,
    /// The call's input, `{}` where absent.
    pub input: Value,
}

/// One message line of the read. chain.ts:52.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainLine {
    pub at: u64,
    pub uuid: String,
    pub parent: Option<String>,
    pub r#type: String,
    pub boundary: bool,
    pub message_id: Option<String>,
    pub calls: Vec<ChainCall>,
    pub answers: Vec<String>,
}

/// chain.ts:70.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadChain {
    pub total: u64,
    pub lines: Vec<ChainLine>,
}

/// JavaScript's `String(value)` of a JSON value read off a loose field; an absent field is "undefined".
pub(crate) fn js_string(value: Option<&Value>) -> String {
    match value {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Number(number)) => number_string(number.as_f64().unwrap_or(f64::NAN)),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items.iter().map(|item| if item.is_null() { String::new() } else { js_string(Some(item)) }).collect::<Vec<_>>().join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

fn safe_count(value: f64) -> Option<u64> {
    (value.fract() == 0.0 && (0.0..=MAX_SAFE_INTEGER).contains(&value)).then_some(value as u64)
}

/// chain.ts:70. `Err` with the read's last line (or "the session's file could not be read") where its first line is
/// no whole number.
pub fn read_chain(read: &[String]) -> Result<ReadChain, AdapterError> {
    let total = read.first().map_or(f64::NAN, |first| to_number(first));
    let Some(total) = safe_count(total) else {
        return Err(AdapterError(read.last().cloned().unwrap_or_else(|| "the session's file could not be read".into())));
    };
    let mut lines = Vec::new();
    for raw in read.iter().skip(1) {
        let colon = utf16::index_of(raw, ":", 0).map_or(-1, |at| at as i64);
        let at = to_number(&utf16::slice(raw, 0, Some(colon)));
        let Some(text) = parse_json_lax(&utf16::slice(raw, colon + 1, None)) else { continue };
        let Value::Object(line) = text else { continue };
        let (Some(at), Some(uuid)) = (safe_count(at), line.get("uuid").and_then(Value::as_str)) else { continue };
        let kind = line.get("type").and_then(Value::as_str).unwrap_or("");
        let message = line.get("message").and_then(Value::as_object);
        let blocks: Vec<&serde_json::Map<String, Value>> = message.and_then(|m| m.get("content")).and_then(Value::as_array).map_or(vec![], |items| items.iter().filter_map(Value::as_object).collect());
        let block_text = |block: &serde_json::Map<String, Value>, key: &str| block.get(key).and_then(Value::as_str).map(String::from);
        lines.push(ChainLine {
            at,
            uuid: uuid.to_string(),
            parent: line.get("parentUuid").and_then(Value::as_str).map(String::from),
            r#type: kind.to_string(),
            boundary: kind == "system" && line.get("subtype").and_then(Value::as_str) == Some("compact_boundary"),
            message_id: message.and_then(|m| m.get("id")).and_then(Value::as_str).filter(|_| kind == "assistant").map(String::from),
            calls: if kind == "assistant" {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                    .filter_map(|b| {
                        Some(ChainCall { id: block_text(b, "id")?, name: js_string(b.get("name")), input: b.get("input").filter(|v| !v.is_null()).cloned().unwrap_or_else(|| json!({})) })
                    })
                    .collect()
            } else {
                vec![]
            },
            answers: if kind == "user" {
                blocks.iter().filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_result")).filter_map(|b| block_text(b, "tool_use_id")).collect()
            } else {
                vec![]
            },
        });
    }
    Ok(ReadChain { total, lines })
}

/// chain.ts:104. Line numbers ascending.
pub fn chain_through(lines: &[ChainLine], anchor: &str) -> Result<Vec<u64>, AdapterError> {
    let by_uuid: HashMap<&str, &ChainLine> = lines.iter().map(|line| (line.uuid.as_str(), line)).collect();
    if !by_uuid.contains_key(anchor) {
        return Err(AdapterError(format!("the session holds no message {anchor}")));
    }
    let mut kept = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut next = Some(anchor);
    while let Some(at) = next.take() {
        let line = by_uuid.get(at).copied().filter(|_| !seen.contains(at)).ok_or_else(|| AdapterError(format!("the session's chain to message {anchor} is broken at {at}")))?;
        seen.insert(at);
        kept.push(line.at);
        if !line.boundary {
            next = line.parent.as_deref();
        }
    }
    kept.sort_unstable();
    Ok(kept)
}

/// Where a copy of a running session ends. chain.ts:124.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveEnd {
    pub anchor: String,
    pub running: Vec<String>,
}

/// chain.ts:124.
pub fn live_end(lines: &[ChainLine]) -> Option<LiveEnd> {
    let last = lines.last()?;
    let newest = lines.iter().rev().find(|line| line.r#type == "assistant");
    let answered: HashSet<&str> = lines.iter().flat_map(|line| line.answers.iter().map(String::as_str)).collect();
    let message = newest.and_then(|line| line.message_id.as_deref());
    let open: Vec<&ChainCall> = match message {
        None => vec![],
        Some(message) => lines.iter().filter(|line| line.message_id.as_deref() == Some(message)).flat_map(|line| line.calls.iter()).filter(|call| !answered.contains(call.id.as_str())).collect(),
    };
    if open.is_empty() {
        return Some(LiveEnd { anchor: last.uuid.clone(), running: vec![] });
    }
    let first = lines.iter().find(|line| line.message_id.as_deref() == message)?;
    Some(LiveEnd {
        anchor: first.parent.clone()?,
        running: open.iter().map(|call| format!("{} {}", call.name, utf16::slice(&json_stringify(&call.input), 0, Some(300)))).collect(),
    })
}

/// chain.ts:137.
pub fn line_ranges(numbers: &[u64]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < numbers.len() {
        let mut j = i;
        while j + 1 < numbers.len() && numbers[j + 1] == numbers[j] + 1 {
            j += 1;
        }
        out.push(if i == j { numbers[i].to_string() } else { format!("{}-{}", numbers[i], numbers[j]) });
        i = j + 1;
    }
    out.join(",")
}

/// chain.ts:151.
pub fn chain_write_command(session: &str, fork: &str, config_dir: &str, count: u64) -> Result<String, AdapterError> {
    let fork = uuid("session identifier", fork)?;
    if count < 1 {
        return Err(AdapterError(format!("a copy keeps a whole number of lines, got {count}")));
    }
    let pick = shell_quote(r#"NR == FNR { n = split($0, r, ","); for (i = 1; i <= n; i++) { m = split(r[i], ab, "-"); for (j = ab[1] + 0; j <= ab[m] + 0; j++) keep[j] = 1 } next } FNR in keep"#);
    let short = shell_quote(&format!("the copy of session {session} came out short"));
    let source = session_file_line(config_dir, session)?;
    Ok(format!(
        "{source}; tmp=\"${{src%/*}}/.{fork}.jsonl.part\"; awk {pick} - \"$src\" > \"$tmp\" && [ $(($(wc -l < \"$tmp\"))) -eq {count} ] && mv -f \"$tmp\" \"${{src%/*}}/{fork}.jsonl\" || {{ rm -f \"$tmp\"; echo {short} >&2; exit 1; }}"
    ))
}

/// chain.ts:161.
pub fn copy_cleanup_command(fork: &str, config_dir: &str) -> Result<String, AdapterError> {
    let fork = uuid("session identifier", fork)?;
    let projects = shell_quote(&format!("{config_dir}/projects"));
    Ok(format!("rm -rf {projects}/*/{fork}.jsonl {projects}/*/{fork}"))
}

/// chain.ts:168.
pub fn copy_wall_line(ms: u64) -> String {
    format!("the copy of the session had not finished after {} and was stopped", fmt_duration(ms as f64))
}

/// chain.ts:171.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyThrough {
    Anchor(String),
    Live,
}

/// What copy_session takes. chain.ts:179.
#[derive(Clone)]
pub struct CopyOptions {
    pub exec: Arc<dyn ExecStreamFactory>,
    pub env: Env,
    pub config_dir: String,
    pub session: String,
    pub through: CopyThrough,
    pub grace_ms: u64,
    /// COPY_WALL_MS where `None`.
    pub wall_ms: Option<u64>,
}

/// A written copy and the calls a live copy left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopiedSession {
    pub session: String,
    pub running: Vec<String>,
}

/// One run of the copy's shell: every line it printed, and its exit code.
struct CopyRun {
    lines: Vec<String>,
    code: Option<i32>,
}

async fn run_copy(o: &CopyOptions, command: &str, input: Option<Vec<String>>, wall_ms: u64) -> Result<CopyRun, AdapterError> {
    let closes_input = input.is_some();
    let stream = o.exec.launch(command, ExecOptions { env: o.env.clone(), input, ..Default::default() });
    if closes_input {
        stream.close_input();
    }
    let mut lines = Vec::new();
    let collected = tokio::time::timeout(Duration::from_millis(wall_ms), async {
        let mut items = stream.lines();
        while let Some(item) = items.next().await {
            lines.push(item.map_err(|failure| failure.message)?);
        }
        Ok::<(), String>(())
    })
    .await;
    match collected {
        Err(_) => {
            end_run(&*stream, o.grace_ms).await;
            Err(AdapterError(copy_wall_line(wall_ms)))
        }
        Ok(Err(message)) => Err(AdapterError(message)),
        Ok(Ok(())) => Ok(CopyRun { lines, code: stream.exited().await }),
    }
}

/// chain.ts:179. Two runs on `exec`: the read, then the write with the ranges as its one input line.
pub async fn copy_session(o: CopyOptions) -> Result<CopiedSession, AdapterError> {
    let wall_ms = o.wall_ms.unwrap_or(COPY_WALL_MS);
    let read = run_copy(&o, &chain_read_command(&o.session, &o.config_dir)?, None, wall_ms).await?;
    if read.code != Some(0) {
        return Err(AdapterError(read.lines.last().cloned().unwrap_or_else(|| no_conversation_line(&o.session))));
    }
    let lines = read_chain(&read.lines)?.lines;
    let end = match &o.through {
        CopyThrough::Anchor(anchor) => Some(LiveEnd { anchor: anchor.clone(), running: vec![] }),
        CopyThrough::Live => live_end(&lines),
    }
    .ok_or_else(|| AdapterError(format!("the session {} holds no message to copy", o.session)))?;
    let kept = chain_through(&lines, &end.anchor)?;
    let fork = new_session_id();
    let command = chain_write_command(&o.session, &fork, &o.config_dir, kept.len() as u64)?;
    let wrote = run_copy(&o, &command, Some(vec![line_ranges(&kept)]), wall_ms).await?;
    if wrote.code != Some(0) {
        return Err(AdapterError(wrote.lines.last().cloned().unwrap_or_else(|| format!("the copy of session {} was not written", o.session))));
    }
    Ok(CopiedSession { session: fork, running: end.running })
}
