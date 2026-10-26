// SPDX-License-Identifier: AGPL-3.0-only
//! Readers for the loose fields of the JSON lines the app server sends. TS adapter-codex/src/fields.ts. Spec:
//! spec/codex_rpc.md rule 13.

use serde_json::{Map, Value};

/// An object, not an array and not null. TS fields.ts:4.
pub fn rec(value: Option<&Value>) -> Option<&Map<String, Value>> {
    value?.as_object()
}

/// Any string, the empty string included. TS fields.ts:8.
pub fn str(value: Option<&Value>) -> Option<&str> {
    value?.as_str()
}

/// A finite number. TS fields.ts:12.
pub fn count(value: Option<&Value>) -> Option<f64> {
    value?.as_f64().filter(|n| n.is_finite())
}
