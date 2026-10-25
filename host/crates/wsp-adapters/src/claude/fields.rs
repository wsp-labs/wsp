// SPDX-License-Identifier: AGPL-3.0-only
//! Readers for the loose fields of the JSON lines the CLI prints. TS: packages/adapter-claude/src/fields.ts at
//! a3b870a6e. Every reader takes `Option<&Value>`, the TS `unknown` with `undefined` as `None`.

use serde_json::{Map, Value};

use crate::port::SessionHarness;

/// A JSON object, keys in the order the line wrote them.
pub type Obj = Map<String, Value>;

/// An object; `None` for an array, null, any other value and absence. fields.ts:5.
pub fn rec(value: Option<&Value>) -> Option<&Obj> {
    todo!()
}

/// A string, the empty one included. fields.ts:11.
pub fn string(value: Option<&Value>) -> Option<&str> {
    todo!()
}

/// A finite number. JSON carries no NaN or infinity, so every JSON number reads. fields.ts:15.
pub fn num(value: Option<&Value>) -> Option<f64> {
    todo!()
}

/// The string items of an array, in order, the rest dropped; `None` for anything but an array. fields.ts:19.
pub fn str_arr(value: Option<&Value>) -> Option<Vec<String>> {
    todo!()
}

/// What a system/init line says of the harness: slash_commands, permissionMode, agents; `None` where it names none
/// of the three. fields.ts:23.
pub fn harness_of(init: &Obj) -> Option<SessionHarness> {
    todo!()
}

/// One stdout line as an event: trimmed, starting with `{`, parsing as an object whose `type` is a string. fields.ts:35.
pub fn parse_line(raw: &str) -> Option<Obj> {
    todo!()
}
