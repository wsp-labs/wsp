// SPDX-License-Identifier: AGPL-3.0-only
//! smol-toml's `stringify` (stringify.js), the writer `refer` falls back to (mcp.ts:1728): a table by its header, an
//! array of tables by `[[key]]`, every other value inline as `[ a, b ]` and `{ k = v }`. Open tables stand on a stack, so
//! no depth of tree overflows a thread's, as the `toml` crate's recursive writer does at 303 levels. The 500-level cap of
//! the reader keeps every tree under smol-toml's own `maxDepth` of 1000, so that error cannot arise here. A float JavaScript
//! cannot tell from an integer is an `Integer` in the tree (smol_toml.rs), so it is written as one.
use wsp_words::js::{number_string, to_fixed};

use crate::jsonc::js_string_with;

/// The offset a date read as `-00:00` carries in the tree: the `toml` crate reads it as `Custom { minutes: 0 }`, the sign lost,
/// and smol-toml writes it back as authored. No real offset is near it (they stop at 23:59), and `-i16::MAX` is the end the
/// crate's own Display of an offset can negate.
pub(crate) const UNKNOWN_OFFSET: i16 = -i16::MAX;

fn bare(key: &str) -> bool {
    !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A TOML string holds every Unicode scalar, so a char the JSON.parse port stands in for a lone surrogate is written as itself.
fn quoted(s: &str) -> String {
    js_string_with(s, false).replace('\u{7f}', "\\u007f")
}

fn format_key(key: &str) -> String {
    if bare(key) {
        key.to_string()
    } else {
        quoted(key)
    }
}

fn float_text(x: f64) -> String {
    if x.is_nan() {
        "nan".into()
    } else if x.is_infinite() {
        if x > 0.0 { "inf" } else { "-inf" }.into()
    } else if x.fract() == 0.0 && x.abs() > 9_007_199_254_740_991.0 {
        to_fixed(x, 1)
    } else {
        number_string(x)
    }
}

/// `TomlDate.toISOString` (date.js): the date and time as authored, the time cut to milliseconds and always written with them.
fn datetime_text(d: &toml::value::Datetime) -> String {
    let mut out = String::new();
    if let Some(date) = &d.date {
        out.push_str(&format!("{:04}-{:02}-{:02}", date.year, date.month, date.day));
    }
    if let Some(time) = &d.time {
        if d.date.is_some() {
            out.push('T');
        }
        out.push_str(&format!("{:02}:{:02}:{:02}.{:03}", time.hour, time.minute, time.second, time.nanosecond / 1_000_000));
    }
    match &d.offset {
        Some(toml::value::Offset::Z) => out.push('Z'),
        Some(toml::value::Offset::Custom { minutes: UNKNOWN_OFFSET }) => out.push_str("-00:00"),
        Some(toml::value::Offset::Custom { minutes }) => {
            out.push_str(&format!("{}{:02}:{:02}", if *minutes < 0 { '-' } else { '+' }, minutes.abs() / 60, minutes.abs() % 60))
        }
        None => {}
    }
    out
}

fn scalar_text(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => quoted(s),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => float_text(*f),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Datetime(d) => datetime_text(d),
        toml::Value::Array(_) | toml::Value::Table(_) => unreachable!("a scalar"),
    }
}

enum Open<'a> {
    Table(toml::map::Iter<'a>, bool),
    Array(std::slice::Iter<'a, toml::Value>, bool),
}

/// A value on one line: `stringifyValue`.
fn inline(root: &toml::Value) -> String {
    let mut out = String::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut entering = Some(root);
    loop {
        if let Some(v) = entering.take() {
            match v {
                toml::Value::Table(t) if t.is_empty() => out.push_str("{}"),
                toml::Value::Table(t) => {
                    out.push_str("{ ");
                    stack.push(Open::Table(t.iter(), true));
                }
                toml::Value::Array(a) if a.is_empty() => out.push_str("[]"),
                toml::Value::Array(a) => {
                    out.push_str("[ ");
                    stack.push(Open::Array(a.iter(), true));
                }
                scalar => out.push_str(&scalar_text(scalar)),
            }
            continue;
        }
        match stack.last_mut() {
            None => return out,
            Some(Open::Table(members, first)) => match members.next() {
                Some((k, v)) => {
                    if !std::mem::replace(first, false) {
                        out.push_str(", ");
                    }
                    out.push_str(&format_key(k));
                    out.push_str(" = ");
                    entering = Some(v);
                }
                None => {
                    out.push_str(" }");
                    stack.pop();
                }
            },
            Some(Open::Array(items, first)) => match items.next() {
                Some(v) => {
                    if !std::mem::replace(first, false) {
                        out.push_str(", ");
                    }
                    entering = Some(v);
                }
                None => {
                    out.push_str(" ]");
                    stack.pop();
                }
            },
        }
    }
}

fn array_of_tables(v: &toml::Value) -> Option<&[toml::Value]> {
    match v {
        toml::Value::Array(a) if !a.is_empty() && a.iter().all(toml::Value::is_table) => Some(a),
        _ => None,
    }
}

enum Frame<'a> {
    Table { header: String, prefix: String, members: toml::map::Iter<'a>, preamble: String, tables: String },
    Tables { key: String, items: std::slice::Iter<'a, toml::Value>, res: String },
}

fn table_frame(header: String, prefix: String, table: &toml::Table) -> Frame<'_> {
    Frame::Table { header, prefix, members: table.iter(), preamble: String::new(), tables: String::new() }
}

/// `stringify(table)`: the whole table as TOML text ending in a newline.
pub fn stringify(root: &toml::Table) -> String {
    let mut stack = vec![table_frame(String::new(), String::new(), root)];
    let mut done: Option<String> = None;
    loop {
        if let Some(text) = done.take() {
            match stack.last_mut() {
                None => return if text.ends_with('\n') { text } else { text + "\n" },
                Some(Frame::Table { tables, .. }) => {
                    if !tables.is_empty() {
                        tables.push('\n');
                    }
                    tables.push_str(&text);
                }
                Some(Frame::Tables { res, .. }) => res.push_str(&text),
            }
            continue;
        }
        let child = match stack.last_mut().expect("the root stays open until it is returned") {
            Frame::Table { header, prefix, members, preamble, tables } => match members.next() {
                Some((k, v)) => {
                    let key = format_key(k);
                    let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };
                    if let Some(items) = array_of_tables(v) {
                        Some(Frame::Tables { key: path, items: items.iter(), res: String::new() })
                    } else if let toml::Value::Table(t) = v {
                        Some(table_frame(path.clone(), path, t))
                    } else {
                        preamble.push_str(&key);
                        preamble.push_str(" = ");
                        preamble.push_str(&inline(v));
                        preamble.push('\n');
                        None
                    }
                }
                None => {
                    let mut head = std::mem::take(preamble);
                    if !header.is_empty() && (!head.is_empty() || tables.is_empty()) {
                        head = if head.is_empty() { format!("[{header}]") } else { format!("[{header}]\n{head}") };
                    }
                    done = Some(match (head.is_empty(), tables.is_empty()) {
                        (false, false) => format!("{head}\n{tables}"),
                        (false, true) => head,
                        (true, _) => std::mem::take(tables),
                    });
                    stack.pop();
                    None
                }
            },
            Frame::Tables { key, items, res } => match items.next() {
                Some(toml::Value::Table(t)) => {
                    if !res.is_empty() {
                        res.push('\n');
                    }
                    res.push_str(&format!("[[{key}]]\n"));
                    Some(table_frame(String::new(), key.clone(), t))
                }
                Some(_) => unreachable!("an array of tables holds tables"),
                None => {
                    done = Some(std::mem::take(res));
                    stack.pop();
                    None
                }
            },
        };
        if let Some(frame) = child {
            stack.push(frame);
        }
    }
}
