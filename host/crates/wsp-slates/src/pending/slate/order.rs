// SPDX-License-Identifier: AGPL-3.0-only
//! The key order a document and a syntax problem are written in. The TS writes a document compiled from text in one order
//! (syntax.ts:281-309, 434, 671-679) and one that came through its JSON schema in the struct's; a syntax problem puts `line`
//! first (syntax.ts:297-298) and the validator's puts it after `prop`.
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Map, Value};

use super::{SlateDoc, SlatePiece};

/// Whether a value was written in the text road's key order; two values equal whichever way they were written.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextOrder(pub bool);

impl PartialEq for TextOrder {
    fn eq(&self, _: &TextOrder) -> bool {
        true
    }
}

const TOP_TEXT: &[&str] = &["schema", "root", "values", "derived", "runs", "reactions", "pieces", "title", "files", "kit"];
const PIECE_TEXT: &[&str] = &["type", "when", "props", "children", "on", "fallback"];
const CMD_TEXT: &[&str] =
    &["kind", "cmd", "every", "once", "always", "confirm", "then", "env", "args", "stdin", "on", "cwd", "timeout", "stream"];
const TOOL_TEXT: &[&str] = &["kind", "server", "tool", "every", "once", "always", "confirm", "then", "description", "args"];

fn rank(key: &str, order: &[&str]) -> usize {
    order.iter().position(|k| *k == key).unwrap_or(usize::MAX)
}

fn sort_keys(m: &mut Map<String, Value>, order: &[&str]) {
    let mut entries: Vec<(String, Value)> = std::mem::take(m).into_iter().collect();
    entries.sort_by_key(|(k, _)| rank(k, order));
    m.extend(entries);
}

/// The keys of one stored object, in the order they were read.
fn keys_of(v: &Value) -> Vec<String> {
    v.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default()
}

/// The order of every key a stored document was read with, so a row written back keeps its bytes.
#[derive(Debug, Default)]
pub struct Observed {
    top: Vec<String>,
    pieces: HashMap<String, Vec<String>>,
    runs: HashMap<String, Vec<String>>,
}

/// The key order a document is written in: the schema's (the struct's), the text road's, or the one a stored row had.
#[derive(Clone, Debug, Default)]
pub enum KeyOrder {
    #[default]
    Schema,
    Text,
    Observed(Arc<Observed>),
}

impl PartialEq for KeyOrder {
    fn eq(&self, _: &KeyOrder) -> bool {
        true
    }
}

impl KeyOrder {
    /// The order the keys of `doc`, as a JSON document, are in.
    pub fn observed(doc: &Value) -> KeyOrder {
        let each = |section: &str| -> HashMap<String, Vec<String>> {
            doc.get(section).and_then(Value::as_object).into_iter().flatten().map(|(id, item)| (id.clone(), keys_of(item))).collect()
        };
        KeyOrder::Observed(Arc::new(Observed { top: keys_of(doc), pieces: each("pieces"), runs: each("runs") }))
    }
}

fn into_text_order(v: &mut Value) {
    let Some(top) = v.as_object_mut() else { return };
    if let Some(Value::Object(pieces)) = top.get_mut("pieces") {
        for m in pieces.values_mut().filter_map(Value::as_object_mut) {
            sort_keys(m, PIECE_TEXT);
        }
    }
    if let Some(Value::Object(runs)) = top.get_mut("runs") {
        for m in runs.values_mut().filter_map(Value::as_object_mut) {
            match m.get("kind").and_then(Value::as_str) {
                Some("cmd") => sort_keys(m, CMD_TEXT),
                Some("tool") => sort_keys(m, TOOL_TEXT),
                _ => {}
            }
        }
    }
    sort_keys(top, TOP_TEXT);
}

fn sort_by_list(m: &mut Map<String, Value>, list: &[String]) {
    let mut entries: Vec<(String, Value)> = std::mem::take(m).into_iter().collect();
    entries.sort_by_key(|(k, _)| list.iter().position(|x| x == k).unwrap_or(usize::MAX));
    m.extend(entries);
}

fn into_observed_order(v: &mut Value, seen: &Observed) {
    let Some(top) = v.as_object_mut() else { return };
    for (section, lists) in [("pieces", &seen.pieces), ("runs", &seen.runs)] {
        let Some(Value::Object(items)) = top.get_mut(section) else { continue };
        for (id, item) in items.iter_mut() {
            if let (Some(list), Some(m)) = (lists.get(id), item.as_object_mut()) {
                sort_by_list(m, list);
            }
        }
    }
    sort_by_list(top, &seen.top);
}

/// For a record's `document` field: the document with the key order its stored keys were written in.
pub fn doc_option<'de, D: Deserializer<'de>>(d: D) -> Result<Option<SlateDoc>, D::Error> {
    let Some(v) = Option::<Value>::deserialize(d)? else { return Ok(None) };
    let order = KeyOrder::observed(&v);
    let mut doc: SlateDoc = serde_json::from_value(v).map_err(D::Error::custom)?;
    doc.order = order;
    Ok(Some(doc))
}

#[derive(Serialize)]
struct DocPlain<'a> {
    schema: &'a i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    kit: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: &'a Option<String>,
    root: &'a String,
    values: &'a indexmap::IndexMap<String, super::SlateValueDecl>,
    derived: &'a indexmap::IndexMap<String, String>,
    runs: &'a indexmap::IndexMap<String, super::SlateRunDecl>,
    reactions: &'a Vec<Value>,
    pieces: &'a indexmap::IndexMap<String, SlatePiece>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: &'a Option<indexmap::IndexMap<String, String>>,
}

impl Serialize for SlateDoc {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let plain = DocPlain {
            schema: &self.schema,
            kit: &self.kit,
            title: &self.title,
            root: &self.root,
            values: &self.values,
            derived: &self.derived,
            runs: &self.runs,
            reactions: &self.reactions,
            pieces: &self.pieces,
            files: &self.files,
        };
        let mut v = serde_json::to_value(&plain).map_err(S::Error::custom)?;
        match &self.order {
            KeyOrder::Schema => {}
            KeyOrder::Text => into_text_order(&mut v),
            KeyOrder::Observed(seen) => into_observed_order(&mut v, seen),
        }
        v.serialize(s)
    }
}
