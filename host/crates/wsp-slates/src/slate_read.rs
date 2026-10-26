// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-read.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 1-4.
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{Map, Value};
use wsp_words::js::utf16;

use crate::pending::slate::{is_slate_source, slate_problem, SlateDoc, SlateProblem};

pub fn is_run_record(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Object(m)) if m.get("state").is_some_and(Value::is_string) && m.get("runs").is_some_and(Value::is_number))
}

/// UTF-16 units.
pub const READ_OUTPUT_CHARS: usize = 4_000;

const SUMMARY_KEYS: [&str; 8] = ["state", "why", "exit", "ms", "runs", "stale", "refreshing", "endedAt"];

/// `rec` none is the TS `undefined`.
pub fn run_shown(rec: Option<&Value>, name: &str, named: bool) -> Value {
    let full = match rec {
        Some(Value::Object(m)) if is_run_record(rec) => m,
        _ => return rec.cloned().unwrap_or(Value::Null),
    };
    if named {
        let kept: Map<String, Value> =
            full.iter().filter(|(k, _)| SUMMARY_KEYS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
        return Value::Object(kept);
    }
    let json_present = full.contains_key("json");
    let mut out = Map::new();
    for (k, v) in full {
        if k == "out" && json_present {
            continue;
        }
        let shown = match (k.as_str(), v) {
            ("out" | "err", Value::String(text)) => Value::String(cut_read(k, name, text)),
            ("lines", Value::Array(lines)) => Value::Array(lines[lines.len().saturating_sub(50)..].to_vec()),
            _ => v.clone(),
        };
        out.insert(k.clone(), shown);
    }
    Value::Object(out)
}

fn cut_read(field: &str, name: &str, text: &str) -> String {
    let len = utf16::len(text);
    if len <= READ_OUTPUT_CHARS {
        return text.to_string();
    }
    format!("{}… ({len} characters; values [\"${name}.{field}\"] reads it whole)", utf16::cut(text, READ_OUTPUT_CHARS))
}

pub fn misread(doc: Option<&SlateDoc>, path: &str) -> Vec<SlateProblem> {
    static SHAPE: OnceLock<Regex> = OnceLock::new();
    let shape = SHAPE.get_or_init(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z0-9_]+|\[[0-9]+\])*$").expect("a fixed pattern"));
    if !shape.is_match(path) {
        return vec![];
    }
    let root = path.split(['.', '[']).next().unwrap_or(path);
    let fix = format!("${path}");
    if let Some(d) = doc {
        if d.runs.contains_key(root) {
            let message = format!("{path} is not a source: ${root} is a run, read as ${path}; its record is also under runs.${root}");
            return vec![slate_problem("X401", &message, Some(&fix))];
        }
        if d.values.contains_key(root) || d.derived.contains_key(root) {
            let message = format!("{path} is not a source: ${root} is a value, read as ${path}");
            return vec![slate_problem("X401", &message, Some(&fix))];
        }
    }
    if !is_slate_source(root) {
        let message = format!("{root} is not a source and not this slate's; a slate's own names start with $");
        return vec![slate_problem("X401", &message, None)];
    }
    vec![]
}
