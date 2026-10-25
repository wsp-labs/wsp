// SPDX-License-Identifier: AGPL-3.0-only
//! A key whose rules zod runs one after another on the same value, each an issue where it fails (`z.string().min(1).refine(..)`
//! lists both for an empty string). `keys::req_key_with` takes one rule that answers one issue; these take a rule that answers
//! every issue the key breaks, in zod's order, and read the key as `keys` reads it (a fraction keeps its bounds, see `Check::dirty`).

use serde::de::DeserializeOwned;

use crate::types::check::{read, read_present, required, Check, SchemaError};
use crate::types::keys::Obj;

pub type Rules<R> = fn(&R) -> Vec<SchemaError>;

/// The issues of the rules that failed, in the order given.
pub fn all(checks: impl IntoIterator<Item = Result<(), SchemaError>>) -> Vec<SchemaError> {
    checks.into_iter().filter_map(Result::err).collect()
}

/// The first of those issues: what a type's `check` answers.
pub fn first(found: Vec<SchemaError>) -> Result<(), SchemaError> {
    found.into_iter().next().map_or(Ok(()), Err)
}

fn push<R>(out: &mut Vec<SchemaError>, key: &str, got: Result<R, Vec<SchemaError>>, dirty: Option<R>, rules: Rules<R>) {
    let rule_on = |out: &mut Vec<SchemaError>, x: &R| out.extend(rules(x).into_iter().map(|e| e.under(key)));
    match got {
        Ok(x) => rule_on(out, &x),
        Err(issues) => {
            out.extend(issues.into_iter().map(|e| e.under(key)));
            if let Some(x) = dirty {
                rule_on(out, &x);
            }
        }
    }
}

/// A key zod requires: absent is its `Required` issue.
pub fn req_key_all<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rules: Rules<T>) {
    match obj.get(key) {
        None => out.push(required::<T>(key)),
        Some(v) => push(out, key, read::<T>(v), T::dirty(v), rules),
    }
}

/// A key zod reads as `.optional()`: absent is fine, `null` is refused.
pub fn opt_key_all<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rules: Rules<Option<T>>) {
    if let Some(v) = obj.get(key) {
        push(out, key, read_present::<T>(v), T::dirty(v).map(Some), rules);
    }
}
