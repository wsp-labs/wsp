// SPDX-License-Identifier: AGPL-3.0-only
//! What zod did beside the shape: the refinements a parse enforces (`.int()`, `.positive()`, `.max(n)`, `.regex()`,
//! `.strict()`, a refine), and the issue a refused value answers with. Serde reads the shape; `Check` reads the rest.
//! See spec/check.md.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

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
        todo!()
    }

    /// The same issue one level down: `segment` goes in front of the path, so a struct's check names the field it
    /// read.
    pub fn under(self, segment: impl Into<Value>) -> SchemaError {
        todo!()
    }
}

/// The refinements of one type. A type with none keeps the default, which checks nothing; a struct's check runs the
/// check of every field whose type has one, in field order, and answers the first issue with the field's key in
/// front of its path.
pub trait Check {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
}

impl<T: Check> Check for Vec<T> {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

impl<T: Check> Check for Option<T> {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

impl<K, V: Check> Check for IndexMap<K, V> {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

impl Check for String {}
impl Check for bool {}
impl Check for i64 {}
impl Check for Value {}

/// zod's `schema.safeParse(value)`: the shape through serde, then the type's check. A serde failure answers code
/// `invalid_type`, path `[]` and serde's message.
pub fn parse<T: DeserializeOwned + Check>(value: &Value) -> Result<T, SchemaError> {
    todo!()
}

/// A zod `.default(v)` value, written in the generated code as the JSON the TS default holds.
pub fn from_json<T: DeserializeOwned>(json: &str) -> T {
    todo!()
}

/// A JavaScript string's `.length`: its UTF-16 code units, the unit zod's `.max(n)` and `.length(n)` count.
pub fn utf16_len(s: &str) -> usize {
    todo!()
}

/// A field zod reads as `.nullable()` and not optional: `null` is `None`, and the key must be there. serde would read a
/// missing `Option` field as `None`; with this as its `deserialize_with` a missing key is a `missing field` error, as
/// zod's `invalid_type` (received undefined) is.
pub fn nullable<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    todo!()
}

/// A field zod reads as `.optional().nullable()` (or the other way round): absent is `None`, `null` is `Some(None)`,
/// a value is `Some(Some(v))`, and each writes back as it was read.
pub mod double_option {
    use super::*;

    pub fn serialize<T: Serialize, S: Serializer>(value: &Option<Option<T>>, s: S) -> Result<S::Ok, S::Error> {
        todo!()
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        todo!()
    }
}
