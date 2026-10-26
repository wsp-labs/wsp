// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/shape.ts at 65f6b89c7.
use serde_json::{Map, Value};
use wsp_wire::text::json_stringify;
use wsp_words::js::{js_key_order, js_trim, number_string, utf16};

pub const SLATE_RESULT_ROWS: usize = 50;

const COLUMNS: usize = 6;
const DEPTH: usize = 2;

type JsonRecord = Map<String, Value>;

#[derive(Clone, Debug, PartialEq)]
pub struct SlateColumn {
    pub key: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlateFact {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlateNested {
    pub title: String,
    pub shape: SlateResultShape,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SlateResultShape {
    None,
    Text(String),
    Table { rows: Vec<JsonRecord>, columns: Vec<SlateColumn> },
    Record { facts: Vec<SlateFact>, nested: Vec<SlateNested> },
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::Array(_) | Value::Object(_))
}

fn plain_items(v: &Value) -> Option<&[Value]> {
    match v {
        Value::Array(items) if items.iter().all(is_scalar) => Some(items),
        _ => None,
    }
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

fn is_path_key(key: &str) -> bool {
    let b = key.as_bytes();
    b.first().is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_') && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

/// A field name as a label: `receivedTime` and `received_time` read "Received time".
pub fn slate_field_words(key: &str) -> String {
    let mut spaced = String::new();
    let mut prev: Option<char> = None;
    for c in key.chars() {
        if let Some(p) = prev {
            if (p.is_ascii_lowercase() || p.is_ascii_digit()) && c.is_ascii_uppercase() {
                spaced.push(' ');
            }
        }
        spaced.push(c);
        prev = Some(c);
    }
    let mut words = String::new();
    let mut in_run = false;
    for c in spaced.chars() {
        if c == '_' || c == '-' {
            if !in_run {
                words.push(' ');
            }
            in_run = true;
        } else {
            words.push(c);
            in_run = false;
        }
    }
    let lowered = js_trim(&words).to_lowercase();
    let mut chars = lowered.chars();
    match chars.next() {
        None => key.to_string(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// The fields that hold a plain value in some row, in the order rows name them; a row reads them as `item.<key>`.
fn columns_of(rows: &[JsonRecord]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for row in rows.iter().take(SLATE_RESULT_ROWS) {
        for (key, value) in row {
            if keys.len() >= COLUMNS {
                return keys;
            }
            let blank = matches!(value, Value::Null) || matches!(value, Value::String(text) if text.is_empty());
            if !keys.contains(key) && is_path_key(key) && is_scalar(value) && !blank {
                keys.push(key.clone());
            }
        }
    }
    keys
}

fn shape_at(value: Option<&Value>, depth: usize) -> SlateResultShape {
    let Some(value) = value else {
        return SlateResultShape::None;
    };
    match value {
        Value::Null => SlateResultShape::None,
        Value::String(text) if text.is_empty() => SlateResultShape::None,
        Value::Array(items) => {
            let rows: Vec<JsonRecord> = items
                .iter()
                .filter_map(|item| match item {
                    Value::Object(record) => Some(record.clone()),
                    _ => None,
                })
                .collect();
            if !items.is_empty() && rows.len() == items.len() {
                let columns = columns_of(&rows).into_iter().map(|key| SlateColumn { title: slate_field_words(&key), key }).collect();
                return SlateResultShape::Table { rows, columns };
            }
            match plain_items(value) {
                Some([]) => SlateResultShape::None,
                Some(list) => SlateResultShape::Text(list.iter().map(scalar_text).collect::<Vec<_>>().join("\n")),
                None => SlateResultShape::Text(json_stringify(value)),
            }
        }
        Value::Object(fields) => {
            let mut facts = Vec::new();
            let mut nested = Vec::new();
            for (key, v) in js_key_order(fields.iter()) {
                if is_scalar(v) || plain_items(v).is_some() {
                    let shown = match v {
                        Value::Array(items) => items.iter().map(scalar_text).collect::<Vec<_>>().join(", "),
                        Value::Null => String::new(),
                        other => scalar_text(other),
                    };
                    if !shown.is_empty() {
                        facts.push(SlateFact { label: slate_field_words(key), value: shown });
                    }
                } else if depth + 1 >= DEPTH {
                    facts.push(SlateFact { label: slate_field_words(key), value: json_stringify(v) });
                } else {
                    nested.push(SlateNested { title: slate_field_words(key), shape: shape_at(Some(v), depth + 1) });
                }
            }
            SlateResultShape::Record { facts, nested }
        }
        scalar => SlateResultShape::Text(scalar_text(scalar)),
    }
}

/// A tool or resource run's result by its shape: a table, a record of facts and nested shapes, or text.
pub fn slate_result_shape(value: Option<&Value>) -> SlateResultShape {
    shape_at(value, 0)
}

fn cut_text(text: &str, n: usize) -> String {
    if utf16::len(text) > n {
        format!("{}…", utf16::cut(text, n - 1))
    } else {
        text.to_string()
    }
}

/// The shape as the sketch says it: what kind of thing shows, its size and its names, never a page of data.
pub fn sketch_slate_result(shape: &SlateResultShape) -> Vec<String> {
    match shape {
        SlateResultShape::None => Vec::new(),
        SlateResultShape::Text(text) => {
            let lines: Vec<&str> = text.split('\n').filter(|line| !line.is_empty()).collect();
            match lines.as_slice() {
                [] | [_] => vec![format!("text: {}", cut_text(lines.first().copied().unwrap_or(""), 60))],
                [first, ..] => vec![format!("text, {} lines: {}", lines.len(), cut_text(first, 48))],
            }
        }
        SlateResultShape::Table { rows, columns } => {
            let shown = rows.len().min(SLATE_RESULT_ROWS);
            let more = if rows.len() > shown { format!(" and {} more", rows.len() - shown) } else { String::new() };
            let plural = if shown == 1 { "" } else { "s" };
            let titles: Vec<&str> = columns.iter().map(|column| column.title.as_str()).collect();
            vec![format!("table of {shown} row{plural}{more}: {}", titles.join(", "))]
        }
        SlateResultShape::Record { facts, nested } => {
            let mut lines = Vec::new();
            if !facts.is_empty() {
                lines.push(facts.iter().map(|fact| format!("{} {}", fact.label, cut_text(&fact.value, 24))).collect::<Vec<_>>().join(", "));
            }
            for part in nested {
                for (at, line) in sketch_slate_result(&part.shape).into_iter().enumerate() {
                    lines.push(if at == 0 { format!("{}: {line}", part.title) } else { format!("  {line}") });
                }
            }
            lines
        }
    }
}
