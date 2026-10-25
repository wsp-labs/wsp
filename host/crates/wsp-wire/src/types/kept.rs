// SPDX-License-Identifier: AGPL-3.0-only
//! A value the TS keeps as it was sent (`z.unknown()`, a passthrough object's other keys), written as `JSON.stringify`
//! writes the JavaScript value it parsed: `5.0` and `1e2` are `5` and `100`, `-0` is `0`, and the keys that read as array
//! indexes come first. See spec/check.md rule 5.

use indexmap::IndexMap;
use serde::de::Deserializer;
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::check::Check;
use crate::types::js_number::JsNumber;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Kept(pub Value);

impl Serialize for Kept {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        write_value(&self.0, s)
    }
}

impl<'de> Deserialize<'de> for Kept {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Value::deserialize(d).map(Kept)
    }
}

impl Check for Kept {
    fn pass(_value: &Value) -> Option<Vec<crate::types::SchemaError>> {
        Some(Vec::new())
    }
}

struct Part<'a>(&'a Value);

impl Serialize for Part<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        write_value(self.0, s)
    }
}

/// Whether a key reads as a JavaScript array index: a canonical non-negative integer below 2^32 - 1.
pub fn is_index(key: &str) -> bool {
    !key.is_empty()
        && key.bytes().all(|b| b.is_ascii_digit())
        && (key == "0" || !key.starts_with('0'))
        && key.parse::<u64>().is_ok_and(|n| n < 4_294_967_295)
}

/// The entries of an object in the order JavaScript holds them: index-like keys first, ascending, then the rest as they were.
pub fn js_order<'a, V>(entries: impl Iterator<Item = (&'a str, V)>) -> Vec<(&'a str, V)> {
    let (mut indexed, rest): (Vec<_>, Vec<_>) = entries.partition(|(k, _)| is_index(k));
    indexed.sort_by_key(|(k, _)| k.parse::<u64>().unwrap_or(0));
    indexed.extend(rest);
    indexed
}

fn write_value<S: Serializer>(value: &Value, s: S) -> Result<S::Ok, S::Error> {
    match value {
        Value::Number(n) => JsNumber(n.as_f64().unwrap_or(0.0)).serialize(s),
        Value::Array(items) => {
            let mut seq = s.serialize_seq(Some(items.len()))?;
            for item in items {
                seq.serialize_element(&Part(item))?;
            }
            seq.end()
        }
        Value::Object(map) => {
            let mut out = s.serialize_map(Some(map.len()))?;
            for (key, item) in js_order(map.iter().map(|(k, v)| (k.as_str(), v))) {
                out.serialize_entry(key, &Part(item))?;
            }
            out.end()
        }
        other => other.serialize(s),
    }
}

/// Writes `entries` as one JavaScript object: index-like keys first, each value as `Kept` writes it.
pub fn serialize_object<S: Serializer>(entries: &[(String, Value)], s: S) -> Result<S::Ok, S::Error> {
    let mut out = s.serialize_map(Some(entries.len()))?;
    for (key, item) in js_order(entries.iter().map(|(k, v)| (k.as_str(), v))) {
        out.serialize_entry(key, &Part(item))?;
    }
    out.end()
}

/// A key zod's `z.unknown()` takes as it is, `null` included: absent is `None`, anything present is kept.
pub fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Kept>, D::Error> {
    Kept::deserialize(d).map(Some)
}

/// A record zod reads: the key `__proto__` is read and not kept.
pub fn record<'de, D: Deserializer<'de>, V: Deserialize<'de>>(d: D) -> Result<IndexMap<String, V>, D::Error> {
    let mut map = IndexMap::<String, V>::deserialize(d)?;
    map.shift_remove("__proto__");
    Ok(map)
}

/// An optional record zod reads: the key `__proto__` is read and not kept.
pub fn record_opt<'de, D: Deserializer<'de>, V: Deserialize<'de>>(d: D) -> Result<Option<IndexMap<String, V>>, D::Error> {
    let mut map = IndexMap::<String, V>::deserialize(d)?;
    map.shift_remove("__proto__");
    Ok(Some(map))
}
