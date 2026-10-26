// SPDX-License-Identifier: AGPL-3.0-only
//! The value helpers, text and dates of expr.ts.
use super::*;

// ---- values ----

pub(super) fn jnum(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 9.0e15 {
        Value::from(x as i64)
    } else {
        Value::from(x)
    }
}

pub(super) fn fin(x: f64) -> Val {
    Some(if x.is_finite() { jnum(x) } else { Value::Null })
}

pub(super) fn nul() -> Val {
    Some(Value::Null)
}

pub(super) fn sval(s: String) -> Val {
    Some(Value::String(s))
}

pub(super) fn as_num(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|x| x.is_finite())
}

pub(super) fn is_list(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Array(_)))
}

pub(super) fn list_ref(v: Option<&Value>) -> Option<&Vec<Value>> {
    match v {
        Some(Value::Array(items)) => Some(items),
        _ => None,
    }
}

pub(super) fn missing(v: Option<&Value>) -> bool {
    matches!(v, None | Some(Value::Null))
}

pub(super) fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(_)) => true,
    }
}

/// A value as a format hole and str() write it: nothing for null, JSON for a list or record.
pub(super) fn text_of(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Some(other) => json_stringify(other),
    }
}

/// Text joined from literal words and the values it reads: null when everything it reads is missing, so a label
/// whose data has not come draws its placeholder rather than half a sentence.
pub(super) fn joined(parts: &[(bool, Val)]) -> Option<String> {
    if parts.iter().any(|(read, _)| *read) && parts.iter().all(|(read, v)| !read || missing(v.as_ref())) {
        return None;
    }
    Some(parts.iter().map(|(_, v)| text_of(v.as_ref())).collect())
}

/// JSON's key order for an object: array-index keys ascending, then the rest as given.
pub(super) fn js_object(entries: impl IntoIterator<Item = (String, Value)>) -> Value {
    let mut map: Map<String, Value> = Map::new();
    for (k, v) in entries {
        map.insert(k, v);
    }
    let ordered: Map<String, Value> = js_key_order(map.iter()).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Value::Object(ordered)
}

/// A time as ms: a number is ms epoch, an ISO string is parsed.
pub(super) fn ms(v: Option<&Value>) -> Option<f64> {
    if let Some(x) = as_num(v) {
        return Some(x);
    }
    match v {
        Some(Value::String(s)) => {
            let t = date_parse(s);
            (!t.is_nan()).then_some(t)
        }
        _ => None,
    }
}

pub(super) fn places(v: Option<&Value>, fallback: u32) -> u32 {
    as_num(v).map_or(fallback, |x| js_round(x).clamp(0.0, 6.0) as u32)
}

/// "3d 7h", "3h 24m", "12m 4s", "40s": the two largest units.
pub(super) fn span(ms_left: f64, two: bool) -> String {
    let s = js_round(ms_left / 1000.0).max(0.0);
    let d = (s / 86_400.0).floor();
    let h = ((s % 86_400.0) / 3_600.0).floor();
    let m = ((s % 3_600.0) / 60.0).floor();
    let sec = s % 60.0;
    if d > 0.0 {
        return if two && h > 0.0 { format!("{}d {}h", number_string(d), number_string(h)) } else { format!("{}d", number_string(d)) };
    }
    if h > 0.0 {
        return if two && m > 0.0 { format!("{}h {}m", number_string(h), number_string(m)) } else { format!("{}h", number_string(h)) };
    }
    if m > 0.0 {
        return if two && sec > 0.0 { format!("{}m {}s", number_string(m), number_string(sec)) } else { format!("{}m", number_string(m)) };
    }
    format!("{}s", number_string(sec))
}

pub(super) fn plain_number(n: f64, places: Option<u32>) -> String {
    match places {
        None => locale_number(n, Grouping::EnUs, 0, 2),
        Some(p) => locale_number(n, Grouping::EnUs, p, p),
    }
}

pub(super) const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const MONTHS_SHORT: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];

pub(super) fn local_fields(at: f64) -> Option<Fields> {
    let t = js_date::time_value(at)?;
    let zone = DateZone::system();
    Some(js_date::fields(t, zone.offset_seconds(t)))
}

/// `toLocaleDateString("en-GB", ...)` with the comma removed, as the TS's `.replace(",", "")` leaves it.
pub(super) fn en_gb_date(at: f64, long: bool) -> String {
    let Some(f) = local_fields(at) else {
        return "Invalid Date".to_string();
    };
    let weekday = WEEKDAYS[f.weekday as usize];
    if long {
        format!("{weekday} {} {} {}", f.day, MONTHS[f.month as usize - 1], js_date::intl_year(f.year))
    } else {
        format!("{} {} {}", &weekday[..3], f.day, MONTHS_SHORT[f.month as usize - 1])
    }
}

pub(super) fn clock_text(at: f64) -> String {
    let zone = DateZone::system();
    fmt_clock(Some(at), &|t: f64| zone.offset_seconds(t.trunc() as i64))
}

/// The app's own word for a known state (04, word()): check states, thread statuses, review and mergeable enums,
/// run states; any other string capitalised once.
fn known_word(x: &str) -> Option<&'static str> {
    if let Some((_, word)) = CHECK_STATE_WORDS.iter().find(|(k, _)| *k == x) {
        return Some(*word);
    }
    Some(match x {
        "starting" => "Starting",
        "working" => "Working",
        "needs-you" => "Needs you",
        "resting" => "Resting",
        "done" => "Done",
        "failed" => "Failed",
        "none" => "None",
        "approved" => "Approved",
        "changes_asked" => "Changes asked",
        "required" => "Review required",
        "mergeable" => "Mergeable",
        "conflicting" => "Conflicting",
        "unknown" => "Unknown",
        "idle" => "Not run yet",
        "held" => "Held",
        "running" => "Running",
        "open" => "Open",
        "merged" => "Merged",
        "closed" => "Closed",
        _ => return None,
    })
}

pub(super) fn slate_word(x: Option<&Value>) -> Val {
    let Some(Value::String(s)) = x else {
        return if missing(x) { nul() } else { sval(text_of(x)) };
    };
    if let Some(known) = known_word(s) {
        return sval(known.to_string());
    }
    let collapsed: String = {
        let mut out = String::new();
        let mut in_run = false;
        for ch in s.chars() {
            if ch == '_' || ch == '-' {
                if !in_run {
                    out.push(' ');
                }
                in_run = true;
            } else {
                out.push(ch);
                in_run = false;
            }
        }
        out
    };
    let spaced = js_trim(&collapsed);
    let mut chars = spaced.chars();
    let Some(first) = chars.next() else {
        return sval(String::new());
    };
    let head: String = if first.len_utf16() == 1 { first.to_uppercase().collect() } else { first.to_string() };
    sval(format!("{head}{}", chars.as_str()))
}

pub(super) fn value_text_for_split(sep: &str, s: &str) -> Vec<String> {
    if sep.is_empty() {
        return s.encode_utf16().map(|u| String::from_utf16_lossy(&[u])).collect();
    }
    s.split(sep).map(str::to_string).collect()
}

pub(super) fn normalize(v: Value) -> Value {
    match v {
        Value::Number(n) => jnum(n.as_f64().unwrap_or(f64::NAN)),
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        Value::Object(m) => js_object(m.into_iter().map(|(k, x)| (k, normalize(x)))),
        other => other,
    }
}

/// A list's first `cap` items, JavaScript's `slice(0, listItems)`.
pub(super) fn capped(mut items: Vec<Value>) -> Vec<Value> {
    items.truncate(SLATE_LIMITS.list_items);
    items
}
