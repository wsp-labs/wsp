// SPDX-License-Identifier: AGPL-3.0-only
//! JSON with the comments and trailing commas Gemini CLI and OpenCode accept, read in one pass, and JSON written as
//! JavaScript writes it. Port of packages/catalog/src/jsonc.ts at a3b870a6e; the spec is spec/jsonc.md.
use serde_json::{Map, Value};

use wsp_words::exit::HostError;

/// A JSON object as the catalog reads one: keys in the order the text names them first.
pub type JsonObject = Map<String, Value>;

/// The value the text holds, comments and trailing commas taken out first; a real syntax error is an error as
/// `JSON.parse`'s throw is (jsonc.ts:12). A key named twice keeps its first place and its last value.
pub fn read_jsonc(text: &str) -> Result<Value, HostError> {
    todo!()
}

/// The value alone (jsonc.ts:44).
pub fn parse_jsonc(text: &str) -> Result<Value, HostError> {
    todo!()
}

/// The value as an object, or nothing for an array, a scalar or null (jsonc.ts:47).
pub fn json_object(value: &Value) -> Option<&JsonObject> {
    todo!()
}

/// `JSON.stringify(value)`: numbers as JavaScript writes them (rule 4.25: 1.0 is `1`, 1e21 is `1e+21`, -0 is `0`),
/// strings escaped as JavaScript escapes them, keys in the map's order.
pub fn js_stringify(value: &Value) -> String {
    todo!()
}

/// `JSON.stringify(value, null, 2)`: the same, laid out with two-space indents; an empty object is `{}` and an empty
/// array `[]`.
pub fn js_stringify_pretty(value: &Value) -> String {
    todo!()
}
