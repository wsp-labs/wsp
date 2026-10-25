// SPDX-License-Identifier: AGPL-3.0-only
//! One key of a struct's schema pass: the key is read as the type it holds, its issues come back with the key in front,
//! and the key's rule runs on the value read, before the next key is read. See spec/check.md rule 2.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::types::check::{
    dirty_of, invalid_literal, issues_of, read, read_double, read_double_trimmed, read_present, read_trimmed, read_trimmed_opt, required,
    Check, SchemaError,
};

pub type Obj = Map<String, Value>;
pub type Rule<R> = fn(&R) -> Result<(), SchemaError>;

fn push<R>(
    out: &mut Vec<SchemaError>,
    key: &str,
    got: Result<R, Vec<SchemaError>>,
    dirty: impl FnOnce() -> Option<R>,
    rule: Option<Rule<R>>,
) {
    let rule_on = |out: &mut Vec<SchemaError>, x: &R| {
        if let Some(Err(e)) = rule.map(|rule| rule(x)) {
            out.push(e.under(key));
        }
    };
    match got {
        Ok(x) => rule_on(out, &x),
        Err(issues) => {
            out.extend(issues.into_iter().map(|e| e.under(key)));
            if let Some(x) = dirty() {
                rule_on(out, &x);
            }
        }
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
        Some(v) => push(out, key, read::<T>(v), || dirty_of::<T>(v), rule),
    }
}

/// A key zod reads as `.optional()`: absent is fine, `null` is refused.
pub fn opt_key<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_present::<T>(v), || dirty_of::<T>(v).map(Some), None::<Rule<Option<T>>>);
    }
}

pub fn opt_key_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<T>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_present::<T>(v), || dirty_of::<T>(v).map(Some), Some(rule));
    }
}

/// A key zod reads as `.optional().nullable()`: absent and `null` are both fine.
pub fn dbl_key<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_double::<T>(v), || dirty_of::<T>(v).map(|x| Some(Some(x))), None::<Rule<Option<Option<T>>>>);
    }
}

pub fn dbl_key_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<Option<T>>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_double::<T>(v), || dirty_of::<T>(v).map(|x| Some(Some(x))), Some(rule));
    }
}

/// A key the struct may leave out whose value is read as the type it holds, `null` included where the type takes it.
pub fn any_key<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read::<T>(v), || dirty_of::<T>(v), None::<Rule<T>>);
    }
}

pub fn any_key_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<T>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read::<T>(v), || dirty_of::<T>(v), Some(rule));
    }
}

/// A required string zod trims as it reads it.
pub fn req_trimmed_with(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<String>) {
    match obj.get(key) {
        None => out.push(required::<String>(key)),
        Some(v) => push(out, key, read_trimmed(v), || None, Some(rule)),
    }
}

pub fn opt_trimmed_with(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<String>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_trimmed_opt(v), || None, Some(rule));
    }
}

pub fn dbl_trimmed_with(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rule: Rule<Option<Option<String>>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_double_trimmed(v), || None, Some(rule));
    }
}

/// A key that is `z.literal("word")`: absent, another type and another word are each `invalid_literal`.
pub fn lit_key(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, expected: &str) {
    match obj.get(key) {
        Some(Value::String(word)) if word == expected => {}
        got => out.push(invalid_literal(vec![Value::from(key)], got.cloned(), Value::from(expected))),
    }
}

/// The length rule of an array key: zod checks it before any item, so it answers first whatever an item holds.
pub type Size = fn(usize) -> Result<(), SchemaError>;

/// An array key with a length rule, which answers before its items' issues and, once failed, leaves the key's other rules
/// unrun as zod's aborted item would.
fn sized<T: DeserializeOwned + Check>(out: &mut Vec<SchemaError>, key: &str, value: &Value, size: Size) -> bool {
    let Value::Array(items) = value else { return false };
    let Err(refused) = size(items.len()) else { return false };
    out.push(refused.under(key));
    out.extend(issues_of::<Vec<T>>(value).into_iter().map(|e| e.under(key)));
    true
}

/// `req_key_with` for a `z.array(T)` key whose length rule is `size`.
pub fn req_arr_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, size: Size, rule: Rule<Vec<T>>) {
    match obj.get(key) {
        Some(v) if sized::<T>(out, key, v, size) => {}
        _ => req(obj, key, out, Some(rule)),
    }
}

/// `opt_key_with` for a `z.array(T).optional()` key whose length rule is `size`.
pub fn opt_arr_with<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, size: Size, rule: Rule<Option<Vec<T>>>) {
    match obj.get(key) {
        Some(v) if sized::<T>(out, key, v, size) => {}
        _ => opt_key_with::<Vec<T>>(obj, key, out, rule),
    }
}

/// `opt_key_with` for a key whose rules each list their issue, as zod lists the item rules and then a refine of a record.
pub fn opt_key_rules<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rules: fn(&T) -> Vec<SchemaError>) {
    let Some(v) = obj.get(key) else { return };
    match read_present::<T>(v) {
        Ok(Some(x)) => out.extend(rules(&x).into_iter().map(|e| e.under(key))),
        Ok(None) => {}
        Err(issues) => {
            out.extend(issues.into_iter().map(|e| e.under(key)));
            if let Some(x) = dirty_of::<T>(v) {
                out.extend(rules(&x).into_iter().map(|e| e.under(key)));
            }
        }
    }
}
