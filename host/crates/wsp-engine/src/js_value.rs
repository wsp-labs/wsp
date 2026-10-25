// SPDX-License-Identifier: AGPL-3.0-only
//! What JavaScript writes for a value a provider's JSON carries where the TS never checked its type.
use serde_json::Value;
use wsp_words::js::number_string;

/// `String(value)`: what a template literal writes for a JSON value.
pub fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => items.iter().map(|v| if v.is_null() { String::new() } else { js_string(v) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}
