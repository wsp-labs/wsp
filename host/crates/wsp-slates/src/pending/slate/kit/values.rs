// SPDX-License-Identifier: AGPL-3.0-only
//! Reads a prop's value as the sketch and the window read it.
use super::*;

/// A value as a sketch shows it: nothing for missing.
pub(super) fn shown(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Number(n)) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Some(other) => json_stringify(other),
    }
}

/// `String(v)` for a value a template writes as it is.
pub(super) fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        other => shown(Some(other)),
    }
}

pub(super) fn word_of(v: Option<Value>, default: &str) -> String {
    match v {
        None | Some(Value::Null) => default.to_string(),
        Some(Value::String(text)) => text,
        Some(other) => js_string(&other),
    }
}

pub(super) fn num_of(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Number(n)) => n.as_f64().filter(|x| x.is_finite()),
        _ => None,
    }
}

pub(super) fn numbers(v: Option<&Value>) -> Vec<f64> {
    match v {
        Some(Value::Array(items)) => items.iter().filter_map(|x| num_of(Some(x))).collect(),
        _ => Vec::new(),
    }
}

pub(super) fn list_of(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    }
}

pub(super) fn rec_of(v: Option<&Value>) -> Row {
    match v {
        Some(Value::Object(fields)) => fields.iter().map(|(k, x)| (k.clone(), x.clone())).collect(),
        _ => Row::new(),
    }
}

pub(super) fn binding_of(v: Option<&Value>) -> Option<&str> {
    let v = v?;
    if is_slate_binding(v) {
        v.get("bind")?.as_str()
    } else {
        None
    }
}

/// A row record built from its entries; a missing entry is null, as the sketch reads it.
pub(super) fn spec_of(entries: Vec<(&str, Option<Value>)>) -> Value {
    Value::Object(entries.into_iter().map(|(k, v)| (k.to_string(), v.unwrap_or(Value::Null))).collect())
}

pub(super) fn pick(r: &Row, keys: &[&str]) -> Row {
    keys.iter().filter_map(|k| r.get(*k).map(|v| (k.to_string(), v.clone()))).collect()
}

pub(super) fn or_default(v: Option<Value>, default: Value) -> Value {
    match v {
        None | Some(Value::Null) => default,
        Some(x) => x,
    }
}

pub(super) fn is_secret_handle(v: Option<&Value>) -> bool {
    let Some(Value::Object(fields)) = v else {
        return false;
    };
    fields.get("secret") == Some(&Value::Bool(true))
        && matches!(fields.get("set"), Some(Value::Bool(_)))
        && matches!(fields.get("len"), Some(Value::Number(_)))
        && fields.keys().all(|k| matches!(k.as_str(), "secret" | "set" | "len" | "at"))
}

pub(super) fn clock_of(at: f64) -> String {
    let zone = DateZone::system();
    fmt_clock(Some(at), &|ms: f64| time_value(ms).map_or(0, |t| zone.offset_seconds(t)))
}
