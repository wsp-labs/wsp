// SPDX-License-Identifier: AGPL-3.0-only
//! A `z.literal("...")` key a struct keeps: a unit type that writes its one value and refuses any other.
//! `z.literal(true)` and `z.literal(false)` are wsp-frames' `True` and `False`. See spec/check.md.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::types::js_number::JsNumber;

/// Declares a unit struct for one string or number literal, with serde that writes it and reads only it.
#[macro_export]
macro_rules! literal {
    ($name:ident, $value:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
        pub struct $name;

        impl $name {
            pub const VALUE: &'static str = stringify!($value);
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                $crate::types::literal::write_literal(s, $value)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                $crate::types::literal::read_literal(d, $value).map(|()| $name)
            }
        }

        impl $crate::types::check::Check for $name {
            fn describe(path: &[serde_json::Value]) -> Option<$crate::types::check::Zod> {
                $crate::types::check::leaf(path, $crate::types::check::Zod::Literal(|| serde_json::Value::from($value)))
            }
        }
    };
}

/// A literal's value as the macro hands it over: a string or a number.
pub trait LiteralValue: Copy + std::fmt::Debug {
    fn write<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error>;
    fn is(self, v: &Value) -> bool;
    fn to_json(self) -> Value;
}

impl LiteralValue for &'static str {
    fn write<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self)
    }

    fn is(self, v: &Value) -> bool {
        v.as_str() == Some(self)
    }

    fn to_json(self) -> Value {
        Value::from(self)
    }
}

impl LiteralValue for i64 {
    fn write<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i64(self)
    }

    fn is(self, v: &Value) -> bool {
        v.as_i64() == Some(self)
    }

    fn to_json(self) -> Value {
        Value::from(self)
    }
}

impl LiteralValue for f64 {
    fn write<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error> {
        JsNumber(self).serialize(s)
    }

    fn is(self, v: &Value) -> bool {
        v.as_f64() == Some(self)
    }

    fn to_json(self) -> Value {
        serde_json::to_value(JsNumber(self)).expect("a number writes as JSON")
    }
}

/// Writes the literal as JSON writes it: a string as a string, a number as JavaScript writes it.
pub fn write_literal<S: Serializer, V: LiteralValue>(s: S, value: V) -> Result<S::Ok, S::Error> {
    value.write(s)
}

/// Reads one value and refuses any other, with zod's `invalid_literal` words: `Invalid literal value, expected "x"`.
pub fn read_literal<'de, D: Deserializer<'de>, V: LiteralValue>(d: D, value: V) -> Result<(), D::Error> {
    let got = Value::deserialize(d)?;
    if value.is(&got) {
        return Ok(());
    }
    let expected = value.to_json().to_string();
    Err(serde::de::Error::custom(format!("Invalid literal value, expected {expected}")))
}
