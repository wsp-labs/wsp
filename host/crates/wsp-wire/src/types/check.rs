// SPDX-License-Identifier: AGPL-3.0-only
//! What zod did beside the shape: the refinements a parse enforces (`.int()`, `.positive()`, `.max(n)`, `.regex()`,
//! `.strict()`, a refine), and the issue a refused value answers with. Serde reads the shape; `Check` reads the rest.
//! See spec/check.md.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::types::js_number::JsNumber;

/// One zod issue, as zod 3's `ZodError` carries it (chapter 05, rule 4.8): the first issue a parse found.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaError {
    /// zod's word: invalid_type, too_big, too_small, invalid_string, invalid_enum_value, invalid_literal,
    /// invalid_union_discriminator, unrecognized_keys, custom.
    pub code: String,
    /// "key" strings and index numbers from the root, `[]` at the root.
    pub path: Vec<Value>,
    /// zod's wording where it can be matched.
    pub message: String,
    /// zod's other keys in zod's order: expected, received, options, keys, minimum, maximum, inclusive, exact, type,
    /// validation.
    pub detail: IndexMap<String, Value>,
}

impl SchemaError {
    /// The issue as zod's `ZodError.message` writes a list of one: `JSON.stringify([issue], null, 2)`, the issue's
    /// keys in zod's order (code, then detail's keys, then path, then message).
    pub fn issue_text(&self) -> String {
        let mut issue: IndexMap<String, Value> = IndexMap::new();
        let code = self.code.as_str();
        let detail = |keys: &[&str]| keys.iter().filter_map(|k| self.detail.get(*k).map(|v| (k.to_string(), v.clone()))).collect::<Vec<_>>();
        let rest = |skip: &[&str]| self.detail.iter().filter(|(k, _)| !skip.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>();
        let float = self.detail.get("received") == Some(&Value::from("float"));
        let order: Vec<(String, Value)> = match code {
            "invalid_enum_value" | "invalid_literal" => {
                let mut o = detail(&["received"]);
                o.push(("code".into(), Value::from(code)));
                o.extend(rest(&["received"]));
                o.push(("path".into(), Value::Array(self.path.clone())));
                o.push(("message".into(), Value::from(self.message.as_str())));
                o
            }
            "invalid_string" => {
                let mut o = detail(&["validation"]);
                o.push(("code".into(), Value::from(code)));
                o.push(("message".into(), Value::from(self.message.as_str())));
                o.push(("path".into(), Value::Array(self.path.clone())));
                o
            }
            "too_small" | "too_big" | "custom" => {
                let mut o = vec![("code".into(), Value::from(code))];
                o.extend(rest(&[]));
                o.push(("message".into(), Value::from(self.message.as_str())));
                o.push(("path".into(), Value::Array(self.path.clone())));
                o
            }
            "invalid_type" if float => {
                let mut o = vec![("code".into(), Value::from(code))];
                o.extend(rest(&[]));
                o.push(("message".into(), Value::from(self.message.as_str())));
                o.push(("path".into(), Value::Array(self.path.clone())));
                o
            }
            _ => {
                let mut o = vec![("code".into(), Value::from(code))];
                o.extend(rest(&[]));
                o.push(("path".into(), Value::Array(self.path.clone())));
                o.push(("message".into(), Value::from(self.message.as_str())));
                o
            }
        };
        issue.extend(order);
        serde_json::to_string_pretty(&Value::Array(vec![Value::Object(issue.into_iter().collect())])).expect("an issue writes as JSON")
    }

    /// The same issue one level down: `segment` goes in front of the path, so a struct's check names the field it
    /// read.
    pub fn under(self, segment: impl Into<Value>) -> SchemaError {
        let mut path = self.path;
        path.insert(0, segment.into());
        SchemaError { path, ..self }
    }
}

/// What zod calls the value a path reaches, for the issue a refused value answers with. `Word` is a plain type word,
/// `Int` an `.int()` number, `Options` a `z.enum`'s words, `Tags` a tagged union's tag key and its tags, `Union` an
/// untagged union, which answers `invalid_union` where it is missing or of no variant, and `Literal` a literal's one value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zod {
    Word(&'static str),
    Int,
    Options(&'static [&'static str]),
    Tags(&'static str, &'static [&'static str]),
    Union,
    Literal(fn() -> Value),
}

/// The refinements of one type. A type with none keeps the default, which checks nothing; a struct's check runs the
/// check of every field whose type has one, in field order, and answers the first issue with the field's key in front
/// of its path.
pub trait Check {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    /// What zod calls the value at `path` below this type, or None where the type does not say. A missing key, a wrong
    /// type or an unknown tag names its expected word through this, since serde's error does not.
    fn describe(_path: &[Value]) -> Option<Zod>
    where
        Self: Sized,
    {
        None
    }
}

/// The word of a type that is one value and has no path below it.
pub fn leaf(path: &[Value], zod: Zod) -> Option<Zod> {
    path.is_empty().then_some(zod)
}

impl<T: Check> Check for Vec<T> {
    fn check(&self) -> Result<(), SchemaError> {
        for (i, item) in self.iter().enumerate() {
            item.check().map_err(|e| e.under(i))?;
        }
        Ok(())
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("array")),
            Some((_, rest)) => T::describe(rest),
        }
    }
}

impl<T: Check> Check for Option<T> {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            Some(v) => v.check(),
            None => Ok(()),
        }
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        T::describe(path)
    }
}

impl<K: Serialize, V: Check> Check for IndexMap<K, V> {
    fn check(&self) -> Result<(), SchemaError> {
        for (k, v) in self {
            let key = serde_json::to_value(k).expect("a map key writes as JSON");
            v.check().map_err(|e| e.under(key))?;
        }
        Ok(())
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((_, rest)) => V::describe(rest),
        }
    }
}

impl Check for String {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Word("string"))
    }
}

impl Check for bool {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Word("boolean"))
    }
}

impl Check for i64 {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Int)
    }
}

impl Check for u64 {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Int)
    }
}

impl Check for u32 {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Int)
    }
}

impl Check for usize {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Int)
    }
}

impl Check for f64 {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Word("number"))
    }
}

impl Check for Value {}

fn bound(code: &str, minimum: Option<Value>, maximum: Option<Value>, ty: &str, inclusive: bool, exact: bool, message: String) -> SchemaError {
    let mut detail = IndexMap::new();
    if let Some(v) = minimum {
        detail.insert("minimum".to_string(), v);
    } else if let Some(v) = maximum {
        detail.insert("maximum".to_string(), v);
    }
    detail.insert("type".to_string(), Value::from(ty));
    detail.insert("inclusive".to_string(), Value::from(inclusive));
    detail.insert("exact".to_string(), Value::from(exact));
    SchemaError { code: code.into(), path: Vec::new(), message, detail }
}

fn num_json(n: f64) -> Value {
    serde_json::to_value(JsNumber(n)).expect("a number writes as JSON")
}

/// zod's `.min(n)` on a number: `>= n` when inclusive, `> n` when not.
pub fn num_at_least(v: f64, min: f64) -> Result<(), SchemaError> {
    if v >= min {
        return Ok(());
    }
    Err(bound("too_small", Some(num_json(min)), None, "number", true, false, format!("Number must be greater than or equal to {}", JsNumber(min).js_string())))
}

pub fn num_above(v: f64, min: f64) -> Result<(), SchemaError> {
    if v > min {
        return Ok(());
    }
    Err(bound("too_small", Some(num_json(min)), None, "number", false, false, format!("Number must be greater than {}", JsNumber(min).js_string())))
}

pub fn num_at_most(v: f64, max: f64) -> Result<(), SchemaError> {
    if v <= max {
        return Ok(());
    }
    Err(bound("too_big", None, Some(num_json(max)), "number", true, false, format!("Number must be less than or equal to {}", JsNumber(max).js_string())))
}

pub fn num_below(v: f64, max: f64) -> Result<(), SchemaError> {
    if v < max {
        return Ok(());
    }
    Err(bound("too_big", None, Some(num_json(max)), "number", false, false, format!("Number must be less than {}", JsNumber(max).js_string())))
}

/// zod's `.min(n)` on a string: its length in UTF-16 units.
pub fn text_at_least(s: &str, min: usize) -> Result<(), SchemaError> {
    if utf16_len(s) >= min {
        return Ok(());
    }
    Err(bound("too_small", Some(Value::from(min)), None, "string", true, false, format!("String must contain at least {min} character(s)")))
}

pub fn text_at_most(s: &str, max: usize) -> Result<(), SchemaError> {
    if utf16_len(s) <= max {
        return Ok(());
    }
    Err(bound("too_big", None, Some(Value::from(max)), "string", true, false, format!("String must contain at most {max} character(s)")))
}

/// zod's `.length(n)` on a string: too short and too long both fail, as zod's exact issues.
pub fn text_exactly(s: &str, n: usize) -> Result<(), SchemaError> {
    let len = utf16_len(s);
    if len < n {
        return Err(bound("too_small", Some(Value::from(n)), None, "string", true, true, format!("String must contain exactly {n} character(s)")));
    }
    if len > n {
        return Err(bound("too_big", None, Some(Value::from(n)), "string", true, true, format!("String must contain exactly {n} character(s)")));
    }
    Ok(())
}

pub fn items_at_least(len: usize, min: usize) -> Result<(), SchemaError> {
    if len >= min {
        return Ok(());
    }
    Err(bound("too_small", Some(Value::from(min)), None, "array", true, false, format!("Array must contain at least {min} element(s)")))
}

pub fn items_at_most(len: usize, max: usize) -> Result<(), SchemaError> {
    if len <= max {
        return Ok(());
    }
    Err(bound("too_big", None, Some(Value::from(max)), "array", true, false, format!("Array must contain at most {max} element(s)")))
}

pub fn items_exactly(len: usize, n: usize) -> Result<(), SchemaError> {
    if len < n {
        return Err(bound("too_small", Some(Value::from(n)), None, "array", true, true, format!("Array must contain exactly {n} element(s)")));
    }
    if len > n {
        return Err(bound("too_big", None, Some(Value::from(n)), "array", true, true, format!("Array must contain exactly {n} element(s)")));
    }
    Ok(())
}

/// A zod `.refine(f, message)`: the message when `ok` is false.
pub fn refine(ok: bool, message: &str) -> Result<(), SchemaError> {
    if ok {
        return Ok(());
    }
    Err(SchemaError { code: "custom".into(), path: Vec::new(), message: message.into(), detail: IndexMap::new() })
}

/// A zod `.regex(r)` that failed: zod's `invalid_string` with validation `regex`.
pub fn regex_check(ok: bool) -> Result<(), SchemaError> {
    if ok {
        return Ok(());
    }
    let mut detail = IndexMap::new();
    detail.insert("validation".to_string(), Value::from("regex"));
    Err(SchemaError { code: "invalid_string".into(), path: Vec::new(), message: "Invalid".into(), detail })
}

/// `/^[^/\s]+\/[^/\s]+$/`: one slash between two non-empty runs of non-space characters.
pub fn tracker_repo(s: &str) -> bool {
    let is_run = |p: &str| !p.is_empty() && !p.chars().any(|c| c == '/' || c.is_whitespace());
    s.split_once('/').is_some_and(|(a, b)| is_run(a) && is_run(b))
}

/// `/^[A-Za-z_][A-Za-z0-9_]*$/`
pub fn env_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `/^[0-9a-f]{64}$/`
pub fn hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `/^[^\n\r\0]*$/`
pub fn no_line_breaks(s: &str) -> bool {
    !s.contains(['\n', '\r', '\0'])
}

/// JavaScript's `String.prototype.trim`: the white space and line terminators it strips, which is not Rust's set.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() && c != '\u{85}' || c == '\u{feff}')
}

/// A zod `.trim()` on a string: the value is trimmed as it is read, so it is written trimmed too.
pub fn trimmed<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    String::deserialize(d).map(|s| js_trim(&s).to_string())
}

/// `.trim()` on a field zod reads as `.optional()`.
pub fn trimmed_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d).map(|s| s.map(|s| js_trim(&s).to_string()))
}

/// `.trim()` on a field zod reads as `.optional().nullable()`, with `double_option`'s three states.
pub mod double_option_trimmed {
    use super::*;

    pub use super::double_option::serialize;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
        trimmed_opt(d).map(Some)
    }
}

/// zod's `schema.safeParse(value)`: the shape through serde, then the type's check. A serde failure answers code
/// `invalid_type`, path `[]` and serde's message.
///
/// Unknown keys are not one serde error: serde stops at the first, so each one is taken out and the read retried, and
/// the keys answer as one `unrecognized_keys` at their object's path once nothing else is refused.
pub fn parse<T: DeserializeOwned + Check>(value: &Value) -> Result<T, SchemaError> {
    let mut work = value.clone();
    let mut unknown: Option<(Vec<Value>, Vec<String>)> = None;
    loop {
        match serde_path_to_error::deserialize::<_, T>(&work) {
            Ok(read) => {
                read.check()?;
                return match unknown {
                    Some((path, keys)) => Err(unrecognized(path, keys)),
                    None => Ok(read),
                };
            }
            Err(e) => {
                let mut path = segments(e.path());
                let msg = e.inner().to_string();
                let Some(rest) = msg.strip_prefix("unknown field `") else {
                    return Err(serde_issue::<T>(&msg, path, &work));
                };
                let key = rest.split('`').next().unwrap_or_default().to_string();
                if path.last() == Some(&Value::from(key.as_str())) {
                    path.pop();
                }
                match &mut unknown {
                    None => unknown = Some((path.clone(), vec![key.clone()])),
                    Some((at, keys)) if *at == path => keys.push(key.clone()),
                    Some((at, keys)) => return Err(unrecognized(at.clone(), std::mem::take(keys))),
                }
                if !remove(&mut work, &path, &key) {
                    return Err(serde_issue::<T>(&msg, path, &work));
                }
            }
        }
    }
}

fn unrecognized(path: Vec<Value>, keys: Vec<String>) -> SchemaError {
    let message = format!("Unrecognized key(s) in object: {}", keys.iter().map(|k| format!("'{k}'")).collect::<Vec<_>>().join(", "));
    let mut detail = IndexMap::new();
    detail.insert("keys".to_string(), Value::Array(keys.into_iter().map(Value::from).collect()));
    SchemaError { code: "unrecognized_keys".into(), path, message, detail }
}

fn segments(path: &serde_path_to_error::Path) -> Vec<Value> {
    path.iter()
        .filter_map(|s| match s {
            serde_path_to_error::Segment::Seq { index } => Some(Value::from(*index)),
            serde_path_to_error::Segment::Map { key } => Some(Value::from(key.as_str())),
            serde_path_to_error::Segment::Enum { variant } => Some(Value::from(variant.as_str())),
            serde_path_to_error::Segment::Unknown => None,
        })
        .collect()
}

fn remove(work: &mut Value, path: &[Value], key: &str) -> bool {
    let mut at = work;
    for seg in path {
        let next = match seg {
            Value::String(k) => at.get_mut(k.as_str()),
            Value::Number(i) => at.get_mut(i.as_u64().unwrap_or(0) as usize),
            _ => None,
        };
        match next {
            Some(v) => at = v,
            None => return false,
        }
    }
    match at {
        Value::Object(map) => map.shift_remove(key).is_some(),
        _ => false,
    }
}

fn value_at<'a>(value: &'a Value, path: &[Value]) -> Option<&'a Value> {
    path.iter().try_fold(value, |at, seg| match seg {
        Value::String(k) => at.get(k.as_str()),
        Value::Number(i) => at.get(i.as_u64()? as usize),
        _ => None,
    })
}

/// zod's type word of a JSON value.
fn received_word(value: Option<&Value>) -> &'static str {
    match value {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(n)) if n.is_f64() && n.as_f64().is_some_and(|f| f.fract() != 0.0) => "float",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn options_text(options: &[&str]) -> String {
    options.iter().map(|o| format!("'{o}'")).collect::<Vec<_>>().join(" | ")
}

fn options_value(options: &[&str]) -> Value {
    Value::Array(options.iter().map(|o| Value::from(*o)).collect())
}

/// The issue for one serde error, by the words serde gives: a missing field, an unknown variant, a value of the wrong
/// type, a literal, an untagged union with no variant. Anything else answers as a `custom` issue with serde's words.
fn serde_issue<T: Check>(msg: &str, path: Vec<Value>, work: &Value) -> SchemaError {
    let quoted = |s: &str| s.split('`').nth(1).unwrap_or_default().to_string();
    if msg.starts_with("missing field `") {
        let mut at = path;
        at.push(Value::from(quoted(msg)));
        return match T::describe(&at) {
            Some(Zod::Tags(key, tags)) => discriminator(at, key, tags),
            Some(Zod::Union) => invalid_union(at),
            Some(Zod::Literal(word)) => invalid_literal(at, Value::from("undefined"), word()),
            zod => {
                let expected = match zod {
                    Some(Zod::Options(o)) => options_text(o),
                    Some(Zod::Word(w)) => w.to_string(),
                    Some(Zod::Int) => "number".to_string(),
                    _ => "unknown".to_string(),
                };
                let mut detail = IndexMap::new();
                detail.insert("expected".to_string(), Value::from(expected));
                detail.insert("received".to_string(), Value::from("undefined"));
                SchemaError { code: "invalid_type".into(), path: at, message: "Required".into(), detail }
            }
        };
    }
    if msg.starts_with("unknown variant `") {
        let got = quoted(msg);
        let options = match T::describe(&path) {
            Some(Zod::Tags(key, tags)) => return discriminator(path, key, tags),
            Some(Zod::Union) => return invalid_union(path),
            Some(Zod::Options(o)) => o.iter().map(|w| w.to_string()).collect(),
            _ => match variant_names(msg) {
                Some(names) => names,
                None => return custom(path, msg),
            },
        };
        let words: Vec<&str> = options.iter().map(String::as_str).collect();
        let mut detail = IndexMap::new();
        detail.insert("received".to_string(), Value::from(got.as_str()));
        detail.insert("options".to_string(), Value::Array(options.iter().map(|o| Value::from(o.as_str())).collect()));
        return SchemaError {
            code: "invalid_enum_value".into(),
            path,
            message: format!("Invalid enum value. Expected {}, received '{got}'", options_text(&words)),
            detail,
        };
    }
    if msg.starts_with("invalid type: ") || msg.starts_with("invalid value: ") {
        let at = value_at(work, &path);
        return match T::describe(&path) {
            Some(Zod::Tags(key, tags)) => discriminator(path, key, tags),
            Some(Zod::Union) => invalid_union(path),
            Some(Zod::Literal(word)) => invalid_literal(path, at.cloned().unwrap_or(Value::Null), word()),
            zod => {
                let (expected, received) = match zod {
                    Some(Zod::Int) if received_word(at) == "float" => ("integer".to_string(), "float"),
                    Some(Zod::Int) => ("number".to_string(), received_word(at)),
                    Some(Zod::Word(w)) => (w.to_string(), received_word(at)),
                    Some(Zod::Options(o)) => (options_text(o), received_word(at)),
                    _ => ("unknown".to_string(), received_word(at)),
                };
                let mut detail = IndexMap::new();
                detail.insert("expected".to_string(), Value::from(expected.as_str()));
                detail.insert("received".to_string(), Value::from(received));
                SchemaError { code: "invalid_type".into(), path, message: format!("Expected {expected}, received {received}"), detail }
            }
        };
    }
    if let Some(literal) = msg.strip_prefix("Invalid literal value, expected ") {
        let expected: Value = serde_json::from_str(literal).unwrap_or(Value::Null);
        let received = value_at(work, &path).cloned().unwrap_or(Value::Null);
        return invalid_literal(path, received, expected);
    }
    if msg.starts_with("data did not match any variant") {
        return invalid_union(path);
    }
    custom(path, msg)
}

/// The words serde lists after `expected` in an unknown-variant message: `` `a` ``, `` `a` or `b` ``, or
/// `` one of `a`, `b` ``.
fn variant_names(msg: &str) -> Option<Vec<String>> {
    let tail = msg.split_once("expected ")?.1;
    let names: Vec<String> = tail.split('`').skip(1).step_by(2).map(str::to_string).collect();
    (!names.is_empty()).then_some(names)
}

fn invalid_literal(path: Vec<Value>, received: Value, expected: Value) -> SchemaError {
    let mut detail = IndexMap::new();
    detail.insert("received".to_string(), received);
    detail.insert("expected".to_string(), expected.clone());
    SchemaError { code: "invalid_literal".into(), path, message: format!("Invalid literal value, expected {expected}"), detail }
}

fn discriminator(path: Vec<Value>, _key: &str, tags: &[&str]) -> SchemaError {
    let mut detail = IndexMap::new();
    detail.insert("options".to_string(), options_value(tags));
    SchemaError {
        code: "invalid_union_discriminator".into(),
        path,
        message: format!("Invalid discriminator value. Expected {}", options_text(tags)),
        detail,
    }
}

fn invalid_union(path: Vec<Value>) -> SchemaError {
    SchemaError { code: "invalid_union".into(), path, message: "Invalid input".into(), detail: IndexMap::new() }
}

fn custom(path: Vec<Value>, msg: &str) -> SchemaError {
    SchemaError { code: "custom".into(), path, message: msg.to_string(), detail: IndexMap::new() }
}

/// A zod `.default(v)` value, written in the generated code as the JSON the TS default holds.
pub fn from_json<T: DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).expect("a generated default is valid JSON for its type")
}

/// A JavaScript string's `.length`: its UTF-16 code units, the unit zod's `.max(n)` and `.length(n)` count.
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// A field zod reads as `.nullable()` and not optional: `null` is `None`, and the key must be there. serde would read a
/// missing `Option` field as `None`; with this as its `deserialize_with` a missing key is a `missing field` error, as
/// zod's `invalid_type` (received undefined) is.
pub fn nullable<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

/// A field zod reads as `.optional().nullable()` (or the other way round): absent is `None`, `null` is `Some(None)`,
/// a value is `Some(Some(v))`, and each writes back as it was read.
pub mod double_option {
    use super::*;

    pub fn serialize<T: Serialize, S: Serializer>(value: &Option<Option<T>>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(inner) => inner.serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

/// wsp-frames' copy of a shape the protocol names as one of ours: a value of it is checked by reading its JSON through
/// ours, so every refusal ours would make is the same here, at the same path.
macro_rules! through_ours {
    ($foreign:path, $ours:path) => {
        impl Check for $foreign {
            fn check(&self) -> Result<(), SchemaError> {
                let json = serde_json::to_value(self).expect("a wsp-frames shape writes as JSON");
                parse::<$ours>(&json).map(|_| ())
            }

            fn describe(path: &[Value]) -> Option<Zod> {
                <$ours as Check>::describe(path)
            }
        }
    };
}

through_ours!(wsp_frames::PullRequest, crate::types::pull_request::PullRequest);
through_ours!(wsp_frames::PullRequestComment, crate::types::pull_request::PullRequestComment);
through_ours!(wsp_frames::PullRequestReviewComment, crate::types::pull_request::PullRequestReviewComment);
