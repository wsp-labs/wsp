// SPDX-License-Identifier: AGPL-3.0-only
//! Held text, figures and the trend line a sketch draws.
use super::*;

/// A held prop as the window reads it: a sentence holds the piece, and anything else (null, false, blank) holds nothing.
pub fn slate_held_text(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(text)) if !js_trim(text).is_empty() => Some(text.clone()),
        _ => None,
    }
}
/// A row's or an item's look as a mark after it: [bad strong mono], nothing when it has none.
pub(super) fn marks(r: &Row) -> String {
    let text = |key: &str| r.get(key).and_then(Value::as_str);
    let mut words: Vec<&str> = [
        text("tone").filter(|t| *t != "default"),
        text("emphasis").filter(|e| *e != "normal"),
        (r.get("mono") == Some(&Value::Bool(true))).then_some("mono"),
        text("align").filter(|a| *a != "start"),
        text("width"),
    ]
    .into_iter()
    .flatten()
    .collect();
    words.retain(|w| !w.is_empty());
    if words.is_empty() {
        String::new()
    } else {
        format!(" [{}]", words.join(" "))
    }
}

pub(super) fn bar(value: Option<&Value>, max: Option<&Value>) -> String {
    let Some(value) = num_of(value) else {
        return "[..........]".to_string();
    };
    let share = match num_of(max) {
        Some(max) if max > 0.0 => value / max,
        _ => value / 100.0,
    };
    let cells = js_round(share * 10.0).clamp(0.0, 10.0) as usize;
    format!("[{}{}]", "#".repeat(cells), ".".repeat(10 - cells))
}

pub(super) fn plain_number(n: f64) -> String {
    locale_number(n, Grouping::EnUs, 0, 2)
}

/// A number in the words a piece's format names, the one rule the panel and the sketch both draw it by: a percent of
/// max where there is one, a fraction over max (the number alone with none), a duration from milliseconds, an integer
/// with its thousands grouped, none as nothing, and any other format the number itself to two places.
pub fn slate_figure(value: f64, format: &str, max: Option<f64>) -> String {
    match format {
        "percent" => {
            let share = match max {
                Some(max) if max > 0.0 => value / max * 100.0,
                _ => value,
            };
            format!("{}%", number_string(js_round(share)))
        }
        "fraction" => match max {
            None => plain_number(value),
            Some(max) => format!("{}/{}", plain_number(value), plain_number(max)),
        },
        "tokens" => fmt_tokens(value),
        "bytes" => fmt_bytes(value),
        "usd" => fmt_cost(value),
        "inr" => fmt_inr(value),
        "duration" => fmt_duration(value, None),
        "integer" => locale_number(js_round(value), Grouping::EnUs, 0, 0),
        "none" => String::new(),
        _ => plain_number(value),
    }
}

pub(super) fn figure(format: &str, value: Option<&Value>, max: Option<f64>) -> String {
    match num_of(value) {
        Some(n) => slate_figure(n, format, max),
        None => shown(value),
    }
}

pub(super) fn join_with(separator: &str, parts: &[String]) -> String {
    parts.iter().filter(|p| !p.is_empty()).map(String::as_str).collect::<Vec<_>>().join(separator)
}

pub(super) fn join2(parts: &[String]) -> String {
    join_with("  ", parts)
}

pub(super) fn js_min(vals: &[f64]) -> f64 {
    vals.iter().copied().fold(f64::INFINITY, f64::min)
}

pub(super) fn js_max(vals: &[f64]) -> f64 {
    vals.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

pub(super) fn trend(label: &str, vals: &[f64]) -> String {
    match vals.last() {
        None => format!("{label}  no points yet"),
        Some(last) => js_trim_start(&format!(
            "{label}  last {}, min {}, max {} over {} points",
            number_string(*last),
            number_string(js_min(vals)),
            number_string(js_max(vals)),
            vals.len()
        ))
        .to_string(),
    }
}
