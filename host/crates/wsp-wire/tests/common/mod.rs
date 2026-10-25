// SPDX-License-Identifier: AGPL-3.0-only
//! What every wire test reads a frame through: the crate's own parse, then a check that it writes back as it came.
#![allow(dead_code)]

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use wsp_wire::wire::{parse, Check, SchemaError};

/// A JSON value with every number as the double JavaScript holds it: JavaScript has no integer and float apart, so
/// `0.0` and `0` are one number, and a value is equal to its own text read back.
pub fn as_js(v: &Value) -> Value {
    match v {
        Value::Number(n) => serde_json::to_value(wsp_wire::types::JsNumber(n.as_f64().expect("a JSON number reads as a double"))).unwrap(),
        Value::Array(items) => Value::Array(items.iter().map(as_js).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), as_js(v))).collect()),
        other => other.clone(),
    }
}

/// `assert_eq!` on two JSON values, numbers compared as JavaScript numbers.
#[track_caller]
pub fn assert_json_eq(got: &Value, want: &Value, why: &str) {
    assert_eq!(as_js(got), as_js(want), "{why}");
}

/// Parses `v` as T and asserts it writes back as the same JSON value (zod's toEqual: keys and values, not their
/// order). Use `reads_exact` where the host writes the shape and its key order is the wire's (rule 4.26), and
/// `reads_as` where zod strips or defaults.
pub fn reads<T: DeserializeOwned + Serialize + Check>(v: Value) -> T {
    let read: T = parse(&v).unwrap_or_else(|e| panic!("refused {v}: {e:?}"));
    assert_json_eq(&serde_json::to_value(&read).unwrap(), &v, "wrote back differently");
    read
}

/// As `reads`, and the text written back is byte for byte the input's, key order included.
pub fn reads_exact<T: DeserializeOwned + Serialize + Check>(v: Value) -> T {
    let read: T = parse(&v).unwrap_or_else(|e| panic!("refused {v}: {e:?}"));
    assert_eq!(serde_json::to_string(&read).unwrap(), serde_json::to_string(&as_js(&v)).unwrap(), "wrote back in another order");
    read
}

/// Parses `v` as T and asserts it writes back as `want`: what zod's strip, default and catch leave.
pub fn reads_as<T: DeserializeOwned + Serialize + Check>(v: Value, want: Value) -> T {
    let read: T = parse(&v).unwrap_or_else(|e| panic!("refused {v}: {e:?}"));
    assert_json_eq(&serde_json::to_value(&read).unwrap(), &want, "wrote back differently");
    read
}

/// Asserts T refuses `v`, and hands back the issue for the test to read its code and path.
pub fn refused<T: DeserializeOwned + Check + std::fmt::Debug>(v: Value) -> SchemaError {
    match parse::<T>(&v) {
        Ok(read) => panic!("took {v} as {read:?}"),
        Err(e) => e,
    }
}

/// A path as zod writes it: `path!["patch", "theme"]`, `path!["procs", 3, "pid"]`.
#[macro_export]
macro_rules! path {
    ($($seg:expr),* $(,)?) => { vec![$(serde_json::json!($seg)),*] as Vec<serde_json::Value> };
}

/// The union readers that answer zod's discriminator issue, each with the same write-back check as `reads`.
pub fn reads_with<T: Serialize>(read: fn(&Value) -> Result<T, SchemaError>, v: Value) -> T {
    let got = read(&v).unwrap_or_else(|e| panic!("refused {v}: {e:?}"));
    assert_json_eq(&serde_json::to_value(&got).unwrap(), &v, "wrote back differently");
    got
}

pub fn reads_with_as<T: Serialize>(read: fn(&Value) -> Result<T, SchemaError>, v: Value, want: Value) -> T {
    let got = read(&v).unwrap_or_else(|e| panic!("refused {v}: {e:?}"));
    assert_json_eq(&serde_json::to_value(&got).unwrap(), &want, "wrote back differently");
    got
}

pub fn refused_with<T: std::fmt::Debug>(read: fn(&Value) -> Result<T, SchemaError>, v: Value) -> SchemaError {
    match read(&v) {
        Ok(got) => panic!("took {v} as {got:?}"),
        Err(e) => e,
    }
}
