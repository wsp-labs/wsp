// SPDX-License-Identifier: AGPL-3.0-only
//! Readers for the loose fields of the JSON lines the CLI prints. TS: packages/adapter-claude/src/fields.ts at
//! a3b870a6e. Every reader takes `Option<&Value>`, the TS `unknown` with `undefined` as `None`.

use serde_json::{Map, Value};

use crate::port::SessionHarness;

/// A JSON object, keys in the order the line wrote them.
pub type Obj = Map<String, Value>;

/// An object; `None` for an array, null, any other value and absence. fields.ts:5.
pub fn rec(value: Option<&Value>) -> Option<&Obj> {
    value?.as_object()
}

/// A string, the empty one included. fields.ts:11.
pub fn string(value: Option<&Value>) -> Option<&str> {
    value?.as_str()
}

/// A finite number. JSON carries no NaN or infinity, so every JSON number reads. fields.ts:15.
pub fn num(value: Option<&Value>) -> Option<f64> {
    value?.as_f64()
}

/// The string items of an array, in order, the rest dropped; `None` for anything but an array. fields.ts:19.
pub fn str_arr(value: Option<&Value>) -> Option<Vec<String>> {
    value?.as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(String::from)).collect())
}

/// What a system/init line says of the harness: slash_commands, permissionMode, agents; `None` where it names none
/// of the three. fields.ts:23.
pub fn harness_of(init: &Obj) -> Option<SessionHarness> {
    let slash_commands = str_arr(init.get("slash_commands"));
    let permission_mode = string(init.get("permissionMode")).map(String::from);
    let agents = str_arr(init.get("agents"));
    if slash_commands.is_none() && permission_mode.is_none() && agents.is_none() {
        return None;
    }
    Some(SessionHarness { slash_commands, permission_mode, agents })
}

/// One stdout line as an event: trimmed, starting with `{`, parsing as an object whose `type` is a string. fields.ts:35.
pub fn parse_line(raw: &str) -> Option<Obj> {
    let line = wsp_words::js::js_trim(raw);
    if !line.starts_with('{') {
        return None;
    }
    match wsp_words::v8_json::parse_json_lax(line)? {
        Value::Object(event) if event.get("type").is_some_and(Value::is_string) => Some(event),
        _ => None,
    }
}
