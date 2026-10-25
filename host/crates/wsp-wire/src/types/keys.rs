// SPDX-License-Identifier: AGPL-3.0-only
//! One key of a struct's schema pass: the key is read as the type it holds, its issues come back with the key in front,
//! and the key's rule runs on the value read, before the next key is read. See spec/check.md rule 2.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::types::check::{read, read_double, read_double_trimmed, read_present, read_trimmed, read_trimmed_opt, required, Check, SchemaError};

pub type Obj = Map<String, Value>;
pub type Rule<R> = fn(&R) -> Result<(), SchemaError>;

fn push<R>(out: &mut Vec<SchemaError>, key: &str, got: Result<R, Vec<SchemaError>>, rule: Option<Rule<R>>) {
    match got {
        Ok(x) => {
            if let Some(Err(e)) = rule.map(|rule| rule(&x)) {
                out.push(e.under(key));
            }
        }
        Err(issues) => out.extend(issues.into_iter().map(|e| e.under(key))),
    }
}

/// A key zod requires: absent is its `Required` issue, or the words of the type the key holds.
pub fn req_key<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>) {
    req(obj, key, out, None::<Rule<T>>);
}

pub fn req_key_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<T>) {
    req(obj, key, out, Some(rule));
}

fn req<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Option<Rule<T>>) {
    match obj.get(key) {
        None => out.push(required::<T>(key)),
        Some(v) => push(out, key, read::<T>(v), rule),
    }
}

/// A key zod reads as `.optional()`: absent is fine, `null` is refused.
pub fn opt_key<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_present::<T>(v), None::<Rule<Option<T>>>);
    }
}

pub fn opt_key_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<T>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_present::<T>(v), Some(rule));
    }
}

/// A key zod reads as `.optional().nullable()`: absent and `null` are both fine.
pub fn dbl_key<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_double::<T>(v), None::<Rule<Option<Option<T>>>>);
    }
}

pub fn dbl_key_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<Option<T>>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_double::<T>(v), Some(rule));
    }
}

/// A key the struct may leave out whose value is read as the type it holds, `null` included where the type takes it.
pub fn any_key<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read::<T>(v), None::<Rule<T>>);
    }
}

pub fn any_key_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<T>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read::<T>(v), Some(rule));
    }
}

/// A required string zod trims as it reads it.
pub fn req_trimmed_with(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<String>) {
    match obj.get(key) {
        None => out.push(required::<String>(key)),
        Some(v) => push(out, key, read_trimmed(v), Some(rule)),
    }
}

pub fn opt_trimmed_with(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<String>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_trimmed_opt(v), Some(rule));
    }
}

pub fn dbl_trimmed_with(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<Option<String>>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_double_trimmed(v), Some(rule));
    }
}
