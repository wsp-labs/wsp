// SPDX-License-Identifier: AGPL-3.0-only
//! The keys of a daemon op or event that carry a rule: each is read as the type it holds and then ruled in zod's order,
//! with every issue zod lists for it (a list answers its length first, then each item). See spec/daemon.md rule 2.

use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_json::Value;

use crate::types::check::{
    dirty_of, hex_64, issues_of, items_at_most, null_issue, num_above, num_at_least, num_at_most, read, refine, regex_check, required,
    text_at_least, text_at_most, Check, SchemaError,
};
use crate::types::js_number::JsInt;
use crate::types::keys::Obj;
use crate::wire::helpers::{cgroup_shape, is_watch_name, js_key_order};
use crate::wire::limits::{is_http_url, EXEC_BODY_MAX, GUEST_CWD_MAX, GUEST_TOKEN_MAX};

pub type Rules<R> = fn(&R) -> Vec<SchemaError>;

fn issues(checks: impl IntoIterator<Item = Result<(), SchemaError>>) -> Vec<SchemaError> {
    checks.into_iter().filter_map(Result::err).collect()
}

fn push<T: DeserializeOwned + Check>(out: &mut Vec<SchemaError>, key: &str, value: &Value, rules: Rules<T>) {
    match read::<T>(value) {
        Ok(x) => out.extend(rules(&x).into_iter().map(|e| e.under(key))),
        Err(found) => {
            out.extend(found.into_iter().map(|e| e.under(key)));
            if let Some(x) = dirty_of::<T>(value) {
                out.extend(rules(&x).into_iter().map(|e| e.under(key)));
            }
        }
    }
}

/// A key zod requires, ruled once it reads clean.
pub fn req_rules<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rules: Rules<T>) {
    match obj.get(key) {
        None => out.push(required::<T>(key)),
        Some(v) => push(out, key, v, rules),
    }
}

/// A key zod reads as `.optional()`: absent is fine, `null` is refused.
pub fn opt_rules<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, rules: Rules<T>) {
    match obj.get(key) {
        None => {}
        Some(Value::Null) => out.push(null_issue::<T>().under(key)),
        Some(v) => push(out, key, v, rules),
    }
}

fn list<T: DeserializeOwned + Check>(items: &[Value], max: Option<usize>, each: Rules<T>) -> Vec<SchemaError> {
    let mut found = Vec::new();
    if let Some(max) = max {
        found.extend(items_at_most(items.len(), max).err());
    }
    for (i, item) in items.iter().enumerate() {
        match read::<T>(item) {
            Ok(x) => found.extend(each(&x).into_iter().map(|e| e.under(i))),
            Err(more) => {
                found.extend(more.into_iter().map(|e| e.under(i)));
                if let Some(x) = dirty_of::<T>(item) {
                    found.extend(each(&x).into_iter().map(|e| e.under(i)));
                }
            }
        }
    }
    found
}

/// A required `z.array(item).max(n)`: the length rule answers before the items do, as zod's array does.
pub fn req_list<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, max: Option<usize>, each: Rules<T>) {
    match obj.get(key) {
        None => out.push(required::<Vec<T>>(key)),
        Some(Value::Array(items)) => out.extend(list(items, max, each).into_iter().map(|e| e.under(key))),
        Some(other) => out.extend(issues_of::<Vec<T>>(other).into_iter().map(|e| e.under(key))),
    }
}

pub fn opt_list<T: DeserializeOwned + Check>(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, max: Option<usize>, each: Rules<T>) {
    match obj.get(key) {
        None => {}
        Some(Value::Null) => out.push(null_issue::<Vec<T>>().under(key)),
        Some(Value::Array(items)) => out.extend(list(items, max, each).into_iter().map(|e| e.under(key))),
        Some(other) => out.extend(issues_of::<Vec<T>>(other).into_iter().map(|e| e.under(key))),
    }
}

/// A `z.unknown()` key: any value is read as sent, `null` included, and absent stays absent.
pub fn unknown_value<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

pub fn none<T>(_: &T) -> Vec<SchemaError> {
    Vec::new()
}

pub fn nonneg(x: &JsInt) -> Vec<SchemaError> {
    issues([num_at_least(x.as_f64(), 0.0)])
}

pub fn positive(x: &JsInt) -> Vec<SchemaError> {
    issues([num_above(x.as_f64(), 0.0)])
}

pub fn timeout(x: &JsInt) -> Vec<SchemaError> {
    issues([num_above(x.as_f64(), 0.0), num_at_most(x.as_f64(), super::daemon::EXEC_TIMEOUT_MAX_MS as f64)])
}

pub fn port(x: &JsInt) -> Vec<SchemaError> {
    issues([num_at_least(x.as_f64(), 1.0), num_at_most(x.as_f64(), 65535.0)])
}

pub fn token(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, GUEST_TOKEN_MAX as usize)])
}

pub fn guest_cwd(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, GUEST_CWD_MAX as usize)])
}

pub fn name_255(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, 255)])
}

pub fn path_4096(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, 4096)])
}

pub fn identity(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, 32)])
}

pub fn reply_body(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, 65536)])
}

pub fn http_body(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, super::daemon::EXEC_OUTPUT_MAX as usize)])
}

pub fn root_path(x: &String) -> Vec<SchemaError> {
    issues([text_at_least(x, 1), text_at_most(x, 4096)])
}

pub fn session(x: &String) -> Vec<SchemaError> {
    issues([text_at_least(x, 1), text_at_most(x, 64)])
}

pub fn authorized_key(x: &String) -> Vec<SchemaError> {
    issues([text_at_least(x, 1), text_at_most(x, 1024)])
}

pub fn stdio_cmd(x: &String) -> Vec<SchemaError> {
    issues([text_at_least(x, 1), text_at_most(x, EXEC_BODY_MAX as usize)])
}

pub fn container(x: &String) -> Vec<SchemaError> {
    issues([regex_check(hex_64(x))])
}

pub fn sha256(x: &String) -> Vec<SchemaError> {
    issues([regex_check(hex_64(x))])
}

pub fn upload_id(x: &String) -> Vec<SchemaError> {
    let plain = !x.is_empty() && x.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    issues([text_at_least(x, 1), text_at_most(x, 32), regex_check(plain)])
}

pub fn watch_name(x: &String) -> Vec<SchemaError> {
    issues([regex_check(is_watch_name(x))])
}

pub fn cgroup(x: &String) -> Vec<SchemaError> {
    issues([text_at_most(x, crate::wire::helpers::CGROUP_PATH_MAX), refine(cgroup_shape(x), "Invalid input")])
}

pub fn http_url(x: &String) -> Vec<SchemaError> {
    issues([refine(is_http_url(x), "http or https URL")])
}

/// `z.record(z.string(), z.string().max(8192)).refine(h => Object.keys(h).length <= 64, "at most 64 headers")`: each
/// value in the order JavaScript walks the keys, then the refine.
pub fn headers(x: &indexmap::IndexMap<String, String>) -> Vec<SchemaError> {
    let mut walk: indexmap::IndexMap<String, Value> = x.iter().map(|(k, v)| (k.clone(), Value::from(v.as_str()))).collect();
    js_key_order(&mut walk);
    let mut found = Vec::new();
    for (key, value) in &walk {
        found.extend(text_at_most(value.as_str().unwrap_or_default(), 8192).err().map(|e| e.under(key.as_str())));
    }
    found.extend(refine(x.len() <= 64, "at most 64 headers").err());
    found
}
